use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::users::model::{
    LoginRequest, LoginResponse, RegisterRequest, SetPasswordRequest, SetUsernameRequest, UserData,
    WalletLoginRegisterRequest,
};

#[async_trait]
pub trait UsersService {
    async fn get_user_by_name(&self, name: &str) -> Result<UserData>;
    async fn get_users_by_ids(&self, ids: &Vec<String>) -> Result<Vec<UserData>>;
    async fn login(&self, body: LoginRequest) -> Result<LoginResponse>;
    async fn register(&self, body: RegisterRequest) -> Result<LoginResponse>;
    async fn wallet_login(&self, body: WalletLoginRegisterRequest) -> Result<LoginResponse>;
    async fn set_username(&self, body: SetUsernameRequest) -> Result<UserData>;
    async fn set_password(&self, body: SetPasswordRequest) -> Result<UserData>;
}

pub type UsersServiceHandle = Arc<dyn UsersService + Send + Sync>;
