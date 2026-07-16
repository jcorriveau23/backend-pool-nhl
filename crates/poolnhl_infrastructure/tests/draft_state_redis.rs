//! Integration tests for the redis-backed draft room state.
//!
//! They need a running redis >= 7.4 (hash field TTLs):
//!   docker compose up -d redis
//!   cargo test -p poolnhl_infrastructure -- --ignored
//!
//! Each test builds its own DraftServerState instances (one per simulated
//! server) against a uniquely-named room, so tests can run in parallel.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redis::AsyncCommands;
use tokio::sync::broadcast;

use poolnhl_infrastructure::redis_connection::{spawn_room_subscriber, RedisManager};
use poolnhl_infrastructure::services::draft_state::{DraftServerState, LocalRooms};
use poolnhl_interface::draft::model::{CommandResponse, RoomUser};
use poolnhl_interface::users::model::{EmailInfo, UserEmailJwtPayload};

fn redis_uri() -> String {
    std::env::var("TEST_REDIS_URI").unwrap_or_else(|_| "redis://localhost:6379".to_string())
}

// One DraftServerState per simulated server instance.
async fn new_instance() -> Arc<DraftServerState> {
    let (client, conn) = RedisManager::connect(&redis_uri())
        .await
        .expect("redis is not reachable; start it with `docker compose up -d redis`");
    let local_rooms = LocalRooms::new();
    let subscriber = spawn_room_subscriber(client, local_rooms.clone());
    Arc::new(DraftServerState::new(local_rooms, conn, subscriber))
}

async fn raw_redis() -> redis::aio::ConnectionManager {
    RedisManager::connect(&redis_uri()).await.unwrap().1
}

fn unique_pool(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test-{}-{}", prefix, nanos)
}

fn jwt_payload(user_id: &str) -> UserEmailJwtPayload {
    UserEmailJwtPayload {
        aud: vec!["test".to_string()],
        email: EmailInfo {
            address: format!("{}@example.com", user_id),
            is_primary: true,
            is_verified: true,
        },
        exp: 0,
        iat: 0,
        sub: user_id.to_string(),
    }
}

// Receive broadcasts until one is a Users message matching the predicate.
async fn wait_for_users(
    rx: &mut broadcast::Receiver<String>,
    predicate: impl Fn(&HashMap<String, RoomUser>) -> bool,
) -> HashMap<String, RoomUser> {
    let deadline = Duration::from_secs(5);
    loop {
        let message = tokio::time::timeout(deadline, rx.recv())
            .await
            .expect("timed out waiting for a room broadcast")
            .expect("the room broadcast channel closed");
        if let Ok(CommandResponse::Users { room_users }) = serde_json::from_str(&message) {
            if predicate(&room_users) {
                return room_users;
            }
        }
    }
}

// Field TTLs of the room members, as reported by HTTL (-1 = no TTL).
async fn member_ttl(pool_name: &str, member_id: &str) -> i64 {
    let mut conn = raw_redis().await;
    let ttls: Vec<i64> = redis::cmd("HTTL")
        .arg(format!("draft:room:{}:users", pool_name))
        .arg("FIELDS")
        .arg(1)
        .arg(member_id)
        .query_async(&mut conn)
        .await
        .unwrap();
    ttls[0]
}

#[tokio::test]
#[ignore = "requires a running redis (docker compose up -d redis)"]
async fn join_and_leave_room_lifecycle() {
    let pool = unique_pool("lifecycle");
    let instance = new_instance().await;

    instance.add_socket("socket-1", jwt_payload("user-1")).unwrap();
    let mut rx = instance.join_room(&pool, 4, "socket-1").await.unwrap();

    // The join broadcast comes back through redis pub/sub with the member.
    let users = wait_for_users(&mut rx, |users| users.contains_key("user-1")).await;
    assert!(!users["user-1"].is_ready);

    // Socket-owned members carry a presence TTL; the meta hash is populated.
    let ttl = member_ttl(&pool, "user-1").await;
    assert!(ttl > 0 && ttl <= 30, "expected a presence TTL, got {}", ttl);
    let mut conn = raw_redis().await;
    let number_poolers: u8 = conn
        .hget(format!("draft:room:{}:meta", pool), "number_poolers")
        .await
        .unwrap();
    assert_eq!(number_poolers, 4);

    // Last member leaving deletes the room keys.
    instance.leave_room(&pool, "socket-1").await.unwrap();
    let users_exists: bool = conn
        .exists(format!("draft:room:{}:users", pool))
        .await
        .unwrap();
    let meta_exists: bool = conn
        .exists(format!("draft:room:{}:meta", pool))
        .await
        .unwrap();
    assert!(!users_exists && !meta_exists);
}

