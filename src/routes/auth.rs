use mongodb::Database;

use rocket::State;
use bcrypt;
use web3;

use crate::db::user;
use crate::errors::response::MyError;
use crate::models::user::LoginRequest;
use crate::models::user::RegisterRequest;
use crate::models::user::WalletLoginRegisterRequest;
use crate::routes::jwt::{UserToken, ApiKeyError};
use crate::routes::jwt;

use rocket::serde::json::{Json, Value, json};

/// Register
#[post("/register", format = "json", data = "<body>")]
pub async fn register_user(
    db: &State<Database>,
    body: Json<RegisterRequest>
) -> Result<Value, MyError> {
    let user = user::find_user_with_name(db, &body.name).await.unwrap();

    // the username provided is already registered.

    if !user.is_none() {
        return Err(MyError::build(
            400,
            Some(format!("this username is not available.")),
        ));
    }

    // hash password before sending it to the function that create the document.

    let password_hash = bcrypt::hash(body.password.clone(), 4).unwrap();

    let new_user = user::create_user_from_login(db, body.0, &password_hash).await.unwrap();

    let new_user_unwrap = new_user.unwrap();

    // create the token before sending the response (might need to change the string returned value)
    let token = jwt::generate_token_register(&new_user_unwrap);

    let user_json = json! ({"user": new_user_unwrap, "token": token});

    Ok(user_json)
}

/// Login
#[post("/login", format = "json", data = "<body>")]
pub async fn login_user(
    db: &State<Database>,
    body: Json<LoginRequest>
) -> Result<Value, MyError> {
    let user = user::find_user_with_name(db, &body.name).await.unwrap();

    if user.is_none() {
        return Err(MyError::build(
            400,
            Some(format!("This account does not exist.")),
        ));
    }

    let user_unwrap = user.unwrap();

    let user_unwrap_copy = user_unwrap.clone();

    if user_unwrap.password.is_none(){
        return Err(MyError::build(
            400,
            Some(format!("This account does not store any password.")), // happens when someone loging with a wallet (no password stored)
        ));
    }

    let password_unwrap = user_unwrap.password.unwrap();

    let is_valid_password = bcrypt::verify(&body.password, &password_unwrap).unwrap();
    
    if !is_valid_password {
        return Err(MyError::build(
            400,
            Some(format!("The password provided is not valid.")),
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
    body: Json<WalletLoginRegisterRequest>
) -> Result<Value, MyError> {
    if !verify_message(&body.addr, &body.sig).await {
        return Err(MyError::build(
            400,
            Some(format!("The signature provided is not valid.")),
        ));
    }

    let mut user = user::find_user_with_address(db, &body.addr).await.unwrap();

    if user.is_none() {
        // create the account if it does not exist 
        user = user::create_user_from_wallet_login(db, body.clone()).await.unwrap();
    }

    let user_unwrap = user.unwrap();

    // create the token before sending the response (might need to change the string returned value)
    let token = jwt::generate_token(&user_unwrap);

    let user_json = json! ({"user": user_unwrap, "token": token});
    
        
    Ok(user_json)
}

/// validate the token received in the header.
#[post("/validate-token")]
pub async fn validate_token(token: Result<UserToken, ApiKeyError>,) -> Result<Value, MyError> {
    if let Err(e) = token {
        return Err(jwt::return_token_error(e));      
    }

    let token_json = json! ({"token": token.unwrap()});
        
    Ok(token_json)
}

async fn verify_message(addr: &String, sig: &String) -> bool {
    let signer_addr = web3::signing::recover("Unlock wallet to access nhl-pool-ethereum.".to_string().as_bytes(), sig.as_bytes(), 1).unwrap();

    signer_addr.to_string() == *addr
}