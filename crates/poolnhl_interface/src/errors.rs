use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub enum AppError {
    // A business rule was violated by the caller (not enough room on the
    // roster, trading after the deadline, ...). The message is written for the
    // end user and is safe to send back as-is. Maps to 400.
    CustomError { msg: String },
    // A requested resource (pool, daily leaders, ...) does not exist. Maps to
    // 404 so clients can tell "you asked for something that isn't here" apart
    // from "the server failed" (500).
    NotFound { msg: String },
    // The caller is not authenticated, or its token is invalid/expired. Maps to
    // 401 so the front end knows to send the user through Hanko again.
    AuthError { msg: String },
    // The caller is authenticated but lacks the rights for this action (not a
    // participant, not an assistant, not the owner). Maps to 403.
    ForbiddenError { msg: String },
    // The document changed between the read and the write (someone else acted
    // on the pool concurrently). Maps to 409 so the client can refetch and
    // retry instead of silently losing its update.
    ConflictError { msg: String },
    // The request could not be parsed (bad date format, out-of-range span,
    // ...). Maps to 400.
    ParseError { msg: String },
    MongoError { msg: String },
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

impl std::error::Error for AppError {}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::CustomError { msg } => write!(f, "Custom Error: '{msg}'"),
            AppError::NotFound { msg } => write!(f, "Not Found: '{msg}'"),
            AppError::AuthError { msg } => write!(f, "Authentication Error: '{msg}'"),
            AppError::ForbiddenError { msg } => write!(f, "Forbidden: '{msg}'"),
            AppError::ConflictError { msg } => write!(f, "Conflict: '{msg}'"),
            AppError::ParseError { msg } => write!(f, "Parse Error: '{msg}'"),
            AppError::MongoError { msg } => write!(f, "MongoDB Error: '{msg}'"),
            AppError::BcryptError { msg } => write!(f, "Bcrypt Error: '{msg}'"),
            AppError::HexError { msg } => write!(f, "Hex Error: '{msg}'"),
            AppError::RecoveryError { msg } => write!(f, "Recovery Error: '{msg}'"),
            AppError::BsonError { msg } => write!(f, "Bson Serialization Error: '{msg}'"),
            AppError::JwtError { msg } => write!(f, "Jwt Decoding Error: '{msg}'"),
            AppError::ObjectIdError { msg } => write!(f, "string to object ID Error: '{msg}'"),
            AppError::ReqwestError { msg } => write!(f, "Reqwest Error: '{msg}'"),
            AppError::NonMatchingKid { msg } => write!(f, "Non matching kid Error: '{msg}'"),
            AppError::RwLockError { msg } => write!(f, "Mutex locking error '{msg}'"),
            AppError::RedisError { msg } => write!(f, "Redis Error: '{msg}'"),
        }
    }
}

// Conversions for the libraries this crate already depends on, so callers can
// `?` instead of repeating `.map_err(...)`. The driver errors (mongo, redis)
// are converted in `poolnhl_infrastructure` instead: they must not become a
// dependency of the domain crate.
impl From<chrono::ParseError> for AppError {
    fn from(e: chrono::ParseError) -> Self {
        AppError::ParseError { msg: e.to_string() }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::ParseError { msg: e.to_string() }
    }
}

impl AppError {
    /// The HTTP status this error maps to.
    ///
    /// Anything not explicitly listed is a server fault: the caller could not
    /// have avoided it, and its message describes our internals.
    fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound { .. } => StatusCode::NOT_FOUND,
            AppError::CustomError { .. } | AppError::ParseError { .. } => StatusCode::BAD_REQUEST,
            AppError::AuthError { .. }
            | AppError::JwtError { .. }
            | AppError::NonMatchingKid { .. } => StatusCode::UNAUTHORIZED,
            AppError::ForbiddenError { .. } => StatusCode::FORBIDDEN,
            AppError::ConflictError { .. } => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();

        // A 5xx message describes our internals (mongo topology, redis URIs,
        // connection strings). Log the detail server-side and hand the client a
        // generic message instead. 4xx messages are written for the end user
        // and are returned as-is so the front end can display them.
        let body = if status.is_server_error() {
            tracing::error!(error = %self, status = %status, "request failed");
            "Internal server error".to_string()
        } else {
            self.to_string()
        };

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_errors_map_to_their_own_status() {
        let cases = [
            (
                AppError::NotFound { msg: "x".into() },
                StatusCode::NOT_FOUND,
            ),
            (
                AppError::CustomError { msg: "x".into() },
                StatusCode::BAD_REQUEST,
            ),
            (
                AppError::AuthError { msg: "x".into() },
                StatusCode::UNAUTHORIZED,
            ),
            (
                AppError::JwtError { msg: "x".into() },
                StatusCode::UNAUTHORIZED,
            ),
            (
                AppError::ForbiddenError { msg: "x".into() },
                StatusCode::FORBIDDEN,
            ),
            (
                AppError::ConflictError { msg: "x".into() },
                StatusCode::CONFLICT,
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.status(), expected, "{error}");
        }
    }

    #[test]
    fn infrastructure_errors_map_to_500() {
        for error in [
            AppError::MongoError { msg: "x".into() },
            AppError::RedisError { msg: "x".into() },
            AppError::BsonError { msg: "x".into() },
            AppError::RwLockError { msg: "x".into() },
        ] {
            assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR, "{error}");
        }
    }

    // A 500 must never carry the underlying driver message to the client.
    #[tokio::test]
    async fn server_errors_do_not_leak_their_message() {
        let response = AppError::MongoError {
            msg: "mongodb://user:password@cluster.internal".into(),
        }
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert_eq!(body, "Internal server error");
        assert!(!body.contains("password"));
    }

    // A 4xx message is written for the end user and must survive intact.
    #[tokio::test]
    async fn client_errors_keep_their_message() {
        let response = AppError::CustomError {
            msg: "You do not possess 'Connor McDavid'.".into(),
        }
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();

        assert!(body.contains("Connor McDavid"));
    }
}
