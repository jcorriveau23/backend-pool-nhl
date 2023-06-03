use crate::errors::response::Result;
use mongodb::bson::doc;
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
    let client_options = ClientOptions::parse(
        "mongodb+srv://<user>:<psw>@cluster0.fxxbzrj.mongodb.net/?retryWrites=true&w=majority",
    )
    .await?;

    // mongoDB client
    let client = Client::with_options(client_options)?;

    // mongoDB database
    let db = client.database("hockeypool");

    client
        .database("hockeypool")
        .run_command(doc! {"ping": 1}, None)
        .await?;

    println!("MongoDB Connected!");

    Ok(db)
}
