//! Mongo-backed store for sparse lineup events (`pool_lineups` collection).
//!
//! Reads a pool's events grouped by participant and sorted for
//! [`lineup_as_of`](poolnhl_interface::pool::lineup::lineup_as_of), and provides
//! an idempotent replace used to migrate events out of the legacy
//! `score_by_day`.

use std::collections::HashMap;

use futures::stream::TryStreamExt;
use mongodb::bson::doc;
use mongodb::Collection;

use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::pool::lineup::LineupEvent;

use crate::database_connection::DatabaseConnection;

#[derive(Clone)]
pub struct LineupStore {
    collection: Collection<LineupEvent>,
}

impl LineupStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            collection: db.collection::<LineupEvent>("pool_lineups"),
        }
    }

    /// A pool's lineup events grouped by participant, each list sorted by
    /// `effective_date` ascending so it can be fed straight to `lineup_as_of`.
    pub async fn events_for_pool(
        &self,
        pool_name: &str,
    ) -> Result<HashMap<String, Vec<LineupEvent>>> {
        let cursor = self
            .collection
            .find(doc! { "pool_name": pool_name }, None)
            .await
            .map_err(mongo_err)?;
        let events: Vec<LineupEvent> = cursor.try_collect().await.map_err(mongo_err)?;

        let mut by_participant: HashMap<String, Vec<LineupEvent>> = HashMap::new();
        for event in events {
            by_participant
                .entry(event.participant.clone())
                .or_default()
                .push(event);
        }
        for participant_events in by_participant.values_mut() {
            participant_events.sort_by(|a, b| a.effective_date.cmp(&b.effective_date));
        }
        Ok(by_participant)
    }

    /// Replace all of a pool's lineup events with `events` (idempotent, so the
    /// migration can be re-run safely).
    pub async fn replace_pool_events(&self, pool_name: &str, events: &[LineupEvent]) -> Result<()> {
        self.collection
            .delete_many(doc! { "pool_name": pool_name }, None)
            .await
            .map_err(mongo_err)?;
        if !events.is_empty() {
            self.collection
                .insert_many(events, None)
                .await
                .map_err(mongo_err)?;
        }
        Ok(())
    }
}

fn mongo_err(e: mongodb::error::Error) -> AppError {
    AppError::MongoError { msg: e.to_string() }
}
