use mongodb::Database;

use rocket::State;

use crate::db::user;
use crate::errors::response::Result;
use crate::models::user::{
    LoginRequest, RegisterRequest, SetPasswordRequest, SetUsernameRequest,
    WalletLoginRegisterRequest,
};
use crate::routes::jwt;
use crate::routes::jwt::UserToken;

use rocket::serde::json::{json, Json, Value};

/// Register
#[post("/register", format = "json", data = "<body>")]
pub async fn register_user(db: &State<Database>, body: Json<RegisterRequest>) -> Result<Value> {
    user::create_user_from_register(db, &body)
        .await
        .map(|user| jwt::generate_user_token(&user))?
}

/// Login
#[post("/login", format = "json", data = "<body>")]
pub async fn login_user(db: &State<Database>, body: Json<LoginRequest>) -> Result<Value> {
    user::login(db, &body)
        .await
        .map(|user| jwt::generate_user_token(&user))?
}

/// Login
#[post("/wallet-login", format = "json", data = "<body>")]
pub async fn wallet_login_user(
    db: &State<Database>,
    body: Json<WalletLoginRegisterRequest>,
) -> Result<Value> {
    user::wallet_login(db, &body)
        .await
        .map(|user| jwt::generate_user_token(&user))?
}

/// Set Username
#[post("/set-username", format = "json", data = "<body>")]
pub async fn set_username(
    db: &State<Database>,
    token: Result<UserToken>,
    body: Json<SetUsernameRequest>,
) -> Result<Value> {
    user::update_user_name(db, &token?._id.to_string(), &body.new_username)
        .await
        .map(|user| json!({ "user": user }))
}

/// Set Username
#[post("/set-password", format = "json", data = "<body>")]
pub async fn set_password(
    db: &State<Database>,
    token: Result<UserToken>,
    body: Json<SetPasswordRequest>,
) -> Result<Value> {
    user::update_password(db, &token?._id.to_string(), &body.password)
        .await
        .map(|user| json!({ "user": user }))
}

/// validate the token received in the header.
#[post("/validate-token")]
pub async fn validate_token(token: Result<UserToken>) -> Result<Value> {
    token.map(move |token| json!({ "token": token }))
}
