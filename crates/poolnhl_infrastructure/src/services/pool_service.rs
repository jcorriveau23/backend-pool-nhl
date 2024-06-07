use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use futures::stream::TryStreamExt;
use mongodb::bson::doc;
use mongodb::bson::{to_bson, Document};
use mongodb::options::{FindOneAndUpdateOptions, FindOneOptions, FindOptions, ReturnDocument};
use mongodb::Collection;
use poolnhl_interface::errors::AppError;

use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{
    GenerateDynastieRequest, PoolContext, PoolState, END_SEASON_DATE, POOL_CREATION_SEASON,
};
use poolnhl_interface::pool::{
    model::{
        AddPlayerRequest, CreateTradeRequest, DeleteTradeRequest, FillSpotRequest,
        MarkAsFinalRequest, ModifyRosterRequest, Pool, PoolCreationRequest, PoolDeletionRequest,
        ProjectedPoolShort, ProtectPlayersRequest, RemovePlayerRequest, RespondTradeRequest,
        UpdatePoolSettingsRequest, START_SEASON_DATE,
    },
    service::PoolService,
};

use crate::database_connection::DatabaseConnection;

#[derive(Clone)]
pub struct MongoPoolService {
    db: DatabaseConnection,
}

impl MongoPoolService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn get_optional_short_pool_by_name(
        &self,
        collection: &Collection<Pool>,
        _name: &str,
    ) -> Result<Option<Pool>> {
        let find_option = FindOneOptions::builder()
            .projection(doc! {"context.score_by_day": 0})
            .build();

        let short_pool = collection
            .find_one(doc! {"name": &_name}, find_option)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        Ok(short_pool)
    }

    // Return the pool information without the score_by_day member
    async fn get_short_pool_by_name(
        &self,
        collection: &Collection<Pool>,
        _name: &str,
    ) -> Result<Pool> {
        let short_pool = self
            .get_optional_short_pool_by_name(collection, _name)
            .await?;

        short_pool.ok_or(AppError::CustomError {
            msg: format!("no pool found with name '{}'", _name),
        })
    }

    async fn update_pool(
        &self,
        updated_field: Document,
        collection: &Collection<Pool>,
        pool_name: &str,
    ) -> Result<Pool> {
        // Update the fields in the mongoDB pool document.
        let find_one_and_update_options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .projection(doc! {"context.score_by_day": 0})
            .build();

        match collection
            .find_one_and_update(
                doc! {"name": pool_name},
                updated_field,
                find_one_and_update_options,
            )
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?
        {
            Some(updated_pool) => Ok(updated_pool),
            None => Err(AppError::CustomError {
                msg: "The pool could not be updated.".to_string(),
            }),
        }
    }
}

#[async_trait]
impl PoolService for MongoPoolService {
    async fn get_pool_by_name(&self, name: &str) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");

