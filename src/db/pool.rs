use crate::errors::response::AppError;
use crate::errors::response::Result;
use chrono::{Date, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use futures::stream::TryStreamExt;
use mongodb::bson::Document;
use mongodb::bson::{doc, to_bson};
use mongodb::options::{FindOneAndUpdateOptions, FindOneOptions, FindOptions, ReturnDocument};
use mongodb::{Collection, Database};
use std::collections::HashMap;

use crate::models::pool::{
    Player, Pool, PoolContext, PoolCreationRequest, PoolState, PoolerRoster, Position,
    ProjectedPoolShort, Trade, TradeItems, TradeStatus, UpdatePoolSettingsRequest,
};

use crate::db::user::add_pool_to_users;
use crate::models::user::User;

use crate::models::response::PoolMessageResponse;

// Date for season

const START_SEASON_DATE: &str = "2022-10-07";
const END_SEASON_DATE: &str = "2023-04-13";

const TRADE_DEADLINE_DATE: &str = "2023-03-03";

// Return the complete Pool information
pub async fn find_pool_by_name(db: &Database, _name: &str) -> Result<Pool> {
    let collection = db.collection::<Pool>("pools");

    let pool = collection.find_one(doc! {"name": _name}, None).await?;

    pool.ok_or(AppError::CustomError {
        msg: format!("no pool found with name {}", _name),
    })
}

pub async fn find_optional_short_pool_by_name(
    collection: &Collection<Pool>,
    _name: &str,
) -> Result<Option<Pool>> {
    let find_option = FindOneOptions::builder()
        .projection(doc! {"context.score_by_day": 0})
        .build();

    let short_pool = collection
        .find_one(doc! {"name": &_name}, find_option)
        .await?;

    Ok(short_pool)
}

// Return the pool information without the score_by_day member
pub async fn find_short_pool_by_name(collection: &Collection<Pool>, _name: &str) -> Result<Pool> {
    let short_pool = find_optional_short_pool_by_name(collection, _name).await?;

    short_pool.ok_or(AppError::CustomError {
        msg: format!("no pool found with name {}", _name),
    })
}

// Return the pool information with a requested range of day for the score_by_day member
pub async fn find_pool_by_name_with_range(db: &Database, _name: &str, _from: &str) -> Result<Pool> {
    let from_date = Date::<Utc>::from_utc(NaiveDate::parse_from_str(_from, "%Y-%m-%d")?, Utc);

    let mut start_date = Date::<Utc>::from_utc(
        NaiveDate::parse_from_str(START_SEASON_DATE, "%Y-%m-%d")?,
        Utc,
    );

    let end_date =
        Date::<Utc>::from_utc(NaiveDate::parse_from_str(END_SEASON_DATE, "%Y-%m-%d")?, Utc);

    if from_date < start_date {
        return Err(AppError::CustomError {
            msg: format!(
                "from date: {} cannot be before start date: {}",
                from_date, start_date
            ),
        });
    }

    if from_date > end_date {
        return Err(AppError::CustomError {
            msg: format!(
                "from date: {} cannot be after end date: {}",
                from_date, end_date
            ),
        });
    }

    let mut projection = doc! {};

    loop {
        let str_date = start_date
            .to_string()
            .strip_suffix("UTC")
            .expect("A Date<Utc> should always be stripable with UTC.")
            .to_string();

        if str_date == *_from {
            break;
        }
        projection.insert(format!("context.score_by_day.{}", str_date), 0);
        start_date = start_date + Duration::days(1);
    }

    let find_option = FindOneOptions::builder().projection(projection).build();
    let collection = db.collection::<Pool>("pools");
    let pool = collection
        .clone_with_type::<Pool>()
        .find_one(doc! {"name": &_name}, find_option)
        .await?;

    pool.ok_or(AppError::CustomError {
        msg: format!("no pool found with name {}", _name),
    })
}

pub async fn find_pools(db: &Database) -> Result<Vec<ProjectedPoolShort>> {
    let collection = db.collection::<Pool>("pools");
    let find_option = FindOptions::builder()
        .projection(doc! {"name": 1, "owner": 1, "status": 1})
        .build();

    let cursor = collection
        .clone_with_type::<ProjectedPoolShort>()
        .find(None, find_option)
        .await?;

    let pools = cursor.try_collect().await?;

    Ok(pools)
}

pub async fn create_pool(
    db: &Database,
    _owner: String,
    _pool_info: PoolCreationRequest,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    if find_optional_short_pool_by_name(&collection, &_pool_info.name)
        .await?
        .is_some()
    {
        return Err(AppError::CustomError {
            msg: "pool name already exist.".to_string(),
        });
    }

    // Create the default Pool creation class.
    let pool = Pool {
        name: _pool_info.name,
        owner: _owner,
        assistants: Vec::new(),
        number_poolers: _pool_info.number_pooler,
        participants: None,
        number_forwards: 9,
        number_defenders: 4,
        number_goalies: 2,
        number_reservists: 2,
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
        next_season_number_players_protected: 8,
        tradable_picks: 3,
        status: PoolState::Created,
        final_rank: None,
        nb_player_drafted: 0,
        nb_trade: 0,
        trades: None,
        context: None,
        date_updated: 0,
        season_start: START_SEASON_DATE.to_string(),
        season_end: END_SEASON_DATE.to_string(),
        roster_modification_date: Vec::new(),
    };

    collection.insert_one(&pool, None).await?;

    Ok(create_success_pool_response(pool).await)
}

pub async fn delete_pool(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    has_owner_privileges(_user_id, &pool)?;

    let delete_result = collection
        .delete_one(doc! {"name": _pool_name}, None)
        .await?;

    if delete_result.deleted_count == 0 {
        return Err(AppError::CustomError {
            msg: "The pool could not be deleted.".to_string(),
        });
    }

    Ok(create_success_pool_response(pool).await)
}

pub async fn start_draft(
    db: &Database,
    _user_id: &str,
    _pool_info: &mut Pool,
) -> Result<PoolMessageResponse> {
    if let Some(participants) = &_pool_info.participants {
        if _pool_info.number_poolers != participants.len() as u8 {
            return Err(AppError::CustomError {
                msg: "The number of participants is not good.".to_string(),
            });
        }

        if !matches!(_pool_info.status, PoolState::Created) {
            return Err(AppError::CustomError {
                msg: "The pool is not in a valid state to start.".to_string(),
            });
        }

        has_owner_privileges(_user_id, &_pool_info)?;

        // TODO: Validate that the list of users provided all exist.

        // Add the new pool to the list of pool in each users.

        let collection_users = db.collection::<User>("users");
        add_pool_to_users(&collection_users, &_pool_info.name, participants).await?;

        let collection = db.collection::<Pool>("pools");

        // create pool context
        let mut pool_context = PoolContext {
            pooler_roster: HashMap::new(),
            score_by_day: Some(HashMap::new()),
            tradable_picks: Some(Vec::new()),
            past_tradable_picks: Some(Vec::new()),
            players_name_drafted: Vec::new(),
            players: HashMap::new(),
        };

        // Initialize all participants roster object.
        for participant in participants.iter() {
            let pooler_roster = PoolerRoster {
                chosen_forwards: Vec::new(),
                chosen_defenders: Vec::new(),
                chosen_goalies: Vec::new(),
                chosen_reservists: Vec::new(),
            };

            pool_context
                .pooler_roster
                .insert(participant.to_string(), pooler_roster);
        }

        // TODO: randomize the list of participants so the draft order is random
        //thread_rng().shuffle(&mut _participants);

        _pool_info.status = PoolState::Draft;
        _pool_info.context = Some(pool_context);

        // updated fields.

        let updated_fields = doc! {
            "$set": to_bson(&_pool_info)?
        };

        // Update the fields in the mongoDB pool document.

        update_pool(updated_fields, &collection, &_pool_info.name).await
    } else {
        return Err(AppError::CustomError {
            msg: "There is no participants added in the pool.".to_string(),
        });
    }
}

pub async fn select_player(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
    _player: &Player,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    let mut pool_context = get_pool_context(pool.context)?;

    if !pool_context.pooler_roster.contains_key(_user_id) {
        return Err(AppError::CustomError {
            msg: "The user is not in the pool.".to_string(),
        });
    }

    // First, validate that the player selected is not picked by any of the other poolers.
    let participants = get_participants(pool.participants)?;

    for participant in participants.iter() {
        if validate_player_possession(_player.id, &pool_context.pooler_roster[participant]).await {
            return Err(AppError::CustomError {
                msg: "This player is already picked.".to_string(),
            });
        }
    }

    let mut users_players_count =
        get_users_players_count(&pool_context.pooler_roster, &participants).await;

    let tot_players_in_roster =
        pool.number_forwards + pool.number_defenders + pool.number_goalies + pool.number_reservists;

    // validate it is the user turn.

    match pool.final_rank {
        Some(final_rank) => {
            // This is a dynastie type pool.

            let tradable_picks = pool_context
                .tradable_picks
                .as_ref()
                .expect("This member should never be optional in dynastie type pool.");

            loop {
                let players_drafted = pool_context.players_name_drafted.len();

                let index = pool.number_poolers as usize
                    - 1
                    - (players_drafted % pool.number_poolers as usize);
                let next_drafter = &final_rank[index];

                if players_drafted < (pool.tradable_picks * pool.number_poolers) as usize {
                    // use the tradable_picks to see who will draft next.

                    let real_next_drafter = &tradable_picks
                        [players_drafted / pool.number_poolers as usize][next_drafter];

                    if users_players_count[real_next_drafter] >= tot_players_in_roster {
                        pool_context.players_name_drafted.push(0); // Id 0 means the players did not draft because is roster is already full
                        continue;
                    }

                    if real_next_drafter != _user_id {
                        return Err(AppError::CustomError {
                            msg: format!("It is {}'s turn.", real_next_drafter),
                        });
                    }
                    break;
                } else {
                    // Use the final_rank to see who draft next.

                    if users_players_count[next_drafter] >= tot_players_in_roster {
                        pool_context.players_name_drafted.push(0); // Id 0 means the players did not draft because is roster is already full
                        continue;
                    }

                    if next_drafter != _user_id {
                        return Err(AppError::CustomError {
                            msg: format!("It is {}'s turn.", next_drafter),
                        });
                    }
                    break;
                }
            }
        }
        None => {
            // there is no final rank so this is the newly created draft logic.

            let players_drafted = pool_context.players_name_drafted.len();

            let index = players_drafted % pool.number_poolers as usize;
            let next_drafter = &participants[index];

            if next_drafter != _user_id {
                return Err(AppError::CustomError {
                    msg: format!("It is {}'s turn.", next_drafter),
                });
            }
        }
    }

    // Then, Add the chosen player in its right spot.
    // When there is no place in the position of the player we will add it to the reservists.

    if let Some(pooler_roster) = pool_context.pooler_roster.get_mut(_user_id) {
        let mut is_added = false;

        match _player.position {
            Position::F => {
                if (pooler_roster.chosen_forwards.len() as u8) < pool.number_forwards {
                    pooler_roster.chosen_forwards.push(_player.id);
                    is_added = true;
                }
            }
            Position::D => {
                if (pooler_roster.chosen_defenders.len() as u8) < pool.number_defenders {
                    pooler_roster.chosen_defenders.push(_player.id);
                    is_added = true;
                }
            }
            Position::G => {
                if (pooler_roster.chosen_goalies.len() as u8) < pool.number_goalies {
                    pooler_roster.chosen_goalies.push(_player.id);
                    is_added = true;
                }
            }
        }

        if !is_added {
            if (pooler_roster.chosen_reservists.len() as u8) < pool.number_reservists {
                pooler_roster.chosen_reservists.push(_player.id);
            } else {
                return Err(AppError::CustomError {
                    msg: "Not enough space for this player.".to_string(),
                });
            }
        }

        pool_context
            .players
            .insert(_player.id.to_string(), _player.clone());
        pool_context.players_name_drafted.push(_player.id);

        if let Some(nCount) = users_players_count.get_mut(_user_id) {
            *nCount += 1;
        }
    }

    // the status change to InProgress when the draft is completed.
    // The draft is completed when all participants has a complete roster.

    let mut is_done = true;

    for participant in &participants {
        if users_players_count[participant] != tot_players_in_roster {
            is_done = false;
            break; // The Draft phase is not done.
        }
    }

    let mut status = PoolState::Draft;

    // generate the list of tradable_picks for the next season

    if is_done {
        status = PoolState::InProgress;

        let mut vect = vec![];

        for _pick_round in 0..pool.tradable_picks {
            let mut round = HashMap::new();

            for participant in participants.iter() {
                round.insert(participant.clone(), participant.clone());
            }

            vect.push(round);
        }

        pool_context.tradable_picks = Some(vect);
    }

    // updated fields.

    let updated_fields = doc! {
        "$set": doc!{
            "context": to_bson(&pool_context)?,
            "status": to_bson(&status)?
        }
    };

    // Update the fields in the mongoDB pool document.

    update_pool(updated_fields, &collection, &_pool_name).await
}

pub async fn add_player(
    db: &Database,
    _user_id: &str,
    _added_to_user_id: &str,
    _pool_name: &str,
    _player: &Player,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");
    let pool = find_short_pool_by_name(&collection, _pool_name).await?;
    has_privileges(_user_id, &pool)?;

    let mut pool_context = get_pool_context(pool.context)?;

    if !pool_context.pooler_roster.contains_key(_added_to_user_id) {
        return Err(AppError::CustomError {
            msg: "The user is not in the pool.".to_string(),
        });
    }

    // First, validate that the player selected is not picked by any of the other poolers.
    for participant in get_participants(pool.participants)?.iter() {
        if validate_player_possession(_player.id, &pool_context.pooler_roster[participant]).await {
            return Err(AppError::CustomError {
                msg: "This player is already picked.".to_string(),
            });
        }
    }

    add_player_to_roster(&mut pool_context, _player.id, _added_to_user_id)?;

    pool_context
        .players
        .insert(_player.id.to_string(), _player.clone());

    // updated fields.

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster)?,
            "context.players": to_bson(&pool_context.players)?
        }
    };

    // Update the fields in the mongoDB pool document.

    update_pool(updated_fields, &collection, &_pool_name).await
}

