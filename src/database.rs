use async_once::AsyncOnce;
use lazy_static::lazy_static;
use mongodb::Database;

use crate::settings::SETTINGS;

lazy_static! {
    // Connection to the mongodb database.
    pub static ref CONNECTION: AsyncOnce<Database> = AsyncOnce::new(async {
        let db_uri = SETTINGS.database.uri.as_str();
        let db_name = SETTINGS.database.name.as_str();

        mongodb::Client::with_uri_str(db_uri)
            .await
            .expect("Failed to initialize MongoDB connection")
            .database(db_name)
    });
}
