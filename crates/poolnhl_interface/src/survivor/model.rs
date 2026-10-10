//! The survivor pool domain model.
//!
//! A survivor pool is a different game from the roster pool in
//! [`crate::pool::model`]: there is no draft, no roster and no scoring. Every
//! pick date — a Saturday — each participant names one team they believe will
//! win that day. Get it right and you go through; get it wrong and you are out.
//! A team a participant has already used is closed to them until they have been
//! through the whole league.
//!
//! It is kept apart from `Pool` rather than folded into it as a variant, for two
//! reasons. A survivor pool shares none of the roster pool's fields, so sharing
//! the type would mean making most of them optional and branching on the pool
//! kind throughout a 3000-line model. And it is built for hundreds of
//! participants, where the roster pool's "read the whole document, change it,
//! write it back under an optimistic lock" shape would turn a Saturday morning
//! into a storm of write conflicts — so the picks live in their own collection,
//! one document each, and this module never holds them.

use std::collections::{HashMap, HashSet};
use std::fmt;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// Participant names are displayed in every standings row of the pool; a longer
/// one would be cut with an ellipsis everywhere it appears.
pub const MAX_PARTICIPANT_NAME_LENGTH: usize = 32;

/// The ceiling the owner's own `max_participants` is itself held to. The
/// participant list is one array inside the pool document, so it cannot grow
/// without bound; a thousand names is still only tens of kilobytes.
pub const MAX_SURVIVOR_PARTICIPANTS: u16 = 1000;

/// Teams in the league, used to tell "this participant has been through every
/// team" from "this participant still has teams left". A pool that opened in a
/// season with a different count keeps its own copy in the settings, so this is
/// only the default.
pub const DEFAULT_LEAGUE_TEAM_COUNT: u16 = 32;

/// What a survivor pool is called in the pool listing and in URLs.
pub const MIN_SURVIVOR_POOL_NAME_LENGTH: usize = 3;
pub const MAX_SURVIVOR_POOL_NAME_LENGTH: usize = 64;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum SurvivorState {
    /// Open for people to join. No pick date has been settled yet.
    Created,
    /// Running: picks are being made and weeks settled.
    InProgress,
    /// Over. `winners` holds whoever was left.
    Final,
}

impl fmt::Display for SurvivorState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SurvivorState::Created => write!(f, "Created"),
            SurvivorState::InProgress => write!(f, "InProgress"),
            SurvivorState::Final => write!(f, "Final"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum ParticipantStatus {
    Alive,
    Eliminated,
}

/// How a pick turned out once its date was settled.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum PickOutcome {
    /// The date has not been settled yet.
    Pending,
    /// The picked team won.
    Won,
    /// The picked team lost.
    Lost,
    /// The picked team's game did not produce a result — postponed, or still
    /// unplayed when the date was settled. Costs the participant nothing.
    Void,
}

/// Where a pick date stands.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum WeekStatus {
    /// Still accepting picks.
    Open,
    /// Past its deadline. Picks are revealed to everyone and no longer change.
    Locked,
    /// Results are in and eliminations have been applied.
    Settled,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorSettings {
    /// Participants allowed to settle a date and edit the pool alongside the
    /// owner.
    pub assistants: Vec<String>,

    /// How many people may join. Held to [`MAX_SURVIVOR_PARTICIPANTS`].
    pub max_participants: u16,

    /// Losses a participant survives. 0 is classic survivor — one wrong pick and
    /// you are out. Large pools often allow one, so a single upset does not end
    /// most of the field in week one.
    pub strikes_allowed: u8,

    /// Whether failing to pick costs a strike (`true`) or eliminates outright
    /// (`false`). It never costs anything when the participant had no legal
    /// pick available — see [`SurvivorPool::blocked_participants`].
    pub missed_pick_is_strike: bool,

    /// Whether a participant may change their pick while the date is still open.
    pub allow_pick_change: bool,

    /// Teams in the league this pool runs over. Decides when a participant has
    /// been through them all and their used list resets.
    pub league_team_count: u16,
}

