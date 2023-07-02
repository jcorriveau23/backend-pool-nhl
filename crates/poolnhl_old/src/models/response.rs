use serde::{Deserialize, Serialize};

use crate::models::pool::Pool;

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageResponse {
    /// This is a message from the server.
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PoolMessageResponse {
    /// This is a message from the server.
    pub success: bool,
    pub message: String,
    pub pool: Pool,
}
