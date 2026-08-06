//! End-to-end test of the derive path: the pool's embedded lineup events +
//! shared day_leaders -> per-participant scoring, matching what `score_by_day`
//! stored.
//!
//! Needs mongo + redis:
//!   docker compose up -d mongo redis
//!   cargo test -p poolnhl_infrastructure -- --ignored
//!
//! Seeds day_leaders in the dedicated `hockeypooltest` database, clearing them
//! both before and after so the run is repeatable even if a previous one died
//! partway. The pool itself is built in memory.

use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::bson::doc;
use mongodb::Collection;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

use poolnhl_infrastructure::database_connection::{DatabaseConnection, DatabaseManager};
use poolnhl_infrastructure::redis_connection::RedisManager;
use poolnhl_infrastructure::services::day_leaders_cache::DayLeadersCache;
use poolnhl_infrastructure::services::pool_scoring_service::PoolScoringService;
use poolnhl_interface::daily_leaders::model::{
    DailyGoaly, DailyLeaders, DailySkater, GoalyStats, SkaterStats,
};
use poolnhl_interface::pool::lineup::LineupEvent;
use poolnhl_interface::pool::model::{
    Pool, PoolContext, PoolSettings, PoolState, PoolUser, SkaterPoints,
};

const TEST_DATABASE: &str = "hockeypooltest";
const OWNER: &str = "shadow-u1";
const USER_2: &str = "shadow-u2";
// Real, far-future dates so derive_range's calendar iteration parses them while
// staying clear of any real seeded data.
const DAY1: &str = "2099-12-30";
const DAY2: &str = "2099-12-31";

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

// Drop the seeded days from both stores. Run before seeding as well as after,
// so a run that died before its cleanup does not leave a duplicate `date`
// (rejected by the unique index) or a stale cache entry for the next run.
async fn clear_seeded_days(leaders: &Collection<DailyLeaders>, redis: &ConnectionManager) {
    for date in [DAY1, DAY2] {
        leaders
            .delete_many(doc! {"date": date}, None)
            .await
            .unwrap();
        let mut conn = redis.clone();
        let _: () = conn.del(format!("dl:v1:{date}")).await.unwrap();
    }
}

fn skater(id: u32, goals: u8, assists: u8) -> DailySkater {
    DailySkater {
        name: format!("s{id}"),
        id,
        team: 1,
        stats: SkaterStats {
            goals,
            assists,
            shootoutGoals: 0,
        },
    }
}

// A pool whose owner ices skater 101 + goalie 301, user-2 ices skater 201, with
// the lineups recorded as embedded events effective on DAY1 (never changed).
fn pool_with_lineups() -> Pool {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut pool = Pool::new(&format!("shadow-pool-{nanos}"), OWNER, &PoolSettings::new());
    pool.status = PoolState::InProgress;

    let ids: Vec<String> = [OWNER, USER_2].iter().map(|id| id.to_string()).collect();
    pool.participants = ids
        .iter()
        .map(|id| PoolUser {
            id: id.clone(),
            name: id.clone(),
            is_owned: true,
        })
        .collect();

    let mut context = PoolContext::new(&ids);
    context.lineup_events = Some(vec![
        LineupEvent {
            participant: OWNER.to_string(),
            effective_date: DAY1.to_string(),
            forwards: vec![101],
            defense: vec![],
            goalies: vec![301],
        },
        LineupEvent {
            participant: USER_2.to_string(),
            effective_date: DAY1.to_string(),
            forwards: vec![201],
            defense: vec![],
            goalies: vec![],
        },
    ]);
    pool.context = Some(context);
    pool
}

#[tokio::test]
#[ignore]
async fn derive_daily_and_range_from_lineup_events() {
    let db = database().await;
    let (_, redis) = RedisManager::connect(&redis_uri())
        .await
        .expect("redis is not reachable; start it with `docker compose up -d redis`");
    let leaders: Collection<DailyLeaders> = db.collection("day_leaders");
    clear_seeded_days(&leaders, &redis).await;

    // Shared day stats: skater 101 scores more on DAY2 than DAY1; goalie 301 is
    // a shutout win both days.
    for (date, g101) in [(DAY1, 1u8), (DAY2, 2u8)] {
        leaders
            .insert_one(
                DailyLeaders {
                    date: date.to_string(),
                    skaters: vec![skater(101, g101, 0), skater(201, 1, 1)],
                    goalies: vec![DailyGoaly {
                        name: "wall".into(),
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
    }

    let scoring = PoolScoringService::new(DayLeadersCache::new(db.clone(), redis.clone()));
    let pool = pool_with_lineups();

    // Single day: lineups reconstruct from the embedded events, points from day_leaders.
    let day1 = scoring.derive_daily(&pool, DAY1).await.unwrap();
    assert_eq!(
        day1[OWNER].roster.F["101"],
        Some(SkaterPoints {
            G: 1,
            A: 0,
            SOG: Some(0)
        })
    );
    let owner_goalie = day1[OWNER].roster.G["301"].as_ref().unwrap();
    assert!(owner_goalie.W && owner_goalie.SO && !owner_goalie.OT);
    assert_eq!(
        day1[USER_2].roster.F["201"],
        Some(SkaterPoints {
            G: 1,
            A: 1,
            SOG: Some(0)
        })
    );

    // Range: both days present; the DAY1 lineup carries to DAY2 (no event on
    // DAY2) while its points track that day's stats (101 scores 2 on DAY2).
    let range = scoring.derive_range(&pool, DAY1, DAY2).await.unwrap();
    assert_eq!(range.len(), 2);
    assert_eq!(range[DAY1][OWNER].roster.F["101"].as_ref().unwrap().G, 1);
    assert_eq!(range[DAY2][OWNER].roster.F["101"].as_ref().unwrap().G, 2);

    // Cleanup mongo + cache keys.
    clear_seeded_days(&leaders, &redis).await;
}
