use std::sync::Arc;

use async_trait::async_trait;

use crate::daily_leaders::model::DailyLeaders;
use crate::errors::Result;

#[async_trait]
pub trait DailyLeadersService {
    async fn get_daily_leaders(&self, date: &str) -> Result<DailyLeaders>;
}

pub type DailyLeadersServiceHandle = Arc<dyn DailyLeadersService + Send + Sync>;
