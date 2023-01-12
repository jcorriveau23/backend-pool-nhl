use mongodb;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    HttpError,
    MongoError,
}

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::HttpError => write!(f, "HTTP Error"),
            AppError::MongoError => write!(f, "Mongo Error"),
        }
    }
}

impl From<mongodb::error::Error> for AppError {
    fn from(_: mongodb::error::Error) -> Self {
        AppError::MongoError
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

// The error bellow will be removed once the issue is solved.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ErrorContent {
    // HTTP Status Code returned
    code: u16,
    // Reason for an error
    reason: String,
    // Description for an error if any
    description: Option<String>,
}

/// Error messages returned to user
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct MyError {
    pub error: ErrorContent,
}

impl MyError {
    // building a custom error.
    pub fn build(code: u16, description: Option<String>) -> MyError {
        let reason: String = match code {
            400 => "Bad Request".to_string(),
            401 => "Unauthorized".to_string(),
            _ => "Error".to_string(),
        };

        MyError {
            error: ErrorContent {
                code,
                reason,
                description,
            },
        }
    }
}

impl<'r> rocket::response::Responder<'r, 'static> for MyError {
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
