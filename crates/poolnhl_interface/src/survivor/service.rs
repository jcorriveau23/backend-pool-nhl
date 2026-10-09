use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::survivor::model::{SurvivorPool, SurvivorPoolShort, SurvivorStandings};
use crate::survivor::picks::{SurvivorPickOptions, SurvivorPickView};
use crate::survivor::requests::{
    JoinSurvivorRequest, LeaveSurvivorRequest, MakePickRequest, SettleWeekRequest,
    SurvivorCreationRequest, SurvivorDeletionRequest, UpdateSurvivorSettingsRequest,
};

#[async_trait]
pub trait SurvivorService {
    async fn init_indexes(&self) -> Result<()>;

    async fn get_pool_by_name(&self, name: &str) -> Result<SurvivorPool>;
    async fn list_pools(&self, season: u32) -> Result<Vec<SurvivorPoolShort>>;

    async fn create_pool(
        &self,
        user_id: &str,
        req: SurvivorCreationRequest,
    ) -> Result<SurvivorPool>;
    async fn delete_pool(&self, user_id: &str, req: SurvivorDeletionRequest) -> Result<()>;
    async fn update_settings(
        &self,
        user_id: &str,
        req: UpdateSurvivorSettingsRequest,
    ) -> Result<SurvivorPool>;

    async fn join_pool(&self, user_id: &str, req: JoinSurvivorRequest) -> Result<SurvivorPool>;
    async fn leave_pool(&self, user_id: &str, req: LeaveSurvivorRequest) -> Result<SurvivorPool>;

    /// What a participant needs to pick for a date: the teams playing, the ones
    /// still open to them, and the pick they already have in.
    async fn get_pick_options(
        &self,
        user_id: &str,
        pool_name: &str,
        week: u16,
    ) -> Result<SurvivorPickOptions>;

    async fn make_pick(&self, user_id: &str, req: MakePickRequest) -> Result<SurvivorPickOptions>;

    /// Every pick of a date, for the reveal.
    ///
    /// Answers with nothing but the caller's own pick while the date is still
    /// open: seeing the field before you pick is the one thing that would break
    /// the game.
    async fn get_week_picks(
        &self,
        user_id: &str,
        pool_name: &str,
        week: u16,
    ) -> Result<Vec<SurvivorPickView>>;

    /// A participant's own picks across the season, which is where the teams
    /// they have spent comes from.
    async fn get_my_picks(&self, user_id: &str, pool_name: &str) -> Result<Vec<SurvivorPickView>>;

    /// Close a date to new picks, which is what reveals them to everybody.
    ///
    /// Separate from settling: the reveal belongs at the first puck drop, the
    /// eliminations hours later when the last game ends. Idempotent.
    async fn lock_week(&self, user_id: &str, req: SettleWeekRequest) -> Result<SurvivorPool>;

    /// Close a date and apply what the games did. Idempotent.
    async fn settle_week(&self, user_id: &str, req: SettleWeekRequest) -> Result<SurvivorPool>;

    /// The standings, aggregated server-side so a pool of hundreds does not ship
    /// every pick to the browser to be counted there.
    async fn get_standings(&self, pool_name: &str) -> Result<SurvivorStandings>;
}

pub type SurvivorServiceHandle = Arc<dyn SurvivorService + Send + Sync>;
