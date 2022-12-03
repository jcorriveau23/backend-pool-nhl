use chrono::{Date, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, to_bson};
use mongodb::options::{FindOneAndUpdateOptions, FindOneOptions, FindOptions, ReturnDocument};
use mongodb::{Collection, Database};
use std::collections::HashMap;

use crate::models::pool::{
    Player, Pool, PoolContext, PoolCreationRequest, PoolState, PoolerRoster, Position,
    ProjectedPoolShort, Trade, TradeItems, TradeStatus,
};

use crate::db::user::add_pool_to_users;
use crate::models::user::User;

use crate::models::response::PoolMessageResponse;

// Date for season

const START_PRE_SEASON_DATE: &str = "2022-09-24";
const START_SEASON_DATE: &str = "2022-10-07";
const END_SEASON_DATE: &str = "2023-04-13";

const TRADE_DEADLINE_DATE: &str = "2023-03-03";

const FIRST_SATHURDAY_OF_MONTHS: [&str; 5] = [
    "2022-11-05",
    "2022-12-03",
    "2023-01-07",
    "2023-02-04",
    "2023-03-04",
];

// Return the complete Pool information
pub async fn find_pool_by_name(
    db: &Database,
    _name: &String,
) -> mongodb::error::Result<Option<Pool>> {
    let collection = db.collection::<Pool>("pools");

    collection.find_one(doc! {"name": _name}, None).await
}

// Return the pool information without the score_by_day member
pub async fn find_short_pool_by_name(
    collection: &Collection<Pool>,
    _name: &String,
) -> mongodb::error::Result<Option<Pool>> {
    let find_option = FindOneOptions::builder()
        .projection(doc! {"context.score_by_day": 0})
        .build();

    collection
        .clone_with_type::<Pool>()
        .find_one(doc! {"name": &_name}, find_option)
        .await
}

// Return the pool information with a requested range of day for the score_by_day member
pub async fn find_pool_by_name_with_range(
    db: &Database,
    _name: &String,
    _from: &String,
) -> mongodb::error::Result<Option<Pool>> {
    let from_date =
        Date::<Utc>::from_utc(NaiveDate::parse_from_str(_from, "%Y-%m-%d").unwrap(), Utc);

    let mut start_date = Date::<Utc>::from_utc(
        NaiveDate::parse_from_str(START_SEASON_DATE, "%Y-%m-%d").unwrap(),
        Utc,
    );

    let end_date = Date::<Utc>::from_utc(
        NaiveDate::parse_from_str(END_SEASON_DATE, "%Y-%m-%d").unwrap(),
        Utc,
    );

    if from_date < start_date || from_date > end_date {
        return Ok(None); // error.
    }

    let mut projection = doc! {};

    loop {
        let str_date = start_date
            .to_string()
            .strip_suffix("UTC")
            .unwrap()
            .to_string();
        // println!("{}", str_date);

        if str_date == *_from {
            break;
        }
        projection.insert(format!("context.score_by_day.{}", str_date), 0);
        start_date = start_date + Duration::days(1);
    }

    // println!("{}", projection);

    let find_option = FindOneOptions::builder().projection(projection).build();
    let collection = db.collection::<Pool>("pools");
    collection
        .clone_with_type::<Pool>()
        .find_one(doc! {"name": &_name}, find_option)
        .await
}

pub async fn find_pools(db: &Database) -> mongodb::error::Result<Vec<ProjectedPoolShort>> {
    let collection = db.collection::<Pool>("pools");
    let find_option = FindOptions::builder()
        .projection(doc! {"name": 1, "owner": 1, "status": 1})
        .build();

    let mut cursor = collection
        .clone_with_type::<ProjectedPoolShort>()
        .find(None, find_option)
        .await?;

    let mut pools = vec![];

    while let Some(pool) = cursor.try_next().await? {
        pools.push(pool);
    }

    Ok(pools)
}

pub async fn create_pool(
    db: &Database,
    _owner: String,
    _pool_info: PoolCreationRequest,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    if find_short_pool_by_name(&collection, &_pool_info.name)
        .await?
        .is_some()
    {
        return Ok(create_error_response("pool name already exist.".to_string()).await);
    } else {
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
        };

        collection.insert_one(pool, None).await?;

        Ok(create_success_response(&None).await)
    }
}

pub async fn delete_pool(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, &_pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    } else if pool.unwrap().owner != *_user_id {
        return Ok(create_error_response(
            "Only the owner of the pool can delete the pool.".to_string(),
        )
        .await);
    } else {
        let delete_result = collection
            .delete_one(doc! {"name": _pool_name.clone()}, None)
            .await?;

        if delete_result.deleted_count == 0 {
            return Ok(create_error_response("The pool could not be deleted.".to_string()).await);
        } else {
            Ok(create_success_response(&None).await)
        }
    }
}

