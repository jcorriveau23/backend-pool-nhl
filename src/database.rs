use crate::settings::SETTINGS;

pub async fn new() -> mongodb::Database {
    mongodb::Client::with_uri_str(SETTINGS.database.uri.as_str())
        .await
        .expect("Failed to initialize MongoDB connection")
        .database(SETTINGS.database.name.as_str())
}
