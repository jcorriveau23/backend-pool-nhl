use std::net::SocketAddr;
use std::time::Instant;

use axum::Router;
use axum::extract::{MatchedPath, Request};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_infrastructure::settings::Settings;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use crate::metrics as app_metrics;

use crate::endpoints::daily_leaders_endpoints::DailyLeadersRouter;
use crate::endpoints::draft_endpoints::DraftRouter;
use crate::endpoints::players_endpoints::PlayersRouter;
use crate::endpoints::pool_endpoints::PoolRouter;
use crate::endpoints::version_endpoints::VersionRouter;

pub struct ApplicationController;

// Per-request counter and latency histogram.
//
// Labelled with the *matched route* (`/pools/:name`), never the concrete URI —
// one series per route instead of one per pool, and it keeps pool names out of
// the metrics entirely. It also means a stale client's `/ws/{jwt}` cannot mint
// a metric series per token.
async fn track_metrics(request: Request, next: Next) -> Response {
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        // An unmatched request is a 404; bucketing them all together avoids an
        // unbounded label from anyone probing random URLs.
        .unwrap_or_else(|| "unmatched".to_owned());
    let method = request.method().to_string();

    let start = Instant::now();
    let response = next.run(request).await;
    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [("method", method), ("path", path), ("status", status)];
    metrics::counter!(app_metrics::HTTP_REQUESTS_TOTAL, &labels).increment(1);
    metrics::histogram!(app_metrics::HTTP_REQUEST_DURATION, &labels).record(latency);

    response
}

impl ApplicationController {
    pub async fn run(settings: Settings, service_registry: ServiceRegistry) {
        // RUST_LOG wins when set (handy for a one-off debug session); otherwise
        // the level comes from config/{debug,release}.json.
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&settings.logger.level));

        tracing_subscriber::fmt().with_env_filter(filter).init();

        // Before anything can emit: metrics recorded ahead of this call go to
        // the no-op recorder and are silently lost.
        match app_metrics::install_recorder() {
            Ok(handle) => {
                app_metrics::describe_metrics();
                app_metrics::init_zero_values();
                let metrics_addr =
                    format!("{}:{}", settings.server.host, settings.server.metrics_port);
                match metrics_addr.parse::<SocketAddr>() {
                    Ok(addr) => {
                        tokio::spawn(app_metrics::serve(handle, addr));
                    }
                    Err(e) => {
                        tracing::error!(%metrics_addr, error = %e, "bad metrics address; continuing without the metrics listener");
                    }
                }
            }
            // Not fatal, for the same reason the listener bind is not: the
            // site keeps working without instrumentation.
            Err(e) => tracing::error!(error = %e, "could not install the metrics recorder"),
        }

        let router: Router = Router::new()
            .route("/health", get(|| async { "ok" }))
            .nest(
                "/api-rust",
                Router::new()
                    .merge(PoolRouter::router(service_registry.clone()))
                    .merge(DraftRouter::router(service_registry.clone()))
                    .merge(DailyLeadersRouter::router(service_registry.clone()))
                    .merge(PlayersRouter::router(service_registry.clone()))
                    // Stateless, so it takes no registry clone.
                    .merge(VersionRouter::router()),
            )
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(|request: &Request| {
                        tracing::info_span!(
                            "request",
                            method = %request.method(),
                            path = %request.uri().path(),
                        )
                    })
                    // tower_http logs these at DEBUG by default, which a
                    // production `info` level filters out entirely. An access
                    // log is exactly what you still want at `info`, so both are
                    // raised; drop the level in the config to silence them.
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            // Above CatchPanicLayer so a panicking handler is still recorded,
            // as the 500 the client actually receives.
            .layer(middleware::from_fn(track_metrics))
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


