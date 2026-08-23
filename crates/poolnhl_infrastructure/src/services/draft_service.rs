use async_trait::async_trait;
use mongodb::Collection;
use mongodb::bson::doc;
use mongodb::bson::to_bson;
use poolnhl_interface::draft::service::DraftService;
use poolnhl_interface::errors::AppError;
use poolnhl_interface::players::model::PlayerInfo;
use poolnhl_interface::users::model::UserEmailJwtPayload;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use poolnhl_interface::draft::model::{CommandResponse, RoomUser, RosterModification};
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{Pool, PoolSettings, PoolState, PoolerRoster};

use crate::database_connection::DatabaseConnection;
use crate::database_connection::bson_err;
use crate::jwt::{CachedJwks, hanko_token_decode};

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
            .publish(
                pool_name,
                &CommandResponse::Pool {
                    pool: Box::new(pool),
                },
            )
            .await
    }

    // The roster of `participant` after a draft mutation. Sent with the draft
    // deltas so clients do not have to redo the roster placement rules.
    fn roster_of(pool: &Pool, participant: &str) -> Result<PoolerRoster> {
        pool.context
            .as_ref()
            .and_then(|context| context.pooler_roster.get(participant))
            .cloned()
            .ok_or_else(|| AppError::CustomError {
                msg: format!("no roster found for participant '{}'.", participant),
            })
    }

    // The number of picks made so far, used by clients to detect that they
    // missed a draft delta and need to refetch the pool.
    fn pick_count(pool: &Pool) -> Result<usize> {
        pool.context
            .as_ref()
            .map(|context| context.players_name_drafted.len())
            .ok_or_else(|| AppError::CustomError {
                msg: "pool context does not exist.".to_string(),
            })
    }
}

#[async_trait]
impl DraftService for MongoDraftService {
    async fn start_draft(
        &self,
        pool_name: &str,
        user_id: &str,
        draft_order: &[String],
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
            "$set": to_bson(&pool).map_err(bson_err)?
        };

        // TODO Add the new pool to the list so that we know in which pool each users participated in.
        // add_pool_to_users(&collection_users, &_pool_info.name, participants).await?;

