//! Derives pool scores on demand from the shared `day_leaders` instead of the
//! per-pool `score_by_day` blob.
//!
//! The lineup a participant iced on a given day is pool-specific and must be
//! stored; the *points* those players scored are shared and are recomputed here
//! from [`DayLeadersCache`]. By rebuilding the same `score_by_day` shape from
//! derived data, the existing scoring and ranking logic
//! ([`PoolContext::get_final_rank`]) is reused verbatim.

use std::collections::HashMap;

use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::pool::model::{DailyRosterPoints, Pool};

use crate::services::day_leaders_cache::DayLeadersCache;

#[derive(Clone)]
pub struct PoolScoringService {
    cache: DayLeadersCache,
}

impl PoolScoringService {
    pub fn new(cache: DayLeadersCache) -> Self {
        Self { cache }
    }

    /// Rebuild a pool's per-day score map by deriving each participant's daily
    /// points from the shared `day_leaders` (via the cache), keeping the
    /// lineups recorded in the pool context. This is the derive-side twin of
    /// the stored `score_by_day`.
    ///
    /// The lineup source here is the pool's own `score_by_day` keys, which lets
    /// this be shadow-compared against the stored points before sparse lineup
    /// events replace that source.
    pub async fn derive_score_by_day(
        &self,
        pool: &Pool,
    ) -> Result<HashMap<String, HashMap<String, DailyRosterPoints>>> {
        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;
        let stored = context
            .score_by_day
            .as_ref()
            .ok_or_else(|| AppError::CustomError {
                msg: "pool has no score_by_day to derive lineups from.".to_string(),
            })?;

        let mut derived = HashMap::with_capacity(stored.len());
        for (date, day) in stored {
            let scores = self.cache.day_scores(date).await?;
            let mut derived_day = HashMap::with_capacity(day.len());
            for (participant, roster_points) in day {
                let (forwards, defense, goalies) = lineup_ids(roster_points);
                derived_day.insert(
                    participant.clone(),
                    DailyRosterPoints {
                        roster: scores.roster_for(&forwards, &defense, &goalies),
                        is_cumulated: true,
                    },
                );
            }
            derived.insert(date.clone(), derived_day);
        }
        Ok(derived)
    }

    /// A pool's final ranking, computed entirely from the shared `day_leaders`.
    pub async fn derive_final_rank(&self, pool: &Pool) -> Result<Vec<String>> {
        let score_by_day = self.derive_score_by_day(pool).await?;
        let mut context = pool.context.clone().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;
        context.score_by_day = Some(score_by_day);
        context.get_final_rank(&pool.settings)
    }
}

// The lineup for a participant on a day is the set of players recorded in each
// position of the stored roster (the map values are the points being re-derived).
fn lineup_ids(roster_points: &DailyRosterPoints) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    (
        keys_as_ids(&roster_points.roster.F),
        keys_as_ids(&roster_points.roster.D),
        keys_as_ids(&roster_points.roster.G),
    )
}

fn keys_as_ids<V>(roster: &HashMap<String, V>) -> Vec<u32> {
    roster
        .keys()
        .filter_map(|id| id.parse::<u32>().ok())
        .collect()
}