#[tokio::test]
#[ignore = "requires a running redis (docker compose up -d redis)"]
async fn broadcasts_and_membership_cross_instances() {
    let pool = unique_pool("cross-instance");
    let instance_a = new_instance().await;
    let instance_b = new_instance().await;

    instance_a.add_socket("socket-a", jwt_payload("user-a")).unwrap();
    let mut rx_a = instance_a.join_room(&pool, 4, "socket-a").await.unwrap();
    wait_for_users(&mut rx_a, |users| users.contains_key("user-a")).await;

    // A second user joins through the other instance; the first instance's
    // socket must see it.
    instance_b.add_socket("socket-b", jwt_payload("user-b")).unwrap();
    let _rx_b = instance_b.join_room(&pool, 4, "socket-b").await.unwrap();
    wait_for_users(&mut rx_a, |users| {
        users.contains_key("user-a") && users.contains_key("user-b")
    })
    .await;

    // A readiness toggle on instance B reaches instance A's socket.
    instance_b.on_ready(&pool, "socket-b").await.unwrap();
    let users = wait_for_users(&mut rx_a, |users| {
        users.get("user-b").map_or(false, |user| user.is_ready)
    })
    .await;
    assert!(!users["user-a"].is_ready);

    // Readiness re-arms the presence TTL (HSET clears field TTLs).
    let ttl = member_ttl(&pool, "user-b").await;
    assert!(ttl > 0, "expected the presence TTL to be re-armed");

    // Leaving on instance B is seen by instance A's socket. (B's own receiver
    // closes here: dropping the last local socket drops the local room.)
    instance_b.leave_room(&pool, "socket-b").await.unwrap();
    wait_for_users(&mut rx_a, |users| !users.contains_key("user-b")).await;
    instance_a.leave_room(&pool, "socket-a").await.unwrap();
}

#[tokio::test]
#[ignore = "requires a running redis (docker compose up -d redis)"]
async fn unmanaged_users_have_no_presence_ttl() {
    let pool = unique_pool("unmanaged");
    let instance = new_instance().await;

    instance.add_socket("socket-1", jwt_payload("user-1")).unwrap();
    let mut rx = instance.join_room(&pool, 4, "socket-1").await.unwrap();

    instance.add_user(&pool, "Guest", "socket-1").await.unwrap();
    let users = wait_for_users(&mut rx, |users| {
        users.values().any(|user| user.name == "Guest")
    })
    .await;
    let guest = users.values().find(|user| user.name == "Guest").unwrap();
    assert!(guest.is_ready);

    // Unmanaged members are not socket-backed: no presence TTL.
    assert_eq!(member_ttl(&pool, &guest.id).await, -1);

    // Duplicate names are rejected.
    let duplicate = instance.add_user(&pool, "Guest", "socket-1").await;
    assert!(duplicate.is_err());

    // RemoveUser drops the member for everyone.
    let guest_id = guest.id.clone();
    instance.remove_user(&pool, &guest_id, "socket-1").await.unwrap();
    wait_for_users(&mut rx, |users| !users.contains_key(&guest_id)).await;

    instance.leave_room(&pool, "socket-1").await.unwrap();
}

#[tokio::test]
#[ignore = "requires a running redis (docker compose up -d redis)"]
async fn heartbeat_republishes_after_member_expiry() {
    let pool = unique_pool("expiry");
    let instance_a = new_instance().await;
    let instance_b = new_instance().await;

    // user-a joins through instance A; instance B serves an unauthenticated
    // spectator socket in the same room.
    instance_a.add_socket("socket-a", jwt_payload("user-a")).unwrap();
    let _rx_a = instance_a.join_room(&pool, 4, "socket-a").await.unwrap();
    let mut rx_b = instance_b.join_room(&pool, 4, "socket-b").await.unwrap();
    wait_for_users(&mut rx_b, |users| users.contains_key("user-a")).await;

    // Simulate instance A crashing: its heartbeat never runs again, so user-a's
    // presence TTL expires (forced to 1s here to keep the test fast).
    let mut conn = raw_redis().await;
    let _: redis::Value = redis::cmd("HEXPIRE")
        .arg(format!("draft:room:{}:users", pool))
        .arg(1)
        .arg("FIELDS")
        .arg(1)
        .arg("user-a")
        .query_async(&mut conn)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Instance B's reconciliation notices the expiry and tells its sockets.
    instance_b.heartbeat_tick().await.unwrap();
    wait_for_users(&mut rx_b, |users| !users.contains_key("user-a")).await;

    instance_b.leave_room(&pool, "socket-b").await.unwrap();
}
