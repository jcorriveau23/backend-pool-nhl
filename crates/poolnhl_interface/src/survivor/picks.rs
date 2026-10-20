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

use std::collections::HashSet;

use chrono::{DateTime, Utc};
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
/// Picks stay hidden until their date's deadline: knowing what the field picked
/// before you pick is the one thing that would break a survivor pool, since the
/// whole game is a guess made without that. Once the last of the date's games
/// has started nothing can change, so they go public — and the reveal is most
/// of what people come to the page for.
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

/// Where one of the date's games stands.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum SurvivorGameState {
    Upcoming,
    Live,
    Final,
    /// Postponed or cancelled. Costs whoever picked either side nothing.
    Abandoned,
}

/// One of the date's games, for the pick screen.
///
/// Who beat whom is still not decided from this — a settlement reads the
/// league's feed for that, and a settled date's legal teams come from the
/// pool's own copy of the fixture list. What it does decide is the deadline:
/// [`SurvivorGame::has_started`] is what closes a team to new picks, and the
/// scores and the clock beside it are there so somebody picking can see who a
/// team is up against and how the game is going.
#[derive(Debug, Serialize, Clone)]
pub struct SurvivorGame {
    pub home_team_id: u32,
    pub away_team_id: u32,
    /// As the league reported it. Absent for a game still to come, and real
    /// but not yet meaning anything while the game is live — `state` is what
    /// tells those apart, and nothing is decided until it is `Final`.
    pub home_score: Option<u32>,
    pub away_score: Option<u32>,
    pub state: SurvivorGameState,
    /// When the puck drops, RFC 3339, for a game still to come.
    pub start_time_utc: Option<String>,
    /// The period being played, or the one a finished game ended in.
    pub period: Option<u32>,
    /// `REG`, `OT` or `SO`, so a game settled past regulation can say so.
    pub period_type: Option<String>,
    /// Left in the period. Only meaningful while the game is live.
    pub time_remaining: Option<String>,
    pub in_intermission: bool,
}

impl SurvivorGame {
    /// Whether the puck has dropped, which is what closes both of its sides to
    /// a new pick.
    ///
    /// A game the feed still calls upcoming though its start time has passed
    /// counts as started: that is our copy of the scoreboard being behind, not
    /// the puck still on the bench, and reading it the other way round is how
    /// somebody would get a pick in on a game already under way.
    ///
    /// A game with no start time at all and no state but `Upcoming` is the one
    /// case left open. There is nothing to judge it by, and refusing every
    /// pick on a date the feed described oddly would cost more than it saves —
    /// the state alone closes it the moment anything happens to it.
    pub fn has_started(&self, now: DateTime<Utc>) -> bool {
        if !matches!(self.state, SurvivorGameState::Upcoming) {
            return true;
        }

        self.start_time_utc
            .as_deref()
            .and_then(|start| DateTime::parse_from_rfc3339(start).ok())
            .is_some_and(|start| start.with_timezone(&Utc) <= now)
    }
}

/// The teams whose game has begun, and so can no longer be picked.
pub fn teams_closed_to_picks(games: &[SurvivorGame], now: DateTime<Utc>) -> HashSet<u32> {
    games
        .iter()
        .filter(|game| game.has_started(now))
        .flat_map(|game| [game.away_team_id, game.home_team_id])
        .collect()
}