pub async fn start_draft(
    db: &Database,
    _user_id: &String,
    _poolInfo: &mut Pool,
) -> mongodb::error::Result<PoolMessageResponse> {
    if let Some(participants) = &_poolInfo.participants {
        if _poolInfo.number_poolers != participants.len() as u8 {
            return Ok(create_error_response(
                "The number of participants is not good.".to_string(),
            )
            .await);
        }

        if !matches!(_poolInfo.status, PoolState::Created) {
            return Ok(create_error_response(
                "The pool is not in a valid state to start.".to_string(),
            )
            .await);
        }

        if _poolInfo.owner != *_user_id {
            return Ok(create_error_response(
                "Only the owner of the pool can start the draft.".to_string(),
            )
            .await);
        }

        // TODO: Validate that the list of users provided all exist.

        // Add the new pool to the list of pool in each users.

        let collection_users = db.collection::<User>("users");
        add_pool_to_users(&collection_users, &_poolInfo.name, participants).await;

        let collection = db.collection::<Pool>("pools");

        // create pool context
        let mut pool_context = PoolContext {
            pooler_roster: HashMap::new(),
            score_by_day: Some(HashMap::new()),
            tradable_picks: Some(Vec::new()),
            past_tradable_picks: Some(Vec::new()),
            players_name_drafted: Vec::new(),
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

        _poolInfo.status = PoolState::Draft;
        _poolInfo.context = Some(pool_context);

        // updated fields.

        let updated_fields = doc! {
            "$set": to_bson(&_poolInfo).unwrap()
        };

        // Update the fields in the mongoDB pool document.
        let find_one_and_update_options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let new_pool = collection
            .find_one_and_update(
                doc! {"name": &_poolInfo.name},
                updated_fields,
                find_one_and_update_options,
            )
            .await
            .unwrap();

        if new_pool.is_none() {
            return Ok(create_error_response("The pool could not be updated.".to_string()).await);
        }

        Ok(create_success_response(&new_pool).await)
    } else {
        return Ok(create_error_response(
            "There is no participants added in the pool.".to_string(),
        )
        .await);
    }
}

// Dynastie:
// Start draft: final_rank would be empty

pub async fn select_player(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _player: &Player,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();

    if pool_unwrap.context.is_none() {
        return Ok(
            create_error_response("There is no context to that pool yet.".to_string()).await,
        );
    }

    let mut pool_context = pool_unwrap.context.unwrap();

    if !pool_context.pooler_roster.contains_key(_user_id) {
        return Ok(create_error_response("The user is not in the pool.".to_string()).await);
    }

    if pool_unwrap.participants.is_none() {
        return Ok(
            create_error_response("There is no participants in that pool yet.".to_string()).await,
        );
    }

    // First, validate that the player selected is not picked by any of the other poolers.
    let participants = pool_unwrap.participants.clone().unwrap();

    for participant in participants.iter() {
        if participant == _user_id {
            continue;
        }
        if validate_player_possession(_player, &pool_context.pooler_roster[participant]).await {
            return Ok(create_error_response("This player is already picked.".to_string()).await);
        }
    }

    let mut users_players_count =
        get_users_players_count(&pool_context.pooler_roster, &participants).await;

    let tot_players_in_roster = pool_unwrap.number_forwards
        + pool_unwrap.number_defenders
        + pool_unwrap.number_goalies
        + pool_unwrap.number_reservists;

    // validate it is the user turn.
    if pool_unwrap.final_rank.is_some() {
        // This comes from a Dynastie draft.

        let final_rank = pool_unwrap.final_rank.clone().unwrap(); // the final rank is used to see who picks
        let tradable_picks = pool_context.tradable_picks.clone().unwrap();

        loop {
            let players_drafted = pool_context.players_name_drafted.len();

            let index = pool_unwrap.number_poolers as usize
                - 1
                - (players_drafted % pool_unwrap.number_poolers as usize);
            let next_drafter = &final_rank[index];

            if players_drafted < (pool_unwrap.tradable_picks * pool_unwrap.number_poolers) as usize
            {
                // use the tradable_picks to see who will draft next.

                let real_next_drafter = &tradable_picks
                    [players_drafted / pool_unwrap.number_poolers as usize][next_drafter];

                if users_players_count[real_next_drafter] >= tot_players_in_roster {
                    pool_context.players_name_drafted.push(0); // Id 0 means the players did not draft because is roster is already full
                    continue;
                }

                if real_next_drafter != _user_id {
                    return Ok(create_error_response(format!(
                        "It is {}'s turn.",
                        real_next_drafter
                    ))
                    .await);
                }
                break;
            } else {
                // Use the final_rank to see who draft next.

                if users_players_count[next_drafter] >= tot_players_in_roster {
                    pool_context.players_name_drafted.push(0); // Id 0 means the players did not draft because is roster is already full
                    continue;
                }

                if next_drafter != _user_id {
                    return Ok(
                        create_error_response(format!("It is {}'s turn.", next_drafter)).await,
                    );
                }
                break;
            }
        }
    } else {
        // this comes from a new draft.

        let participants = pool_unwrap.participants.unwrap(); // the participants is used to see who picks

        let players_drafted = pool_context.players_name_drafted.len();

        let index = players_drafted % pool_unwrap.number_poolers as usize;
        let next_drafter = &participants[index];

        if next_drafter != _user_id {
            return Ok(create_error_response(format!("It is {}'s turn.", next_drafter)).await);
        }
    }

    // Then, Add the chosen player in its right spot.
    // When there is no place in the position of the player we will add it to the reservists.

    if let Some(pooler_roster) = pool_context.pooler_roster.get_mut(_user_id) {
        let mut is_added = false;

        match _player.position {
            Position::F => {
                if (pooler_roster.chosen_forwards.len() as u8) < pool_unwrap.number_forwards {
                    pooler_roster.chosen_forwards.push(_player.clone());
                    is_added = true;
                }
            }
            Position::D => {
                if (pooler_roster.chosen_defenders.len() as u8) < pool_unwrap.number_defenders {
                    pooler_roster.chosen_defenders.push(_player.clone());
                    is_added = true;
                }
            }
            Position::G => {
                if (pooler_roster.chosen_goalies.len() as u8) < pool_unwrap.number_goalies {
                    pooler_roster.chosen_goalies.push(_player.clone());
                    is_added = true;
                }
            }
        }

        if !is_added {
            if (pooler_roster.chosen_reservists.len() as u8) < pool_unwrap.number_reservists {
                pooler_roster.chosen_reservists.push(_player.clone());
            } else {
                return Ok(
                    create_error_response("Not enough space for this player.".to_string()).await,
                );
            }
        }

        pool_context.players_name_drafted.push(_player.id.clone());

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

        for _pick_round in 0..pool_unwrap.tradable_picks {
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
            "context": to_bson(&pool_context).unwrap(),
            "status": to_bson(&status).unwrap()
        }
    };

    // Update the fields in the mongoDB pool document.
    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let new_pool = collection
        .find_one_and_update(
            doc! {"name": pool_unwrap.name},
            updated_fields,
            find_one_and_update_options,
        )
        .await
        .unwrap();

    if new_pool.is_none() {
        return Ok(create_error_response("The pool could not be updated.".to_string()).await);
    }

    Ok(create_success_response(&new_pool).await)
}

pub async fn create_trade(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _trade: &mut Trade,
) -> mongodb::error::Result<PoolMessageResponse> {
    let trade_deadline_date = Date::<Utc>::from_utc(
        NaiveDate::parse_from_str(TRADE_DEADLINE_DATE, "%Y-%m-%d").unwrap(),
        Utc,
    );

    let today = Utc::today();

    if today > trade_deadline_date {
        return Ok(create_error_response(
            "Trade cannot be created after the trade deadline.".to_string(),
        )
        .await);
    }

    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let mut pool_unwrap = pool.unwrap();

    if pool_unwrap.context.is_none() {
        return Ok(
            create_error_response("There is no context to that pool yet.".to_string()).await,
        );
    }

    let pool_context = pool_unwrap.context.unwrap();

    // does the proposedBy and askTo field are valid

    if !pool_context.pooler_roster.contains_key(&_trade.proposed_by)
        || !pool_context.pooler_roster.contains_key(&_trade.ask_to)
    {
        return Ok(create_error_response(
            "The users in the trade are not in the pool.".to_string(),
        )
        .await);
    }

    if pool_unwrap.trades.is_none() {
        pool_unwrap.trades = Some(Vec::new());
    }

    let mut trades = pool_unwrap.trades.unwrap().clone();

    // Make sure that user can only have 1 active trade at a time. return an error if already one trade active in this pool. (Active trade = NEW, ACCEPTED, )

    for trade in trades.iter() {
        if (matches!(trade.status, TradeStatus::NEW)) && (trade.proposed_by == *_user_id) {
            return Ok(create_error_response(
                "User can only have one active trade at a time.".to_string(),
            )
            .await);
        }
    }

    // does the the from or to side has items in the trade ?

    if (_trade.from_items.picks.len() + _trade.from_items.players.len()) == 0
        || (_trade.to_items.picks.len() + _trade.to_items.players.len()) == 0
    {
        return Ok(create_error_response(
            "There is no items traded on one of the 2 sides.".to_string(),
        )
        .await);
    }

    // Maximum of 5 items traded on each side ?

    if (_trade.from_items.picks.len() + _trade.from_items.players.len()) > 5
        || (_trade.to_items.picks.len() + _trade.to_items.players.len()) > 5
    {
        return Ok(create_error_response("There is to much items in the trade.".to_string()).await);
    }

    // Does the pooler really poccess the players ?

    if !validate_trade_possession(&_trade.from_items, &pool_context, &_trade.proposed_by).await
        || !validate_trade_possession(&_trade.to_items, &pool_context, &_trade.ask_to).await
    {
        return Ok(create_error_response(
            "One of the to pooler does not poccess the items list provided for the trade."
                .to_string(),
        )
        .await);
    }

    _trade.date_created = Utc::now().timestamp_millis();
    _trade.status = TradeStatus::NEW;
    _trade.id = pool_unwrap.nb_trade;
    trades.push(_trade.clone());

    // Update fields with the new trade

    let updated_fields = doc! {
        "$set": doc!{
            "trades": to_bson(&trades).unwrap(),
            "nb_trade": pool_unwrap.nb_trade + 1
        }
    };

    match collection
        .update_one(doc! {"name": _pool_name}, updated_fields, None)
        .await
    {
        Ok(res) => {
            println!("{:?}", res);
            return Ok(create_success_response(&None).await);
        }
        Err(e) => {
            println!("{}", e);
            return Ok(create_error_response("The pool could not be updated.".to_string()).await);
        }
    }
}

pub async fn cancel_trade(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _trade_id: u32,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();

    if pool_unwrap.nb_trade < _trade_id {
        return Ok(create_error_response("This trade does not exist.".to_string()).await);
    }

    let mut trades = pool_unwrap.trades.unwrap().clone();

    // validate that the status of the trade is NEW

    if !matches!(trades[_trade_id as usize].status, TradeStatus::NEW) {
        return Ok(create_error_response(
            "The trade is not in a valid state to be cancelled.".to_string(),
        )
        .await);
    }

    // validate only the owner can cancel a trade

    if trades[_trade_id as usize].proposed_by != *_user_id {
        return Ok(create_error_response(
            "Only the one that created the trade can cancel it.".to_string(),
        )
        .await);
    }

    trades[_trade_id as usize].status = TradeStatus::CANCELLED;
    // Update fields with the new trade

    let updated_fields = doc! {
        "$set": doc!{
            "trades": to_bson(&trades).unwrap(),
        }
    };

    match collection
        .update_one(doc! {"name": _pool_name}, updated_fields, None)
        .await
    {
        Ok(res) => {
            println!("{:?}", res);
            return Ok(create_success_response(&None).await);
        }
        Err(e) => {
            println!("{}", e);
            return Ok(create_error_response("The pool could not be updated.".to_string()).await);
        }
    }
}

pub async fn respond_trade(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _is_accepted: bool,
    _trade_id: u32,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();

    if pool_unwrap.nb_trade < _trade_id {
        return Ok(create_error_response("This trade does not exist.".to_string()).await);
    }

    let mut trades = pool_unwrap.trades.clone().unwrap();
    let mut pool_context = pool_unwrap.context.clone().unwrap();

    // validate that the status of the trade is NEW

    if !matches!(trades[_trade_id as usize].status, TradeStatus::NEW) {
        return Ok(create_error_response(
            "The trade is not in a valid state to be responded.".to_string(),
        )
        .await);
    }

    // validate that only the one that was ask for the trade can accept it.

    if trades[_trade_id as usize].ask_to != *_user_id
        && !owner_and_assitants_rights(_user_id, &pool_unwrap).await
    {
        return Ok(create_error_response(
            "Only the one that was ask for the trade can accept it.".to_string(),
        )
        .await);
    }

    // validate that 24h have been passed since the trade was created.

    let now = Utc::now().timestamp_millis();

    if trades[_trade_id as usize].date_created + 8640000 > now
        && !owner_and_assitants_rights(_user_id, &pool_unwrap).await
    {
        return Ok(create_error_response(
            "The trade needs to be active for 24h before being able to accept it.".to_string(),
        )
        .await);
    }

    // validate that both trade parties own those items

    if _is_accepted {
        if !trade_roster_items(&mut pool_context, &trades[_trade_id as usize]).await {
            return Ok(create_error_response("Trading items is not valid.".to_string()).await);
        }

        trades[_trade_id as usize].status = TradeStatus::ACCEPTED;
        trades[_trade_id as usize].date_accepted = Utc::now().timestamp_millis();
    } else {
        trades[_trade_id as usize].status = TradeStatus::REFUSED;
    };

    // Update fields with the new trade response

    let updated_fields = doc! {
        "$set": doc!{
            "trades": to_bson(&trades).unwrap(),
            "context.pooler_roster": to_bson(&pool_context.pooler_roster ).unwrap(),
            "context.tradable_picks": to_bson(&pool_context.tradable_picks ).unwrap()
        }
    };

    match collection
        .update_one(doc! {"name": _pool_name}, updated_fields, None)
        .await
    {
        Ok(res) => {
            println!("{:?}", res);
            return Ok(create_success_response(&None).await);
        }
        Err(e) => {
            println!("{}", e);
            return Ok(create_error_response("The pool could not be updated.".to_string()).await);
        }
    }
}

pub async fn fill_spot(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _player: &Player,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();

    let mut pooler_roster = pool_unwrap.context.unwrap().pooler_roster;

    if !pooler_roster.contains_key(_user_id) {
        return Ok(create_error_response(
            "The pooler is not a participant of the pool.".to_string(),
        )
        .await);
    }

    if pooler_roster[_user_id].chosen_forwards.contains(_player)
        || pooler_roster[_user_id].chosen_defenders.contains(_player)
        || pooler_roster[_user_id].chosen_goalies.contains(_player)
        || !pooler_roster[_user_id].chosen_reservists.contains(_player)
    {
        return Ok(create_error_response(
            "The player should only be in the reservist pooler's list.".to_string(),
        )
        .await);
    }

    let mut is_added = false;

    match _player.position {
        Position::F => {
            if (pooler_roster[_user_id].chosen_forwards.len() as u8) < pool_unwrap.number_forwards {
                if let Some(x) = pooler_roster.get_mut(_user_id) {
                    x.chosen_forwards.push(_player.clone());
                    is_added = true;
                }
            }
        }
        Position::D => {
            if (pooler_roster[_user_id].chosen_defenders.len() as u8) < pool_unwrap.number_defenders
            {
                if let Some(x) = pooler_roster.get_mut(_user_id) {
                    x.chosen_defenders.push(_player.clone());
                    is_added = true;
                }
            }
        }
        Position::G => {
            if (pooler_roster[_user_id].chosen_goalies.len() as u8) < pool_unwrap.number_goalies {
                if let Some(x) = pooler_roster.get_mut(_user_id) {
                    x.chosen_goalies.push(_player.clone());
                    is_added = true;
                }
            }
        }
    }

    if !is_added {
        return Ok(create_error_response("There is no space for that player.".to_string()).await);
    }

    if let Some(x) = pooler_roster.get_mut(_user_id) {
        x.chosen_reservists.retain(|player| player != _player);
    }
    // Update fields with the filled spot

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pooler_roster).unwrap()
        }
    };

    match collection
        .update_one(doc! {"name": _pool_name}, updated_fields, None)
        .await
    {
        Ok(res) => {
            println!("{:?}", res);
            return Ok(create_success_response(&None).await);
        }
        Err(e) => {
            println!("{}", e);
            return Ok(create_error_response("The pool could not be updated.".to_string()).await);
        }
    }
}

