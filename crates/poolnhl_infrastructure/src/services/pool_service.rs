use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{Duration, Local, NaiveDate, Utc};
use futures::stream::TryStreamExt;
use mongodb::bson::doc;
use mongodb::bson::{Document, to_bson};
use mongodb::options::{
    FindOneAndUpdateOptions, FindOneOptions, FindOptions, IndexOptions, ReturnDocument,
};
use mongodb::{Collection, IndexModel};
use poolnhl_interface::errors::AppError;

use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{
    END_SEASON_DATE, POOL_CREATION_SEASON, Pool, PoolContext, PoolState, ProjectedPoolShort,
    START_SEASON_DATE,
};
use poolnhl_interface::pool::requests::{
    AddPlayerRequest, CompleteProtectionRequest, CreateTradeRequest, DeleteTradeRequest,
    FillSpotRequest, GenerateDynastyRequest, MarkAsFinalRequest, ModifyRosterRequest,
    PoolCreationRequest, PoolDeletionRequest, ProtectPlayersRequest, RemovePlayerRequest,
    RespondTradeRequest, UpdatePoolSettingsRequest,
};
use poolnhl_interface::pool::service::PoolService;

use crate::database_connection::DatabaseConnection;
use crate::database_connection::{bson_err, mongo_err};

#[derive(Clone)]
pub struct MongoPoolService {
    collection: Collection<Pool>,
}

pub async fn get_optional_short_pool_by_name(
    collection: &Collection<Pool>,
    _name: &str,
) -> Result<Option<Pool>> {
    let find_option = FindOneOptions::builder()
        .projection(doc! {"context.score_by_day": 0})
        .build();

    let short_pool = collection
        .find_one(doc! {"name": &_name}, find_option)
        .await
        .map_err(mongo_err)?;

    Ok(short_pool)
}

/// The next value of a pool's `date_updated` version stamp.
///
/// Every mutation reads a pool, changes it in memory and writes the result
/// back, so two concurrent mutations would otherwise silently overwrite each
/// other. `date_updated` doubles as the optimistic-locking version: the update
/// only applies if the document still carries the value that was read.
///
/// It stays a wall-clock millisecond timestamp (it is one on the wire), but is
/// forced to strictly increase so two writes landing in the same millisecond
/// still produce distinct versions.
fn next_version(current_version: i64) -> i64 {
    Utc::now().timestamp_millis().max(current_version + 1)
}

/// Apply `updated_field` to the pool, but only if it has not been modified
/// since it was read at version `expected_version`.
///
/// Returns [`AppError::ConflictError`] when someone else wrote to the pool in
/// the meantime, so the caller can refetch and retry rather than clobbering
/// that update.
pub async fn update_pool(
    mut updated_field: Document,
    collection: &Collection<Pool>,
    pool_name: &str,
    expected_version: i64,
) -> Result<Pool> {
    // Stamp the new version inside the caller's `$set`. Callers that `$set` a
    // whole serialized pool carry the *old* `date_updated`, so this insert has
    // to happen last to win.
    updated_field
        .get_document_mut("$set")
        .map_err(|e| AppError::BsonError {
            msg: format!("a pool update must carry a `$set` document: {e}"),
        })?
        .insert("date_updated", next_version(expected_version));

    // Update the fields in the mongoDB pool document.
    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .projection(doc! {"context.score_by_day": 0})
        .build();

    let updated = collection
        .find_one_and_update(
            doc! {"name": pool_name, "date_updated": expected_version},
            updated_field,
            find_one_and_update_options,
        )
        .await
        .map_err(mongo_err)?;

    match updated {
        Some(pool) => Ok(pool),
        // The filter matched nothing: either the pool is gone, or its version
        // moved on. Tell those two apart so the client gets 404 vs 409.
        None => match get_optional_short_pool_by_name(collection, pool_name).await? {
            Some(_) => Err(AppError::ConflictError {
                msg: "This pool was modified by someone else while you were editing it. \
                      Refresh and try again."
                    .to_string(),
            }),
            None => Err(AppError::NotFound {
                msg: format!("no pool found with name '{pool_name}'"),
            }),
        },
    }
}

