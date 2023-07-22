use std::sync::Arc;

use axum::extract::FromRef;

use poolnhl_interface::users::service::UsersServiceHandle;

use crate::database_connection::DatabaseConnection;

pub mod users_service;

use users_service::MongoUsersService;

use crate::settings::Settings;

#[derive(FromRef, Clone)]
pub struct ServiceRegistry {
    pub users_service: UsersServiceHandle,
    pub secret: String,
}

impl ServiceRegistry {
    pub fn new(db: DatabaseConnection, _settings: &Settings) -> Self {
        let users_service = Arc::new(MongoUsersService::new(
            db.clone(),
            _settings.auth.secret.clone(),
        ));

        Self {
            users_service: users_service,
            secret: _settings.auth.secret.clone(),
        }
    }
}
