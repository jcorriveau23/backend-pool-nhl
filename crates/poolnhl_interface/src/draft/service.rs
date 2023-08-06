use std::sync::Arc;

use async_trait::async_trait;

use crate::draft::model::{SelectPlayerRequest, StartDraftRequest, UndoSelectionRequest};
use crate::errors::Result;
use crate::pool::model::Pool;

#[async_trait]
pub trait DraftService {
    async fn start_draft(&self, user_id: &str, req: &mut StartDraftRequest) -> Result<Pool>;
    async fn draft_player(&self, user_id: &str, req: SelectPlayerRequest) -> Result<Pool>;
    async fn undo_draft_player(&self, user_id: &str, req: UndoSelectionRequest) -> Result<Pool>;
}

pub type DraftServiceHandle = Arc<dyn DraftService + Send + Sync>;
