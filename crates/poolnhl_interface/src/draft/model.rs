use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::RwLock};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    errors::AppError,
    pool::model::{Pool, PoolPlayerInfo, PoolSettings},
    users::model::UserEmailJwtPayload,
};

#[derive(Debug, Clone)]
pub struct RoomState {
    pub pool_name: String,
    pub number_poolers: u8,

    // Map a user id to its informations room information.
    pub users: HashMap<String, RoomUser>,
    tx: broadcast::Sender<String>,
}

impl RoomState {
    pub fn new(pool_name: &str, number_poolers: u8) -> Self {
        Self {
            pool_name: pool_name.to_string(),
            number_poolers,
            users: HashMap::new(),
            tx: broadcast::channel(100).0,
        }
    }

    pub fn add_user(&mut self, user: &UserEmailJwtPayload) -> () {
        // Add a user to a room.
        self.users.insert(
            user.sub.to_string(),
            RoomUser {
                id: user.sub.to_string(),
                name: user.email.address.to_string(),
                email: Some(user.email.address.to_string()),
                is_ready: false,
            },
        );
    }

    pub fn add_unmanaged_user(&mut self, user_name: &str) -> () {
        // Add a user to a room.
        let user_id = Uuid::new_v4();
        self.users.insert(
            user_id.to_string(),
            RoomUser {
                id: user_id.to_string(),
                name: user_name.to_string(),
                email: None,
                is_ready: true,
            },
        );
    }

    pub fn remove_user(&mut self, user_id: &str) -> () {
        // Add a user to a room.
        self.users.remove(user_id);
    }

    pub fn on_ready(&mut self, user_id: &str) -> () {
        // Change the is_ready state of a user and send they updated users informations to the room.
        if let Some(room_user) = self.users.get_mut(user_id) {
            room_user.is_ready = !room_user.is_ready;
        }
    }
}

#[derive(Debug)]
pub struct DraftServerInfo {
    // Mapping of pool names to its corresponding room informations.
    pub rooms: RwLock<HashMap<String, RoomState>>,

    // Map a socket id to the user information, these list only authenticated users are authenticated.
    pub authenticated_sockets: RwLock<HashMap<String, UserEmailJwtPayload>>,
}

