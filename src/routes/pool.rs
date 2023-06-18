use crate::database::CONNECTION;
use crate::db::pool;
use crate::errors::response::AppError;
use crate::models::pool::{
    AddPlayerRequest, CancelTradeRequest, CreateTradeRequest, FillSpotRequest, ModifyRosterRequest,
    Pool, PoolCreationRequest, PoolDeletionRequest, PoolUndoSelectionRequest, ProjectedPoolShort,
    ProtectPlayersRequest, RemovePlayerRequest, RespondTradeRequest, SelectPlayerRequest,
    StartDraftRequest, UpdatePoolSettingsRequest,
};
use crate::models::response::PoolMessageResponse;
use crate::routes::jwt::UserToken;
use axum::{
    extract::Path,
    routing::{get, post},
    Json, Router,
};

pub fn create_route() -> Router {
    Router::new()
        .route("/pool/:name", get(get_pool_by_name))
        .route("/pool/:name/:from", get(get_pool_by_name_with_range))
        .route("/pools", get(get_pools))
        .route("/create-pool", post(create_pool))
        .route("/delete-pool", post(delete_pool))
        .route("/start-draft", post(start_draft))
        .route("/select-player", post(select_player))
        .route("/add-player", post(add_player))
        .route("/remove-player", post(remove_player))
        .route("/undo_select-player", post(undo_select_player))
        .route("/create-trade", post(create_trade))
        .route("/cancel-trade", post(cancel_trade))
        .route("/respond-trade", post(respond_trade))
        .route("/fill-spot", post(fill_spot))
        .route("/protect-players", post(protect_players))
        .route("/modify-roster", post(modify_roster))
        .route("/update-pool-settings", post(update_pool_settings))
}

/// get Pool document by _name
async fn get_pool_by_name(Path(_name): Path<String>) -> Result<Json<Pool>, AppError> {
    pool::find_pool_by_name(CONNECTION.get().await, &_name)
        .await
        .map(Json)
}

/// get Pool document by _name
async fn get_pool_by_name_with_range(
    Path((_name, _from)): Path<(String, String)>,
) -> Result<Json<Pool>, AppError> {
    pool::find_pool_by_name_with_range(CONNECTION.get().await, &_name, &_from)
        .await
        .map(Json)
}

/// get all Pool documents but only part of the information.
async fn get_pools() -> Result<Json<Vec<ProjectedPoolShort>>, AppError> {
    pool::find_pools(CONNECTION.get().await).await.map(Json)
}

async fn create_pool(
    token: UserToken,
    Json(body): Json<PoolCreationRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::create_pool(CONNECTION.get().await, token._id.to_string(), body)
        .await
        .map(Json)
}

async fn delete_pool(
    token: UserToken,
    Json(body): Json<PoolDeletionRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::delete_pool(CONNECTION.get().await, &token._id.to_string(), &body.name)
        .await
        .map(Json)
}

async fn start_draft(
    token: UserToken,
    Json(mut body): Json<StartDraftRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::start_draft(
        CONNECTION.get().await,
        &token._id.to_string(),
        &mut body.poolInfo,
    )
    .await
    .map(Json)
}

async fn select_player(
    token: UserToken,
    Json(body): Json<SelectPlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::select_player(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.name,
        &body.player,
    )
    .await
    .map(Json)
}

async fn add_player(
    token: UserToken,
    Json(body): Json<AddPlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::add_player(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.user_id,
        &body.name,
        &body.player,
    )
    .await
    .map(Json)
}

async fn remove_player(
    token: UserToken,
    Json(body): Json<RemovePlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::remove_player(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.user_id,
        &body.name,
        body.player_id,
    )
    .await
    .map(Json)
}

async fn undo_select_player(
    token: UserToken,
    Json(body): Json<PoolUndoSelectionRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::undo_select_player(CONNECTION.get().await, &token._id.to_string(), &body.name)
        .await
        .map(Json)
}

async fn create_trade(
    token: UserToken,
    Json(body): Json<CreateTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::create_trade(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.name,
        &mut body.trade.clone(),
    )
    .await
    .map(Json)
}

async fn cancel_trade(
    token: UserToken,
    Json(body): Json<CancelTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::cancel_trade(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.name,
        body.trade_id,
    )
    .await
    .map(Json)
}

async fn respond_trade(
    token: UserToken,
    Json(body): Json<RespondTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::respond_trade(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.name,
        body.is_accepted,
        body.trade_id,
    )
    .await
    .map(Json)
}

async fn fill_spot(
    token: UserToken,
    Json(body): Json<FillSpotRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::fill_spot(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.user_id,
        &body.name,
        body.player_id,
    )
    .await
    .map(Json)
}

async fn protect_players(
    token: UserToken,
    Json(body): Json<ProtectPlayersRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::protect_players(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.name,
        &body.forw_protected,
        &body.def_protected,
        &body.goal_protected,
        &body.reserv_protected,
    )
    .await
    .map(Json)
}

async fn modify_roster(
    token: UserToken,
    Json(body): Json<ModifyRosterRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::modify_roster(
        CONNECTION.get().await,
        &token._id.to_string(),
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

async fn update_pool_settings(
    token: UserToken,
    Json(body): Json<UpdatePoolSettingsRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::update_pool_settings(CONNECTION.get().await, &token._id.to_string(), &body)
        .await
        .map(Json)
}
