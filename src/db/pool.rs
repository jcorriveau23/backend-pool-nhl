use futures::stream::TryStreamExt;
use mongodb::bson::{doc, to_bson};
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::Database;
use std::collections::HashMap;

use crate::models::pool::{
    Player, Pool, PoolContext, PoolCreationRequest, PoolState, PoolerRoster, Position, Trade,
    TradeItems, TradeStatus,
};
use crate::models::response::PoolMessageResponse;

pub async fn find_pool_with_name(
    db: &Database,
    _name: String,
) -> mongodb::error::Result<Option<Pool>> {
    let collection = db.collection::<Pool>("pools");

    // let find_one_options = FindOneOptions::builder()
    // .projection(doc! {"name": 1, "status" : 1})
    // .build();

    let pool_doc = collection.find_one(doc! {"name": _name}, None).await?;

    Ok(pool_doc)
}

pub async fn find_pools(db: &Database) -> mongodb::error::Result<Vec<Pool>> {
    let collection = db.collection::<Pool>("pools");

    let mut cursor = collection.find(None, None).await?;

    let mut users: Vec<Pool> = vec![];

    while let Some(user) = cursor.try_next().await? {
        users.push(user);
    }

    Ok(users)
}

pub async fn create_pool(
    db: &Database,
    _owner: String,
    _pool_info: PoolCreationRequest,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    if find_pool_with_name(db, _pool_info.name.clone())
        .await?
        .is_some()
    {
        return Ok(create_error_response("pool name already exist.".to_string()).await);
    } else {
        // Create the default Pool creation class.
        let pool = Pool {
            name: _pool_info.name,
            owner: _owner,
            number_poolers: _pool_info.number_pooler,
            participants: None,
            number_forwards: 9,
            number_defenders: 4,
            number_goalies: 2,
            number_reservists: 2,
            forward_pts_goals: 2,
            forward_pts_assists: 1,
            forward_pts_hattricks: 3,
            defender_pts_goals: 3,
            defender_pts_assists: 2,
            defender_pts_hattricks: 2,
            goalies_pts_wins: 2,
            goalies_pts_shutouts: 3,
            goalies_pts_goals: 3,
            goalies_pts_assists: 2,
            next_season_number_players_protected: 8,
            tradable_picks: 3,
            status: PoolState::Created,
            final_rank: None,
            nb_player_drafted: 0,
            nb_trade: 0,
            trades: None,
            context: None,
        };

        collection.insert_one(pool, None).await?;

        Ok(create_success_response(&None).await)
    }
}

