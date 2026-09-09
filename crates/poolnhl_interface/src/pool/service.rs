use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::pool::model::{Pool, ProjectedPoolShort};
use crate::pool::requests::{
    AddPlayerRequest, CompleteProtectionRequest, ConfirmTradeRequest, CreateTradeRequest,
    DeleteTradeRequest, FillSpotRequest, GenerateDynastyRequest, MarkAsFinalRequest,
    ModifyRosterRequest, PoolCreationRequest, PoolDeletionRequest, PoolerLinkRequest,
    ProtectPlayersRequest, RemovePlayerRequest, RequestPoolerLinkRequest,
    UpdatePoolSettingsRequest, UpdatePoolerNameRequest, UpdateTradeRequest,
};
use crate::pool::scoring::DailyRosterPoints;

#[async_trait]
pub trait PoolService {
    // Get pool info calls
    async fn init_indexes(&self) -> Result<()>;
    async fn get_pool_by_name(&self, name: &str) -> Result<Pool>;
    async fn list_pools(&self, season: u32) -> Result<Vec<ProjectedPoolShort>>;
    // Pool creation/deletion calls
    async fn create_pool(&self, user_id: &str, req: PoolCreationRequest) -> Result<Pool>;
    async fn delete_pool(&self, user_id: &str, req: PoolDeletionRequest) -> Result<Pool>;
    // Pool in progress calls
    async fn add_player(&self, user_id: &str, req: AddPlayerRequest) -> Result<Pool>;
    async fn remove_player(&self, user_id: &str, req: RemovePlayerRequest) -> Result<Pool>;
    async fn create_trade(&self, user_id: &str, req: &mut CreateTradeRequest) -> Result<Pool>;
    async fn update_trade(&self, user_id: &str, req: UpdateTradeRequest) -> Result<Pool>;
    async fn confirm_trade(&self, user_id: &str, req: ConfirmTradeRequest) -> Result<Pool>;
    async fn delete_trade(&self, user_id: &str, req: DeleteTradeRequest) -> Result<Pool>;
    async fn fill_spot(&self, user_id: &str, req: FillSpotRequest) -> Result<Pool>;
    async fn modify_roster(&self, user_id: &str, req: ModifyRosterRequest) -> Result<Pool>;
    async fn update_pool_settings(
        &self,
        user_id: &str,
        req: UpdatePoolSettingsRequest,
    ) -> Result<Pool>;
    async fn update_pooler_name(&self, user_id: &str, req: UpdatePoolerNameRequest)
    -> Result<Pool>;
    async fn request_pooler_link(
        &self,
        user_id: &str,
        req: RequestPoolerLinkRequest,
    ) -> Result<Pool>;
    async fn cancel_pooler_link(&self, user_id: &str, req: PoolerLinkRequest) -> Result<Pool>;
    async fn accept_pooler_link(
        &self,
        user_id: &str,
        email: &str,
        req: PoolerLinkRequest,
    ) -> Result<Pool>;
    async fn decline_pooler_link(&self, email: &str, req: PoolerLinkRequest) -> Result<Pool>;
    // Dynasty call
    async fn protect_players(&self, user_id: &str, req: ProtectPlayersRequest) -> Result<Pool>;
    async fn complete_protection(
        &self,
        user_id: &str,
        req: CompleteProtectionRequest,
    ) -> Result<Pool>;
    /// `scores` is the season's derived days: the ranking is computed from the
    /// shared day leaders, which the pool itself no longer keeps a copy of.
    async fn mark_as_final(
        &self,
        user_id: &str,
        req: MarkAsFinalRequest,
        scores: &HashMap<String, HashMap<String, DailyRosterPoints>>,
    ) -> Result<Pool>;
    async fn generate_dynasty(&self, user_id: &str, req: GenerateDynastyRequest) -> Result<Pool>;
}

pub type PoolServiceHandle = Arc<dyn PoolService + Send + Sync>;
