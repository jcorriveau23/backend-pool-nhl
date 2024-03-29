use std::time::{self, SystemTime};

use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation};

use once_cell::sync::Lazy;
use poolnhl_interface::{draft::model::UserToken, errors::AppError};
use serde::{Deserialize, Serialize};

use axum::{
    async_trait,
    extract::{FromRequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::request::Parts,
    RequestPartsExt,
};

use crate::services::{users_service::User, ServiceRegistry};

static VALIDATION: Lazy<Validation> = Lazy::new(Validation::default);
static HEADER: Lazy<Header> = Lazy::new(Header::default);

#[async_trait]
impl FromRequestParts<ServiceRegistry> for UserToken
where
    ServiceRegistry: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServiceRegistry,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|err| AppError::AuthError {
                msg: err.to_string(),
            })?;

        let token_data = decode(bearer.token(), &state.secret)?;

        // Validate if the token is expired.
        if token_data.claims.exp < Utc::now().timestamp() as usize {
            return Err(AppError::AuthError {
                msg: "The token is expired, please reconnect.".to_string(),
            });
        }

        Ok(token_data.claims.user)
    }
}

impl From<&User> for UserToken {
    fn from(user: &User) -> Self {
        Self {
            _id: user._id.to_string(),
            name: user.name.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub exp: usize, // Expiration time (as UTC timestamp). validate_exp defaults to true in validation
    pub iat: usize, // Issued at (as UTC timestamp)
    pub user: UserToken,
}

impl Claims {
    pub fn new(user: &User) -> Self {
        Self {
            exp: (chrono::Local::now() + chrono::Duration::days(7)).timestamp() as usize,
            iat: chrono::Local::now().timestamp() as usize,
            user: UserToken::from(user),
        }
    }
}

pub fn create(user: &User, secret: &str) -> Result<String, AppError> {
    let encoding_key = EncodingKey::from_secret(secret.as_bytes());
    let claims = Claims::new(user);

    jsonwebtoken::encode(&HEADER, &claims, &encoding_key).map_err(|_| AppError::JwtError {
        msg: "Could not create the token".to_string(),
    })
}

pub fn decode(token: &str, secret: &str) -> Result<TokenData<Claims>, AppError> {
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());

    jsonwebtoken::decode::<Claims>(token, &decoding_key, &VALIDATION).map_err(|_| {
        AppError::JwtError {
            msg: "Could not decode the token".to_string(),
        }
    })
}
