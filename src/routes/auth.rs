use mongodb::Database;

use bcrypt;
use rocket::State;
use web3;

use crate::db::user;
use crate::errors::response::MyError;
use crate::models::user::{
    LoginRequest, RegisterRequest, SetUsernameRequest, WalletLoginRegisterRequest,
};
use crate::routes::jwt;
use crate::routes::jwt::{ApiKeyError, UserToken};

use rocket::serde::json::{json, Json, Value};

/// Register
#[post("/register", format = "json", data = "<body>")]
pub async fn register_user(
    db: &State<Database>,
    body: Json<RegisterRequest>,
) -> Result<Value, MyError> {
    let user = user::find_user_with_name(db, &body.name).await.unwrap();

    // the username provided is already registered.

    if user.is_some() {
        return Err(MyError::build(
            400,
            Some("this username is not available.".to_string()),
        ));
    }

    // hash password before sending it to the function that create the document.

    let password_hash = bcrypt::hash(body.password.clone(), 4).unwrap();

    let new_user = user::create_user_from_login(db, &body.0, &password_hash)
        .await
        .unwrap();

    let new_user_unwrap = new_user.unwrap();

    // create the token before sending the response (might need to change the string returned value)
    let token = jwt::generate_token_register(&new_user_unwrap);

    let user_json = json! ({"user": new_user_unwrap, "token": token});

    Ok(user_json)
}

/// Login
#[post("/login", format = "json", data = "<body>")]
pub async fn login_user(db: &State<Database>, body: Json<LoginRequest>) -> Result<Value, MyError> {
    let user = user::find_user_with_name(db, &body.name).await.unwrap();

    if user.is_none() {
        return Err(MyError::build(
            400,
            Some("This account does not exist.".to_string()),
        ));
    }

    let user_unwrap = user.unwrap();

    let user_unwrap_copy = user_unwrap.clone();

    if user_unwrap.password.is_none() {
        return Err(MyError::build(
            400,
            Some("This account does not store any password.".to_string()), // happens when someone loging with a wallet (no password stored)
        ));
    }

    let password_unwrap = user_unwrap.password.unwrap();

    let is_valid_password = bcrypt::verify(&body.password, &password_unwrap).unwrap();

    if !is_valid_password {
        return Err(MyError::build(
            400,
            Some("The password provided is not valid.".to_string()),
        ));
    }

    // create the token before sending the response

    let token = jwt::generate_token(&user_unwrap_copy);

    let user_json = json! ({"user": user_unwrap_copy, "token": token});

    Ok(user_json)
}

/// Login
#[post("/wallet-login", format = "json", data = "<body>")]
pub async fn wallet_login_user(
    db: &State<Database>,
    body: Json<WalletLoginRegisterRequest>,
) -> Result<Value, MyError> {
    if !verify_message(&body.addr, &body.sig).await {
        return Err(MyError::build(
            400,
            Some("The signature provided is not valid.".to_string()),
        ));
    }

    let mut user = user::find_user_with_address(db, &body.addr).await.unwrap();

    if user.is_none() {
        // create the account if it does not exist
        user = user::create_user_from_wallet_login(db, body.into_inner())
            .await
            .unwrap();
    }

    let user_unwrap = user.unwrap();

    // create the token before sending the response (might need to change the string returned value)
    let token = jwt::generate_token(&user_unwrap);

    let user_json = json! ({"user": user_unwrap, "token": token});

    Ok(user_json)
}

/// Set Username
#[post("/set-username", format = "json", data = "<body>")]
pub async fn set_username(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
    body: Json<SetUsernameRequest>,
) -> Result<Value, MyError> {
    if let Err(e) = token {
        return Err(jwt::return_token_error(e));
    }

    let user = user::update_user_name(db, &token.unwrap()._id.to_string(), &body.new_username)
        .await
        .unwrap();

    let user_json = json! ({"user": user.unwrap()});

    Ok(user_json)
}

/// validate the token received in the header.
#[post("/validate-token")]
pub async fn validate_token(token: Result<UserToken, ApiKeyError>) -> Result<Value, MyError> {
    if let Err(e) = token {
        return Err(jwt::return_token_error(e));
    }

    let token_json = json! ({"token": token.unwrap()});

    Ok(token_json)
}

async fn verify_message(addr: &String, sig: &String) -> bool {
    let signer_addr = web3::signing::recover(
        "Unlock wallet to access nhl-pool-ethereum."
            .to_string()
            .as_bytes(),
        sig.as_bytes(),
        1,
    )
    .unwrap();

    signer_addr.to_string() == *addr
}