impl Default for SurvivorSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl SurvivorSettings {
    pub fn new() -> Self {
        Self {
            assistants: Vec::new(),
            max_participants: 100,
            strikes_allowed: 0,
            missed_pick_is_strike: true,
            allow_pick_change: true,
            league_team_count: DEFAULT_LEAGUE_TEAM_COUNT,
        }
    }

    /// Reject settings a pool could not run with, before one is created or
    /// updated with them.
    pub fn validate(&self) -> Result<(), AppError> {
        if self.max_participants < 2 {
            return Err(AppError::CustomError {
                msg: "A survivor pool needs room for at least 2 participants.".to_string(),
            });
        }

        if self.max_participants > MAX_SURVIVOR_PARTICIPANTS {
            return Err(AppError::CustomError {
                msg: format!(
                    "A survivor pool cannot hold more than {MAX_SURVIVOR_PARTICIPANTS} participants."
                ),
            });
        }

        if self.league_team_count < 2 {
            return Err(AppError::CustomError {
                msg: "A survivor pool needs at least 2 teams to pick from.".to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorUser {
    pub id: String,
    pub name: String,
    pub status: ParticipantStatus,
    /// Losses recorded so far. Elimination happens once this passes
    /// `strikes_allowed`.
    pub strikes: u8,
    /// The pick date that knocked them out, when they are out.
    pub eliminated_week: Option<u16>,
    pub date_joined: i64,

    /// Whether this is somebody signed in under their own account, or a spot
    /// the organiser keeps on their behalf.
    ///
    /// The same distinction `PoolUser` draws. A survivor pool is built for
    /// hundreds of people signing themselves up, but a pool of six friends is
    /// usually one organiser entering everybody — and then nobody but them has
    /// an account to pick with, so the organiser picks for them too.
    ///
    /// Defaults to `true` because every participant that predates the flag
    /// joined by signing in.
    #[serde(default = "owned_by_default")]
    pub is_owned: bool,
}

fn owned_by_default() -> bool {
    true
}

impl SurvivorUser {
    /// Somebody who signed themselves up.
    pub fn new(id: &str, name: &str, date_joined: i64) -> Self {
        Self::with_ownership(id, name, date_joined, true)
    }

    /// A spot the organiser keeps on somebody's behalf.
    pub fn managed(id: &str, name: &str, date_joined: i64) -> Self {
        Self::with_ownership(id, name, date_joined, false)
    }

    fn with_ownership(id: &str, name: &str, date_joined: i64, is_owned: bool) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            status: ParticipantStatus::Alive,
            strikes: 0,
            eliminated_week: None,
            date_joined,
            is_owned,
        }
    }

    pub fn is_alive(&self) -> bool {
        matches!(self.status, ParticipantStatus::Alive)
    }
}

/// One pick date of the pool.
///
/// `eligible_team_ids` is filled in from the schedule the first time the date is
/// looked at and is what a pick is validated against. It stays empty until then,
/// which is why [`SurvivorPool::week`] hands back the week rather than the list.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorWeek {
    /// 1-based, and the key a pick carries.
    pub week: u16,
    /// The Saturday this week's games are played on, `yyyy-mm-dd`.
    pub pick_date: String,
    pub status: WeekStatus,
    /// Teams with a game on `pick_date`. Empty until the schedule is read.
    pub eligible_team_ids: Vec<u32>,
    pub settled_at: Option<i64>,
}

/// A participant's pick, as the pool reads it back when deciding what is still
/// open to them. The pick documents themselves live in their own collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsedTeam {
    pub cycle: u16,
    pub team_id: u32,
}

/// A survivor pool as the pool listing shows it, without its weeks or its
/// participant list.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorPoolShort {
    pub name: String,
    pub owner: String,
    pub status: SurvivorState,
    pub season: u32,
    pub participant_count: u16,
    pub max_participants: u16,
}

