use axum::extract::{Json, State};
use axum::routing::post;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::draft::model::{
    SelectPlayerRequest, StartDraftRequest, UndoSelectionRequest,
};
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::Pool;

use poolnhl_infrastructure::jwt::UserToken;

pub struct DraftRouter;

impl DraftRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/start-draft", post(DraftRouter::start_draft))
            .route("/draft-player", post(DraftRouter::draft_player))
            .route("/undo-draft-player", post(DraftRouter::undo_draft_player))
            .with_state(service_registry)
    }

    // Initiate a draft.
    async fn start_draft(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(mut body): Json<StartDraftRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .start_draft(&token._id.to_string(), &mut body)
            .await
            .map(Json)
    }

    // Draft a player.
    async fn draft_player(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(body): Json<SelectPlayerRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .draft_player(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    // Undo the last draft player action.
    async fn undo_draft_player(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(body): Json<UndoSelectionRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .undo_draft_player(&token._id.to_string(), body)
            .await
            .map(Json)
    }
}
