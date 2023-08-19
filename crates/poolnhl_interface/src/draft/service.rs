use std::sync::Arc;

use async_trait::async_trait;

use crate::draft::model::{SelectPlayerRequest, StartDraftRequest, UndoSelectionRequest};
use crate::errors::Result;
use crate::pool::model::Pool;
use std::net::SocketAddr;

#[async_trait]
pub trait DraftService {
    async fn start_draft(&self, user_id: &str, req: &mut StartDraftRequest) -> Result<Pool>;
    async fn draft_player(&self, user_id: &str, req: SelectPlayerRequest) -> Result<Pool>;
    async fn undo_draft_player(&self, user_id: &str, req: UndoSelectionRequest) -> Result<Pool>;
    async fn list_rooms(&self) -> Result<Vec<String>>;

    fn authentificate_web_socket(&self, token: &str, socket_addr: SocketAddr);
    fn join_room(&self, pool_name: &str, socket_addr: SocketAddr);
    fn leave_room(&self, pool_name: &str, socket_addr: SocketAddr);
}

pub type DraftServiceHandle = Arc<dyn DraftService + Send + Sync>;
