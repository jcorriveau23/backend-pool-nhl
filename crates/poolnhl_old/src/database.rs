use crate::errors::response::AppError;
use crate::settings::SETTINGS;
use mongodb::bson::doc;

pub async fn new() -> Result<mongodb::Database, AppError> {
    let uri = SETTINGS.database.uri.as_str();
    let database = SETTINGS.database.name.as_str();
    println!("Mongodb uri: {uri}");
    println!("Database name: {database}");

    let db = mongodb::Client::with_uri_str(uri).await?.database(database);

    db.run_command(doc! {"ping": 1}, None).await?;

    Ok(db)
}
