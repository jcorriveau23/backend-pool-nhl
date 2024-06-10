use axum::routing::get;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::errors::Result;
use poolnhl_interface::users::model::UserEmailJwtPayload;

pub struct UsersRouter;

impl UsersRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/token", get(Self::validate_token))
            .with_state(service_registry)
    }

    /// Validate the token, the validation is being done in the from_request_parts() implementation for UserEmailJwtPayload.
    async fn validate_token(token: UserEmailJwtPayload) -> Result<()> {
        Ok(())
    }
}
