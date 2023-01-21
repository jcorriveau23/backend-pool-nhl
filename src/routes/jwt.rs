use chrono::Utc;

use jsonwebtoken::errors::Result;
use jsonwebtoken::TokenData;
use jsonwebtoken::{DecodingKey, EncodingKey};
use jsonwebtoken::{Header, Validation};
use mongodb::bson::oid::ObjectId;
use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::{self, FromRequest, Request};
use serde::{Deserialize, Serialize};

use crate::errors::response::AppError;
use crate::models::user::User;

static ONE_WEEK: i64 = 60 * 60 * 24 * 7; // in seconds

#[derive(Debug, Serialize, Deserialize)]
pub struct UserToken {
    // issued at
    pub iat: i64,
    // expiration
    pub exp: i64,
    // data
    pub _id: ObjectId,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserToken {
    type Error = ApiKeyError;
    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        //let conn = req.guard::<Database>().unwrap();
        if let Some(authen_header) = req.headers().get_one("Authorization") {
            let authen_str = authen_header.to_string();
            if authen_str.starts_with("Bearer") {
                let token = authen_str[6..authen_str.len()].trim();
                if let Ok(token_data) = decode_token(token.to_string()) {
                    return verify_token(token_data.claims);
                } else {
                    return Outcome::Failure((Status::BadRequest, ApiKeyError::Invalid));
                }
            }
        }

        Outcome::Failure((Status::BadRequest, ApiKeyError::Missing))
    }
}

pub fn return_token_error(e: ApiKeyError) -> AppError {
    match e {
        ApiKeyError::Invalid => AppError::AuthError {
            msg: "The token provided is not valid.".to_string(),
            code: 400,
        },
        ApiKeyError::Missing => AppError::AuthError {
            msg: "The token was not provided.".to_string(),
            code: 400,
        },
        ApiKeyError::Expired => AppError::AuthError {
            msg: "The token has expired.".to_string(),
            code: 400,
        },
    }
}

pub fn generate_token(_user: &User) -> String {
    let now = Utc::now().timestamp_nanos() / 1_000_000_000; // nanosecond -> second
    let payload = UserToken {
        iat: now,
        exp: now + ONE_WEEK,
        _id: _user._id,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &payload,
        &EncodingKey::from_secret(include_bytes!("secret.key")),
    )
    .unwrap()
}

fn decode_token(token: String) -> Result<TokenData<UserToken>> {
    jsonwebtoken::decode::<UserToken>(
        &token,
        &DecodingKey::from_secret(include_bytes!("secret.key")),
        &Validation::default(),
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ApiKeyError {
    Missing,
    Invalid,
    Expired,
}

fn verify_token(token: UserToken) -> request::Outcome<UserToken, ApiKeyError> {
    if token.exp < (Utc::now().timestamp_nanos() / 1_000_000_000) {
        // the token is expired, the user will need to re-generate a jwt token

        return Outcome::Failure((Status::BadRequest, ApiKeyError::Expired));
    }

    Outcome::Success(token)
}
