use crate::{
    draft::model::RoomUser,
    errors::AppError,
    players::model::{PlayerInfo, Position},
    pool::lineup::LineupEvent,
    pool::scoring::DailyRosterPoints,
};
use chrono::{Duration, Local, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
// Date for season
//

pub const START_SEASON_DATE: &str = "2026-09-29";
pub const END_SEASON_DATE: &str = "2027-04-10";
pub const POOL_CREATION_SEASON: u32 = 20262027;
pub const TRADE_DEADLINE_DATE: &str = "2027-03-01";

// Pooler names are displayed in every ranking, table and chart of the pool, a
// name longer than this would be cut with an ellipsis everywhere it appears.
pub const MAX_POOLER_NAME_LENGTH: usize = 32;

/// Season date information exposed to the front end.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SeasonInfo {
    pub start_season_date: String,
    pub end_season_date: String,
    pub season: u32,
    pub trade_deadline_date: String,
}

impl SeasonInfo {
    pub fn current() -> Self {
        Self {
            start_season_date: START_SEASON_DATE.to_string(),
            end_season_date: END_SEASON_DATE.to_string(),
            season: POOL_CREATION_SEASON,
            trade_deadline_date: TRADE_DEADLINE_DATE.to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ProjectedPoolShort {
    pub name: String, // the name of the pool.
    pub owner: String,
    pub status: PoolState, // State of the pool.
    pub season: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerTypeSettings {
    // Other pool configuration
    pub forwards: u8,
    pub defense: u8,
    pub goalies: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DynastySettings {
    // Other pool configuration
    pub next_season_number_players_protected: u8,
    pub tradable_picks: u8, // numbers of the next season picks participants are able to trade with each other.
    pub past_season_pool_name: Vec<String>,
    pub next_season_pool_name: Option<String>,
}

impl PartialEq<DynastySettings> for DynastySettings {
    fn eq(&self, other: &DynastySettings) -> bool {
        self.next_season_number_players_protected == other.next_season_number_players_protected
            && self.tradable_picks == other.tradable_picks
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SkaterSettings {
    pub points_per_goals: u8,
    pub points_per_assists: u8,
    pub points_per_hattricks: u8,
    pub points_per_shootout_goals: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoaliesSettings {
    pub points_per_wins: u8,
    pub points_per_shutouts: u8,
    pub points_per_overtimes: u8,
    pub points_per_goals: u8,
    pub points_per_assists: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum DraftType {
    Serpentine,
    Standard,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PoolSettings {
    pub assistants: Vec<String>, // Participants that are allowed to make some pool modifications.

    pub number_poolers: u8,
    pub draft_type: DraftType,

    // Roster configuration.
    pub number_forwards: u8,
    pub number_defenders: u8,
    pub number_goalies: u8,
    pub number_reservists: u8,

    pub salary_cap: Option<f64>,

    // Date where where roster modification are allowed to everyone.
    pub roster_modification_date: Vec<String>,

    pub forwards_settings: SkaterSettings,
    pub defense_settings: SkaterSettings,
    pub goalies_settings: GoaliesSettings,

    pub ignore_x_worst_players: Option<PlayerTypeSettings>,
    pub dynasty_settings: Option<DynastySettings>,
}

impl Default for PoolSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolSettings {
    pub fn new() -> Self {
        Self {
            number_poolers: 6,
            draft_type: DraftType::Serpentine,
            assistants: Vec::new(),
            number_forwards: 9,
            number_defenders: 4,
            number_goalies: 2,
            number_reservists: 2,
            salary_cap: None,
            roster_modification_date: Vec::new(),
            forwards_settings: SkaterSettings {
                points_per_goals: 2,
                points_per_assists: 1,
                points_per_hattricks: 3,
                points_per_shootout_goals: 1,
            },
            defense_settings: SkaterSettings {
                points_per_goals: 3,
                points_per_assists: 2,
                points_per_hattricks: 2,
                points_per_shootout_goals: 1,
            },
            goalies_settings: GoaliesSettings {
                points_per_wins: 2,
                points_per_shutouts: 3,
                points_per_goals: 3,
                points_per_assists: 2,
                points_per_overtimes: 1,
            },
            ignore_x_worst_players: None,
            dynasty_settings: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PoolUser {
    pub id: String,
    pub name: String,

    // tells if the user is owned by an app users or manage by the pool owner
    pub is_owned: bool,
}

impl From<RoomUser> for PoolUser {
    fn from(room_user: RoomUser) -> Self {
        PoolUser {
            id: room_user.id,
            name: room_user.name,
            is_owned: room_user.email.is_some(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pool {
    pub name: String, // the name of the pool.
    pub owner: String,

    pub participants: Vec<PoolUser>, // The ID of each participants.

    pub settings: PoolSettings,

    pub status: PoolState, // State of the pool.

    // When the pool is complete, this stored the pool final rank.
    pub final_rank: Option<Vec<String>>,

    // When the draft is on, this is filled up with the draft order.
    pub draft_order: Option<Vec<String>>,

    // Trade information.
    pub trades: Option<Vec<Trade>>,

    // context of the pool.
    pub context: Option<PoolContext>,
    pub date_updated: i64,
    pub season_start: String,
    pub season_end: String,
    pub season: u32, // 20232024
}

impl Pool {
    pub fn new(pool_name: &str, owner: &str, pool_settings: &PoolSettings) -> Self {
        Self {
            name: pool_name.to_string(),
            owner: owner.to_string(),
            participants: Vec::new(),
            settings: pool_settings.clone(),
            status: PoolState::Created,
            final_rank: None,
            draft_order: None,
            trades: None,
            context: None,
            date_updated: 0,
            season_start: START_SEASON_DATE.to_string(),
            season_end: END_SEASON_DATE.to_string(),
            season: POOL_CREATION_SEASON,
        }
    }

    pub fn create_trade(&mut self, trade: &mut Trade, user_id: &str) -> Result<(), AppError> {
        self.create_trade_at(trade, user_id, Local::now().date_naive())
    }

    // Same as create_trade, with the current date injected so the trade
    // deadline rule can be exercised in tests.
    pub fn create_trade_at(
        &mut self,
        trade: &mut Trade,
        user_id: &str,
        today: NaiveDate,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;
        // Create a trade in the pool if it is valid to do so..
        let trade_deadline_date = NaiveDate::parse_from_str(TRADE_DEADLINE_DATE, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        if today > trade_deadline_date {
            return Err(AppError::CustomError {
                msg: "Trade cannot be created after the trade deadline.".to_string(),
            });
        }

        // If the user is not the one who proposed the trade it needs to have privileges.
        if user_id != trade.proposed_by {
            self.has_privileges(user_id)?;
        }

        let context = self.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        context.validate_trade(trade)?;

        // does the proposedBy and askTo field are valid

        if !context.pooler_roster.contains_key(&trade.proposed_by)
            || !context.pooler_roster.contains_key(&trade.ask_to)
        {
            return Err(AppError::CustomError {
                msg: "The users in the trade are not in the pool.".to_string(),
            });
        }
        if self.trades.is_none() {
            self.trades = Some(Vec::new());
        }

        if let Some(trades) = &mut self.trades {
            // Make sure that user can only have 1 active trade at a time.
            //return an error if already one trade active in this pool. (Active trade = NEW )
            for existing_trade in trades.iter() {
                if (matches!(existing_trade.status, TradeStatus::NEW))
                    && (existing_trade.proposed_by == trade.proposed_by)
                {
                    return Err(AppError::CustomError {
                        msg: "User can only have one active trade at a time.".to_string(),
                    });
                }
            }

            trade.date_created = Utc::now().timestamp_millis();
            trade.status = TradeStatus::NEW;
            trade.id = trades.len() as u32;
            trades.push(trade.clone());
        }

        Ok(())
    }

    pub fn delete_trade(&mut self, user_id: &str, trade_id: u32) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;

        // Owner and pool assistant can delete any new trade.
        let priviledge_right =
            self.has_owner_rights(user_id) || self.has_assistants_rights(user_id);

        let trades = self.trades.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "There is no trade to the pool yet.".to_string(),
        })?;

        let trade_index = trades
            .iter()
            .position(|trade| trade.id == trade_id)
            .ok_or_else(|| AppError::CustomError {
                msg: "The trade does not exist.".to_string(),
            })?;

        // validate that the status of the trade is NEW

        if !matches!(trades[trade_index].status, TradeStatus::NEW) {
            return Err(AppError::CustomError {
                msg: "The trade is not in a valid state to be deleted.".to_string(),
            });
        }

        // validate that only the one that create the trade or the
        // owner/assistants can delete it.

        if !priviledge_right && trades[trade_index].proposed_by != *user_id {
            return Err(AppError::CustomError {
                msg: "Only the one that created the trade can cancel it.".to_string(),
            });
        }

        trades.remove(trade_index);
        Ok(())
    }

    pub fn respond_trade(
        &mut self,
        user_id: &str,
        is_accepted: bool,
        trade_id: u32,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;

        // Owner and pool assistant can respond any new trade.
        let priviledge_right =
            self.has_owner_rights(user_id) || self.has_assistants_rights(user_id);

        let trades = self.trades.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "There is no trade to the pool yet.".to_string(),
        })?;

        let trade_index = trades
            .iter()
            .position(|trade| trade.id == trade_id)
            .ok_or_else(|| AppError::CustomError {
                msg: "The trade does not exist.".to_string(),
            })?;

        // validate that the status of the trade is NEW

        if !matches!(trades[trade_index].status, TradeStatus::NEW) {
            return Err(AppError::CustomError {
                msg: "The trade is not in a valid state to be responded.".to_string(),
            });
        }

        // validate that only the one that was ask for the trade or the owner can accept it.

        if !priviledge_right && trades[trade_index].ask_to != *user_id {
            return Err(AppError::CustomError {
                msg: "Only the one that was ask for the trade or the owner can accept it."
                    .to_string(),
            });
        }

        // validate that 24h have been passed since the trade was created.
        let now = Utc::now().timestamp_millis();

        if !priviledge_right && trades[trade_index].date_created + 86_400_000 > now {
            return Err(AppError::CustomError {
                msg: "The trade needs to be active for 24h before being able to accept it."
                    .to_string(),
            });
        }
        if is_accepted {
            match &mut self.context {
                None => Err(AppError::CustomError {
                    msg: "The pool has no context yet.".to_string(),
                }),
                Some(pool_context) => {
                    pool_context.trade_roster_items(&trades[trade_index])?;
                    trades[trade_index].status = TradeStatus::ACCEPTED;
                    trades[trade_index].date_accepted = Utc::now().timestamp_millis();
                    Ok(())
                }
            }
        } else {
            trades[trade_index].status = TradeStatus::REFUSED;
            Ok(())
        }
    }

    pub fn fill_spot(
        &mut self,
        user_id: &str,
        filled_spot_user_id: &str,
        player_id: u32,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;
        self.validate_participant(filled_spot_user_id)?;
        if user_id != filled_spot_user_id {
            self.has_privileges(user_id)?;
        }

        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        // Is the player in the pool?
        let player = context
            .players
            .get(&player_id.to_string())
            .ok_or(AppError::CustomError {
                msg: "This player is not included in the pool.".to_string(),
            })?;

        if !context.can_add_player_to_roster(player, filled_spot_user_id, &self.settings)? {
            return Err(AppError::CustomError {
                msg: format!(
                    "{} cannot be added to roster due to salary cap limit.",
                    player.name
                ),
            });
        }

        // The player should be a reservist to be filled into a the roster.
        if context.pooler_roster[filled_spot_user_id]
            .chosen_forwards
            .contains(&player.id)
            || context.pooler_roster[filled_spot_user_id]
                .chosen_defenders
                .contains(&player.id)
            || context.pooler_roster[filled_spot_user_id]
                .chosen_goalies
                .contains(&player.id)
            || !context.pooler_roster[filled_spot_user_id]
                .chosen_reservists
                .contains(&player.id)
        {
            return Err(AppError::CustomError {
                msg: "The player should only be in the reservist pooler's list.".to_string(),
            });
        }

        let mut is_added = false;

        // Add the player in the roster in its position.
        match player.position {
            Position::F => {
                if (context.pooler_roster[filled_spot_user_id]
                    .chosen_forwards
                    .len() as u8)
                    < self.settings.number_forwards
                    && let Some(x) = context.pooler_roster.get_mut(filled_spot_user_id)
                {
                    x.chosen_forwards.push(player.id);
                    is_added = true;
                }
            }
            Position::D => {
                if (context.pooler_roster[filled_spot_user_id]
                    .chosen_defenders
                    .len() as u8)
                    < self.settings.number_defenders
                    && let Some(x) = context.pooler_roster.get_mut(filled_spot_user_id)
                {
                    x.chosen_defenders.push(player.id);
                    is_added = true;
                }
            }
            Position::G => {
                if (context.pooler_roster[filled_spot_user_id]
                    .chosen_goalies
                    .len() as u8)
                    < self.settings.number_goalies
                    && let Some(x) = context.pooler_roster.get_mut(filled_spot_user_id)
                {
                    x.chosen_goalies.push(player.id);
                    is_added = true;
                }
            }
        }

        if !is_added {
            return Err(AppError::CustomError {
                msg: "There is no space for that player.".to_string(),
            });
        }
        // Removed from reservist
        if let Some(x) = context.pooler_roster.get_mut(filled_spot_user_id) {
            x.chosen_reservists
                .retain(|player_id| player_id != &player.id);
        }

        Ok(())
    }
    pub fn add_player(
        &mut self,
        user_id: &str,
        added_to_user_id: &str,
        player: &PlayerInfo,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;
        // Add a player new player into the reservists of a participant.
        self.has_privileges(user_id)?;

        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        if !context.pooler_roster.contains_key(added_to_user_id) {
            return Err(AppError::CustomError {
                msg: "The user is not in the pool.".to_string(),
            });
        }

        // First, validate that the player selected is not picked by any of the other poolers.

        for participant in self.participants.iter() {
            if context.pooler_roster[&participant.id].validate_player_possession(player.id) {
                return Err(AppError::CustomError {
                    msg: "This player is already picked.".to_string(),
                });
            }
        }

        context.add_player_to_reservists(player.id, added_to_user_id)?;

        context
            .players
            .insert(player.id.to_string(), player.clone());

        Ok(())
    }

    pub fn remove_player(
        &mut self,
        user_id: &str,
        removed_to_user_id: &str,
        player_id: u32,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;
        self.has_privileges(user_id)?;

        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        if !context.pooler_roster.contains_key(removed_to_user_id) {
            return Err(AppError::CustomError {
                msg: "The user is not in the pool.".to_string(),
            });
        }

        // First, validate that the player selected is not picked by any of the other poolers.
        if !context.pooler_roster[removed_to_user_id].validate_player_possession(player_id) {
            return Err(AppError::CustomError {
                msg: "This player is not own by the user.".to_string(),
            });
        }
        context.remove_player_from_roster(player_id, removed_to_user_id)?;
        Ok(())
    }

    pub fn modify_roster(
        &mut self,
        user_id: &str,
        roster_modified_user_id: &str,
        forw_list: &[u32],
        def_list: &[u32],
        goal_list: &[u32],
        reserv_list: &[u32],
    ) -> Result<(), AppError> {
        // Apply a roster modification. This action can only be done during the start and
        // end season on the days that the users are allowed to make roster modifications.
        // This is being hold in the variable self.settings.roster_modification_date

        let mut today = Local::now().date_naive();

        // At 12PM we start to count the action for the next day.
        if Local::now().time().hour() >= 12 {
            today += Duration::days(1);
        }

        self.modify_roster_at(
            user_id,
            roster_modified_user_id,
            forw_list,
            def_list,
            goal_list,
            reserv_list,
            today,
        )
    }

    // Same as modify_roster, with the effective date injected so the
    // allowed-dates rule can be exercised in tests.
    #[allow(clippy::too_many_arguments)]
    pub fn modify_roster_at(
        &mut self,
        user_id: &str,
        roster_modified_user_id: &str,
        forw_list: &[u32],
        def_list: &[u32],
        goal_list: &[u32],
        reserv_list: &[u32],
        today: NaiveDate,
    ) -> Result<(), AppError> {
        self.validate_pool_status_any(&[PoolState::Draft, PoolState::InProgress])?;
        self.validate_participant(roster_modified_user_id)?;

        if user_id != roster_modified_user_id {
            // If the user making the request is not the roster asking to be modified, the user need to have privilege.
            self.has_privileges(user_id)?;
        }

        let start_season_date = NaiveDate::parse_from_str(&self.season_start, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        // Make sure it is allowed to make a modification today.
        if today > start_season_date {
            let mut is_allowed = false;

            for date in &self.settings.roster_modification_date {
                let day_allowed = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

                if day_allowed == today {
                    is_allowed = true;
                    break;
                }
            }

            if !is_allowed {
                return Err(AppError::CustomError {
                    msg: format!(
                        "You are not allowed to modify your roster today. (available date: {:?})",
                        self.settings.roster_modification_date
                    )
                    .to_string(),
                });
            }
        }

        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        // Validate the total amount of forwards selected
        if forw_list.len() > self.settings.number_forwards as usize {
            return Err(AppError::CustomError {
                msg: format!(
                    "The amount of forwards selected is higher than the limit {}",
                    self.settings.number_forwards
                ),
            });
        }

        // Validate the total amount of defenders selected
        if def_list.len() > self.settings.number_defenders as usize {
            return Err(AppError::CustomError {
                msg: format!(
                    "The amount of defenders selected is higher than the limit {}",
                    self.settings.number_defenders
                ),
            });
        }

        // Validate the total amount of goalies selected
        if goal_list.len() > self.settings.number_goalies as usize {
            return Err(AppError::CustomError {
                msg: format!(
                    "The amount of goalies selected is higher than the limit {}",
                    self.settings.number_goalies
                ),
            });
        }

        let roster = context
            .pooler_roster
            .get_mut(roster_modified_user_id)
            .ok_or_else(|| AppError::CustomError {
                msg: format!(
                    "Roster for user {} does not exist.",
                    roster_modified_user_id
                ),
            })?;

        // Validate the total amount of players selected (It should be the same as before)
        let amount_selected_players =
            forw_list.len() + def_list.len() + goal_list.len() + reserv_list.len();

        let amount_players_before = roster.chosen_forwards.len()
            + roster.chosen_defenders.len()
            + roster.chosen_goalies.len()
            + roster.chosen_reservists.len();

        if amount_players_before != amount_selected_players {
            return Err(AppError::CustomError {
                msg: format!(
                    "The amount of selected players '{amount_selected_players}' is not the same as before '{amount_players_before}'."
                ),
            });
        }

        let mut selected_player_map = HashSet::new(); // used to validate dupplication

        // Validate that the salary cap limit is respeced.
        let mut total_salary_cap = 0.0;
        if let Some(team_salary_cap) = self.settings.salary_cap {
            for player_id in forw_list
                .iter()
                .chain(def_list.iter().chain(goal_list.iter()))
            {
                let player =
                    context
                        .players
                        .get(&player_id.to_string())
                        .ok_or(AppError::CustomError {
                            msg: "This player is not included in this pool".to_string(),
                        })?;

                let player_salary = player.salary_cap.ok_or(AppError::CustomError {
                    msg: format!(
                        "{} cannot be in alignment since he does not have contract.",
                        player.name
                    ),
                })?;

                total_salary_cap += player_salary;
                if total_salary_cap > team_salary_cap {
                    return Err(AppError::CustomError {
                        msg: format!(
                            "The selected players for the alignment are over the salary cap limit '{}$'.",
                            team_salary_cap
                        ),
                    });
                }
            }
        }

        // validate each selected players possession by the user asking the modification.
        // Also validate dupplication in the new list.
        for player_id in forw_list.iter().chain(
            def_list
                .iter()
                .chain(goal_list.iter())
                .chain(reserv_list.iter()),
        ) {
            let player =
                context
                    .players
                    .get(&player_id.to_string())
                    .ok_or(AppError::CustomError {
                        msg: "This player is not included in this pool".to_string(),
                    })?;
            if selected_player_map.contains(&player.id) {
                return Err(AppError::CustomError {
                    msg: format!("The player '{}' was dupplicated", player.name),
                });
            }
            selected_player_map.insert(player.id);

            if !roster.validate_player_possession(player.id) {
                return Err(AppError::CustomError {
                    msg: format!("You do not possess '{}'.", player.name),
                });
            }
        }

        // Finally update the roster of the player if everything went well.
        roster.chosen_forwards = forw_list.to_vec();
        roster.chosen_defenders = def_list.to_vec();
        roster.chosen_goalies = goal_list.to_vec();
        roster.chosen_reservists = reserv_list.to_vec();
        Ok(())
    }

    pub fn protect_players(
        &mut self,
        user_id: &str,
        protected_players_user_id: &str,
        protected_players: &[u32],
    ) -> Result<(), AppError> {
        // make sure the user making the resquest is a pool participants.
        self.validate_pool_status(&PoolState::Dynasty)?;
        self.validate_participant(protected_players_user_id)?;
        if user_id != protected_players_user_id {
            // If the user making the request is not the roster asking to be modified, the user need to have privilege.
            self.has_privileges(user_id)?;
        }

        let dynasty_settings =
            self.settings
                .dynasty_settings
                .as_ref()
                .ok_or_else(|| AppError::CustomError {
                    msg: "Dynasty settings does not exist.".to_string(),
                })?;

        if protected_players.len() != dynasty_settings.next_season_number_players_protected as usize
        {
            return Err(AppError::CustomError {
                msg: format!(
                    "The amount of players protected should be {}.",
                    dynasty_settings.next_season_number_players_protected
                ),
            });
        }

        // Validate that the players protection list does not contains dupplication and also validate that the user possess those players.
        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        let user_protected_players = context.protected_players.get_or_insert_with(HashMap::new);

        for player_id in protected_players.iter() {
            let player =
                context
                    .players
                    .get(&player_id.to_string())
                    .ok_or(AppError::CustomError {
                        msg: "This player is not included in this pool".to_string(),
                    })?;

            if !context.pooler_roster[protected_players_user_id]
                .validate_player_possession(player.id)
            {
                return Err(AppError::CustomError {
                    msg: format!("You do not possess '{}'.", player.name),
                });
            }

            user_protected_players.insert(
                protected_players_user_id.to_string(),
                protected_players.to_vec(),
            );
        }

        Ok(())
    }

    pub fn complete_protection(&mut self, user_id: &str) -> Result<(), AppError> {
        // Make sure the user making the request is the owner.
        self.validate_pool_status(&PoolState::Dynasty)?;
        self.has_owner_privileges(user_id)?;

        let dynasty_settings =
            self.settings
                .dynasty_settings
                .as_ref()
                .ok_or_else(|| AppError::CustomError {
                    msg: "Dynasty settings does not exist.".to_string(),
                })?;

        // Validate that the players' protection list does not contain duplications and that the user possesses those players.
        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        let protected_players_map =
            context
                .protected_players
                .clone()
                .ok_or_else(|| AppError::CustomError {
                    msg: "The protected players object does not exist.".to_string(),
                })?;

        if protected_players_map.len() != self.participants.len() {
            return Err(AppError::CustomError {
                msg: format!(
                    "{} out of {} poolers have protected their players.",
                    protected_players_map.len(),
                    self.participants.len()
                ),
            });
        }

        let mut all_added_player_ids = HashSet::new();

        for (pooler_user_id, protected_players) in protected_players_map {
            if protected_players.len()
                != dynasty_settings.next_season_number_players_protected as usize
            {
                return Err(AppError::CustomError {
                    msg: "The number of players protected is not valid.".to_string(),
                });
            }

            let pooler_roster =
                context
                    .pooler_roster
                    .get_mut(&pooler_user_id)
                    .ok_or_else(|| AppError::CustomError {
                        msg: "The user ID does not exist in the pool.".to_string(),
                    })?;

            // Clear the chosen rosters
            pooler_roster.chosen_forwards.clear();
            pooler_roster.chosen_defenders.clear();
            pooler_roster.chosen_goalies.clear();
            pooler_roster.chosen_reservists.clear();

            // The list of added players.
            let mut added_player_ids = HashSet::new();

            // Collect the players that should be added to the roster or reservists
            let mut players_to_add = Vec::new();

            for player_id in protected_players.iter() {
                added_player_ids.insert(player_id.to_string());

                let player = context.players.get(&player_id.to_string()).ok_or_else(|| {
                    AppError::CustomError {
                        msg: "The player ID is not included in the pool.".to_string(),
                    }
                })?;

                // Add the player to the roster or reservists
                players_to_add.push(player.clone());
            }
            // After iterating, perform the mutations
            for player in players_to_add {
                context.add_drafted_player(&player, &pooler_user_id, &self.settings)?;
            }

            // Add all refreshed player IDs to the global set
            all_added_player_ids.extend(added_player_ids);
        }

        // Remove all players that are no longer selected for the pool
        context
            .players
            .retain(|key, _| all_added_player_ids.contains(key));

        // At that point, the dynasty status is done, we can update to draft status.
        self.status = PoolState::Draft;

        Ok(())
    }

    pub fn mark_as_final(&mut self, user_id: &str) -> Result<(), AppError> {
        self.mark_as_final_at(user_id, Local::now().date_naive())
    }

    // Same as mark_as_final, with the current date injected so the
    // end-of-season rule can be exercised in tests.
    pub fn mark_as_final_at(&mut self, user_id: &str, today: NaiveDate) -> Result<(), AppError> {
        self.has_privileges(user_id)?;
        self.validate_pool_status(&PoolState::InProgress)?;

        let context = self.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        // Make sure the current date is after the end of the season.
        let end_season_date = NaiveDate::parse_from_str(&self.season_end, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        if today <= end_season_date {
            return Err(AppError::CustomError {
                msg: "The pool cannot be marked as final before the end of the season.".to_string(),
            });
        }

        // Get the final ranking of the pool. For dynasty pool, this will be use as draft order for the next season.
        self.final_rank = Some(context.get_final_rank(&self.settings)?);
        self.status = PoolState::Final;

        Ok(())
    }

    // Pure validators: they read the pool, so they borrow it rather than
    // consuming it (callers still need the pool afterwards, e.g. for its
    // version stamp).
    // Settings of a pool that has already been drafted. Dynasty is included:
    // between two seasons the pool is still live (trades, protections) and its
    // scoring, salary cap, assistants and roster modification dates are the
    // ones the participants keep tuning.
    pub fn can_update_started_pool_settings(
        &self,
        user_id: &str,
        settings: &PoolSettings,
    ) -> Result<(), AppError> {
        self.has_privileges(user_id)?;
        self.validate_pool_status_any(&[PoolState::InProgress, PoolState::Dynasty])?;

        // The roster shape and the dynasty rules are baked into the rosters
        // that are already drafted (and, in dynasty, into the protections being
        // made), so they stay frozen until the next pool is generated.
        if settings.number_forwards != self.settings.number_forwards
            || settings.number_defenders != self.settings.number_defenders
            || settings.number_goalies != self.settings.number_goalies
            || settings.number_reservists != self.settings.number_reservists
            || settings.dynasty_settings != self.settings.dynasty_settings
        {
            return Err(AppError::CustomError {
                msg: "These settings cannot be updated once the pool has started.".to_string(),
            }); // Need to make this robust, potentially need another pool status
        }

        Ok(())
    }

    /// Rename one of the poolers of the pool.
    ///
    /// Only the display name carried by the participant changes: everything
    /// else in the pool (rosters, trades, draft order, scores) is keyed by the
    /// pooler's id, which stays untouched. This is what lets the owner replace
    /// the email address a pooler registered with by the name everybody in the
    /// pool actually calls them.
    ///
    /// Renaming is the owner's alone, and only once the pool has participants:
    /// before the draft they still live in the draft room, not in the pool.
    pub fn update_pooler_name(
        &mut self,
        user_id: &str,
        pooler_user_id: &str,
        new_name: &str,
    ) -> Result<(), AppError> {
        self.has_owner_privileges(user_id)?;

        let new_name = new_name.trim();

        if new_name.is_empty() {
            return Err(AppError::CustomError {
                msg: "A pooler name cannot be empty.".to_string(),
            });
        }

        if new_name.chars().count() > MAX_POOLER_NAME_LENGTH {
            return Err(AppError::CustomError {
                msg: format!(
                    "A pooler name cannot be longer than {MAX_POOLER_NAME_LENGTH} characters."
                ),
            });
        }

        // The front end tells poolers apart by name in its rankings and in the
        // shareable `selectedParticipant` link, so two poolers sharing one name
        // would make the pool ambiguous to read.
        if self
            .participants
            .iter()
            .any(|user| user.id != pooler_user_id && user.name == new_name)
        {
            return Err(AppError::CustomError {
                msg: format!("Another pooler of this pool is already named '{new_name}'."),
            });
        }

        let participant = self
            .participants
            .iter_mut()
            .find(|user| user.id == pooler_user_id)
            .ok_or_else(|| AppError::CustomError {
                msg: format!("User {pooler_user_id} is not a pool participants."),
            })?;

        participant.name = new_name.to_string();

        Ok(())
    }

    pub fn can_update_pool_settings(&self, user_id: &str) -> Result<(), AppError> {
        self.has_privileges(user_id)?;
        self.validate_pool_status(&PoolState::Created)?;

        Ok(())
    }

    pub fn start_draft(
        &mut self,
        user_id: &str,
        room_users: &[RoomUser],
        draft_order: &[String],
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::Created)?;
        self.has_owner_privileges(user_id)?;

        // Shuffle the pool participants. so the draft order is
        let room_users = room_users.to_vec();

        let user_ids: Vec<String> = room_users.iter().map(|user| user.id.clone()).collect();

        if user_ids.len() as u8 != self.settings.number_poolers {
            return Err(AppError::CustomError {
                msg: "The number of pooler should match the expected in the settings.".to_string(),
            });
        }

        // Set the draft order with the shuffle list.
        if !draft_order.iter().all(|user_id| user_ids.contains(user_id)) {
            return Err(AppError::CustomError {
                msg: "The draft order list provided is not valid.".to_string(),
            });
        }

        self.status = PoolState::Draft;
        self.context = Some(PoolContext::new(&user_ids));
        self.settings.number_poolers = user_ids.len() as u8;
        self.participants = room_users.into_iter().map(PoolUser::from).collect();
        self.draft_order = Some(draft_order.to_vec());

        Ok(())
    }

    pub fn draft_player(
        &mut self,
        user_id: &str,
        player: &PlayerInfo,
    ) -> Result<DraftOutcome, AppError> {
        // Match against

        // Only a running draft accepts picks. Without this, a pick arriving
        // after the last one (a late socket message, a double click) would find
        // every roster full and fall through to the reservists, which are
        // appended to without a capacity check.
        self.validate_pool_status(&PoolState::Draft)?;

        let has_privileges = self.has_owner_rights(user_id);

        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let draft_order = self
            .draft_order
            .as_ref()
            .ok_or_else(|| AppError::CustomError {
                msg: "draft order does not exist.".to_string(),
            })?;

        let outcome = if self.settings.dynasty_settings.is_some()
            && context.past_tradable_picks.is_some()
        {
            // This is a dynasty draft context.
            // The final rank is being used as draft order.
            context.draft_player_dynasty(
                user_id,
                player,
                draft_order,
                &self.settings,
                has_privileges,
            )?
        } else {
            // This is a dynasty draft context.
            // The participant order is being used as draft order.
            context.draft_player(user_id, player, draft_order, &self.settings, has_privileges)?
        };

        if outcome.is_done {
            // The draft is done.
            self.status = PoolState::InProgress;
        }

        Ok(outcome)
    }

    // The date a lineup change made on `today` takes effect.
    //
    // A change made before the season starts has no day of its own to apply to:
    // no game has been played, so it does not switch anything mid-season, it
    // redefines the lineup the pool opens with. Landing it on the season start
    // makes it replace the opening event already recorded there instead of
    // stacking events on days that will never be scored — and, more to the
    // point, an event dated before the season start is never the latest one on
    // or before the opening day, so the change would simply not be applied.
    pub fn lineup_effective_date(&self, today: &str) -> String {
        // Dates are ISO (yyyy-MM-dd), so they order lexicographically.
        if today < self.season_start.as_str() {
            return self.season_start.clone();
        }
        today.to_string()
    }

    pub fn undo_draft_player(&mut self, user_id: &str) -> Result<UndoOutcome, AppError> {
        // Undo the last draft selection.
        // This call can only be made if the user id is the owner.
        self.has_owner_privileges(user_id)?;
        self.validate_pool_status(&PoolState::Draft)?;

        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let draft_order = self
            .draft_order
            .as_ref()
            .ok_or_else(|| AppError::CustomError {
                msg: "draft order does not exist.".to_string(),
            })?;

        context.undo_draft_player(draft_order, &self.settings)
    }

    pub fn validate_participant(&self, user_id: &str) -> Result<(), AppError> {
        // Validate that the user is a pool participant.
        if !self.participants.iter().any(|user| user.id == user_id) {
            return Err(AppError::CustomError {
                msg: format!("User {} is not a pool participants.", user_id),
            });
        }

        Ok(())
    }

    pub fn validate_pool_status(&self, expected_status: &PoolState) -> Result<(), AppError> {
        // Validate that the pool is in the expected status.
        if &self.status != expected_status {
            return Err(AppError::CustomError {
                msg: format!(
                    "The expected pool status '{}', current pool status '{}'.",
                    expected_status, self.status
                ),
            });
        }
        Ok(())
    }

    pub fn validate_pool_status_any(&self, expected_status: &[PoolState]) -> Result<(), AppError> {
        // Validate that the pool is in one of the expected statuses.
        if !expected_status.contains(&self.status) {
            return Err(AppError::CustomError {
                msg: format!(
                    "The expected pool status '{}', current pool status '{}'.",
                    expected_status
                        .iter()
                        .map(|status| status.to_string())
                        .collect::<Vec<_>>()
                        .join("' or '"),
                    self.status
                ),
            });
        }
        Ok(())
    }

    pub fn has_assistants_rights(&self, user_id: &str) -> bool {
        self.settings.assistants.contains(&user_id.to_string())
    }

    pub fn has_owner_rights(&self, user_id: &str) -> bool {
        self.owner == user_id
    }

    pub fn has_privileges(&self, user_id: &str) -> Result<(), AppError> {
        if !self.has_assistants_rights(user_id) && !self.has_owner_rights(user_id) {
            return Err(AppError::CustomError {
                msg: "This action require privileged rights.".to_string(),
            });
        }

        Ok(())
    }

    pub fn has_owner_privileges(&self, user_id: &str) -> Result<(), AppError> {
        if !self.has_owner_rights(user_id) {
            return Err(AppError::CustomError {
                msg: "This action require owner rights.".to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum PoolState {
    Final,
    InProgress,
    Dynasty,
    Draft,
    Created,
}

impl fmt::Display for PoolState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // To be able to print out the PoolState enum.
        match self {
            PoolState::Final => write!(f, "Final"),
            PoolState::InProgress => write!(f, "In progress"),
            PoolState::Dynasty => write!(f, "Dynasty"),
            PoolState::Draft => write!(f, "Draft"),
            PoolState::Created => write!(f, "Created"),
        }
    }
}

// What a single draft pick changed. Returned by the draft calls so the caller
// can broadcast the delta instead of the whole pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftOutcome {
    // The participant the player was drafted for. Not necessarily the caller:
    // the pool owner can draft on behalf of whoever's turn it is.
    pub drafter: String,
    // Exactly what got pushed onto `players_name_drafted`: the player id,
    // followed by a 0 for each drafter skipped because its roster is full.
    pub appended_picks: Vec<u32>,
    pub is_done: bool,
}

// What an undo reverted, so the caller can broadcast the delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoOutcome {
    pub drafter: String,
    pub player_id: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)] // Copy
pub struct PoolContext {
    pub pooler_roster: HashMap<String, PoolerRoster>,
    pub players_name_drafted: Vec<u32>,
    pub score_by_day: Option<HashMap<String, HashMap<String, DailyRosterPoints>>>,
    pub tradable_picks: Option<Vec<HashMap<String, String>>>,
    pub past_tradable_picks: Option<Vec<HashMap<String, String>>>,
    pub protected_players: Option<HashMap<String, Vec<u32>>>,
    pub players: HashMap<String, PlayerInfo>,
    // Sparse lineup events: one entry per starting-lineup change per
    // participant. The lineup on any date is the latest event on or before it;
    // daily points are derived from the shared day_leaders. Replaces the per-day
    // roster snapshots that lived in `score_by_day`.
    #[serde(default)]
    pub lineup_events: Option<Vec<LineupEvent>>,
}

impl PoolContext {
    pub fn new(participants: &[String]) -> Self {
        let mut pooler_roster = HashMap::new();

        // Initialize all participants roster object.
        for participant in participants.iter() {
            pooler_roster.insert(participant.to_string(), PoolerRoster::new());
        }

        Self {
            pooler_roster,
            score_by_day: Some(HashMap::new()),
            tradable_picks: Some(Vec::new()),
            past_tradable_picks: Some(Vec::new()),
            players_name_drafted: Vec::new(),
            protected_players: None,
            players: HashMap::new(),
            lineup_events: Some(Vec::new()),
        }
    }

    /// Append a lineup event for `participant` effective `date` if their current
    /// starting roster (chosen forwards/defense/goalies) differs from their
    /// latest recorded lineup. Keeps `lineup_events` sparse; a same-day re-edit
    /// replaces that day's entry. Returns whether an event was appended.
    pub fn record_lineup_change(&mut self, participant: &str, date: &str) -> bool {
        let Some(roster) = self.pooler_roster.get(participant) else {
            return false;
        };
        let mut forwards = roster.chosen_forwards.clone();
        let mut defense = roster.chosen_defenders.clone();
        let mut goalies = roster.chosen_goalies.clone();
        forwards.sort_unstable();
        defense.sort_unstable();
        goalies.sort_unstable();

        let events = self.lineup_events.get_or_insert_with(Vec::new);

        let unchanged = events
            .iter()
            .filter(|event| event.participant == participant)
            .max_by(|a, b| a.effective_date.cmp(&b.effective_date))
            .is_some_and(|latest| {
                let sorted = |mut ids: Vec<u32>| {
                    ids.sort_unstable();
                    ids
                };
                sorted(latest.forwards.clone()) == forwards
                    && sorted(latest.defense.clone()) == defense
                    && sorted(latest.goalies.clone()) == goalies
            });
        if unchanged {
            return false;
        }

        events.retain(|event| !(event.participant == participant && event.effective_date == date));
        events.push(LineupEvent {
            participant: participant.to_string(),
            effective_date: date.to_string(),
            forwards,
            defense,
            goalies,
        });
        true
    }

    pub fn get_final_rank(&self, pool_settings: &PoolSettings) -> Result<Vec<String>, AppError> {
        let Some(score_by_day) = &self.score_by_day else {
            return Err(AppError::CustomError {
                msg: "No score is being recorded in this pool yet.".to_string(),
            });
        };

        // Per-user season tally: (total points, total number of games, then for
        // each player type a map of player id -> (total points, total games)).
        type UserSeasonTally = (
            u16,
            u16,
            HashMap<String, (u16, u16)>, // Forwards
            HashMap<String, (u16, u16)>, // Defense
            HashMap<String, (u16, u16)>, // Goalies
        );

        // Map the user to its total points, total number of games
        // and for each player type, a hashmap of the player id with their corresponding total number of points, total number of games.
        let mut user_total_points: HashMap<String, UserSeasonTally> = HashMap::new();

        for (date, daily_roster_points) in score_by_day {
            for (participant, roster_daily_points) in daily_roster_points {
                // Initialize the participant with 0 points and 0 games and no players.
                if !user_total_points.contains_key(participant) {
                    user_total_points.insert(
                        participant.clone(),
                        (0, 0, HashMap::new(), HashMap::new(), HashMap::new()),
                    );
                }

                // Ranking a pool on data that is still being written would give
                // a wrong final order, so a day that has not been cumulated
                // blocks the ranking.
                //
                // A day with no scoring line at all is exempt: it contributes
                // zero to every tally whatever happens to it later. That covers
                // the league's off days (all-star and olympic breaks, playoff
                // gaps), which the ingest leaves uncumulated because there were
                // simply no games to cumulate — those must not make a pool
                // impossible to finalize.
                if !roster_daily_points.is_cumulated && !roster_daily_points.is_scoreless() {
                    return Err(AppError::CustomError {
                        msg: format!(
                            "There are no cumulative data on the {date} for the user {participant}"
                        ),
                    });
                }

                if let Some((
                    total_points,
                    number_of_games,
                    forwards_points,
                    defenders_points,
                    goalies_points,
                )) = user_total_points.get_mut(participant)
                {
                    let (daily_points, daily_games) = roster_daily_points.get_total_points(
                        pool_settings,
                        forwards_points,
                        defenders_points,
                        goalies_points,
                    );

                    *total_points += daily_points;
                    *number_of_games += daily_games;
                }
            }
        }

        // Convert the HashMap into a Vec of tuples
        if let Some(ignore_x_worst_players) = &pool_settings.ignore_x_worst_players {
            for (
                total_points,
                total_number_of_games,
                forwards_points,
                defenders_points,
                goalies_points,
            ) in user_total_points.values_mut()
            {
                // Find the x worst forwards that points should be ignored.
                let mut forwards_vec: Vec<(&String, &(u16, u16))> =
                    forwards_points.iter().collect();

                // Sort the vector by total points in ascending order
                forwards_vec.sort_by(|a, b| a.1.0.cmp(&b.1.0).then_with(|| a.1.1.cmp(&b.1.1)));

                // Take the first x elements
                let least_points_players = forwards_vec
                    .iter()
                    .take(ignore_x_worst_players.forwards as usize);

                // Print the players with the least total points
                for (_, (points, number_of_games)) in least_points_players {
                    *total_points -= points;
                    *total_number_of_games -= number_of_games;
                }

                // Find the x worst defenders that points should be ignored.
                let mut defenders_vec: Vec<(&String, &(u16, u16))> =
                    defenders_points.iter().collect();

                // Sort the vector by total points in ascending order
                defenders_vec.sort_by(|a, b| a.1.0.cmp(&b.1.0).then_with(|| a.1.1.cmp(&b.1.1)));

                // Take the first x elements
                let least_points_players = defenders_vec
                    .iter()
                    .take(ignore_x_worst_players.defense as usize);

                // Print the players with the least total points
                for (_, (points, number_of_games)) in least_points_players {
                    *total_points -= points;
                    *total_number_of_games -= number_of_games;
                }

                // Find the x worst goalies that points should be ignored.
                let mut goalies_vec: Vec<(&String, &(u16, u16))> = goalies_points.iter().collect();

                // Sort the vector by total points in ascending order
                goalies_vec.sort_by(|a, b| a.1.0.cmp(&b.1.0).then_with(|| a.1.1.cmp(&b.1.1)));

                // Take the first x elements
                let least_points_players = goalies_vec
                    .iter()
                    .take(ignore_x_worst_players.goalies as usize);

                // Print the players with the least total points
                for (_, (points, number_of_games)) in least_points_players {
                    *total_points -= points;
                    *total_number_of_games -= number_of_games;
                }
            }
        }

        let mut user_points_vec: Vec<(&String, &UserSeasonTally)> =
            user_total_points.iter().collect();

        // Sort the total points vector. And fill the final_rank list with it.
        // Sort the vector by total points and then by total games in descending order
        user_points_vec.sort_by(|a, b| {
            b.1.0
                .cmp(&a.1.0) // Compare total points
                .then_with(|| a.1.1.cmp(&b.1.1)) // If points are equal, compare total games (The pooler with less games wins)
        });

        let mut final_rank = Vec::new();
        for participant in user_points_vec {
            final_rank.push(participant.0.clone())
        }

        Ok(final_rank)
    }

    pub fn calculate_cumulated_salary_cap(
        &self,
        pooler_roster: &PoolerRoster,
        players: &HashMap<String, PlayerInfo>,
    ) -> Result<f64, AppError> {
        pooler_roster
            .chosen_forwards
            .iter()
            .chain(pooler_roster.chosen_defenders.iter()) // Chain defenders
            .chain(pooler_roster.chosen_goalies.iter())
            .map(|player_id| {
                players
                    .get(&player_id.to_string())
                    .ok_or_else(|| AppError::CustomError {
                        msg: "Player does not exist.".to_string(),
                    })
                    .and_then(|player| {
                        player.salary_cap.ok_or_else(|| AppError::CustomError {
                            msg: "Player salary cap not available.".to_string(),
                        })
                    })
            })
            .try_fold(0.0, |acc, salary_cap| salary_cap.map(|sc| acc + sc))
    }

    pub fn can_add_player_to_roster(
        &self,
        player: &PlayerInfo,
        pool_user_id: &str,
        settings: &PoolSettings,
    ) -> Result<bool, AppError> {
        // If there is salary cap management, don't add to the starting roster players without contracts or if the user doesn't have enough space.
        if let Some(team_salary_cap) = settings.salary_cap {
            let pooler_roster =
                self.pooler_roster
                    .get(pool_user_id)
                    .ok_or_else(|| AppError::CustomError {
                        msg: "Pooler roster does not exist.".to_string(),
                    })?;

            let cumulated_salary_cap =
                self.calculate_cumulated_salary_cap(pooler_roster, &self.players)?;

            if let Some(player_salary_cap) = player.salary_cap {
                if cumulated_salary_cap + player_salary_cap <= team_salary_cap {
                    return Ok(true);
                }
                return Ok(false);
            }
            return Ok(false);
        }
        Ok(true)
    }

    pub fn add_drafted_player(
        &mut self,
        player: &PlayerInfo,
        next_drafter: &str,
        settings: &PoolSettings,
    ) -> Result<(), AppError> {
        // Then, Add the chosen player in its right spot.
        // When there is no place in the position of the player we will add it to the reservists.

        let can_add_player_to_roster =
            self.can_add_player_to_roster(player, next_drafter, settings)?;

        if let Some(pooler_roster) = self.pooler_roster.get_mut(next_drafter) {
            let mut is_added = false;
            if can_add_player_to_roster {
                match player.position {
                    Position::F => {
                        if (pooler_roster.chosen_forwards.len() as u8) < settings.number_forwards {
                            pooler_roster.chosen_forwards.push(player.id);
                            is_added = true;
                        }
                    }
                    Position::D => {
                        if (pooler_roster.chosen_defenders.len() as u8) < settings.number_defenders
                        {
                            pooler_roster.chosen_defenders.push(player.id);
                            is_added = true;
                        }
                    }
                    Position::G => {
                        if (pooler_roster.chosen_goalies.len() as u8) < settings.number_goalies {
                            pooler_roster.chosen_goalies.push(player.id);
                            is_added = true;
                        }
                    }
                }
            }

            // If the there is not enough place in the roster, try to add the player in the reservists.
            if !is_added {
                // Return an error when the player could not be added.
                if settings.number_reservists == 0 {
                    return Err(AppError::CustomError {
                        msg: format!("There is no space for {} in the roster.", player.name),
                    });
                }
                pooler_roster.chosen_reservists.push(player.id);
            }
        }
        Ok(())
    }

    pub fn is_draft_done(&mut self, settings: &PoolSettings) -> Result<bool, AppError> {
        // the status change to InProgress when the draft is completed.
        // The draft is completed when all participants has a complete roster.

        let mut is_done = true;

        for participant in self.pooler_roster.keys() {
            if self.get_roster_count(participant)?
                < (settings.number_forwards
                    + settings.number_defenders
                    + settings.number_goalies
                    + settings.number_reservists) as usize
            {
                is_done = false;
                break; // The Draft phase is not done.
            }
        }

        // generate the list of tradable_picks for the next season

        if is_done {
            // If done, clone the tradable picks, into the past_tradable_picks and reset the tradable picks.
            let mut new_tradable_picks = vec![];

            if let Some(dynasty_settings) = &settings.dynasty_settings {
                for _ in 0..dynasty_settings.tradable_picks {
                    let mut round = HashMap::new();

                    for participant in self.pooler_roster.keys() {
                        round.insert(participant.clone(), participant.clone());
                    }

                    new_tradable_picks.push(round);
                }
            }

            self.tradable_picks = Some(new_tradable_picks);
        }
        Ok(is_done)
    }

    pub fn draft_player_dynasty(
        &mut self,
        user_id: &str,
        player: &PlayerInfo,
        draft_order: &[String], // being used as draft order.
        settings: &PoolSettings,
        has_privileges: bool,
    ) -> Result<DraftOutcome, AppError> {
        // First, validate that the player selected is not already picked by any of the other poolers.

        for (id, roster) in &self.pooler_roster {
            if roster.validate_player_possession(player.id) {
                return Err(AppError::CustomError {
                    msg: format!("{} is already picked by {}.", player.name, id),
                });
            }
        }
        // Find the next draft id for dynasty type pool.
        let next_drafter = self.find_dynasty_next_drafter(draft_order)?;

        if !has_privileges && next_drafter != user_id {
            return Err(AppError::CustomError {
                msg: format!("It is {}'s turn.", next_drafter),
            });
        }

        // Add the drafted player if everything goes right.
        self.add_drafted_player(player, &next_drafter, settings)?;

        self.players.insert(player.id.to_string(), player.clone());
        self.players_name_drafted.push(player.id);
        let mut appended_picks = vec![player.id];

        // Get the maximum number of player a user can draft.
        let mut continue_count = 0;
        let max_player_count = settings.number_forwards
            + settings.number_defenders
            + settings.number_goalies
            + settings.number_reservists;

        // Fill the drafters that have completed with 0.
        loop {
            let new_next_drafter = self.find_dynasty_next_drafter(draft_order)?;
            if self.get_roster_count(&new_next_drafter)? >= max_player_count as usize {
                if self.is_draft_done(settings)? {
                    return Ok(DraftOutcome {
                        drafter: next_drafter,
                        appended_picks,
                        is_done: true,
                    });
                }
                self.players_name_drafted.push(0); // Id 0 means the players did not draft because his roster is already full
                appended_picks.push(0);

                continue_count += 1;

                if continue_count >= draft_order.len() {
                    break;
                }
                continue;
            }
            break;
        }

        Ok(DraftOutcome {
            drafter: next_drafter,
            appended_picks,
            is_done: self.is_draft_done(settings)?,
        })
    }

    pub fn find_dynasty_next_drafter(
        &mut self,
        draft_order: &[String], // being used as draft order.
    ) -> Result<String, AppError> {
        // Draft the right player in dynasty mode.
        // This takes into account the trade that have been traded during last season (past_tradable_picks).

        let past_tradable_picks =
            self.past_tradable_picks
                .as_ref()
                .ok_or_else(|| AppError::CustomError {
                    msg: "Pool context does not exist.".to_string(),
                })?;

        // To make sure the program never go into an infinite loop. we use a counter.
        let mut next_drafter;
        let nb_players_drafted = self.players_name_drafted.len();

        let index_draft = nb_players_drafted % draft_order.len();
        // Fetch the next drafter without considering if the trade has been traded yet.
        next_drafter = &draft_order[index_draft];

        if nb_players_drafted < (past_tradable_picks.len() * draft_order.len()) {
            // use the tradable_picks to see if the pick got traded so it is to the person owning the pick to draft.

            next_drafter =
                &past_tradable_picks[nb_players_drafted / draft_order.len()][next_drafter];
        }

        Ok(next_drafter.clone())
    }

    pub fn draft_player(
        &mut self,
        user_id: &str,
        player: &PlayerInfo,
        draft_order: &[String], // being used as draft order.
        settings: &PoolSettings,
        has_privileges: bool,
    ) -> Result<DraftOutcome, AppError> {
        // Draft the right player in normal mode.
        // Taking only into account the draft order

        for roster in self.pooler_roster.values() {
            if roster.validate_player_possession(player.id) {
                return Err(AppError::CustomError {
                    msg: "This player is already picked.".to_string(),
                });
            }
        }

        // there is no final rank so this is the newly created draft logic.

        let players_drafted = self.players_name_drafted.len();

        // Snake draft, reverse draft order each round.
        let round = players_drafted / draft_order.len();

        let index = if round % 2 == 1 {
            draft_order.len() - 1 - (players_drafted % draft_order.len())
        } else {
            players_drafted % draft_order.len()
        };

        let next_drafter = &draft_order[index];

        if !has_privileges && next_drafter != user_id {
            return Err(AppError::CustomError {
                msg: format!("It is {}'s turn.", next_drafter),
            });
        }

        // Add the drafted player if everything goes right.
        self.add_drafted_player(player, next_drafter, settings)?;
        let drafter = next_drafter.clone();

        self.players.insert(player.id.to_string(), player.clone());
        self.players_name_drafted.push(player.id);
        Ok(DraftOutcome {
            drafter,
            appended_picks: vec![player.id],
            is_done: self.is_draft_done(settings)?,
        })
    }

    pub fn undo_draft_player(
        &mut self,
        participants: &[String],
        settings: &PoolSettings,
    ) -> Result<UndoOutcome, AppError> {
        // validate there is something to undo.

        let latest_pick_id;

        loop {
            match self.players_name_drafted.pop() {
                Some(player_id) => {
                    if player_id > 0 {
                        latest_pick_id = player_id; // found the last drafted player.
                        break;
                    }
                }
                None => {
                    return Err(AppError::CustomError {
                        msg: "Ther is nothing to undo yet.".to_string(),
                    });
                }
            }
        }

        let pick_number = self.players_name_drafted.len();
        let latest_drafter;

        match (&settings.dynasty_settings, &self.past_tradable_picks) {
            (Some(dynasty_settings), Some(past_tradable_picks)) => {
                // This comes from a Dynasty draft.

                let nb_tradable_picks = dynasty_settings.tradable_picks;

                let index = pick_number % participants.len();

                let next_drafter = &participants[index];

                if pick_number < nb_tradable_picks as usize * participants.len() {
                    // use the tradable_picks to see who will draft next.
                    latest_drafter =
                        past_tradable_picks[pick_number / participants.len()][next_drafter].clone();
                } else {
                    // Use the draft order to see who draft next.
                    latest_drafter = next_drafter.clone();
                }
            }
            _ => {
                // this comes from a newly created draft.

                let round = pick_number / participants.len();

                // Snake draft, reverse draft order each round.
                let index = if round % 2 == 1 {
                    participants.len() - 1 - (pick_number % participants.len()) // reversed
                } else {
                    pick_number % participants.len() // Original
                };

                latest_drafter = participants[index].clone();
            }
        }

        // Remove the player from the player roster.

        self.remove_player_from_roster(latest_pick_id, &latest_drafter)?;
        self.players.remove(&latest_pick_id.to_string()); // Also remove the player from the pool players list.
        Ok(UndoOutcome {
            drafter: latest_drafter,
            player_id: latest_pick_id,
        })
    }

    pub fn remove_player_from_roster(
        &mut self,
        player_id: u32,
        user_id: &str,
    ) -> Result<(), AppError> {
        // Remove a player from the roster.
        if let Some(roster) = self.pooler_roster.get_mut(user_id) {
            if roster.remove_forward(player_id) {
                return Ok(());
            };
            if roster.remove_defender(player_id) {
                return Ok(());
            };
            if roster.remove_goalie(player_id) {
                return Ok(());
            };
            if roster.remove_reservist(player_id) {
                return Ok(());
            };
        }

        Err(AppError::CustomError {
            msg: "The player could not be removed".to_string(),
        }) // could not be removed
    }

    pub fn add_player_to_reservists(
        &mut self,
        player_id: u32,
        user_id: &str,
    ) -> Result<(), AppError> {
        // Add a player to the reservist of a pooler.
        if let Some(roster) = self.pooler_roster.get_mut(user_id) {
            roster.chosen_reservists.push(player_id);
            return Ok(());
        }

        Err(AppError::CustomError {
            msg: "The player could not be added".to_string(),
        }) // could not be added
    }

    pub fn trade_roster_player(
        &mut self,
        player_id: u32,
        user_giver: &str,
        user_receiver: &str,
    ) -> Result<(), AppError> {
        // Trade 1 player.
        self.remove_player_from_roster(player_id, user_giver)?;
        self.add_player_to_reservists(player_id, user_receiver)
    }

    pub fn trade_roster_items(&mut self, trade: &Trade) -> Result<(), AppError> {
        // Make sure the trade is valid before executing it.
        self.validate_trade(trade)?;

        // Migrate players "from" -> "to"
        for player_id in trade.from_items.players.iter() {
            self.trade_roster_player(*player_id, &trade.proposed_by, &trade.ask_to)?;
        }

        // Migrate players "to" -> "from"
        for player_id in trade.to_items.players.iter() {
            self.trade_roster_player(*player_id, &trade.ask_to, &trade.proposed_by)?;
        }

        // Migrate picks "from" -> "to"
        for pick in trade.from_items.picks.iter() {
            if let Some(tradable_picks) = &mut self.tradable_picks
                && let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from)
            {
                *owner = trade.ask_to.clone();
            }
        }

        // Migrate picks "to" -> "from"
        for pick in trade.to_items.picks.iter() {
            if let Some(tradable_picks) = &mut self.tradable_picks
                && let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from)
            {
                *owner = trade.proposed_by.clone();
            }
        }

        Ok(())
    }

    pub fn validate_trade_items(
        &self,
        trade_items: &TradeItems,
        user_id: &str,
    ) -> Result<(), AppError> {
        // Validate that the trade items are valid for a trade side.
        if let Some(from_pooler_roster) = self.pooler_roster.get(user_id) {
            for player_id in &trade_items.players {
                if !from_pooler_roster.validate_player_possession(*player_id) {
                    return Err(AppError::CustomError {
                        msg: "ther user does not possess one of the traded player!".to_string(),
                    });
                }
            }

            if let Some(tradable_picks) = &self.tradable_picks {
                for pick in &trade_items.picks {
                    if tradable_picks[pick.round as usize][&pick.from] != user_id {
                        return Err(AppError::CustomError {
                            msg: "ther user does not possess the traded pick!".to_string(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    pub fn validate_trade(&self, trade: &Trade) -> Result<(), AppError> {
        // Validate if the full trade is valid

        // does the the from or to side has items in the trade ?

        if (trade.from_items.picks.len() + trade.from_items.players.len()) == 0
            || (trade.to_items.picks.len() + trade.to_items.players.len()) == 0
        {
            return Err(AppError::CustomError {
                msg: "There is no items traded on one of the 2 sides.".to_string(),
            });
        }

        // Maximum of 5 items traded on each side ?

        if (trade.from_items.picks.len() + trade.from_items.players.len()) > 5
            || (trade.to_items.picks.len() + trade.to_items.players.len()) > 5
        {
            return Err(AppError::CustomError {
                msg: "There is to much items in the trade.".to_string(),
            });
        }

        self.validate_trade_items(&trade.from_items, &trade.proposed_by)?;
        self.validate_trade_items(&trade.to_items, &trade.ask_to)
    }

    pub fn get_forwards_count(&self, user_id: &str) -> Result<usize, AppError> {
        // Get the count of forward for a pooler.
        match self.pooler_roster.get(user_id) {
            None => Err(AppError::CustomError {
                msg: "The user does not exist.".to_string(),
            }),
            Some(roster) => Ok(roster.chosen_forwards.len()),
        }
    }

    pub fn get_defenders_count(&self, user_id: &str) -> Result<usize, AppError> {
        // Get the count of defender for a pooler.
        match self.pooler_roster.get(user_id) {
            None => Err(AppError::CustomError {
                msg: "The user does not exist.".to_string(),
            }),
            Some(roster) => Ok(roster.chosen_defenders.len()),
        }
    }

    pub fn get_goalies_count(&self, user_id: &str) -> Result<usize, AppError> {
        // Get the count of goalies for a pooler.
        match self.pooler_roster.get(user_id) {
            None => Err(AppError::CustomError {
                msg: "The user does not exist.".to_string(),
            }),
            Some(roster) => Ok(roster.chosen_goalies.len()),
        }
    }

    pub fn get_reservists_count(&self, user_id: &str) -> Result<usize, AppError> {
        // Get the count of reservist for a pooler.
        match self.pooler_roster.get(user_id) {
            None => Err(AppError::CustomError {
                msg: "The user does not exist.".to_string(),
            }),
            Some(roster) => Ok(roster.chosen_reservists.len()),
        }
    }

    pub fn get_roster_count(&self, user_id: &str) -> Result<usize, AppError> {
        // Get the count of the full roster for a pooler.
        Ok(self.get_forwards_count(user_id)?
            + self.get_defenders_count(user_id)?
            + self.get_goalies_count(user_id)?
            + self.get_reservists_count(user_id)?)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)] // Copy
pub struct PoolerRoster {
    pub chosen_forwards: Vec<u32>,
    pub chosen_defenders: Vec<u32>,
    pub chosen_goalies: Vec<u32>,
    pub chosen_reservists: Vec<u32>,
}
impl Default for PoolerRoster {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolerRoster {
    pub fn new() -> Self {
        Self {
            chosen_forwards: Vec::new(),
            chosen_defenders: Vec::new(),
            chosen_goalies: Vec::new(),
            chosen_reservists: Vec::new(),
        }
    }

    pub fn remove_forward(&mut self, player_id: u32) -> bool {
        // Remove a forward from a pooler roster
        self.chosen_forwards
            .iter()
            .position(|id| id == &player_id)
            .map(|index| self.chosen_forwards.remove(index))
            .is_some()
    }

    pub fn remove_defender(&mut self, player_id: u32) -> bool {
        // Remove a defenders from a pooler roster
        self.chosen_defenders
            .iter()
            .position(|id| id == &player_id)
            .map(|index| self.chosen_defenders.remove(index))
            .is_some()
    }

    pub fn remove_goalie(&mut self, player_id: u32) -> bool {
        // Remove a goalies from a pooler roster
        self.chosen_goalies
            .iter()
            .position(|id| id == &player_id)
            .map(|index| self.chosen_goalies.remove(index))
            .is_some()
    }

    pub fn remove_reservist(&mut self, player_id: u32) -> bool {
        // Remove a reservist from a pooler roster
        self.chosen_reservists
            .iter()
            .position(|id| id == &player_id)
            .map(|index| self.chosen_reservists.remove(index))
            .is_some()
    }

    pub fn validate_player_possession(&self, player_id: u32) -> bool {
        self.chosen_forwards.contains(&player_id)
            || self.chosen_defenders.contains(&player_id)
            || self.chosen_goalies.contains(&player_id)
            || self.chosen_reservists.contains(&player_id)
    }
}

impl PartialEq<PlayerInfo> for PlayerInfo {
    fn eq(&self, other: &PlayerInfo) -> bool {
        self.id == other.id
    }
}

impl PartialEq<Pick> for Pick {
    fn eq(&self, other: &Pick) -> bool {
        self.round == other.round && self.from == other.from
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pick {
    pub round: u8,
    pub from: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Trade {
    pub proposed_by: String,
    pub ask_to: String,
    pub from_items: TradeItems,
    pub to_items: TradeItems,
    pub status: TradeStatus,
    pub id: u32,
    pub date_created: i64,
    pub date_accepted: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TradeItems {
    pub players: Vec<u32>, // Id of the player
    pub picks: Vec<Pick>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum TradeStatus {
    NEW,       // trade created by a requester (not yet ACCEPTED/CANCELLED/REFUSED)
    ACCEPTED,  // trade accepted items were officially traded
    CANCELLED, // items were not traded cancelled by the requester
    REFUSED,   // items were not traded cancelled by the one requested for the traded
}

#[cfg(test)]
mod tests;
