use std::collections::HashMap;

use axum::Router;
use axum::extract::{Json, Path, State};
use axum::routing::{get, post};

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_infrastructure::services::pool_scoring_service::PoolScoringService;
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{Pool, ProjectedPoolShort, SeasonInfo};
use poolnhl_interface::pool::requests::{
    AddPlayerRequest, CompleteProtectionRequest, ConfirmTradeRequest, CreateTradeRequest,
    DeleteTradeRequest, FillSpotRequest, GenerateDynastyRequest, MarkAsFinalRequest,
    ModifyRosterRequest, PoolCreationRequest, PoolDeletionRequest, ProtectPlayersRequest,
    RemovePlayerRequest, UpdatePoolSettingsRequest, UpdatePoolerNameRequest, UpdateTradeRequest,
};
use poolnhl_interface::pool::scoring::DailyRosterPoints;
use poolnhl_interface::pool::service::PoolServiceHandle;
use poolnhl_interface::users::model::UserEmailJwtPayload;

pub struct PoolRouter;

impl PoolRouter {
    pub fn router(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/pool/:name", get(Self::get_pool_by_name))
            .route(
                "/pool/:name/:start_date/:from",
                get(Self::get_pool_by_name_with_range),
            )
            // Scores derived on demand from the shared day_leaders. A distinct
            // prefix avoids a matchit 0.7 static-vs-param conflict with the
            // `/pool/:name/:start_date/:from` route above.
            .route(
                "/pool-scores/:name/daily/:date",
                get(Self::get_pool_daily_scores),
            )
            .route(
                "/pool-scores/:name/cumulative/:from/:to",
                get(Self::get_pool_cumulative_scores),
            )
            .route("/pools/:season", get(Self::get_pools))
            .route("/season-info", get(Self::get_season_info))
            .route("/create-pool", post(Self::create_pool))
            .route("/delete-pool", post(Self::delete_pool))
            .route("/add-player", post(Self::add_player))
            .route("/remove-player", post(Self::remove_player))
            .route("/create-trade", post(Self::create_trade))
            .route("/update-trade", post(Self::update_trade))
            .route("/confirm-trade", post(Self::confirm_trade))
            .route("/delete-trade", post(Self::delete_trade))
            .route("/fill-spot", post(Self::fill_spot))
            .route("/protect-players", post(Self::protect_players))
            .route("/complete-protection", post(Self::complete_protection))
            .route("/modify-roster", post(Self::modify_roster))
            .route("/update-pool-settings", post(Self::update_pool_settings))
            .route("/update-pooler-name", post(Self::update_pooler_name))
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

    /// Per-participant scoring breakdown for a single day, derived from the
    /// shared day_leaders. Same shape as one day of `score_by_day`.
    async fn get_pool_daily_scores(
        Path((name, date)): Path<(String, String)>,
        State(pool_service): State<PoolServiceHandle>,
        State(scoring): State<PoolScoringService>,
    ) -> Result<Json<HashMap<String, DailyRosterPoints>>> {
        let pool = pool_service.get_pool_by_name(&name).await?;
        scoring.derive_daily(&pool, &date).await.map(Json)
    }

    /// Per-participant scoring breakdown for every day in `[from, to]`, derived
    /// from the shared day_leaders. Feeds the cumulative/history views and graphs.
    async fn get_pool_cumulative_scores(
        Path((name, from, to)): Path<(String, String, String)>,
        State(pool_service): State<PoolServiceHandle>,
        State(scoring): State<PoolScoringService>,
    ) -> Result<Json<HashMap<String, HashMap<String, DailyRosterPoints>>>> {
        let pool = pool_service.get_pool_by_name(&name).await?;
        scoring.derive_range(&pool, &from, &to).await.map(Json)
    }

    /// get all Pool documents but only part of the information.
    async fn get_pools(
        Path(season): Path<u32>,
        State(pool_service): State<PoolServiceHandle>,
    ) -> Result<Json<Vec<ProjectedPoolShort>>> {
        pool_service.list_pools(season).await.map(Json)
    }

    /// Return the season date information (start/end dates, season number and
    /// trade deadline) the front end needs to render the current season.
    async fn get_season_info() -> Result<Json<SeasonInfo>> {
        Ok(Json(SeasonInfo::current()))
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

    async fn update_trade(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<UpdateTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.update_trade(&token.sub, body).await.map(Json)
    }

    async fn confirm_trade(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<ConfirmTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.confirm_trade(&token.sub, body).await.map(Json)
    }

    async fn delete_trade(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<DeleteTradeRequest>,
    ) -> Result<Json<Pool>> {
        pool_service.delete_trade(&token.sub, body).await.map(Json)
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
    async fn complete_protection(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<CompleteProtectionRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .complete_protection(&token.sub, body)
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

    async fn update_pooler_name(
        token: UserEmailJwtPayload,
        State(pool_service): State<PoolServiceHandle>,
        Json(body): Json<UpdatePoolerNameRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .update_pooler_name(&token.sub, body)
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
        Json(body): Json<GenerateDynastyRequest>,
    ) -> Result<Json<Pool>> {
        pool_service
            .generate_dynasty(&token.sub, body)
            .await
            .map(Json)
    }
}
