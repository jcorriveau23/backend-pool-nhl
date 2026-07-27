//! Shadow-compare test for the derive path: scores computed from the shared
//! `day_leaders` must match what the stored `score_by_day` produces.
//!
//! Needs mongo + redis:
//!   docker compose up -d mongo redis
//!   cargo test -p poolnhl_infrastructure -- --ignored
//!
//! Seeds a uniquely-dated day in the dedicated `hockeypooltest` database whose
//! stored points are consistent with the seeded leaders, so the derived ranking
//! must equal the stored ranking exactly. Cleans up after itself.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::bson::doc;
use mongodb::Collection;
use redis::AsyncCommands;

use poolnhl_infrastructure::database_connection::{DatabaseConnection, DatabaseManager};
use poolnhl_infrastructure::redis_connection::RedisManager;
use poolnhl_infrastructure::services::day_leaders_cache::DayLeadersCache;
use poolnhl_infrastructure::services::pool_scoring_service::PoolScoringService;
use poolnhl_interface::daily_leaders::model::{
    DailyGoaly, DailyLeaders, DailySkater, GoalyStats, SkaterStats,
};
use poolnhl_interface::pool::model::{
    DailyRosterPoints, GoalyPoints, Pool, PoolContext, PoolSettings, PoolState, PoolUser, Roster,
    SkaterPoints,
};

const TEST_DATABASE: &str = "hockeypooltest";
const OWNER: &str = "shadow-u1";
const USER_2: &str = "shadow-u2";

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

fn unique_date() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("shadow-day-{nanos}")
}

// A pool with one cumulated day whose stored points match the seeded leaders:
// owner ices skater 101 (2 goals) + goalie 301 (shutout win); user-2 ices
// skater 201 (1 goal, 1 assist). With default settings the owner leads.
fn seeded_pool(name: &str, date: &str) -> Pool {
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

    let mut day = HashMap::new();
    day.insert(
        OWNER.to_string(),
        DailyRosterPoints {
            roster: Roster {
                F: HashMap::from([(
                    "101".to_string(),
                    Some(SkaterPoints {
                        G: 2,
                        A: 0,
                        SOG: Some(0),
                    }),
                )]),
                D: HashMap::new(),
                G: HashMap::from([(
                    "301".to_string(),
                    Some(GoalyPoints {
                        G: 0,
                        A: 0,
                        W: true,
                        SO: true,
                        OT: false,
                    }),
                )]),
            },
            is_cumulated: true,
        },
    );
    day.insert(
        USER_2.to_string(),
        DailyRosterPoints {
            roster: Roster {
                F: HashMap::from([(
                    "201".to_string(),
                    Some(SkaterPoints {
                        G: 1,
                        A: 1,
                        SOG: Some(0),
                    }),
                )]),
                D: HashMap::new(),
                G: HashMap::new(),
            },
            is_cumulated: true,
        },
    );

    context.score_by_day = Some(HashMap::from([(date.to_string(), day)]));
    pool.context = Some(context);
    pool
}

#[tokio::test]
#[ignore]
async fn derived_ranking_matches_stored_ranking() {
    let db = database().await;
    let (_, redis) = RedisManager::connect(&redis_uri())
        .await
        .expect("redis is not reachable; start it with `docker compose up -d redis`");
    let leaders: Collection<DailyLeaders> = db.collection("day_leaders");
    let date = unique_date();

    leaders
        .insert_one(
            DailyLeaders {
                date: date.clone(),
                skaters: vec![
                    DailySkater {
                        name: "Sniper".into(),
                        id: 101,
                        team: 1,
                        stats: SkaterStats {
                            goals: 2,
                            assists: 0,
                            shootoutGoals: 0,
                        },
                    },
                    DailySkater {
                        name: "Playmaker".into(),
                        id: 201,
                        team: 1,
                        stats: SkaterStats {
                            goals: 1,
                            assists: 1,
                            shootoutGoals: 0,
                        },
                    },
                ],
                goalies: vec![DailyGoaly {
                    name: "Wall".into(),
                    id: 301,
                    team: 2,
                    stats: GoalyStats {
                        goals: 0,
                        assists: 0,
                        decision: Some("W".into()),
                        savePercentage: Some(1.0),
                        OT: None,
                    },
                }],
                played: vec![101, 201, 301],
            },
            None,
        )
        .await
        .unwrap();

    let pool = seeded_pool("shadow-pool", &date);
    let scoring = PoolScoringService::new(DayLeadersCache::new(db.clone(), redis.clone()));

    // The ranking derived from day_leaders must equal the one from stored points.
    let derived_rank = scoring.derive_final_rank(&pool).await.unwrap();
    let stored_rank = pool
        .context
        .as_ref()
        .unwrap()
        .get_final_rank(&pool.settings)
        .unwrap();

    assert_eq!(derived_rank, stored_rank);
    assert_eq!(derived_rank, vec![OWNER.to_string(), USER_2.to_string()]);

    // The goalie shutout bonus survives the full derive path (cache -> roster).
    let derived = scoring.derive_score_by_day(&pool).await.unwrap();
    let owner_goalie = derived[&date][OWNER].roster.G["301"].as_ref().unwrap();
    assert!(owner_goalie.W && owner_goalie.SO);

    // Cleanup mongo + the cache key.
    leaders
        .delete_one(doc! {"date": &date}, None)
        .await
        .unwrap();
    let mut conn = redis.clone();
    let _: () = conn.del(format!("dl:v1:{date}")).await.unwrap();
}