pub async fn remove_player(
    db: &Database,
    _user_id: &str,
    _removed_to_user_id: &str,
    _pool_name: &str,
    _player_id: u32,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");
    let pool = find_short_pool_by_name(&collection, _pool_name).await?;
    has_privileges(_user_id, &pool)?;

    let mut pool_context = get_pool_context(pool.context)?;

    if !pool_context.pooler_roster.contains_key(_removed_to_user_id) {
        return Err(AppError::CustomError {
            msg: "The user is not in the pool.".to_string(),
        });
    }

    // First, validate that the player selected is not picked by any of the other poolers.
    if !validate_player_possession(_player_id, &pool_context.pooler_roster[_removed_to_user_id])
        .await
    {
        return Err(AppError::CustomError {
            msg: "This player is not own by the user.".to_string(),
        });
    }

    remove_player_from_roster(&mut pool_context, _player_id, _removed_to_user_id);

    // updated fields.

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster)?,
        }
    };

    // Update the fields in the mongoDB pool document.

    update_pool(updated_fields, &collection, &_pool_name).await
}

pub async fn create_trade(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
    _trade: &mut Trade,
) -> Result<PoolMessageResponse> {
    let trade_deadline_date =
        Local.from_utc_date(&NaiveDate::parse_from_str(TRADE_DEADLINE_DATE, "%Y-%m-%d")?);

    let today = Local::today();

    if today > trade_deadline_date {
        return Err(AppError::CustomError {
            msg: "Trade cannot be created after the trade deadline.".to_string(),
        });
    }

    let collection = db.collection::<Pool>("pools");
    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if _user_id != &_trade.proposed_by {
        has_privileges(_user_id, &pool)?;
    }

    let pool_context = get_pool_context(pool.context)?;

    // does the proposedBy and askTo field are valid

    if !pool_context.pooler_roster.contains_key(&_trade.proposed_by)
        || !pool_context.pooler_roster.contains_key(&_trade.ask_to)
    {
        return Err(AppError::CustomError {
            msg: "The users in the trade are not in the pool.".to_string(),
        });
    }

    let mut trades = get_trades(pool.trades)?;

    // Make sure that user can only have 1 active trade at a time. return an error if already one trade active in this pool. (Active trade = NEW, ACCEPTED, )

    for trade in trades.iter() {
        if (matches!(trade.status, TradeStatus::NEW)) && (trade.proposed_by == _trade.proposed_by) {
            return Err(AppError::CustomError {
                msg: "User can only have one active trade at a time.".to_string(),
            });
        }
    }

    // does the the from or to side has items in the trade ?

    if (_trade.from_items.picks.len() + _trade.from_items.players.len()) == 0
        || (_trade.to_items.picks.len() + _trade.to_items.players.len()) == 0
    {
        return Err(AppError::CustomError {
            msg: "There is no items traded on one of the 2 sides.".to_string(),
        });
    }

    // Maximum of 5 items traded on each side ?

    if (_trade.from_items.picks.len() + _trade.from_items.players.len()) > 5
        || (_trade.to_items.picks.len() + _trade.to_items.players.len()) > 5
    {
        return Err(AppError::CustomError {
            msg: "There is to much items in the trade.".to_string(),
        });
    }

    // Does the pooler really poccess the players ?

    validate_trade_possession(&_trade.from_items, &pool_context, &_trade.proposed_by).await?;
    validate_trade_possession(&_trade.to_items, &pool_context, &_trade.ask_to).await?;

    _trade.date_created = Utc::now().timestamp_millis();
    _trade.status = TradeStatus::NEW;
    _trade.id = pool.nb_trade;
    trades.push(_trade.clone());

    // Update fields with the new trade

    let updated_fields = doc! {
        "$set": doc!{
            "trades": to_bson(&trades)?,
            "nb_trade": pool.nb_trade + 1
        }
    };

    update_pool(updated_fields, &collection, &_pool_name).await
}

