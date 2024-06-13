use axum::extract::{Json, Path, State};
use axum::routing::{get, post};
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{
    AddPlayerRequest, CreateTradeRequest, DeleteTradeRequest, FillSpotRequest,
    GenerateDynastieRequest, MarkAsFinalRequest, ModifyRosterRequest, Pool, PoolCreationRequest,
    PoolDeletionRequest, ProjectedPoolShort, ProtectPlayersRequest, RemovePlayerRequest,
    RespondTradeRequest, UpdatePoolSettingsRequest,
};
use poolnhl_interface::pool::service::PoolServiceHandle;
use poolnhl_interface::users::model::UserEmailJwtPayload;

pub struct PoolRouter;

impl PoolRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/pool/:name", get(Self::get_pool_by_name))
            .route(
                "/pool/:name/:start_date/:from",
                get(Self::get_pool_by_name_with_range),
            )
            .route("/pools/:season", get(Self::get_pools))
            .route("/create-pool", post(Self::create_pool))
            .route("/delete-pool", post(Self::delete_pool))
            .route("/add-player", post(Self::add_player))
            .route("/remove-player", post(Self::remove_player))
            .route("/create-trade", post(Self::create_trade))
            .route("/delete-trade", post(Self::delete_trade))
            .route("/respond-trade", post(Self::respond_trade))
            .route("/fill-spot", post(Self::fill_spot))
            .route("/protect-players", post(Self::protect_players))
            .route("/modify-roster", post(Self::modify_roster))
            .route("/update-pool-settings", post(Self::update_pool_settings))
            .route("/mark-as-final", post(Self::mark_as_final))
            .route("/generate-dynasty", post(Self::generate_dynasty))
            .with_state(service_registry)
    }

    async fn get_pool_by_name(
        Path(name): Path<String>,
        State(pool_service): State<PoolServiceHandle>,
    ) -> Result<Json<Pool>> {
        pool_service.get_pool_by_name(&name).await.map(Json)
    }

    async fn get_pool_by_name_with_range(
        Path((name, start_date, from)): Path<(String, String, String)>,
        State(pool_service): State<PoolServiceHandle>,
    ) -> Result<Json<Pool>> {
        pool_service
            .get_pool_by_name_with_range(&name, &start_date, &from)
            .await
            .map(Json)
    }

    /// get all Pool documents but only part of the information.
    async fn get_pools(
        Path(season): Path<u32>,
        State(pool_service): State<PoolServiceHandle>,
    ) -> Result<Json<Vec<ProjectedPoolShort>>> {
        print!("{}", season);
        pool_service.list_pools(season).await.map(Json)
    }

    async fn create_pool(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<PoolCreationRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.create_pool(&token.sub, body).await.map(Json)
    }

    async fn delete_pool(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<PoolDeletionRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.delete_pool(&token.sub, body).await.map(Json)
    }

    async fn add_player(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<AddPlayerRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.add_player(&token.sub, body).await.map(Json)
    }

    async fn remove_player(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<RemovePlayerRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.remove_player(&token.sub, body).await.map(Json)
    }

    async fn create_trade(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(mut body): Json<CreateTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .create_trade(&token.sub, &mut body)
            .await
            .map(Json)
    }

    async fn delete_trade(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<DeleteTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.delete_trade(&token.sub, body).await.map(Json)
    }

    async fn respond_trade(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<RespondTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.respond_trade(&token.sub, body).await.map(Json)
    }

    async fn fill_spot(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<FillSpotRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.fill_spot(&token.sub, body).await.map(Json)
    }

    async fn protect_players(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<ProtectPlayersRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .protect_players(&token.sub, body)
            .await
            .map(Json)
    }

    async fn modify_roster(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<ModifyRosterRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.modify_roster(&token.sub, body).await.map(Json)
    }

    async fn update_pool_settings(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<UpdatePoolSettingsRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .update_pool_settings(&token.sub, body)
            .await
            .map(Json)
    }

    async fn mark_as_final(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<MarkAsFinalRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.mark_as_final(&token.sub, body).await.map(Json)
    }
    async fn generate_dynasty(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<GenerateDynastieRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .generate_dynasty(&token.sub, body)
            .await
            .map(Json)
    }
}
