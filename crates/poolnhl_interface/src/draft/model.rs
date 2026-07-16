use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    pool::model::{Pool, PoolSettings},
    users::model::UserEmailJwtPayload,
};

// A room authenticated users, There users can make some socket commands.
#[derive(Debug, Serialize, Deserialize, Eq, Clone)]
pub struct RoomUser {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub is_ready: bool,
}

impl RoomUser {
    // A room member backed by an authenticated socket. Not ready until the user says so.
    pub fn from_jwt(user: &UserEmailJwtPayload) -> Self {
        Self {
            id: user.sub.to_string(),
            name: user.email.address.to_string(),
            email: Some(user.email.address.to_string()),
            is_ready: false,
        }
    }

    // A member added manually to the room (not tied to any socket), always considered ready.
    pub fn new_unmanaged(user_name: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: user_name.to_string(),
            email: None,
            is_ready: true,
        }
    }
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
        player_id: i64,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::users::model::EmailInfo;

    fn jwt_payload(user_id: &str, email: &str) -> UserEmailJwtPayload {
        UserEmailJwtPayload {
            aud: vec!["test".to_string()],
            email: EmailInfo {
                address: email.to_string(),
                is_primary: true,
                is_verified: true,
            },
            exp: 0,
            iat: 0,
            sub: user_id.to_string(),
        }
    }

    #[test]
    fn room_user_from_jwt_is_not_ready() {
        let user = RoomUser::from_jwt(&jwt_payload("user-1", "someone@example.com"));
        assert_eq!(user.id, "user-1");
        assert_eq!(user.name, "someone@example.com");
        assert_eq!(user.email.as_deref(), Some("someone@example.com"));
        assert!(!user.is_ready);
    }

    #[test]
    fn unmanaged_room_user_is_ready_with_unique_id() {
        let first = RoomUser::new_unmanaged("Guest");
        let second = RoomUser::new_unmanaged("Guest");
        assert_eq!(first.name, "Guest");
        assert!(first.email.is_none());
        assert!(first.is_ready);
        assert_ne!(first.id, second.id);
    }
}
