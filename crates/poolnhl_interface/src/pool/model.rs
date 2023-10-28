use crate::errors::AppError;
use chrono::{Duration, Local, NaiveDate, Timelike, Utc};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
// Date for season

pub const START_SEASON_DATE: &str = "2023-10-10";
pub const END_SEASON_DATE: &str = "2024-04-18";

pub const TRADE_DEADLINE_DATE: &str = "2024-03-08";

#[derive(Deserialize, Serialize, Clone)]
pub struct ProjectedPoolShort {
    pub name: String, // the name of the pool.
    pub owner: String,
    pub status: PoolState, // State of the pool.
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DynastieSettings {
    // Other pool configuration
    pub next_season_number_players_protected: u8,
    pub tradable_picks: u8, // numbers of the next season picks participants are able to trade with each other.
}

impl PartialEq<DynastieSettings> for DynastieSettings {
    fn eq(&self, other: &DynastieSettings) -> bool {
        self.next_season_number_players_protected == other.next_season_number_players_protected
            && self.tradable_picks == other.tradable_picks
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PoolSettings {
    pub assistants: Vec<String>, // Participants that are allowed to make some pool modifications.
    // Roster configuration.
    pub number_forwards: u8,
    pub number_defenders: u8,
    pub number_goalies: u8,
    pub number_reservists: u8,
    pub number_worst_forwards_to_ignore: u8,
    pub number_worst_defenders_to_ignore: u8,
    pub number_worst_goalies_to_ignore: u8,
    pub roster_modification_date: Vec<String>, // Date where reservist can be traded.

    // Forwards points configuration.
    pub forward_pts_goals: u8,
    pub forward_pts_assists: u8,
    pub forward_pts_hattricks: u8,
    pub forward_pts_shootout_goals: u8,

    // Defenders points configuration.
    pub defender_pts_goals: u8,
    pub defender_pts_assists: u8,
    pub defender_pts_hattricks: u8,
    pub defender_pts_shootout_goals: u8,

    // Goalies points configuration.
    pub goalies_pts_wins: u8,
    pub goalies_pts_shutouts: u8,
    pub goalies_pts_overtimes: u8,
    pub goalies_pts_goals: u8,
    pub goalies_pts_assists: u8,

    pub can_trade: bool, // Tell if trades are activated.

    pub dynastie_settings: Option<DynastieSettings>,
}

impl PoolSettings {
    pub fn new() -> Self {
        Self {
            assistants: Vec::new(),
            number_forwards: 9,
            number_defenders: 4,
            number_goalies: 2,
            number_reservists: 2,
            number_worst_forwards_to_ignore: 0,
            number_worst_defenders_to_ignore: 0,
            number_worst_goalies_to_ignore: 0,
            roster_modification_date: Vec::new(),
            forward_pts_goals: 2,
            forward_pts_assists: 1,
            forward_pts_hattricks: 3,
            forward_pts_shootout_goals: 1,
            defender_pts_goals: 3,
            defender_pts_assists: 2,
            defender_pts_hattricks: 2,
            defender_pts_shootout_goals: 1,
            goalies_pts_wins: 2,
            goalies_pts_shutouts: 3,
            goalies_pts_goals: 3,
            goalies_pts_assists: 2,
            goalies_pts_overtimes: 1,
            can_trade: false,
            dynastie_settings: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pool {
    pub name: String, // the name of the pool.
    pub owner: String,
    pub number_poolers: u8, // the number of participants in the pool.

    pub participants: Option<Vec<String>>, // The ID of each participants.

    pub settings: PoolSettings,

    pub status: PoolState, // State of the pool.
    pub final_rank: Option<Vec<String>>,

    pub nb_player_drafted: u8,

    // Trade information.
    pub nb_trade: u32,
    pub trades: Option<Vec<Trade>>,

    // context of the pool.
    pub context: Option<PoolContext>,
    pub date_updated: i64,
    pub season_start: String,
    pub season_end: String,
}

impl Pool {
    pub fn new(pool_name: &str, owner: &str, nuber_poolers: u8) -> Self {
        Self {
            name: pool_name.to_string(),
            owner: owner.to_string(),
            number_poolers: nuber_poolers,
            participants: None,
            settings: PoolSettings::new(),
            status: PoolState::Created,
            final_rank: None,
            nb_player_drafted: 0,
            nb_trade: 0,
            trades: None,
            context: None,
            date_updated: 0,
            season_start: START_SEASON_DATE.to_string(),
            season_end: END_SEASON_DATE.to_string(),
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

        match &self.context {
            None => Err(AppError::CustomError {
                msg: "There is no context to the pool yet.".to_string(),
            }),
            Some(pool_context) => {
                pool_context.validate_trade(trade)?;

                // does the proposedBy and askTo field are valid

                if !pool_context.pooler_roster.contains_key(&trade.proposed_by)
                    || !pool_context.pooler_roster.contains_key(&trade.ask_to)
                {
                    return Err(AppError::CustomError {
                        msg: "The users in the trade are not in the pool.".to_string(),
                    });
                }

                match &mut self.trades {
                    None => Err(AppError::CustomError {
                        msg: "There is no trade to the pool yet.".to_string(),
                    }),
                    Some(trades) => {
                        // Make sure that user can only have 1 active trade at a time.
                        //return an error if already one trade active in this pool. (Active trade = NEW )

                        for trade in trades.iter() {
                            if (matches!(trade.status, TradeStatus::NEW))
                                && (trade.proposed_by == trade.proposed_by)
                            {
                                return Err(AppError::CustomError {
                                    msg: "User can only have one active trade at a time."
                                        .to_string(),
                                });
                            }
                        }

                        trade.date_created = Utc::now().timestamp_millis();
                        trade.status = TradeStatus::NEW;
                        trade.id = self.nb_trade;
                        trades.push(trade.clone());

                        Ok(())
                    }
                }
            }
        }
    }

    pub fn delete_trade(&mut self, user_id: &str, trade_id: u32) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;
        if self.nb_trade < trade_id {
            return Err(AppError::CustomError {
                msg: "This trade does not exist.".to_string(),
            });
        }

        // Owner and pool assistant can delete any new trade.
        let priviledge_right =
            self.has_owner_rights(user_id) || self.has_assistants_rights(user_id);

        match &mut self.trades {
            None => Err(AppError::CustomError {
                msg: "There is no trade to the pool yet.".to_string(),
            }),
            Some(trades) => {
                match trades.iter().position(|trade| trade.id == trade_id) {
                    None => Err(AppError::CustomError {
                        msg: "The trade was not found.".to_string(),
                    }),
                    Some(i) => {
                        // validate that the status of the trade is NEW

                        if !matches!(trades[i].status, TradeStatus::NEW) {
                            return Err(AppError::CustomError {
                                msg: "The trade is not in a valid state to be deleted.".to_string(),
                            });
                        }

                        // validate that only the one that create the trade or the
                        // owner/assistants can delete it.

                        if !priviledge_right && trades[i].proposed_by != *user_id {
                            return Err(AppError::CustomError {
                                msg: "Only the one that created the trade can cancel it."
                                    .to_string(),
                            });
                        }

                        trades.remove(i);
                        Ok(())
                    }
                }
            }
        }
    }

    pub fn respond_trade(
        &mut self,
        user_id: &str,
        is_accepted: bool,
        trade_id: u32,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;

        if self.nb_trade < trade_id {
            return Err(AppError::CustomError {
                msg: "This trade does not exist.".to_string(),
            });
        }
        // Owner and pool assistant can respond any new trade.
        let priviledge_right =
            self.has_owner_rights(user_id) || self.has_assistants_rights(user_id);

        match &mut self.trades {
            None => Err(AppError::CustomError {
                msg: "The trade was not found.".to_string(),
            }),
            Some(trades) => {
                match trades.iter().position(|trade| trade.id == trade_id) {
                    None => Err(AppError::CustomError {
                        msg: "The trade was not found.".to_string(),
                    }),
                    Some(i) => {
                        // validate that the status of the trade is NEW

                        if !matches!(trades[i].status, TradeStatus::NEW) {
                            return Err(AppError::CustomError {
                                msg: "The trade is not in a valid state to be responded."
                                    .to_string(),
                            });
                        }

                        // validate that only the one that was ask for the trade or the owner can accept it.

                        if !priviledge_right && trades[i].ask_to != *user_id {
                            return Err(AppError::CustomError {
                                msg: "Only the one that was ask for the trade or the owner can accept it."
                                    .to_string(),
                            });
                        }

                        // validate that 24h have been passed since the trade was created.
                        let now = Utc::now().timestamp_millis();

                        if trades[i].date_created + 8640000 > now {
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
                                    pool_context.trade_roster_items(&trades[i])?;
                                    trades[i].status = TradeStatus::ACCEPTED;
                                    trades[i].date_accepted = Utc::now().timestamp_millis();
                                    Ok(())
                                }
                            }
                        } else {
                            trades[i].status = TradeStatus::REFUSED;
                            Ok(())
                        }
                    }
                }
            }
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

        match &mut self.context {
            None => Err(AppError::CustomError {
                msg: "The pool has no context yet.".to_string(),
            }),
            Some(context) => {
                // Is the player in the pool?
                let player =
                    context
                        .players
                        .get(&player_id.to_string())
                        .ok_or(AppError::CustomError {
                            msg: "This player is not included in the pool.".to_string(),
                        })?;

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
                        msg: "The player should only be in the reservist pooler's list."
                            .to_string(),
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
        }
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

        match &mut self.context {
            None => Err(AppError::CustomError {
                msg: "The pool has no context yet.".to_string(),
            }),
            Some(context) => {
                if !context.pooler_roster.contains_key(added_to_user_id) {
                    return Err(AppError::CustomError {
                        msg: "The user is not in the pool.".to_string(),
                    });
                }

                // First, validate that the player selected is not picked by any of the other poolers.
                match &self.participants {
                    None => Err(AppError::CustomError {
                        msg: "The pool has no context yet.".to_string(),
                    }),
                    Some(participants) => {
                        for participant in participants.iter() {
                            if context.pooler_roster[participant]
                                .validate_player_possession(player.id)
                            {
                                return Err(AppError::CustomError {
                                    msg: "This player is already picked.".to_string(),
                                });
                            }
                        }

                        context.add_player_to_roster(player.id, added_to_user_id)?;

                        context
                            .players
                            .insert(player.id.to_string(), player.clone());

                        Ok(())
                    }
                }
            }
        }
    }

    pub fn remove_player(
        &mut self,
        user_id: &str,
        removed_to_user_id: &str,
        player_id: u32,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::InProgress)?;
        self.has_privileges(user_id)?;

        match &mut self.context {
            None => Err(AppError::CustomError {
                msg: "The pool has no context yet.".to_string(),
            }),
            Some(context) => {
                if !context.pooler_roster.contains_key(removed_to_user_id) {
                    return Err(AppError::CustomError {
                        msg: "The user is not in the pool.".to_string(),
                    });
                }

                // First, validate that the player selected is not picked by any of the other poolers.
                if !context.pooler_roster[removed_to_user_id].validate_player_possession(player_id)
                {
                    return Err(AppError::CustomError {
                        msg: "This player is not own by the user.".to_string(),
                    });
                }
                context.remove_player_from_roster(player_id, removed_to_user_id)?;
                Ok(())
            }
        }
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
        let end_season_date = NaiveDate::parse_from_str(END_SEASON_DATE, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        let mut today = Local::now().date_naive();

        let time = Local::now().time();

        // At 12PM we start to count the action for the next day.

        if time.hour() >= 12 {
            today += Duration::days(1);
        }

        if today > start_season_date && today <= end_season_date {
            let mut is_allowed = false;

            for date in &self.settings.roster_modification_date {
                let sathurday = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

                if sathurday == today {
                    is_allowed = true;
                    break;
                }
            }

            if !is_allowed {
                return Err(AppError::CustomError {
                    msg: "You are not allowed to modify your roster today.".to_string(),
                });
            }
        }

        match &mut self.context {
            None => Err(AppError::CustomError {
                msg: "The pool has no context yet.".to_string(),
            }),
            Some(context) => {
                // Validate the total amount of forwards selected

                if forw_list.len() != self.settings.number_forwards as usize {
                    return Err(AppError::CustomError {
                        msg: "The amount of forwards selected is not valid".to_string(),
                    });
                }

                // Validate the total amount of defenders selected

                if def_list.len() != self.settings.number_defenders as usize {
                    return Err(AppError::CustomError {
                        msg: "The amount of defenders selected is not valid".to_string(),
                    });
                }

                // Validate the total amount of goalies selected

                if goal_list.len() != self.settings.number_goalies as usize {
                    return Err(AppError::CustomError {
                        msg: "The amount of goalies selected is not valid".to_string(),
                    });
                }

                // Validate the total amount of players selected (It should be the same as before)

                if let Some(roster) = context.pooler_roster.get(roster_modified_user_id) {
                    let amount_selected_players =
                        forw_list.len() + def_list.len() + goal_list.len() + reserv_list.len();

                    let amount_players_before = roster.chosen_forwards.len()
                        + roster.chosen_defenders.len()
                        + roster.chosen_goalies.len()
                        + roster.chosen_reservists.len();

                    if amount_players_before != amount_selected_players {
                        return Err(AppError::CustomError {
                            msg: "The amount of selected players is not valid.".to_string(),
                        });
                    }
                }

                // validate each selected players possession by the user asking the modification.
                // Also validate dupplication in the new list.

                let mut selected_player_map = HashSet::new(); // used to validate dupplication

                // Validate that the roster modification does not contains Dupplication and also validate that the user possess those players.

                for player_id in forw_list.iter().chain(
                    def_list
                        .iter()
                        .chain(goal_list.iter())
                        .chain(reserv_list.iter()),
                ) {
                    let player = context.players.get(&player_id.to_string()).ok_or(
                        AppError::CustomError {
                            msg: "This player is not included in this pool".to_string(),
                        },
                    )?;

                    if selected_player_map.contains(&player.id) {
                        return Err(AppError::CustomError {
                            msg: format!("The player '{}' was dupplicated", player.name),
                        });
                    }
                    selected_player_map.insert(player.id);
                    if !context.pooler_roster[roster_modified_user_id]
                        .validate_player_possession(player.id)
                    {
                        return Err(AppError::CustomError {
                            msg: format!("You do not possess '{}'.", player.name),
                        });
                    }
                }

                // Finally update the roster of the player if everything went well.
                if let Some(roster) = context.pooler_roster.get_mut(roster_modified_user_id) {
                    roster.chosen_forwards = forw_list.clone();
                    roster.chosen_defenders = def_list.clone();
                    roster.chosen_goalies = goal_list.clone();
                    roster.chosen_reservists = reserv_list.clone();
                }
                Ok(())
            }
        }
    }

    pub fn protect_players(
        &mut self,
        user_id: &str,
        forw_protected: &Vec<u32>,
        def_protected: &Vec<u32>,
        goal_protected: &Vec<u32>,
        reserv_protected: &Vec<u32>,
    ) -> Result<(), AppError> {
        // make sure the user making the resquest is a pool participants.

        self.validate_pool_status(&PoolState::Dynastie)?;
        self.validate_participant(user_id)?;

        match &self.settings.dynastie_settings {
            None => Err(AppError::CustomError {
                msg: "Dynastie settings does not exist.".to_string(),
            }),
            Some(dynastie_settings) => {
                // validate that the numbers of players protected is ok.

                if forw_protected.len() > self.settings.number_forwards as usize {
                    return Err(AppError::CustomError {
                        msg: "To much forwards protected".to_string(),
                    });
                }

                if def_protected.len() > self.settings.number_defenders as usize {
                    return Err(AppError::CustomError {
                        msg: "To much defenders protected".to_string(),
                    });
                }

                if goal_protected.len() > self.settings.number_goalies as usize {
                    return Err(AppError::CustomError {
                        msg: "To much goalies protected".to_string(),
                    });
                }

                if reserv_protected.len() > self.settings.number_reservists as usize {
                    return Err(AppError::CustomError {
                        msg: "To much reservists protected".to_string(),
                    });
                }

                let tot_player_protected = forw_protected.len()
                    + def_protected.len()
                    + goal_protected.len()
                    + reserv_protected.len();

                if tot_player_protected as u8
                    != dynastie_settings.next_season_number_players_protected
                {
                    return Err(AppError::CustomError {
                        msg: "The number of protected players is not valid".to_string(),
                    });
                }

                // Validate that the players protection list does not contains dupplication and also validate that the user possess those players.

                let mut selected_player_map = HashSet::new(); // used to validate dupplication

                match &mut self.context {
                    None => Err(AppError::CustomError {
                        msg: "The pool has no context yet.".to_string(),
                    }),
                    Some(context) => {
                        for player_id in forw_protected.iter().chain(
                            def_protected
                                .iter()
                                .chain(goal_protected.iter())
                                .chain(reserv_protected.iter()),
                        ) {
                            let player = context.players.get(&player_id.to_string()).ok_or(
                                AppError::CustomError {
                                    msg: "This player is not included in this pool".to_string(),
                                },
                            )?;
                            // Make sure the player is not dupplicated. if so return an error.
                            if selected_player_map.contains(&player.id) {
                                return Err(AppError::CustomError {
                                    msg: format!("The player '{}' was dupplicated", player.name),
                                });
                            }

                            selected_player_map.insert(player.id);

                            if !context.pooler_roster[user_id].validate_player_possession(player.id)
                            {
                                return Err(AppError::CustomError {
                                    msg: format!("You do not possess '{}'.", player.name),
                                });
                            }
                        }

                        // clear previous season roster and add those players list to the new roster.

                        if let Some(roster) = context.pooler_roster.get_mut(user_id) {
                            roster.chosen_forwards = forw_protected.clone();
                            roster.chosen_defenders = def_protected.clone();
                            roster.chosen_goalies = goal_protected.clone();
                            roster.chosen_reservists = reserv_protected.clone();
                        }

                        // Look if all participants have protected their players

                        for (_, roster) in &context.pooler_roster {
                            if roster.chosen_forwards.len()
                                + roster.chosen_defenders.len()
                                + roster.chosen_goalies.len()
                                + roster.chosen_reservists.len()
                                != dynastie_settings.next_season_number_players_protected as usize
                            {
                                return Ok(());
                            }
                        }

                        // All participants have protected their players, we can migrate pool status from dynastie to draft.
                        self.status = PoolState::Draft;

                        Ok(())
                    }
                }
            }
        }
    }

    pub fn mark_as_final(&mut self, user_id: &str) -> Result<(), AppError> {
        self.has_privileges(user_id)?;
        self.validate_pool_status(&PoolState::InProgress)?;

        let Some(context) = &self.context else {
            return Err(AppError::CustomError {
                msg: "The pool has no context yet.".to_string(),
            });
        };

        // Get the final ranking of the pool. For dynastie pool, this will be use as draft order for the next season.
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
            || settings.dynastie_settings != self.settings.dynastie_settings
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
        participants: &Vec<String>,
    ) -> Result<(), AppError> {
        self.validate_pool_status(&PoolState::Created)?;
        self.has_owner_privileges(user_id)?;

        if self.number_poolers as usize != participants.len() {
            return Err(AppError::CustomError {
                msg: "The number of participants is not good.".to_string(),
            });
        }
        let mut participants = participants.clone();
        participants.shuffle(&mut thread_rng());
        self.participants = Some(participants.clone());

        // TODO: randomize the list of participants so the draft order is random

        self.status = PoolState::Draft;
        self.context = Some(PoolContext::new(&participants));
        Ok(())
    }

    pub fn draft_player(&mut self, user_id: &str, player: &Player) -> Result<(), AppError> {
        // Match against

        let has_privileges = self.has_owner_rights(user_id);
        match (&mut self.context, &self.participants, &self.final_rank) {
            (Some(context), _, Some(final_rank)) => {
                // This is a dynastie draft context.
                // The final rank is being used as draft order.
                if context.draft_player_dynastie(
                    user_id,
                    player,
                    final_rank,
                    &self.settings,
                    has_privileges,
                )? {
                    // The draft is done.
                    self.status = PoolState::InProgress;
                }
                Ok(())
            }
            (Some(context), Some(participants), None) => {
                // This is a dynastie draft context.
                // The participant order is being used as draft order.
                if context.draft_player(
                    user_id,
                    player,
                    participants,
                    &self.settings,
                    has_privileges,
                )? {
                    // The draft is done.
                    self.status = PoolState::InProgress;
                }
                Ok(())
            }
            _ => Err(AppError::CustomError {
                msg: "There is no pool context or participants in the pool yet.".to_string(),
            }),
        }
    }

    pub fn undo_draft_player(&mut self, user_id: &str) -> Result<(), AppError> {
        // Undo the last draft selection.
        // This call can only be made if the user id is the owner.
        self.has_owner_privileges(user_id)?;
        self.validate_pool_status(&PoolState::Draft)?;

        match (&mut self.context, &self.participants, &self.final_rank) {
            (Some(context), _, Some(final_rank)) => {
                // This is a dynastie draft context.
                // The final rank is being used as draft order.
                context.undo_draft_player(final_rank, &self.settings)
            }
            (Some(context), Some(participants), None) => {
                // This is a classic draft context.
                // The participants order is being used as draft order.
                context.undo_draft_player(participants, &self.settings)
            }
            _ => Err(AppError::CustomError {
                msg: "There is no pool context or participants in the pool yet.".to_string(),
            }),
        }
    }

    pub fn validate_participant(&self, user_id: &str) -> Result<(), AppError> {
        // Validate that the user is a pool participant.
        match &self.participants {
            None => Err(AppError::CustomError {
                msg: "Pool has no participants yet.".to_string(),
            }),
            Some(participants) => {
                if !participants.contains(&user_id.to_string()) {
                    return Err(AppError::CustomError {
                        msg: format!("User {} is not a pool participants.", user_id),
                    });
                }

                Ok(())
            }
        }
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
    Dynastie,
    Draft,
    Created,
}

impl fmt::Display for PoolState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // To be able to print out the PoolState enum.
        match self {
            PoolState::Final => write!(f, "Final"),
            PoolState::InProgress => write!(f, "In progress"),
            PoolState::Dynastie => write!(f, "Dynastie"),
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
    pub players: HashMap<String, Player>,
}

impl PoolContext {
    pub fn new(participants: &Vec<String>) -> Self {
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
            players: HashMap::new(),
        }
    }

    pub fn get_final_rank(&self, pool_settings: &PoolSettings) -> Result<Vec<String>, AppError> {
        let Some(score_by_day) = &self.score_by_day else {
            return Err(AppError::CustomError {
                msg: "No score is being recorded in this pool yet.".to_string(),
            });
        };

        let mut user_total_points: HashMap<String, u16> = HashMap::new();
        let mut total_points_to_user: HashMap<u16, String> = HashMap::new();

        for (date, score_by_day) in score_by_day {
            for (participant, roster_daily_points) in score_by_day {
                if !user_total_points.contains_key(participant) {
                    user_total_points.insert(participant.clone(), 0);
                }

                if !roster_daily_points.is_cumulated {
                    return Err(AppError::CustomError {
                        msg: format!(
                            "There are no cumulative data on the {date} for the user {participant}"
                        ),
                    });
                }

                if let Some(tot) = user_total_points.get_mut(participant) {
                    *tot += roster_daily_points.get_total_points(pool_settings);
                }
            }
        }

        for (participant, total_points) in &user_total_points {
            total_points_to_user.insert(*total_points, participant.clone());
        }

        // With the full season cumulated, we can determine what is the final rank for this pool.
        let mut final_rank = Vec::new();
        let mut total_points: Vec<u16> = user_total_points.into_values().collect();

        // TODO: needs to consider the settings that ignore the X worst players of each position.

        // Sort the total points vector. And fill the final_rank list with it.
        total_points.sort();
        total_points.reverse();

        for points in total_points {
            final_rank.push(total_points_to_user[&points].clone())
        }

        Ok(final_rank)
    }

    pub fn add_drafted_player(
        &mut self,
        player: &Player,
        next_drafter: &str,
        settings: &PoolSettings,
    ) -> Result<(), AppError> {
        // Then, Add the chosen player in its right spot.
        // When there is no place in the position of the player we will add it to the reservists.

        if let Some(pooler_roster) = self.pooler_roster.get_mut(next_drafter) {
            let mut is_added = false;

            match player.position {
                Position::F => {
                    if (pooler_roster.chosen_forwards.len() as u8) < settings.number_forwards {
                        pooler_roster.chosen_forwards.push(player.id);
                        is_added = true;
                    }
                }
                Position::D => {
                    if (pooler_roster.chosen_defenders.len() as u8) < settings.number_defenders {
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

            // If the there is not enough place in the roster, try to add the player in the reservists.
            if !is_added {
                if (pooler_roster.chosen_reservists.len() as u8) < settings.number_reservists {
                    pooler_roster.chosen_reservists.push(player.id);
                } else {
                    return Err(AppError::CustomError {
                        msg: "Not enough space for this player.".to_string(),
                    });
                }
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

        for (participant, _) in &self.pooler_roster {
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

            if let Some(dynastie_settings) = &settings.dynastie_settings {
                for _pick_round in 0..dynastie_settings.tradable_picks {
                    let mut round = HashMap::new();

                    for (participant, _) in &self.pooler_roster {
                        round.insert(participant.clone(), participant.clone());
                    }

                    new_tradable_picks.push(round);
                }
            }

            self.tradable_picks = Some(new_tradable_picks);
        }
        Ok(is_done)
    }

    pub fn draft_player_dynastie(
        &mut self,
        user_id: &str,
        player: &Player,
        final_rank: &Vec<String>, // being used as draft order.
        settings: &PoolSettings,
        has_privileges: bool,
    ) -> Result<bool, AppError> {
        // First, validate that the player selected is not already picked by any of the other poolers.

        for (_, roster) in &self.pooler_roster {
            if roster.validate_player_possession(player.id) {
                return Err(AppError::CustomError {
                    msg: "This player is already picked.".to_string(),
                });
            }
        }
        // Find the next draft id for dynastie type pool.
        let next_drafter = self.find_dynastie_next_drafter(final_rank, settings)?;

        if !has_privileges && next_drafter != user_id {
            return Err(AppError::CustomError {
                msg: format!("It is {}'s turn.", next_drafter),
            });
        }

        // Add the drafted player if everything goes right.
        self.add_drafted_player(player, &next_drafter, settings)?;

        self.is_draft_done(settings)
    }

    pub fn find_dynastie_next_drafter(
        &mut self,
        final_rank: &Vec<String>, // being used as draft order.
        settings: &PoolSettings,
    ) -> Result<String, AppError> {
        // Draft the right player in dynastie mode.
        // This takes into account the trade that have been traded during last season (past_tradable_picks).

        // Get the maximum number of player a user can draft.
        let max_player_count = settings.number_forwards
            + settings.number_defenders
            + settings.number_goalies
            + settings.number_reservists;

        match &self.past_tradable_picks {
            None => Err(AppError::CustomError {
                msg: "There should be tradable_picks in dynastie type pool.".to_string(),
            }),
            Some(past_tradable_picks) => {
                // To make sure the program never go into an infinite loop. we use a counter.
                let mut continue_count = 0;
                let mut next_drafter;
                loop {
                    let nb_players_drafted = self.players_name_drafted.len();

                    let index_draft =
                        final_rank.len() - 1 - (nb_players_drafted % final_rank.len());
                    // Fetch the next drafter without considering if the trade has been traded yet.
                    next_drafter = &final_rank[index_draft];

                    if nb_players_drafted < (past_tradable_picks.len() * final_rank.len()) {
                        // use the tradable_picks to see if the pick got traded so it is to the person owning the pick to draft.

                        next_drafter = &past_tradable_picks[nb_players_drafted / final_rank.len()]
                            [next_drafter];
                    }

                    if self.get_roster_count(next_drafter)? >= max_player_count as usize {
                        self.players_name_drafted.push(0); // Id 0 means the players did not draft because his roster is already full

                        continue_count += 1;

                        if continue_count >= final_rank.len() {
                            return Err(AppError::CustomError {
                                msg: "All poolers have the maximum amount player drafted."
                                    .to_string(),
                            });
                        }
                        continue;
                    }

                    break;
                }

                Ok(next_drafter.clone())
            }
        }
    }

    pub fn draft_player(
        &mut self,
        user_id: &str,
        player: &Player,
        participants: &Vec<String>, // being used as draft order.
        settings: &PoolSettings,
        has_privileges: bool,
    ) -> Result<bool, AppError> {
        // Draft the right player in normal mode.
        // Taking only into account the draft order

        for (_, roster) in &self.pooler_roster {
            if roster.validate_player_possession(player.id) {
                return Err(AppError::CustomError {
                    msg: "This player is already picked.".to_string(),
                });
            }
        }

        // there is no final rank so this is the newly created draft logic.

        let players_drafted = self.players_name_drafted.len();

        // Snake draft, reverse draft order each round.
        let round = players_drafted / participants.len();

        let index = if round % 2 == 1 {
            participants.len() - 1 - (players_drafted % participants.len())
        } else {
            players_drafted % participants.len()
        };

        let next_drafter = &participants[index];

        if !has_privileges && next_drafter != user_id {
            return Err(AppError::CustomError {
                msg: format!("It is {}'s turn.", next_drafter),
            });
        }

        // Add the drafted player if everything goes right.
        self.add_drafted_player(player, next_drafter, settings)?;

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

        match (&settings.dynastie_settings, &self.past_tradable_picks) {
            (Some(dynastie_settings), Some(past_tradable_picks)) => {
                // This comes from a Dynastie draft.

                let nb_tradable_picks = dynastie_settings.tradable_picks;

                let index = participants.len() - 1 - (pick_number % participants.len());

                let next_drafter = &participants[index];

                if pick_number < nb_tradable_picks as usize * participants.len() {
                    // use the tradable_picks to see who will draft next.

                    latest_drafter =
                        past_tradable_picks[pick_number / participants.len()][next_drafter].clone();
                } else {
                    // Use the final_rank to see who draft next.
                    latest_drafter = next_drafter.clone();
                }
            }
            _ => {
                // this comes from a newly created draft.

                // Snake draft, reverse draft order each round.
                let round = pick_number / participants.len();

                let index = if round % 2 == 1 {
                    participants.len() - 1 - (pick_number % participants.len())
                } else {
                    pick_number % participants.len()
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

    pub fn add_player_to_roster(&mut self, player_id: u32, user_id: &str) -> Result<(), AppError> {
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
        self.add_player_to_roster(player_id, user_receiver)
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
    pub fn get_total_points(&self, pool_settings: &PoolSettings) -> u16 {
        let mut total_points = 0;

        // Forwards
        for (_, skater_points) in &self.roster.F {
            if let Some(skater_points) = skater_points {
                total_points += skater_points.G as u16 * pool_settings.forward_pts_goals as u16
                    + skater_points.A as u16 * pool_settings.forward_pts_assists as u16;

                if let Some(shootout_goal) = skater_points.SOG {
                    total_points +=
                        shootout_goal as u16 * pool_settings.forward_pts_shootout_goals as u16;
                }

                if skater_points.G >= 3 {
                    total_points += pool_settings.forward_pts_hattricks as u16;
                }
            }
        }

        // Defenders
        for (_, skater_points) in &self.roster.D {
            if let Some(skater_points) = skater_points {
                total_points += skater_points.G as u16 * pool_settings.defender_pts_goals as u16
                    + skater_points.A as u16 * pool_settings.defender_pts_assists as u16;

                if let Some(shootout_goal) = skater_points.SOG {
                    total_points +=
                        shootout_goal as u16 * pool_settings.defender_pts_shootout_goals as u16;
                }

                if skater_points.G >= 3 {
                    total_points += pool_settings.defender_pts_hattricks as u16;
                }
            }
        }

        // Goalies
        for (_, goalie_points) in &self.roster.G {
            if let Some(goalie_points) = goalie_points {
                total_points += goalie_points.G as u16 * pool_settings.goalies_pts_goals as u16
                    + goalie_points.A as u16 * pool_settings.goalies_pts_assists as u16;

                if goalie_points.W {
                    total_points += pool_settings.goalies_pts_wins as u16;
                }

                if goalie_points.SO {
                    total_points += pool_settings.goalies_pts_shutouts as u16;
                }

                if goalie_points.OT {
                    total_points += pool_settings.goalies_pts_overtimes as u16;
                }
            }
        }

        return total_points;
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

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoalyPoints {
    pub G: u8,
    pub A: u8,
    pub W: bool,
    pub SO: bool,
    pub OT: bool,
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
    pub team: u32,
    pub position: Position,
    pub caps: Option<Vec<u32>>,
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
    pub number_pooler: u8,
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

// payload to sent when protecting the list of players for dynastie draft.
#[derive(Debug, Deserialize, Clone)]
pub struct ProtectPlayersRequest {
    pub pool_name: String,
    pub forw_protected: Vec<u32>,
    pub def_protected: Vec<u32>,
    pub goal_protected: Vec<u32>,
    pub reserv_protected: Vec<u32>,
}

// payload to sent when updating pool settings.
#[derive(Debug, Deserialize, Clone)]
pub struct UpdatePoolSettingsRequest {
    pub pool_name: String,
    // Roster configuration.
    pub pool_settings: PoolSettings,
}

// payload to sent when marking a pool as final
#[derive(Debug, Deserialize, Clone)]
pub struct MarkAsFinalRequest {
    pub pool_name: String,
}
