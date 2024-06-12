use std::fmt;

use chrono::Utc;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

use poolnhl_interface::{errors::AppError, users::model::UserEmailJwtPayload};
use serde::Deserialize;
use std::sync::RwLock;

use axum::{async_trait, extract::FromRequestParts, http::request::Parts, RequestPartsExt};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};

use crate::{services::ServiceRegistry, settings::Auth};

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

pub struct CachedJwks {
    jwks: RwLock<Jwks>,
    pub auth_info: Auth,
}

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

    Ok(new_jwks)
}

impl CachedJwks {
    pub async fn new(auth_info: &Auth) -> Result<Self, AppError> {
        // On the cached creation first fetch the JSON web key sets.
        let jwks = fetch_new_jwks(&auth_info.jwks_url).await?;

        Ok(CachedJwks {
            jwks: RwLock::new(jwks),
            auth_info: auth_info.clone(),
        })
    }

    async fn update_jwks(&self) -> Result<(), AppError> {
        let new_jwks = fetch_new_jwks(&self.auth_info.jwks_url).await?;

        // The following 2 lines lock the mutex to update its value.
        // It needs to be fast since the cached jwks is shared across thread.
        let mut jwks_write_lock = self
            .jwks
            .write()
            .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

        *jwks_write_lock = new_jwks;
        Ok(())
    }

    fn get_matching_key(&self, token_kid: &str) -> Result<Option<Jwk>, AppError> {
        // Copy the matching key. Since this is stored in a mutex,
        // we create a copy to avoid locking the value for to long.
        let jwks_read_lock = self
            .jwks
            .read()
            .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

        Ok(jwks_read_lock
            .keys
            .iter()
            .find(|jwk| jwk.kid == token_kid)
            .cloned())
    }
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

        let token_data = hanko_token_decode(bearer.token(), &state.cached_keys).await?;

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
    cached_jwk: &CachedJwks,
) -> Result<UserEmailJwtPayload, AppError> {
    fn decode_token(
        jwk: &Jwk,
        token: &str,
        token_audience: &str,
    ) -> Result<UserEmailJwtPayload, AppError> {
        // Decode the string token. using the related jwk. A related jwk is then the token 'kid' match the jwk 'kid'.
        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|e| AppError::JwtError { msg: e.to_string() })?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[token_audience]);

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

    let token_kid = find_token_kid(token)?;

    match cached_jwk.get_matching_key(&token_kid) {
        Ok(jwk) => match jwk {
            Some(jwk) => decode_token(&jwk, token, &cached_jwk.auth_info.token_audience),
            None => {
                // If no matching jwk found, we need to update the jkws by querying hanko.
                // This is due to key rotation, should not happen often.
                cached_jwk.update_jwks().await?;
                match cached_jwk.get_matching_key(&token_kid)? {
                    Some(jwk) => decode_token(&jwk, token, &cached_jwk.auth_info.token_audience),
                    None => Err(AppError::NonMatchingKid {
                        msg: format!("No json web token found with kid {}", token_kid),
                    }),
                }
            }
        },
        Err(err) => Err(err),
    }
}
