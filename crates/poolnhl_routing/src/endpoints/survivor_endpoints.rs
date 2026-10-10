use axum::Router;
use axum::extract::{Json, Path, Query, State};
use axum::routing::{get, post};

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::errors::Result;
use poolnhl_interface::survivor::model::{SurvivorPool, SurvivorPoolShort, SurvivorStandings};
use poolnhl_interface::survivor::picks::{SurvivorPickOptions, SurvivorPickView};
use poolnhl_interface::survivor::requests::{
    AddSurvivorParticipantRequest, JoinSurvivorRequest, LeaveSurvivorRequest, MakePickRequest,
    PickOptionsQuery, SettleWeekRequest, SurvivorCreationRequest, SurvivorDeletionRequest,
    UpdateSurvivorSettingsRequest,
};
use poolnhl_interface::survivor::service::SurvivorServiceHandle;
use poolnhl_interface::users::model::UserEmailJwtPayload;

pub struct SurvivorRouter;

impl SurvivorRouter {
    pub fn router(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/survivor/:name", get(Self::get_pool_by_name))
            .route("/survivor-pools/:season", get(Self::get_pools))
            .route("/survivor/:name/standings", get(Self::get_standings))
            // The pick screen and the reveal. Both need the caller's identity:
            // what is open to them differs per participant, and an open date's
            // picks are theirs alone to see.
            .route(
                "/survivor/:name/pick-options/:week",
                get(Self::get_pick_options),
            )
            .route("/survivor/:name/picks/:week", get(Self::get_week_picks))
            .route("/survivor/:name/my-picks", get(Self::get_my_picks))
            .route("/create-survivor-pool", post(Self::create_pool))
            .route("/delete-survivor-pool", post(Self::delete_pool))
            .route("/update-survivor-settings", post(Self::update_settings))
            .route("/join-survivor-pool", post(Self::join_pool))
            .route("/add-survivor-participant", post(Self::add_participant))
            .route("/leave-survivor-pool", post(Self::leave_pool))
            .route("/survivor-pick", post(Self::make_pick))
            .route("/lock-survivor-week", post(Self::lock_week))
            .route("/settle-survivor-week", post(Self::settle_week))
            .with_state(service_registry)
    }

    async fn get_pool_by_name(
        Path(name): Path<String>,
        State(survivor_service): State<SurvivorServiceHandle>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service.get_pool_by_name(&name).await.map(Json)
    }

    async fn get_pools(
        Path(season): Path<u32>,
        State(survivor_service): State<SurvivorServiceHandle>,
    ) -> Result<Json<Vec<SurvivorPoolShort>>> {
        survivor_service.list_pools(season).await.map(Json)
    }

    /// The standings, counted server-side: a pool of hundreds would otherwise
    /// ship every pick of every date for the browser to aggregate.
    async fn get_standings(
        Path(name): Path<String>,
        State(survivor_service): State<SurvivorServiceHandle>,
    ) -> Result<Json<SurvivorStandings>> {
        survivor_service.get_standings(&name).await.map(Json)
    }

    /// What this participant may pick for a date, and what they already have in.
    async fn get_pick_options(
        token: UserEmailJwtPayload,
        Path((name, week)): Path<(String, u16)>,
        Query(query): Query<PickOptionsQuery>,
        State(survivor_service): State<SurvivorServiceHandle>,
    ) -> Result<Json<SurvivorPickOptions>> {
        survivor_service
            .get_pick_options(&token.sub, &name, week, query.participant_id.as_deref())
            .await
            .map(Json)
    }

    /// A date's picks. Only the caller's own until the date locks — see
    /// `get_week_picks` on the service for why.
    async fn get_week_picks(
        token: UserEmailJwtPayload,
        Path((name, week)): Path<(String, u16)>,
        State(survivor_service): State<SurvivorServiceHandle>,
    ) -> Result<Json<Vec<SurvivorPickView>>> {
        survivor_service
            .get_week_picks(&token.sub, &name, week)
            .await
            .map(Json)
    }

    async fn get_my_picks(
        token: UserEmailJwtPayload,
        Path(name): Path<String>,
        State(survivor_service): State<SurvivorServiceHandle>,
    ) -> Result<Json<Vec<SurvivorPickView>>> {
        survivor_service
            .get_my_picks(&token.sub, &name)
            .await
            .map(Json)
    }

    async fn create_pool(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<SurvivorCreationRequest>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service
            .create_pool(&token.sub, body)
            .await
            .map(Json)
    }

    async fn delete_pool(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<SurvivorDeletionRequest>,
    ) -> Result<()> {
        survivor_service.delete_pool(&token.sub, body).await
    }

    async fn update_settings(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<UpdateSurvivorSettingsRequest>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service
            .update_settings(&token.sub, body)
            .await
            .map(Json)
    }

    /// Sign yourself up.
    ///
    /// Self-serve on purpose: a pool for hundreds of people cannot have its
    /// owner enter every name by hand the way the roster pool does. The
    /// participant is whoever the JWT names, never a body field, so a caller
    /// cannot sign anybody else up.
    async fn join_pool(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<JoinSurvivorRequest>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service.join_pool(&token.sub, body).await.map(Json)
    }

    /// Add a spot the organiser keeps on somebody's behalf.
    ///
    /// The counterpart to self-serve joining, for the pool of a few friends
    /// where one person enters everybody. The participant it creates has no
    /// account, so the organiser files their picks too — see `make_pick`.
    async fn add_participant(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<AddSurvivorParticipantRequest>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service
            .add_participant(&token.sub, body)
            .await
            .map(Json)
    }

    async fn leave_pool(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<LeaveSurvivorRequest>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service
            .leave_pool(&token.sub, body)
            .await
            .map(Json)
    }

    /// Name the team you are backing. Answers with the pick screen as it now
    /// stands, so the client does not have to re-derive what is left open.
    async fn make_pick(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<MakePickRequest>,
    ) -> Result<Json<SurvivorPickOptions>> {
        survivor_service.make_pick(&token.sub, body).await.map(Json)
    }

    async fn lock_week(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<SettleWeekRequest>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service.lock_week(&token.sub, body).await.map(Json)
    }

    /// Close a date and apply what the games did.
    ///
    /// Who won is read from the league's own scoreboard inside the service, not
    /// taken from this request: a caller who could report a result could report
    /// their own win.
    async fn settle_week(
        token: UserEmailJwtPayload,
        State(survivor_service): State<SurvivorServiceHandle>,
        Json(body): Json<SettleWeekRequest>,
    ) -> Result<Json<SurvivorPool>> {
        survivor_service
            .settle_week(&token.sub, body)
            .await
            .map(Json)
    }
}
