use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use poolnhl_interface::errors::AppError;
use serde::{Deserialize, Serialize};

use poolnhl_interface::errors::Result;
use poolnhl_interface::users::{model::UserData, service::UsersService};

use crate::database_connection::DatabaseConnection;

pub struct MongoUsersService {
    db: DatabaseConnection,
}

impl MongoUsersService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    pub _id: ObjectId,
    pub name: String,
    pub password: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub addr: Option<String>,   // Ethereum public address of user.
    pub pool_list: Vec<String>, // list of pool name this user participate in.
}

impl From<User> for UserData {
    fn from(user: User) -> Self {
        UserData {
            name: user.name,
            email: user.email,
            phone: user.phone,
            addr: user.addr,
            pool_list: user.pool_list,
        }
    }
}

#[async_trait]
impl UsersService for MongoUsersService {
    async fn get_user_by_name(&self, name: &str) -> Result<UserData> {
        let collection = self.db.collection::<User>("users");

        let user = collection
            .find_one(doc! {"name": name}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        user.ok_or_else(|| AppError::CustomError {
            msg: format!("No user found with name '{}'", name),
        })
        .map(|u| UserData {
            name: u.name,
            email: u.email,
            phone: u.phone,
            addr: u.addr,
            pool_list: u.pool_list,
        })
    }

    async fn list_all_users(&self) -> Result<Vec<UserData>> {
        let collection = self.db.collection::<User>("users");

        let cursor = collection
            .find(None, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        let users: Vec<User> = cursor
            .try_collect()
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        // Convert Vec<User> to Vec<UserData> using the Into trait
        Ok(users.into_iter().map(Into::into).collect())
    }
}