pub async fn delete_pool(
    db: &Database,
    _user_id: String,
    _pool_name: String,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_pool_with_name(db, _pool_name.clone()).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    } else if pool.unwrap().owner != _user_id {
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
                "Only the owner of the pool can delete the pool.".to_string(),
            )
            .await);
        }

        let collection = db.collection::<Pool>("pools");

        // create pool context
        let mut pool_context = PoolContext {
            pooler_roster: HashMap::new(),
            draft_order: Vec::new(),
            score_by_day: Some(HashMap::new()),
            tradable_picks: Some(Vec::new()),
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

        // TODO: randomize the list of participants and fill up the draft_order members like before.
        //thread_rng().shuffle(&mut _participants);

        let number_picks: u16 = _poolInfo.number_poolers as u16
            * (_poolInfo.number_forwards as u16
                + _poolInfo.number_defenders as u16
                + _poolInfo.number_goalies as u16
                + _poolInfo.number_reservists as u16);

        // fill up the draft list.

        for i in 0..number_picks {
            let index = i as usize % _poolInfo.number_poolers as usize;
            pool_context.draft_order.push(participants[index].clone())
        }

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

pub async fn select_player(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _player: &Player,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_pool_with_name(db, _pool_name.clone()).await?;

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

    if pool_context.draft_order[0] != *_user_id {
        return Ok(create_error_response("Not your turn.".to_string()).await);
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
        if validate_player_possession(_player, &pool_context, participant).await {
            return Ok(create_error_response("This player is already picked.".to_string()).await);
        }
    }

    // Then, Add the chosen player in its right spot. When there is no place in the position of the player we will add it to the reservists.

    let mut pooler_roster = pool_context.pooler_roster[_user_id].clone();

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

    // Save the dictionnary modified with the new pick inside the pool_context.

    if let Some(x) = pool_context.pooler_roster.get_mut(_user_id) {
        *x = pooler_roster;
    }
    // change the

    pool_context.draft_order.remove(0);

    // the status change to InProgress when the draft is completed.

    let status = if pool_context.draft_order.is_empty() {
        PoolState::InProgress
    } else {
        pool_unwrap.status
    };

    // generate the list of tradable_picks for the next season

    let mut vect = vec![];

    for _pick_round in 0..pool_unwrap.tradable_picks {
        let mut round = HashMap::new();

        for participant in participants.iter() {
            round.insert(participant.clone(), participant.clone());
        }

        vect.push(round);
    }

    pool_context.tradable_picks = Some(vect);

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
    let collection = db.collection::<Pool>("pools");

    let pool = find_pool_with_name(db, _pool_name.clone()).await?;

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
        if (matches!(trade.status, TradeStatus::NEW)
            || matches!(trade.status, TradeStatus::ACCEPTED))
            && (trade.proposed_by == *_user_id && trade.ask_to == *_user_id)
        {
            return Ok(create_error_response(
                "User can only have one active trade at a time.".to_string(),
            )
            .await);
        }
    }

    // does the the from or to side has too much items in the trade ?

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

    // Does the pooler really possess the players ?

    if !validate_trade_possession(&_trade.from_items, &pool_context, &_trade.proposed_by).await
        || !validate_trade_possession(&_trade.to_items, &pool_context, &_trade.ask_to).await
    {
        return Ok(create_error_response(
            "One of the to pooler does not poccess the items list provided for the trade."
                .to_string(),
        )
        .await);
    }

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

pub async fn cancel_trade(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _trade_id: u32,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_pool_with_name(db, _pool_name.clone()).await?;

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

pub async fn respond_trade(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _is_accepted: bool,
    _trade_id: u32,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_pool_with_name(db, _pool_name.clone()).await?;

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

    // validate only the owner can cancel a trade

    if trades[_trade_id as usize].ask_to != *_user_id {
        return Ok(create_error_response(
            "Only the one that was ask for the trade can accept it.".to_string(),
        )
        .await);
    }

    // validate the both trade parties own those items

    if !remove_roster_items(&mut pool_context, &trades[_trade_id as usize]).await {
        return Ok(create_error_response("Trading items is not valid.".to_string()).await);
    }

    trades[_trade_id as usize].status = if _is_accepted {
        TradeStatus::ACCEPTED
    } else {
        TradeStatus::REFUSED
    };

    trades[_trade_id as usize].status = TradeStatus::CANCELLED;
    // Update fields with the new trade

    let updated_fields = doc! {
        "$set": doc!{
            "trades": to_bson(&trades).unwrap(),
            "context.pooler_roster": to_bson(&pool_context.pooler_roster ).unwrap()
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

pub async fn fill_spot(
    db: &Database,
    _user_id: &String,
    _pool_name: &String,
    _player: &Player,
) -> mongodb::error::Result<PoolMessageResponse> {
    let collection = db.collection::<Pool>("pools");

    let pool = find_pool_with_name(db, _pool_name.clone()).await?;

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
        x.chosen_forwards.retain(|player| player != _player);
    }
    // Update fields with the new trade

    let updated_fields = doc! {
        "$set": doc!{
            "context.pooler_roster": to_bson(&pooler_roster).unwrap()
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

    let pool = find_pool_with_name(db, _pool_name.clone()).await?;

    if pool.is_none() {
        return Ok(create_error_response("Pool name does not exist.".to_string()).await);
    }

    let pool_unwrap = pool.unwrap();

    let mut pooler_context = pool_unwrap.context.unwrap();

    // validate that the numbers of players protected is ok.

    if !pooler_context.pooler_roster.contains_key(_user_id) {
        return Ok(create_error_response(
            "The pooler is not a participant of the pool.".to_string(),
        )
        .await);
    }

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
            validate_player_possession(player, &pooler_context, _user_id).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected forwards".to_string(),
            )
            .await);
        }
    }

    for player in _def_protected.iter() {
        is_selected_players_valid =
            validate_player_possession(player, &pooler_context, _user_id).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected defenders".to_string(),
            )
            .await);
        }
    }

    for player in _goal_protected.iter() {
        is_selected_players_valid =
            validate_player_possession(player, &pooler_context, _user_id).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected goalies".to_string(),
            )
            .await);
        }
    }

    for player in _reserv_protected.iter() {
        is_selected_players_valid =
            validate_player_possession(player, &pooler_context, _user_id).await;

        if !is_selected_players_valid {
            return Ok(create_error_response(
                "The pooler does not poccess  one of the selected reservisists".to_string(),
            )
            .await);
        }
    }

    // clear previous season roster and add those players list to the new roster.

    if let Some(x) = pooler_context.pooler_roster.get_mut(_user_id) {
        x.chosen_forwards = _forw_protected.clone();
        x.chosen_defenders = _def_protected.clone();
        x.chosen_goalies = _goal_protected.clone();
        x.chosen_reservists = _reserv_protected.clone();
    }

    // Look if all participants have protected their players

    let mut is_all_participants_ready = true;

    for participant in pool_unwrap.participants.unwrap().iter() {
        if (pooler_context.pooler_roster[participant]
            .chosen_forwards
            .len()
            + pooler_context.pooler_roster[participant]
                .chosen_defenders
                .len()
            + pooler_context.pooler_roster[participant]
                .chosen_goalies
                .len()
            + pooler_context.pooler_roster[participant]
                .chosen_reservists
                .len()) as u8
            != pool_unwrap.next_season_number_players_protected
        {
            is_all_participants_ready = false; // not all participants are ready
        }
    }

    // Fill up the draft list, we need to use the tradable_picks with the final_rank, so the draft list take the last season into account.

    let mut status = PoolState::Dynastie;

    let mut updated_fields = doc!{};

    if is_all_participants_ready {
        status = PoolState::Draft;

        let final_rank = pool_unwrap.final_rank.clone().unwrap();

        let total_round = pool_unwrap.number_forwards
            + pool_unwrap.number_defenders
            + pool_unwrap.number_goalies
            + pool_unwrap.number_reservists;

        let tradable_picks = pooler_context.tradable_picks.unwrap();

        for round in 0..pool_unwrap.tradable_picks {
            for participant in final_rank.iter() {
                if total_round < pool_unwrap.tradable_picks {
                    pooler_context
                        .draft_order
                        .push(tradable_picks[round as usize][participant].clone());
                } else {
                    pooler_context.draft_order.push(participant.clone());
                }
            }
        }
            // Update fields with the new trade
    
            updated_fields = doc! {
                "$set": doc!{
                    "context.pooler_roster": to_bson(&pooler_context.pooler_roster).unwrap(),
                    "context.draft_order": to_bson(&pooler_context.draft_order).unwrap(),
                    "status": to_bson(&status).unwrap()
                }
            };
    }
    else{
        // Update fields with the new trade

        updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&pooler_context.pooler_roster).unwrap(),
                "status": to_bson(&status).unwrap()
            }
        };
    }

    

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

