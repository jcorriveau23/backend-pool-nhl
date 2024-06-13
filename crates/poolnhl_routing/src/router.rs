use crate::logger;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_infrastructure::settings::Settings;
use tower_http::trace::TraceLayer;

use crate::endpoints::daily_leaders_endpoints::DailyLeadersRouter;
use crate::endpoints::draft_endpoints::DraftRouter;
use crate::endpoints::nhl_endpoints::NhlRouter;
use crate::endpoints::pool_endpoints::PoolRouter;

pub struct ApplicationController;

impl ApplicationController {
    pub async fn run(settings: Settings, service_registry: ServiceRegistry) {
        logger::setup(&settings.logger.level);

        let router: Router = Router::new()
            .nest(
                "/api-rust",
                Router::new()
                    .merge(PoolRouter::new(service_registry.clone()))
                    .merge(DraftRouter::new(service_registry.clone()))
                    .merge(DailyLeadersRouter::new(service_registry.clone()))
                    .merge(NhlRouter::new(service_registry.clone())),
            )
            .layer(TraceLayer::new_for_http());

        let listener =
            tokio::net::TcpListener::bind(&format!("127.0.0.1:{}", settings.server.port))
                .await
                .unwrap();
        axum::serve(listener, router)
            .await
            .expect("Failed to start the server");
    }
}
