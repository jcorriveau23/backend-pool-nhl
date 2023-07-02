use serde::Serialize;

#[derive(Serialize)]
pub struct UserData {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub addr: Option<String>,   // Ethereum public address of user.
    pub pool_list: Vec<String>, // list of pool name this user participate in.
}