pub async fn undo_select_player(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();

    // validate that the user making the request is the pool owner.

    if &pool_unwrap.owner != _user_id {
        return Ok(create_error_response("Only the owner of the pool can undo.".to_string()).await);
    }

    // validate that the pool is into the draft status.

    if !matches!(pool_unwrap.status, PoolState::Draft) {
        return Ok(create_error_response(
            "The pool must be into the draft status to perform an undo.".to_string(),
        )
        .await);
    }

    let mut pool_context = pool_unwrap.context.unwrap();

    // validate there is something to undo.

    if pool_context.players_name_drafted.len() == 0 {
        return Ok(create_error_response("There is nothing to undo".to_string()).await);
    }

    let mut latest_pick;

    loop {
        latest_pick = pool_context.players_name_drafted.pop().unwrap();
        if latest_pick > 0 {
            break;
        }
    }

    let pick_number = pool_context.players_name_drafted.len();
    let latest_drafter;

    if pool_unwrap.final_rank.is_some() {
        // This comes from a Dynastie draft.

        let final_rank = pool_unwrap.final_rank.clone().unwrap(); // the final rank is used to see who picks

        let index = pool_unwrap.number_poolers as usize
            - 1
            - (pick_number % pool_unwrap.number_poolers as usize);

        let next_drafter = &final_rank[index];

        if pick_number < (pool_unwrap.tradable_picks * pool_unwrap.number_poolers) as usize {
            // use the tradable_picks to see who will draft next.
            let tradable_picks = pool_context.tradable_picks.clone().unwrap();

            latest_drafter = tradable_picks[pick_number / pool_unwrap.number_poolers as usize]
                [next_drafter]
                .clone();
        } else {
            // Use the final_rank to see who draft next.
            latest_drafter = next_drafter.clone();
        }
    } else {
        // this comes from a new draft.

        let participants = pool_unwrap.participants.unwrap(); // the participants is used to see who picks

        let index = pick_number % pool_unwrap.number_poolers as usize;
        latest_drafter = participants[index].clone();
    }

    // Remove the player from the player roster.

    remove_roster_player(&mut pool_context, latest_pick, &latest_drafter).await;

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster).unwrap(),
            "context.players_name_drafted": to_bson(&pool_context.players_name_drafted).unwrap(),
        }
    };

    // Update the fields in the mongoDB pool document.

    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let new_pool = collection
        .find_one_and_update(
            doc! {"name": pool_unwrap.name},
            updated_fields,
            find_one_and_update_options,
        )
        .await
        .unwrap();

    if new_pool.is_none() {
        return Ok(create_error_response("The pool could not be updated.".to_string()).await);
    }

    Ok(create_success_response(&new_pool).await)
}

