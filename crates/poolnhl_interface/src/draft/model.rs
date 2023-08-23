use std::collections::{HashMap, HashSet};
use tokio::sync::broadcast;

use serde::{Deserialize, Serialize};

use crate::pool::model::{Player, Pool};

// payload to sent when deleting a pool.
#[derive(Debug, Deserialize)]
pub struct StartDraftRequest {
    pub pool: Pool,
}

// payload to sent when undoing a selection in a pool by the owner.
#[derive(Debug, Deserialize)]
pub struct UndoSelectionRequest {
    pub pool_name: String,
}
// payload to sent when selecting a player.
#[derive(Debug, Deserialize)]
pub struct SelectPlayerRequest {
    pub pool_name: String,
    pub player: Player,
}

#[derive(Debug)]
pub struct RoomState {
    pub pool_name: String,
    pub users: HashSet<UserToken>,
    tx: broadcast::Sender<String>,
}

impl RoomState {
    pub fn new(pool_name: &str) -> Self {
        Self {
            pool_name: pool_name.to_string(),
            users: HashSet::new(),
            tx: broadcast::channel(69).0,
        }
    }
}

#[derive(Debug)]
pub struct DraftServerInfo {
    // Mapping of pool names to coresponding room informations.
    pub rooms: HashMap<String, RoomState>,
    // Map a socket id to a user information.
    pub authentificated_sockets: HashMap<String, UserToken>,
}

impl DraftServerInfo {
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            authentificated_sockets: HashMap::new(),
        }
    }

    pub fn list_rooms(&self) -> Vec<String> {
        // Return the list of active rooms.

        self.rooms
            .keys()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    }

    pub fn add_socket(&mut self, socket_id: &str, user_token: UserToken) {
        // Add the socket id to the list of authentificated sockets.

        if !self.authentificated_sockets.contains_key(socket_id) {
            self.authentificated_sockets
                .insert(socket_id.to_string(), user_token);
        }
    }

    pub fn remove_socket(&mut self, socket_id: &str) {
        // Add the socket id to the list of authentificated sockets.

        if !self.authentificated_sockets.contains_key(socket_id) {
            self.authentificated_sockets.remove(socket_id);
        }
    }

    pub fn join_room(&mut self, pool_name: &str, socket_id: &str) {
        // Join a room if authentificated.
        // If the room does not exist create it.

        println!("user '{}' joining room '{}'\n", socket_id, pool_name);

        if let Some(user) = self.authentificated_sockets.get(socket_id) {
            let room = self
                .rooms
                .entry(pool_name.to_string())
                .or_insert(RoomState::new(pool_name));

            room.users.insert(user.clone());
        }
    }

    pub fn leave_room(&mut self, pool_name: &str, socket_id: &str) {
        // Leave the room.

        if let Some(user) = self.authentificated_sockets.get(socket_id) {
            if let Some(room) = self.rooms.get_mut(pool_name) {
                room.users.remove(user);

                if self.rooms.len() == 0 {
                    // There is no more user in the room, we can remove the room.
                    self.rooms.remove(pool_name);
                }
            }
        }
    }
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
