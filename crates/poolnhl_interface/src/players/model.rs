use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GetPlayerQuery {
    pub active: Option<bool>,
    #[serde(deserialize_with = "comma_separated")]
    pub positions: Option<Vec<String>>,
    pub sort: Option<String>,
    pub descending: Option<bool>,
    pub skip: Option<u64>,
    pub limit: Option<i64>,
}

// Custom deserializer to handle comma-separated values in a query string
fn comma_separated<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    // Split by commas and convert to Vec<String>
    Ok(Some(s.split(',').map(|s| s.to_string()).collect()))
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerInfo {
    pub active: bool,
    pub id: u32, // ID from the NHL API.
    pub name: String,
    pub team: Option<u32>,
    pub position: String,
    pub age: Option<u8>,
    pub salary_cap: Option<f64>,
    pub contract_expiration_season: Option<u32>,
    pub game_played: Option<u32>,
    pub goals: Option<u32>,
    pub assists: Option<u32>,
    pub points: Option<u32>,
    pub points_per_game: Option<f32>,
    pub goal_against_average: Option<f32>,
    pub save_percentage: Option<f32>,
}