        let pool = collection
            .find_one(doc! {"name": name}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        pool.ok_or(AppError::CustomError {
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
        let mut projection = doc! {};
        if from_date >= start_date {
            loop {
                let str_date = start_date.to_string();

                if str_date == *from_date_str {
                    break;
                }
                projection.insert(format!("context.score_by_day.{}", str_date), 0);
                start_date += Duration::days(1);
            }
        }

        let find_option = FindOneOptions::builder().projection(projection).build();
        let collection = self.db.collection::<Pool>("pools");
        let pool = collection
            .clone_with_type::<Pool>()
            .find_one(doc! {"name": &name}, find_option)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        pool.ok_or(AppError::CustomError {
            msg: format!("no pool found with name '{}'", name),
        })
    }

    async fn list_pools(&self, season: u32) -> Result<Vec<ProjectedPoolShort>> {
        let collection = self.db.collection::<Pool>("pools");
        let find_option = FindOptions::builder()
            .projection(doc! {"name": 1, "owner": 1, "status": 1, "season": 1})
            .build();

        let filter = doc! { "season": season };

        let cursor = collection
            .clone_with_type::<ProjectedPoolShort>()
            .find(filter, find_option)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        let pools = cursor
            .try_collect()
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        Ok(pools)
    }

    async fn create_pool(&self, user_id: &str, req: PoolCreationRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");

        // Create the default Pool class.
        let pool = Pool::new(&req.pool_name, user_id, &req.settings);

        collection
            .insert_one(&pool, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        Ok(pool)
    }

    async fn delete_pool(&self, user_id: &str, req: PoolDeletionRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        pool.has_owner_privileges(user_id)?;

        let delete_result = collection
            .delete_one(doc! {"name": req.pool_name}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        if delete_result.deleted_count == 0 {
            return Err(AppError::CustomError {
                msg: "The pool could not be deleted.".to_string(),
            });
        }

        Ok(pool)
    }

    async fn create_trade(&self, user_id: &str, req: &mut CreateTradeRequest) -> Result<Pool> {
        // Create a trade and update the database
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // Create the new trade in the pool
        pool.create_trade(&mut req.trade, user_id)?;

        // Update the field in the pool
        let updated_fields = doc! {
            "$set": doc!{
                "trades": to_bson(&pool.trades).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
            }
        };

        self.update_pool(updated_fields, &collection, &req.pool_name)
            .await
    }

    async fn delete_trade(&self, user_id: &str, req: DeleteTradeRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // Delete the trade
        pool.delete_trade(user_id, req.trade_id)?;

        // Update the field in the pool
        let updated_fields = doc! {
            "$set": doc!{
                "trades": to_bson(&pool.trades).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
            }
        };

        self.update_pool(updated_fields, &collection, &req.pool_name)
            .await
    }

    async fn respond_trade(&self, user_id: &str, req: RespondTradeRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // repond the trade
        pool.respond_trade(user_id, req.is_accepted, req.trade_id)?;

        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "The pool does not have pool context.".to_string(),
            }),
            Some(context) => {
                // Update the field in the pool
                let updated_fields = doc! {
                    "$set": doc!{
                        "trades": to_bson(&pool.trades).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                        "context.pooler_roster": to_bson(&context.pooler_roster ).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                        "context.tradable_picks": to_bson(&context.tradable_picks ).map_err(|e| AppError::MongoError { msg: e.to_string() })?
                    }
                };

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }
    async fn fill_spot(&self, user_id: &str, req: FillSpotRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // Fill the player into the starting roster.
        pool.fill_spot(user_id, &req.filled_spot_user_id, req.player_id)?;

        // Update fields with the filled spot

        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "The pool does not have pool context.".to_string(),
            }),
            Some(context) => {
                // Update the field in the pool
                let updated_fields = doc! {
                    "$set": doc!{
                        "context.pooler_roster": to_bson(&context.pooler_roster).map_err(|e| AppError::MongoError { msg: e.to_string() })?
                    }
                };

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }
    async fn add_player(&self, user_id: &str, req: AddPlayerRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // Add the player into the reservist of a pooler
        pool.add_player(user_id, &req.added_player_user_id, &req.player)?;

        // updated fields.
        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "The pool does not have pool context.".to_string(),
            }),
            Some(context) => {
                let updated_fields = doc! {
                    "$set": doc!{
                        "context.pooler_roster": to_bson(&context.pooler_roster).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                        "context.players": to_bson(&context.players).map_err(|e| AppError::MongoError { msg: e.to_string() })?
                    }
                };

                // Update the fields in the mongoDB pool document.

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }

    async fn remove_player(&self, user_id: &str, req: RemovePlayerRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // Remove the player from the roster.
        pool.remove_player(user_id, &req.removed_player_user_id, req.player_id)?;

        // updated fields.

        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "The pool does not have pool context.".to_string(),
            }),
            Some(context) => {
                let updated_fields = doc! {
                    "$set": doc!{
                        "context.pooler_roster": to_bson(&context.pooler_roster).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                    }
                };

                // Update the fields in the mongoDB pool document.

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }

    async fn update_pool_settings(
        &self,
        user_id: &str,
        req: UpdatePoolSettingsRequest,
    ) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");

        let pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        pool.can_update_in_progress_pool_settings(user_id, &req.pool_settings)?;