pub async fn cancel_trade(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
    _trade_id: u32,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.nb_trade < _trade_id {
        return Err(AppError::CustomError {
            msg: "This trade does not exist.".to_string(),
        });
    }

    let mut trades = get_trades(pool.trades)?;

    // validate that the status of the trade is NEW

    if !matches!(trades[_trade_id as usize].status, TradeStatus::NEW) {
        return Err(AppError::CustomError {
            msg: "The trade is not in a valid state to be cancelled.".to_string(),
        });
    }

    // validate only the owner can cancel a trade

    if !has_owner_rights(_user_id, &pool.owner)
        && !has_assistants_rights(_user_id, &pool.assistants)
    {
        // validate that only the one that was ask for the trade or the owner can accept it.

        if trades[_trade_id as usize].proposed_by != *_user_id {
            return Err(AppError::CustomError {
                msg: "Only the one that created the trade can cancel it.".to_string(),
            });
        }
    }

    trades[_trade_id as usize].status = TradeStatus::CANCELLED;
    // Update fields with the new trade

    let updated_fields = doc! {
        "$set": doc!{
            "trades": to_bson(&trades)?,
        }
    };

    update_pool(updated_fields, &collection, &_pool_name).await
}

pub async fn respond_trade(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
    _is_accepted: bool,
    _trade_id: u32,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.nb_trade < _trade_id {
        return Err(AppError::CustomError {
            msg: "This trade does not exist.".to_string(),
        });
    }

    let mut trades = get_trades(pool.trades)?;
    let mut pool_context = get_pool_context(pool.context)?;

    // validate that the status of the trade is NEW

    if !matches!(trades[_trade_id as usize].status, TradeStatus::NEW) {
        return Err(AppError::CustomError {
            msg: "The trade is not in a valid state to be responded.".to_string(),
        });
    }

    if !has_owner_rights(_user_id, &pool.owner)
        && !has_assistants_rights(_user_id, &pool.assistants)
    {
        // validate that only the one that was ask for the trade or the owner can accept it.

        if trades[_trade_id as usize].ask_to != *_user_id {
            return Err(AppError::CustomError {
                msg: "Only the one that was ask for the trade or the owner can accept it."
                    .to_string(),
            });
        }

        // validate that 24h have been passed since the trade was created.

        let now = Utc::now().timestamp_millis();

        if trades[_trade_id as usize].date_created + 8640000 > now {
            return Err(AppError::CustomError {
                msg: "The trade needs to be active for 24h before being able to accept it."
                    .to_string(),
            });
        }
    }

    // validate that both trade parties own their corresponding trade items

    if _is_accepted {
        trade_roster_items(&mut pool_context, &trades[_trade_id as usize]).await?;

        trades[_trade_id as usize].status = TradeStatus::ACCEPTED;
        trades[_trade_id as usize].date_accepted = Utc::now().timestamp_millis();
    } else {
        trades[_trade_id as usize].status = TradeStatus::REFUSED;
    };

    // Update fields with the new trade response

    let updated_fields = doc! {
        "$set": doc!{
            "trades": to_bson(&trades)?,
            "context.pooler_roster": to_bson(&pool_context.pooler_roster )?,
            "context.tradable_picks": to_bson(&pool_context.tradable_picks )?
        }
    };

    update_pool(updated_fields, &collection, &_pool_name).await
}

