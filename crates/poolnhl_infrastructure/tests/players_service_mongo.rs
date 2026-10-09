//! Integration tests for the mongo-backed players service.
//!
//! They need a running mongo:
//!   docker compose up -d mongo
//!   cargo test -p poolnhl_infrastructure -- --ignored
//!
//! The point of this file is the past-season read: the pipeline that swaps a
//! player's stats for the ones recorded for another season is an aggregation,
//! so nothing short of a real mongo exercises it.

use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::Collection;
use mongodb::bson::{Document, doc};

use poolnhl_infrastructure::database_connection::{DatabaseConnection, DatabaseManager};
use poolnhl_infrastructure::services::players_service::MongoPlayersService;
use poolnhl_interface::errors::AppError;
use poolnhl_interface::players::model::{GetPlayerQuery, PlayerInfo, Position};
use poolnhl_interface::players::service::PlayersService;
use poolnhl_interface::pool::model::{CURRENT_SEASON, SEASONS};

const TEST_DATABASE: &str = "hockeypooltest";

fn previous_season() -> u32 {
    SEASONS[SEASONS.len() - 2].season
}

fn mongo_uri() -> String {
    std::env::var("TEST_MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string())
}

async fn database() -> DatabaseConnection {
    DatabaseManager::new_pool(&mongo_uri(), TEST_DATABASE)
        .await
        .expect("mongo is not reachable; start it with `docker compose up -d mongo`")
}

// Ids are unique per run so the tests can run in parallel and leave nothing
// behind that a later run would read as its own fixture.
fn unique_id() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Well above any real NHL player id, and small enough to stay an i32.
    900_000_000 + (nanos % 50_000_000) as u32
}

fn player(id: u32, name: &str, points: u32, salary_cap: f64) -> PlayerInfo {
    PlayerInfo {
        active: true,
        id,
        name: name.to_string(),
        team: Some(1),
        position: Position::F,
        age: Some(27),
        salary_cap: Some(salary_cap),
        contract_expiration_season: Some(CURRENT_SEASON.season),
        game_played: Some(3),
        goals: Some(points),
        assists: Some(0),
        points: Some(points),
        points_per_game: Some(points as f32 / 3.0),
        goal_against_average: None,
        save_percentage: None,
        saves: None,
        shots: None,
        wins: None,
        ot: None,
    }
}

