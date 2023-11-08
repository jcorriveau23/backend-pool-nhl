use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::nhl::model::{DailyGames, GameBoxScore, GameLanding};

#[async_trait]
pub trait NhlService {
    async fn get_daily_games(&self, date: &str) -> Result<DailyGames>;
    async fn get_game_box_score(&self, id: u32) -> Result<GameBoxScore>;
    async fn get_game_landing(&self, id: u32) -> Result<GameLanding>;
}

pub type NhlServiceHandle = Arc<dyn NhlService + Send + Sync>;
