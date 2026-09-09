//! Free-agent transactions: a pooler swapping a player they hold for one
//! nobody in the pool does.
//!
//! A transaction is always a pair — one player dropped, one picked up — so a
//! roster never changes size, and each pair costs one unit of the budget the
//! owner sets in [`PlayerDropSettings`]. The log lives on the pool's context
//! next to the lineup events: the swap also moves the starting lineup, so it
//! records a [`crate::pool::lineup::LineupEvent`] on the same effective date
//! and the scoring derives from that as it does for any other lineup change.

use serde::{Deserialize, Serialize};

/// How often a pooler's drop budget refills.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum DropPeriod {
    /// One budget for the whole pool. Nothing refills it.
    Season,
    /// The budget refills on the first of every calendar month.
    Month,
}

/// The owner's free-agent rule. Absent from a pool's settings means the pool
/// has no free agency at all and the endpoint refuses every swap.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub struct PlayerDropSettings {
    /// Swaps a pooler may make per [`DropPeriod`].
    pub max_drops: u8,
    pub period: DropPeriod,
}

/// One completed swap, kept so the budget can be counted and the pool can show
/// what happened when.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct RosterTransaction {
    pub participant: String,
    /// The day the swap takes effect for scoring ("YYYY-MM-DD"), which is also
    /// the date of the lineup event it records.
    pub effective_date: String,
    pub dropped_player_id: u32,
    pub added_player_id: u32,
    /// When the swap was filed, in milliseconds. Only for display: the budget
    /// is counted on `effective_date`, which is the day the swap is *for*.
    pub date_created: i64,
}

/// The calendar month a date falls in, as its "YYYY-MM" prefix.
///
/// Dates are ISO, so this is a slice rather than a parse. A string that is too
/// short to hold a month is returned whole, which puts it in a bucket of its
/// own instead of panicking on a byte index.
fn month_of(date: &str) -> &str {
    date.get(..7).unwrap_or(date)
}

/// How many of `participant`'s swaps count against the budget that `date`
/// falls in.
///
/// The budget is counted on the effective dates, not on when the swaps were
/// filed: a pooler filing a swap on the last day of a month for the first of
/// the next one spends the next month's budget, which is the month the swap
/// actually applies to.
pub fn drops_used_in_period(
    transactions: &[RosterTransaction],
    participant: &str,
    settings: &PlayerDropSettings,
    date: &str,
) -> usize {
    transactions
        .iter()
        .filter(|transaction| transaction.participant == participant)
        .filter(|transaction| match settings.period {
            DropPeriod::Season => true,
            DropPeriod::Month => month_of(&transaction.effective_date) == month_of(date),
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction(participant: &str, effective_date: &str) -> RosterTransaction {
        RosterTransaction {
            participant: participant.to_string(),
            effective_date: effective_date.to_string(),
            dropped_player_id: 1,
            added_player_id: 2,
            date_created: 0,
        }
    }

    fn settings(period: DropPeriod) -> PlayerDropSettings {
        PlayerDropSettings {
            max_drops: 3,
            period,
        }
    }

    #[test]
    fn season_budget_counts_every_swap_of_the_participant() {
        let transactions = vec![
            transaction("u1", "2025-10-05"),
            transaction("u1", "2025-12-24"),
            transaction("u2", "2025-12-24"),
        ];

        let used = drops_used_in_period(
            &transactions,
            "u1",
            &settings(DropPeriod::Season),
            "2026-02-01",
        );
        assert_eq!(used, 2);
    }

    #[test]
    fn month_budget_only_counts_the_month_the_date_falls_in() {
        let transactions = vec![
            transaction("u1", "2025-12-01"),
            transaction("u1", "2025-12-24"),
            transaction("u1", "2026-01-03"),
        ];
        let month = settings(DropPeriod::Month);

        assert_eq!(
            drops_used_in_period(&transactions, "u1", &month, "2025-12-31"),
            2
        );
        assert_eq!(
            drops_used_in_period(&transactions, "u1", &month, "2026-01-15"),
            1
        );
        assert_eq!(
            drops_used_in_period(&transactions, "u1", &month, "2026-02-15"),
            0
        );
    }

    #[test]
    fn a_participant_with_no_transaction_has_spent_nothing() {
        assert_eq!(
            drops_used_in_period(&[], "u1", &settings(DropPeriod::Season), "2025-12-01"),
            0
        );
    }
}
