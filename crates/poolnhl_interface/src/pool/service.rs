use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::pool::model::{
    AddPlayerRequest, CreateTradeRequest, DeleteTradeRequest, FillSpotRequest,
    GenerateDynastyRequest, MarkAsFinalRequest, ModifyRosterRequest, Pool, PoolCreationRequest,
    PoolDeletionRequest, ProjectedPoolShort, ProtectPlayersRequest, RemovePlayerRequest,
    RespondTradeRequest, UpdatePoolSettingsRequest,
};

#[async_trait]
pub trait PoolService {
    // Get pool info calls
    async fn get_pool_by_name(&self, name: &str) -> Result<Pool>;
    async fn get_pool_by_name_with_range(
        &self,
        name: &str,
        start_season_date: &str,
        from_date: &str,
    ) -> Result<Pool>;
    async fn list_pools(&self, season: u32) -> Result<Vec<ProjectedPoolShort>>;
    // Pool creation/deletion calls
    async fn create_pool(&self, user_id: &str, req: PoolCreationRequest) -> Result<Pool>;
    async fn delete_pool(&self, user_id: &str, req: PoolDeletionRequest) -> Result<Pool>;
    // Pool in progress calls
    async fn add_player(&self, user_id: &str, req: AddPlayerRequest) -> Result<Pool>;
    async fn remove_player(&self, user_id: &str, req: RemovePlayerRequest) -> Result<Pool>;
    async fn create_trade(&self, user_id: &str, req: &mut CreateTradeRequest) -> Result<Pool>;
    async fn delete_trade(&self, user_id: &str, req: DeleteTradeRequest) -> Result<Pool>;
    async fn respond_trade(&self, user_id: &str, req: RespondTradeRequest) -> Result<Pool>;
    async fn fill_spot(&self, user_id: &str, req: FillSpotRequest) -> Result<Pool>;
    async fn modify_roster(&self, user_id: &str, req: ModifyRosterRequest) -> Result<Pool>;
    async fn update_pool_settings(
        &self,
        user_id: &str,
        req: UpdatePoolSettingsRequest,
    ) -> Result<Pool>;
    // Dynasty call
    async fn protect_players(&self, user_id: &str, req: ProtectPlayersRequest) -> Result<Pool>;
    async fn mark_as_final(&self, user_id: &str, req: MarkAsFinalRequest) -> Result<Pool>;
    async fn generate_dynasty(&self, user_id: &str, req: GenerateDynastyRequest) -> Result<Pool>;
}

pub type PoolServiceHandle = Arc<dyn PoolService + Send + Sync>;