pub async fn fill_spot(
    db: &Database,
    _user_id: &str,
    _user_modified_id: &str,
    _pool_name: &str,
    _player_id: u32,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");
    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if _user_id != _user_modified_id {
        has_privileges(_user_id, &pool)?;
    }

    let mut pool_context = get_pool_context(pool.context)?;

    if !pool_context.pooler_roster.contains_key(_user_modified_id) {
        return Err(AppError::CustomError {
            msg: "The pooler is not a participant of the pool.".to_string(),
        });
    }

    let player =
        pool_context
            .players
            .get(&_player_id.to_string())
            .ok_or(AppError::CustomError {
                msg: "This player is not included in the pool.".to_string(),
            })?;

    if pool_context.pooler_roster[_user_modified_id]
        .chosen_forwards
        .contains(&player.id)
        || pool_context.pooler_roster[_user_modified_id]
            .chosen_defenders
            .contains(&player.id)
        || pool_context.pooler_roster[_user_modified_id]
            .chosen_goalies
            .contains(&player.id)
        || !pool_context.pooler_roster[_user_modified_id]
            .chosen_reservists
            .contains(&player.id)
    {
        return Err(AppError::CustomError {
            msg: "The player should only be in the reservist pooler's list.".to_string(),
        });
    }

    let mut is_added = false;

    match player.position {
        Position::F => {
            if (pool_context.pooler_roster[_user_modified_id]
                .chosen_forwards
                .len() as u8)
                < pool.number_forwards
            {
                if let Some(x) = pool_context.pooler_roster.get_mut(_user_modified_id) {
                    x.chosen_forwards.push(player.id);
                    is_added = true;
                }
            }
        }
        Position::D => {
            if (pool_context.pooler_roster[_user_modified_id]
                .chosen_defenders
                .len() as u8)
                < pool.number_defenders
            {
                if let Some(x) = pool_context.pooler_roster.get_mut(_user_modified_id) {
                    x.chosen_defenders.push(player.id);
                    is_added = true;
                }
            }
        }
        Position::G => {
            if (pool_context.pooler_roster[_user_modified_id]
                .chosen_goalies
                .len() as u8)
                < pool.number_goalies
            {
                if let Some(x) = pool_context.pooler_roster.get_mut(_user_modified_id) {
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

    if let Some(x) = pool_context.pooler_roster.get_mut(_user_modified_id) {
        x.chosen_reservists
            .retain(|playerId| playerId != &player.id);
    }
    // Update fields with the filled spot

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster)?
        }
    };

    update_pool(updated_fields, &collection, &_pool_name).await
}

pub async fn undo_select_player(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;
    has_owner_privileges(_user_id, &pool)?;
    let mut pool_context = get_pool_context(pool.context)?;

    // validate that the pool is into the draft status.

    if !matches!(pool.status, PoolState::Draft) {
        return Err(AppError::CustomError {
            msg: "The pool must be into the draft status to perform an undo.".to_string(),
        });
    }

    // validate there is something to undo.

    if pool_context.players_name_drafted.is_empty() {
        return Err(AppError::CustomError {
            msg: "There is nothing to undo".to_string(),
        });
    }

    let mut latest_pick;

    loop {
        match pool_context.players_name_drafted.pop() {
            Some(player_id) => {
                if player_id > 0 {
                    latest_pick = player_id; // found the last drafted player.
                    break;
                }
            }
            None => {
                return Err(AppError::CustomError {
                    msg: "The is nothing to undo.".to_string(),
                })
            }
        }
    }

    let pick_number = pool_context.players_name_drafted.len();
    let latest_drafter;

    match pool.final_rank {
        Some(final_rank) => {
            // This comes from a Dynastie draft.

            let tradable_picks = pool_context
                .tradable_picks
                .as_ref()
                .expect("This member should never be optional in dynastie type pool.");

            let index =
                pool.number_poolers as usize - 1 - (pick_number % pool.number_poolers as usize);

            let next_drafter = &final_rank[index];

            if pick_number < (pool.tradable_picks * pool.number_poolers) as usize {
                // use the tradable_picks to see who will draft next.

                latest_drafter = tradable_picks[pick_number / pool.number_poolers as usize]
                    [next_drafter]
                    .clone();
            } else {
                // Use the final_rank to see who draft next.
                latest_drafter = next_drafter.clone();
            }
        }
        None => {
            // this comes from a newly created draft.

            let participants = get_participants(pool.participants)?; // the participants is used to see who picks

            let index = pick_number % pool.number_poolers as usize;
            latest_drafter = participants[index].clone();
        }
    }

    // Remove the player from the player roster.

    remove_player_from_roster(&mut pool_context, latest_pick, &latest_drafter)?;
    pool_context.players.remove(&latest_pick.to_string()); // Also remove the player from the pool players list.

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster)?,
            "context.players_name_drafted": to_bson(&pool_context.players_name_drafted)?,
        }
    };

    // Update the fields in the mongoDB pool document.

    update_pool(updated_fields, &collection, &pool.name).await
}

