use async_trait::async_trait;
use mongodb::bson::doc;
use mongodb::bson::{to_bson, Document};
use mongodb::options::{FindOneAndUpdateOptions, FindOneOptions, ReturnDocument};
use mongodb::Collection;
use poolnhl_interface::draft::service::DraftService;
use poolnhl_interface::errors::AppError;
use std::net::SocketAddr;
use std::sync::Mutex;
use tokio::sync::broadcast;

use poolnhl_interface::draft::model::{DraftServerInfo, UserToken};
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{Player, Pool, PoolSettings};

use crate::database_connection::DatabaseConnection;
use crate::jwt::decode;

pub struct MongoDraftService {
    db: DatabaseConnection,
    secret: String,

    draft_server_info: Mutex<DraftServerInfo>,
}

impl MongoDraftService {
    pub fn new(db: DatabaseConnection, secret: String) -> Self {
        Self {
            db,
            secret,
            draft_server_info: Mutex::new(DraftServerInfo::new()),
        }
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
    ) -> Result<()> {
        // Update the fields in the mongoDB pool document.
        // and send back the pool informations to the socket room.
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
            Some(updated_pool) => {
                let draft_server_info = self
                    .draft_server_info
                    .lock()
                    .expect("Could not acquire the mutex");

                if let Some(room) = draft_server_info.rooms.get(pool_name) {
                    return room.send_pool_info(updated_pool);
                }
                Err(AppError::CustomError {
                    msg: "Could not find the room.".to_string(),
                })
            }
            None => Err(AppError::CustomError {
                msg: "The pool could not be updated.".to_string(),
            }),
        }
    }
}
#[async_trait]
impl DraftService for MongoDraftService {
    // Commands that initiate the draft. This command update the pool state from CREATED -> DRAFT
    // This update the pool in the database.
    async fn start_draft(&self, pool_name: &str, user_id: &str) -> Result<()> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = self.get_short_pool_by_name(&collection, pool_name).await?;

        // Create a context so the mutex is getting released
        {
            let draft_server_info = self
                .draft_server_info
                .lock()
                .expect("Could not acquire the mutex");

            // List all users that participate in the pool.
            // These will be added as official pool participants.
            if let Some(room) = draft_server_info.rooms.get(pool_name) {
                let participants = room
                    .users
                    .keys()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>();

                pool.start_draft(user_id, &participants)?;
            }
        }

        // Update the whole pool information in database.
        let collection = self.db.collection::<Pool>("pools");

        // Update the fields in the mongoDB pool document.

        let updated_fields = doc! {
            "$set": to_bson(&pool).map_err(|e| AppError::MongoError { msg: e.to_string() })?
        };

        // TODO Add the new pool to the list so that we know in which pool each users participated in.
        // add_pool_to_users(&collection_users, &_pool_info.name, participants).await?;

        self.update_pool(updated_fields, &collection, &pool.name)
            .await
    }

    // This commands is being made when a user try to draft a player.
    // An error is returned if the command is not valid (i.e, not the user turn).
    async fn draft_player(&self, pool_name: &str, user_id: &str, player: Player) -> Result<()> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = self.get_short_pool_by_name(&collection, pool_name).await?;

        // Draft the player.
        pool.draft_player(user_id, &player)?;

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

                self.update_pool(updated_fields, &collection, pool_name)
                    .await
            }
        }
    }

    // Undo the last DraftPlayer command. This command can only be made by the pool owner.
    async fn undo_draft_player(&self, pool_name: &str, user_id: &str) -> Result<()> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = self.get_short_pool_by_name(&collection, pool_name).await?;

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

                self.update_pool(updated_fields, &collection, pool_name)
                    .await
            }
        }
    }

    // Update pool settings, this command can only be made by the owner.
    // The pool needs to be into the status CREATED.
    async fn update_pool_settings(
        &self,
        use_id: &str,
        pool_name: &str,
        pool_settings: &PoolSettings,
    ) -> Result<()> {
        let collection = self.db.collection::<Pool>("pools");

        let pool = self.get_short_pool_by_name(&collection, pool_name).await?;

        pool.can_update_pool_settings(use_id)?;

        let updated_fields = doc! {
            "$set": doc!{
                "settings": to_bson(&pool_settings).map_err(|e| AppError::MongoError { msg: e.to_string() })?,

            }
        };

        self.update_pool(updated_fields, &collection, pool_name)
            .await
    }

    // List the active room.
    async fn list_rooms(&self) -> Result<Vec<String>> {
        let draft_server_info = self
            .draft_server_info
            .lock()
            .expect("Could not acquire the mutex");
        Ok(draft_server_info.list_rooms())
    }

    // Authentificate the token received as inputs.
    // This commands is only being made during the socket initial negociation.
    fn authentificate_web_socket(&self, token: &str, socket_addr: SocketAddr) -> Option<UserToken> {
        if let Ok(user) = decode(token, &self.secret) {
            let mut draft_server_info = self
                .draft_server_info
                .lock()
                .expect("Could not acquire the mutex");
            draft_server_info.add_socket(&socket_addr.to_string(), user.claims.user.clone());
            return Some(user.claims.user);
        }

        None
    }

    // JoinRoom command.
    fn join_room(
        &self,
        pool_name: &str,
        socket_addr: SocketAddr,
    ) -> (broadcast::Receiver<String>, String) {
        let mut draft_server_info = self
            .draft_server_info
            .lock()
            .expect("Could not acquire the mutex");
        draft_server_info.join_room(pool_name, &socket_addr.to_string())
    }

    // LeaveRoom command.
    fn leave_room(&self, pool_name: &str, socket_addr: SocketAddr) {
        let mut draft_server_info = self
            .draft_server_info
            .lock()
            .expect("Could not acquire the mutex");
        draft_server_info.leave_room(pool_name, &socket_addr.to_string())
    }

    // OnReady command. This command can only be made when the pool is into CREATED status.
    fn on_ready(&self, pool_name: &str, socket_addr: SocketAddr) {
        let mut draft_server_info = self
            .draft_server_info
            .lock()
            .expect("Could not acquire the mutex");
        draft_server_info.on_ready(pool_name, &socket_addr.to_string())
    }
}
