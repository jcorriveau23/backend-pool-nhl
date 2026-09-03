use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    players::model::PlayerInfo,
    pool::model::{Pool, PoolSettings, PoolState, PoolerRoster},
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

// A roster rearrangement asked for over the draft socket. Same shape as the
// `/modify-roster` REST body minus the pool name, which the room already knows.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RosterModification {
    pub roster_modified_user_id: String,
    pub forw_list: Vec<u32>,
    pub def_list: Vec<u32>,
    pub goal_list: Vec<u32>,
    pub reserv_list: Vec<u32>,
}

// Commands that the soket server can receive.
#[derive(Deserialize, Serialize)]
pub enum Command {
    Auth {
        token: String,
    },
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
    // Rosters can be rearranged during the draft too, and the room has to see
    // it. The REST endpoint stays for pools that are already running, where
    // there is no room and no socket.
    ModifyRoster(RosterModification),
}

// Response return to the sockets clients as commands response.
#[derive(Deserialize, Serialize)]
pub enum CommandResponse {
    Pool {
        pool: Box<Pool>,
    },
    Users {
        room_users: HashMap<String, RoomUser>,
    },
    // A single draft pick, sent instead of the whole pool. The pool is only a
    // few fields larger after a pick but tens of kilobytes in total, and it is
    // rebroadcast to every socket of the room on every pick, so the draft sends
    // the delta and lets the clients apply it to the copy they already hold.
    PlayerDrafted {
        player: PlayerInfo,
        // The participant the player was drafted for, which is not necessarily
        // the socket that sent the command (the owner drafts for others).
        participant_id: String,
        // That participant's roster after the pick, sent whole so clients never
        // have to reimplement the slot/reservist placement rules.
        roster: PoolerRoster,
        // What was appended to `players_name_drafted`.
        appended_picks: Vec<u32>,
        // `players_name_drafted.len()` after the pick. A client whose own list
        // does not reach this length missed an update and must refetch the pool.
        pick_count: usize,
        // Flips to InProgress on the final pick of the draft.
        status: PoolState,
        date_updated: i64,
    },
    DraftPickUndone {
        player_id: u32,
        participant_id: String,
        roster: PoolerRoster,
        // `players_name_drafted` is truncated to this length.
        pick_count: usize,
        date_updated: i64,
    },
    // A participant rearranged the players it already holds. Nothing is drafted
    // and no pick is consumed, so this carries no pick count.
    RosterModified {
        participant_id: String,
        roster: PoolerRoster,
        date_updated: i64,
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

    // The draft deltas are the contract with the web client, which branches on
    // the variant key and reads these exact field names. Pin the shape so a
    // rename here cannot silently freeze every draft board.
    #[test]
    fn player_drafted_serializes_as_a_tagged_delta() {
        let response = CommandResponse::PlayerDrafted {
            player: PlayerInfo {
                active: true,
                id: 7,
                name: "player-7".to_string(),
                team: None,
                position: crate::players::model::Position::F,
                age: None,
                salary_cap: None,
                contract_expiration_season: None,
                game_played: None,
                goals: None,
                assists: None,
                points: None,
                points_per_game: None,
                goal_against_average: None,
                save_percentage: None,
                saves: None,
                shots: None,
                wins: None,
                ot: None,
            },
            participant_id: "user-1".to_string(),
            roster: PoolerRoster {
                chosen_forwards: vec![7],
                chosen_defenders: Vec::new(),
                chosen_goalies: Vec::new(),
                chosen_reservists: Vec::new(),
            },
            appended_picks: vec![7, 0],
            pick_count: 3,
            status: PoolState::Draft,
            date_updated: 42,
        };

        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();

        let delta = &json["PlayerDrafted"];
        assert_eq!(delta["player"]["id"], 7);
        assert_eq!(delta["participant_id"], "user-1");
        assert_eq!(delta["roster"]["chosen_forwards"][0], 7);
        assert_eq!(delta["appended_picks"], serde_json::json!([7, 0]));
        assert_eq!(delta["pick_count"], 3);
        assert_eq!(delta["status"], "Draft");
        assert_eq!(delta["date_updated"], 42);
    }

    #[test]
    fn draft_pick_undone_serializes_as_a_tagged_delta() {
        let response = CommandResponse::DraftPickUndone {
            player_id: 7,
            participant_id: "user-1".to_string(),
            roster: PoolerRoster::new(),
            pick_count: 1,
            date_updated: 42,
        };

        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();

        let delta = &json["DraftPickUndone"];
        assert_eq!(delta["player_id"], 7);
        assert_eq!(delta["participant_id"], "user-1");
        assert_eq!(delta["roster"]["chosen_forwards"], serde_json::json!([]));
        assert_eq!(delta["pick_count"], 1);
        assert_eq!(delta["date_updated"], 42);
    }

    #[test]
    fn roster_modified_serializes_as_a_tagged_delta() {
        let response = CommandResponse::RosterModified {
            participant_id: "user-1".to_string(),
            roster: PoolerRoster {
                chosen_forwards: vec![7],
                chosen_defenders: Vec::new(),
                chosen_goalies: Vec::new(),
                chosen_reservists: vec![9],
            },
            date_updated: 42,
        };

        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();

        let delta = &json["RosterModified"];
        assert_eq!(delta["participant_id"], "user-1");
        assert_eq!(delta["roster"]["chosen_forwards"], serde_json::json!([7]));
        assert_eq!(delta["roster"]["chosen_reservists"], serde_json::json!([9]));
        assert_eq!(delta["date_updated"], 42);
    }

    // The web client builds this command as {"ModifyRoster": {...}}, so the
    // variant has to stay a newtype around the flat payload.
    #[test]
    fn modify_roster_command_deserializes_from_a_flat_payload() {
        let command: Command = serde_json::from_str(
            r#"{"ModifyRoster":{"roster_modified_user_id":"user-1","forw_list":[1,2],"def_list":[3],"goal_list":[4],"reserv_list":[5]}}"#,
        )
        .unwrap();

        let Command::ModifyRoster(modification) = command else {
            panic!("expected a ModifyRoster command");
        };
        assert_eq!(modification.roster_modified_user_id, "user-1");
        assert_eq!(modification.forw_list, vec![1, 2]);
        assert_eq!(modification.reserv_list, vec![5]);
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

    // The exact bytes the web client puts on the wire, from
    // `createSocketCommand(Command.Auth, JSON.stringify({ token }))` in
    // src/context/socket-context.tsx. The token moved out of the URL path and
    // into this frame, so if this stops parsing every signed-in pooler silently
    // drops to read-only in the draft room — with no error anywhere.
    #[test]
    fn auth_command_parses_the_frame_the_web_client_sends() {
        let command: Command =
            serde_json::from_str(r#"{"Auth":{"token":"header.payload.signature"}}"#)
                .expect("the client's auth frame must deserialize");

        match command {
            Command::Auth { token } => assert_eq!(token, "header.payload.signature"),
            _ => panic!("expected Command::Auth"),
        }
    }

    // Adding a variant to an externally tagged enum is the kind of change that
    // can quietly alter how the others parse. These two cover both shapes the
    // client emits: a struct variant and a bare unit variant.
    #[test]
    fn adding_auth_left_the_other_commands_parsing() {
        let join: Command =
            serde_json::from_str(r#"{"JoinRoom":{"pool_name":"my pool","number_poolers":6}}"#)
                .expect("JoinRoom must still deserialize");
        match join {
            Command::JoinRoom {
                pool_name,
                number_poolers,
            } => {
                assert_eq!(pool_name, "my pool");
                assert_eq!(number_poolers, 6);
            }
            _ => panic!("expected Command::JoinRoom"),
        }

        // Unit variants go over the wire as a bare JSON string.
        let ready: Command =
            serde_json::from_str(r#""OnReady""#).expect("OnReady must still deserialize");
        assert!(matches!(ready, Command::OnReady));
    }
}
