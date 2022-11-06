use chrono::Local;
use mongodb::bson::doc;
use mongodb::Database;

use rocket::serde::json::Json;
use rocket::State;

use crate::db::pool;
use crate::errors::response::MyError;
use crate::models::pool::{
    CancelTradeRequest, CreateTradeRequest, FillSpotRequest, Pool, PoolCreationRequest,
    PoolDeletionRequest, PoolUndoSelectionRequest, ProjectedPoolShort, ProtectPlayersRequest,
    RespondTradeRequest, SelectPlayerRequest, StartDraftRequest,
};
use crate::models::response::PoolMessageResponse;
use crate::routes::jwt::{return_token_error, ApiKeyError, UserToken};

/// get Pool document by _name
//  http://127.0.0.1:8000/rust-api/pool/test4
#[get("/pool/<_name>")]
pub async fn get_pool_by_name(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    _name: String,
) -> Result<Json<Pool>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    match pool::find_pool_by_name(db, &_name).await {
        Ok(data) => {
            if data.is_none() {
                return Err(MyError::build(
                    400,
                    Some("Pool not found with name".to_string()),
                ));
            }

            Ok(Json(data.unwrap()))
        }
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

/// get Pool document by _name
//  http://127.0.0.1:8000/rust-api/pool/test4
#[get("/pool/<_name>/<_from>")]
pub async fn get_pool_by_name_with_range(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    _name: String,
    _from: String,
) -> Result<Json<Pool>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }
    let user_token = token.unwrap();

    println!("Time: {}, user: {}", Local::now(), user_token._id);

    match pool::find_pool_by_name_with_range(db, &_name, &_from).await {
        Ok(data) => {
            if data.is_none() {
                return Err(MyError::build(
                    400,
                    Some("Pool not found with name".to_string()),
                ));
            }

            Ok(Json(data.unwrap()))
        }
        Err(e) => {
            println!("{}", e);
            Err(MyError::build(400, Some(e.to_string())))
        }
    }
}

/// get all Pool documents
//  http://127.0.0.1:8000/rust-api/pools
#[get("/pools")]
pub async fn get_pools(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
) -> Result<Json<Vec<ProjectedPoolShort>>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    match pool::find_pools(db).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/create-pool", format = "json", data = "<body>")]
pub async fn create_pool(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<PoolCreationRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let owner = token.unwrap()._id;

    match pool::create_pool(db, owner.to_string(), body.0).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/delete-pool", format = "json", data = "<body>")]
pub async fn delete_pool(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<PoolDeletionRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::delete_pool(db, &user_id.to_string(), &body.name).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/start-draft", format = "json", data = "<body>")]
pub async fn start_draft(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    mut body: Json<StartDraftRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::start_draft(db, &user_id.to_string(), &mut body.poolInfo).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/select-player", format = "json", data = "<body>")]
pub async fn select_player(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<SelectPlayerRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::select_player(db, &user_id.to_string(), &body.name, &body.player).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/undo-select-player", format = "json", data = "<body>")]
pub async fn undo_select_player(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<PoolUndoSelectionRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::undo_select_player(db, &user_id.to_string(), &body.name).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/create-trade", format = "json", data = "<body>")]
pub async fn create_trade(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<CreateTradeRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    let mut trade = body.trade.clone();

    match pool::create_trade(db, &user_id.to_string(), &body.name, &mut trade).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/cancel-trade", format = "json", data = "<body>")]
pub async fn cancel_trade(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<CancelTradeRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::cancel_trade(db, &user_id.to_string(), &body.name, body.trade_id).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/respond-trade", format = "json", data = "<body>")]
pub async fn respond_trade(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<RespondTradeRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::respond_trade(
        db,
        &user_id.to_string(),
        &body.name,
        body.is_accepted,
        body.trade_id,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/fill-spot", format = "json", data = "<body>")]
pub async fn fill_spot(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<FillSpotRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::fill_spot(db, &user_id.to_string(), &body.name, &body.player).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/protect-players", format = "json", data = "<body>")]
pub async fn protect_players(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<ProtectPlayersRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::protect_players(
        db,
        &user_id.to_string(),
        &body.name,
        &body.forw_protected,
        &body.def_protected,
        &body.goal_protected,
        &body.reserv_protected,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}

#[post("/modify-roster", format = "json", data = "<body>")]
pub async fn modify_roster(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<ProtectPlayersRequest>,
) -> Result<Json<PoolMessageResponse>, MyError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    let user_id = token.unwrap()._id;

    match pool::modify_roster(
        db,
        &user_id.to_string(),
        &body.name,
        &body.forw_protected,
        &body.def_protected,
        &body.goal_protected,
        &body.reserv_protected,
    )
    .await
    {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(MyError::build(400, Some(e.to_string()))),
    }
}
