use mongodb::bson::doc;
use mongodb::Database;

use rocket::serde::json::Json;
use rocket::State;

use crate::db::pool;
use crate::errors::response::AppError;
use crate::models::pool::{
    AddRemovePlayerRequest, CancelTradeRequest, CreateTradeRequest, FillSpotRequest,
    ModifyRosterRequest, Pool, PoolCreationRequest, PoolDeletionRequest, PoolUndoSelectionRequest,
    ProjectedPoolShort, ProtectPlayersRequest, RespondTradeRequest, SelectPlayerRequest,
    StartDraftRequest, UpdatePoolSettingsRequest,
};
use crate::models::response::PoolMessageResponse;
use crate::routes::jwt::UserToken;

/// get Pool document by _name
//  http://127.0.0.1:8000/rust-api/pool/test4
#[get("/pool/<_name>")]
pub async fn get_pool_by_name(db: &State<Database>, _name: String) -> Result<Json<Pool>, AppError> {
    pool::find_pool_by_name(db, &_name).await.map(Json)
}

/// get Pool document by _name
//  http://127.0.0.1:8000/rust-api/pool/test4
#[get("/pool/<_name>/<_from>")]
pub async fn get_pool_by_name_with_range(
    db: &State<Database>,
    _name: String,
    _from: String,
) -> Result<Json<Pool>, AppError> {
    pool::find_pool_by_name_with_range(db, &_name, &_from)
        .await
        .map(Json)
}

/// get all Pool documents
//  http://127.0.0.1:8000/rust-api/pools
#[get("/pools")]
pub async fn get_pools(db: &State<Database>) -> Result<Json<Vec<ProjectedPoolShort>>, AppError> {
    pool::find_pools(db).await.map(Json)
}

#[post("/create-pool", format = "json", data = "<body>")]
pub async fn create_pool(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<PoolCreationRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::create_pool(db, token?._id.to_string(), body.0)
        .await
        .map(|data| Json(data))
}

#[post("/delete-pool", format = "json", data = "<body>")]
pub async fn delete_pool(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<PoolDeletionRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::delete_pool(db, &token?._id.to_string(), &body.name)
        .await
        .map(Json)
}

#[post("/start-draft", format = "json", data = "<body>")]
pub async fn start_draft(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    mut body: Json<StartDraftRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::start_draft(db, &token?._id.to_string(), &mut body.poolInfo)
        .await
        .map(Json)
}

#[post("/select-player", format = "json", data = "<body>")]
pub async fn select_player(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<SelectPlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::select_player(db, &token?._id.to_string(), &body.name, &body.player)
        .await
        .map(Json)
}

#[post("/add-player", format = "json", data = "<body>")]
pub async fn add_player(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<AddRemovePlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::add_player(
        db,
        &token?._id.to_string(),
        &body.user_id,
        &body.name,
        &body.player,
    )
    .await
    .map(Json)
}

#[post("/remove-player", format = "json", data = "<body>")]
pub async fn remove_player(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<AddRemovePlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::remove_player(
        db,
        &token?._id.to_string(),
        &body.user_id,
        &body.name,
        &body.player,
    )
    .await
    .map(Json)
}

#[post("/undo-select-player", format = "json", data = "<body>")]
pub async fn undo_select_player(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<PoolUndoSelectionRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::undo_select_player(db, &token?._id.to_string(), &body.name)
        .await
        .map(Json)
}

#[post("/create-trade", format = "json", data = "<body>")]
pub async fn create_trade(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<CreateTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::create_trade(
        db,
        &token?._id.to_string(),
        &body.name,
        &mut body.trade.clone(),
    )
    .await
    .map(Json)
}

#[post("/cancel-trade", format = "json", data = "<body>")]
pub async fn cancel_trade(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<CancelTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::cancel_trade(db, &token?._id.to_string(), &body.name, body.trade_id)
        .await
        .map(Json)
}

#[post("/respond-trade", format = "json", data = "<body>")]
pub async fn respond_trade(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<RespondTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::respond_trade(
        db,
        &token?._id.to_string(),
        &body.name,
        body.is_accepted,
        body.trade_id,
    )
    .await
    .map(Json)
}

#[post("/fill-spot", format = "json", data = "<body>")]
pub async fn fill_spot(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<FillSpotRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::fill_spot(db, &token?._id.to_string(), &body.name, &body.player)
        .await
        .map(Json)
}

#[post("/protect-players", format = "json", data = "<body>")]
pub async fn protect_players(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<ProtectPlayersRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::protect_players(
        db,
        &token?._id.to_string(),
        &body.name,
        &body.forw_protected,
        &body.def_protected,
        &body.goal_protected,
        &body.reserv_protected,
    )
    .await
    .map(Json)
}

#[post("/modify-roster", format = "json", data = "<body>")]
pub async fn modify_roster(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<ModifyRosterRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::modify_roster(
        db,
        &token?._id.to_string(),
        &body.name,
        &body.user_id,
        &body.forw_list,
        &body.def_list,
        &body.goal_list,
        &body.reserv_list,
    )
    .await
    .map(Json)
}

#[post("/update-pool-settings", format = "json", data = "<body>")]
pub async fn update_pool_settings(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<UpdatePoolSettingsRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::update_pool_settings(db, &token?._id.to_string(), &body)
        .await
        .map(Json)
}
