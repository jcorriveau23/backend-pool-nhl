//! Derives pool scores on demand from the shared `day_leaders` and the pool's
//! embedded sparse lineup events, instead of the per-pool `score_by_day` blob.
//!
//! The lineup a participant iced on a given day is reconstructed from the pool's
//! `lineup_events` ([`lineup_as_of`]); the points those players scored come from
//! [`DayLeadersCache`]. Building the familiar `DailyRosterPoints` shape lets the
//! frontend consume the same structure it did from `score_by_day`.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};
use futures::stream::{self, StreamExt, TryStreamExt};

use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::pool::lineup::{LineupEvent, lineup_as_of};
use poolnhl_interface::pool::model::{Pool, PoolUser};
use poolnhl_interface::pool::scoring::{DailyRosterPoints, DayScores};

use crate::services::day_leaders_cache::DayLeadersCache;

const DATE_FMT: &str = "%Y-%m-%d";

// A pool season runs ~200 days; this leaves room for a full season plus slack
// while keeping a single request bounded.
const MAX_RANGE_DAYS: i64 = 400;

// How many days are fetched from the cache at once. High enough to collapse a
// season into a handful of waves, low enough not to burst the redis pool.
const RANGE_CONCURRENCY: usize = 16;

#[derive(Clone)]
pub struct PoolScoringService {
    cache: DayLeadersCache,
}

impl PoolScoringService {
    pub fn new(cache: DayLeadersCache) -> Self {
        Self { cache }
    }

    /// Per-participant scoring breakdown for a single date, derived from the
    /// shared `day_leaders` and each participant's lineup on that date.
    pub async fn derive_daily(
        &self,
        pool: &Pool,
        date: &str,
    ) -> Result<HashMap<String, DailyRosterPoints>> {
        let day_scores = self.cache.day_scores(date).await?;
        Ok(build_day(
            &pool.participants,
            pool_events(pool),
            &day_scores,
            date,
        ))
    }

    /// Per-participant breakdown for every date in `[from, to]` (inclusive),
    /// keyed by date. Feeds the cumulative/history views and graphs.
    pub async fn derive_range(
        &self,
        pool: &Pool,
        from: &str,
        to: &str,
    ) -> Result<HashMap<String, HashMap<String, DailyRosterPoints>>> {
        let events = pool_events(pool);
        let dates = range_dates(from, to)?;

        // A season is ~200 days; fetching them one after another would be as
        // many sequential round-trips. Run a bounded number in flight instead.
        let derived = stream::iter(dates.into_iter().map(|date| async move {
            let day_scores = self.cache.day_scores(&date).await?;
            let day = build_day(&pool.participants, events, &day_scores, &date);
            Ok::<_, AppError>((date, day))
        }))
        .buffer_unordered(RANGE_CONCURRENCY)
        .try_collect()
        .await?;

        Ok(derived)
    }
}

/// Every date in `[from, to]` inclusive.
///
/// The span comes straight from the URL and each day costs at least a redis
/// round-trip, so it is validated and capped here: an unbounded range would let
/// one request tie up a worker for as long as it likes.
fn range_dates(from: &str, to: &str) -> Result<Vec<String>> {
    let start = parse_date(from)?;
    let end = parse_date(to)?;

    if end < start {
        return Err(AppError::ParseError {
            msg: format!("'{from}' is after '{to}'."),
        });
    }

    let days = (end - start).num_days() + 1;
    if days > MAX_RANGE_DAYS {
        return Err(AppError::ParseError {
            msg: format!(
                "A scoring range is limited to {MAX_RANGE_DAYS} days, '{from}'..'{to}' spans {days}."
            ),
        });
    }

    Ok((0..days)
        .map(|offset| {
            (start + Duration::days(offset))
                .format(DATE_FMT)
                .to_string()
        })
        .collect())
}

// The pool's embedded lineup events (empty if none recorded yet).
fn pool_events(pool: &Pool) -> &[LineupEvent] {
    pool.context
        .as_ref()
        .and_then(|context| context.lineup_events.as_deref())
        .unwrap_or(&[])
}

// Score every participant's lineup for one day: reconstruct the lineup as of the
// date from the events, then source the points from the shared day scores.
fn build_day(
    participants: &[PoolUser],
    events: &[LineupEvent],
    day_scores: &DayScores,
    date: &str,
) -> HashMap<String, DailyRosterPoints> {
    let mut day = HashMap::with_capacity(participants.len());
    for participant in participants {
        let (forwards, defense, goalies) = lineup_as_of(events, &participant.id, date);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_expands_to_every_day_inclusive() {
        let dates = range_dates("2026-10-01", "2026-10-04").unwrap();
        assert_eq!(
            dates,
            ["2026-10-01", "2026-10-02", "2026-10-03", "2026-10-04"]
        );
    }

    #[test]
    fn a_single_day_range_is_valid() {
        assert_eq!(range_dates("2026-10-01", "2026-10-01").unwrap().len(), 1);
    }

    // An unbounded span would mean one redis round-trip per day, forever.
    #[test]
    fn an_oversized_range_is_rejected() {
        let result = range_dates("1900-01-01", "2100-01-01");
        assert!(matches!(result, Err(AppError::ParseError { .. })));
    }

    #[test]
    fn an_inverted_range_is_rejected() {
        let result = range_dates("2026-10-04", "2026-10-01");
        assert!(matches!(result, Err(AppError::ParseError { .. })));
    }

    #[test]
    fn a_malformed_date_is_rejected() {
        assert!(range_dates("not-a-date", "2026-10-01").is_err());
    }
}
