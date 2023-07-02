use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::users::model::UserData;

#[async_trait]
pub trait UsersService {
    async fn get_user_by_name(&self, name: &str) -> Result<UserData>;
    async fn list_all_users(&self) -> Result<Vec<UserData>>;
}

pub type UsersServiceHandle = Arc<dyn UsersService + Send + Sync>;
