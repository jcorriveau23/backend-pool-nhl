use std::sync::Arc;

use axum::extract::FromRef;

use crate::redis_connection::{spawn_room_subscriber, RedisManager};
use crate::{database_connection::DatabaseConnection, jwt::CachedJwks};
use poolnhl_interface::daily_leaders::service::DailyLeadersServiceHandle;
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::errors::Result;
use poolnhl_interface::players::service::PlayersServiceHandle;
use poolnhl_interface::pool::service::PoolServiceHandle;

pub mod daily_leaders_service;
pub mod draft_service;
pub mod draft_state;
pub mod players_service;
pub mod pool_service;

use daily_leaders_service::MongoDailyLeadersService;
use draft_service::MongoDraftService;
use draft_state::{spawn_heartbeat, DraftServerState, LocalRooms};
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
    pub async fn new(
        db: DatabaseConnection,
        cached_jwks: Arc<CachedJwks>,
        redis_uri: &str,
    ) -> Result<Self> {
        // Draft rooms state is shared across instances through redis: pub/sub
        // for the room broadcasts, hashes for the room membership/presence.
        let (redis_client, redis_conn) = RedisManager::connect(redis_uri).await?;
        let local_rooms = LocalRooms::new();
        let subscriber = spawn_room_subscriber(redis_client, local_rooms.clone());
        let draft_state = Arc::new(DraftServerState::new(local_rooms, redis_conn, subscriber));
        spawn_heartbeat(draft_state.clone());

        let pool_service = Arc::new(MongoPoolService::new(db.clone()));
        let players_service = Arc::new(MongoPlayersService::new(db.clone()));
        let draft_service = Arc::new(MongoDraftService::new(
            db.clone(),
            cached_jwks.clone(),
            draft_state,
        ));
        let daily_leaders_service = Arc::new(MongoDailyLeadersService::new(db));

        Ok(Self {
            pool_service,
            players_service,
            draft_service,
            daily_leaders_service,
            cached_keys: cached_jwks.clone(),
        })
    }
}