impl DraftServerInfo {
    // Create a new room.
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
            authenticated_sockets: RwLock::new(HashMap::new()),
        }
    }

    pub fn is_user_in_room(&self, user_id: &str, pool_name: &str) -> Result<bool, AppError> {
        // Tells us if the user is in room. Read lock without copy.
        Ok(self
            .rooms
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .get(pool_name)
            .map_or(false, |room| room.users.contains_key(user_id)))
    }

    pub fn list_rooms(&self) -> Result<Vec<String>, AppError> {
        // Return the list of active rooms ascopies. (only callable to debug)

        Ok(self
            .rooms
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .keys()
            .map(|s| s.to_string())
            .collect::<Vec<String>>())
    }

    pub fn list_room_users(&self, pool_name: &str) -> Result<HashMap<String, RoomUser>, AppError> {
        self.rooms
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .get(pool_name)
            .map(|room_state| room_state.users.clone())
            .ok_or(AppError::CustomError {
                msg: format!("The room {} does not exist.", pool_name),
            })
    }

    pub fn list_authenticated_sockets(
        &self,
    ) -> Result<HashMap<String, UserEmailJwtPayload>, AppError> {
        Ok(self
            .authenticated_sockets
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .clone())
    }

    pub fn get_room_tx(&self, pool_name: &str) -> Result<broadcast::Sender<String>, AppError> {
        // Return the room tx sender as copy to avoid locking readlock the room to long.
        // The tx is very lightweight it contains an Arc. The goal to limit the amount of time read locking whole rooms.
        let rooms = self
            .rooms
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?;

        let room = rooms.get(pool_name).ok_or(AppError::CustomError {
            msg: format!("Room '{}' could not be found.", pool_name),
        })?;

        Ok(room.tx.clone())
    }

    pub fn get_room_users(&self, pool_name: &str) -> Result<Vec<RoomUser>, AppError> {
        // Return the list of the room users as copy. There is a limit of 20 users per room.
        let rooms = self
            .rooms
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?;

        let room = rooms.get(pool_name).ok_or(AppError::CustomError {
            msg: format!("Room '{}' could not be found.", pool_name),
        })?;

        Ok(room.users.values().cloned().collect())
    }

    pub fn is_socket_authenticated(&self, socket_id: &str) -> Result<bool, AppError> {
        // Tells if the socket is autenticated. Read lock without any copies.
        Ok(self
            .authenticated_sockets
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .contains_key(socket_id))
    }

    pub fn get_authenticated_user_with_socket(
        &self,
        socket_id: &str,
    ) -> Result<Option<UserEmailJwtPayload>, AppError> {
        // Get the a copy of the authenticated user associated with a socket id.
        Ok(self
            .authenticated_sockets
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .get(socket_id)
            .cloned())
    }

    pub fn is_room_created(&self, pool_name: &str) -> Result<bool, AppError> {
        Ok(self
            .rooms
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .contains_key(pool_name))
    }

    pub fn add_user_to_room(
        &self,
        user: &UserEmailJwtPayload,
        pool_name: &str,
        number_poolers: u8,
    ) -> Result<(), AppError> {
        let mut rooms = self
            .rooms
            .write()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?;

        let room = rooms
            .entry(pool_name.to_string())
            .or_insert_with(|| RoomState {
                pool_name: pool_name.to_string(),
                number_poolers,
                users: HashMap::new(),
                tx: broadcast::channel(24).0,
            });

        room.add_user(user);

        Ok(())
    }

    pub fn remove_user_from_room(
        &self,
        user_id: &str,
        pool_name: &str,
    ) -> Result<HashMap<String, RoomUser>, AppError> {
        if self.is_user_in_room(user_id, pool_name)? {
            let mut rooms = self
                .rooms
                .write()
                .map_err(|e| AppError::RwLockError { msg: e.to_string() })?;

            match rooms.get_mut(pool_name) {
                Some(room) => {
                    room.users.remove(user_id);

                    let room_users = room.users.clone();
                    // If the room is empty, we can delete the room.
                    if room.users.is_empty() {
                        rooms.remove(pool_name);
                    }

                    return Ok(room_users);
                }
                None => {
                    return Err(AppError::CustomError {
                        msg: format!("The room '{}' was not found.", pool_name),
                    })
                }
            }
        }

        Ok(HashMap::new())
    }

    pub fn add_socket(
        &self,
        socket_id: &str,
        user_token: UserEmailJwtPayload,
    ) -> Result<(), AppError> {
        // Add the socket id to the list of authenticated sockets.
        if !self.is_socket_authenticated(socket_id)? {
            self.authenticated_sockets
                .write()
                .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
                .insert(socket_id.to_string(), user_token);
        }
        Ok(())
    }

    pub fn remove_socket(&self, socket_id: &str) -> Result<(), AppError> {
        // Remove the socket id to the list of authenticated sockets.
        if self.is_socket_authenticated(socket_id)? {
            self.authenticated_sockets
                .write()
                .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
                .remove(socket_id);
        }
        Ok(())
    }

    pub fn join_room(
        &self,
        pool_name: &str,
        number_poolers: u8,
        socket_id: &str,
    ) -> Result<(broadcast::Receiver<String>, HashMap<String, RoomUser>), AppError> {
        // Socket command: Join the socket room. (1 room per pool)

        // If the user is authenticated, add the user to the room.
        if let Some(user) = self.get_authenticated_user_with_socket(socket_id)? {
            self.add_user_to_room(&user, pool_name, number_poolers)?
        }

        let (room_tx, room_users) = {
            // Scope the read lock to a block to ensure it's released as soon as possible
            match self
                .rooms
                .read()
                .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
                .get(pool_name)
            {
                Some(room) => (room.tx.clone(), room.users.clone()),
                None => {
                    return Err(AppError::RwLockError {
                        msg: "The room could not be found.".to_string(),
                    })
                }
            }
        };

        // Send the updated users list to the room using the sender.
        // return the receiver even to non authenticated users so the user socket is able to receive pool updates
        // even if the user is not authenticated.

        Ok((room_tx.subscribe(), room_users))
    }

    pub fn leave_room(
        &self,
        pool_name: &str,
        socket_id: &str,
    ) -> Result<HashMap<String, RoomUser>, AppError> {
        // Socket command: Leave the socket room. (1 room per pool)
        match self.get_authenticated_user_with_socket(socket_id)? {
            Some(user) => Ok(self.remove_user_from_room(&user.sub, pool_name)?),
            None => Err(AppError::CustomError {
                msg: format!(
                    "user with socket id '{}' is not authentificated in the pool '{}'",
                    socket_id, pool_name
                ),
            }),
        }
    }

    pub fn on_ready(
        &self,
        pool_name: &str,
        socket_id: &str,
    ) -> Result<HashMap<String, RoomUser>, AppError> {
        // Socket command: Change the is_ready state to true or false.
        // All users in room needs to be ready to start the draft.
        if let Some(user) = self.get_authenticated_user_with_socket(socket_id)? {
            if self.is_room_created(pool_name)? {
                let mut rooms = self
                    .rooms
                    .write()
                    .map_err(|e| AppError::RwLockError { msg: e.to_string() })?;

                let room = rooms.get_mut(pool_name).ok_or(AppError::CustomError {
                    msg: format!("Room '{}' could not be found.", pool_name),
                })?;

                room.on_ready(&user.sub);
                return Ok(room.users.clone());
            }
        }
        Err(AppError::CustomError {
            msg: "The user is not authenticated".to_string(),
        })
    }

    pub fn add_user(
        &self,
        pool_name: &str,
        user_name: &str,
        socket_id: &str,
    ) -> Result<HashMap<String, RoomUser>, AppError> {
        if let Some(user) = self.get_authenticated_user_with_socket(socket_id)? {
            if self.is_room_created(pool_name)? {
                let mut rooms = self
                    .rooms
                    .write()
                    .map_err(|e| AppError::RwLockError { msg: e.to_string() })?;

                let room = rooms.get_mut(pool_name).ok_or(AppError::CustomError {
                    msg: format!("Room '{}' could not be found.", pool_name),
                })?;

                if room.users.values().any(|user| user.name == user_name) {
                    return Err(AppError::CustomError {
                        msg: format!("There is already a user with the name {}", user_name),
                    });
                }
                room.add_unmanaged_user(&user_name);
                return Ok(room.users.clone());
            }
        }
        Err(AppError::CustomError {
            msg: "The user is not authenticated".to_string(),
        })
    }

    pub fn remove_user(
        &self,
        pool_name: &str,
        user_id: &str,
        socket_id: &str,
    ) -> Result<HashMap<String, RoomUser>, AppError> {
        if let Some(user) = self.get_authenticated_user_with_socket(socket_id)? {
            if self.is_room_created(pool_name)? {
                let mut rooms = self
                    .rooms
                    .write()
                    .map_err(|e| AppError::RwLockError { msg: e.to_string() })?;

                let room = rooms.get_mut(pool_name).ok_or(AppError::CustomError {
                    msg: format!("Room '{}' could not be found.", pool_name),
                })?;

                room.users.remove(user_id);
                return Ok(room.users.clone());
            }
        }
        Err(AppError::CustomError {
            msg: "The user is not authenticated".to_string(),
        })
    }
}

// A room authenticated users, There users can make some socket commands.
#[derive(Debug, Serialize, Deserialize, Eq, Clone)]
pub struct RoomUser {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
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
    JoinRoom {
        pool_name: String,
        number_poolers: u8,
    },
    LeaveRoom,
    OnReady,
    AddUser {
        user_name: String,
    },
    RemoveUser {
        user_id: String,
    },
    OnPoolSettingChanges {
        pool_settings: PoolSettings,
    },
    StartDraft {
        draft_order: Vec<String>,
    },
    UndoDraftPlayer,
    DraftPlayer {
        player: PoolPlayerInfo,
    },
}

// Response return to the sockets clients as commands response.
#[derive(Deserialize, Serialize)]
pub enum CommandResponse {
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
