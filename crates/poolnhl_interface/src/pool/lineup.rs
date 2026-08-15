//! Sparse lineup events: the pool-specific half of a pool's scoring history.
//!
//! A participant's active lineup only changes on a handful of days per season,
//! so instead of storing a full roster snapshot for every day (as the legacy
//! `score_by_day` did), one event is recorded per change, embedded on the pool
//! ([`PoolContext::lineup_events`]). The lineup effective on any date is the
//! latest event on or before it; points are then derived from `day_leaders`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::pool::scoring::DailyRosterPoints;

/// A participant's lineup taking effect on `effective_date`, stored sparsely
/// (one entry per change) inside the pool's context.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct LineupEvent {
    pub participant: String,
    pub effective_date: String, // "YYYY-MM-DD"
    pub forwards: Vec<u32>,
    pub defense: Vec<u32>,
    pub goalies: Vec<u32>,
}

/// A lineup by position, sorted so equal lineups compare equal regardless of
/// the (unordered) roster map iteration order.
type Lineup = (Vec<u32>, Vec<u32>, Vec<u32>);

fn sorted_ids<V>(roster: &HashMap<String, V>) -> Vec<u32> {
    let mut ids: Vec<u32> = roster.keys().filter_map(|id| id.parse().ok()).collect();
    ids.sort_unstable();
    ids
}

fn lineup_of(roster_points: &DailyRosterPoints) -> Lineup {
    (
        sorted_ids(&roster_points.roster.F),
        sorted_ids(&roster_points.roster.D),
        sorted_ids(&roster_points.roster.G),
    )
}

/// Extract sparse lineup events from a pool's stored per-day score map. For
/// each participant the dates are walked in order and an event is emitted only
/// when the lineup differs from the previous day, collapsing the ~200 daily
/// snapshots of a season into a handful of events.
pub fn extract_lineup_events(
    score_by_day: &HashMap<String, HashMap<String, DailyRosterPoints>>,
) -> Vec<LineupEvent> {
    let mut by_participant: HashMap<&str, Vec<(&str, Lineup)>> = HashMap::new();
    for (date, day) in score_by_day {
        for (participant, roster_points) in day {
            by_participant
                .entry(participant.as_str())
                .or_default()
                .push((date.as_str(), lineup_of(roster_points)));
        }
    }

    let mut events = Vec::new();
    for (participant, mut days) in by_participant {
        days.sort_by(|a, b| a.0.cmp(b.0));
        let mut prev: Option<&Lineup> = None;
        for (date, lineup) in &days {
            if prev != Some(lineup) {
                events.push(LineupEvent {
                    participant: participant.to_string(),
                    effective_date: (*date).to_string(),
                    forwards: lineup.0.clone(),
                    defense: lineup.1.clone(),
                    goalies: lineup.2.clone(),
                });
            }
            prev = Some(lineup);
        }
    }
    events
}

/// The lineup effective on `date` for `participant`: the latest event on or
/// before that date. Returns empty slices if the participant has no event yet
/// (e.g. a date before the draft).
pub fn lineup_as_of<'a>(
    events: &'a [LineupEvent],
    participant: &str,
    date: &str,
) -> (&'a [u32], &'a [u32], &'a [u32]) {
    events
        .iter()
        .filter(|event| event.participant == participant && event.effective_date.as_str() <= date)
        .max_by(|a, b| a.effective_date.cmp(&b.effective_date))
        .map(|event| {
            (
                event.forwards.as_slice(),
                event.defense.as_slice(),
                event.goalies.as_slice(),
            )
        })
        .unwrap_or((&[], &[], &[]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::scoring::Roster;

    fn day(forwards: &[u32], goalies: &[u32]) -> DailyRosterPoints {
        DailyRosterPoints {
            roster: Roster {
                F: forwards.iter().map(|id| (id.to_string(), None)).collect(),
                D: HashMap::new(),
                G: goalies.iter().map(|id| (id.to_string(), None)).collect(),
            },
            is_cumulated: true,
        }
    }

    // The same lineup for three days then a change yields two events, not four.
    #[test]
    fn extract_collapses_unchanged_days_into_one_event() {
        let mut score_by_day = HashMap::new();
        for date in ["2025-10-01", "2025-10-02", "2025-10-03"] {
            score_by_day.insert(
                date.to_string(),
                HashMap::from([("u1".to_string(), day(&[10, 11], &[30]))]),
            );
        }
        // Day 4: swapped a forward.
        score_by_day.insert(
            "2025-10-04".to_string(),
            HashMap::from([("u1".to_string(), day(&[10, 12], &[30]))]),
        );

        let mut events = extract_lineup_events(&score_by_day);
        events.sort_by(|a, b| a.effective_date.cmp(&b.effective_date));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].effective_date, "2025-10-01");
        assert_eq!(events[0].forwards, vec![10, 11]);
        assert_eq!(events[1].effective_date, "2025-10-04");
        assert_eq!(events[1].forwards, vec![10, 12]);
    }

    #[test]
    fn lineup_as_of_returns_the_latest_prior_event() {
        let events = extract_lineup_events(&HashMap::from([
            (
                "2025-10-01".to_string(),
                HashMap::from([("u1".to_string(), day(&[10, 11], &[30]))]),
            ),
            (
                "2025-10-04".to_string(),
                HashMap::from([("u1".to_string(), day(&[10, 12], &[30]))]),
            ),
        ]));

        // Before any event: empty.
        assert_eq!(
            lineup_as_of(&events, "u1", "2025-09-30"),
            (&[][..], &[][..], &[][..])
        );
        // Between the two events: the first lineup still holds.
        assert_eq!(lineup_as_of(&events, "u1", "2025-10-03").0, &[10, 11]);
        // On/after the change: the new lineup.
        assert_eq!(lineup_as_of(&events, "u1", "2025-10-10").0, &[10, 12]);
        // Unknown participant: empty.
        assert_eq!(
            lineup_as_of(&events, "nobody", "2025-10-10"),
            (&[][..], &[][..], &[][..])
        );
    }
}
