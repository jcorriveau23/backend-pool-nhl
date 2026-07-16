use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub enum AppError {
    CustomError { msg: String },
    // A requested resource (pool, daily leaders, ...) does not exist. Maps to
    // 404 so clients can tell "you asked for something that isn't here" apart
    // from "the server failed" (500).
    NotFound { msg: String },
    AuthError { msg: String },
    MongoError { msg: String },
    ParseError { msg: String },
    BcryptError { msg: String },
    HexError { msg: String },
    RecoveryError { msg: String },
    BsonError { msg: String },
    JwtError { msg: String },
    ObjectIdError { msg: String },
    ReqwestError { msg: String },
    NonMatchingKid { msg: String },
    RwLockError { msg: String },
    RedisError { msg: String },
}

pub type Result<T> = std::result::Result<T, AppError>;

impl std::error::Error for AppError {} // TODO: why?

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError { msg } => write!(f, "Custom Error: '{}'", msg),
            AppError::NotFound { msg } => write!(f, "Not Found: '{}'", msg),
            AppError::AuthError { msg } => write!(f, "Authentication Error: '{}'", msg),
            AppError::MongoError { msg } => write!(f, "MongoDB Error: '{}'", msg),
            AppError::ParseError { msg } => write!(f, "Parse Error: '{}'", msg),
            AppError::BcryptError { msg } => write!(f, "Bcrypt Error: '{}'", msg),
            AppError::HexError { msg } => write!(f, "Hex Error: '{}'", msg),
            AppError::RecoveryError { msg } => write!(f, "Recovery Error: '{}'", msg),
            AppError::BsonError { msg } => write!(f, "Bson Serialization Error: '{}'", msg),
            AppError::JwtError { msg } => write!(f, "Jwt Decoding Error: '{}'", msg),
            AppError::ObjectIdError { msg } => write!(f, "string to object ID Error: '{}'", msg),
            AppError::ReqwestError { msg } => write!(f, "Reqwest Error: '{}'", msg),
            AppError::NonMatchingKid { msg } => write!(f, "Non matching kid Error: '{}'", msg),
            AppError::RwLockError { msg } => write!(f, "Mutex locking error '{}'", msg),
            AppError::RedisError { msg } => write!(f, "Redis Error: '{}'", msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Map each error kind to an appropriate HTTP status. New client-error
        // variants (e.g. 400/401/409) can be added to this match as needed;
        // everything unlisted is treated as a server fault.
        let status = match self {
            AppError::NotFound { .. } => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = self.to_string();
        (status, body).into_response()
    }
}
