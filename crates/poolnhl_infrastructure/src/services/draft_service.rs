use async_trait::async_trait;
use mongodb::bson::doc;
use mongodb::bson::to_bson;
use mongodb::Collection;
use poolnhl_interface::draft::service::DraftService;
use poolnhl_interface::errors::AppError;
use poolnhl_interface::players::model::PlayerInfo;
use poolnhl_interface::users::model::UserEmailJwtPayload;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;

use poolnhl_interface::draft::model::{CommandResponse, RoomUser};
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{Pool, PoolSettings};

use crate::database_connection::DatabaseConnection;
use crate::jwt::{hanko_token_decode, CachedJwks};

use crate::services::draft_state::DraftServerState;
use crate::services::players_service::get_player_with_id;
use crate::services::pool_service::{get_short_pool_by_name, update_pool};

pub struct MongoDraftService {
    // Need both collections during Draft session.
    pool_collection: Collection<Pool>,
    players_collection: Collection<PlayerInfo>,

    // Room membership/presence and broadcasts, shared across instances through redis.
    state: Arc<DraftServerState>,
    cached_jwks: Arc<CachedJwks>,
}

impl MongoDraftService {
    pub fn new(
        db: DatabaseConnection,
        cached_jwks: Arc<CachedJwks>,
        state: Arc<DraftServerState>,
    ) -> Self {
        let pool_collection = db.collection::<Pool>("pools");
        let players_collection = db.collection::<PlayerInfo>("players");
        Self {
            pool_collection,
            players_collection,
            state,
            cached_jwks,
        }
    }

    // Send the pool updated informations to the room (on every instance).
    async fn publish_pool_info(&self, pool_name: &str, pool: Pool) -> Result<()> {
        self.state
            .publish(pool_name, &CommandResponse::Pool { pool })
            .await
    }
}

#[async_trait]
impl DraftService for MongoDraftService {
    async fn start_draft(
        &self,
        pool_name: &str,
        user_id: &str,
        draft_order: &Vec<String>,
    ) -> Result<()> {
        // Commands that initiate the draft. This command update the pool state from CREATED -> DRAFT
        // This update the pool in the database.

        let mut pool = get_short_pool_by_name(&self.pool_collection, pool_name).await?;
        // List all users that participate in the pool.
        // These will be added as official pool participants.
        let room_users = self.state.get_room_users(pool_name).await?;

        pool.start_draft(user_id, &room_users, draft_order)?;

        // Update the fields in the mongoDB pool document.

        let updated_fields = doc! {
            "$set": to_bson(&pool).map_err(|e| AppError::MongoError { msg: e.to_string() })?
        };

        // TODO Add the new pool to the list so that we know in which pool each users participated in.
        // add_pool_to_users(&collection_users, &_pool_info.name, participants).await?;

        let updated_pool = update_pool(updated_fields, &self.pool_collection, pool_name).await?;
        self.publish_pool_info(pool_name, updated_pool).await
    }

    async fn draft_player(&self, pool_name: &str, user_id: &str, player_id: i64) -> Result<()> {
        // This commands is being made when a user try to draft a player.
        // An error is returned if the command is not valid (i.e, not the user turn).

        let mut pool = get_short_pool_by_name(&self.pool_collection, pool_name).await?;
        let player = get_player_with_id(&self.players_collection, player_id).await?;

        // Draft the player.
        pool.draft_player(user_id, &player)?;

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context": to_bson(context).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                "status": to_bson(&pool.status).map_err(|e| AppError::MongoError { msg: e.to_string() })?
            }
        };
        // Update the fields in the mongoDB pool document.

        let updated_pool = update_pool(updated_fields, &self.pool_collection, pool_name).await?;

        self.publish_pool_info(pool_name, updated_pool).await
    }

    // Undo the last DraftPlayer command. This command can only be made by the pool owner.
    async fn undo_draft_player(&self, pool_name: &str, user_id: &str) -> Result<()> {
        let mut pool = get_short_pool_by_name(&self.pool_collection, pool_name).await?;

        // Undo the last draft selection.
        pool.undo_draft_player(user_id)?;

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
                "context.players_name_drafted": to_bson(&context.players_name_drafted).map_err(|e| AppError::MongoError { msg: e.to_string() })?,
            }
        };
        // Update the fields in the mongoDB pool document.
        let updated_pool = update_pool(updated_fields, &self.pool_collection, &pool.name).await?;
        self.publish_pool_info(pool_name, updated_pool).await
    }

    // Update pool settings, this command can only be made by the owner.
    // The pool needs to be into the status CREATED.
    async fn update_pool_settings(
        &self,
        use_id: &str,
        pool_name: &str,
        pool_settings: &PoolSettings,
    ) -> Result<()> {
        let pool = get_short_pool_by_name(&self.pool_collection, pool_name).await?;

        pool.can_update_pool_settings(use_id)?;

        let updated_fields = doc! {
            "$set": doc!{
                "settings": to_bson(&pool_settings).map_err(|e| AppError::MongoError { msg: e.to_string() })?,

            }
        };

        let updated_pool = update_pool(updated_fields, &self.pool_collection, pool_name).await?;
        self.publish_pool_info(pool_name, updated_pool).await
    }

    // List the active room.
    async fn list_rooms(&self) -> Result<Vec<String>> {
        self.state.list_rooms().await
    }

    async fn list_room_users(&self, pool_name: &str) -> Result<HashMap<String, RoomUser>> {
        self.state.list_room_users(pool_name).await
    }

    // Note: sockets are owned by a single instance, so this only lists the
    // sockets authenticated against the instance serving the request.
    async fn list_authenticated_sockets(&self) -> Result<HashMap<String, UserEmailJwtPayload>> {
        self.state.list_authenticated_sockets()
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
                    .state
                    .add_socket(&socket_addr.to_string(), user.clone())
                {
                    Ok(()) => Some(user),
                    Err(_) => None,
                }
            }
            Err(e) => {
                println!("{}", e);
                None
            }
        }
    }

    async fn unauthenticate_web_socket(&self, socket_addr: SocketAddr) -> Result<()> {
        self.state.remove_socket(&socket_addr.to_string())
    }

    // JoinRoom command.
    async fn join_room(
        &self,
        pool_name: &str,
        number_poolers: u8,
        socket_addr: SocketAddr,
    ) -> Result<broadcast::Receiver<String>> {
        self.state
            .join_room(pool_name, number_poolers, &socket_addr.to_string())
            .await
    }

    // LeaveRoom command.
    async fn leave_room(&self, pool_name: &str, socket_addr: SocketAddr) -> Result<()> {
        self.state
            .leave_room(pool_name, &socket_addr.to_string())
            .await
    }

    // OnReady command. This command can only be made when the pool is into CREATED status.
    async fn on_ready(&self, pool_name: &str, socket_addr: SocketAddr) -> Result<()> {
        self.state
            .on_ready(pool_name, &socket_addr.to_string())
            .await
    }

    // AddUser command. This command can only be made when the pool is into CREATED status.
    async fn add_user(
        &self,
        pool_name: &str,
        user_name: &str,
        socket_addr: SocketAddr,
    ) -> Result<()> {
        self.state
            .add_user(pool_name, user_name, &socket_addr.to_string())
            .await
    }

    // RemoveUser command. This command can only be made when the pool is into CREATED status.
    async fn remove_user(
        &self,
        pool_name: &str,
        user_id: &str,
        socket_addr: SocketAddr,
    ) -> Result<()> {
        self.state
            .remove_user(pool_name, user_id, &socket_addr.to_string())
            .await
    }
}