pub async fn get_short_pool_by_name(
    collection: &Collection<Pool>,
    pool_name: &str,
) -> Result<Pool> {
    // Return the pool information without the score_by_day member
    get_optional_short_pool_by_name(collection, pool_name)
        .await?
        .ok_or(AppError::NotFound {
            msg: format!("no pool found with name '{}'", pool_name),
        })
}

impl MongoPoolService {
    pub fn new(db: DatabaseConnection) -> Self {
        let collection = db.collection::<Pool>("pools");
        Self { collection }
    }
}

// Today's date, used as the effective date of a lineup change. There is no
// noon-lock / 12PM rule anymore: a change takes effect the day it is made, and
// `roster_modification_date` still governs when changes are permitted.
fn today() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

#[async_trait]
impl PoolService for MongoPoolService {
    async fn init_indexes(&self) -> Result<()> {
        let index_model = IndexModel::builder()
            .keys(doc! { "name": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.collection
            .create_index(index_model, None)
            .await
            .map_err(mongo_err)?;
        Ok(())
    }
    async fn get_pool_by_name(&self, name: &str) -> Result<Pool> {
        let pool = self
            .collection
            .find_one(doc! {"name": name}, None)
            .await
            .map_err(mongo_err)?;

        pool.ok_or(AppError::NotFound {
            msg: format!("no pool found with name '{}'", name),
        })
    }

    async fn get_pool_by_name_with_range(
        &self,
        name: &str,
        start_season_date: &str,
        from_date_str: &str,
    ) -> Result<Pool> {
        let from_date = NaiveDate::parse_from_str(from_date_str, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        let mut start_date = NaiveDate::parse_from_str(start_season_date, "%Y-%m-%d")
            .map_err(|e| AppError::ParseError { msg: e.to_string() })?;

        // Projection will allow to filter all the date that the user did not want
        // (All the date before the from date received will be ignore).
        //
        // The loop compares dates, not their string forms: a caller-supplied
        // `from` that chrono accepts but that does not round-trip to the same
        // string (e.g. "2026-1-5") would otherwise never match the break
        // condition and spin forever.
        let mut projection = doc! {};
        while start_date < from_date {
            projection.insert(
                format!("context.score_by_day.{}", start_date.format("%Y-%m-%d")),
                0,
            );
            start_date += Duration::days(1);
        }

        let find_option = FindOneOptions::builder().projection(projection).build();
        let pool = self
            .collection
            .clone_with_type::<Pool>()
            .find_one(doc! {"name": &name}, find_option)
            .await
            .map_err(mongo_err)?;

        pool.ok_or(AppError::NotFound {
            msg: format!("no pool found with name '{}'", name),
        })
    }

    async fn list_pools(&self, season: u32) -> Result<Vec<ProjectedPoolShort>> {
        let find_option = FindOptions::builder()
            .projection(doc! {"name": 1, "owner": 1, "status": 1, "season": 1})
            .build();

        let filter = doc! { "season": season };

        let cursor = self
            .collection
            .clone_with_type::<ProjectedPoolShort>()
            .find(filter, find_option)
            .await
            .map_err(mongo_err)?;

        let pools = cursor.try_collect().await.map_err(mongo_err)?;

        Ok(pools)
    }

    async fn create_pool(&self, user_id: &str, req: PoolCreationRequest) -> Result<Pool> {
        // Create the default Pool class.
        let pool = Pool::new(&req.pool_name, user_id, &req.settings);

        self.collection
            .insert_one(&pool, None)
            .await
            .map_err(mongo_err)?;

        Ok(pool)
    }

    async fn delete_pool(&self, user_id: &str, req: PoolDeletionRequest) -> Result<Pool> {
        let pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        pool.has_owner_privileges(user_id)?;

        let delete_result = self
            .collection
            .delete_one(doc! {"name": req.pool_name}, None)
            .await
            .map_err(mongo_err)?;

        if delete_result.deleted_count == 0 {
            return Err(AppError::CustomError {
                msg: "The pool could not be deleted.".to_string(),
            });
        }

        Ok(pool)
    }

    async fn create_trade(&self, user_id: &str, req: &mut CreateTradeRequest) -> Result<Pool> {
        // Create a trade and update the database
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        // Create the new trade in the pool
        pool.create_trade(&mut req.trade, user_id)?;

        // Update the field in the pool
        let updated_fields = doc! {
            "$set": doc!{
                "trades": to_bson(&pool.trades).map_err(bson_err)?,
            }
        };

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn delete_trade(&self, user_id: &str, req: DeleteTradeRequest) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        // Delete the trade
        pool.delete_trade(user_id, req.trade_id)?;

        // Update the field in the pool
        let updated_fields = doc! {
            "$set": doc!{
                "trades": to_bson(&pool.trades).map_err(bson_err)?,
            }
        };

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn respond_trade(&self, user_id: &str, req: RespondTradeRequest) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        // repond the trade
        pool.respond_trade(user_id, req.is_accepted, req.trade_id)?;

        // A trade can move players between the two rosters, so record both.
        let effective = today();
        if let Some(context) = pool.context.as_mut() {
            let participants: Vec<String> = context.pooler_roster.keys().cloned().collect();
            for participant in participants {
                context.record_lineup_change(&participant, &effective);
            }
        }

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        // Update the field in the pool
        let updated_fields = doc! {
            "$set": doc!{
                "trades": to_bson(&pool.trades).map_err(bson_err)?,
                "context.pooler_roster": to_bson(&context.pooler_roster ).map_err(bson_err)?,
                "context.tradable_picks": to_bson(&context.tradable_picks ).map_err(bson_err)?,
                "context.lineup_events": to_bson(&context.lineup_events).map_err(bson_err)?
            }
        };

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn fill_spot(&self, user_id: &str, req: FillSpotRequest) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        // Fill the player into the starting roster.
        pool.fill_spot(user_id, &req.filled_spot_user_id, req.player_id)?;

        // Update fields with the filled spot

        if let Some(context) = pool.context.as_mut() {
            context.record_lineup_change(&req.filled_spot_user_id, &today());
        }

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        // Update the field in the pool
        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
                "context.lineup_events": to_bson(&context.lineup_events).map_err(bson_err)?
            }
        };

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn add_player(&self, user_id: &str, req: AddPlayerRequest) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        // Add the player into the reservist of a pooler
        pool.add_player(user_id, &req.added_player_user_id, &req.player)?;

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
                "context.players": to_bson(&context.players).map_err(bson_err)?
            }
        };

