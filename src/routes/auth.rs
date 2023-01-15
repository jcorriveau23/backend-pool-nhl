use mongodb::Database;

use rocket::State;

use crate::db::user;
use crate::errors::response::ResponseError;
use crate::models::user::{
    LoginRequest, RegisterRequest, SetPasswordRequest, SetUsernameRequest,
    WalletLoginRegisterRequest,
};
use crate::routes::jwt;
use crate::routes::jwt::{ApiKeyError, UserToken};

use rocket::serde::json::{json, Json, Value};

/// Register
#[post("/register", format = "json", data = "<body>")]
pub async fn register_user(
    db: &State<Database>,
    body: Json<RegisterRequest>,
) -> Result<Value, ResponseError> {
    match user::create_user_from_register(db, &body).await {
        Ok(user) => {
            // create the token before sending the response (might need to change the string returned value)
            let token = jwt::generate_token(&user);
            let user_json = json! ({"user": user, "token": token});
            Ok(user_json)
        }
        Err(e) => Err(ResponseError::build(400, Some(e.to_string()))),
    }
}

/// Login
#[post("/login", format = "json", data = "<body>")]
pub async fn login_user(
    db: &State<Database>,
    body: Json<LoginRequest>,
) -> Result<Value, ResponseError> {
    match user::login(db, &body).await {
        Ok(user) => {
            // create the token before sending the response (might need to change the string returned value)
            let token = jwt::generate_token(&user);
            let user_json = json! ({"user": user, "token": token});
            Ok(user_json)
        }
        Err(e) => Err(ResponseError::build(400, Some(e.to_string()))),
    }
}

/// Login
#[post("/wallet-login", format = "json", data = "<body>")]
pub async fn wallet_login_user(
    db: &State<Database>,
    body: Json<WalletLoginRegisterRequest>,
) -> Result<Value, ResponseError> {
    match user::wallet_login(db, &body).await {
        Ok(user) => {
            // create the token before sending the response (might need to change the string returned value)
            let token = jwt::generate_token(&user);
            let user_json = json! ({"user": user, "token": token});
            Ok(user_json)
        }
        Err(e) => Err(ResponseError::build(400, Some(e.to_string()))),
    }
}

/// Set Username
#[post("/set-username", format = "json", data = "<body>")]
pub async fn set_username(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<SetUsernameRequest>,
) -> Result<Value, ResponseError> {
    if let Err(e) = token {
        return Err(jwt::return_token_error(e));
    }

    match user::update_user_name(db, &token.unwrap()._id.to_string(), &body.new_username).await {
        Ok(user) => {
            // create the token before sending the response (might need to change the string returned value)
            let user_json = json!({ "user": user });
            Ok(user_json)
        }
        Err(e) => Err(ResponseError::build(400, Some(e.to_string()))),
    }
}

/// Set Username
#[post("/set-password", format = "json", data = "<body>")]
pub async fn set_password(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<SetPasswordRequest>,
) -> Result<Value, ResponseError> {
    if let Err(e) = token {
        return Err(jwt::return_token_error(e));
    }

    match user::update_password(db, &token.unwrap()._id.to_string(), &body.password).await {
        Ok(user) => {
            // create the token before sending the response (might need to change the string returned value)
            let user_json = json!({ "user": user });
            Ok(user_json)
        }
        Err(e) => Err(ResponseError::build(400, Some(e.to_string()))),
    }
}

/// validate the token received in the header.
#[post("/validate-token")]
pub async fn validate_token(token: Result<UserToken, ApiKeyError>) -> Result<Value, ResponseError> {
    if let Err(e) = token {
        return Err(jwt::return_token_error(e));
    }

    let token_json = json! ({"token": token.unwrap()});

    Ok(token_json)
}
