use axum::extract::{Json, Path, State};
use axum::routing::{get, post};
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::draft::model::UserToken;
use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::users::{
    model::{
        LoginRequest, LoginResponse, RegisterRequest, SetPasswordRequest, SetUsernameRequest,
        SocialLoginRequest, UserData, WalletLoginRegisterRequest,
    },
    service::UsersServiceHandle,
};

pub struct UsersRouter;

impl UsersRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/user/:name", get(Self::get_user_by_name))
            .route("/users/:ids", get(Self::get_user_by_ids))
            .route("/user/login", post(Self::login))
            .route("/user/register", post(Self::register))
            .route("/user/wallet-login", post(Self::wallet_login))
            .route("/user/social-login", post(Self::social_login))
            .route("/user/link-social-account", post(Self::link_social_account))
            .route("/user/set-username", post(Self::set_username))
            .route("/user/set-password", post(Self::set_password))
            .route("/token", post(Self::validate_token))
            .with_state(service_registry)
    }

    async fn get_user_by_name(
        State(users_service): State<UsersServiceHandle>,
        Path(name): Path<String>,
    ) -> Result<Json<UserData>> {
        users_service.get_user_by_name(&name).await.map(Json)
    }

    async fn get_user_by_ids(
        State(users_service): State<UsersServiceHandle>,
        Path(ids): Path<String>,
    ) -> Result<Json<Vec<UserData>>> {
        let ids: Vec<&str> = ids.split(',').collect();
        users_service.get_users_by_ids(&ids).await.map(Json)
    }

    async fn login(
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<LoginRequest>,
    ) -> Result<Json<LoginResponse>> {
        users_service.login(body).await.map(Json)
    }

    async fn register(
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<RegisterRequest>,
    ) -> Result<Json<LoginResponse>> {
        users_service.register(body).await.map(Json)
    }

    async fn wallet_login(
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<WalletLoginRegisterRequest>,
    ) -> Result<Json<LoginResponse>> {
        users_service.wallet_login(body).await.map(Json)
    }

    async fn social_login(
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<SocialLoginRequest>,
    ) -> Result<Json<LoginResponse>> {
        users_service.social_login(body).await.map(Json)
    }

    async fn link_social_account(
        token: UserToken,
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<SocialLoginRequest>,
    ) -> Result<Json<UserData>> {
        users_service
            .link_social_account(&token._id, body)
            .await
            .map(Json)
    }

    /// Set Username
    async fn set_username(
        token: UserToken,
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<SetUsernameRequest>,
    ) -> Result<Json<UserData>> {
        users_service.set_username(&token._id, body).await.map(Json)
    }

    /// Set Username
    async fn set_password(
        token: UserToken,
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<SetPasswordRequest>,
    ) -> Result<Json<UserData>> {
        users_service.set_password(&token._id, body).await.map(Json)
    }

    /// Validate the token, the validation is being done in the from_request_parts() implementation for UserToken.
    async fn validate_token(token: UserToken) -> Result<()> {
        Ok(())
    }
}
