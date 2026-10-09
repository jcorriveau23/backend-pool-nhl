use std::sync::Arc;

use axum::extract::FromRef;

use crate::redis_connection::{RedisManager, spawn_room_subscriber};
use crate::{database_connection::DatabaseConnection, jwt::CachedJwks};
use poolnhl_interface::daily_leaders::service::DailyLeadersServiceHandle;
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::errors::Result;
use poolnhl_interface::players::service::PlayersServiceHandle;
use poolnhl_interface::pool::service::PoolServiceHandle;
use poolnhl_interface::survivor::service::SurvivorServiceHandle;

pub mod daily_leaders_service;
pub mod day_leaders_cache;
pub mod draft_service;
pub mod draft_state;
pub mod nhl_schedule_service;
pub mod players_service;
pub mod pool_scoring_service;
pub mod pool_service;
pub mod survivor_service;

use daily_leaders_service::MongoDailyLeadersService;
use day_leaders_cache::DayLeadersCache;
use draft_service::MongoDraftService;
use draft_state::{DraftServerState, LocalRooms, spawn_heartbeat};
use nhl_schedule_service::NhlScheduleService;
use players_service::MongoPlayersService;
use pool_scoring_service::PoolScoringService;
use pool_service::MongoPoolService;
use survivor_service::MongoSurvivorService;
#[derive(FromRef, Clone)]
pub struct ServiceRegistry {
    pub pool_service: PoolServiceHandle,
    pub players_service: PlayersServiceHandle,
    pub draft_service: DraftServiceHandle,
    pub daily_leaders_service: DailyLeadersServiceHandle,
    pub pool_scoring_service: PoolScoringService,
    pub survivor_service: SurvivorServiceHandle,

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

        // Shared, read-through cache of the compact day_leaders projection, used
        // to derive pool scores on demand from lineup events + daily stats.
        let day_leaders_cache = DayLeadersCache::new(db.clone(), redis_conn.clone());
        let pool_scoring_service = PoolScoringService::new(day_leaders_cache);

        let local_rooms = LocalRooms::new();
        let subscriber = spawn_room_subscriber(redis_client, local_rooms.clone());
        let draft_state = Arc::new(DraftServerState::new(local_rooms, redis_conn, subscriber));
        spawn_heartbeat(draft_state.clone());

        let pool_service = Arc::new(MongoPoolService::new(db.clone()));
        let players_service = Arc::new(MongoPlayersService::new(db.clone()));
        // The league's own scoreboard. A survivor pool eliminates people, so
        // who won is read here rather than taken from a client.
        let survivor_service = Arc::new(MongoSurvivorService::new(
            db.clone(),
            NhlScheduleService::new(),
        ));
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
            pool_scoring_service,
            survivor_service,
            cached_keys: cached_jwks.clone(),
        })
    }
}
