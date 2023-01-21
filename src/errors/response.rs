use mongodb;
use std::fmt;

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub enum AppError {
    CustomError { msg: String, code: u16 },
    AuthError { msg: String, code: u16 },
    MongoError { msg: String, code: u16 },
    ParseError { msg: String, code: u16 },
    BcryptError { msg: String, code: u16 },
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError { msg, code } => write!(f, "Custom Error: {}", msg),
            AppError::AuthError { msg, code } => write!(f, "Authentification Error: {}", msg),
            AppError::MongoError { msg, code } => write!(f, "Mongo Error: {}", msg),
            AppError::ParseError { msg, code } => write!(f, "Parse Error: {}", msg),
            AppError::BcryptError { msg, code } => write!(f, "Bcrypt Error: {}", msg),
        }
    }
}

impl From<mongodb::error::Error> for AppError {
    fn from(e: mongodb::error::Error) -> Self {
        AppError::MongoError {
            msg: e.to_string(),
            code: 500,
        }
    }
}

impl From<chrono::format::ParseError> for AppError {
    fn from(e: chrono::format::ParseError) -> Self {
        AppError::ParseError {
            msg: e.to_string(),
            code: 500,
        }
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::BcryptError {
            msg: e.to_string(),
            code: 500,
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

impl<'r> rocket::response::Responder<'r, 'static> for AppError {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        // Convert object to json
        let body = self.to_string();

        let code = match self {
            AppError::CustomError { msg, code } => code,
            AppError::AuthError { msg, code } => code,
            AppError::MongoError { msg, code } => code,
            AppError::ParseError { msg, code } => code,
            AppError::BcryptError { msg, code } => code,
        };

        rocket::Response::build()
            .sized_body(body.len(), std::io::Cursor::new(body))
            .header(rocket::http::ContentType::JSON)
            .status(rocket::http::Status::new(code))
            .ok()
    }
}