        // Update the fields in the mongoDB pool document.

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn remove_player(&self, user_id: &str, req: RemovePlayerRequest) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        // Remove the player from the roster.
        pool.remove_player(user_id, &req.removed_player_user_id, req.player_id)?;

        // updated fields.
        if let Some(context) = pool.context.as_mut() {
            context.record_lineup_change(&req.removed_player_user_id, &today());
        }

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
                "context.lineup_events": to_bson(&context.lineup_events).map_err(bson_err)?,
            }
        };

        // Update the fields in the mongoDB pool document.

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn update_pool_settings(
        &self,
        user_id: &str,
        req: UpdatePoolSettingsRequest,
    ) -> Result<Pool> {
        let pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        pool.can_update_started_pool_settings(user_id, &req.settings)?;

        let updated_fields = doc! {
            "$set": doc!{
                "settings": to_bson(&req.settings).map_err(bson_err)?,

            }
        };

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn modify_roster(&self, user_id: &str, req: ModifyRosterRequest) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        pool.modify_roster(
            user_id,
            &req.roster_modified_user_id,
            &req.forw_list,
            &req.def_list,
            &req.goal_list,
            &req.reserv_list,
        )?;
        // Modify the all the pooler_roster (we could update only the pooler_roster[userId] if necessary)

        if let Some(context) = pool.context.as_mut() {
            context.record_lineup_change(&req.roster_modified_user_id, &today());
        }

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
                "context.lineup_events": to_bson(&context.lineup_events).map_err(bson_err)?,
            }
        };

        // Update the fields in the mongoDB pool document.

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn protect_players(&self, user_id: &str, req: ProtectPlayersRequest) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        pool.protect_players(
            user_id,
            &req.protected_players_user_id,
            &req.protected_players,
        )?;

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
                "context.protected_players": to_bson(&context.protected_players).map_err(bson_err)?,
                "status":  to_bson(&pool.status).map_err(bson_err)?
            }
        };

        // Update the fields in the mongoDB pool document.

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn complete_protection(
        &self,
        user_id: &str,
        req: CompleteProtectionRequest,
    ) -> Result<Pool> {
        let mut pool = get_short_pool_by_name(&self.collection, &req.pool_name).await?;

        pool.complete_protection(user_id)?;

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
                "context.players": to_bson(&context.players).map_err(bson_err)?,
                "status":  to_bson(&pool.status).map_err(bson_err)?
            }
        };

        // Update the fields in the mongoDB pool document.

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn mark_as_final(&self, user_id: &str, req: MarkAsFinalRequest) -> Result<Pool> {
        let mut pool = self.get_pool_by_name(&req.pool_name).await?;

        pool.mark_as_final(user_id)?;

        let updated_fields = doc! {
            "$set": doc!{
                "draft_order": to_bson(&pool.draft_order).map_err(bson_err)?,
                "final_rank": to_bson(&pool.final_rank).map_err(bson_err)?,
                "status":  to_bson(&pool.status).map_err(bson_err)?
            }
        };

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn generate_dynasty(&self, user_id: &str, req: GenerateDynastyRequest) -> Result<Pool> {
        let pool = self.get_pool_by_name(&req.pool_name).await?;

        pool.has_privileges(user_id)?;
        pool.validate_pool_status(&PoolState::Final)?;

        let mut new_settings = pool.settings.clone();
        let new_dynasty_settings =
            new_settings
                .dynasty_settings
                .as_mut()
                .ok_or_else(|| AppError::CustomError {
                    msg: "The pool does not have dynasty settings.".to_string(),
                })?;

        // Insert the past pool at the first element of the list.
        new_dynasty_settings
            .past_season_pool_name
            .insert(0, pool.name.clone());
        new_dynasty_settings.next_season_pool_name = None;

        let mut protected_players = HashMap::new();

        for pool_user in &pool.participants {
            protected_players.insert(pool_user.id.clone(), Vec::new());
        }

        // If the pool is dynasty type, we need to create a new pool in dynasty status.
        // With almost everying thing from the last pool save into it.
        let pool_context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "The pool does not have a pool context.".to_string(),
        })?;
        let new_dynasty_pool = Pool {
            name: req.new_pool_name,
            owner: pool.owner,
            participants: pool.participants,
            settings: new_settings,
            status: PoolState::Dynasty,
            final_rank: None,
            draft_order: pool
                .final_rank
                .as_ref()
                .map(|rank| rank.iter().cloned().rev().collect::<Vec<_>>()), // The default draft order is reverse the final ranking.
            trades: None,
            context: Some(PoolContext {
                pooler_roster: pool_context.pooler_roster.clone(),
                players_name_drafted: Vec::new(),
                score_by_day: Some(HashMap::new()),
                tradable_picks: Some(Vec::new()),
                past_tradable_picks: pool_context.tradable_picks.clone(),
                protected_players: Some(protected_players),
                players: pool_context.players.clone(),
                lineup_events: Some(Vec::new()),
            }),
            date_updated: 0,
            season_start: START_SEASON_DATE.to_string(),
            season_end: END_SEASON_DATE.to_string(),
            season: POOL_CREATION_SEASON,
        };

        self.collection
            .insert_one(&new_dynasty_pool, None)
            .await
            .map_err(mongo_err)?;

        let updated_fields = doc! {
            "$set": doc!{
                "settings": to_bson(&pool.settings).map_err(bson_err)?,
            }
        };

        update_pool(
            updated_fields,
            &self.collection,
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }
}
