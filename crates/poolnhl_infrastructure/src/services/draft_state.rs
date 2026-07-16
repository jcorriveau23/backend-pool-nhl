use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use tokio::sync::broadcast;

use poolnhl_interface::draft::model::{CommandResponse, RoomUser};
use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::users::model::UserEmailJwtPayload;

use crate::redis_connection::{RoomSubscriberHandle, ROOM_CHANNEL_PREFIX};

// How long a socket-owned room member survives in redis without a heartbeat
// refresh. Covers instance crashes where leave_room never runs.
const MEMBER_TTL_SECONDS: i64 = 30;

// Heartbeat period; must stay comfortably below MEMBER_TTL_SECONDS.
const HEARTBEAT_PERIOD: Duration = Duration::from_secs(10);

// GC backstop on the room keys themselves, in case every instance of a room
// dies before running its cleanup.
const ROOM_KEY_TTL_SECONDS: i64 = 86400;

fn users_key(pool_name: &str) -> String {
    format!("{}{}:users", ROOM_CHANNEL_PREFIX, pool_name)
}

fn meta_key(pool_name: &str) -> String {
    format!("{}{}:meta", ROOM_CHANNEL_PREFIX, pool_name)
}

fn room_channel(pool_name: &str) -> String {
    format!("{}{}", ROOM_CHANNEL_PREFIX, pool_name)
}

fn redis_err(e: redis::RedisError) -> AppError {
    AppError::RedisError { msg: e.to_string() }
}

fn lock_err<T: std::fmt::Display>(e: T) -> AppError {
    AppError::RwLockError { msg: e.to_string() }
}

fn parse_err(e: serde_json::Error) -> AppError {
    AppError::ParseError { msg: e.to_string() }
}

// Per-room state that only matters to this instance: the local fan-out channel
// feeding the sockets connected here, and which members' presence this
// instance is responsible for keeping alive in redis.
struct LocalRoom {
    tx: broadcast::Sender<String>,
    socket_count: usize,
    owned_member_ids: HashSet<String>,
    // The last member list broadcast to the room, used by the heartbeat to
    // detect members that silently expired (their instance crashed).
    last_users_snapshot: HashMap<String, RoomUser>,
}

