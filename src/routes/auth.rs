use mongodb::Database;

use rocket::State;

use crate::db::user;
use crate::errors::response::AppError;
use crate::models::user::{
    LoginRequest, RegisterRequest, SetPasswordRequest, SetUsernameRequest,
    WalletLoginRegisterRequest,
};
use crate::routes::jwt;
use crate::routes::jwt::UserToken;

use rocket::serde::json::{json, Json, Value};

/// Register
#[post("/register", format = "json", data = "<body>")]
pub async fn register_user(
    db: &State<Database>,
    body: Json<RegisterRequest>,
) -> Result<Value, AppError> {
    user::create_user_from_register(db, &body)
        .await
        .map(|user| {
            // create the token before sending the response (might need to change the string returned value)
            let token = jwt::generate_token(&user);
            let user_json = json! ({"user": user, "token": token});
            user_json
        })
}

/// Login
#[post("/login", format = "json", data = "<body>")]
pub async fn login_user(db: &State<Database>, body: Json<LoginRequest>) -> Result<Value, AppError> {
    user::login(db, &body).await.map(|user| {
        // create the token before sending the response (might need to change the string returned value)
        let token = jwt::generate_token(&user);
        let user_json = json! ({"user": user, "token": token});
        user_json
    })
}

/// Login
#[post("/wallet-login", format = "json", data = "<body>")]
pub async fn wallet_login_user(
    db: &State<Database>,
    body: Json<WalletLoginRegisterRequest>,
) -> Result<Value, AppError> {
    user::wallet_login(db, &body).await.map(|user| {
        // create the token before sending the response (might need to change the string returned value)
        let token = jwt::generate_token(&user);
        let user_json = json! ({"user": user, "token": token});
        user_json
    })
}

/// Set Username
#[post("/set-username", format = "json", data = "<body>")]
pub async fn set_username(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<SetUsernameRequest>,
) -> Result<Value, AppError> {
    user::update_user_name(db, &token?._id.to_string(), &body.new_username)
        .await
        .map(|user| {
            // create the token before sending the response (might need to change the string returned value)
            let user_json = json!({ "user": user });
            user_json
        })
}

/// Set Username
#[post("/set-password", format = "json", data = "<body>")]
pub async fn set_password(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
    body: Json<SetPasswordRequest>,
) -> Result<Value, AppError> {
    user::update_password(db, &token?._id.to_string(), &body.password)
        .await
        .map(|user| {
            // create the token before sending the response (might need to change the string returned value)
            let user_json = json!({ "user": user });
            user_json
        })
}

/// validate the token received in the header.
#[post("/validate-token")]
pub async fn validate_token(token: Result<UserToken, AppError>) -> Result<Value, AppError> {
    token.map(move |token| {
        let token_json = json!({ "token": token });

        token_json
    })
}
