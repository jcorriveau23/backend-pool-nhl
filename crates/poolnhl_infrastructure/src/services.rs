use std::sync::Arc;

use axum::extract::FromRef;

use crate::database_connection::DatabaseConnection;
use poolnhl_interface::daily_leaders::service::DailyLeadersServiceHandle;
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::nhl::service::NhlServiceHandle;
use poolnhl_interface::pool::service::PoolServiceHandle;
use poolnhl_interface::users::service::UsersServiceHandle;

pub mod daily_leaders_service;
pub mod draft_service;
pub mod nhl_service;
pub mod pool_service;
pub mod users_service;

use daily_leaders_service::MongoDailyLeadersService;
use draft_service::MongoDraftService;
use nhl_service::MongoNhlService;
use pool_service::MongoPoolService;
use users_service::MongoUsersService;

use crate::settings::Settings;

#[derive(FromRef, Clone)]
pub struct ServiceRegistry {
    pub users_service: UsersServiceHandle,
    pub pool_service: PoolServiceHandle,
    pub nhl_service: NhlServiceHandle,
    pub draft_service: DraftServiceHandle,
    pub daily_leaders_service: DailyLeadersServiceHandle,
    pub secret: String,
}

impl ServiceRegistry {
    pub fn new(db: DatabaseConnection, settings: &Settings) -> Self {
        let users_service = Arc::new(MongoUsersService::new(
            db.clone(),
            settings.auth.secret.clone(),
        ));
        let pool_service = Arc::new(MongoPoolService::new(db.clone()));
        let nhl_service = Arc::new(MongoNhlService::new(db.clone()));
        let draft_service = Arc::new(MongoDraftService::new(
            db.clone(),
            settings.auth.secret.clone(),
        ));
        let daily_leaders_service = Arc::new(MongoDailyLeadersService::new(db));

        Self {
            users_service,
            pool_service,
            nhl_service,
            draft_service,
            daily_leaders_service,
            secret: settings.auth.secret.clone(),
        }
    }
}
