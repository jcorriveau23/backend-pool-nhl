use std::sync::Arc;

use axum::extract::FromRef;

use crate::{database_connection::DatabaseConnection, jwt::CachedJwks};
use poolnhl_interface::daily_leaders::service::DailyLeadersServiceHandle;
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::players::service::PlayersServiceHandle;
use poolnhl_interface::pool::service::PoolServiceHandle;

pub mod daily_leaders_service;
pub mod draft_service;
pub mod players_service;
pub mod pool_service;

use daily_leaders_service::MongoDailyLeadersService;
use draft_service::MongoDraftService;
use players_service::MongoPlayersService;
use pool_service::MongoPoolService;
#[derive(FromRef, Clone)]
pub struct ServiceRegistry {
    pub pool_service: PoolServiceHandle,
    pub players_service: PlayersServiceHandle,
    pub draft_service: DraftServiceHandle,
    pub daily_leaders_service: DailyLeadersServiceHandle,

    pub cached_keys: Arc<CachedJwks>,
}

impl ServiceRegistry {
    pub fn new(db: DatabaseConnection, cached_jwks: Arc<CachedJwks>) -> Self {
        let pool_service = Arc::new(MongoPoolService::new(db.clone()));
        let players_service = Arc::new(MongoPlayersService::new(db.clone()));
        let draft_service = Arc::new(MongoDraftService::new(db.clone(), cached_jwks.clone()));
        let daily_leaders_service = Arc::new(MongoDailyLeadersService::new(db));

        Self {
            pool_service,
            players_service,
            draft_service,
            daily_leaders_service,
            cached_keys: cached_jwks.clone(),
        }
    }
}
