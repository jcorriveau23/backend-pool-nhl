use axum::extract::ws::{Message, WebSocket};
use std::collections::{HashMap, HashSet};
use tokio::sync::broadcast;

use serde::{Deserialize, Serialize};

use crate::{
    errors::AppError,
    pool::model::{Player, Pool, PoolSettings},
};

#[derive(Debug)]
pub struct RoomState {
    pub pool_name: String,

    // Map a user to its ready state.
    pub users: HashMap<String, RoomUser>,
    tx: broadcast::Sender<String>,
}

impl RoomState {
    pub fn new(pool_name: &str) -> Self {
        Self {
            pool_name: pool_name.to_string(),
            users: HashMap::new(),
            tx: broadcast::channel(100).0,
        }
    }

    // Send the pool updated informations to the room.
    pub fn send_pool_info(&self, pool: Pool) -> Result<(), AppError> {
        if let Ok(pool_string) = serde_json::to_string(&CommandResponse::Pool { pool: pool }) {
            let _ = self.tx.send(pool_string);
            return Ok(());
        }
        Err(AppError::CustomError {
            msg: "Could not serialize the pool into a json string.".to_string(),
        })
    }

    // Change the is_ready state of a user and send they updated users informations to the room.
    pub fn on_ready(&mut self, user: &UserToken) -> Result<(), AppError> {
        if let Some(room_user) = self.users.get_mut(&user._id) {
            room_user.is_ready = !room_user.is_ready;
            if let Ok(pool_string) = serde_json::to_string(&CommandResponse::Users {
                room_users: self.users.clone(),
            }) {
                let _ = self.tx.send(pool_string);
                return Ok(());
            }
            return Err(AppError::CustomError {
                msg: "Could not serialize the users info into a json string.".to_string(),
            });
        }
        Err(AppError::CustomError {
            msg: "could not found the user in the room.".to_string(),
        })
    }
}

#[derive(Debug)]
pub struct DraftServerInfo {
    // Mapping of pool names to coresponding room informations.
    pub rooms: HashMap<String, RoomState>,
    // Map a socket id to the user information, these users are authentificated..
    pub authentificated_sockets: HashMap<String, UserToken>,
}

impl DraftServerInfo {
    // Create a new room.
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            authentificated_sockets: HashMap::new(),
        }
    }

    // List the active rooms.
    pub fn list_rooms(&self) -> Vec<String> {
        // Return the list of active rooms.

        self.rooms
            .keys()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    }

    // Add the socket id to the list of authentificated sockets.
    pub fn add_socket(&mut self, socket_id: &str, user_token: UserToken) {
        if !self.authentificated_sockets.contains_key(socket_id) {
            self.authentificated_sockets
                .insert(socket_id.to_string(), user_token);
        }
    }

    // Remove the socket id to the list of authentificated sockets.
    pub fn remove_socket(&mut self, socket_id: &str) {
        if !self.authentificated_sockets.contains_key(socket_id) {
            self.authentificated_sockets.remove(socket_id);
        }
    }

    // Socket command: Join the socket room. (1 room per pool)
    pub fn join_room(
        &mut self,
        pool_name: &str,
        socket_id: &str,
    ) -> (broadcast::Receiver<String>, String) {
        // If the room does not exist create it.
        let room = self
            .rooms
            .entry(pool_name.to_string())
            .or_insert(RoomState::new(pool_name));

        // If the user is authentificated
        if let Some(user) = self.authentificated_sockets.get(socket_id) {
            room.users.insert(
                user._id.clone(),
                RoomUser {
                    _id: user._id.clone(),
                    name: user.name.clone(),
                    is_ready: false,
                },
            );
        }

        // Send the updated users list to the room using the sender.
        // return the receiver even to non authentificated users so they the
        // socket is able to receive update even if the user is not authentificated.

        let users = serde_json::to_string(&CommandResponse::Users {
            room_users: room.users.clone(),
        })
        .unwrap();
        let _ = room.tx.send(users.clone());

        return (room.tx.subscribe(), users);
    }

    // Socket command: Leave the socket room. (1 room per pool)
    pub fn leave_room(&mut self, pool_name: &str, socket_id: &str) {
        if let Some(user) = self.authentificated_sockets.get(socket_id) {
            if let Some(room) = self.rooms.get_mut(pool_name) {
                room.users.remove(&user._id);

                // Send the updated users list to the room.
                // User in the room, will be able to know that
                let _ = room.tx.send(
                    serde_json::to_string(&CommandResponse::Users {
                        room_users: room.users.clone(),
                    })
                    .unwrap(),
                );

                if room.users.len() == 0 {
                    // There is no more user listening to the room, we can remove the room.
                    self.rooms.remove(pool_name);
                }
            }
        }
    }

    // Socket command: Change the is_ready state to true or false.
    // All users in room needs to be ready to start the draft.
    pub fn on_ready(&mut self, pool_name: &str, socket_id: &str) {
        if let Some(user) = self.authentificated_sockets.get(socket_id) {
            if let Some(room) = self.rooms.get_mut(pool_name) {
                room.on_ready(user);
            }
        }
    }
}

// A room authentificated users, There users can make some socket commands.
#[derive(Debug, Serialize, Deserialize, Eq, Hash, Clone)]
pub struct RoomUser {
    pub _id: String,
    pub name: String,
    pub is_ready: bool,
}
#[derive(Debug, Serialize, Deserialize, Eq, Hash, Clone)]
pub struct UserToken {
    // The User token information.
    pub _id: String,
    pub name: String,
}
impl PartialEq for UserToken {
    fn eq(&self, other: &Self) -> bool {
        self._id == other._id
    }
}

impl PartialEq for RoomUser {
    fn eq(&self, other: &Self) -> bool {
        self._id == other._id
    }
}

// Commands that the soket server can receive.
#[derive(Deserialize, Serialize)]
pub enum Command {
    JoinRoom { pool_name: String },
    LeaveRoom,
    OnReady,
    OnPoolSettingChanges { pool_settings: PoolSettings },
    StartDraft,
    UndoDraftPlayer,
    DraftPlayer { player: Player },
}

// Response return to the sockets clients as commands response.
#[derive(Deserialize, Serialize)]
enum CommandResponse {
    Pool {
        pool: Pool,
    },
    Users {
        room_users: HashMap<String, RoomUser>,
    },
    Error {
        message: String,
    },
}
