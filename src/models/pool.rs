use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct Pool {
    pub name: String, // the name of the pool.
    pub owner: String,
    pub number_poolers: u8, // the number of participants in the pool.

    pub participants: Option<Vec<String>>,    // The mongoDB ID of each participants.

    // Roster configuration.
    pub number_forwards: u8,
    pub number_defenders: u8,
    pub number_goalies: u8,
    pub number_reservists: u8,

    // Forwards points configuration.
    pub forward_pts_goals: u8,
    pub forward_pts_assists: u8,
    pub forward_pts_hattricks: u8,

    // Defenders points configuration.
    pub defender_pts_goals: u8,
    pub defender_pts_assists: u8,
    pub defender_pts_hattricks: u8,

    // Goalies points configuration.
    pub goalies_pts_wins: u8,
    pub goalies_pts_shutouts: u8,
    pub goalies_pts_goals: u8,
    pub goalies_pts_assists: u8,

    // Other pool configuration
    pub next_season_number_players_protected: u8,
    pub tradable_picks: u8, // numbers of the next season picks participants are able to trade with each other.

    pub status: PoolState, // State of the pool.
    pub final_rank: Option<Vec<String>>,

    pub nb_player_drafted: u8,
    
    // Trade information.
    pub nb_trade: u32,
    pub trades: Option<Vec<Trade>>, 

    // context of the pool.
    pub context: Option<PoolContext>, 
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub enum PoolState {
    InProgress,
    Dynastie,
    Draft,
    Created
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]   // Copy
pub struct PoolContext {
    pub pooler_roster: HashMap<String, PoolerRoster>,
    pub draft_order: Vec<String>,
    pub score_by_day: Option<HashMap<String, HashMap<String, DailyRosterPoints>>>,
    pub tradable_picks: Vec<HashMap<String, String>>
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]   // Copy
pub struct PoolerRoster {
    pub chosen_forwards: Vec<Player>,
    pub chosen_defenders: Vec<Player>,
    pub chosen_goalies: Vec<Player>,
    pub chosen_reservists: Vec<Player>,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]  
pub struct DailyRosterPoints {
    pub roster: Roster,
    pub F_tot: Option<SkaterPoolPoints>,
    pub D_tot: Option<SkaterPoolPoints>,
    pub G_tot: Option<GoalyPoolPoints>,
    pub tot_pts: Option<u32>,
    pub cumulate: Option<DailyCumulate>,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]  
pub struct Roster {
    pub F: HashMap<String, Option<SkaterPoints>>,
    pub D: HashMap<String, Option<SkaterPoints>>,
    pub G: HashMap<String, Option<GoalyPoints>>,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]  
pub struct SkaterPoints {
    pub G: u8,
    pub A: u8,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]  
pub struct GoalyPoints {
    pub G: u8,
    pub A: u8,
    pub W: bool, 
    pub SO: bool, 
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]  
pub struct SkaterPoolPoints {
    pub G: u8,
    pub A: u8,
    pub HT: u8,
    pub pts: u8,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]  
pub struct GoalyPoolPoints {
    pub G: u8,
    pub A: u8,
    pub W: u8,
    pub SO: u8,
    pub pts: u8,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]  
pub struct DailyCumulate {
    // Forwards
    pub G_F: u16,
    pub A_F: u16,
    pub HT_F: u8,
    pub P_F: u16,
    // Defenders
    pub G_D: u16,
    pub A_D: u16,
    pub HT_D: u8,
    pub P_D: u16,
    // Goalies
    pub G_G: u8,
    pub A_G: u8,
    pub W_G: u16,
    pub SO_G: u8,
    pub P_G: u16,
    pub P: u16
}

impl PartialEq<Player> for Player {
    fn eq(&self, other: &Player) -> bool {
        self.id == other.id
    }
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]
pub struct Player {
    pub id: u32,    // ID from the NHL API.
    pub name: String,
    pub team: String,
    pub position: Position,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]
pub enum Position {
    F,
    D,
    G
}

impl PartialEq<Pick> for Pick {
    fn eq(&self, other: &Pick) -> bool {
        self.rank == other.rank && self.pooler == other.pooler
    }
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]
pub struct Pick {
    pub rank: u8,
    pub pooler: String
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]
pub struct Trade {
    pub proposed_by: String,
    pub ask_to: String,
    pub from_items: TradeItems,
    pub to_items: TradeItems,
    pub status: TradeStatus,
    pub id: u32,
    pub date_accepted: String,    
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]
pub struct TradeItems {
    pub players: Vec<Player>,
    pub picks: Vec<Pick>,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema, Clone )]
pub enum TradeStatus {
    NEW,
    ACCEPTED,
    COMPLETED,
    CANCELLED,
    REFUSED,
}

// payload to sent when creating a new pool.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct PoolCreationRequest {
    pub name: String,
    pub number_pooler: u8,
}

// payload to sent when deleting a pool.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct PoolDeletionRequest {
    pub name: String,
}

// payload to sent when deleting a pool.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct StartDraftRequest {
    pub name: String,
    pub participants: Vec<String>,
}

// payload to sent when selecting a player.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct SelectPlayerRequest {
    pub name: String,
    pub player: Player,
}

// payload to sent when creating a trade.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct CreateTradeRequest {
    pub name: String,
    pub trade: Trade,
}

// payload to sent when cancelling a trade.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct CancelTradeRequest {
    pub name: String,
    pub trade_id: u32,
}

// payload to sent when responding to a trade.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct RespondTradeRequest {
    pub name: String,
    pub is_accepted: bool,
    pub trade_id: u32,
}

// payload to sent when filling a spot with a reservist.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct FillSpotRequest {
    pub name: String,
    pub player: Player,
}

// payload to sent when filling a spot with a reservist.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct ProtectPlayersRequest {
    pub name: String,
    pub forw_protected: Vec<Player>,
    pub def_protected: Vec<Player>,
    pub goal_protected: Vec<Player>,
    pub reserv_protected: Vec<Player>,
}