impl From<&SurvivorPool> for SurvivorPoolShort {
    fn from(pool: &SurvivorPool) -> Self {
        Self {
            name: pool.name.clone(),
            owner: pool.owner.clone(),
            status: pool.status.clone(),
            season: pool.season,
            participant_count: pool.participants.len() as u16,
            max_participants: pool.settings.max_participants,
        }
    }
}

/// A pick as the standings grid shows it, once its date has been revealed.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RevealedPick {
    pub team_id: u32,
    pub outcome: PickOutcome,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorStandingRow {
    pub participant_id: String,
    pub name: String,
    pub status: ParticipantStatus,
    pub strikes: u8,
    pub eliminated_week: Option<u16>,
    /// Dates this participant came through, which is how the standings sort.
    pub wins: u16,
    /// Their pick per revealed date. Open dates are absent, not empty: what the
    /// field picked before a date locks is the one thing that would break the
    /// game to show.
    pub picks: HashMap<u16, RevealedPick>,
}

/// The standings, counted server-side.
///
/// A pool of hundreds would otherwise ship every pick of every date to the
/// browser for it to aggregate, on a page people reload all Saturday evening.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorStandings {
    pub pool_name: String,
    pub status: SurvivorState,
    pub alive_count: u16,
    pub eliminated_count: u16,
    /// The dates whose picks are public, in order.
    pub revealed_weeks: Vec<u16>,
    pub rows: Vec<SurvivorStandingRow>,
    pub winners: Option<Vec<String>>,
}

/// A survivor pool, without its picks.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SurvivorPool {
    pub name: String,
    pub owner: String,

    pub settings: SurvivorSettings,
    pub status: SurvivorState,

    pub participants: Vec<SurvivorUser>,
    pub weeks: Vec<SurvivorWeek>,

    /// Whoever was left when the pool ended. More than one when a date took out
    /// everybody still standing — they share it rather than the pool ending with
    /// nobody.
    pub winners: Option<Vec<String>>,

    /// Doubles as the optimistic-locking version, the same way `Pool` uses it.
    pub date_updated: i64,

    pub season: u32,
    pub season_start: String,
    pub season_end: String,
}

impl SurvivorPool {
    /// A new pool, with a week per Saturday of the season.
    pub fn new(
        name: &str,
        owner: &str,
        settings: &SurvivorSettings,
        season: u32,
        season_start: &str,
        season_end: &str,
    ) -> Result<Self, AppError> {
        validate_pool_name(name)?;
        settings.validate()?;

        let weeks = saturdays_between(season_start, season_end)?
            .into_iter()
            .enumerate()
            .map(|(index, pick_date)| SurvivorWeek {
                // 1-based: "week 1" is what a participant is told they are
                // picking for.
                week: index as u16 + 1,
                pick_date,
                status: WeekStatus::Open,
                eligible_team_ids: Vec::new(),
                settled_at: None,
            })
            .collect::<Vec<_>>();

        if weeks.is_empty() {
            return Err(AppError::CustomError {
                msg: format!(
                    "There is no Saturday between {season_start} and {season_end} to pick on."
                ),
            });
        }

        Ok(Self {
            name: name.to_string(),
            owner: owner.to_string(),
            settings: settings.clone(),
            status: SurvivorState::Created,
            participants: Vec::new(),
            weeks,
            winners: None,
            date_updated: 0,
            season,
            season_start: season_start.to_string(),
            season_end: season_end.to_string(),
        })
    }

    pub fn week(&self, week: u16) -> Result<&SurvivorWeek, AppError> {
        self.weeks
            .iter()
            .find(|candidate| candidate.week == week)
            .ok_or_else(|| AppError::NotFound {
                msg: format!("'{}' has no week {week}.", self.name),
            })
    }

    pub fn week_mut(&mut self, week: u16) -> Result<&mut SurvivorWeek, AppError> {
        let name = self.name.clone();
        self.weeks
            .iter_mut()
            .find(|candidate| candidate.week == week)
            .ok_or_else(|| AppError::NotFound {
                msg: format!("'{name}' has no week {week}."),
            })
    }

