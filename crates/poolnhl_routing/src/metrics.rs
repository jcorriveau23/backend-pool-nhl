//! Prometheus instrumentation.
//!
//! Served from its own listener on a separate port, never from the application
//! router. Caddy only proxies `/api-rust/*` to the app port, so a metrics route
//! mounted there would be one routing mistake away from public — and these
//! series carry pool names as labels. The metrics port is published to the
//! compose network only.

use std::net::SocketAddr;
use std::time::Duration;

use axum::{Router, routing::get};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

// The names themselves are shared with poolnhl_infrastructure, which also
// emits; re-exported so `app_metrics::X` keeps resolving throughout routing.
pub use poolnhl_interface::metrics::*;

// Buckets in seconds. Tuned low: a draft pick that takes a second is already a
// visibly laggy room, so the interesting resolution is all under ~2s, with a
// couple of buckets above to catch the pathological case.
const LATENCY_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Installs the global recorder. Must run before any metric is emitted —
/// anything recorded earlier goes to the no-op recorder and is lost.
///
/// Returns the handle the `/metrics` route renders from.
pub fn install_recorder() -> Result<PrometheusHandle, String> {
    PrometheusBuilder::new()
        .set_buckets_for_metric(Matcher::Full(COMMAND_DURATION.to_string()), LATENCY_BUCKETS)
        .map_err(|e| e.to_string())?
        .set_buckets_for_metric(
            Matcher::Full(HTTP_REQUEST_DURATION.to_string()),
            LATENCY_BUCKETS,
        )
        .map_err(|e| e.to_string())?
        .set_buckets_for_metric(
            Matcher::Full(WS_SEND_BLOCKED_SECONDS.to_string()),
            LATENCY_BUCKETS,
        )
        .map_err(|e| e.to_string())?
        // Counters and gauges are never dropped, but histograms are only
        // rendered while they hold samples. Without an idle timeout a room that
        // drafted once keeps reporting a frozen histogram forever.
        .idle_timeout(
            metrics_util::MetricKindMask::HISTOGRAM,
            Some(Duration::from_secs(10 * 60)),
        )
        .install_recorder()
        .map_err(|e| e.to_string())
}

/// Metadata for the exposition format. Purely descriptive, but it is what makes
/// the metrics readable in Grafana's explorer without cross-referencing here.
pub fn describe_metrics() {
    use metrics::{Unit, describe_counter, describe_gauge, describe_histogram};

    describe_gauge!(
        WS_CONNECTIONS,
        "Draft websockets currently open on this instance"
    );
    describe_counter!(WS_CONNECTED_TOTAL, "Draft websockets accepted");
    describe_counter!(
        WS_CLOSED_TOTAL,
        "Draft websockets closed, by reason. `no_join` and `join_failed` never reached a room"
    );
    describe_gauge!(
        ROOMS_ACTIVE,
        "Draft rooms with at least one socket on this instance"
    );
    describe_counter!(
        WS_LAGGED_TOTAL,
        "Times a socket fell behind its room's broadcast buffer. Non-zero means clients are missing draft deltas and refetching"
    );
    describe_counter!(
        WS_LAGGED_MESSAGES_TOTAL,
        "Broadcast messages skipped by lagging sockets"
    );
    describe_histogram!(
        WS_SEND_BLOCKED_SECONDS,
        Unit::Seconds,
        "Time spent blocked writing to a socket's outbound queue. Rises when a client cannot drain as fast as the room produces"
    );
    describe_counter!(
        COMMAND_TOTAL,
        "Draft commands handled, by command and outcome"
    );
    describe_histogram!(
        COMMAND_DURATION,
        Unit::Seconds,
        "Wall time handling a draft command, including its mongo and redis work"
    );
    describe_counter!(HTTP_REQUESTS_TOTAL, "HTTP requests, by method/path/status");
    describe_histogram!(
        HTTP_REQUEST_DURATION,
        Unit::Seconds,
        "HTTP request duration"
    );
}

/// Materialises the series that would otherwise be absent until they first
/// fire.
///
/// The prometheus exporter only renders a metric once it has been touched, so a
/// backend that has not yet lagged omits `draft_ws_lagged_total` entirely and
/// the panel reads "No data" — indistinguishable from a broken metric or a
/// misspelled name. Zero is the honest and much more useful answer, and it is
/// the value you most want to be able to trust at a glance during a draft.
///
/// Only the label-free series and the closed label sets are pre-registered.
/// Anything keyed by command or pool appears on first use, as it should.
pub fn init_zero_values() {
    use metrics::{counter, gauge};

    gauge!(WS_CONNECTIONS).set(0.0);
    gauge!(ROOMS_ACTIVE).set(0.0);
    counter!(WS_CONNECTED_TOTAL).increment(0);
    counter!(WS_LAGGED_TOTAL).increment(0);
    counter!(WS_LAGGED_MESSAGES_TOTAL).increment(0);

    // Closed set, so the stacked "closes by reason" panel has a stable series
    // list from the first scrape rather than growing as each reason first hits.
    for reason in [
        "client",
        "keepalive_timeout",
        "broadcast_closed",
        "no_join_timeout",
        "join_failed",
        "unknown",
    ] {
        counter!(WS_CLOSED_TOTAL, &[("reason", reason)]).increment(0);
    }
}

/// Serves `/metrics` on its own port until the process exits.
pub async fn serve(handle: PrometheusHandle, addr: SocketAddr) {
    // Rendering walks every series, so it is done per request rather than on a
    // timer; prometheus scrapes this every 15s.
    let app = Router::new()
        .route(
            "/metrics",
            get(move || {
                let handle = handle.clone();
                async move { handle.render() }
            }),
        )
        // A bare liveness probe that does not depend on mongo or redis being
        // reachable, unlike the app port's /health.
        .route("/health", get(|| async { "ok" }));

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            // Deliberately not fatal. Losing metrics is not a reason to take
            // the site down mid-draft, which is exactly when this would bite.
            tracing::error!(%addr, error = %e, "could not bind the metrics listener; continuing without it");
            return;
        }
    };

    tracing::info!(%addr, "metrics listening");
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!(error = %e, "metrics server stopped");
    }
}
