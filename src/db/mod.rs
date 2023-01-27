use crate::errors::response::Result;
use mongodb::options::ClientOptions;
use mongodb::{Client, Database};
use rocket::fairing::AdHoc;

// collection library.
pub mod daily_leaders;
pub mod pool;
pub mod user;

pub fn init() -> AdHoc {
    AdHoc::on_ignite("Connecting to MongoDB", |rocket| async {
        match connect().await {
            Ok(database) => rocket.manage(database),
            Err(error) => {
                panic!("Cannot connect to instance:: {:?}", error)
            }
        }
    })
}

async fn connect() -> Result<Database> {
    let client_options = ClientOptions::parse("mongodb://localhost:27017").await?;

    // mongoDB client
    let client = Client::with_options(client_options)?;

    // mongoDB database
    let db = client.database("hockeypool");

    println!("MongoDB Connected!");

    Ok(db)
}