    pub fn participant(&self, user_id: &str) -> Option<&SurvivorUser> {
        self.participants
            .iter()
            .find(|participant| participant.id == user_id)
    }

    pub fn alive_participants(&self) -> impl Iterator<Item = &SurvivorUser> {
        self.participants
            .iter()
            .filter(|participant| participant.is_alive())
    }

    /// The first week not settled yet — what the pool is currently playing for.
    pub fn current_week(&self) -> Option<&SurvivorWeek> {
        self.weeks
            .iter()
            .find(|week| !matches!(week.status, WeekStatus::Settled))
    }

    pub fn has_assistant_rights(&self, user_id: &str) -> bool {
        self.owner == user_id || self.settings.assistants.iter().any(|id| id == user_id)
    }

    pub fn has_owner_rights(&self, user_id: &str) -> bool {
        self.owner == user_id
    }

    pub fn validate_assistant_rights(&self, user_id: &str) -> Result<(), AppError> {
        if self.has_assistant_rights(user_id) {
            return Ok(());
        }
        Err(AppError::ForbiddenError {
            msg: format!("Only the owner of '{}' can do this.", self.name),
        })
    }

    pub fn validate_owner_rights(&self, user_id: &str) -> Result<(), AppError> {
        if self.has_owner_rights(user_id) {
            return Ok(());
        }
        Err(AppError::ForbiddenError {
            msg: format!("Only the owner of '{}' can do this.", self.name),
        })
    }

    /// Add a participant, as whoever is signing themselves up.
    ///
    /// Joining is self-serve on purpose: a pool for hundreds of people cannot
    /// have its owner enter every name by hand the way the roster pool does.
    pub fn add_participant(
        &mut self,
        user_id: &str,
        name: &str,
        date_joined: i64,
    ) -> Result<(), AppError> {
        let name = self.validate_new_participant(user_id, name)?;

        self.participants
            .push(SurvivorUser::new(user_id, &name, date_joined));

        Ok(())
    }

    /// Add a spot the organiser keeps on somebody's behalf.
    ///
    /// For the pool of six friends where one person enters everybody. `id` is
    /// generated by the caller rather than taken from a request: an id that
    /// could be named would let an organiser attach a pool to somebody else's
    /// account, and the people this creates have no account at all.
    ///
    /// Open to the assistants as well as the owner — they already settle dates,
    /// which eliminates people, so entering one is the lesser power.
    pub fn add_managed_participant(
        &mut self,
        acting_user_id: &str,
        id: &str,
        name: &str,
        now: i64,
    ) -> Result<(), AppError> {
        self.validate_assistant_rights(acting_user_id)?;

        let name = self.validate_new_participant(id, name)?;

        self.participants
            .push(SurvivorUser::managed(id, &name, now));

        Ok(())
    }

    /// The checks every new participant goes through, and their trimmed name.
    fn validate_new_participant(&self, id: &str, name: &str) -> Result<String, AppError> {
        let name = validate_participant_name(name)?;

        if matches!(self.status, SurvivorState::Final) {
            return Err(AppError::CustomError {
                msg: format!("'{}' is over.", self.name),
            });
        }

        if self.participant(id).is_some() {
            return Err(AppError::CustomError {
                msg: format!("You have already joined '{}'.", self.name),
            });
        }

        // Joining is only closed once a date has actually been settled. Until
        // then a latecomer has missed nothing.
        if self
            .weeks
            .iter()
            .any(|week| matches!(week.status, WeekStatus::Settled))
        {
            return Err(AppError::CustomError {
                msg: format!("'{}' has already started.", self.name),
            });
        }

        if self.participants.len() >= self.settings.max_participants as usize {
            return Err(AppError::CustomError {
                msg: format!("'{}' is full.", self.name),
            });
        }

        if self
            .participants
            .iter()
            .any(|participant| participant.name == name)
        {
            return Err(AppError::CustomError {
                msg: format!("Somebody in '{}' already goes by '{name}'.", self.name),
            });
        }

        Ok(name)
    }

