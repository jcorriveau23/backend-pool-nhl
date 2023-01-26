use hex::FromHexError;
use mongodb;
use std::fmt;
use web3;

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub enum AppError {
    CustomError { msg: String },
    AuthError { msg: String },
    MongoError { msg: String },
    ParseError { msg: String },
    BcryptError { msg: String },
    HexError { msg: String },
    RecoveryError { msg: String },
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError { msg } => write!(f, "Custom Error: {}", msg),
            AppError::AuthError { msg } => write!(f, "Authentification Error: {}", msg),
            AppError::MongoError { msg } => write!(f, "Mongo Error: {}", msg),
            AppError::ParseError { msg } => write!(f, "Parse Error: {}", msg),
            AppError::BcryptError { msg } => write!(f, "Bcrypt Error: {}", msg),
            AppError::HexError { msg } => write!(f, "Hex Error: {}", msg),
            AppError::RecoveryError { msg } => write!(f, "Recovery Error: {}", msg),
        }
    }
}

impl From<mongodb::error::Error> for AppError {
    fn from(e: mongodb::error::Error) -> Self {
        AppError::MongoError { msg: e.to_string() }
    }
}

impl From<chrono::format::ParseError> for AppError {
    fn from(e: chrono::format::ParseError) -> Self {
        AppError::ParseError { msg: e.to_string() }
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::BcryptError { msg: e.to_string() }
    }
}

impl From<FromHexError> for AppError {
    fn from(e: FromHexError) -> Self {
        AppError::HexError { msg: e.to_string() }
    }
}

impl From<web3::signing::RecoveryError> for AppError {
    fn from(e: web3::signing::RecoveryError) -> Self {
        AppError::RecoveryError { msg: e.to_string() }
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
