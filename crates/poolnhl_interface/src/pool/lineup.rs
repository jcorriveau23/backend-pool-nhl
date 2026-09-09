//! Sparse lineup events: the pool-specific half of a pool's scoring history.
//!
//! A participant's active lineup only changes on a handful of days per season,
//! so one event is recorded per change, embedded on the pool
//! ([`PoolContext::lineup_events`]). The lineup effective on any date is the
//! latest event on or before it; points are then derived from `day_leaders`.

use serde::{Deserialize, Serialize};

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

    fn event(date: &str, forwards: &[u32]) -> LineupEvent {
        LineupEvent {
            participant: "u1".to_string(),
            effective_date: date.to_string(),
            forwards: forwards.to_vec(),
            defense: Vec::new(),
            goalies: vec![30],
        }
    }

    #[test]
    fn lineup_as_of_returns_the_latest_prior_event() {
        let events = vec![
            event("2025-10-01", &[10, 11]),
            event("2025-10-04", &[10, 12]),
        ];

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
