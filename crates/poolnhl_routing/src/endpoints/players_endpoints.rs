use axum::extract::{Json, Path, Query, State};
use axum::routing::get;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;

use poolnhl_interface::errors::Result;
use poolnhl_interface::players::model::{GetPlayerQuery, PlayerInfo};
use poolnhl_interface::players::service::PlayersServiceHandle;

pub struct PlayersRouter;

impl PlayersRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/get-players", get(Self::get_players))
            .route("/get-players/:name", get(Self::get_players_with_name))
            .with_state(service_registry)
    }

    async fn get_players(
        State(players_service): State<PlayersServiceHandle>,
        Query(params): Query<GetPlayerQuery>,
    ) -> Result<Json<Vec<PlayerInfo>>> {
        players_service.get_players(params).await.map(Json)
    }

    async fn get_players_with_name(
        State(players_service): State<PlayersServiceHandle>,
        Path(name): Path<String>,
    ) -> Result<Json<Vec<PlayerInfo>>> {
        players_service.get_players_with_name(&name).await.map(Json)
    }
}
