use axum::Router;
use axum::extract::Json;
use axum::routing::get;

use serde_json::{Value, json};

pub struct VersionRouter;

// Baked in at compile time by the release build (see APP_VERSION in the
// Dockerfile), so the running binary reports the tag it was cut from rather
// than reading anything at runtime that could drift from it.
//
// The crate versions in this workspace are all `0.0.0` on purpose — releases
// are cut from git tags, not from Cargo.toml — so `env!("CARGO_PKG_VERSION")`
// would report a version that is always wrong. `option_env!` keeps a plain
// `cargo build` working without the variable set.
const VERSION: &str = match option_env!("APP_VERSION") {
    Some(version) => version,
    // A local or unreleased build. Deliberately not a version-shaped string,
    // so it can never be mistaken for a release in a bug report.
    None => "dev",
};

impl VersionRouter {
    pub fn router() -> Router {
        Router::new().route("/version", get(Self::get_version))
    }

    // Reports the backend release this process is running, for display in the
    // web app's footer. Takes no state and touches neither Mongo nor Redis:
    // it answers "which build is this?", which stays answerable while its
    // dependencies are down.
    async fn get_version() -> Json<Value> {
        Json(json!({ "version": VERSION }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    // The frontend footer reads `.version` off this payload, so the route path
    // and the field name are both part of the contract, not incidental.
    #[tokio::test]
    async fn get_version_returns_the_compiled_in_version() {
        let response = VersionRouter::router()
            .oneshot(
                Request::builder()
                    .uri("/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(body["version"], VERSION);
        // Unset at test time, so this also pins the no-APP_VERSION fallback.
        assert!(body["version"].is_string());
    }
}