        let updated_fields = doc! {
            "$set": doc!{
                "settings": to_bson(&req.pool_settings).map_err(|e| AppError::MongoError { msg: e.to_string() })?,

            }
        };

        self.update_pool(updated_fields, &collection, &req.pool_name)
            .await
    }

    async fn modify_roster(&self, user_id: &str, req: ModifyRosterRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        pool.modify_roster(
            user_id,
            &req.roster_modified_user_id,
            &req.forw_list,
            &req.def_list,
            &req.goal_list,
            &req.reserv_list,
        )?;
        // Modify the all the pooler_roster (we could update only the pooler_roster[userId] if necessary)

        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "The pool does not have pool context.".to_string(),
            }),
            Some(context) => {
                let updated_fields = doc! {
                    "$set": doc!{
                        "context.pooler_roster": to_bson(&context.pooler_roster).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                    }
                };

                // Update the fields in the mongoDB pool document.

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }

    async fn protect_players(&self, user_id: &str, req: ProtectPlayersRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        pool.protect_players(
            user_id,
            &req.forw_protected,
            &req.def_protected,
            &req.goal_protected,
            &req.reserv_protected,
        )?;

        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "The pool does not have pool context.".to_string(),
            }),
            Some(context) => {
                let updated_fields = doc! {
                    "$set": doc!{
                        "context.pooler_roster": to_bson(&context.pooler_roster).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                        "status":  to_bson(&pool.status).map_err(|e| AppError::MongoError { msg: e.to_string() })?
                    }
                };

                // Update the fields in the mongoDB pool document.

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }

    async fn mark_as_final(&self, user_id: &str, req: MarkAsFinalRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self.get_pool_by_name(&req.pool_name).await?;

        pool.mark_as_final(user_id)?;

        let updated_fields = doc! {
            "$set": doc!{
                "final_rank": to_bson(&pool.final_rank).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                "status":  to_bson(&pool.status).map_err(|e| AppError::MongoError { msg: e.to_string() })?
            }
        };

        self.update_pool(updated_fields, &collection, &req.pool_name)
            .await
    }

    async fn generate_dynasty(&self, user_id: &str, req: GenerateDynastieRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");
        let mut pool = self.get_pool_by_name(&req.pool_name).await?;

        pool.generate_dynasty(&user_id, &req.new_pool_name)?;

        let mut new_settings = pool.settings.clone();
        let new_dynasty_settings = new_settings
            .dynastie_settings
            .as_mut()
            .expect("The pool should have dynasty object.");

        // Insert the past pool at the first element of the list.
        new_dynasty_settings
            .past_season_pool_name
            .insert(0, pool.name.clone());
        new_dynasty_settings.next_season_pool_name = None;

        // If the pool is dynastie type, we need to create a new pool in dynastie status.
        // With almost everying thing from the last pool save into it.

        let pool_context = &pool.context.expect("The pool should have a pool context.");
        let new_dynastie_pool = Pool {
            name: req.new_pool_name,
            owner: pool.owner,
            participants: pool.participants,
            settings: new_settings,
            status: PoolState::Dynastie,
            final_rank: pool.final_rank.clone(),
            nb_player_drafted: 0,
            trades: None,
            context: Some(PoolContext {
                pooler_roster: pool_context.pooler_roster.clone(),
                players_name_drafted: Vec::new(),
                score_by_day: Some(HashMap::new()),
                tradable_picks: Some(Vec::new()),
                past_tradable_picks: pool_context.tradable_picks.clone(),
                players: pool_context.players.clone(),
            }),
            date_updated: 0,
            season_start: START_SEASON_DATE.to_string(),
            season_end: END_SEASON_DATE.to_string(),
            season: POOL_CREATION_SEASON,
        };

        collection
            .insert_one(&new_dynastie_pool, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        let updated_fields = doc! {
            "$set": doc!{
                "settings": to_bson(&pool.settings).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
            }
        };
        let updated_pool = self
            .update_pool(updated_fields, &collection, &req.pool_name)
            .await?;

        return Ok(updated_pool);
    }
}
