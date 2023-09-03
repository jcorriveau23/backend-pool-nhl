use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::pool::model::{Player, PoolSettings};
use std::net::SocketAddr;
use tokio::sync::broadcast;

use super::model::UserToken;

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
    fn join_room(
        &self,
        pool_name: &str,
        socket_addr: SocketAddr,
    ) -> (broadcast::Receiver<String>, String);
    fn leave_room(&self, pool_name: &str, socket_addr: SocketAddr);
    fn on_ready(&self, pool_name: &str, socket_addr: SocketAddr);

    // Socket jwt token authentifications (called only on socket connection)
    fn authentificate_web_socket(&self, token: &str, socket_addr: SocketAddr) -> Option<UserToken>;

    // end point that list the active rooms.
    async fn list_rooms(&self) -> Result<Vec<String>>;
}

pub type DraftServiceHandle = Arc<dyn DraftService + Send + Sync>;
