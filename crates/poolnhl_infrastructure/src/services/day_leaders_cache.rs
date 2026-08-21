//! Read-through Redis cache of the compact, scoring-only projection of the
//! shared `day_leaders` collection.
//!
//! Pool scores are derived on demand from these day scores rather than being
//! duplicated per pool. Because the same day is shared by every pool, caching
//! it once here serves all of them. Finalized days never change, so they are
//! cached with a long TTL; the current ("live") day is still being written by
//! the ingest poller, so it gets a short TTL and is refreshed from Mongo.

use chrono::{Duration, Local, Timelike};
use mongodb::Collection;
use mongodb::bson::doc;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use poolnhl_interface::daily_leaders::model::DailyLeaders;
use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::pool::scoring::DayScores;

use crate::database_connection::DatabaseConnection;
use crate::database_connection::mongo_err;
use crate::redis_connection::redis_err;

// Cache-key schema version. Bump to invalidate every cached day at once (e.g.
// if the compact format or a scoring-derivation rule changes).
// v2: `DayScores` carries the `played` set, so scoreless games are counted.
const CACHE_PREFIX: &str = "dl:v2:";

// Finalized days are immutable; two weeks is plenty for the working set while
// still letting cold entries expire. The live day is rewritten every game
// night, so keep it short and let Mongo be the source of truth.
const FINAL_TTL_SECONDS: u64 = 60 * 60 * 24 * 14;
const LIVE_TTL_SECONDS: u64 = 45;

#[derive(Clone)]
pub struct DayLeadersCache {
    collection: Collection<DailyLeaders>,
    redis: ConnectionManager,
}

impl DayLeadersCache {
    pub fn new(db: DatabaseConnection, redis: ConnectionManager) -> Self {
        Self {
            collection: db.collection::<DailyLeaders>("day_leaders"),
            redis,
        }
    }

    /// The currently-live day. Matches `MongoDailyLeadersService`: before noon
    /// the games that finished after midnight still count as the previous day.
    fn live_date() -> String {
        let mut today = Local::now().date_naive();
        if Local::now().time().hour() < 12 {
            today -= Duration::days(1);
        }
        today.format("%Y-%m-%d").to_string()
    }

    /// Compact scoring lines for `date`, served read-through: Redis first, then
    /// Mongo on a miss (caching the result). A date with no games yields — and
    /// caches — empty [`DayScores`], so callers get a uniform answer.
    pub async fn day_scores(&self, date: &str) -> Result<DayScores> {
        let key = format!("{CACHE_PREFIX}{date}");
        let mut conn = self.redis.clone();

        let cached: Option<String> = conn.get(&key).await.map_err(redis_err)?;
        if let Some(json) = cached {
            // A corrupt/old-format value is ignored and rebuilt below.
            if let Ok(scores) = serde_json::from_str::<DayScores>(&json) {
                return Ok(scores);
            }
        }

        let scores = self
            .collection
            .find_one(doc! { "date": date }, None)
            .await
            .map_err(mongo_err)?
            .map(|daily_leaders| DayScores::from_daily_leaders(&daily_leaders))
            .unwrap_or_default();

        let ttl = if date >= Self::live_date().as_str() {
            LIVE_TTL_SECONDS
        } else {
            FINAL_TTL_SECONDS
        };
        let json = serde_json::to_string(&scores)
            .map_err(|e| AppError::RedisError { msg: e.to_string() })?;
        let _: () = conn.set_ex(&key, json, ttl).await.map_err(redis_err)?;

        Ok(scores)
    }
}
