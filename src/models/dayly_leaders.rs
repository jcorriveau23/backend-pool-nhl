use mongodb::bson::oid::ObjectId;
use mongodb::bson::DateTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive( Debug, Deserialize, Serialize, JsonSchema )]
pub struct DaylyLeaders {
    pub date: String,
    pub goalies: Vec<DaylyGoaly>,
    pub skaters: Vec<DaylySkater>,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema )]
pub struct DaylyGoaly {
    pub name: String,
    pub id: i32,
    pub team: String,
    pub stats: GoalyStats,
}

#[derive( Debug, Deserialize, Serialize, JsonSchema )]
pub struct DaylySkater {
    pub name: String,
    pub id: i32,
    pub team: String,
    pub stats: SkaterStats,
}

#[allow(non_snake_case)]
#[derive( Debug, Deserialize, Serialize, JsonSchema )]
pub struct GoalyStats {
    pub timeOnIce: String,
    pub assists: u8,
    pub goals: u8,
    pub pim: u8,
    pub shots: u8,
    pub saves: u8,
    pub powerPlaySaves: u8,
    pub shortHandedSaves: u8,
    pub evenSaves: u8,
    pub shortHandedShotsAgainst: u8,
    pub evenShotsAgainst:u8,
    pub powerPlayShotsAgainst: u8,
    //pub decision: String,
    pub savePercentage: f32,
    //pub powerPlaySavePercentage: f32,
    //pub shortHandedSavePercentage: f32,
    //pub evenStrengthSavePercentage: f32,
}

#[allow(non_snake_case)]
#[derive( Debug, Deserialize, Serialize, JsonSchema )]
pub struct SkaterStats {
    pub timeOnIce: String,
    pub assists: u8,
    pub goals: u8,
    pub shots: u8,
    pub hits: u8,
    pub powerPlayGoals: u8,
    pub powerPlayAssists: u8,
    pub penaltyMinutes: u8,
    //pub faceOffPct: f32,
    pub faceOffWins: u8,
    pub faceoffTaken: u8,
    pub takeaways: u8,
    pub giveaways: u8,
    pub shortHandedGoals: u8,
    pub shortHandedAssists: u8,
    pub blocked: u8,
    pub plusMinus: i8,
    pub evenTimeOnIce: String,
    pub powerPlayTimeOnIce: String,
    pub shortHandedTimeOnIce: String,
}

