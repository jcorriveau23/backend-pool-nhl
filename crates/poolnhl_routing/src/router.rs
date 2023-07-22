use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;

use crate::endpoints::users_endpoints::UsersRouter;

pub struct ApplicationController;

impl ApplicationController {
    pub async fn run(port: u16, service_registry: ServiceRegistry) {
        let router: Router = Router::new().merge(UsersRouter::new(service_registry));

        axum::Server::bind(&format!("127.0.0.1:{}", port).parse().unwrap())
            .serve(router.into_make_service())
            .await
            .expect("Failed to start the server");
    }
}
