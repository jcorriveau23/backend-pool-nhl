mod database;
mod db;
mod errors;
mod logger;
mod models;
mod routes;
mod settings;
use std::net::SocketAddr;
use tower_http::trace;
use tracing;

use axum::Router;
use settings::SETTINGS;

#[tokio::main]
async fn main() {
    logger::setup();
    let router = Router::new().merge(routes::user::create_route()).merge(
        Router::new()
            .nest(
                "/api-rust",
                // All public routes are nested here.
                Router::new()
                    .merge(routes::user::create_route())
                    .merge(routes::pool::create_route())
                    .merge(routes::auth::create_route())
                    .merge(routes::daily_leaders::create_route()),
            )
            .layer(
                trace::TraceLayer::new_for_http()
                    .on_response(trace::DefaultOnResponse::new().level(tracing::Level::INFO)),
            ),
    );

    let port = SETTINGS.server.port;
    let address = SocketAddr::from(([127, 0, 0, 1], port));

    println!("Server listening on {}", &address);
    axum::Server::bind(&address)
        .serve(router.into_make_service())
        .await
        .expect("Failed to start server");
}
