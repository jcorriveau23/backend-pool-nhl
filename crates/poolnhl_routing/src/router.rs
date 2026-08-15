use std::net::SocketAddr;

use axum::Router;
use axum::extract::Request;
use axum::routing::get;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_infrastructure::settings::Settings;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use crate::endpoints::daily_leaders_endpoints::DailyLeadersRouter;
use crate::endpoints::draft_endpoints::DraftRouter;
use crate::endpoints::players_endpoints::PlayersRouter;
use crate::endpoints::pool_endpoints::PoolRouter;

pub struct ApplicationController;

// The draft socket carries its bearer token in the path (`/ws/{jwt}`), so the
// raw path must never reach a log sink. Everything after the `/ws/` prefix is
// replaced before the span is built.
fn redacted_path(path: &str) -> &str {
    if path.starts_with("/api-rust/ws/") {
        "/api-rust/ws/[redacted]"
    } else {
        path
    }
}

impl ApplicationController {
    pub async fn run(settings: Settings, service_registry: ServiceRegistry) {
        // RUST_LOG wins when set (handy for a one-off debug session); otherwise
        // the level comes from config/{debug,release}.json.
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&settings.logger.level));

        tracing_subscriber::fmt().with_env_filter(filter).init();

        let router: Router = Router::new()
            .route("/health", get(|| async { "ok" }))
            .nest(
                "/api-rust",
                Router::new()
                    .merge(PoolRouter::router(service_registry.clone()))
                    .merge(DraftRouter::router(service_registry.clone()))
                    .merge(DailyLeadersRouter::router(service_registry.clone()))
                    .merge(PlayersRouter::router(service_registry.clone())),
            )
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(|request: &Request| {
                        tracing::info_span!(
                            "request",
                            method = %request.method(),
                            path = %redacted_path(request.uri().path()),
                        )
                    })
                    // tower_http logs these at DEBUG by default, which a
                    // production `info` level filters out entirely. An access
                    // log is exactly what you still want at `info`, so both are
                    // raised; drop the level in the config to silence them.
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            // A panic in a handler (e.g. an unexpected data shape) returns a
            // 500 instead of silently dropping the connection.
            .layer(CatchPanicLayer::new())
            // Compress responses (gzip/br/deflate/zstd per Accept-Encoding) —
            // the pool-scores payloads are large and highly compressible.
            .layer(CompressionLayer::new());

        let addr = format!("{}:{}", settings.server.host, settings.server.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .expect("Could not start the TCP listener");

        tracing::info!(%addr, "server listening");

        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("Failed to start the server");
    }
}

#[cfg(test)]
mod tests {
    use super::redacted_path;

    #[test]
    fn the_socket_token_never_reaches_the_span() {
        assert_eq!(
            redacted_path("/api-rust/ws/eyJhbGciOiJSUzI1NiIsImtpZCI6IjEyMyJ9.payload.sig"),
            "/api-rust/ws/[redacted]"
        );
    }

    #[test]
    fn other_paths_are_untouched() {
        assert_eq!(
            redacted_path("/api-rust/pools/20262027"),
            "/api-rust/pools/20262027"
        );
    }
}
