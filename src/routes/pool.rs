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
use crate::AppState;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

pub fn create_route() -> Router<AppState> {
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
async fn get_pool_by_name(
    state: State<AppState>,
    Path(_name): Path<String>,
) -> Result<Json<Pool>, AppError> {
    pool::find_pool_by_name(&state.db, &_name).await.map(Json)
}

/// get Pool document by _name
async fn get_pool_by_name_with_range(
    state: State<AppState>,
    Path((_name, _from)): Path<(String, String)>,
) -> Result<Json<Pool>, AppError> {
    pool::find_pool_by_name_with_range(&state.db, &_name, &_from)
        .await
        .map(Json)
}

/// get all Pool documents but only part of the information.
async fn get_pools(state: State<AppState>) -> Result<Json<Vec<ProjectedPoolShort>>, AppError> {
    pool::find_pools(&state.db).await.map(Json)
}

async fn create_pool(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<PoolCreationRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::create_pool(&state.db, token._id.to_string(), body)
        .await
        .map(Json)
}

async fn delete_pool(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<PoolDeletionRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::delete_pool(&state.db, &token._id.to_string(), &body.name)
        .await
        .map(Json)
}

async fn start_draft(
    state: State<AppState>,
    token: UserToken,
    Json(mut body): Json<StartDraftRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::start_draft(&state.db, &token._id.to_string(), &mut body.poolInfo)
        .await
        .map(Json)
}

async fn select_player(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<SelectPlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::select_player(&state.db, &token._id.to_string(), &body.name, &body.player)
        .await
        .map(Json)
}

async fn add_player(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<AddPlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::add_player(
        &state.db,
        &token._id.to_string(),
        &body.user_id,
        &body.name,
        &body.player,
    )
    .await
    .map(Json)
}

async fn remove_player(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<RemovePlayerRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::remove_player(
        &state.db,
        &token._id.to_string(),
        &body.user_id,
        &body.name,
        body.player_id,
    )
    .await
    .map(Json)
}

async fn undo_select_player(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<PoolUndoSelectionRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::undo_select_player(&state.db, &token._id.to_string(), &body.name)
        .await
        .map(Json)
}

async fn create_trade(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<CreateTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::create_trade(
        &state.db,
        &token._id.to_string(),
        &body.name,
        &mut body.trade.clone(),
    )
    .await
    .map(Json)
}

async fn cancel_trade(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<CancelTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::cancel_trade(&state.db, &token._id.to_string(), &body.name, body.trade_id)
        .await
        .map(Json)
}

async fn respond_trade(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<RespondTradeRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::respond_trade(
        &state.db,
        &token._id.to_string(),
        &body.name,
        body.is_accepted,
        body.trade_id,
    )
    .await
    .map(Json)
}

async fn fill_spot(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<FillSpotRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::fill_spot(
        &state.db,
        &token._id.to_string(),
        &body.user_id,
        &body.name,
        body.player_id,
    )
    .await
    .map(Json)
}

async fn protect_players(
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<ProtectPlayersRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::protect_players(
        &state.db,
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
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<ModifyRosterRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::modify_roster(
        &state.db,
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
    state: State<AppState>,
    token: UserToken,
    Json(body): Json<UpdatePoolSettingsRequest>,
) -> Result<Json<PoolMessageResponse>, AppError> {
    pool::update_pool_settings(&state.db, &token._id.to_string(), &body)
        .await
        .map(Json)
}
