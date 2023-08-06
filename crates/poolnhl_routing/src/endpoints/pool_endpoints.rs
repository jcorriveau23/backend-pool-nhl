use axum::extract::{Json, Path, State};
use axum::routing::{get, post};
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{
    AddPlayerRequest, CreateTradeRequest, DeleteTradeRequest, FillSpotRequest, ModifyRosterRequest,
    Pool, PoolCreationRequest, PoolDeletionRequest, ProjectedPoolShort, ProtectPlayersRequest,
    RemovePlayerRequest, RespondTradeRequest, UpdatePoolSettingsRequest,
};
use poolnhl_interface::pool::service::PoolServiceHandle;

use poolnhl_infrastructure::jwt::UserToken;

pub struct PoolRouter;

impl PoolRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/pool/:name", get(PoolRouter::get_pool_by_name))
            .route(
                "/pool/:name/:from",
                get(PoolRouter::get_pool_by_name_with_range),
            )
            .route("/pools", get(PoolRouter::get_pools))
            .route("/create-pool", post(PoolRouter::create_pool))
            .route("/delete-pool", post(PoolRouter::delete_pool))
            .route("/add-player", post(PoolRouter::add_player))
            .route("/remove-player", post(PoolRouter::remove_player))
            .route("/create-trade", post(PoolRouter::create_trade))
            .route("/delete-trade", post(PoolRouter::delete_trade))
            .route("/respond-trade", post(PoolRouter::respond_trade))
            .route("/fill-spot", post(PoolRouter::fill_spot))
            .route("/protect-players", post(PoolRouter::protect_players))
            .route("/modify-roster", post(PoolRouter::modify_roster))
            .route(
                "/update-pool-settings",
                post(PoolRouter::update_pool_settings),
            )
            .with_state(service_registry)
    }

    async fn get_pool_by_name(
        Path(name): Path<String>,
        State(pool_service): State<PoolServiceHandle>,
    ) -> Result<Json<Pool>> {
        pool_service.get_pool_by_name(&name).await.map(Json)
    }

    async fn get_pool_by_name_with_range(
        Path((name, from)): Path<(String, String)>,
        State(pool_service): State<PoolServiceHandle>,
    ) -> Result<Json<Pool>> {
        pool_service
            .get_pool_by_name_with_range(&name, &from)
            .await
            .map(Json)
    }

    /// get all Pool documents but only part of the information.
    async fn get_pools(
        State(pool_service): State<PoolServiceHandle>,
    ) -> Result<Json<Vec<ProjectedPoolShort>>> {
        pool_service.list_pools().await.map(Json)
    }

    async fn create_pool(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<PoolCreationRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .create_pool(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn delete_pool(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<PoolDeletionRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .delete_pool(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn add_player(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<AddPlayerRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .add_player(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn remove_player(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<RemovePlayerRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .remove_player(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn create_trade(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(mut body): Json<CreateTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .create_trade(&token._id.to_string(), &mut body)
            .await
            .map(Json)
    }

    async fn delete_trade(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<DeleteTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .delete_trade(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn respond_trade(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<RespondTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .respond_trade(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn fill_spot(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<FillSpotRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .fill_spot(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn protect_players(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<ProtectPlayersRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .protect_players(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn modify_roster(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<ModifyRosterRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .modify_roster(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    async fn update_pool_settings(
        token: UserToken,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<UpdatePoolSettingsRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .update_pool_settings(&token._id.to_string(), body)
            .await
            .map(Json)
    }
}
