use crate::{draft::model::RoomUser, errors::AppError};
use chrono::{Duration, Local, NaiveDate, Timelike, Utc};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
// Date for season
//

pub const START_SEASON_DATE: &str = "2024-10-4";
pub const END_SEASON_DATE: &str = "2024-04-18";
pub const POOL_CREATION_SEASON: u32 = 20242025;

pub const TRADE_DEADLINE_DATE: &str = "2024-03-08";

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
        self.validate_pool_status(&PoolState::InProgress)?;
        // Create a trade in the pool if it is valid to do so..
        let trade_deadline_date = NaiveDate::parse_from_str(TRADE_DEADLINE_DATE, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        let today = Local::now().date_naive();

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
            for trade in trades.iter() {
                if (matches!(trade.status, TradeStatus::NEW))
                    && (trade.proposed_by == trade.proposed_by)
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

        if !priviledge_right && trades[trade_index].date_created + 8640000 > now {
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
                {
                    if let Some(x) = context.pooler_roster.get_mut(filled_spot_user_id) {
                        x.chosen_forwards.push(player.id);
                        is_added = true;
                    }
                }
            }
            Position::D => {
                if (context.pooler_roster[filled_spot_user_id]
                    .chosen_defenders
                    .len() as u8)
                    < self.settings.number_defenders
                {
                    if let Some(x) = context.pooler_roster.get_mut(filled_spot_user_id) {
                        x.chosen_defenders.push(player.id);
                        is_added = true;
                    }
                }
            }
            Position::G => {
                if (context.pooler_roster[filled_spot_user_id]
                    .chosen_goalies
                    .len() as u8)
                    < self.settings.number_goalies
                {
                    if let Some(x) = context.pooler_roster.get_mut(filled_spot_user_id) {
                        x.chosen_goalies.push(player.id);
                        is_added = true;
                    }
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
        player: &Player,
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
        forw_list: &Vec<u32>,
        def_list: &Vec<u32>,
        goal_list: &Vec<u32>,
        reserv_list: &Vec<u32>,
    ) -> Result<(), AppError> {
        // Apply a roster modification. This action can only be done during the start and
        // end season on the days that the users are allowed to make roster modifications.
        // This is being hold in the variable self.settings.roster_modification_date

        self.validate_pool_status(&PoolState::InProgress)?;
        self.validate_participant(roster_modified_user_id)?;

        if user_id != roster_modified_user_id {
            // If the user making the request is not the roster asking to be modified, the user need to have privilege.
            self.has_privileges(user_id)?;
        }

        let start_season_date = NaiveDate::parse_from_str(START_SEASON_DATE, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        let mut today = Local::now().date_naive();

        let time = Local::now().time();

        // At 12PM we start to count the action for the next day.

        if time.hour() >= 12 {
            today += Duration::days(1);
        }

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
                        msg: format!("The selected players for the alignment are over the salary cap limit '{}$'.", team_salary_cap),
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
        roster.chosen_forwards = forw_list.clone();
        roster.chosen_defenders = def_list.clone();
        roster.chosen_goalies = goal_list.clone();
        roster.chosen_reservists = reserv_list.clone();
        Ok(())
    }

    pub fn protect_players(
        &mut self,
        user_id: &str,
        protected_players: &HashSet<u32>,
    ) -> Result<(), AppError> {
        // make sure the user making the resquest is a pool participants.
        self.validate_pool_status(&PoolState::Dynasty)?;
        self.validate_participant(user_id)?;

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
                msg: "The amount of players protected is not valid.".to_string(),
            });
        }

        // Validate that the players protection list does not contains dupplication and also validate that the user possess those players.
        let context = self.context.as_mut().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        if let Some(ref mut user_protected_players) = context.protected_players {
            for player_id in protected_players.iter() {
                let player =
                    context
                        .players
                        .get(&player_id.to_string())
                        .ok_or(AppError::CustomError {
                            msg: "This player is not included in this pool".to_string(),
                        })?;

                if !context.pooler_roster[user_id].validate_player_possession(player.id) {
                    return Err(AppError::CustomError {
                        msg: format!("You do not possess '{}'.", player.name),
                    });
                }

                user_protected_players.insert(user_id.to_string(), protected_players.clone());
            }
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
            let mut players_to_reserve = Vec::new();

            for player_id in protected_players.iter() {
                added_player_ids.insert(player_id.to_string());

                let player = context.players.get(&player_id.to_string()).ok_or_else(|| {
                    AppError::CustomError {
                        msg: "The player ID is not included in the pool.".to_string(),
                    }
                })?;

                // Add the player to the roster or reservists
                if context.can_add_player_to_roster(player, &pooler_user_id, &self.settings)? {
                    players_to_add.push(player.clone());
                } else {
                    players_to_reserve.push(player_id.clone());
                }
            }
            // After iterating, perform the mutations
            for player in players_to_add {
                context.add_drafted_player(&player, &pooler_user_id, &self.settings)?;
            }

            for player_id in players_to_reserve {
                context.add_player_to_reservists(player_id, &pooler_user_id)?;
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
        self.has_privileges(user_id)?;
        self.validate_pool_status(&PoolState::InProgress)?;

        let context = self.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "Pool context does not exist.".to_string(),
        })?;

        // Make sure the current date is after the end of the season.
        let end_season_date = NaiveDate::parse_from_str(&self.season_end, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        let today = Local::now().date_naive();

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

    pub fn can_update_in_progress_pool_settings(
        self,
        user_id: &str,
        settings: &PoolSettings,
    ) -> Result<(), AppError> {
        self.has_privileges(user_id)?;
        self.validate_pool_status(&PoolState::InProgress)?;

        if settings.number_forwards != self.settings.number_forwards
            || settings.number_defenders != self.settings.number_defenders
            || settings.number_goalies != self.settings.number_goalies
            || settings.number_reservists != self.settings.number_reservists
            || settings.dynasty_settings != self.settings.dynasty_settings
        {
            return Err(AppError::CustomError {
                msg: "These settings cannot be updated while the pool is in progress.".to_string(),
            }); // Need to make this robust, potentially need another pool status
        }

        Ok(())
    }

    pub fn can_update_pool_settings(self, user_id: &str) -> Result<(), AppError> {
        self.has_privileges(user_id)?;
        self.validate_pool_status(&PoolState::Created)?;

        Ok(())
    }

    pub fn start_draft(
        &mut self,
        user_id: &str,
        room_users: &Vec<RoomUser>,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::Created)?;
        self.has_owner_privileges(user_id)?;

        if self.settings.number_poolers as usize != room_users.len() {
            return Err(AppError::CustomError {
                msg: format!(
                    "The number of participants should be {}.",
                    self.settings.number_poolers
                ),
            });
        }

        // Shuffle the pool participants. so the draft order is
        let mut room_users = room_users.clone();
        room_users.shuffle(&mut thread_rng());

        self.status = PoolState::Draft;

        let user_ids: Vec<String> = room_users.iter().map(|user| user.id.clone()).collect();

        self.context = Some(PoolContext::new(&user_ids));

        self.participants = room_users.into_iter().map(PoolUser::from).collect();

        // Set the draft order with the shuffle list.
        self.draft_order = Some(user_ids);
        Ok(())
    }

    pub fn draft_player(&mut self, user_id: &str, player: &Player) -> Result<(), AppError> {
        // Match against

        let has_privileges = self.has_owner_rights(user_id);
        match (&mut self.context, &self.draft_order) {
            (Some(context), Some(draft_order)) => {
                let mut is_done = false;

                if self.settings.dynasty_settings.is_some() && context.past_tradable_picks.is_some()
                {
                    // This is a dynasty draft context.
                    // The final rank is being used as draft order.
                    is_done = context.draft_player_dynasty(
                        user_id,
                        player,
                        draft_order,
                        &self.settings,
                        has_privileges,
                    )?;
                } else {
                    // This is a dynasty draft context.
                    // The participant order is being used as draft order.
                    is_done = context.draft_player(
                        user_id,
                        player,
                        draft_order,
                        &self.settings,
                        has_privileges,
                    )?;
                }

                if is_done {
                    // The draft is done.
                    self.status = PoolState::InProgress;
                }

                Ok(())
            }
            _ => Err(AppError::CustomError {
                msg: "There is no pool context or draft order in the pool yet.".to_string(),
            }),
        }
    }

    pub fn undo_draft_player(&mut self, user_id: &str) -> Result<(), AppError> {
        // Undo the last draft selection.
        // This call can only be made if the user id is the owner.
        self.has_owner_privileges(user_id)?;
        self.validate_pool_status(&PoolState::Draft)?;

        match (&mut self.context, &self.draft_order) {
            (Some(context), Some(draft_order)) => {
                // This is a dynasty draft context.
                // The final rank is being used as draft order.
                context.undo_draft_player(draft_order, &self.settings)
            }
            _ => Err(AppError::CustomError {
                msg: "There is no pool context or draft order in the pool yet.".to_string(),
            }),
        }
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

        // ignore the unused_variables warning since the warning is false positive and is
        // caused by the compiler not recognizing the matches! patterns.
        #[allow(unused_variables)]
        if !matches!(&self.status, expected_status) {
            return Err(AppError::CustomError {
                msg: format!(
                    "The expected pool status '{}', current pool status '{}'.",
                    expected_status, self.status
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone)] // Copy
pub struct PoolContext {
    pub pooler_roster: HashMap<String, PoolerRoster>,
    pub players_name_drafted: Vec<u32>,
    pub score_by_day: Option<HashMap<String, HashMap<String, DailyRosterPoints>>>,
    pub tradable_picks: Option<Vec<HashMap<String, String>>>,
    pub past_tradable_picks: Option<Vec<HashMap<String, String>>>,
    pub protected_players: Option<HashMap<String, HashSet<u32>>>,
    pub players: HashMap<String, Player>,
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
        }
    }

    pub fn get_final_rank(&self, pool_settings: &PoolSettings) -> Result<Vec<String>, AppError> {
        let Some(score_by_day) = &self.score_by_day else {
            return Err(AppError::CustomError {
                msg: "No score is being recorded in this pool yet.".to_string(),
            });
        };

        // Map the user to its total points, total number of games
        // and for each player type, a hashmap of the player id with their corresponding total number of points, total number of games.
        let mut user_total_points: HashMap<
            String,
            (
                u16,                         // Total points.
                u16,                         // Total number of games.
                HashMap<String, (u16, u16)>, // Forwards
                HashMap<String, (u16, u16)>, // Defense
                HashMap<String, (u16, u16)>, // Goalies
            ),
        > = HashMap::new();

        for (date, daily_roster_points) in score_by_day {
            for (participant, roster_daily_points) in daily_roster_points {
                // Initialize the participant with 0 points and 0 games and no players.
                if !user_total_points.contains_key(participant) {
                    user_total_points.insert(
                        participant.clone(),
                        (0, 0, HashMap::new(), HashMap::new(), HashMap::new()),
                    );
                }

                // Return an error if at least one day have not been cumulated yet.
                if !roster_daily_points.is_cumulated {
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
                forwards_vec.sort_by(|a, b| a.1 .0.cmp(&b.1 .0).then_with(|| a.1 .1.cmp(&b.1 .1)));

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
                defenders_vec.sort_by(|a, b| a.1 .0.cmp(&b.1 .0).then_with(|| a.1 .1.cmp(&b.1 .1)));

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
                goalies_vec.sort_by(|a, b| a.1 .0.cmp(&b.1 .0).then_with(|| a.1 .1.cmp(&b.1 .1)));

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

        let mut user_points_vec: Vec<(
            &String,
            &(
                u16,
                u16,
                HashMap<String, (u16, u16)>,
                HashMap<String, (u16, u16)>,
                HashMap<String, (u16, u16)>,
            ),
        )> = user_total_points.iter().collect();

        // Sort the total points vector. And fill the final_rank list with it.
        // Sort the vector by total points and then by total games in descending order
        user_points_vec.sort_by(|a, b| {
            b.1 .0
                .cmp(&a.1 .0) // Compare total points
                .then_with(|| a.1 .1.cmp(&b.1 .1)) // If points are equal, compare total games (The pooler with less games wins)
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
        players: &HashMap<String, Player>,
    ) -> Result<f64, AppError> {
        let cumulated_salary_cap = pooler_roster
            .chosen_forwards
            .iter()
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
            .try_fold(0.0, |acc, salary_cap| salary_cap.map(|sc| acc + sc));

        cumulated_salary_cap
    }

    pub fn can_add_player_to_roster(
        &self,
        player: &Player,
        pool_user_id: &str,
        settings: &PoolSettings,
    ) -> Result<bool, AppError> {
        // If there is salary cap management, don't add to the starting roster players without contracts or if the user doesn't have enough space.
        let pooler_roster =
            self.pooler_roster
                .get(pool_user_id)
                .ok_or_else(|| AppError::CustomError {
                    msg: "Pooler roster does not exist.".to_string(),
                })?;

        if let Some(team_salary_cap) = settings.salary_cap {
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
        player: &Player,
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
                pooler_roster.chosen_reservists.push(player.id);
            }

            self.players.insert(player.id.to_string(), player.clone());
            self.players_name_drafted.push(player.id);
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
        player: &Player,
        draft_order: &Vec<String>, // being used as draft order.
        settings: &PoolSettings,
        has_privileges: bool,
    ) -> Result<bool, AppError> {
        // First, validate that the player selected is not already picked by any of the other poolers.

        for roster in self.pooler_roster.values() {
            if roster.validate_player_possession(player.id) {
                return Err(AppError::CustomError {
                    msg: "This player is already picked.".to_string(),
                });
            }
        }
        // Find the next draft id for dynasty type pool.
        let next_drafter = self.find_dynasty_next_drafter(draft_order, settings)?;

        if !has_privileges && next_drafter != user_id {
            return Err(AppError::CustomError {
                msg: format!("It is {}'s turn.", next_drafter),
            });
        }

        // Add the drafted player if everything goes right.
        self.add_drafted_player(player, &next_drafter, settings)?;

        self.is_draft_done(settings)
    }

    pub fn find_dynasty_next_drafter(
        &mut self,
        draft_order: &Vec<String>, // being used as draft order.
        settings: &PoolSettings,
    ) -> Result<String, AppError> {
        // Draft the right player in dynasty mode.
        // This takes into account the trade that have been traded during last season (past_tradable_picks).

        // Get the maximum number of player a user can draft.
        let max_player_count = settings.number_forwards
            + settings.number_defenders
            + settings.number_goalies
            + settings.number_reservists;

        let past_tradable_picks =
            self.past_tradable_picks
                .as_ref()
                .ok_or_else(|| AppError::CustomError {
                    msg: "Pool context does not exist.".to_string(),
                })?;

        // To make sure the program never go into an infinite loop. we use a counter.
        let mut continue_count = 0;
        let mut next_drafter;
        loop {
            let nb_players_drafted = self.players_name_drafted.len();

            let index_draft = draft_order.len() - 1 - (nb_players_drafted % draft_order.len());
            // Fetch the next drafter without considering if the trade has been traded yet.
            next_drafter = &draft_order[index_draft];

            if nb_players_drafted < (past_tradable_picks.len() * draft_order.len()) {
                // use the tradable_picks to see if the pick got traded so it is to the person owning the pick to draft.

                next_drafter =
                    &past_tradable_picks[nb_players_drafted / draft_order.len()][next_drafter];
            }

            if self.get_roster_count(next_drafter)? >= max_player_count as usize {
                self.players_name_drafted.push(0); // Id 0 means the players did not draft because his roster is already full

                continue_count += 1;

                if continue_count >= draft_order.len() {
                    return Err(AppError::CustomError {
                        msg: "All poolers have the maximum amount player drafted.".to_string(),
                    });
                }
                continue;
            }

            break;
        }

        Ok(next_drafter.clone())
    }

    pub fn draft_player(
        &mut self,
        user_id: &str,
        player: &Player,
        draft_order: &Vec<String>, // being used as draft order.
        settings: &PoolSettings,
        has_privileges: bool,
    ) -> Result<bool, AppError> {
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
        self.add_drafted_player(player, &next_drafter, settings)?;

        self.is_draft_done(settings)
    }

    pub fn undo_draft_player(
        &mut self,
        participants: &Vec<String>,
        settings: &PoolSettings,
    ) -> Result<(), AppError> {
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
                    })
                }
            }
        }

        let pick_number = self.players_name_drafted.len();
        let latest_drafter;

        match (&settings.dynasty_settings, &self.past_tradable_picks) {
            (Some(dynasty_settings), Some(past_tradable_picks)) => {
                // This comes from a Dynasty draft.

                let nb_tradable_picks = dynasty_settings.tradable_picks;

                let index = participants.len() - 1 - (pick_number % participants.len());

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
        Ok(())
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
            if let Some(tradable_picks) = &mut self.tradable_picks {
                if let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from) {
                    *owner = trade.ask_to.clone();
                }
            }
        }

        // Migrate picks "to" -> "from"
        for pick in trade.to_items.picks.iter() {
            if let Some(tradable_picks) = &mut self.tradable_picks {
                if let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from) {
                    *owner = trade.proposed_by.clone();
                }
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

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DailyRosterPoints {
    pub roster: Roster,
    pub is_cumulated: bool,
}

impl DailyRosterPoints {
    pub fn get_total_points(
        &self,
        pool_settings: &PoolSettings,
        forwards_points: &mut HashMap<String, (u16, u16)>,
        defenders_points: &mut HashMap<String, (u16, u16)>,
        goalies_points: &mut HashMap<String, (u16, u16)>,
    ) -> (u16, u16) {
        let mut total_points = 0;
        let mut number_of_games = 0;

        // Forwards
        for (player_id, skater_points) in &self.roster.F {
            if let Some(skater_points) = skater_points {
                let daily_points = skater_points.get_total_points(&pool_settings.forwards_settings);
                total_points += daily_points;
                number_of_games += 1;
                if let Some((points, number_of_games)) = forwards_points.get_mut(player_id) {
                    *points += daily_points;
                    *number_of_games += 1;
                } else {
                    forwards_points.insert(player_id.clone(), (daily_points, 1));
                }
            }
        }

        // Defenders
        for (player_id, skater_points) in &self.roster.D {
            if let Some(skater_points) = skater_points {
                let daily_points = skater_points.get_total_points(&pool_settings.defense_settings);
                total_points += daily_points;
                number_of_games += 1;

                if let Some((points, number_of_games)) = defenders_points.get_mut(player_id) {
                    *points += daily_points;
                    *number_of_games += 1;
                } else {
                    defenders_points.insert(player_id.clone(), (daily_points, 1));
                }
            }
        }

        // Goalies
        for (player_id, goalie_points) in &self.roster.G {
            if let Some(goalie_points) = goalie_points {
                let daily_points = goalie_points.get_total_points(&pool_settings.goalies_settings);
                total_points += daily_points;
                number_of_games += 1;

                if let Some((points, number_of_games)) = goalies_points.get_mut(player_id) {
                    *points += daily_points;
                    *number_of_games += 1;
                } else {
                    goalies_points.insert(player_id.clone(), (daily_points, 1));
                }
            }
        }

        (total_points, number_of_games)
    }
}
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Roster {
    pub F: HashMap<String, Option<SkaterPoints>>,
    pub D: HashMap<String, Option<SkaterPoints>>,
    pub G: HashMap<String, Option<GoalyPoints>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SkaterPoints {
    pub G: u8,
    pub A: u8,
    pub SOG: Option<u8>,
}

impl SkaterPoints {
    pub fn get_total_points(&self, skater_settings: &SkaterSettings) -> u16 {
        let mut total_points = 0;

        total_points += self.G as u16 * skater_settings.points_per_goals as u16
            + self.A as u16 * skater_settings.points_per_assists as u16;

        if let Some(shootout_goal) = self.SOG {
            total_points += shootout_goal as u16 * skater_settings.points_per_shootout_goals as u16;
        }

        if self.G >= 3 {
            total_points += skater_settings.points_per_hattricks as u16;
        }

        total_points
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoalyPoints {
    pub G: u8,
    pub A: u8,
    pub W: bool,
    pub SO: bool,
    pub OT: bool,
}

impl GoalyPoints {
    pub fn get_total_points(&self, goalies_settings: &GoaliesSettings) -> u16 {
        let mut total_points = 0;
        total_points += self.G as u16 * goalies_settings.points_per_goals as u16
            + self.A as u16 * goalies_settings.points_per_assists as u16;

        if self.W {
            total_points += goalies_settings.points_per_wins as u16;
        }

        if self.SO {
            total_points += goalies_settings.points_per_shutouts as u16;
        }

        if self.OT {
            total_points += goalies_settings.points_per_overtimes as u16;
        }

        total_points
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SkaterPoolPoints {
    pub G: u8,
    pub A: u8,
    pub HT: u8,
    pub SOG: u8,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoalyPoolPoints {
    pub G: u8,
    pub A: u8,
    pub W: u8,
    pub SO: u8,
    pub OT: u8,
}

impl PartialEq<Player> for Player {
    fn eq(&self, other: &Player) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Player {
    pub id: u32, // ID from the NHL API.
    pub name: String,
    pub team: Option<u32>,
    pub position: Position,
    pub age: Option<u8>,
    pub salary_cap: Option<f64>,
    pub contract_expiration_season: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Position {
    F,
    D,
    G,
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

// payload to sent when creating a new pool.
#[derive(Debug, Deserialize, Clone)]
pub struct PoolCreationRequest {
    pub pool_name: String,
    pub settings: PoolSettings,
}

// payload to sent when deleting a pool.
#[derive(Debug, Deserialize, Clone)]
pub struct PoolDeletionRequest {
    pub pool_name: String,
}

// payload to sent when adding player by the owner of the pool.
#[derive(Debug, Deserialize, Clone)]
pub struct AddPlayerRequest {
    pub pool_name: String,
    pub added_player_user_id: String,
    pub player: Player,
}

// payload to sent when removing player by the owner of the pool.
#[derive(Debug, Deserialize, Clone)]
pub struct RemovePlayerRequest {
    pub pool_name: String,
    pub removed_player_user_id: String,
    pub player_id: u32,
}

// payload to sent when creating a trade.
#[derive(Debug, Deserialize, Clone)]
pub struct CreateTradeRequest {
    pub pool_name: String,
    pub trade: Trade,
}

// payload to sent when cancelling a trade.
#[derive(Debug, Deserialize, Clone)]
pub struct DeleteTradeRequest {
    pub pool_name: String,
    pub trade_id: u32,
}

// payload to sent when responding to a trade.
#[derive(Debug, Deserialize, Clone)]
pub struct RespondTradeRequest {
    pub pool_name: String,
    pub trade_id: u32,
    pub is_accepted: bool,
}

// payload to sent when filling a spot with a reservist.
#[derive(Debug, Deserialize, Clone)]
pub struct FillSpotRequest {
    pub pool_name: String,
    pub filled_spot_user_id: String,
    pub player_id: u32,
}

// payload to sent when modifying roster of a pooler
#[derive(Debug, Deserialize, Clone)]
pub struct ModifyRosterRequest {
    pub pool_name: String,
    pub roster_modified_user_id: String,
    pub forw_list: Vec<u32>,
    pub def_list: Vec<u32>,
    pub goal_list: Vec<u32>,
    pub reserv_list: Vec<u32>,
}

// payload to sent when protecting the list of players for dynasty draft.
#[derive(Debug, Deserialize, Clone)]
pub struct ProtectPlayersRequest {
    pub pool_name: String,
    pub protected_players: HashSet<u32>,
}

// payload to sent when generating a new season for a dynasty type of pool.
#[derive(Debug, Deserialize, Clone)]
pub struct CompleteProtectionRequest {
    pub pool_name: String,
}

// payload to sent when updating pool settings.
#[derive(Debug, Deserialize, Clone)]
pub struct UpdatePoolSettingsRequest {
    pub pool_name: String,
    pub pool_settings: PoolSettings,
}

// payload to sent when marking a pool as final
#[derive(Debug, Deserialize, Clone)]
pub struct MarkAsFinalRequest {
    pub pool_name: String,
}

// payload to sent when generating a new season for a dynasty type of pool.
#[derive(Debug, Deserialize, Clone)]
pub struct GenerateDynastyRequest {
    pub pool_name: String,
    pub new_pool_name: String,
}
