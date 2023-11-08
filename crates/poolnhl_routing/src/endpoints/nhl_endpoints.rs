use axum::extract::{Json, Path, State};
use axum::routing::get;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;

use poolnhl_interface::errors::Result;
use poolnhl_interface::nhl::model::{DailyGames, GameBoxScore, GameLanding};
use poolnhl_interface::nhl::service::NhlServiceHandle;

pub struct NhlRouter;

impl NhlRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/daily_games/:date", get(Self::get_daily_games))
            .route("/game/landing/:id", get(Self::get_game_landing))
            .route("/game/boxscore/:id", get(Self::get_game_box_score))
            .with_state(service_registry)
    }

    async fn get_daily_games(
        State(nhl_service): State<NhlServiceHandle>,
        Path(date): Path<String>,
    ) -> Result<Json<DailyGames>> {
        nhl_service.get_daily_games(&date).await.map(Json)
    }

    async fn get_game_landing(
        State(nhl_service): State<NhlServiceHandle>,
        Path(id): Path<u32>,
    ) -> Result<Json<GameLanding>> {
        nhl_service.get_game_landing(id).await.map(Json)
    }

    async fn get_game_box_score(
        State(nhl_service): State<NhlServiceHandle>,
        Path(id): Path<u32>,
    ) -> Result<Json<GameBoxScore>> {
        nhl_service.get_game_box_score(id).await.map(Json)
    }
}
