use axum::extract::{Json, Path, State};
use axum::routing::{get, post};
use axum::Router;

use poolnhl_interface::errors::Result;
use poolnhl_interface::users::{
    model::{LoginRequest, LoginResponse, UserData},
    service::UsersServiceHandle,
};

pub struct UsersRouter;

impl UsersRouter {
    pub fn new(users_service: UsersServiceHandle) -> Router {
        Router::new()
            .route("/user/:name", get(UsersRouter::get_user_by_name))
            .route("/users/:ids", get(UsersRouter::get_user_by_ids))
            .route("user/login", post(UsersRouter::login))
            .route("user/register", post(UsersRouter::register))
            .route("user/wallet-login", post(UsersRouter::wallet_login))
            .route("user/set-username", post(UsersRouter::set_username))
            .route("user/set-password", post(UsersRouter::set_password))
            .with_state(users_service)
    }

    pub async fn get_user_by_name(
        Path(name): Path<String>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<UserData>> {
        users_service.get_user_by_name(&name).await.map(Json)
    }

    pub async fn get_user_by_ids(
        Path(ids): Path<Vec<String>>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<Vec<UserData>>> {
        users_service.get_users_by_ids(&ids).await.map(Json)
    }

    pub async fn login(
        Json(body): Json<LoginRequest>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<LoginResponse>> {
        users_service.login(&body).await.map(Json)
    }

    pub async fn register(
        Json(body): Json<RegisterRequest>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<LoginResponse>> {
        users_service.register(&body).await.map(Json)
    }

    pub async fn wallet_login(
        Json(body): Json<WalletLoginRegisterRequest>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<LoginResponse>> {
        users_service.wallet_login(&body).await.map(Json)
    }

    /// Set Username
    pub async fn set_username(
        token: UserToken,
        Json(body): Json<SetUsernameRequest>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<UserData>> {
        users_service.set_username(&body).await.map(Json)
    }

    /// Set Username
    pub async fn set_password(
        token: UserToken,
        body: Json<SetPasswordRequest>,
        State(users_service): State<UsersServiceHandle>,
    ) -> Result<Json<UserData>> {
        users_service.set_password(&body).await.map(Json)
    }
}
