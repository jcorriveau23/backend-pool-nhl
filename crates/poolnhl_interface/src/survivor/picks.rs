//! The pick documents of a survivor pool.
//!
//! A pick is its own document rather than an entry inside the pool, and that is
//! the decision the whole feature's scale rests on. Hundreds of participants
//! pick within the same hour on a Saturday morning; if a pick were a field of
//! the pool document, every one of them would read the pool, change it and write
//! it back under the same optimistic lock, and almost all of them would lose the
//! race and have to retry.
//!
//! As separate documents, a pick is an insert that touches nothing anybody else
//! is writing, and the two rules of the game become unique indexes rather than
//! checks that a race could slip past:
//!
//! - `(pool_name, participant_id, week)` unique — one pick per participant per
//!   date, so a double submit cannot produce two.
//! - `(pool_name, participant_id, cycle, team_id)` unique — a team a
//!   participant has used is closed to them, so two picks racing each other
//!   cannot both take it.

use serde::{Deserialize, Serialize};

use crate::survivor::model::{PickOutcome, UsedTeam};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorPick {
    pub pool_name: String,
    pub participant_id: String,

    /// The pick date this is for, as [`crate::survivor::model::SurvivorWeek::week`].
    pub week: u16,

    /// Which pass through the league this pick belongs to. Part of the unique
    /// key that closes a used team, so the key reopens when the cycle advances.
    pub cycle: u16,

    pub team_id: u32,

    pub outcome: PickOutcome,

    pub date_picked: i64,
    /// Set when the pick is changed rather than first made, for the audit trail
    /// a participant disputing an elimination would ask for.
    ///
    /// Absent from a pick that was never changed — the upsert that writes a
    /// first pick does not create the field — so it needs a default to read
    /// back.
    #[serde(default)]
    pub date_modified: Option<i64>,
}

impl SurvivorPick {
    pub fn used_team(&self) -> UsedTeam {
        UsedTeam {
            cycle: self.cycle,
            team_id: self.team_id,
        }
    }
}

/// A pick as it is shown to somebody who is not the one who made it.
///
/// Picks stay hidden until their date locks: knowing what the field picked
/// before you pick is the one thing that would break a survivor pool, since the
/// whole game is a guess made without that. After the lock they are public, and
/// the reveal is most of what people come to the page for.
#[derive(Debug, Serialize, Clone)]
pub struct SurvivorPickView {
    pub participant_id: String,
    pub week: u16,
    pub team_id: u32,
    pub outcome: PickOutcome,
}

impl From<&SurvivorPick> for SurvivorPickView {
    fn from(pick: &SurvivorPick) -> Self {
        Self {
            participant_id: pick.participant_id.clone(),
            week: pick.week,
            team_id: pick.team_id,
            outcome: pick.outcome,
        }
    }
}

/// What a participant needs to pick: the teams playing, the ones still open to
/// them, and the pick they already have in.
#[derive(Debug, Serialize, Clone)]
pub struct SurvivorPickOptions {
    pub week: u16,
    pub pick_date: String,
    /// Teams with a game on `pick_date`.
    pub eligible_team_ids: Vec<u32>,
    /// Of those, the ones this participant has not used in their current cycle.
    pub available_team_ids: Vec<u32>,
    /// Teams already spent, so the UI can say why the rest are closed.
    pub used_team_ids: Vec<u32>,
    pub current_pick: Option<u32>,
    /// True when nothing is open to them and the date costs them nothing.
    pub is_blocked: bool,
    pub can_pick: bool,
}
