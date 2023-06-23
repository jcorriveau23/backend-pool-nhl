use crate::db::user;
use crate::errors::response::Result;
use crate::models::user::User;

use crate::AppState;
use axum::{extract::Path, extract::State, routing::get, Json, Router};

pub fn create_route() -> Router<AppState> {
    Router::new()
        .route("/user/:name", get(get_user_by_name))
        .route("/users", get(get_users))
        .route("/users/:names", get(get_users_with_id))
}

/// Get user by _name
async fn get_user_by_name(state: State<AppState>, Path(_name): Path<String>) -> Result<Json<User>> {
    user::find_user_with_name(&state.db, &_name).await.map(Json)
}

/// Get all users
async fn get_users(state: State<AppState>) -> Result<Json<Vec<User>>> {
    user::find_users(&state.db, &None).await.map(Json)
}

/// Get a specific list of users
async fn get_users_with_id(
    state: State<AppState>,
    Path(names): Path<Vec<String>>,
) -> Result<Json<Vec<User>>> {
    user::find_users(&state.db, &Some(names)).await.map(Json)
}
