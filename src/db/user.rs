use std::str::FromStr;

use futures::stream::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId, Document};
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::{Collection, Database};

use crate::models::user::{RegisterRequest, User, WalletLoginRegisterRequest};

pub async fn find_user_with_name(
    db: &Database,
    _name: &String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection.find_one(doc! {"name": _name}, None).await?;

    Ok(user)
}

pub async fn find_user_with_address(
    db: &Database,
    _addr: &String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection.find_one(doc! {"addr": _addr}, None).await?;

    Ok(user)
}

pub async fn find_users(db: &Database) -> mongodb::error::Result<Vec<User>> {
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

pub async fn create_user_from_login(
    db: &Database,
    user: &RegisterRequest,
    password_hash: &String,
) -> mongodb::error::Result<Option<User>> {
    // this function needd to be call after calling find_user() and validate a user does not exist
    let collection = db.collection::<Document>("users");

    let d = doc! {
        "name": user.name.clone(),
        "password": password_hash,
        "email": user.email.clone(),
        "phone": user.phone.clone(),
        "pool_list": [],
    };

    let insert_one_result = collection.insert_one(d, None).await?;

    // creating the data instead of find into the database.
    let new_user = User {
        _id: insert_one_result.inserted_id.as_object_id().unwrap(),
        name: user.name.clone(),
        password: Some(user.password.clone()),
        email: Some(user.email.clone()),
        phone: Some(user.phone.clone()),
        addr: None,
        pool_list: Vec::new(),
    };

    Ok(Some(new_user))
}

pub async fn create_user_from_wallet_login(
    db: &Database,
    user: WalletLoginRegisterRequest,
) -> mongodb::error::Result<Option<User>> {
    // this function needd to be call after calling find_user() and validate a user does not exist
    let collection = db.collection::<Document>("users");

    let d = doc! {
        "name": user.addr.clone(),
        "addr": user.addr.clone(),
        "pool_list": []
    };

    let insert_one_result = collection.insert_one(d, None).await?;

    // creating the data instead of find into the database.
    let new_user = User {
        _id: insert_one_result.inserted_id.as_object_id().unwrap(),
        name: user.addr.clone(),
        password: None,
        email: None,
        phone: None,
        addr: Some(user.addr.clone()),
        pool_list: Vec::new(),
    };

    Ok(Some(new_user))
}

pub async fn update_user_name(
    db: &Database,
    _user_id: &str,
    _new_name: &str,
) -> mongodb::error::Result<Option<User>> {
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

    Ok(user)
}

pub async fn update_password(
    db: &Database,
    _user_id: &str,
    _new_password: &String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");
    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let filter = doc! {"_id": ObjectId::from_str(_user_id).unwrap()};

    let updated_fields = doc! {
        "$set":  doc!{
            "password": _new_password
        }
    };

    let user = collection
        .find_one_and_update(filter, updated_fields, find_one_and_update_options)
        .await?;

    Ok(user)
}
