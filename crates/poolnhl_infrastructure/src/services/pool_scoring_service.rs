//! Derives pool scores on demand from the shared `day_leaders` and sparse
//! lineup events, instead of the per-pool `score_by_day` blob.
//!
//! The lineup a participant iced on a given day comes from [`LineupStore`]
//! (reconstructed with [`lineup_as_of`]); the points those players scored come
//! from [`DayLeadersCache`]. Building the familiar `DailyRosterPoints` shape
//! lets the frontend consume the same structure it did from `score_by_day`.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};

use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::pool::lineup::{lineup_as_of, LineupEvent};
use poolnhl_interface::pool::model::{DailyRosterPoints, DayScores, Pool, PoolUser};

use crate::services::day_leaders_cache::DayLeadersCache;
use crate::services::lineup_store::LineupStore;

const DATE_FMT: &str = "%Y-%m-%d";

#[derive(Clone)]
pub struct PoolScoringService {
    cache: DayLeadersCache,
    lineup_store: LineupStore,
}

impl PoolScoringService {
    pub fn new(cache: DayLeadersCache, lineup_store: LineupStore) -> Self {
        Self {
            cache,
            lineup_store,
        }
    }

    /// Per-participant scoring breakdown for a single date, derived from the
    /// shared `day_leaders` and each participant's lineup on that date.
    pub async fn derive_daily(
        &self,
        pool: &Pool,
        date: &str,
    ) -> Result<HashMap<String, DailyRosterPoints>> {
        let events = self.lineup_store.events_for_pool(&pool.name).await?;
        let day_scores = self.cache.day_scores(date).await?;
        Ok(build_day(&pool.participants, &events, &day_scores, date))
    }

    /// Per-participant breakdown for every date in `[from, to]` (inclusive),
    /// keyed by date. Feeds the cumulative/history views and graphs.
    pub async fn derive_range(
        &self,
        pool: &Pool,
        from: &str,
        to: &str,
    ) -> Result<HashMap<String, HashMap<String, DailyRosterPoints>>> {
        let events = self.lineup_store.events_for_pool(&pool.name).await?;
        let start = parse_date(from)?;
        let end = parse_date(to)?;

        let mut derived = HashMap::new();
        let mut current = start;
        while current <= end {
            let date = current.format(DATE_FMT).to_string();
            let day_scores = self.cache.day_scores(&date).await?;
            derived.insert(
                date.clone(),
                build_day(&pool.participants, &events, &day_scores, &date),
            );
            current += Duration::days(1);
        }
        Ok(derived)
    }
}

// Score every participant's lineup for one day: reconstruct the lineup as of the
// date from its events, then source the points from the shared day scores.
fn build_day(
    participants: &[PoolUser],
    events: &HashMap<String, Vec<LineupEvent>>,
    day_scores: &DayScores,
    date: &str,
) -> HashMap<String, DailyRosterPoints> {
    let empty: Vec<LineupEvent> = Vec::new();
    let mut day = HashMap::with_capacity(participants.len());
    for participant in participants {
        let participant_events = events.get(&participant.id).unwrap_or(&empty);
        let (forwards, defense, goalies) = lineup_as_of(participant_events, date);
        day.insert(
            participant.id.clone(),
            DailyRosterPoints {
                roster: day_scores.roster_for(forwards, defense, goalies),
                is_cumulated: true,
            },
        );
    }
    day
}

fn parse_date(date: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(date, DATE_FMT)
        .map_err(|e| AppError::ParseError { msg: e.to_string() })
}
