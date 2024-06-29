use async_trait::async_trait;
use mongodb::bson::doc;
use mongodb::bson::to_bson;
use poolnhl_interface::draft::service::DraftService;
use poolnhl_interface::errors::AppError;
use poolnhl_interface::users::model::UserEmailJwtPayload;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;

use poolnhl_interface::draft::model::{CommandResponse, DraftServerInfo, RoomUser};
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{Player, Pool, PoolSettings};

use crate::database_connection::DatabaseConnection;
use crate::jwt::{hanko_token_decode, CachedJwks};

use crate::services::pool_service::{get_short_pool_by_name, update_pool};

pub struct MongoDraftService {
    db: DatabaseConnection,

    draft_server_info: DraftServerInfo,
    cached_jwks: Arc<CachedJwks>,
}

// Send the pool updated informations to the room.
pub fn send_pool_info(tx: broadcast::Sender<String>, pool: Pool) -> Result<()> {
    let pool_string = serde_json::to_string(&CommandResponse::Pool { pool })
        .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

    let _ = tx.send(pool_string);
    Ok(())
}

// Send the pool updated informations to the room.
pub fn send_users_info(
    tx: broadcast::Sender<String>,
    room_users: HashMap<String, RoomUser>,
) -> Result<()> {
    let room_users = serde_json::to_string(&CommandResponse::Users { room_users })
        .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

    let _ = tx.send(room_users);
    Ok(())
}

impl MongoDraftService {
    pub fn new(db: DatabaseConnection, cached_jwks: Arc<CachedJwks>) -> Self {
        Self {
            db,
            cached_jwks: cached_jwks,
            draft_server_info: DraftServerInfo::new(),
        }
    }
}

#[async_trait]
impl DraftService for MongoDraftService {
    async fn start_draft(&self, pool_name: &str, user_id: &str) -> Result<()> {
        // Commands that initiate the draft. This command update the pool state from CREATED -> DRAFT
        // This update the pool in the database.
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = get_short_pool_by_name(&collection, pool_name).await?;
        // List all users that participate in the pool.
        // These will be added as official pool participants.
        let room_users = self.draft_server_info.get_room_users(pool_name)?;

        pool.start_draft(user_id, &room_users)?;

        // Update the whole pool information in database.
        let collection = self.db.collection::<Pool>("pools");

        // Update the fields in the mongoDB pool document.

        let updated_fields = doc! {
            "$set": to_bson(&pool).map_err(|e| AppError::MongoError { msg: e.to_string() })?
        };

        // TODO Add the new pool to the list so that we know in which pool each users participated in.
        // add_pool_to_users(&collection_users, &_pool_info.name, participants).await?;

        let updated_pool = update_pool(updated_fields, &collection, pool_name).await?;
        send_pool_info(self.draft_server_info.get_room_tx(pool_name)?, updated_pool)
    }

    async fn draft_player(&self, pool_name: &str, user_id: &str, player: Player) -> Result<()> {
        // This commands is being made when a user try to draft a player.
        // An error is returned if the command is not valid (i.e, not the user turn).
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = get_short_pool_by_name(&collection, pool_name).await?;

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

                let updated_pool = update_pool(updated_fields, &collection, pool_name).await?;

                // Get a copy of the pool tx than send the pool information.
                send_pool_info(self.draft_server_info.get_room_tx(pool_name)?, updated_pool)
            }
        }
    }

    // Undo the last DraftPlayer command. This command can only be made by the pool owner.
    async fn undo_draft_player(&self, pool_name: &str, user_id: &str) -> Result<()> {
        let collection = self.db.collection::<Pool>("pools");

        let mut pool = get_short_pool_by_name(&collection, pool_name).await?;

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
                let updated_pool = update_pool(updated_fields, &collection, &pool.name).await?;
                send_pool_info(self.draft_server_info.get_room_tx(pool_name)?, updated_pool)
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

        let pool = get_short_pool_by_name(&collection, pool_name).await?;

        pool.can_update_pool_settings(use_id)?;

        let updated_fields = doc! {
            "$set": doc!{
                "settings": to_bson(&pool_settings).map_err(|e| AppError::MongoError { msg: e.to_string() })?,

            }
        };

        let updated_pool = update_pool(updated_fields, &collection, pool_name).await?;
        send_pool_info(self.draft_server_info.get_room_tx(pool_name)?, updated_pool)
    }

    // List the active room.
    async fn list_rooms(&self) -> Result<Vec<String>> {
        self.draft_server_info.list_rooms()
    }

    async fn list_room_users(&self, pool_name: &str) -> Result<HashMap<String, RoomUser>> {
        self.draft_server_info.list_room_users(pool_name)
    }

    async fn list_authenticated_sockets(&self) -> Result<HashMap<String, UserEmailJwtPayload>> {
        self.draft_server_info.list_authenticated_sockets()
    }

    // Authenticate the token received as inputs.
    // This commands is only being made during the socket initial negociation.
    async fn authenticate_web_socket(
        &self,
        token: &str,
        socket_addr: SocketAddr,
    ) -> Option<UserEmailJwtPayload> {
        match hanko_token_decode(token, &self.cached_jwks).await {
            Ok(user) => {
                match self
                    .draft_server_info
                    .add_socket(&socket_addr.to_string(), user.clone())
                {
                    Ok(()) => return Some(user),
                    Err(_) => return None,
                }
            }
            Err(e) => {
                println!("{}", e);
                None
            }
        }
    }

    async fn unauthenticate_web_socket(&self, socket_addr: SocketAddr) -> Result<()> {
        self.draft_server_info
            .remove_socket(&socket_addr.to_string())
    }

    // JoinRoom command.
    async fn join_room(
        &self,
        pool_name: &str,
        number_poolers: u8,
        socket_addr: SocketAddr,
    ) -> Result<broadcast::Receiver<String>> {
        let (rx, room_users) = self.draft_server_info.join_room(
            pool_name,
            number_poolers,
            &socket_addr.to_string(),
        )?;

        let tx = self.draft_server_info.get_room_tx(pool_name)?;
        send_users_info(tx, room_users)?;

        Ok(rx)
    }

    // LeaveRoom command.
    async fn leave_room(&self, pool_name: &str, socket_addr: SocketAddr) -> Result<()> {
        let room_users = self
            .draft_server_info
            .leave_room(pool_name, &socket_addr.to_string())?;

        let tx = self.draft_server_info.get_room_tx(pool_name)?;
        send_users_info(tx, room_users)
    }

    // OnReady command. This command can only be made when the pool is into CREATED status.
    async fn on_ready(&self, pool_name: &str, socket_addr: SocketAddr) -> Result<()> {
        let room_users = self
            .draft_server_info
            .on_ready(pool_name, &socket_addr.to_string())?;

        let tx = self.draft_server_info.get_room_tx(pool_name)?;
        send_users_info(tx, room_users)
    }
}
