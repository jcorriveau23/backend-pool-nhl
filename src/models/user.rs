use mongodb::bson::oid::ObjectId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive( Debug, Deserialize, Serialize )]
pub struct User {
    _id: ObjectId,
    pub name: String,
    pub password: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub addr: String,   // Ethereum public address of user.

    pub pool_list: Vec<String>, // list of pool name this user participate in.
}

// #[derive( Debug, Deserialize, Serialize )]
// pub struct UserDB {
//     _id: ObjectId,
//     child_members: User
// }

// #[derive( Debug, Deserialize, Serialize, JsonSchema )]
// pub struct UserJson {
//     _id: String,
//     child_members: User
// }