pub async fn modify_roster(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _forw_selected: &Vec<Player>,
    _def_selected: &Vec<Player>,
    _goal_selected: &Vec<Player>,
    _reserv_selected: &Vec<Player>,
) -> mongodb::error::Result<PoolMessageResponse> {
    let start_season_date =
        Local.from_utc_date(&NaiveDate::parse_from_str(START_SEASON_DATE, "%Y-%m-%d").unwrap());
    let end_season_date =
        Local.from_utc_date(&NaiveDate::parse_from_str(END_SEASON_DATE, "%Y-%m-%d").unwrap());

    let mut today = Local::today();

    let time = Local::now();

    println!(
        "Roster modification by {}, performed at {} {}",
        _user_id, today, time
    );

    // At 12PM we start to count the action for the next day.

    if time.hour() >= 12 {
        today = today + Duration::days(1);
    }

    if today >= start_season_date && today <= end_season_date {
        let mut bAllowed = false;

        for DATE in FIRST_SATHURDAY_OF_MONTHS {
            let sathurday =
                Local.from_utc_date(&NaiveDate::parse_from_str(DATE, "%Y-%m-%d").unwrap());

            if sathurday == today {
                bAllowed = true;
                break;
            }
        }

        if !bAllowed {
            return Ok(create_error_response(
                "You are not allowed to modify your roster today.".to_string(),
            )
            .await);
        }
    }

    let collection = db.collection::<Pool>("pools");
    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();
    let pool_context = pool_unwrap.context.unwrap();
    let mut pool_roster = pool_context.pooler_roster;

    if !pool_roster.contains_key(_user_id) {
        return Ok(create_error_response("User is not in the pool.".to_string()).await);
    }

    // Validate the total amount of forwards selected

    if _forw_selected.len() != pool_unwrap.number_forwards as usize {
        return Ok(create_error_response(
            "The amount of forwards selected is not valid".to_string(),
        )
        .await);
    }

    // Validate the total amount of defenders selected

    if _def_selected.len() != pool_unwrap.number_defenders as usize {
        return Ok(create_error_response(
            "The amount of defenders selected is not valid".to_string(),
        )
        .await);
    }

    // Validate the total amount of goalies selected

    if _goal_selected.len() != pool_unwrap.number_goalies as usize {
        return Ok(create_error_response(
            "The amount of goalies selected is not valid".to_string(),
        )
        .await);
    }

    // Validate the total amount of players selected (It should be the same as before)

    if let Some(roster) = pool_roster.get(_user_id) {
        let amount_selected_players = _forw_selected.len()
            + _def_selected.len()
            + _goal_selected.len()
            + _reserv_selected.len();

        let amount_players_before = roster.chosen_forwards.len()
            + roster.chosen_defenders.len()
            + roster.chosen_goalies.len()
            + roster.chosen_reservists.len();

        if amount_players_before != amount_selected_players {
            return Ok(create_error_response(
                "The amount of selected players is not valid.".to_string(),
            )
            .await);
        }
    }

    // validate each selected players possession by the user asking the modification.
    // Also validate dupplication in the new list.

    let mut selected_player_map = HashMap::<u32, bool>::new(); // used to validate dupplication

    // Forwards validation

    for forward in _forw_selected {
        if selected_player_map.contains_key(&forward.id) {
            return Ok(create_error_response(format!(
                "The player {} was dupplicated",
                forward.name
            ))
            .await);
        }
        selected_player_map.insert(forward.id, true);
        if !validate_player_possession(forward, &pool_roster[_user_id]).await {
            return Ok(
                create_error_response(format!("You do not possess {}.", forward.name)).await,
            );
        }
    }

    // Defenders validation

    for defender in _def_selected {
        if selected_player_map.contains_key(&defender.id) {
            return Ok(create_error_response(format!("{} was dupplicated.", defender.name)).await);
        }
        selected_player_map.insert(defender.id, true);
        if !validate_player_possession(defender, &pool_roster[_user_id]).await {
            return Ok(
                create_error_response(format!("You do not possess {}.", defender.name)).await,
            );
        }
    }

    // Goalies validation

    for goaly in _goal_selected {
        if selected_player_map.contains_key(&goaly.id) {
            return Ok(create_error_response(format!("{} was dupplicated", goaly.name)).await);
        }
        selected_player_map.insert(goaly.id, true);
        if !validate_player_possession(goaly, &pool_roster[_user_id]).await {
            return Ok(create_error_response(format!("You do not possess {}.", goaly.name)).await);
        }
    }

    // Reservists validation

    for reservist in _reserv_selected {
        if selected_player_map.contains_key(&reservist.id) {
            return Ok(create_error_response(format!("{} was dupplicated", reservist.name)).await);
        }
        selected_player_map.insert(reservist.id, true);
        if !validate_player_possession(reservist, &pool_roster[_user_id]).await {
            return Ok(
                create_error_response(format!("You do not possess {}.", reservist.name)).await,
            );
        }
    }

    if let Some(roster) = pool_roster.get_mut(_user_id) {
        roster.chosen_forwards = _forw_selected.clone();
        roster.chosen_defenders = _def_selected.clone();
        roster.chosen_goalies = _goal_selected.clone();
        roster.chosen_reservists = _reserv_selected.clone();
    }

    // Modify the all the pooler_roster (we could update only the pooler_roster[userId] if necessary)

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_roster).unwrap(),
        }
    };

    match collection
        .update_one(doc! {"name": _pool_name}, updated_fields, None)
        .await
    {
        Ok(res) => {
            println!("{:?}", res);
            return Ok(create_success_response(&None).await);
        }
        Err(e) => {
            println!("{}", e);
            return Ok(create_error_response("The pool could not be updated.".to_string()).await);
        }
    }
}

