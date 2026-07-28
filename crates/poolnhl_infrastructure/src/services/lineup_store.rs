//! Mongo-backed store for sparse lineup events (`pool_lineups` collection).
//!
//! Reads a pool's events grouped by participant and sorted for
//! [`lineup_as_of`](poolnhl_interface::pool::lineup::lineup_as_of), and provides
//! an idempotent replace used to migrate events out of the legacy
//! `score_by_day`.

use std::collections::HashMap;

use futures::stream::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::FindOneOptions;
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

    /// Record a lineup change: write an event effective `effective_date` only if
    /// the lineup differs from the participant's most recent event, keeping
    /// `pool_lineups` sparse. Re-emitting the same day replaces that day's event.
    /// Returns whether an event was written.
    pub async fn record_if_changed(
        &self,
        pool_name: &str,
        participant: &str,
        effective_date: &str,
        mut forwards: Vec<u32>,
        mut defense: Vec<u32>,
        mut goalies: Vec<u32>,
    ) -> Result<bool> {
        forwards.sort_unstable();
        defense.sort_unstable();
        goalies.sort_unstable();

        let latest = self
            .collection
            .find_one(
                doc! { "pool_name": pool_name, "participant": participant },
                FindOneOptions::builder()
                    .sort(doc! { "effective_date": -1 })
                    .build(),
            )
            .await
            .map_err(mongo_err)?;

        if let Some(latest) = latest {
            let sorted = |mut ids: Vec<u32>| {
                ids.sort_unstable();
                ids
            };
            if sorted(latest.forwards) == forwards
                && sorted(latest.defense) == defense
                && sorted(latest.goalies) == goalies
            {
                return Ok(false);
            }
        }

        let event = LineupEvent {
            pool_name: pool_name.to_string(),
            participant: participant.to_string(),
            effective_date: effective_date.to_string(),
            forwards,
            defense,
            goalies,
        };
        // Replace any existing event for this exact day so a same-day re-edit is
        // not duplicated.
        self.collection
            .delete_many(
                doc! { "pool_name": pool_name, "participant": participant, "effective_date": effective_date },
                None,
            )
            .await
            .map_err(mongo_err)?;
        self.collection
            .insert_one(&event, None)
            .await
            .map_err(mongo_err)?;
        Ok(true)
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