/// Whether a date has nothing left to pick, because every game on it has
/// started.
///
/// This is the pool's deadline, and it is derived rather than declared: a pick
/// stands until its own game starts, so the date as a whole closes when the
/// last of its games does. It used to be a button the owner pressed, which
/// meant a pool whose owner was away either stayed open past puck drop or shut
/// hours early.
///
/// It is also the moment the date's picks become everybody's to see. Nothing
/// can change once it has passed, so there is nothing left for the secrecy to
/// protect — and the reveal is most of what people come to the page for.
///
/// False for a date with no games at all, and false when `games` is empty
/// because the league's feed could not be reached. A date is not closed on the
/// strength of an answer we never got: the open side of that guess refuses a
/// late pick (the pick path requires the feed) rather than publishing the
/// field's picks to somebody who has yet to make one.
pub fn pick_window_closed(games: &[SurvivorGame], now: DateTime<Utc>) -> bool {
    !games.is_empty() && games.iter().all(|game| game.has_started(now))
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
    /// Of the teams playing, the ones whose game has already started.
    ///
    /// Closed to a pick however much of this participant's own list is still
    /// open, and kept apart from `used_team_ids` because the screen has to say
    /// which of the two reasons applies: a used team is closed for the rest of
    /// the cycle, a started one was closed by the clock.
    pub started_team_ids: Vec<u32>,
    pub current_pick: Option<u32>,
    /// True when nothing is open to them and the date costs them nothing.
    ///
    /// The used-team rule only. A team whose game has started is not in this:
    /// having let the deadline pass is not the same as having had nothing to
    /// pick, and only the second is forgiven when the date settles.
    pub is_blocked: bool,
    pub can_pick: bool,
    /// True once every game of the date has started: nothing is left to pick
    /// and the date's picks are public. See [`pick_window_closed`].
    pub pick_window_closed: bool,
    /// The date's games, so the screen can show who plays whom.
    ///
    /// Empty when the league's feed could not be reached, which is not an
    /// error: the matchups are a nicety and the teams above are what a pick
    /// actually needs.
    pub games: Vec<SurvivorGame>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(instant: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(instant)
            .unwrap()
            .with_timezone(&Utc)
    }

    /// An upcoming game between two teams, dropping the puck at `start`.
    fn upcoming(away_team_id: u32, home_team_id: u32, start: Option<&str>) -> SurvivorGame {
        SurvivorGame {
            home_team_id,
            away_team_id,
            home_score: None,
            away_score: None,
            state: SurvivorGameState::Upcoming,
            start_time_utc: start.map(str::to_string),
            period: None,
            period_type: None,
            time_remaining: None,
            in_intermission: false,
        }
    }

    fn in_state(mut game: SurvivorGame, state: SurvivorGameState) -> SurvivorGame {
        game.state = state;
        game
    }

    #[test]
    fn a_game_starts_when_its_start_time_passes() {
        let game = upcoming(8, 10, Some("2026-10-17T23:00:00Z"));

        assert!(!game.has_started(at("2026-10-17T22:59:59Z")));
        // On the minute, not a second after it: the deadline is the drop.
        assert!(game.has_started(at("2026-10-17T23:00:00Z")));
        assert!(game.has_started(at("2026-10-18T02:00:00Z")));
    }

    #[test]
    fn a_game_the_feed_has_moved_on_has_started_whatever_its_start_time_says() {
        let before_everything = at("2020-01-01T00:00:00Z");

        // The reason this is not judged on the clock alone: a game being
        // played, over, or called off is closed to a pick even if our copy of
        // its start time is wrong or missing.
        for state in [
            SurvivorGameState::Live,
            SurvivorGameState::Final,
            SurvivorGameState::Abandoned,
        ] {
            let game = in_state(upcoming(8, 10, Some("2026-10-17T23:00:00Z")), state);
            assert!(game.has_started(before_everything), "{state:?}");
        }
    }

    #[test]
    fn an_upcoming_game_with_no_start_time_stays_open() {
        // Nothing to judge it by. Refusing every pick on a date the feed
        // described oddly would cost more than the rule saves.
        assert!(!upcoming(8, 10, None).has_started(at("2026-10-17T23:00:00Z")));
        assert!(!upcoming(8, 10, Some("not a date")).has_started(at("2026-10-17T23:00:00Z")));
    }

    #[test]
    fn both_sides_of_a_started_game_are_closed() {
        let games = vec![
            upcoming(8, 10, Some("2026-10-17T17:00:00Z")),
            upcoming(12, 14, Some("2026-10-18T00:00:00Z")),
        ];

        let closed = teams_closed_to_picks(&games, at("2026-10-17T18:00:00Z"));

        assert_eq!(closed, HashSet::from([8, 10]));
    }

    #[test]
    fn the_window_closes_with_the_last_game_of_the_date() {
        let games = vec![
            upcoming(8, 10, Some("2026-10-17T17:00:00Z")),
            upcoming(12, 14, Some("2026-10-18T00:00:00Z")),
        ];

        // The afternoon game is gone, so its two teams are; the evening one is
        // still there to be picked, and the date with it. This is the whole
        // point of a per-game deadline rather than one for the date.
        assert!(!pick_window_closed(&games, at("2026-10-17T18:00:00Z")));
        assert!(pick_window_closed(&games, at("2026-10-18T00:00:00Z")));
    }

    #[test]
    fn a_date_with_no_games_is_not_a_date_whose_deadline_passed() {
        // Which is also the answer when the league's feed could not be
        // reached: nothing is closed on the strength of an answer we never
        // got, because that is the side that keeps the field's picks private.
        assert!(!pick_window_closed(&[], at("2026-10-17T18:00:00Z")));
    }
}