pub async fn modify_roster(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
    _user_modified_id: &str,
    _forw_selected: &Vec<u32>,
    _def_selected: &Vec<u32>,
    _goal_selected: &Vec<u32>,
    _reserv_selected: &Vec<u32>,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");
    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if _user_id != _user_modified_id {
        has_privileges(_user_id, &pool)?;
    }

    let start_season_date =
        Local.from_utc_date(&NaiveDate::parse_from_str(START_SEASON_DATE, "%Y-%m-%d")?);
    let end_season_date =
        Local.from_utc_date(&NaiveDate::parse_from_str(END_SEASON_DATE, "%Y-%m-%d")?);

    let mut today = Local::today();

    let time = Local::now();

    // At 12PM we start to count the action for the next day.

    if time.hour() >= 12 {
        today = today + Duration::days(1);
    }

    if today >= start_season_date && today <= end_season_date {
        let mut bAllowed = false;

        for DATE in &pool.roster_modification_date {
            let sathurday = Local.from_utc_date(&NaiveDate::parse_from_str(DATE, "%Y-%m-%d")?);

            if sathurday == today {
                bAllowed = true;
                break;
            }
        }

        if !bAllowed {
            return Err(AppError::CustomError {
                msg: "You are not allowed to modify your roster today.".to_string(),
            });
        }
    }

    let mut pool_context = get_pool_context(pool.context)?;

    if !pool_context.pooler_roster.contains_key(_user_modified_id) {
        return Err(AppError::CustomError {
            msg: "User is not in the pool.".to_string(),
        });
    }

    // Validate the total amount of forwards selected

    if _forw_selected.len() != pool.number_forwards as usize {
        return Err(AppError::CustomError {
            msg: "The amount of forwards selected is not valid".to_string(),
        });
    }

    // Validate the total amount of defenders selected

    if _def_selected.len() != pool.number_defenders as usize {
        return Err(AppError::CustomError {
            msg: "The amount of defenders selected is not valid".to_string(),
        });
    }

    // Validate the total amount of goalies selected

    if _goal_selected.len() != pool.number_goalies as usize {
        return Err(AppError::CustomError {
            msg: "The amount of goalies selected is not valid".to_string(),
        });
    }

    // Validate the total amount of players selected (It should be the same as before)

    if let Some(roster) = pool_context.pooler_roster.get(_user_modified_id) {
        let amount_selected_players = _forw_selected.len()
            + _def_selected.len()
            + _goal_selected.len()
            + _reserv_selected.len();

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

    let mut selected_player_map = HashMap::<u32, bool>::new(); // used to validate dupplication

    // Validate that the roster modification does not contains Dupplication and also validate that the user possess those players.

    for player_id in _forw_selected.iter().chain(
        _def_selected
            .iter()
            .chain(_goal_selected.iter())
            .chain(_reserv_selected.iter()),
    ) {
        let player =
            pool_context
                .players
                .get(&player_id.to_string())
                .ok_or(AppError::CustomError {
                    msg: "This player is not included in this pool".to_string(),
                })?;

        if selected_player_map.contains_key(&player.id) {
            return Err(AppError::CustomError {
                msg: format!("The player {} was dupplicated", player.name),
            });
        }
        selected_player_map.insert(player.id, true);
        if !validate_player_possession(player.id, &pool_context.pooler_roster[_user_modified_id])
            .await
        {
            return Err(AppError::CustomError {
                msg: format!("You do not possess {}.", player.name),
            });
        }
    }

    if let Some(roster) = pool_context.pooler_roster.get_mut(_user_modified_id) {
        roster.chosen_forwards = _forw_selected.clone();
        roster.chosen_defenders = _def_selected.clone();
        roster.chosen_goalies = _goal_selected.clone();
        roster.chosen_reservists = _reserv_selected.clone();
    }

    // Modify the all the pooler_roster (we could update only the pooler_roster[userId] if necessary)

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster)?,
        }
    };

    update_pool(updated_fields, &collection, &pool.name).await
}