impl LocalRoom {
    fn new() -> Self {
        Self {
            tx: broadcast::channel(100).0,
            socket_count: 0,
            owned_member_ids: HashSet::new(),
            last_users_snapshot: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct LocalRooms(Arc<RwLock<HashMap<String, LocalRoom>>>);

impl LocalRooms {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    // Register a socket on the room's local fan-out channel (the room is
    // created on the first local socket). Returns the receiver and whether the
    // room is new on this instance.
    fn register_socket(
        &self,
        pool_name: &str,
        owned_member_id: Option<&str>,
    ) -> Result<(broadcast::Receiver<String>, bool)> {
        let mut rooms = self.0.write().map_err(lock_err)?;
        let is_new = !rooms.contains_key(pool_name);
        let room = rooms
            .entry(pool_name.to_string())
            .or_insert_with(LocalRoom::new);
        room.socket_count += 1;
        if let Some(member_id) = owned_member_id {
            room.owned_member_ids.insert(member_id.to_string());
        }
        Ok((room.tx.subscribe(), is_new))
    }

    // Release a socket from the local room. Returns whether this was the last
    // local socket (the local room was dropped).
    fn release_socket(&self, pool_name: &str, owned_member_id: Option<&str>) -> Result<bool> {
        let mut rooms = self.0.write().map_err(lock_err)?;
        if let Some(room) = rooms.get_mut(pool_name) {
            room.socket_count = room.socket_count.saturating_sub(1);
            if let Some(member_id) = owned_member_id {
                room.owned_member_ids.remove(member_id);
            }
            if room.socket_count == 0 {
                rooms.remove(pool_name);
                return Ok(true);
            }
        }
        Ok(false)
    }

    // Forward a message received from redis pub/sub to the sockets of this
    // instance. Called by the room subscriber task.
    pub fn forward(&self, pool_name: &str, message: String) {
        // Users broadcasts also refresh the reconciliation snapshot, so the
        // heartbeat only re-publishes when members expired without a broadcast.
        let users = match serde_json::from_str::<CommandResponse>(&message) {
            Ok(CommandResponse::Users { room_users }) => Some(room_users),
            _ => None,
        };

        if let Ok(mut rooms) = self.0.write() {
            if let Some(room) = rooms.get_mut(pool_name) {
                if let Some(users) = users {
                    room.last_users_snapshot = users;
                }
                let _ = room.tx.send(message);
            }
        }
    }
}

// Draft server state shared across instances. Room membership/presence lives
// in redis and broadcasts go through redis pub/sub, so any instance can serve
// any socket of a room. Only the socket authentication map stays local: a
// socket is owned by exactly one instance.
pub struct DraftServerState {
    local_rooms: LocalRooms,
    authenticated_sockets: RwLock<HashMap<String, UserEmailJwtPayload>>,
    redis: ConnectionManager,
    subscriber: RoomSubscriberHandle,
}

impl DraftServerState {
    pub fn new(
        local_rooms: LocalRooms,
        redis: ConnectionManager,
        subscriber: RoomSubscriberHandle,
    ) -> Self {
        Self {
            local_rooms,
            authenticated_sockets: RwLock::new(HashMap::new()),
            redis,
            subscriber,
        }
    }

    // Socket authentication (process-local).

    pub fn add_socket(&self, socket_id: &str, user: UserEmailJwtPayload) -> Result<()> {
        self.authenticated_sockets
            .write()
            .map_err(lock_err)?
            .insert(socket_id.to_string(), user);
        Ok(())
    }

    pub fn remove_socket(&self, socket_id: &str) -> Result<()> {
        self.authenticated_sockets
            .write()
            .map_err(lock_err)?
            .remove(socket_id);
        Ok(())
    }

    pub fn get_authenticated_user_with_socket(
        &self,
        socket_id: &str,
    ) -> Result<Option<UserEmailJwtPayload>> {
        Ok(self
            .authenticated_sockets
            .read()
            .map_err(lock_err)?
            .get(socket_id)
            .cloned())
    }

    pub fn list_authenticated_sockets(&self) -> Result<HashMap<String, UserEmailJwtPayload>> {
        Ok(self.authenticated_sockets.read().map_err(lock_err)?.clone())
    }

    // Room membership/presence (redis-backed).

    // Publish a command response to every instance serving the room.
    pub async fn publish(&self, pool_name: &str, response: &CommandResponse) -> Result<()> {
        let message = serde_json::to_string(response).map_err(parse_err)?;
        let mut conn = self.redis.clone();
        let _: () = conn
            .publish(room_channel(pool_name), message)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn fetch_users(&self, pool_name: &str) -> Result<HashMap<String, RoomUser>> {
        let mut conn = self.redis.clone();
        let raw: HashMap<String, String> = conn
            .hgetall(users_key(pool_name))
            .await
            .map_err(redis_err)?;

        raw.into_iter()
            .map(|(user_id, json)| {
                serde_json::from_str::<RoomUser>(&json)
                    .map(|user| (user_id, user))
                    .map_err(parse_err)
            })
            .collect()
    }

    async fn publish_users(&self, pool_name: &str) -> Result<()> {
        let room_users = self.fetch_users(pool_name).await?;
        self.publish(pool_name, &CommandResponse::Users { room_users })
            .await
    }

    // Re-arm the redis field TTL of the given members. Must be called after
    // every HSET too, since overwriting a field clears its TTL.
    async fn refresh_member_ttls(&self, pool_name: &str, member_ids: &[String]) -> Result<()> {
        if member_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.redis.clone();
        let mut cmd = redis::cmd("HEXPIRE");
        cmd.arg(users_key(pool_name))
            .arg(MEMBER_TTL_SECONDS)
            .arg("FIELDS")
            .arg(member_ids.len());
        for member_id in member_ids {
            cmd.arg(member_id);
        }

        let _: redis::Value = cmd.query_async(&mut conn).await.map_err(redis_err)?;
        Ok(())
    }

    // Join the socket to a room: register it on the local fan-out channel and,
    // when authenticated, add the user to the shared member list.
    pub async fn join_room(
        &self,
        pool_name: &str,
        number_poolers: u8,
        socket_id: &str,
    ) -> Result<broadcast::Receiver<String>> {
        let user = self.get_authenticated_user_with_socket(socket_id)?;

        let (rx, is_new_local_room) = self
            .local_rooms
            .register_socket(pool_name, user.as_ref().map(|u| u.sub.as_str()))?;

        if let Err(e) = self
            .join_room_redis(pool_name, number_poolers, user.as_ref(), is_new_local_room)
            .await
        {
            // Roll back the local bookkeeping so the local room does not leak.
            let _ = self
                .local_rooms
                .release_socket(pool_name, user.as_ref().map(|u| u.sub.as_str()));
            return Err(e);
        }

        Ok(rx)
    }

    async fn join_room_redis(
        &self,
        pool_name: &str,
        number_poolers: u8,
        user: Option<&UserEmailJwtPayload>,
        is_new_local_room: bool,
    ) -> Result<()> {
        // Subscribe before writing so this instance receives the Users
        // broadcast produced below.
        if is_new_local_room {
            self.subscriber.subscribe(pool_name).await?;
        }

        let mut conn = self.redis.clone();
        let _: () = conn
            .hset_nx(meta_key(pool_name), "number_poolers", number_poolers)
            .await
            .map_err(redis_err)?;
        let _: () = conn
            .expire(meta_key(pool_name), ROOM_KEY_TTL_SECONDS)
            .await
            .map_err(redis_err)?;

        if let Some(user) = user {
            let member = RoomUser::from_jwt(user);
            let json = serde_json::to_string(&member).map_err(parse_err)?;
            let _: () = conn
                .hset(users_key(pool_name), &member.id, json)
                .await
                .map_err(redis_err)?;
            let _: () = conn
                .expire(users_key(pool_name), ROOM_KEY_TTL_SECONDS)
                .await
                .map_err(redis_err)?;
            self.refresh_member_ttls(pool_name, std::slice::from_ref(&member.id))
                .await?;
        }

        self.publish_users(pool_name).await
    }

    // Called for every socket on disconnect (authenticated or not).
    pub async fn leave_room(&self, pool_name: &str, socket_id: &str) -> Result<()> {
        let user = self.get_authenticated_user_with_socket(socket_id)?;

        let room_dropped = self
            .local_rooms
            .release_socket(pool_name, user.as_ref().map(|u| u.sub.as_str()))?;
        if room_dropped {
            self.subscriber.unsubscribe(pool_name).await?;
        }

        let mut conn = self.redis.clone();
        if let Some(user) = &user {
            let _: () = conn
                .hdel(users_key(pool_name), &user.sub)
                .await
                .map_err(redis_err)?;
        }

        // Drop the room keys entirely once the last member is gone.
        let remaining: i64 = conn.hlen(users_key(pool_name)).await.map_err(redis_err)?;
        if remaining == 0 {
            let _: () = conn
                .del(&[users_key(pool_name), meta_key(pool_name)])
                .await
                .map_err(redis_err)?;
        } else if user.is_some() {
            self.publish_users(pool_name).await?;
        }

        Ok(())
    }

    pub async fn on_ready(&self, pool_name: &str, socket_id: &str) -> Result<()> {
        let user = self.require_authenticated(socket_id)?;

        let mut conn = self.redis.clone();
        let raw: Option<String> = conn
            .hget(users_key(pool_name), &user.sub)
            .await
            .map_err(redis_err)?;
        let raw = raw.ok_or_else(|| AppError::CustomError {
            msg: format!("The user is not a member of the room '{}'.", pool_name),
        })?;

        let mut member: RoomUser = serde_json::from_str(&raw).map_err(parse_err)?;
        member.is_ready = !member.is_ready;
        let json = serde_json::to_string(&member).map_err(parse_err)?;
        let _: () = conn
            .hset(users_key(pool_name), &user.sub, json)
            .await
            .map_err(redis_err)?;
        self.refresh_member_ttls(pool_name, std::slice::from_ref(&user.sub))
            .await?;

        self.publish_users(pool_name).await
    }

    // Add a member that is not backed by any socket (no presence TTL; it stays
    // until removed explicitly or the room is deleted).
    pub async fn add_user(&self, pool_name: &str, user_name: &str, socket_id: &str) -> Result<()> {
        self.require_authenticated(socket_id)?;

        let users = self.fetch_users(pool_name).await?;
        if users.values().any(|user| user.name == user_name) {
            return Err(AppError::CustomError {
                msg: format!("There is already a user with the name {}", user_name),
            });
        }

        let member = RoomUser::new_unmanaged(user_name);
        let json = serde_json::to_string(&member).map_err(parse_err)?;
        let mut conn = self.redis.clone();
        let _: () = conn
            .hset(users_key(pool_name), &member.id, json)
            .await
            .map_err(redis_err)?;
        let _: () = conn
            .expire(users_key(pool_name), ROOM_KEY_TTL_SECONDS)
            .await
            .map_err(redis_err)?;

        self.publish_users(pool_name).await
    }

    pub async fn remove_user(&self, pool_name: &str, user_id: &str, socket_id: &str) -> Result<()> {
        self.require_authenticated(socket_id)?;

        let mut conn = self.redis.clone();
        let _: () = conn
            .hdel(users_key(pool_name), user_id)
            .await
            .map_err(redis_err)?;

        self.publish_users(pool_name).await
    }

    fn require_authenticated(&self, socket_id: &str) -> Result<UserEmailJwtPayload> {
        self.get_authenticated_user_with_socket(socket_id)?
            .ok_or_else(|| AppError::CustomError {
                msg: "The user is not authenticated".to_string(),
            })
    }

    pub async fn get_room_users(&self, pool_name: &str) -> Result<Vec<RoomUser>> {
        Ok(self
            .fetch_users(pool_name)
            .await?
            .into_values()
            .collect())
    }

    pub async fn list_room_users(&self, pool_name: &str) -> Result<HashMap<String, RoomUser>> {
        self.fetch_users(pool_name).await
    }

    pub async fn list_rooms(&self) -> Result<Vec<String>> {
        let mut conn = self.redis.clone();
        let pattern = format!("{}*:users", ROOM_CHANNEL_PREFIX);
        let keys: Vec<String> = {
            let mut iter = conn
                .scan_match::<_, String>(pattern)
                .await
                .map_err(redis_err)?;
            let mut keys = Vec::new();
            while let Some(key) = iter.next_item().await {
                keys.push(key);
            }
            keys
        };

        Ok(keys
            .iter()
            .filter_map(|key| {
                key.strip_prefix(ROOM_CHANNEL_PREFIX)
                    .and_then(|key| key.strip_suffix(":users"))
                    .map(|pool_name| pool_name.to_string())
            })
            .collect())
    }

    // One reconciliation pass: keep this instance's members alive in redis and
    // notify rooms whose member list changed without a broadcast (typically
    // members expired because their instance crashed).
    // Public so integration tests can drive the reconciliation deterministically.
    pub async fn heartbeat_tick(&self) -> Result<()> {
        let rooms: Vec<(String, Vec<String>, HashMap<String, RoomUser>)> = {
            let rooms = self.local_rooms.0.read().map_err(lock_err)?;
            rooms
                .iter()
                .map(|(pool_name, room)| {
                    (
                        pool_name.clone(),
                        room.owned_member_ids.iter().cloned().collect(),
                        room.last_users_snapshot.clone(),
                    )
                })
                .collect()
        };

        for (pool_name, owned_member_ids, snapshot) in rooms {
            self.refresh_member_ttls(&pool_name, &owned_member_ids)
                .await?;

            let current = self.fetch_users(&pool_name).await?;
            // RoomUser equality is id-based, so this compares membership (which
            // is what expiration changes); is_ready changes always broadcast.
            if current != snapshot {
                self.publish(&pool_name, &CommandResponse::Users { room_users: current })
                    .await?;
            }
        }

        Ok(())
    }
}

pub fn spawn_heartbeat(state: Arc<DraftServerState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(HEARTBEAT_PERIOD);
        loop {
            interval.tick().await;
            if let Err(e) = state.heartbeat_tick().await {
                println!("draft room heartbeat error: {}", e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_release_socket_bookkeeping() {
        let rooms = LocalRooms::new();

        let (_rx1, is_new) = rooms.register_socket("pool", Some("user-1")).unwrap();
        assert!(is_new);
        let (_rx2, is_new) = rooms.register_socket("pool", None).unwrap();
        assert!(!is_new);
        {
            let guard = rooms.0.read().unwrap();
            let room = guard.get("pool").unwrap();
            assert_eq!(room.socket_count, 2);
            assert!(room.owned_member_ids.contains("user-1"));
        }

        // First release keeps the room alive and drops the owned member.
        assert!(!rooms.release_socket("pool", Some("user-1")).unwrap());
        {
            let guard = rooms.0.read().unwrap();
            let room = guard.get("pool").unwrap();
            assert_eq!(room.socket_count, 1);
            assert!(room.owned_member_ids.is_empty());
        }

        // Last release drops the room entirely.
        assert!(rooms.release_socket("pool", None).unwrap());
        assert!(rooms.0.read().unwrap().is_empty());
    }

    #[test]
    fn release_socket_on_unknown_room_is_a_noop() {
        let rooms = LocalRooms::new();
        assert!(!rooms.release_socket("nope", Some("user-1")).unwrap());
    }

    #[test]
    fn forward_broadcasts_to_local_subscribers() {
        let rooms = LocalRooms::new();
        let (mut rx, _) = rooms.register_socket("pool", None).unwrap();

        rooms.forward("pool", "not even json".to_string());
        assert_eq!(rx.try_recv().unwrap(), "not even json");
    }

    #[test]
    fn forward_users_message_refreshes_snapshot() {
        let rooms = LocalRooms::new();
        let (mut rx, _) = rooms.register_socket("pool", None).unwrap();

        let user = RoomUser::new_unmanaged("Guest");
        let room_users = HashMap::from([(user.id.clone(), user.clone())]);
        let message = serde_json::to_string(&CommandResponse::Users { room_users }).unwrap();

        rooms.forward("pool", message.clone());

        assert_eq!(rx.try_recv().unwrap(), message);
        let guard = rooms.0.read().unwrap();
        let snapshot = &guard.get("pool").unwrap().last_users_snapshot;
        assert!(snapshot.contains_key(&user.id));
    }

    #[test]
    fn forward_to_unknown_room_is_dropped() {
        let rooms = LocalRooms::new();
        // Must not panic nor create a room.
        rooms.forward("nope", "message".to_string());
        assert!(rooms.0.read().unwrap().is_empty());
    }
}
