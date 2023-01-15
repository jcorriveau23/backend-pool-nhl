use mongodb;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    CustomError(String),
    MongoError(String),
    ParseError(String),
    BcryptError(String),
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError(e) => write!(f, "Custom Error: {}", e),
            AppError::MongoError(e) => write!(f, "Mongo Error: {}", e),
            AppError::ParseError(e) => write!(f, "Parse Error: {}", e),
            AppError::BcryptError(e) => write!(f, "Bcrypt Error: {}", e),
        }
    }
}

impl From<mongodb::error::Error> for AppError {
    fn from(e: mongodb::error::Error) -> Self {
        AppError::MongoError(e.to_string())
    }
}

impl From<chrono::format::ParseError> for AppError {
    fn from(e: chrono::format::ParseError) -> Self {
        AppError::ParseError(e.to_string())
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(e: bcrypt::BcryptError) -> Self {
        AppError::BcryptError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

// Error Response being sent when an AppError has been captured into a request.

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ResponseError {
    pub error: ErrorContent,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ErrorContent {
    // HTTP Status Code returned
    code: u16,
    // Reason for an error
    reason: String,
    // Description for an error if any
    description: Option<String>,
}

impl ResponseError {
    // building a custom error.
    pub fn build(code: u16, description: Option<String>) -> ResponseError {
        let reason: String = match code {
            400 => "Bad Request".to_string(),
            401 => "Unauthorized".to_string(),
            _ => "Error".to_string(),
        };

        ResponseError {
            error: ErrorContent {
                code,
                reason,
                description,
            },
        }
    }
}

impl<'r> rocket::response::Responder<'r, 'static> for ResponseError {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        // Convert object to json
        let body = serde_json::to_string(&self).unwrap();
        rocket::Response::build()
            .sized_body(body.len(), std::io::Cursor::new(body))
            .header(rocket::http::ContentType::JSON)
            .status(rocket::http::Status::new(self.error.code))
            .ok()
    }
}