pub async fn protect_players(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _forw_protected: &Vec<Player>,
    _def_protected: &Vec<Player>,
    _goal_protected: &Vec<Player>,
    _reserv_protected: &Vec<Player>,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_short_pool_by_name(&collection, _pool_name).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();

    let mut pool_context = pool_unwrap.context.unwrap();

    // make sure the user making the resquest is a pool participants.

    if !pool_context.pooler_roster.contains_key(_user_id) {
        return Ok(create_error_response(
            "The pooler is not a participant of the pool.".to_string(),
        )
        .await);
    }

    // validate that the numbers of players protected is ok.

    if (_forw_protected.len() as u8) > pool_unwrap.number_forwards {
        return Ok(create_error_response("To much forwards protected".to_string()).await);
    }

    if (_def_protected.len() as u8) > pool_unwrap.number_defenders {
        return Ok(create_error_response("To much defenders protected".to_string()).await);
    }

    if (_goal_protected.len() as u8) > pool_unwrap.number_goalies {
        return Ok(create_error_response("To much goalies protected".to_string()).await);
    }

    if (_reserv_protected.len() as u8) > pool_unwrap.number_reservists {
        return Ok(create_error_response("To much reservists protected".to_string()).await);
    }

    let tot_player_protected = _forw_protected.len()
        + _def_protected.len()
        + _goal_protected.len()
        + _reserv_protected.len();

    if tot_player_protected as u8 != pool_unwrap.next_season_number_players_protected {
        return Ok(create_error_response(
            "The number of selected players is not valid".to_string(),
        )
        .await);
    }

    // Validate that the participant realy possess the selected players.

    let mut is_selected_players_valid: bool;

    for player in _forw_protected.iter() {
        is_selected_players_valid =
            validate_player_possession(player, &pool_context.pooler_roster[_user_id]).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected forwards".to_string(),
            )
            .await);
        }
    }

    for player in _def_protected.iter() {
        is_selected_players_valid =
            validate_player_possession(player, &pool_context.pooler_roster[_user_id]).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected defenders".to_string(),
            )
            .await);
        }
    }

    for player in _goal_protected.iter() {
        is_selected_players_valid =
            validate_player_possession(player, &pool_context.pooler_roster[_user_id]).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected goalies".to_string(),
            )
            .await);
        }
    }

    for player in _reserv_protected.iter() {
        is_selected_players_valid =
            validate_player_possession(player, &pool_context.pooler_roster[_user_id]).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected reservists".to_string(),
            )
            .await);
        }
    }

    // clear previous season roster and add those players list to the new roster.

    if let Some(x) = pool_context.pooler_roster.get_mut(_user_id) {
        x.chosen_forwards = _forw_protected.clone();
        x.chosen_defenders = _def_protected.clone();
        x.chosen_goalies = _goal_protected.clone();
        x.chosen_reservists = _reserv_protected.clone();
    }

    // Look if all participants have protected their players

    let participants = &pool_unwrap.participants.unwrap();
    let mut is_done = true;

    let users_players_count =
        get_users_players_count(&pool_context.pooler_roster, participants).await;

    for participant in participants.iter() {
        if users_players_count[participant] != pool_unwrap.next_season_number_players_protected {
            is_done = false; // not all participants are ready
            break;
        }
    }

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pool_context.pooler_roster).unwrap(),
            //"context.score_by_day": Some(), // TODO: clear this field since it is not usefull for the new season.
            "status": if is_done {to_bson(&PoolState::Draft).unwrap()} else {to_bson(&PoolState::Dynastie).unwrap()}
        }
    };

    // Update the fields in the mongoDB pool document.

    match collection
        .update_one(doc! {"name": _pool_name}, updated_fields, None)
        .await
    {
        Ok(res) => {
            println!("{:?}", res);
            return Ok(create_success_response(&None).await);
        }
        Err(e) => {
            println!("{}", e);
            return Ok(create_error_response("The pool could not be updated.".to_string()).await);
        }
    }
}

