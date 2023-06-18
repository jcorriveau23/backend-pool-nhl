use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use hex::FromHexError;
use jsonwebtoken;
use mongodb;
use std::fmt;
use web3;

#[derive(Debug)]
pub enum AppError {
    CustomError { msg: String },
    AuthError { msg: String },
    MongoError { e: mongodb::error::Error },
    ParseError { e: chrono::format::ParseError },
    BcryptError { e: bcrypt::BcryptError },
    HexError { e: FromHexError },
    RecoveryError { e: web3::signing::RecoveryError },
    BsonError { e: mongodb::bson::ser::Error },
    JwtError { e: jsonwebtoken::errors::Error },
    ObjectIdError { e: mongodb::bson::oid::Error },
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError { msg } => write!(f, "Custom Error: '{}'", msg),
            AppError::AuthError { msg } => write!(f, "Authentification Error: '{}'", msg),
            AppError::MongoError { e } => write!(f, "Mongo Error: '{}'", e),
            AppError::ParseError { e } => write!(f, "Parse Error: '{}'", e),
            AppError::BcryptError { e } => write!(f, "Bcrypt Error: '{}'", e),
            AppError::HexError { e } => write!(f, "Hex Error: '{}'", e),
            AppError::RecoveryError { e } => write!(f, "Recovery Error: '{}'", e),
            AppError::BsonError { e } => write!(f, "Bson Serialization Error: '{}'", e),
            AppError::JwtError { e } => write!(f, "Jwt Decoding Error: '{}'", e),
            AppError::ObjectIdError { e } => write!(f, "string to object ID Error: '{}'", e),
        }
    }
}

impl From<mongodb::error::Error> for AppError {
    fn from(e: mongodb::error::Error) -> Self {
        AppError::MongoError { e }
    }
}

impl From<chrono::format::ParseError> for AppError {
    fn from(e: chrono::format::ParseError) -> Self {
        AppError::ParseError { e }
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::BcryptError { e }
    }
}

impl From<FromHexError> for AppError {
    fn from(e: FromHexError) -> Self {
        AppError::HexError { e }
    }
}

impl From<web3::signing::RecoveryError> for AppError {
    fn from(e: web3::signing::RecoveryError) -> Self {
        AppError::RecoveryError { e }
    }
}

impl From<mongodb::bson::ser::Error> for AppError {
    fn from(e: mongodb::bson::ser::Error) -> Self {
        AppError::BsonError { e }
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        AppError::JwtError { e }
    }
}

impl From<mongodb::bson::oid::Error> for AppError {
    fn from(e: mongodb::bson::oid::Error) -> Self {
        AppError::ObjectIdError { e }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Convert object to json
        let body = self.to_string();
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
