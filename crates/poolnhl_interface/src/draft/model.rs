use std::collections::HashMap;
use tokio::sync::broadcast;

use serde::{Deserialize, Serialize};

use crate::{
    errors::AppError,
    pool::model::{Player, Pool, PoolSettings},
    users::model::UserEmailJwtPayload,
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
        if let Ok(pool_string) = serde_json::to_string(&CommandResponse::Pool { pool }) {
            let _ = self.tx.send(pool_string);
            return Ok(());
        }
        Err(AppError::CustomError {
            msg: "Could not serialize the pool into a json string.".to_string(),
        })
    }

    // Change the is_ready state of a user and send they updated users informations to the room.
    pub fn on_ready(&mut self, user: &UserEmailJwtPayload) -> Result<(), AppError> {
        if let Some(room_user) = self.users.get_mut(&user.sub) {
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
    pub authentificated_sockets: HashMap<String, UserEmailJwtPayload>,
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
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    }

    // Add the socket id to the list of authentificated sockets.
    pub fn add_socket(&mut self, socket_id: &str, user_token: UserEmailJwtPayload) {
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
                user.sub.clone(),
                RoomUser {
                    id: user.sub.clone(),
                    email: user.email.address.clone(),
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

        (room.tx.subscribe(), users)
    }

    // Socket command: Leave the socket room. (1 room per pool)
    pub fn leave_room(&mut self, pool_name: &str, socket_id: &str) {
        if let Some(user) = self.authentificated_sockets.get(socket_id) {
            if let Some(room) = self.rooms.get_mut(pool_name) {
                room.users.remove(&user.sub);

                // Send the updated users list to the room.
                // User in the room, will be able to know that
                let _ = room.tx.send(
                    serde_json::to_string(&CommandResponse::Users {
                        room_users: room.users.clone(),
                    })
                    .unwrap(),
                );

                if room.users.is_empty() {
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
                let _ = room.on_ready(user);
            }
        }
    }
}

// A room authentificated users, There users can make some socket commands.
#[derive(Debug, Serialize, Deserialize, Eq, Clone)]
pub struct RoomUser {
    pub id: String,
    pub email: String,
    pub is_ready: bool,
}

impl PartialEq for RoomUser {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
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
