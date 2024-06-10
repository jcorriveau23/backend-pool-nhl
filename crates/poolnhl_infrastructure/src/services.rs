use std::sync::Arc;

use axum::extract::FromRef;

use crate::database_connection::DatabaseConnection;
use poolnhl_interface::daily_leaders::service::DailyLeadersServiceHandle;
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::nhl::service::NhlServiceHandle;
use poolnhl_interface::pool::service::PoolServiceHandle;

pub mod daily_leaders_service;
pub mod draft_service;
pub mod nhl_service;
pub mod pool_service;

use daily_leaders_service::MongoDailyLeadersService;
use draft_service::MongoDraftService;
use nhl_service::MongoNhlService;
use pool_service::MongoPoolService;

use crate::settings::Settings;

#[derive(FromRef, Clone)]
pub struct ServiceRegistry {
    pub pool_service: PoolServiceHandle,
    pub nhl_service: NhlServiceHandle,
    pub draft_service: DraftServiceHandle,
    pub daily_leaders_service: DailyLeadersServiceHandle,

    pub jwks_url: String,
}

impl ServiceRegistry {
    pub fn new(db: DatabaseConnection, settings: &Settings) -> Self {
        let pool_service = Arc::new(MongoPoolService::new(db.clone()));
        let nhl_service = Arc::new(MongoNhlService::new(db.clone()));
        let draft_service = Arc::new(MongoDraftService::new(
            db.clone(),
            settings.auth.jwks_url.clone(),
        ));
        let daily_leaders_service = Arc::new(MongoDailyLeadersService::new(db));

        Self {
            pool_service,
            nhl_service,
            draft_service,
            daily_leaders_service,
            jwks_url: settings.auth.jwks_url.clone(),
        }
    }
}
