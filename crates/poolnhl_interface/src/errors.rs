use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub enum AppError {
    CustomError { msg: String },
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
}

pub type Result<T> = std::result::Result<T, AppError>;

impl std::error::Error for AppError {} // TODO: why?

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError { msg } => write!(f, "Custom Error: '{}'", msg),
            AppError::AuthError { msg } => write!(f, "Authentification Error: '{}'", msg),
            AppError::MongoError { msg } => write!(f, "MongoDB Error: '{}'", msg),
            AppError::ParseError { msg } => write!(f, "Parse Error: '{}'", msg),
            AppError::BcryptError { msg } => write!(f, "Bcrypt Error: '{}'", msg),
            AppError::HexError { msg } => write!(f, "Hex Error: '{}'", msg),
            AppError::RecoveryError { msg } => write!(f, "Recovery Error: '{}'", msg),
            AppError::BsonError { msg } => write!(f, "Bson Serialization Error: '{}'", msg),
            AppError::JwtError { msg } => write!(f, "Jwt Decoding Error: '{}'", msg),
            AppError::ObjectIdError { msg } => write!(f, "string to object ID Error: '{}'", msg),
            AppError::ReqwestError { msg } => write!(f, "Reqwest Error: '{}'", msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        //
        // Convert object to json
        let body = self.to_string();

        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