    /// Remove a participant. Theirs to do while the pool has not started; the
    /// owner's at any point.
    pub fn remove_participant(
        &mut self,
        acting_user_id: &str,
        removed_user_id: &str,
    ) -> Result<(), AppError> {
        if acting_user_id != removed_user_id && !self.has_owner_rights(acting_user_id) {
            return Err(AppError::ForbiddenError {
                msg: "Only the owner can remove somebody else from the pool.".to_string(),
            });
        }

        let before = self.participants.len();
        self.participants
            .retain(|participant| participant.id != removed_user_id);

        if self.participants.len() == before {
            return Err(AppError::NotFound {
                msg: format!("'{removed_user_id}' is not in '{}'.", self.name),
            });
        }

        Ok(())
    }

    /// Check a participant may pick for `week` at all, before the team itself is
    /// looked at.
    pub fn validate_can_pick(&self, user_id: &str, week: u16) -> Result<(), AppError> {
        self.validate_can_pick_for(user_id, user_id, week)
    }

    /// Whether `acting_user_id` may file `participant_id`'s pick for `week`.
    ///
    /// A participant files their own; the owner and the assistants file
    /// anyone's, which is what makes a pool of managed spots playable at all —
    /// those people have no account to pick with. The same rule the roster
    /// pool applies to a free-agency swap.
    pub fn validate_can_pick_for(
        &self,
        acting_user_id: &str,
        participant_id: &str,
        week: u16,
    ) -> Result<(), AppError> {
        if acting_user_id != participant_id && !self.has_assistant_rights(acting_user_id) {
            return Err(AppError::ForbiddenError {
                msg: format!(
                    "Only the owner of '{}' can pick for somebody else.",
                    self.name
                ),
            });
        }

        // Written for whoever reads it: the participant when they are filing
        // their own, the organiser when they are filing somebody else's.
        let picking_for_self = acting_user_id == participant_id;

        let participant =
            self.participant(participant_id)
                .ok_or_else(|| AppError::ForbiddenError {
                    msg: if picking_for_self {
                        format!("You are not in '{}'.", self.name)
                    } else {
                        format!("'{participant_id}' is not in '{}'.", self.name)
                    },
                })?;

        if !participant.is_alive() {
            return Err(AppError::CustomError {
                msg: if picking_for_self {
                    "You have been eliminated from this pool.".to_string()
                } else {
                    format!("{} has been eliminated from this pool.", participant.name)
                },
            });
        }

        if matches!(self.status, SurvivorState::Final) {
            return Err(AppError::CustomError {
                msg: format!("'{}' is over.", self.name),
            });
        }

        match self.week(week)?.status {
            WeekStatus::Open => Ok(()),
            WeekStatus::Locked => Err(AppError::CustomError {
                msg: format!("Week {week} is locked, its picks are in."),
            }),
            WeekStatus::Settled => Err(AppError::CustomError {
                msg: format!("Week {week} has already been settled."),
            }),
        }
    }

