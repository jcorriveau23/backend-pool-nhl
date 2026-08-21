//! Integration tests for the read-through day_leaders cache.
//!
//! They need a running mongo and redis:
//!   docker compose up -d mongo redis
//!   cargo test -p poolnhl_infrastructure -- --ignored
//!
//! The test seeds a uniquely-dated document in the dedicated `hockeypooltest`
//! database (never the seeded `hockeypool` one) and cleans up after itself.

use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::Collection;
use mongodb::bson::doc;
use redis::AsyncCommands;

use poolnhl_infrastructure::database_connection::{DatabaseConnection, DatabaseManager};
use poolnhl_infrastructure::redis_connection::RedisManager;
use poolnhl_infrastructure::services::day_leaders_cache::DayLeadersCache;
use poolnhl_interface::daily_leaders::model::{
    DailyGoaly, DailyLeaders, DailySkater, GoalyStats, SkaterStats,
};

const TEST_DATABASE: &str = "hockeypooltest";

fn mongo_uri() -> String {
    std::env::var("TEST_MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string())
}

fn redis_uri() -> String {
    std::env::var("TEST_REDIS_URI").unwrap_or_else(|_| "redis://localhost:6379".to_string())
}

async fn database() -> DatabaseConnection {
    DatabaseManager::new_pool(&mongo_uri(), TEST_DATABASE)
        .await
        .expect("mongo is not reachable; start it with `docker compose up -d mongo`")
}

// A unique, collision-free key for parallel runs. The cache never parses the
// date, so any unique string works and keeps runs isolated in both stores.
fn unique_date() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test-day-{nanos}")
}

#[tokio::test]
#[ignore]
async fn day_scores_are_derived_read_through_and_cached() {
    let db = database().await;
    let (_, redis) = RedisManager::connect(&redis_uri())
        .await
        .expect("redis is not reachable; start it with `docker compose up -d redis`");
    let leaders: Collection<DailyLeaders> = db.collection("day_leaders");
    let date = unique_date();

    // Seed a raw day_leaders doc: a hat-trick skater and a shutout goalie.
    leaders
        .insert_one(
            DailyLeaders {
                date: date.clone(),
                skaters: vec![DailySkater {
                    name: "Hat Trick".into(),
                    id: 101,
                    team: 1,
                    stats: SkaterStats {
                        goals: 3,
                        assists: 1,
                        shootoutGoals: 1,
                    },
                }],
                goalies: vec![DailyGoaly {
                    name: "Perfect Night".into(),
                    id: 501,
                    team: 2,
                    stats: GoalyStats {
                        goals: 0,
                        assists: 0,
                        decision: Some("W".into()),
                        savePercentage: Some(1.0),
                        OT: None,
                    },
                }],
                played: vec![101, 501],
            },
            None,
        )
        .await
        .unwrap();

    let cache = DayLeadersCache::new(db.clone(), redis.clone());

    // First call: cache miss -> derived from Mongo.
    let scores = cache.day_scores(&date).await.unwrap();
    let skater = scores.skaters.get(&101).expect("skater derived");
    assert_eq!((skater.G, skater.A, skater.SOG), (3, 1, Some(1)));
    let goalie = scores.goalies.get(&501).expect("goalie derived");
    assert!(
        goalie.W && goalie.SO && !goalie.OT,
        "win with a perfect save % must derive a shutout"
    );

    // The value is now cached under the versioned key.
    let mut conn = redis.clone();
    let key = format!("dl:v2:{date}");
    let cached: Option<String> = conn.get(&key).await.unwrap();
    assert!(
        cached.is_some(),
        "day scores should be cached after first read"
    );

    // Second call is served from cache: deleting the Mongo doc must not change it.
    leaders
        .delete_one(doc! {"date": &date}, None)
        .await
        .unwrap();
    let again = cache.day_scores(&date).await.unwrap();
    assert!(
        again.skaters.contains_key(&101) && again.goalies.contains_key(&501),
        "second read must be served from the cache, not Mongo"
    );

    // Cleanup the cache key (the Mongo doc is already gone).
    let _: () = conn.del(&key).await.unwrap();
}
