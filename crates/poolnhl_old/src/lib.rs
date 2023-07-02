pub mod database;
pub mod db;
pub mod errors;
pub mod logger;
pub mod models;
pub mod routes;
pub mod settings;

#[derive(Clone)]
pub struct AppState {
    pub db: mongodb::Database,
}
