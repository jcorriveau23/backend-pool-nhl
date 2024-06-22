use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::pool::model::{Player, PoolSettings};
use crate::users::model::UserEmailJwtPayload;
use std::net::SocketAddr;
use tokio::sync::broadcast;

use super::model::RoomUser;

#[async_trait]
pub trait DraftService {
    // Socket Pool commands
    async fn start_draft(&self, pool_name: &str, user_id: &str) -> Result<()>;
    async fn draft_player(&self, pool_name: &str, user_id: &str, player: Player) -> Result<()>;
    async fn undo_draft_player(&self, pool_name: &str, user_id: &str) -> Result<()>;
    async fn update_pool_settings(
        &self,
        use_id: &str,
        pool_name: &str,
        pool_settings: &PoolSettings,
    ) -> Result<()>;

    // Socket Room commands:
    async fn join_room(
        &self,
        pool_name: &str,
        socket_addr: SocketAddr,
    ) -> Result<broadcast::Receiver<String>>;
    async fn leave_room(&self, pool_name: &str, socket_addr: SocketAddr) -> Result<()>;
    async fn on_ready(&self, pool_name: &str, socket_addr: SocketAddr) -> Result<()>;

    // Socket jwt token authentications (called only on socket connection)
    async fn authenticate_web_socket(
        &self,
        token: &str,
        socket_addr: SocketAddr,
    ) -> Option<UserEmailJwtPayload>;
    async fn unauthenticate_web_socket(&self, socket_addr: SocketAddr) -> Result<()>;

    // end point that list the active rooms.
    async fn list_rooms(&self) -> Result<Vec<String>>;
    async fn list_room_users(&self, pool_name: &str) -> Result<HashMap<String, RoomUser>>;
    async fn list_authenticated_sockets(&self) -> Result<HashMap<String, UserEmailJwtPayload>>;
}

pub type DraftServiceHandle = Arc<dyn DraftService + Send + Sync>;
