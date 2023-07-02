use mongodb::bson::doc;

use poolnhl_interface::errors::{AppError, Result};

pub type DatabaseConnection = mongodb::Database;

pub struct DatabaseManager;

impl DatabaseManager {
    pub async fn new_pool(database_uri: &str, database_name: &str) -> Result<DatabaseConnection> {
        let db = mongodb::Client::with_uri_str(database_uri)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?
            .database(database_name);

        db.run_command(doc! {"ping": 1}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        Ok(db)
    }
}
