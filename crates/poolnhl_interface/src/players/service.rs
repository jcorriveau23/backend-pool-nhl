use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::players::model::{GetPlayerQuery, PlayerInfo};

#[async_trait]
pub trait PlayersService {
    async fn get_players(&self, date: GetPlayerQuery) -> Result<Vec<PlayerInfo>>;
    async fn get_players_with_name(&self, name: &str) -> Result<Vec<PlayerInfo>>;
}

pub type PlayersServiceHandle = Arc<dyn PlayersService + Send + Sync>;
