use std::net::SocketAddr;

use crate::logger;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_infrastructure::settings::Settings;
use tower_http::trace;

use crate::endpoints::daily_leaders_endpoints::DailyLeadersRouter;
use crate::endpoints::draft_endpoints::DraftRouter;
use crate::endpoints::pool_endpoints::PoolRouter;
use crate::endpoints::users_endpoints::UsersRouter;

pub struct ApplicationController;

impl ApplicationController {
    pub async fn run(settings: Settings, service_registry: ServiceRegistry) {
        logger::setup(&settings.logger.level);

        let router: Router = Router::new()
            .nest(
                "/api-rust",
                Router::new()
                    .merge(UsersRouter::new(service_registry.clone()))
                    .merge(PoolRouter::new(service_registry.clone()))
                    .merge(DraftRouter::new(service_registry.clone()))
                    .merge(DailyLeadersRouter::new(service_registry.clone())),
            )
            .layer(
                trace::TraceLayer::new_for_http()
                    .on_response(trace::DefaultOnResponse::new().level(tracing::Level::DEBUG)),
            );

        axum::Server::bind(
            &format!("127.0.0.1:{}", settings.server.port)
                .parse()
                .unwrap(),
        )
        .serve(router.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("Failed to start the server");
    }
}