async fn trade_roster_items(_pool_context: &mut PoolContext, _trade: &Trade) -> bool {
    if !validate_trade_possession(&_trade.from_items, _pool_context, &_trade.proposed_by).await
        || !validate_trade_possession(&_trade.from_items, _pool_context, &_trade.proposed_by).await
    {
        return false;
    }

    for player_id in _trade.from_items.players.iter() {
        trade_roster_player(
            _pool_context,
            *player_id,
            &_trade.proposed_by,
            &_trade.ask_to,
        )
        .await;
    }

    for player_id in _trade.to_items.players.iter() {
        trade_roster_player(
            _pool_context,
            *player_id,
            &_trade.ask_to,
            &_trade.proposed_by,
        )
        .await;
    }

    for pick in _trade.from_items.picks.iter() {
        if let Some(tradable_picks) = &mut _pool_context.tradable_picks {
            if let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from) {
                *owner = _trade.ask_to.clone();
                println!("From: {}", _trade.ask_to);
            }
        }
    }

    for pick in _trade.to_items.picks.iter() {
        if let Some(tradable_picks) = &mut _pool_context.tradable_picks {
            if let Some(owner) = tradable_picks[pick.round as usize].get_mut(&pick.from) {
                *owner = _trade.proposed_by.clone();
                println!("To: {}", _trade.proposed_by)
            }
        }
    }

    true
}

