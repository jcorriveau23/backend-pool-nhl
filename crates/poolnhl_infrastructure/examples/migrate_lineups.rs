//! One-off migration: extract sparse lineup events from each pool's stored
//! `score_by_day` into the `pool_lineups` collection. Idempotent.
//!
//!   cargo run -p poolnhl_infrastructure --example migrate_lineups
//!
//! Env: MONGO_URI (default mongodb://localhost:27017),
//!      MONGO_DB  (default hockeypool).

use futures::stream::TryStreamExt;
use mongodb::bson::doc;

use poolnhl_infrastructure::database_connection::DatabaseManager;
use poolnhl_infrastructure::services::lineup_store::LineupStore;
use poolnhl_interface::pool::lineup::extract_lineup_events;
use poolnhl_interface::pool::model::Pool;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let uri = std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".into());
    let db_name = std::env::var("MONGO_DB").unwrap_or_else(|_| "hockeypool".into());

    let db = DatabaseManager::new_pool(&uri, &db_name)
        .await
        .expect("could not connect to mongo");
    let pools = db.collection::<Pool>("pools");
    let store = LineupStore::new(db.clone());

    let mut cursor = pools
        .find(doc! {}, None)
        .await
        .expect("could not list pools");
    let (mut total_pools, mut total_events) = (0usize, 0usize);

    while let Some(pool) = cursor.try_next().await.expect("cursor error") {
        let days = pool
            .context
            .as_ref()
            .and_then(|context| context.score_by_day.as_ref());
        let events = match days {
            Some(score_by_day) => extract_lineup_events(&pool.name, score_by_day),
            None => Vec::new(),
        };
        store
            .replace_pool_events(&pool.name, &events)
            .await
            .expect("could not write lineup events");

        total_pools += 1;
        total_events += events.len();
        println!(
            "{}: {} events from {} days",
            pool.name,
            events.len(),
            days.map(|d| d.len()).unwrap_or(0),
        );
    }

    println!("migrated {total_pools} pools, {total_events} lineup events total");
}
