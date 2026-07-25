//! Integration tests for the mongo-backed pool service.
//!
//! They need a running mongo:
//!   docker compose up -d mongo
//!   cargo test -p poolnhl_infrastructure -- --ignored
//!
//! The tests run against a dedicated `hockeypooltest` database (never the
//! seeded `hockeypool` one) and each test uses a uniquely-named pool, so
//! tests can run in parallel and leave the dev data untouched.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::Collection;

use poolnhl_infrastructure::database_connection::{DatabaseConnection, DatabaseManager};
use poolnhl_infrastructure::services::pool_service::MongoPoolService;
use poolnhl_interface::errors::AppError;
use poolnhl_interface::players::model::{PlayerInfo, Position};
use poolnhl_interface::pool::model::{
    DailyRosterPoints, Pool, PoolContext, PoolCreationRequest, PoolDeletionRequest, PoolSettings,
    PoolState, PoolUser, RespondTradeRequest, Roster, Trade, TradeItems, TradeStatus,
};
use poolnhl_interface::pool::service::PoolService;

const TEST_DATABASE: &str = "hockeypooltest";

const OWNER: &str = "owner";
const USER_2: &str = "user-2";

fn mongo_uri() -> String {
    std::env::var("TEST_MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string())
}

async fn database() -> DatabaseConnection {
    DatabaseManager::new_pool(&mongo_uri(), TEST_DATABASE)
        .await
        .expect("mongo is not reachable; start it with `docker compose up -d mongo`")
}

// The service under test plus a raw collection handle for setup/assertions.
async fn service_and_collection() -> (MongoPoolService, Collection<Pool>) {
    let db = database().await;
    (MongoPoolService::new(db.clone()), db.collection("pools"))
}

fn unique_pool_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test-{}-{}", prefix, nanos)
}

fn player(id: u32) -> PlayerInfo {
    PlayerInfo {
        active: true,
        id,
        name: format!("player-{}", id),
        team: None,
        position: Position::F,
        age: None,
        salary_cap: None,
        contract_expiration_season: None,
        game_played: None,
        goals: None,
        assists: None,
        points: None,
        points_per_game: None,
        goal_against_average: None,
        save_percentage: None,
        saves: None,
        shots: None,
        wins: None,
        ot: None,
    }
}

fn empty_day() -> DailyRosterPoints {
    DailyRosterPoints {
        roster: Roster {
            F: HashMap::new(),
            D: HashMap::new(),
            G: HashMap::new(),
        },
        is_cumulated: true,
    }
}

// An InProgress two-user pool where each participant owns one reservist
// (owner: player 1, user-2: player 2) and three days of scores are recorded.
fn in_progress_pool(name: &str) -> Pool {
    let mut pool = Pool::new(name, OWNER, &PoolSettings::new());
    pool.status = PoolState::InProgress;
    pool.participants = [OWNER, USER_2]
        .iter()
        .map(|id| PoolUser {
            id: id.to_string(),
            name: id.to_string(),
            is_owned: true,
        })
        .collect();

    let ids: Vec<String> = [OWNER, USER_2].iter().map(|id| id.to_string()).collect();
    let mut context = PoolContext::new(&ids);
    context.pooler_roster.get_mut(OWNER).unwrap().chosen_reservists = vec![1];
    context.pooler_roster.get_mut(USER_2).unwrap().chosen_reservists = vec![2];
    context.players.insert("1".to_string(), player(1));
    context.players.insert("2".to_string(), player(2));

    let mut score_by_day = HashMap::new();
    for date in ["2025-12-01", "2025-12-02", "2025-12-03"] {
        let mut day = HashMap::new();
        day.insert(OWNER.to_string(), empty_day());
        day.insert(USER_2.to_string(), empty_day());
        score_by_day.insert(date.to_string(), day);
    }
    context.score_by_day = Some(score_by_day);

    pool.context = Some(context);
    pool
}