async fn remove_roster_player(_pool_context: &mut PoolContext, _player_id: u32, _user_id: &String) {
    if let Some(x) = _pool_context.pooler_roster.get_mut(_user_id) {
        x.chosen_forwards.retain(|player| player.id != _player_id);
    }
    if let Some(x) = _pool_context.pooler_roster.get_mut(_user_id) {
        x.chosen_defenders.retain(|player| player.id != _player_id);
    }
    if let Some(x) = _pool_context.pooler_roster.get_mut(_user_id) {
        x.chosen_goalies.retain(|player| player.id != _player_id);
    }
    if let Some(x) = _pool_context.pooler_roster.get_mut(_user_id) {
        x.chosen_reservists.retain(|player| player.id != _player_id);
    }
}

async fn trade_roster_player(
    _pool_context: &mut PoolContext,
    _player_id: u32,
    _participant_giver: &String,
    _participant_receiver: &String,
) -> bool {
    let mut player: Option<Player> = None; // The player traded from the giver.

    if let Some(giver) = _pool_context.pooler_roster.get_mut(_participant_giver) {
        // 1) Look into Forwards

        let mut index = giver
            .chosen_forwards
            .iter()
            .position(|r| r.id == _player_id);

        if index.is_some() {
            player = Some(giver.chosen_forwards[index.unwrap()].clone());
            giver.chosen_forwards.remove(index.unwrap());
        } else {
            // 2) Look into Defenders

            index = giver
                .chosen_defenders
                .iter()
                .position(|r| r.id == _player_id);

            if index.is_some() {
                player = Some(giver.chosen_defenders[index.unwrap()].clone());
                giver.chosen_defenders.remove(index.unwrap());
            } else {
                // 3) Look into Goalies

                index = giver.chosen_goalies.iter().position(|r| r.id == _player_id);

                if index.is_some() {
                    player = Some(giver.chosen_goalies[index.unwrap()].clone());
                    giver.chosen_goalies.remove(index.unwrap());
                } else {
                    // 4) Look into Reservists

                    index = giver
                        .chosen_reservists
                        .iter()
                        .position(|r| r.id == _player_id);

                    if index.is_some() {
                        player = Some(giver.chosen_reservists[index.unwrap()].clone());
                        giver.chosen_reservists.remove(index.unwrap());
                    } else {
                        return false;
                    }
                }
            }
        }
    }

    // Add the player to the receiver's reservists.

    if let Some(receiver) = _pool_context.pooler_roster.get_mut(_participant_receiver) {
        if player.is_some() {
            receiver.chosen_reservists.push(player.unwrap());
            return true;
        }
    }

    false
}

