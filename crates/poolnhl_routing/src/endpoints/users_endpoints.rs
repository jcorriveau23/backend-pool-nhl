use axum::extract::{Json, Path, State};
use axum::routing::{get, post};
use axum::{debug_handler, Router};

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::errors::Result;
use poolnhl_interface::users::{
    model::{
        LoginRequest, LoginResponse, RegisterRequest, SetPasswordRequest, SetUsernameRequest,
        UserData, WalletLoginRegisterRequest,
    },
    service::UsersServiceHandle,
};

use poolnhl_infrastructure::jwt::UserToken;

pub struct UsersRouter;

impl UsersRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/user/:name", get(UsersRouter::get_user_by_name))
            .route("/users/:ids", get(UsersRouter::get_user_by_ids))
            .route("/user/login", post(UsersRouter::login))
            .route("/user/register", post(UsersRouter::register))
            .route("/user/wallet-login", post(UsersRouter::wallet_login))
            .route("/user/set-username", post(UsersRouter::set_username))
            .route("/user/set-password", post(UsersRouter::set_password))
            .with_state(service_registry)
    }

    async fn get_user_by_name(
        Path(name): Path<String>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<UserData>> {
        users_service.get_user_by_name(&name).await.map(Json)
    }

    async fn get_user_by_ids(
        Path(ids): Path<Vec<String>>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<Vec<UserData>>> {
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

    /// Set Username
    async fn set_username(
        token: UserToken,
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<SetUsernameRequest>,
    ) -> Result<Json<UserData>> {
        users_service
            .set_username(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    /// Set Username
    async fn set_password(
        token: UserToken,
        State(users_service): State<UsersServiceHandle>,
        Json(body): Json<SetPasswordRequest>,
    ) -> Result<Json<UserData>> {
        users_service
            .set_password(&token._id.to_string(), body)
            .await
            .map(Json)
    }
}