    /// Apply a settled date's results: record the strikes it cost and eliminate
    /// whoever ran out of them.
    ///
    /// `outcomes` holds an entry per alive participant that picked. An alive
    /// participant missing from it did not pick, and is treated per
    /// `missed_pick_is_strike` — unless they are in `blocked`, in which case
    /// they had no legal pick to make and it costs them nothing.
    pub fn apply_week_results(
        &mut self,
        week: u16,
        outcomes: &HashMap<String, PickOutcome>,
        blocked: &HashSet<String>,
        now: i64,
    ) -> Result<(), AppError> {
        if matches!(self.week(week)?.status, WeekStatus::Settled) {
            return Err(AppError::CustomError {
                msg: format!("Week {week} has already been settled."),
            });
        }

        // Who was standing when the date began, so a date that takes out the
        // whole remaining field can hand it to them jointly instead of ending
        // the pool with nobody.
        let contenders: Vec<String> = self
            .alive_participants()
            .map(|participant| participant.id.clone())
            .collect();

        let strikes_allowed = self.settings.strikes_allowed;
        let missed_pick_is_strike = self.settings.missed_pick_is_strike;

        for participant in self.participants.iter_mut() {
            if !participant.is_alive() {
                continue;
            }

            // A participant with no unused team playing that day is not
            // penalised for having nothing to pick.
            if blocked.contains(&participant.id) {
                continue;
            }

            let costs_a_strike = match outcomes.get(&participant.id) {
                Some(PickOutcome::Won) | Some(PickOutcome::Void) => false,
                Some(PickOutcome::Lost) => true,
                // Still pending at settle time means the game never resolved;
                // treat it the way a void is rather than punishing the pick.
                Some(PickOutcome::Pending) => false,
                None if missed_pick_is_strike => true,
                None => {
                    participant.status = ParticipantStatus::Eliminated;
                    participant.eliminated_week = Some(week);
                    continue;
                }
            };

            if !costs_a_strike {
                continue;
            }

            participant.strikes = participant.strikes.saturating_add(1);
            if participant.strikes > strikes_allowed {
                participant.status = ParticipantStatus::Eliminated;
                participant.eliminated_week = Some(week);
            }
        }

        let week_entry = self.week_mut(week)?;
        week_entry.status = WeekStatus::Settled;
        week_entry.settled_at = Some(now);

        self.status = SurvivorState::InProgress;
        self.conclude_if_decided(&contenders);

        Ok(())
    }

    /// End the pool when the field is down to one, or to nobody.
    ///
    /// `contenders` is who was alive before the date that just settled: when a
    /// date eliminates everybody left, they share the win rather than the pool
    /// ending with no winner at all.
    fn conclude_if_decided(&mut self, contenders: &[String]) {
        let still_alive: Vec<String> = self
            .alive_participants()
            .map(|participant| participant.id.clone())
            .collect();

        let every_week_settled = self
            .weeks
            .iter()
            .all(|week| matches!(week.status, WeekStatus::Settled));

        if still_alive.is_empty() && !contenders.is_empty() {
            self.status = SurvivorState::Final;
            self.winners = Some(contenders.to_vec());
            return;
        }

        if still_alive.len() == 1 || (every_week_settled && !still_alive.is_empty()) {
            self.status = SurvivorState::Final;
            self.winners = Some(still_alive);
        }
    }
}

/// Reject a pool name a pool could not be looked up by.
pub fn validate_pool_name(name: &str) -> Result<(), AppError> {
    let trimmed = name.trim();

    if trimmed.chars().count() < MIN_SURVIVOR_POOL_NAME_LENGTH {
        return Err(AppError::CustomError {
            msg: format!("A pool name needs at least {MIN_SURVIVOR_POOL_NAME_LENGTH} characters."),
        });
    }

    if trimmed.chars().count() > MAX_SURVIVOR_POOL_NAME_LENGTH {
        return Err(AppError::CustomError {
            msg: format!(
                "A pool name cannot be longer than {MAX_SURVIVOR_POOL_NAME_LENGTH} characters."
            ),
        });
    }

    Ok(())
}

/// The trimmed participant name, when it is one.
pub fn validate_participant_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(AppError::CustomError {
            msg: "A participant needs a name.".to_string(),
        });
    }

    if trimmed.chars().count() > MAX_PARTICIPANT_NAME_LENGTH {
        return Err(AppError::CustomError {
            msg: format!("A name cannot be longer than {MAX_PARTICIPANT_NAME_LENGTH} characters."),
        });
    }

    Ok(trimmed.to_string())
}