pub async fn protect_players(
    db: &Database,
    _user_id: &str,
    _pool_name: &str,
    _forw_protected: &Vec<u32>,
    _def_protected: &Vec<u32>,
    _goal_protected: &Vec<u32>,
    _reserv_protected: &Vec<u32>,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");
    let pool = find_short_pool_by_name(&collection, _pool_name).await?;
    let mut pool_context = get_pool_context(pool.context)?;

    // make sure the user making the resquest is a pool participants.

    if !pool_context.pooler_roster.contains_key(_user_id) {
        return Err(AppError::CustomError {
            msg: "The pooler is not a participant of the pool.".to_string(),
        });
    }

    // validate that the numbers of players protected is ok.

    if (_forw_protected.len() as u8) > pool.number_forwards {
        return Err(AppError::CustomError {
            msg: "To much forwards protected".to_string(),
        });
    }

    if (_def_protected.len() as u8) > pool.number_defenders {
        return Err(AppError::CustomError {
            msg: "To much defenders protected".to_string(),
        });
    }

    if (_goal_protected.len() as u8) > pool.number_goalies {
        return Err(AppError::CustomError {
            msg: "To much goalies protected".to_string(),
        });
    }

    if (_reserv_protected.len() as u8) > pool.number_reservists {
        return Err(AppError::CustomError {
            msg: "To much reservists protected".to_string(),
        });
    }

    let tot_player_protected = _forw_protected.len()
        + _def_protected.len()
        + _goal_protected.len()
        + _reserv_protected.len();

    if tot_player_protected as u8 != pool.next_season_number_players_protected {
        return Err(AppError::CustomError {
            msg: "The number of selected players is not valid".to_string(),
        });
    }

    // Validate that the players protection list does not contains dupplication and also validate that the user possess those players.

    let mut selected_player_map = HashMap::<u32, bool>::new(); // used to validate dupplication

    for player_id in _forw_protected.iter().chain(
        _def_protected
            .iter()
            .chain(_goal_protected.iter())
            .chain(_reserv_protected.iter()),
    ) {
        let player =
            pool_context
                .players
                .get(&player_id.to_string())
                .ok_or(AppError::CustomError {
                    msg: "This player is not included in this pool".to_string(),
                })?;

        if selected_player_map.contains_key(&player.id) {
            return Err(AppError::CustomError {
                msg: format!("The player {} was dupplicated", player.name),
            });
        }
        selected_player_map.insert(player.id, true);
        if !validate_player_possession(player.id, &pool_context.pooler_roster[_user_id]).await {
            return Err(AppError::CustomError {
                msg: format!("You do not possess {}.", player.name),
            });
        }
    }

    // clear previous season roster and add those players list to the new roster.

    if let Some(roster) = pool_context.pooler_roster.get_mut(_user_id) {
        roster.chosen_forwards = _forw_protected.clone();
        roster.chosen_defenders = _def_protected.clone();
        roster.chosen_goalies = _goal_protected.clone();
        roster.chosen_reservists = _reserv_protected.clone();
    }

    // Look if all participants have protected their players

    let participants = get_participants(pool.participants)?;
    let mut is_done = true;

    let users_players_count =
        get_users_players_count(&pool_context.pooler_roster, &participants).await;

    for participant in participants.iter() {
        if users_players_count[participant] != pool.next_season_number_players_protected {
            is_done = false; // not all participants are ready
            break;
        }
    }

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster)?,
            //"context.score_by_day": Some(), // TODO: clear this field since it is not usefull for the new season.
            "status": if is_done {to_bson(&PoolState::Draft)?} else {to_bson(&PoolState::Dynastie)?}
        }
    };

    // Update the fields in the mongoDB pool document.

    update_pool(updated_fields, &collection, &pool.name).await
}

