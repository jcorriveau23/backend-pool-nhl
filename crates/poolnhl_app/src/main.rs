use poolnhl_infrastructure::{
    database_connection::DatabaseManager, services::ServiceRegistry, settings::Settings,
};

use poolnhl_routing::router::ApplicationController;

#[tokio::main]
async fn main() {
    let settings = Settings::new().expect("Could not parse settings");

    // Make the database connection.
    let db = DatabaseManager::new_pool(
        settings.database.uri.as_str(),
        settings.database.name.as_str(),
    )
    .await
    .expect("Could not initialize the database");

    // setup the services to be served.
    let services = ServiceRegistry::new(db, &settings);

    // Run the application.
    ApplicationController::run(settings, services).await;
}
