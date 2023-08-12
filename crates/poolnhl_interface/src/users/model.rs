use serde::{Deserialize, Serialize};

// The user data model sent publicly.
#[derive(Serialize)]
pub struct UserData {
    pub _id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub addr: Option<String>,   // Ethereum public address of user.
    pub pool_list: Vec<String>, // list of pool name this user participate in.
}

// payload to register with name and password
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub password: String,
    pub email: String,
    pub phone: String,
}

// payload to register or login with a Ethereum wallet
#[derive(Deserialize)]
pub struct WalletLoginRegisterRequest {
    pub addr: String,
    pub sig: String,
}

// payload to login with username and password
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub name: String,
    pub password: String,
}

// payload to set a username.
#[derive(Deserialize)]
pub struct SetUsernameRequest {
    pub new_username: String,
}

// payload to set a password.
#[derive(Deserialize)]
pub struct SetPasswordRequest {
    pub password: String,
}

// Response provided for login request
#[derive(Serialize)]
pub struct LoginResponse {
    pub user: UserData,
    pub token: String,
}
