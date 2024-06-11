use std::fmt;

use chrono::Utc;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

use once_cell::sync::Lazy;
use poolnhl_interface::{errors::AppError, users::model::UserEmailJwtPayload};
use serde::Deserialize;
use tokio::sync::Mutex;

use axum::{async_trait, extract::FromRequestParts, http::request::Parts, RequestPartsExt};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};

use crate::services::ServiceRegistry;

#[derive(Debug, Deserialize, Clone)]
struct Jwk {
    kty: String,
    kid: String,
    n: String,
    e: String,
    alg: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Jwks {
    keys: Vec<Jwk>,
}

impl fmt::Display for Jwks {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for key in &self.keys {
            writeln!(f, "kty: {:?}", key.kty)?;
            writeln!(f, "kid: {:?}", key.kid)?;
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

        let token_data = hanko_token_decode(
            bearer.token(),
            &state.auth.jwks_url,
            &state.auth.token_audience,
        )
        .await?;

        // Validate if the token is expired.
        if token_data.exp < Utc::now().timestamp() {
            return Err(AppError::AuthError {
                msg: "The token is expired, please reconnect.".to_string(),
            });
        }

        Ok(token_data)
    }
}

pub async fn hanko_token_decode(
    token: &str,
    jwks_url: &str,
    token_audience: &str,
) -> Result<UserEmailJwtPayload, AppError> {
    async fn fetch_new_jwks(jwks_url: &str) -> Result<Jwks, AppError> {
        // Fetch the latest jwks stored into the Hanko server using the endpoints.
        // This is called when we discovered the jwks kid does not exist in the cache variable.
        // The key rotation is not that often so this function should not be called a lot.
        let response = reqwest::get(jwks_url)
            .await
            .map_err(|e| AppError::ReqwestError { msg: e.to_string() })?;
        let new_jwks = response
            .json::<Jwks>()
            .await
            .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

        // The following 2 lines lock the mutex to re-write it. It needs to be fast since the cached jwks is shared across thread.
        let mut cache_write = JWKS_CACHE.lock().await;

        *cache_write = Some(new_jwks.clone());

        Ok(new_jwks)
    }

    fn decode_token(
        jwk: &Jwk,
        token: &str,
        token_audience: &str,
    ) -> Result<UserEmailJwtPayload, AppError> {
        // Decode the string token. using the related jwk. A related jwk is then the token 'kid' match the jwk 'kid'.
        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[token_audience.to_string()]);

        let token_data = decode::<UserEmailJwtPayload>(token, &decoding_key, &validation)
            .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

        Ok(token_data.claims)
    }

    fn find_token_kid(token: &str) -> Result<String, AppError> {
        // Find the User token kid value. This kid is compared with the jwks kid to find which key should be used to validate the token.
        let header = decode_header(token).map_err(|e| AppError::JwtError { msg: e.to_string() })?;
        match header.kid {
            Some(kid) => Ok(kid),
            None => {
                return Err(AppError::JwtError {
                    msg: "Could not recover the kid of the header.".to_string(),
                })
            }
        }
    }

    // This static variable is shared across all axum threads.
    // It cached the JWKS hosted on the hanko server so we don't have to query it from hanko everytime.
    static JWKS_CACHE: Lazy<Mutex<Option<Jwks>>> = Lazy::new(|| Mutex::new(None));
    let token_kid = find_token_kid(token)?;

    // Try to get the JWKS from cache. The mutex is being locked for the copy of the object only.
    let cached_jwks = {
        let cache_read = JWKS_CACHE.lock().await;
        cache_read.clone()
    };

    let jwk = match &cached_jwks {
        Some(cached_jwks) => {
            let used_jwk = cached_jwks.keys.iter().find(|jwk| jwk.kid == token_kid);

            match used_jwk {
                Some(jwk) => jwk.clone(),
                None => {
                    // This will be trigger when the jwks needs to be updated. The user token present a kid not existing in the list
                    let new_jwks = fetch_new_jwks(jwks_url).await?;

                    match new_jwks.keys.iter().find(|jwk| jwk.kid == token_kid) {
                        Some(jwk) => jwk.clone(),
                        None => {
                            return Err(AppError::JwtError {
                                msg: "The kid of the user does not math any of the jwk."
                                    .to_string(),
                            });
                        }
                    }
                }
            }
        }
        None => {
            // This will only occured the first time the function is ran. We need to fetch the jwks at least the first time.
            let new_jwks = fetch_new_jwks(jwks_url).await?;

            match new_jwks.keys.iter().find(|jwk| jwk.kid == token_kid) {
                Some(jwk) => jwk.clone(),
                None => {
                    return Err(AppError::JwtError {
                        msg: "The kid of the user does not math any of the jwk.".to_string(),
                    });
                }
            }
        }
    };

    // Finally decode the token using the found jwk and the token.
    decode_token(&jwk, token, token_audience)
}
