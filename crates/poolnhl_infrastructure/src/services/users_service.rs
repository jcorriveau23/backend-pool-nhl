use std::str::FromStr;

use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::bson::Document;
use mongodb::bson::{doc, oid::ObjectId};
use poolnhl_interface::errors::AppError;
use serde::{Deserialize, Serialize};

use poolnhl_interface::errors::Result;
use poolnhl_interface::users::{
    model::{
        LoginRequest, LoginResponse, RegisterRequest, SetPasswordRequest, SetUsernameRequest,
        UserData, WalletLoginRegisterRequest,
    },
    service::UsersService,
};

use crate::{database_connection::DatabaseConnection, jwt};

pub struct MongoUsersService {
    db: DatabaseConnection,
    secret: String,
}

impl MongoUsersService {
    pub fn new(db: DatabaseConnection, secret: String) -> Self {
        Self { db, secret }
    }

    async fn get_optional_raw_user_by_name(&self, name: &str) -> Result<Option<User>> {
        let collection = self.db.collection::<User>("users");

        collection
            .find_one(doc! {"name": name}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })
    }

    async fn get_optional_raw_user_by_address(&self, name: &str) -> Result<Option<User>> {
        let collection = self.db.collection::<User>("users");

        collection
            .find_one(doc! {"addr": name}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })
    }

    // Get the raw User data. This includes the password. It should never be return to the clients.
    async fn get_raw_user_by_name(&self, name: &str) -> Result<User> {
        let collection = self.db.collection::<User>("users");

        let user = self.get_optional_raw_user_by_name(name).await?;

        user.ok_or_else(|| AppError::CustomError {
            msg: format!("No user found with name '{}'", name),
        })
    }

    async fn verify_message(&self, addr: &str, sig: &str) -> Result<bool> {
        let message = "Unlock wallet to access nhl-pool-ethereum.";

        match sig.strip_prefix("0x") {
            Some(hex) => {
                let signer_addr = web3::signing::recover(
                    &self.eth_message(message).await,
                    &hex::decode(hex).map_err(|e| AppError::HexError { msg: e.to_string() })?[..64],
                    0,
                )
                .map_err(|e| AppError::RecoveryError { msg: e.to_string() })?;
                Ok(format!("{:02X?}", signer_addr) == *addr.to_lowercase())
            }
            None => Err(AppError::CustomError {
                msg: "Could not deserialize the signature provided".to_string(),
            }),
        }
    }

    async fn eth_message(&self, message: &str) -> [u8; 32] {
        web3::signing::keccak256(
            format!(
                "{}{}{}",
                "\x19Ethereum Signed Message:\n",
                message.len(),
                message
            )
            .as_bytes(),
        )
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
            _id: user._id.to_string(),
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
        // Get the information of 1 user with its name.

        self.get_raw_user_by_name(name)
            .await
            .map(|u| UserData::from(u))
    }

    async fn get_users_by_ids(&self, ids: &Vec<String>) -> Result<Vec<UserData>> {
        // Get the users informations of the list provided in the parameters.

        if ids.is_empty() {
            return Err(AppError::CustomError {
                msg: "The users list provided cannot be empty.".to_string(),
            });
        }

        let collection = self.db.collection::<User>("users");

        // if list of users is empty, send all users.

        let participants_object_id: Vec<ObjectId> = ids
            .iter()
            .map(|id| {
                ObjectId::from_str(id).expect("The user id list should all be valid at that point.")
            })
            .collect();

        // Only the users from the list provided will be retrieved.
        let filter = Some(doc! {"_id": {"$in": participants_object_id}});

        let cursor = collection
            .find(filter, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        let users: Vec<User> = cursor
            .try_collect()
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        // Convert Vec<User> to Vec<UserData> using the Into trait
        Ok(users.into_iter().map(Into::into).collect())
    }

    async fn login(&self, body: LoginRequest) -> Result<LoginResponse> {
        let user = self.get_raw_user_by_name(&body.name).await?;

        match &user.password {
            Some(psw) => {
                let is_valid_password = bcrypt::verify(&body.password, psw)
                    .map_err(|e| AppError::BcryptError { msg: e.to_string() })?;

                if !is_valid_password {
                    return Err(AppError::CustomError {
                        msg: "The password provided is not valid.".to_string(),
                    });
                }
            }
            None => {
                return Err(AppError::CustomError {
                    msg: "This account does not store any password.".to_string(),
                })
            }
        }

        Ok(LoginResponse {
            user: UserData::from(user),
            token: jwt::create(user, &self.secret)?,
        })
    }

    async fn register(&self, body: RegisterRequest) -> Result<LoginResponse> {
        let user = self.get_optional_raw_user_by_name(&body.name).await?;
        // the username provided is already registered.

        if user.is_some() {
            return Err(AppError::CustomError {
                msg: "this username is not available.".to_string(),
            });
        }
        let collection = self.db.collection::<Document>("users");

        // hash password before sending it to the function that create the document.

        let password_hash = bcrypt::hash(&body.password, 4)
            .map_err(|e| AppError::BcryptError { msg: e.to_string() })?;

        let doc = doc! {
            "name": body.name.clone(),
            "password": password_hash,
            "email": body.email.clone(),
            "phone": body.phone.clone(),
            "pool_list": [],
        };

        let insert_one_result = collection
            .insert_one(doc, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        match insert_one_result.inserted_id.as_object_id() {
            // creating the data instead of find into the database.
            Some(inserted_id) => {
                let user = User {
                    _id: inserted_id,
                    name: body.name.clone(),
                    password: Some(body.password.clone()),
                    email: Some(body.email.clone()),
                    phone: Some(body.phone.clone()),
                    addr: None,
                    pool_list: Vec::new(),
                };
                Ok(LoginResponse {
                    user: UserData::from(user),
                    token: jwt::create(user, &self.secret)?,
                })
            }
            None => Err(AppError::CustomError {
                msg: "The user could not be added to the data base.".to_string(),
            }),
        }
    }

    async fn wallet_login(&self, body: WalletLoginRegisterRequest) -> Result<LoginResponse> {
        let user = self.get_optional_raw_user_by_address(&body.addr).await?;
        let collection = self.db.collection::<Document>("users");

        if !self.verify_message(&body.addr, &body.sig).await? {
            return Err(AppError::CustomError {
                msg: "The signature provided is not valid.".to_string(),
            });
        }

        match user {
            Some(user) => Ok(LoginResponse {
                user: UserData::from(user),
                token: jwt::create(user, &self.secret)?,
            }),
            None => {
                // create the account if it does not exist
                let d = doc! {
                    "name": body.addr.clone(),
                    "addr": body.addr.clone(),
                    "pool_list": []
                };

                let insert_one_result = collection
                    .insert_one(d, None)
                    .await
                    .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

                match insert_one_result.inserted_id.as_object_id() {
                    // creating the data instead of find into the database.
                    Some(inserted_id) => {
                        let user = User {
                            _id: inserted_id,
                            name: body.addr.clone(),
                            password: None,
                            email: None,
                            phone: None,
                            addr: Some(body.addr.clone()),
                            pool_list: Vec::new(),
                        };
                        Ok(LoginResponse {
                            user: UserData::from(user),
                            token: jwt::create(user, &self.secret)?,
                        })
                    }
                    None => Err(AppError::CustomError {
                        msg: "The user could not be added to the data base.".to_string(),
                    }),
                }
            }
        }
    }
    // async fn set_username(&self, body: SetUsernameRequest) -> Result<UserData> {}
    // async fn set_password(&self, body: SetPasswordRequest) -> Result<UserData> {}
}