async fn cleanup(collection: &Collection<Pool>, pool_name: &str) {
    let _ = collection
        .delete_one(mongodb::bson::doc! {"name": pool_name}, None)
        .await;
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn create_pool_roundtrip_and_unique_name_index() {
    let (service, collection) = service_and_collection().await;
    service.init_indexes().await.unwrap();
    let pool_name = unique_pool_name("create");

    let request = PoolCreationRequest {
        pool_name: pool_name.clone(),
        settings: PoolSettings::new(),
    };
    let created = service.create_pool(OWNER, request.clone()).await.unwrap();
    assert_eq!(created.owner, OWNER);
    assert!(matches!(created.status, PoolState::Created));

    // The pool is persisted and readable back.
    let fetched = service.get_pool_by_name(&pool_name).await.unwrap();
    assert_eq!(fetched.name, pool_name);
    assert_eq!(fetched.owner, OWNER);

    // The unique index on the name rejects a second pool with the same name.
    let duplicate = service.create_pool(USER_2, request).await;
    assert!(matches!(duplicate, Err(AppError::MongoError { .. })));

    cleanup(&collection, &pool_name).await;
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn get_pool_by_name_with_range_prunes_the_earlier_days() {
    let (service, collection) = service_and_collection().await;
    let pool_name = unique_pool_name("range");
    collection
        .insert_one(&in_progress_pool(&pool_name), None)
        .await
        .unwrap();

    let fetched = service
        .get_pool_by_name_with_range(&pool_name, "2025-12-01", "2025-12-02")
        .await
        .unwrap();

    // Days before the requested from-date are projected out, the rest stay.
    let score_by_day = fetched.context.unwrap().score_by_day.unwrap();
    assert!(!score_by_day.contains_key("2025-12-01"));
    assert!(score_by_day.contains_key("2025-12-02"));
    assert!(score_by_day.contains_key("2025-12-03"));

    cleanup(&collection, &pool_name).await;
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn respond_trade_persists_the_roster_swap() {
    let (service, collection) = service_and_collection().await;
    let pool_name = unique_pool_name("trade");
    let mut pool = in_progress_pool(&pool_name);
    // A pending trade, created more than 24h ago, of reservist 1 for 2.
    pool.trades = Some(vec![Trade {
        proposed_by: OWNER.to_string(),
        ask_to: USER_2.to_string(),
        from_items: TradeItems {
            players: vec![1],
            picks: Vec::new(),
        },
        to_items: TradeItems {
            players: vec![2],
            picks: Vec::new(),
        },
        status: TradeStatus::NEW,
        id: 0,
        date_created: 0,
        date_accepted: 0,
    }]);
    collection.insert_one(&pool, None).await.unwrap();

    let updated = service
        .respond_trade(
            USER_2,
            RespondTradeRequest {
                pool_name: pool_name.clone(),
                trade_id: 0,
                is_accepted: true,
            },
        )
        .await
        .unwrap();

    // The returned document holds the swapped rosters and the accepted trade.
    let roster = &updated.context.as_ref().unwrap().pooler_roster;
    assert_eq!(roster[OWNER].chosen_reservists, vec![2]);
    assert_eq!(roster[USER_2].chosen_reservists, vec![1]);
    assert!(matches!(
        updated.trades.as_ref().unwrap()[0].status,
        TradeStatus::ACCEPTED
    ));
    // update_pool projects the heavy score_by_day field out of its response.
    assert!(updated.context.as_ref().unwrap().score_by_day.is_none());

    // The swap survived a full round-trip to the database.
    let fetched = service.get_pool_by_name(&pool_name).await.unwrap();
    let roster = &fetched.context.as_ref().unwrap().pooler_roster;
    assert_eq!(roster[OWNER].chosen_reservists, vec![2]);
    assert_eq!(roster[USER_2].chosen_reservists, vec![1]);

    cleanup(&collection, &pool_name).await;
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn delete_pool_requires_the_owner() {
    let (service, collection) = service_and_collection().await;
    let pool_name = unique_pool_name("delete");
    service
        .create_pool(
            OWNER,
            PoolCreationRequest {
                pool_name: pool_name.clone(),
                settings: PoolSettings::new(),
            },
        )
        .await
        .unwrap();

    // Someone else cannot delete the pool.
    let request = PoolDeletionRequest {
        pool_name: pool_name.clone(),
    };
    assert!(service.delete_pool(USER_2, request.clone()).await.is_err());

    // The owner can, after which the pool is gone.
    service.delete_pool(OWNER, request).await.unwrap();
    let fetched = service.get_pool_by_name(&pool_name).await;
    assert!(matches!(fetched, Err(AppError::NotFound { .. })));

    cleanup(&collection, &pool_name).await;
}
