use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::users::model::{
    LoginRequest, LoginResponse, RegisterRequest, SetPasswordRequest, SetUsernameRequest,
    SocialLoginRequest, UserData, WalletLoginRegisterRequest,
};

#[async_trait]
pub trait UsersService {
    async fn get_user_by_name(&self, name: &str) -> Result<UserData>;
    async fn get_users_by_ids(&self, ids: &'life1 [&str]) -> Result<Vec<UserData>>;
    async fn login(&self, body: LoginRequest) -> Result<LoginResponse>;
    async fn register(&self, body: RegisterRequest) -> Result<LoginResponse>;
    async fn wallet_login(&self, body: WalletLoginRegisterRequest) -> Result<LoginResponse>;
    async fn social_login(&self, body: SocialLoginRequest) -> Result<LoginResponse>;
    async fn link_social_account(
        &self,
        user_id: &str,
        body: SocialLoginRequest,
    ) -> Result<UserData>;
    async fn set_username(&self, user_id: &str, body: SetUsernameRequest) -> Result<UserData>;
    async fn set_password(&self, user_id: &str, body: SetPasswordRequest) -> Result<UserData>;
}

pub type UsersServiceHandle = Arc<dyn UsersService + Send + Sync>;
