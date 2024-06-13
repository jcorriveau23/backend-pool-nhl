use std::sync::Arc;

use poolnhl_infrastructure::{
    database_connection::DatabaseManager, jwt::CachedJwks, services::ServiceRegistry,
    settings::Settings,
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

    // query and cached the JSON Web key set fetch from hanko.
    // This will allow to validate the JWT sent to the application.
    let cached_jwks = Arc::new(
        CachedJwks::new(&settings.auth)
            .await
            .expect("Was not able to query the JWKS from hanko server."),
    );
    let services = ServiceRegistry::new(db, cached_jwks);

    // Run the application.
    ApplicationController::run(settings, services).await;
}