pub async fn update_pool_settings(
    db: &Database,
    _user_id: &str,
    _update: &UpdatePoolSettingsRequest,
) -> Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, &_update.name).await?;
    has_privileges(_user_id, &pool)?;

    // Update the fields in the mongoDB pool document.
    if (_update.pool_settings.number_forwards.is_some()
        || _update.pool_settings.number_defenders.is_some()
        || _update.pool_settings.number_goalies.is_some()
        || _update.pool_settings.number_reservists.is_some()
        || _update
            .pool_settings
            .next_season_number_players_protected
            .is_some()
        || _update.pool_settings.tradable_picks.is_some())
        && matches!(pool.status, PoolState::InProgress)
    {
        return Err(AppError::CustomError {
            msg: "These settings cannot be updated while the pool is in progress.".to_string(),
        }); // Need to make this robust, potentially need another pool status
    }

    let updated_fields = doc! {
        "$set": doc!{
            "number_forwards": to_bson(&_update.pool_settings.number_forwards.unwrap_or(pool.number_forwards))?,
            "number_defenders": to_bson(&_update.pool_settings.number_defenders.unwrap_or(pool.number_defenders))?,
            "number_goalies": to_bson(&_update.pool_settings.number_goalies.unwrap_or(pool.number_goalies))?,
            "number_reservists": to_bson(&_update.pool_settings.number_reservists.unwrap_or(pool.number_reservists))?,
            "next_season_number_players_protected": to_bson(&_update.pool_settings.next_season_number_players_protected.unwrap_or(pool.next_season_number_players_protected))?,
            "tradable_picks": to_bson(&_update.pool_settings.tradable_picks.unwrap_or(pool.tradable_picks))?,
            //Points per forwards
            "forward_pts_goals": to_bson(&_update.pool_settings.forward_pts_goals.unwrap_or(pool.forward_pts_goals))?,
            "forward_pts_assists": to_bson(&_update.pool_settings.forward_pts_assists.unwrap_or(pool.forward_pts_assists))?,
            "forward_pts_hattricks": to_bson(&_update.pool_settings.forward_pts_hattricks.unwrap_or(pool.forward_pts_hattricks))?,
            "forward_pts_shootout_goals": to_bson(&_update.pool_settings.forward_pts_shootout_goals.unwrap_or(pool.forward_pts_shootout_goals))?,
            //Points per Defenders
            "defender_pts_goals": to_bson(&_update.pool_settings.defender_pts_goals.unwrap_or(pool.defender_pts_goals))?,
            "defender_pts_assists": to_bson(&_update.pool_settings.defender_pts_assists.unwrap_or(pool.defender_pts_assists))?,
            "defender_pts_hattricks": to_bson(&_update.pool_settings.defender_pts_hattricks.unwrap_or(pool.defender_pts_hattricks))?,
            "defender_pts_shootout_goals": to_bson(&_update.pool_settings.defender_pts_shootout_goals.unwrap_or(pool.defender_pts_shootout_goals))?,
            //Points per Goalies
            "goalies_pts_wins": to_bson(&_update.pool_settings.goalies_pts_wins.unwrap_or(pool.goalies_pts_wins))?,
            "goalies_pts_shutouts": to_bson(&_update.pool_settings.goalies_pts_shutouts.unwrap_or(pool.goalies_pts_shutouts))?,
            "goalies_pts_overtimes": to_bson(&_update.pool_settings.goalies_pts_overtimes.unwrap_or(pool.goalies_pts_overtimes))?,
            "goalies_pts_goals": to_bson(&_update.pool_settings.goalies_pts_goals.unwrap_or(pool.goalies_pts_goals))?,
            "goalies_pts_assists": to_bson(&_update.pool_settings.goalies_pts_assists.unwrap_or(pool.goalies_pts_assists))?,

        }
    };

    update_pool(updated_fields, &collection, &_update.name).await
}

async fn trade_roster_items(_pool_context: &mut PoolContext, _trade: &Trade) -> Result<()> {
    validate_trade_possession(&_trade.from_items, _pool_context, &_trade.proposed_by).await?;
    validate_trade_possession(&_trade.from_items, _pool_context, &_trade.proposed_by).await?;

    // Migrate players "from" -> "to"
    for player_id in _trade.from_items.players.iter() {
        trade_roster_player(
            _pool_context,
            *player_id,
            &_trade.proposed_by,
            &_trade.ask_to,
        )
        .await?;
    }

    // Migrate players "to" -> "from"
    for player_id in _trade.to_items.players.iter() {
        trade_roster_player(
            _pool_context,
            *player_id,
            &_trade.ask_to,
            &_trade.proposed_by,
        )
        .await?;
    }

    // Migrate picks "from" -> "to"
    for pick in _trade.from_items.picks.iter() {
        if let Some(tradable_picks) = &mut _pool_context.tradable_picks {
            if let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from) {
                *owner = _trade.ask_to.clone();
                println!("From: {}", _trade.ask_to);
            }
        }
    }

    // Migrate picks "to" -> "from"
    for pick in _trade.to_items.picks.iter() {
        if let Some(tradable_picks) = &mut _pool_context.tradable_picks {
            if let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from) {
                *owner = _trade.proposed_by.clone();
                println!("To: {}", _trade.proposed_by)
            }
        }
    }

    Ok(())
}

fn remove_player_from_roster(
    _pool_context: &mut PoolContext,
    _player_id: u32,
    _user_id: &str,
) -> Result<()> {
    if let Some(roster) = _pool_context.pooler_roster.get_mut(_user_id) {
        if _remove_player_from_roster(&mut roster.chosen_forwards, _player_id) {
            return Ok(());
        };
        if _remove_player_from_roster(&mut roster.chosen_defenders, _player_id) {
            return Ok(());
        };
        if _remove_player_from_roster(&mut roster.chosen_goalies, _player_id) {
            return Ok(());
        };
        if _remove_player_from_roster(&mut roster.chosen_reservists, _player_id) {
            return Ok(());
        };
    }

    Err(AppError::CustomError {
        msg: "The player could not be removed".to_string(),
    }) // could not be removed
}

