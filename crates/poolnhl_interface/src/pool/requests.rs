//! Request payloads accepted by the pool endpoints.
//!
//! These are the wire shapes of the HTTP API, kept apart from the domain types
//! in [`crate::pool::model`] so a change to the transport does not reach into
//! the model.

use serde::Deserialize;

use crate::players::model::PlayerInfo;
use crate::pool::model::{PoolSettings, Trade};

// payload to sent when creating a new pool.
#[derive(Debug, Deserialize, Clone)]
pub struct PoolCreationRequest {
    pub pool_name: String,
    pub settings: PoolSettings,
}

// payload to sent when deleting a pool.
#[derive(Debug, Deserialize, Clone)]
pub struct PoolDeletionRequest {
    pub pool_name: String,
}

// payload to sent when adding player by the owner of the pool.
#[derive(Debug, Deserialize, Clone)]
pub struct AddPlayerRequest {
    pub pool_name: String,
    pub added_player_user_id: String,
    pub player: PlayerInfo,
}

// payload to sent when removing player by the owner of the pool.
#[derive(Debug, Deserialize, Clone)]
pub struct RemovePlayerRequest {
    pub pool_name: String,
    pub removed_player_user_id: String,
    pub player_id: u32,
}

// payload to sent when creating a trade.
#[derive(Debug, Deserialize, Clone)]
pub struct CreateTradeRequest {
    pub pool_name: String,
    pub trade: Trade,
}

// payload to sent when cancelling a trade.
#[derive(Debug, Deserialize, Clone)]
pub struct DeleteTradeRequest {
    pub pool_name: String,
    pub trade_id: u32,
}

// payload to sent when responding to a trade.
#[derive(Debug, Deserialize, Clone)]
pub struct RespondTradeRequest {
    pub pool_name: String,
    pub trade_id: u32,
    pub is_accepted: bool,
}

// payload to sent when filling a spot with a reservist.
#[derive(Debug, Deserialize, Clone)]
pub struct FillSpotRequest {
    pub pool_name: String,
    pub filled_spot_user_id: String,
    pub player_id: u32,
}

// payload to sent when modifying roster of a pooler
#[derive(Debug, Deserialize, Clone)]
pub struct ModifyRosterRequest {
    pub pool_name: String,
    pub roster_modified_user_id: String,
    pub forw_list: Vec<u32>,
    pub def_list: Vec<u32>,
    pub goal_list: Vec<u32>,
    pub reserv_list: Vec<u32>,
}

// payload to sent when protecting the list of players for dynasty draft.
#[derive(Debug, Deserialize, Clone)]
pub struct ProtectPlayersRequest {
    pub pool_name: String,
    pub protected_players_user_id: String,
    pub protected_players: Vec<u32>,
}

// payload to sent when generating a new season for a dynasty type of pool.
#[derive(Debug, Deserialize, Clone)]
pub struct CompleteProtectionRequest {
    pub pool_name: String,
}

// payload to sent when updating pool settings.
#[derive(Debug, Deserialize, Clone)]
pub struct UpdatePoolSettingsRequest {
    pub pool_name: String,
    pub settings: PoolSettings,
}

// payload to sent when the owner renames one of the poolers of the pool.
#[derive(Debug, Deserialize, Clone)]
pub struct UpdatePoolerNameRequest {
    pub pool_name: String,
    pub pooler_user_id: String,
    pub new_name: String,
}

// payload to sent when marking a pool as final
#[derive(Debug, Deserialize, Clone)]
pub struct MarkAsFinalRequest {
    pub pool_name: String,
}

// payload to sent when generating a new season for a dynasty type of pool.
#[derive(Debug, Deserialize, Clone)]
pub struct GenerateDynastyRequest {
    pub pool_name: String,
    pub new_pool_name: String,
}
