use crate::errors::response::AppError;
use crate::errors::response::Result;
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, Document};
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::{Collection, Database};
use std::str::FromStr;

use crate::models::user::{LoginRequest, RegisterRequest, User, WalletLoginRegisterRequest};

pub async fn find_optional_user_with_name(db: &Database, _name: &String) -> Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection.find_one(doc! {"name": _name}, None).await?;

    Ok(user)
}

pub async fn find_user_with_name(db: &Database, _name: &String) -> Result<User> {
    let user = find_optional_user_with_name(db, _name).await?;

    user.ok_or_else(move || AppError::CustomError {
        msg: format!("no user found with name {}", _name),
    })
}

pub async fn find_optional_user_with_address(
    db: &Database,
    _addr: &String,
) -> Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection.find_one(doc! {"addr": _addr}, None).await?;

    Ok(user)
}

pub async fn find_user_with_address(db: &Database, _addr: &String) -> Result<User> {
    let user = find_optional_user_with_address(db, _addr).await?;

    user.ok_or_else(move || AppError::CustomError {
        msg: format!("no user found with address {}", _addr),
    })
}

pub async fn find_users(db: &Database) -> Result<Vec<User>> {
    let collection = db.collection::<User>("users");

    let cursor = collection.find(None, None).await?;

    let users: Vec<User> = cursor.try_collect().await?;

    Ok(users)
}

pub async fn add_pool_to_users(
    _collection: &Collection<User>,
    _pool_name: &String,
    _user_ids: &Vec<String>,
) {
    // Add the new pool to the list of pool in each users.

    let participants_objectId: Vec<ObjectId> = _user_ids
        .iter()
        .map(|id| ObjectId::from_str(id).unwrap())
        .collect();

    let query = doc! {"_id": {"$in": participants_objectId}};

    let update = doc! {"$push": {"pool_list": _pool_name}}; // Add the name of the pool

    _collection.update_many(query, update, None).await;
}

pub async fn create_user_from_register(
    db: &Database,
    register_req: &RegisterRequest,
) -> Result<User> {
    // this function needd to be call after calling find_user() and validate a user does not exist
    let collection = db.collection::<Document>("users");

    let user = find_optional_user_with_name(db, &register_req.name).await?;

    // the username provided is already registered.

    if let Some(_) = user {
        return Err(AppError::CustomError {
            msg: "this username is not available.".to_string(),
        });
    }

    // hash password before sending it to the function that create the document.

    let password_hash = bcrypt::hash(&register_req.password, 4)?;

    let d = doc! {
        "name": register_req.name.clone(),
        "password": password_hash,
        "email": register_req.email.clone(),
        "phone": register_req.phone.clone(),
        "pool_list": [],
    };

    let insert_one_result = collection.insert_one(d, None).await?;

    // creating the data instead of find into the database.
    let new_user = User {
        _id: insert_one_result.inserted_id.as_object_id().unwrap(),
        name: register_req.name.clone(),
        password: Some(register_req.password.clone()),
        email: Some(register_req.email.clone()),
        phone: Some(register_req.phone.clone()),
        addr: None,
        pool_list: Vec::new(),
    };

    Ok(new_user)
}

pub async fn login(db: &Database, login_req: &LoginRequest) -> Result<User> {
    println!("Login from {}", login_req.name);

    let user = find_user_with_name(db, &login_req.name).await?;

    if user.password.is_none() {
        return Err(
            AppError::CustomError {
                msg: "This account does not store any password.".to_string(),
            }, // happens when someone loging with a wallet (no password stored)
        );
    }

    let psw = &user.password.as_ref().unwrap();
    let is_valid_password = bcrypt::verify(&login_req.password, psw)?;

    if !is_valid_password {
        return Err(AppError::CustomError {
            msg: "The password provided is not valid.".to_string(),
        });
    }

    Ok(user)
}

pub async fn wallet_login(
    db: &Database,
    wallet_login_req: &WalletLoginRegisterRequest,
) -> Result<User> {
    // this function needd to be call after calling find_user() and validate a user does not exist
    let collection = db.collection::<Document>("users");

    if !verify_message(&wallet_login_req.addr, &wallet_login_req.sig).await? {
        return Err(AppError::CustomError {
            msg: "The signature provided is not valid.".to_string(),
        });
    }

    let user = find_optional_user_with_address(db, &wallet_login_req.addr).await?;

    if user.is_none() {
        // create the account if it does not exist
        let d = doc! {
            "name": wallet_login_req.addr.clone(),
            "addr": wallet_login_req.addr.clone(),
            "pool_list": []
        };

        let insert_one_result = collection.insert_one(d, None).await?;

        // creating the data instead of find into the database.
        let new_user = User {
            _id: insert_one_result.inserted_id.as_object_id().unwrap(),
            name: wallet_login_req.addr.clone(),
            password: None,
            email: None,
            phone: None,
            addr: Some(wallet_login_req.addr.clone()),
            pool_list: Vec::new(),
        };

        return Ok(new_user);
    }

    Ok(user.unwrap())
}

pub async fn update_user_name(db: &Database, _user_id: &str, _new_name: &str) -> Result<User> {
    let collection = db.collection::<User>("users");

    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let filter = doc! {"_id": ObjectId::from_str(_user_id).unwrap()};

    let doc = doc! {
        "$set":  doc!{
            "name": _new_name
        }
    };

    let user = collection
        .find_one_and_update(filter, doc, find_one_and_update_options)
        .await?;

    user.ok_or_else(move || AppError::CustomError {
        msg: format!("no user found with id {}", _user_id),
    })
}

pub async fn update_password(db: &Database, _user_id: &str, _new_password: &str) -> Result<User> {
    let collection = db.collection::<User>("users");

    let password_hash = bcrypt::hash(_new_password.clone(), 4)?;

    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let filter = doc! {"_id": ObjectId::from_str(_user_id).unwrap()};

    let updated_fields = doc! {
        "$set":  doc!{
            "password": password_hash
        }
    };

    let user = collection
        .find_one_and_update(filter, updated_fields, find_one_and_update_options)
        .await?;

    user.ok_or(AppError::CustomError {
        msg: format!("no user found with id {}", _user_id),
    })
}

async fn verify_message(addr: &str, sig: &str) -> Result<bool> {
    let message = "Unlock wallet to access nhl-pool-ethereum.";

    let signature = hex::decode(sig.strip_prefix("0x").unwrap())?;
    let signer_addr = web3::signing::recover(&eth_message(message), &signature[..64], 0)?;

    Ok(format!("{:02X?}", signer_addr) == *addr.to_lowercase())
}

pub fn eth_message(message: &str) -> [u8; 32] {
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
