use hex::FromHexError;
use mongodb;
use std::{f32::consts::E, fmt};
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
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError { msg } => write!(f, "Custom Error: {}", msg),
            AppError::AuthError { msg } => write!(f, "Authentification Error: {}", msg),
            AppError::MongoError { e } => write!(f, "Mongo Error: {}", e),
            AppError::ParseError { e } => write!(f, "Parse Error: {}", e),
            AppError::BcryptError { e } => write!(f, "Bcrypt Error: {}", e),
            AppError::HexError { e } => write!(f, "Hex Error: {}", e),
            AppError::RecoveryError { e } => write!(f, "Recovery Error: {}", e),
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

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn code(&self) -> u16 {
        match &self {
            Self::CustomError { .. } => 500,
            Self::AuthError { .. } => 401, // Unauthorized
            Self::MongoError { .. } => 501,
            Self::ParseError { .. } => 502,
            Self::BcryptError { .. } => 504,
            Self::HexError { .. } => 505,
            Self::RecoveryError { .. } => 506,
        }
    }
}

impl<'r> rocket::response::Responder<'r, 'static> for AppError {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        // Convert object to json
        let body = self.to_string();

        rocket::Response::build()
            .sized_body(body.len(), std::io::Cursor::new(body))
            .header(rocket::http::ContentType::JSON)
            .status(rocket::http::Status::new(self.code()))
            .ok()
    }
}
