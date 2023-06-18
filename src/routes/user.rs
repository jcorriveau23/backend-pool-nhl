use crate::db::user;
use crate::errors::response::Result;
use crate::models::user::User;

use crate::database::CONNECTION;
use axum::{extract::Path, routing::get, Json, Router};

pub fn create_route() -> Router {
    Router::new()
        .route("/user/:name", get(get_user_by_name))
        .route("/users", get(get_users))
        .route("/users/:names", get(get_users_with_id))
}

/// Get user by _name
async fn get_user_by_name(Path(_name): Path<String>) -> Result<Json<User>> {
    user::find_user_with_name(CONNECTION.get().await, &_name)
        .await
        .map(Json)
}

/// Get all users
async fn get_users() -> Result<Json<Vec<User>>> {
    user::find_users(CONNECTION.get().await, &None)
        .await
        .map(Json)
}

/// Get a specific list of users
async fn get_users_with_id(Path(names): Path<Vec<String>>) -> Result<Json<Vec<User>>> {
    user::find_users(CONNECTION.get().await, &Some(names))
        .await
        .map(Json)
}