async fn cleanup(players: &Collection<PlayerInfo>, stats: &Collection<Document>, ids: &[u32]) {
    let ids: Vec<i64> = ids.iter().map(|id| *id as i64).collect();
    players
        .delete_many(doc! { "id": { "$in": &ids } }, None)
        .await
        .unwrap();
    stats
        .delete_many(doc! { "id": { "$in": &ids } }, None)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn a_past_season_read_keeps_the_identity_and_swaps_the_stats() {
    let db = database().await;
    let service = MongoPlayersService::new(db.clone());
    let players: Collection<PlayerInfo> = db.collection("players");
    let season_stats: Collection<Document> = db.collection("player_season_stats");

    // Four games into the new season, traded over the summer and on a new deal.
    let id = unique_id();
    let name = format!("swapped-{id}");
    players
        .insert_one(player(id, &name, 2, 9.5), None)
        .await
        .unwrap();
    season_stats
        .insert_one(
            doc! {
                "id": id as i64,
                "season": previous_season() as i64,
                "game_played": 82_i64,
                "goals": 40_i64,
                "assists": 60_i64,
                "points": 100_i64,
                "points_per_game": 1.2195_f64,
            },
            None,
        )
        .await
        .unwrap();

    let found = service
        .get_players_with_name(&name, Some(previous_season()))
        .await
        .unwrap();

    let found = found.first().expect("the player is in the collection");
    // Last season's numbers...
    assert_eq!(found.points, Some(100));
    assert_eq!(found.goals, Some(40));
    assert_eq!(found.game_played, Some(82));
    // ...on today's contract and team, which is the whole point of the split.
    assert_eq!(found.salary_cap, Some(9.5));
    assert_eq!(found.team, Some(1));
    assert_eq!(found.age, Some(27));

    cleanup(&players, &season_stats, &[id]).await;
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn a_player_with_no_row_for_the_season_reads_as_having_not_played() {
    let db = database().await;
    let service = MongoPlayersService::new(db.clone());
    let players: Collection<PlayerInfo> = db.collection("players");
    let season_stats: Collection<Document> = db.collection("player_season_stats");

    // A rookie: current-season totals, nothing on record for last season.
    let id = unique_id() + 1;
    let name = format!("rookie-{id}");
    players
        .insert_one(player(id, &name, 5, 0.95), None)
        .await
        .unwrap();

    let found = service
        .get_players_with_name(&name, Some(previous_season()))
        .await
        .unwrap();

    let found = found.first().expect("the player is in the collection");
    // Not this season's 5 points under last season's heading.
    assert_eq!(found.points, None);
    assert_eq!(found.goals, None);
    assert_eq!(found.game_played, None);
    assert_eq!(found.name, name);
    assert_eq!(found.salary_cap, Some(0.95));

    cleanup(&players, &season_stats, &[id]).await;
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn a_past_season_read_sorts_on_that_season() {
    let db = database().await;
    let service = MongoPlayersService::new(db.clone());
    let players: Collection<PlayerInfo> = db.collection("players");
    let season_stats: Collection<Document> = db.collection("player_season_stats");

    // Hot start, quiet last season -- and the reverse. Sorting on points for
    // the past season has to put them the other way round from today's table.
    let (fast, slow) = (unique_id() + 2, unique_id() + 3);
    let tag = format!("sorted-{fast}");
    players
        .insert_many(
            vec![
                player(fast, &format!("{tag}-fast"), 9, 1.0),
                player(slow, &format!("{tag}-slow"), 1, 1.0),
            ],
            None,
        )
        .await
        .unwrap();
    season_stats
        .insert_many(
            vec![
                doc! { "id": fast as i64, "season": previous_season() as i64, "points": 10_i64 },
                doc! { "id": slow as i64, "season": previous_season() as i64, "points": 90_i64 },
            ],
            None,
        )
        .await
        .unwrap();

    let by_current = service.get_players_with_name(&tag, None).await.unwrap();
    let mut by_current: Vec<_> = by_current.iter().map(|p| (p.id, p.points)).collect();
    by_current.sort_by_key(|(_, points)| std::cmp::Reverse(*points));
    assert_eq!(by_current.first().unwrap().0, fast);

    let found = service
        .get_players(GetPlayerQuery {
            active: Some(true),
            positions: None,
            sort: Some("points".to_string()),
            descending: Some(true),
            skip: None,
            limit: Some(100),
            season: Some(previous_season()),
        })
        .await
        .unwrap();

    let ranked: Vec<u32> = found
        .iter()
        .filter(|p| p.name.starts_with(&tag))
        .map(|p| p.id)
        .collect();
    assert_eq!(ranked, vec![slow, fast]);

    cleanup(&players, &season_stats, &[fast, slow]).await;
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn the_unique_index_keeps_one_row_per_player_per_season() {
    let db = database().await;
    let service = MongoPlayersService::new(db.clone());
    let season_stats: Collection<Document> = db.collection("player_season_stats");
    service.init_indexes().await.unwrap();

    let id = unique_id() + 4;
    let row = doc! { "id": id as i64, "season": previous_season() as i64, "points": 1_i64 };
    season_stats.insert_one(&row, None).await.unwrap();

    let duplicate = season_stats.insert_one(&row, None).await;
    assert!(
        duplicate.is_err(),
        "a second row for the same season got in"
    );

    season_stats
        .delete_many(doc! { "id": id as i64 }, None)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a running mongo (docker compose up -d mongo)"]
async fn a_season_the_pool_has_no_data_for_is_refused() {
    let db = database().await;
    let service = MongoPlayersService::new(db.clone());

    let refused = service
        .get_players(GetPlayerQuery {
            active: None,
            positions: None,
            sort: None,
            descending: None,
            skip: None,
            limit: None,
            season: Some(19992000),
        })
        .await;

    assert!(matches!(refused, Err(AppError::CustomError { .. })));
}