async fn remove_roster_items(_pool_context: &mut PoolContext, _trade: &Trade) -> bool {
    if !validate_trade_possession(&_trade.from_items, _pool_context, &_trade.proposed_by).await
        || !validate_trade_possession(&_trade.from_items, _pool_context, &_trade.proposed_by).await
    {
        return false;
    }

    for player in _trade.from_items.players.iter() {
        remove_roster_player(_pool_context, player, &_trade.proposed_by).await;
    }

    for player in _trade.to_items.players.iter() {
        remove_roster_player(_pool_context, player, &_trade.ask_to).await;
    }

    for pick in _trade.from_items.picks.iter() {
        if let Some(tradable_picks) = &mut _pool_context.tradable_picks {
            if let Some(owner) = tradable_picks[pick.rank as usize].get_mut(&pick.pooler) {
                *owner = _trade.ask_to.clone();
            }
        }
    }

    for pick in _trade.to_items.picks.iter() {
        if let Some(tradable_picks) = &mut _pool_context.tradable_picks {
            if let Some(owner) = tradable_picks[pick.rank as usize].get_mut(&pick.pooler) {
                *owner = _trade.proposed_by.clone();
            }
        }
    }

    true
}

async fn remove_roster_player(
    _pool_context: &mut PoolContext,
    _player: &Player,
    _participant: &String,
) {
    match _player.position {
        Position::F => {
            if let Some(x) = _pool_context.pooler_roster.get_mut(_participant) {
                x.chosen_forwards.retain(|player| player != _player);
            }
            if let Some(x) = _pool_context.pooler_roster.get_mut(_participant) {
                x.chosen_reservists.retain(|player| player != _player);
            }
        }
        Position::D => {
            if let Some(x) = _pool_context.pooler_roster.get_mut(_participant) {
                x.chosen_defenders.retain(|player| player != _player);
            }
            if let Some(x) = _pool_context.pooler_roster.get_mut(_participant) {
                x.chosen_reservists.retain(|player| player != _player);
            }
        }
        Position::G => {
            if let Some(x) = _pool_context.pooler_roster.get_mut(_participant) {
                x.chosen_goalies.retain(|player| player != _player);
            }
            if let Some(x) = _pool_context.pooler_roster.get_mut(_participant) {
                x.chosen_reservists.retain(|player| player != _player);
            }
        }
    }
}

async fn validate_trade_possession(
    _trading_list: &TradeItems,
    _pool_context: &PoolContext,
    _participant: &String,
) -> bool {
    for player in _trading_list.players.iter() {
        if !validate_player_possession(player, _pool_context, _participant).await {
            return false;
        }
    }

    if let Some(tradable_picks) = &_pool_context.tradable_picks {
        for pick in _trading_list.picks.iter() {
            if tradable_picks[pick.rank as usize][&pick.pooler] != *_participant {
                return false;
            }
        }
    }

    true
}

async fn validate_player_possession(
    _player: &Player,
    _pool_context: &PoolContext,
    _participant: &String,
) -> bool {
    match _player.position {
        Position::F => {
            if !_pool_context.pooler_roster[_participant]
                .chosen_forwards
                .contains(_player)
                && !_pool_context.pooler_roster[_participant]
                    .chosen_reservists
                    .contains(_player)
            {
                return false;
            }
        }
        Position::D => {
            if !_pool_context.pooler_roster[_participant]
                .chosen_defenders
                .contains(_player)
                && !_pool_context.pooler_roster[_participant]
                    .chosen_reservists
                    .contains(_player)
            {
                return false;
            }
        }
        Position::G => {
            if !_pool_context.pooler_roster[_participant]
                .chosen_goalies
                .contains(_player)
                && !_pool_context.pooler_roster[_participant]
                    .chosen_reservists
                    .contains(_player)
            {
                return false;
            }
        }
    }

    true
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
