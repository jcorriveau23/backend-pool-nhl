use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::pool::model::{CURRENT_SEASON, SEASONS};

#[derive(Debug, Deserialize)]
pub struct GetPlayerQuery {
    pub active: Option<bool>,
    #[serde(deserialize_with = "comma_separated")]
    pub positions: Option<Vec<String>>,
    pub sort: Option<String>,
    pub descending: Option<bool>,
    pub skip: Option<u64>,
    pub limit: Option<i64>,
    // Whose season's stats to serve. Absent is the current season, which is
    // what a `players` document already carries.
    pub season: Option<u32>,
}

/// The only thing a name search takes beyond the name itself: a draft board
/// searching for a player has to show him under the same season as the table
/// it sits above.
#[derive(Debug, Deserialize)]
pub struct SearchPlayerQuery {
    pub season: Option<u32>,
}

/// Resolve which season's stats a request wants, as an override of the current
/// ones or as nothing to override.
///
/// `None` comes back for the current season as well as for no season at all:
/// `players` holds the current totals at the top level, so both are served by
/// the plain read with no lookup. A past season comes back as itself, and an
/// unknown one is rejected -- a lookup against a season with no rows would
/// otherwise answer 200 with a full table of nulls, which reads as "nobody
/// scored" rather than as the typo it is.
pub fn resolve_stats_season(season: Option<u32>) -> crate::errors::Result<Option<u32>> {
    match season {
        None => Ok(None),
        Some(season) if season == CURRENT_SEASON.season => Ok(None),
        Some(season) if SEASONS.iter().any(|known| known.season == season) => Ok(Some(season)),
        Some(season) => Err(AppError::CustomError {
            msg: format!(
                "{season} is not a season on record. Known: {}.",
                SEASONS
                    .iter()
                    .map(|known| known.season.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }),
    }
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
    pub position: Position,
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
    pub saves: Option<u32>,
    pub shots: Option<u32>,
    pub wins: Option<u32>,
    pub ot: Option<u32>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Position {
    F,
    D,
    G,
}

impl Position {
    pub fn as_str(&self) -> &'static str {
        match self {
            Position::F => "F",
            Position::D => "D",
            Position::G => "G",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_season_needs_no_override() {
        assert_eq!(resolve_stats_season(None).unwrap(), None);
        assert_eq!(
            resolve_stats_season(Some(CURRENT_SEASON.season)).unwrap(),
            None
        );
    }

    #[test]
    fn a_past_season_is_served_from_its_own_rows() {
        let previous = SEASONS[SEASONS.len() - 2].season;

        assert_eq!(
            resolve_stats_season(Some(previous)).unwrap(),
            Some(previous)
        );
    }

    #[test]
    fn a_season_with_no_rows_is_refused_rather_than_served_empty() {
        assert!(matches!(
            resolve_stats_season(Some(19992000)),
            Err(AppError::CustomError { .. })
        ));
    }
}
