use mongodb::bson::doc;

use poolnhl_interface::errors::{AppError, Result};

pub type DatabaseConnection = mongodb::Database;

pub fn mongo_err(e: mongodb::error::Error) -> AppError {
    AppError::MongoError { msg: e.to_string() }
}

pub fn bson_err(e: mongodb::bson::ser::Error) -> AppError {
    AppError::BsonError { msg: e.to_string() }
}

pub struct DatabaseManager;

impl DatabaseManager {
    pub async fn new_pool(database_uri: &str, database_name: &str) -> Result<DatabaseConnection> {
        let db = mongodb::Client::with_uri_str(database_uri)
            .await
            .map_err(mongo_err)?
            .database(database_name);

        db.run_command(doc! {"ping": 1}, None)
            .await
            .map_err(mongo_err)?;

        Ok(db)
    }
}
