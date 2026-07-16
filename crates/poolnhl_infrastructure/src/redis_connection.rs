use std::collections::HashSet;
use std::time::Duration;

use futures::StreamExt;
use redis::aio::ConnectionManager;
use tokio::sync::mpsc;

use poolnhl_interface::errors::{AppError, Result};

use crate::services::draft_state::LocalRooms;

// Prefix of the per-room pub/sub channels (`draft:room:{pool_name}`) and of the
// room state keys (`draft:room:{pool_name}:users` / `:meta`).
pub const ROOM_CHANNEL_PREFIX: &str = "draft:room:";

fn room_channel(pool_name: &str) -> String {
    format!("{}{}", ROOM_CHANNEL_PREFIX, pool_name)
}

pub struct RedisManager;

impl RedisManager {
    pub async fn connect(redis_uri: &str) -> Result<(redis::Client, ConnectionManager)> {
        let client = redis::Client::open(redis_uri)
            .map_err(|e| AppError::RedisError { msg: e.to_string() })?;

        // ConnectionManager is cheaply cloneable and reconnects on its own. It is
        // used for all regular commands; pub/sub needs its own connection (below).
        let manager = ConnectionManager::new(client.clone())
            .await
            .map_err(|e| AppError::RedisError { msg: e.to_string() })?;

        Ok((client, manager))
    }
}

enum SubscriberCmd {
    Subscribe(String),
    Unsubscribe(String),
}

// Handle to the background pub/sub task. Rooms with at least one socket on this
// instance are subscribed so their broadcasts reach the local sockets.
#[derive(Clone)]
pub struct RoomSubscriberHandle {
    control_tx: mpsc::Sender<SubscriberCmd>,
}

impl RoomSubscriberHandle {
    pub async fn subscribe(&self, pool_name: &str) -> Result<()> {
        self.control_tx
            .send(SubscriberCmd::Subscribe(room_channel(pool_name)))
            .await
            .map_err(|e| AppError::RedisError { msg: e.to_string() })
    }

    pub async fn unsubscribe(&self, pool_name: &str) -> Result<()> {
        self.control_tx
            .send(SubscriberCmd::Unsubscribe(room_channel(pool_name)))
            .await
            .map_err(|e| AppError::RedisError { msg: e.to_string() })
    }
}

// Spawn the single pub/sub connection of this instance. Every message received
// on a room channel is forwarded to the room's local broadcast channel.
pub fn spawn_room_subscriber(client: redis::Client, local_rooms: LocalRooms) -> RoomSubscriberHandle {
    let (control_tx, mut control_rx) = mpsc::channel::<SubscriberCmd>(64);

    tokio::spawn(async move {
        // The channels this instance must stay subscribed to; kept here so they
        // can be re-subscribed after a connection loss.
        let mut channels: HashSet<String> = HashSet::new();

        'reconnect: loop {
            let pubsub = match client.get_async_pubsub().await {
                Ok(pubsub) => pubsub,
                Err(e) => {
                    println!("redis pub/sub connection failed: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue 'reconnect;
                }
            };

            // Split so we can keep subscribing/unsubscribing while consuming messages.
            let (mut sink, mut stream) = pubsub.split();

            for channel in &channels {
                if let Err(e) = sink.subscribe(channel).await {
                    println!("redis re-subscribe to '{}' failed: {}", channel, e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue 'reconnect;
                }
            }

            loop {
                tokio::select! {
                    cmd = control_rx.recv() => match cmd {
                        Some(SubscriberCmd::Subscribe(channel)) => {
                            if channels.insert(channel.clone()) {
                                if let Err(e) = sink.subscribe(&channel).await {
                                    println!("redis subscribe to '{}' failed: {}", channel, e);
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    continue 'reconnect;
                                }
                            }
                        }
                        Some(SubscriberCmd::Unsubscribe(channel)) => {
                            if channels.remove(&channel) {
                                if let Err(e) = sink.unsubscribe(&channel).await {
                                    println!("redis unsubscribe from '{}' failed: {}", channel, e);
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    continue 'reconnect;
                                }
                            }
                        }
                        // Every handle was dropped: the services are gone, stop the task.
                        None => return,
                    },
                    msg = stream.next() => match msg {
                        Some(msg) => {
                            if let Some(pool_name) =
                                msg.get_channel_name().strip_prefix(ROOM_CHANNEL_PREFIX)
                            {
                                match msg.get_payload::<String>() {
                                    Ok(payload) => local_rooms.forward(pool_name, payload),
                                    Err(e) => println!("invalid redis pub/sub payload: {}", e),
                                }
                            }
                        }
                        // The pub/sub connection dropped; reconnect and re-subscribe.
                        None => {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue 'reconnect;
                        }
                    },
                }
            }
        }
    });

    RoomSubscriberHandle { control_tx }
}