async fn validate_trade_possession(
    _trading_list: &TradeItems,
    _pool_context: &PoolContext,
    _participant: &String,
) -> bool {
    for player_id in _trading_list.players.iter() {
        if !validate_player_possession_with_id(
            *player_id,
            &_pool_context.pooler_roster[_participant],
        )
        .await
        {
            return false;
        }
    }

    if let Some(tradable_picks) = &_pool_context.tradable_picks {
        for pick in _trading_list.picks.iter() {
            if tradable_picks[pick.round as usize][&pick.from] != *_participant {
                return false;
            }
        }
    }

    true
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

async fn validate_player_possession(_player: &Player, _pooler_roster: &PoolerRoster) -> bool {
    match _player.position {
        Position::F => {
            if !_pooler_roster.chosen_forwards.contains(_player)
                && !_pooler_roster.chosen_reservists.contains(_player)
            {
                return false;
            }
        }
        Position::D => {
            if !_pooler_roster.chosen_defenders.contains(_player)
                && !_pooler_roster.chosen_reservists.contains(_player)
            {
                return false;
            }
        }
        Position::G => {
            if !_pooler_roster.chosen_goalies.contains(_player)
                && !_pooler_roster.chosen_reservists.contains(_player)
            {
                return false;
            }
        }
    }

    true
}

async fn validate_player_possession_with_id(
    _player_id: u32,
    _pooler_roster: &PoolerRoster,
) -> bool {
    let player = Player {
        id: _player_id,
        position: Position::F,
        name: "".to_string(),
        team: 0,
        caps: None,
    };

    _pooler_roster.chosen_forwards.contains(&player)
        || _pooler_roster.chosen_defenders.contains(&player)
        || _pooler_roster.chosen_goalies.contains(&player)
        || _pooler_roster.chosen_reservists.contains(&player)
}

async fn create_error_response(_message: String) -> PoolMessageResponse {
    PoolMessageResponse {
        success: false,
        message: _message,
        pool: None,
    }
}

async fn create_success_response(_pool: &Option<Pool>) -> PoolMessageResponse {
    PoolMessageResponse {
        success: true,
        message: "".to_string(),
        pool: _pool.clone(),
    }
}

async fn owner_and_assitants_rights(_user_id: &String, _pool_info: &Pool) -> bool {
    *_user_id == _pool_info.owner || _pool_info.assistants.contains(_user_id)
}

async fn owner_rights(_user_id: &String, _pool_info: &Pool) -> bool {
    *_user_id == _pool_info.owner
}
