//! Integration tests for the Mongo-backed lineup store.
//!
//!   docker compose up -d mongo
//!   cargo test -p poolnhl_infrastructure -- --ignored
//!
//! Uses the dedicated `hockeypooltest` database and a uniquely-named pool.

use std::time::{SystemTime, UNIX_EPOCH};

use poolnhl_infrastructure::database_connection::{DatabaseConnection, DatabaseManager};
use poolnhl_infrastructure::services::lineup_store::LineupStore;
use poolnhl_interface::pool::lineup::LineupEvent;

const TEST_DATABASE: &str = "hockeypooltest";

fn mongo_uri() -> String {
    std::env::var("TEST_MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string())
}

async fn database() -> DatabaseConnection {
    DatabaseManager::new_pool(&mongo_uri(), TEST_DATABASE)
        .await
        .expect("mongo is not reachable; start it with `docker compose up -d mongo`")
}

fn unique_pool() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("lineup-store-test-{nanos}")
}

fn event(pool: &str, participant: &str, date: &str, forwards: Vec<u32>) -> LineupEvent {
    LineupEvent {
        pool_name: pool.to_string(),
        participant: participant.to_string(),
        effective_date: date.to_string(),
        forwards,
        defense: vec![],
        goalies: vec![],
    }
}

#[tokio::test]
#[ignore]
async fn events_round_trip_grouped_sorted_and_idempotent() {
    let store = LineupStore::new(database().await);
    let pool = unique_pool();

    // Insert out of date order and across two participants.
    let events = vec![
        event(&pool, "u1", "2025-10-04", vec![12]),
        event(&pool, "u1", "2025-10-01", vec![10, 11]),
        event(&pool, "u2", "2025-10-01", vec![20]),
    ];
    store.replace_pool_events(&pool, &events).await.unwrap();

    let by_participant = store.events_for_pool(&pool).await.unwrap();
    assert_eq!(by_participant.len(), 2);

    // u1's events come back sorted by effective_date ascending.
    let u1 = &by_participant["u1"];
    assert_eq!(u1.len(), 2);
    assert_eq!(u1[0].effective_date, "2025-10-01");
    assert_eq!(u1[0].forwards, vec![10, 11]);
    assert_eq!(u1[1].effective_date, "2025-10-04");

    // Re-running the migration replaces rather than duplicates.
    store.replace_pool_events(&pool, &events).await.unwrap();
    let again = store.events_for_pool(&pool).await.unwrap();
    assert_eq!(again["u1"].len(), 2);

    // Cleanup.
    store.replace_pool_events(&pool, &[]).await.unwrap();
    assert!(store.events_for_pool(&pool).await.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn record_if_changed_writes_only_on_change() {
    let store = LineupStore::new(database().await);
    let pool = unique_pool();

    // First lineup is recorded.
    assert!(store
        .record_if_changed(&pool, "u1", "2025-10-01", vec![10, 11], vec![], vec![30])
        .await
        .unwrap());
    // Same lineup (even reordered) is not recorded again.
    assert!(!store
        .record_if_changed(&pool, "u1", "2025-10-05", vec![11, 10], vec![], vec![30])
        .await
        .unwrap());
    // A real change is recorded.
    assert!(store
        .record_if_changed(&pool, "u1", "2025-10-08", vec![10, 12], vec![], vec![30])
        .await
        .unwrap());

    let by_participant = store.events_for_pool(&pool).await.unwrap();
    let u1 = &by_participant["u1"];
    assert_eq!(u1.len(), 2);
    assert_eq!(u1[0].effective_date, "2025-10-01");
    assert_eq!(u1[1].effective_date, "2025-10-08");
    assert_eq!(u1[1].forwards, vec![10, 12]);

    store.replace_pool_events(&pool, &[]).await.unwrap();
}
