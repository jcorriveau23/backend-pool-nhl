use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct SkaterStats {
    pub assists: u8,
    pub goals: u8,
    pub shootoutGoals: u8,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GoalyStats {
    pub assists: u8,
    pub goals: u8,
    pub decision: Option<String>,
    pub savePercentage: Option<f32>,
    pub OT: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DailySkater {
    pub name: String,
    pub id: u32,
    pub team: u32,
    pub stats: SkaterStats,
}


#[derive(Debug, Deserialize, Serialize)]
pub struct DailyGoaly {
    pub name: String,
    pub id: u32,
    pub team: u32,
    pub stats: GoalyStats,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DailyLeaders {
    pub date: String,
    pub goalies: Vec<DailyGoaly>,
    pub skaters: Vec<DailySkater>,
    pub played: Vec<u32>,
}