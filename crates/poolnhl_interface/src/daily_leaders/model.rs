use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct DailyLeaders {
    pub date: String,
    pub goalies: Vec<DailyGoaly>,
    pub skaters: Vec<DailySkater>,
    pub played: Vec<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DailyGoaly {
    pub name: String,
    pub id: u32,
    pub team: u32,
    pub stats: GoalyStats,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DailySkater {
    pub name: String,
    pub id: u32,
    pub team: u32,
    pub stats: SkaterStats,
}

// To reduce only the unused members have been commented out to reduce data usage.

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GoalyStats {
    // pub timeOnIce: String,
    pub assists: u8,
    pub goals: u8,
    // pub pim: u8,
    pub shots: u8,
    pub saves: u8,
    // pub powerPlaySaves: u8,
    // pub shortHandedSaves: u8,
    // pub evenSaves: u8,
    // pub shortHandedShotsAgainst: u8,
    // pub evenShotsAgainst: u8,
    // pub powerPlayShotsAgainst: u8,
    pub decision: Option<String>,
    pub savePercentage: Option<f32>,
    // pub powerPlaySavePercentage: Option<f32>,
    // pub shortHandedSavePercentage: Option<f32>,
    // pub evenStrengthSavePercentage: Option<f32>,
    pub OT: Option<bool>,
}
// To reduce only the unused members have been commented out to reduce data usage.

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct SkaterStats {
    // pub timeOnIce: String,
    pub assists: u8,
    pub goals: u8,
    // pub shots: u8,
    // pub hits: u8,
    // pub powerPlayGoals: u8,
    // pub powerPlayAssists: u8,
    // pub penaltyMinutes: Option<u8>,
    // pub faceOffPct: Option<f32>,
    // pub faceOffWins: u8,
    // pub faceoffTaken: u8,
    // pub takeaways: u8,
    // pub giveaways: u8,
    // pub shortHandedGoals: u8,
    // pub shortHandedAssists: u8,
    // pub blocked: u8,
    // pub plusMinus: i8,
    // pub evenTimeOnIce: String,
    // pub powerPlayTimeOnIce: String,
    // pub shortHandedTimeOnIce: String,
    pub shootoutGoals: u8,
}