use axum::extract::{Json, Path, State};
use axum::routing::get;
use axum::Router;

use poolnhl_interface::errors::Result;
use poolnhl_interface::users::{model::UserData, service::UsersServiceHandle};

pub struct UsersRouter;

impl UsersRouter {
    pub fn new(users_service: UsersServiceHandle) -> Router {
        Router::new()
            .route("/user/:name", get(UsersRouter::get_user_by_name))
            .route("/users", get(UsersRouter::get_users))
            .with_state(users_service)
    }

    pub async fn get_user_by_name(
        Path(name): Path<String>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<UserData>> {
        let user = users_service.get_user_by_name(&name).await?;

        Ok(Json(user))
    }

    pub async fn get_users(
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<Vec<UserData>>> {
        let users = users_service.list_all_users().await?;

        Ok(Json(users))
    }
}
