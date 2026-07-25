use std::net::SocketAddr;

use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_infrastructure::settings::Settings;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::trace::TraceLayer;

use crate::endpoints::daily_leaders_endpoints::DailyLeadersRouter;
use crate::endpoints::draft_endpoints::DraftRouter;
use crate::endpoints::players_endpoints::PlayersRouter;
use crate::endpoints::pool_endpoints::PoolRouter;

pub struct ApplicationController;

impl ApplicationController {
    pub async fn run(settings: Settings, service_registry: ServiceRegistry) {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .init();

        let router: Router = Router::new()
            .nest(
                "/api-rust",
                Router::new()
                    .merge(PoolRouter::router(service_registry.clone()))
                    .merge(DraftRouter::router(service_registry.clone()))
                    .merge(DailyLeadersRouter::router(service_registry.clone()))
                    .merge(PlayersRouter::router(service_registry.clone())),
            )
            // logging so we can see whats going on
            .layer(TraceLayer::new_for_http())
            // A panic in a handler (e.g. an unexpected data shape) returns a
            // 500 instead of silently dropping the connection.
            .layer(CatchPanicLayer::new());

        let listener = tokio::net::TcpListener::bind(&format!(
            "{}:{}",
            settings.server.host, settings.server.port
        ))
        .await
        .expect("Could not start the TCP listener");

        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("Failed to start the server");
    }
}