        let updated_pool = update_pool(
            updated_fields,
            &self.pool_collection,
            pool_name,
            pool.date_updated,
        )
        .await?;
        self.publish_pool_info(pool_name, updated_pool).await
    }

    async fn draft_player(&self, pool_name: &str, user_id: &str, player_id: i64) -> Result<()> {
        // This commands is being made when a user try to draft a player.
        // An error is returned if the command is not valid (i.e, not the user turn).

        let mut pool = get_short_pool_by_name(&self.pool_collection, pool_name).await?;
        let player = get_player_with_id(&self.players_collection, player_id).await?;

        // Draft the player.
        let outcome = pool.draft_player(user_id, &player)?;

        // The final pick flips the pool to InProgress: record each participant's
        // initial lineup, effective from the season start, so scores derive from
        // the first game day (later roster changes append their own events).
        if matches!(pool.status, PoolState::InProgress) {
            let season_start = pool.season_start.clone();
            if let Some(context) = pool.context.as_mut() {
                let participants: Vec<String> = context.pooler_roster.keys().cloned().collect();
                for participant in participants {
                    context.record_lineup_change(&participant, &season_start);
                }
            }
        }

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context": to_bson(context).map_err(bson_err)?,
                "status": to_bson(&pool.status).map_err(bson_err)?
            }
        };
        // Update the fields in the mongoDB pool document.

        let updated_pool = update_pool(
            updated_fields,
            &self.pool_collection,
            pool_name,
            pool.date_updated,
        )
        .await?;

        // Only the pick itself is broadcast: the pool is rebroadcast to every
        // socket of the room on every pick and grows with each drafted player,
        // so sending it whole costs tens of kilobytes per pick per socket.
        self.state
            .publish(
                pool_name,
                &CommandResponse::PlayerDrafted {
                    roster: Self::roster_of(&updated_pool, &outcome.drafter)?,
                    pick_count: Self::pick_count(&updated_pool)?,
                    participant_id: outcome.drafter,
                    appended_picks: outcome.appended_picks,
                    player,
                    status: updated_pool.status.clone(),
                    date_updated: updated_pool.date_updated,
                },
            )
            .await
    }

    // Undo the last DraftPlayer command. This command can only be made by the pool owner.
    async fn undo_draft_player(&self, pool_name: &str, user_id: &str) -> Result<()> {
        let mut pool = get_short_pool_by_name(&self.pool_collection, pool_name).await?;

        // Undo the last draft selection.
        let outcome = pool.undo_draft_player(user_id)?;

        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
                "context.players_name_drafted": to_bson(&context.players_name_drafted).map_err(bson_err)?,
            }
        };
        // Update the fields in the mongoDB pool document.
        let updated_pool = update_pool(
            updated_fields,
            &self.pool_collection,
            &pool.name,
            pool.date_updated,
        )
        .await?;
        self.state
            .publish(
                pool_name,
                &CommandResponse::DraftPickUndone {
                    roster: Self::roster_of(&updated_pool, &outcome.drafter)?,
                    pick_count: Self::pick_count(&updated_pool)?,
                    participant_id: outcome.drafter,
                    player_id: outcome.player_id,
                    date_updated: updated_pool.date_updated,
                },
            )
            .await
    }

    // Rearrange the players a participant already holds, during the draft.
    // Everyone in the room has to see it: their copy of the pool is now only
    // refreshed by the deltas, so an unbroadcast roster change would leave the
    // other draft boards showing a lineup that no longer exists.
    async fn modify_roster(
        &self,
        pool_name: &str,
        user_id: &str,
        modification: &RosterModification,
    ) -> Result<()> {
        let mut pool = get_short_pool_by_name(&self.pool_collection, pool_name).await?;

        pool.modify_roster(
            user_id,
            &modification.roster_modified_user_id,
            &modification.forw_list,
            &modification.def_list,
            &modification.goal_list,
            &modification.reserv_list,
        )?;

        // No lineup event here. The rosters are still being filled, and the
        // final pick records everyone's opening lineup from the season start,
        // which is the only lineup a draft-time arrangement can ever produce.
        let context = pool.context.as_ref().ok_or_else(|| AppError::CustomError {
            msg: "pool context does not exist.".to_string(),
        })?;

        let updated_fields = doc! {
            "$set": doc!{
                "context.pooler_roster": to_bson(&context.pooler_roster).map_err(bson_err)?,
            }
        };

        let updated_pool = update_pool(
            updated_fields,
            &self.pool_collection,
            pool_name,
            pool.date_updated,
        )
        .await?;

        self.state
            .publish(
                pool_name,
                &CommandResponse::RosterModified {
                    roster: Self::roster_of(&updated_pool, &modification.roster_modified_user_id)?,
                    participant_id: modification.roster_modified_user_id.clone(),
                    date_updated: updated_pool.date_updated,
                },
            )
            .await
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
                "settings": to_bson(&pool_settings).map_err(bson_err)?,

            }
        };

        let updated_pool = update_pool(
            updated_fields,
            &self.pool_collection,
            pool_name,
            pool.date_updated,
        )
        .await?;
        self.publish_pool_info(pool_name, updated_pool).await
    }

    // List the active room.
    async fn list_rooms(&self) -> Result<Vec<String>> {
        self.state.list_rooms().await
    }

    async fn list_room_users(&self, pool_name: &str) -> Result<HashMap<String, RoomUser>> {
        self.state.list_room_users(pool_name).await
    }

    // Authenticate the token received as inputs.
    // This commands is only being made during the socket initial negociation.
    async fn authenticate_web_socket(
        &self,
        token: &str,
        socket_id: &str,
    ) -> Option<UserEmailJwtPayload> {
        match hanko_token_decode(token, &self.cached_jwks).await {
            Ok(user) => match self.state.add_socket(socket_id, user.clone()) {
                Ok(()) => Some(user),
                Err(_) => None,
            },
            Err(e) => {
                tracing::warn!(error = %e, "web socket authentication failed");
                None
            }
        }
    }

    async fn unauthenticate_web_socket(&self, socket_id: &str) -> Result<()> {
        self.state.remove_socket(socket_id)
    }

    // JoinRoom command.
    async fn join_room(
        &self,
        pool_name: &str,
        number_poolers: u8,
        socket_id: &str,
    ) -> Result<broadcast::Receiver<String>> {
        self.state
            .join_room(pool_name, number_poolers, socket_id)
            .await
    }

    // LeaveRoom command.
    async fn leave_room(&self, pool_name: &str, socket_id: &str) -> Result<()> {
        self.state.leave_room(pool_name, socket_id).await
    }

    // OnReady command. This command can only be made when the pool is into CREATED status.
    async fn on_ready(&self, pool_name: &str, socket_id: &str) -> Result<()> {
        self.state.on_ready(pool_name, socket_id).await
    }

    // AddUser command. This command can only be made when the pool is into CREATED status.
    async fn add_user(&self, pool_name: &str, user_name: &str, socket_id: &str) -> Result<()> {
        self.state.add_user(pool_name, user_name, socket_id).await
    }

    // RemoveUser command. This command can only be made when the pool is into CREATED status.
    async fn remove_user(&self, pool_name: &str, user_id: &str, socket_id: &str) -> Result<()> {
        self.state.remove_user(pool_name, user_id, socket_id).await
    }
}
