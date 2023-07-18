use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation};
use mongodb::bson::oid::ObjectId;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::errors::response::AppError;
use crate::settings::SETTINGS;

use crate::models::user::User;
use axum::{
    async_trait,
    extract::{FromRequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::request::Parts,
    RequestPartsExt,
};

static VALIDATION: Lazy<Validation> = Lazy::new(Validation::default);
static HEADER: Lazy<Header> = Lazy::new(Header::default);

#[derive(Debug, Serialize, Deserialize)]
pub struct UserToken {
    // data
    pub _id: ObjectId,
    pub name: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for UserToken
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::AuthError {
                msg: "Invalid token".to_string(),
            })?;

        let token_data = decode(bearer.token())?;

        Ok(token_data.claims.user)
    }
}

impl From<User> for UserToken {
    fn from(user: User) -> Self {
        Self {
            _id: user._id,
            name: user.name,
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
    pub fn new(user: User) -> Self {
        Self {
            exp: (chrono::Local::now() + chrono::Duration::days(7)).timestamp() as usize,
            iat: chrono::Local::now().timestamp() as usize,
            user: UserToken::from(user),
        }
    }
}

pub fn create(user: User) -> Result<String, AppError> {
    let encoding_key = EncodingKey::from_secret(SETTINGS.auth.secret.as_ref());
    let claims = Claims::new(user);

    Ok(jsonwebtoken::encode(&HEADER, &claims, &encoding_key)?)
}

pub fn decode(token: &str) -> Result<TokenData<Claims>, AppError> {
    let decoding_key = DecodingKey::from_secret(SETTINGS.auth.secret.as_ref());

    Ok(jsonwebtoken::decode::<Claims>(
        token,
        &decoding_key,
        &VALIDATION,
    )?)
}