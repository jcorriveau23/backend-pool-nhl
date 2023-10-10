use axum::extract::{Json, State};
use axum::routing::get;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::daily_leaders::service::DailyLeadersServiceHandle;

use poolnhl_interface::daily_leaders::model::DailyLeaders;
use poolnhl_interface::errors::Result;

pub struct DailyLeadersRouter;

impl DailyLeadersRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/daily_leaders/:date", get(Self::get_daily_leaders))
            .with_state(service_registry)
    }

    // Get the daily pointers of a specific date.
    // This allow to display in the web app all the pointers of a specific date.
    async fn get_daily_leaders(
        State(daily_leaders_service): State<DailyLeadersServiceHandle>,
        date: String,
    ) -> Result<Json<DailyLeaders>> {
        daily_leaders_service
            .get_daily_leaders(&date)
            .await
            .map(Json)
    }
}