/// Every Saturday in `[start, end]`, as `yyyy-mm-dd`.
///
/// These are the pool's pick dates: a survivor pool plays one round a week, on
/// the day the league schedules most of its games.
pub fn saturdays_between(start: &str, end: &str) -> Result<Vec<String>, AppError> {
    let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d")?;
    let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d")?;

    if end_date < start_date {
        return Err(AppError::CustomError {
            msg: format!("'{end}' is before '{start}'."),
        });
    }

    // Jump straight to the first Saturday rather than walking a day at a time.
    let days_to_saturday = (Weekday::Sat.num_days_from_monday() as i64
        - start_date.weekday().num_days_from_monday() as i64)
        .rem_euclid(7);

    let mut saturdays = Vec::new();
    let mut cursor = start_date + Duration::days(days_to_saturday);

    while cursor <= end_date {
        saturdays.push(cursor.format("%Y-%m-%d").to_string());
        cursor += Duration::days(7);
    }

    Ok(saturdays)
}

/// The cycle a participant's used teams currently sit in.
///
/// A cycle is one pass through the league. It advances only once a participant
/// has used every team, at which point all of them open up again.
pub fn current_cycle(used: &[UsedTeam]) -> u16 {
    used.iter().map(|team| team.cycle).max().unwrap_or(0)
}

/// Teams a participant has already used in their current cycle.
pub fn used_in_current_cycle(used: &[UsedTeam]) -> HashSet<u32> {
    let cycle = current_cycle(used);
    used.iter()
        .filter(|team| team.cycle == cycle)
        .map(|team| team.team_id)
        .collect()
}

/// Which of `eligible_team_ids` the participant may still pick.
///
/// This is what the pick screen greys out, and it is derived from the same
/// function the pick itself is validated with, so the two cannot disagree.
pub fn available_team_ids(
    used: &[UsedTeam],
    eligible_team_ids: &[u32],
    league_team_count: u16,
) -> Vec<u32> {
    let spent = used_in_current_cycle(used);

    // Been through the whole league: the used list resets and everything
    // playing that day is open again.
    if spent.len() >= league_team_count as usize {
        return eligible_team_ids.to_vec();
    }

    eligible_team_ids
        .iter()
        .copied()
        .filter(|team_id| !spent.contains(team_id))
        .collect()
}

/// The cycle a pick of `team_id` belongs in, or why it cannot be made.
///
/// The returned cycle is what the pick document carries, and what the unique
/// index on `(pool, participant, cycle, team)` enforces — so two picks racing
/// each other cannot both land the same team.
pub fn plan_pick(
    used: &[UsedTeam],
    eligible_team_ids: &[u32],
    league_team_count: u16,
    team_id: u32,
) -> Result<u16, AppError> {
    if !eligible_team_ids.contains(&team_id) {
        return Err(AppError::CustomError {
            msg: "That team does not play on this pick date.".to_string(),
        });
    }

    let cycle = current_cycle(used);
    let spent = used_in_current_cycle(used);

    if !spent.contains(&team_id) {
        return Ok(cycle);
    }

    // Already used, and there is still a team left to use: the pick is closed.
    if spent.len() < league_team_count as usize {
        let remaining = available_team_ids(used, eligible_team_ids, league_team_count).len();
        return Err(AppError::CustomError {
            msg: format!(
                "You have already used this team. {remaining} of the teams playing \
                 that day are still open to you."
            ),
        });
    }

    // Been through every team in the league, so the used list resets and this
    // pick opens the next cycle.
    Ok(cycle + 1)
}

/// Whether a participant has no legal pick for a date, and so cannot be
/// penalised for not making one.
///
/// It happens when every team playing that day is one they have already used
/// and they have not yet been through the whole league for the list to reset —
/// and on a date the league scheduled nothing on, where nobody can pick at all.
pub fn is_blocked(used: &[UsedTeam], eligible_team_ids: &[u32], league_team_count: u16) -> bool {
    available_team_ids(used, eligible_team_ids, league_team_count).is_empty()
}

#[cfg(test)]
mod tests;
