use async_trait::async_trait;

use mongodb::bson::doc;
use mongodb::bson::{to_bson, Document};
use mongodb::options::{FindOneAndUpdateOptions, FindOneOptions, ReturnDocument};
use mongodb::Collection;
use poolnhl_interface::errors::AppError;

use poolnhl_interface::draft::{
    model::{SelectPlayerRequest, StartDraftRequest, UndoSelectionRequest},
    service::DraftService,
};
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::Pool;

use crate::database_connection::DatabaseConnection;

#[derive(Clone)]
pub struct MongoDraftService {
    db: DatabaseConnection,
}

impl MongoDraftService {
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
impl DraftService for MongoDraftService {
    async fn start_draft(&self, user_id: &str, req: &mut StartDraftRequest) -> Result<Pool> {
        // TODO: Validate that the list of users provided all exist.
        // let collection_users = self.db.collection::<User>("users");

        // try to start the draft.
        req.pool.start_draft(user_id)?;

        let collection = self.db.collection::<Pool>("pools");

        // Update the fields in the mongoDB pool document.

        let updated_fields = doc! {
            "$set": to_bson(&req.pool).map_err(|e| AppError::MongoError { msg: e.to_string() })?
        };

        // create pool context
        return self
            .update_pool(updated_fields, &collection, &req.pool.name)
            .await;

        // Add the new pool to the list of pool in each users.
        // add_pool_to_users(&collection_users, &_pool_info.name, participants).await?;
    }

    async fn draft_player(&self, user_id: &str, req: SelectPlayerRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // Draft the player.
        pool.draft_player(user_id, &req.player)?;

        // updated fields.

        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "There is no context in the pool yet.".to_string(),
            }),
            Some(context) => {
                let updated_fields = doc! {
                    "$set": doc!{
                        "context": to_bson(context).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                        "status": to_bson(&pool.status).map_err(|e| AppError::MongoError { msg: e.to_string() })?
                    }
                };
                // Update the fields in the mongoDB pool document.

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }
    async fn undo_draft_player(&self, user_id: &str, req: UndoSelectionRequest) -> Result<Pool> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = self
            .get_short_pool_by_name(&collection, &req.pool_name)
            .await?;

        // Undo the last draft selection.
        pool.undo_draft_player(user_id)?;

        match &pool.context {
            None => Err(AppError::CustomError {
                msg: "There is no context in the pool yet.".to_string(),
            }),
            Some(context) => {
                let updated_fields = doc! {
                    "$set": doc!{
                        "context.pooler_roster": to_bson(&context.pooler_roster).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                        "context.players_name_drafted": to_bson(&context.players_name_drafted).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                    }
                };
                // Update the fields in the mongoDB pool document.

                self.update_pool(updated_fields, &collection, &req.pool_name)
                    .await
            }
        }
    }
}