fn _remove_player_from_roster(_chosen_players: &mut Vec<u32>, _player_id: u32) -> bool {
    _chosen_players
        .iter()
        .position(|player_id| player_id == &_player_id)
        .map(|index| _chosen_players.remove(index))
        .is_some()
}

fn add_player_to_roster(
    _pool_context: &mut PoolContext,
    _player_id: u32,
    _user_id: &str,
) -> Result<()> {
    if let Some(roster) = _pool_context.pooler_roster.get_mut(_user_id) {
        roster.chosen_reservists.push(_player_id);
        return Ok(());
    }

    Err(AppError::CustomError {
        msg: "The player could not be added".to_string(),
    }) // could not be added
}

async fn trade_roster_player(
    _pool_context: &mut PoolContext,
    _player_id: u32,
    _user_giver: &str,
    _user_receiver: &str,
) -> Result<()> {
    remove_player_from_roster(_pool_context, _player_id, _user_giver)?;

    // Add the player to the receiver's reservists.

    add_player_to_roster(_pool_context, _player_id, _user_receiver)
}

async fn validate_trade_possession(
    _trading_list: &TradeItems,
    _pool_context: &PoolContext,
    _user: &str,
) -> Result<()> {
    for player_id in _trading_list.players.iter() {
        if !validate_player_possession(*player_id, &_pool_context.pooler_roster[_user]).await {
            return Err(AppError::CustomError {
                msg: "ther user does not possess one of the traded player!".to_string(),
            });
        }
    }

    if let Some(tradable_picks) = &_pool_context.tradable_picks {
        for pick in _trading_list.picks.iter() {
            if tradable_picks[pick.round as usize][&pick.from] != *_user {
                return Err(AppError::CustomError {
                    msg: "ther user does not possess the traded pick!".to_string(),
                });
            }
        }
    }

    Ok(())
}

// return a hash map of number of players for each participant in a pool.

async fn get_users_players_count(
    _pooler_roster: &HashMap<String, PoolerRoster>,
    _participants: &Vec<String>,
) -> HashMap<String, u8> {
    let mut hashCount = HashMap::new();

    for participant in _participants {
        let nb_players = (_pooler_roster[participant].chosen_forwards.len()
            + _pooler_roster[participant].chosen_defenders.len()
            + _pooler_roster[participant].chosen_goalies.len()
            + _pooler_roster[participant].chosen_reservists.len()) as u8;

        hashCount.insert(participant.clone(), nb_players);
    }

    hashCount
}

async fn validate_player_possession(_player_id: u32, _pooler_roster: &PoolerRoster) -> bool {
    _pooler_roster.chosen_forwards.contains(&_player_id)
        || _pooler_roster.chosen_defenders.contains(&_player_id)
        || _pooler_roster.chosen_goalies.contains(&_player_id)
        || _pooler_roster.chosen_reservists.contains(&_player_id)
}

async fn create_success_pool_response(_pool: Pool) -> PoolMessageResponse {
    PoolMessageResponse {
        success: true,
        message: "".to_string(),
        pool: _pool,
    }
}

fn has_assistants_rights(_user_id: &str, _assistants: &Vec<String>) -> bool {
    _assistants.contains(&_user_id.to_string())
}

fn has_owner_rights(_user_id: &str, _owner: &str) -> bool {
    _user_id == _owner
}

fn has_privileges(_user_id: &str, _pool: &Pool) -> Result<()> {
    if !has_assistants_rights(_user_id, &_pool.assistants)
        && !has_owner_rights(_user_id, &_pool.owner)
    {
        return Err(AppError::CustomError {
            msg: "This action require privileged rights.".to_string(),
        });
    }

    Ok(())
}

fn has_owner_privileges(_user_id: &str, _pool: &Pool) -> Result<()> {
    if !has_owner_rights(_user_id, &_pool.owner) {
        return Err(AppError::CustomError {
            msg: "This action require privileged rights.".to_string(),
        });
    }

    Ok(())
}

fn get_pool_context(_pool_context: Option<PoolContext>) -> Result<PoolContext> {
    match _pool_context {
        Some(pool_context) => Ok(pool_context),
        None => Err(AppError::CustomError {
            msg: "There is no context to that pool yet.".to_string(),
        }),
    }
}

fn get_participants(_participants: Option<Vec<String>>) -> Result<Vec<String>> {
    match _participants {
        Some(participants) => Ok(participants),
        None => Err(AppError::CustomError {
            msg: "There is no participants in the pool.".to_string(),
        }),
    }
}

fn get_trades(_trades: Option<Vec<Trade>>) -> Result<Vec<Trade>> {
    match _trades {
        Some(_trades) => Ok(_trades),
        None => Err(AppError::CustomError {
            msg: "Trade are not activated in the pool settings.".to_string(),
        }),
    }
}

async fn update_pool(
    _updated_field: Document,
    _collection: &Collection<Pool>,
    _pool_name: &str,
) -> Result<PoolMessageResponse> {
    // Update the fields in the mongoDB pool document.
    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .projection(doc! {"context.score_by_day": 0})
        .build();

    match _collection
        .find_one_and_update(
            doc! {"name": _pool_name},
            _updated_field,
            find_one_and_update_options,
        )
        .await?
    {
        Some(updated_pool) => Ok(create_success_pool_response(updated_pool).await),
        None => Err(AppError::CustomError {
            msg: "The pool could not be updated.".to_string(),
        }),
    }
}
