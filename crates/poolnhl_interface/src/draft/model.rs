use serde::Deserialize;

use crate::pool::model::{Player, Pool};

// payload to sent when deleting a pool.
#[derive(Debug, Deserialize)]
pub struct StartDraftRequest {
    pub pool: Pool,
}

// payload to sent when undoing a selection in a pool by the owner.
#[derive(Debug, Deserialize)]
pub struct UndoSelectionRequest {
    pub pool_name: String,
}
// payload to sent when selecting a player.
#[derive(Debug, Deserialize)]
pub struct SelectPlayerRequest {
    pub pool_name: String,
    pub player: Player,
}
