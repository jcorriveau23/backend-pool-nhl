use std::fmt;

use chrono::Utc;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

use poolnhl_interface::{errors::AppError, users::model::UserEmailJwtPayload};
use serde::Deserialize;

use axum::{
    async_trait,
    extract::{FromRequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::request::Parts,
    RequestPartsExt,
};

use crate::services::ServiceRegistry;

#[derive(Debug, Deserialize)]
struct Jwk {
    kty: String,
    // use: String,
    kid: String,
    n: String,
    e: String,
    alg: String,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

impl fmt::Display for Jwks {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for key in &self.keys {
            writeln!(f, "kty: {:?}", key.kid)?;
            writeln!(f, "n: {:?}", key.n)?;
            writeln!(f, "e: {:?}", key.e)?;
            writeln!(f, "alg: {:?}", key.alg)?;
        }
        Ok(())
    }
}

#[async_trait]
impl FromRequestParts<ServiceRegistry> for UserEmailJwtPayload
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

        let token_data = hanko_decode(bearer.token(), &state.jwks_url).await?;

        let exp = token_data
            .exp
            .parse::<i64>()
            .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

        // Validate if the token is expired.
        if exp < Utc::now().timestamp() {
            return Err(AppError::AuthError {
                msg: "The token is expired, please reconnect.".to_string(),
            });
        }

        Ok(token_data)
    }
}

pub async fn hanko_decode(token: &str, jwks_url: &str) -> Result<UserEmailJwtPayload, AppError> {
    let response = reqwest::get(jwks_url.to_string())
        .await
        .map_err(|e| AppError::ReqwestError { msg: e.to_string() })?;

    let jwks = response
        .json::<Jwks>()
        .await
        .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

    let header = decode_header(token).map_err(|e| AppError::JwtError { msg: e.to_string() })?;

    let kid = match header.kid {
        Some(k) => k,
        None => {
            return Err(AppError::JwtError {
                msg: "Could not recover the kid of the header.".to_string(),
            })
        }
    };

    print!("response Hanko {}", jwks);
    println!("Token: {}", token);
    println!("kid: {}", kid);
    let jwk = jwks
        .keys
        .iter()
        .find(|jwk| jwk.kid == kid)
        .ok_or_else(|| {
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken)
        })
        .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| AppError::JwtError { msg: e.to_string() })?;
    let validation = Validation::new(Algorithm::RS256);

    let token_data = decode::<UserEmailJwtPayload>(token, &decoding_key, &validation)
        .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

    Ok(token_data.claims)
}
