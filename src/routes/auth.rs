use crate::database::CONNECTION;
use crate::db::user;
use crate::errors::response::Result;
use crate::models::user::{
    LoginRequest, RegisterRequest, SetPasswordRequest, SetUsernameRequest, User,
    WalletLoginRegisterRequest,
};
use crate::routes::jwt;
use crate::routes::jwt::UserToken;
use serde::Serialize;

use axum::{routing::post, Json, Router};

pub fn create_route() -> Router {
    Router::new()
        .route("/register", post(register_user))
        .route("/login", post(login_user))
        .route("/wallet-login", post(wallet_login_user))
        .route("/set-username", post(set_username))
        .route("/set-password", post(set_password))
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    user: User,
    token: String,
}

/// Register
async fn register_user(Json(body): Json<RegisterRequest>) -> Result<Json<LoginResponse>> {
    let user = user::create_user_from_register(CONNECTION.get().await, &body).await?;

    Ok(Json(LoginResponse {
        user: user.clone(),
        token: jwt::create(user)?,
    }))
}

/// Login
async fn login_user(Json(body): Json<LoginRequest>) -> Result<Json<LoginResponse>> {
    let user = user::login(CONNECTION.get().await, &body).await?;

    Ok(Json(LoginResponse {
        user: user.clone(),
        token: jwt::create(user)?,
    }))
}

/// Login
async fn wallet_login_user(
    Json(body): Json<WalletLoginRegisterRequest>,
) -> Result<Json<LoginResponse>> {
    let user = user::wallet_login(CONNECTION.get().await, &body).await?;

    Ok(Json(LoginResponse {
        user: user.clone(),
        token: jwt::create(user)?,
    }))
}

/// Set Username
async fn set_username(
    token: UserToken,
    Json(body): Json<SetUsernameRequest>,
) -> Result<Json<User>> {
    user::update_user_name(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.new_username,
    )
    .await
    .map(Json)
}

/// Set Username
async fn set_password(token: UserToken, body: Json<SetPasswordRequest>) -> Result<Json<User>> {
    user::update_password(
        CONNECTION.get().await,
        &token._id.to_string(),
        &body.password,
    )
    .await
    .map(Json)
}
