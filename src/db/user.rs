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

    let user = collection
        .find_one(doc! {"name": _name}, None)
        .await
        .unwrap();

    Ok(user)
}

pub async fn find_user_with_address(
    db: &Database,
    _addr: &String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection
        .find_one(doc! {"addr": _addr}, None)
        .await
        .unwrap();

    Ok(user)
}

pub async fn find_users(db: &Database) -> mongodb::error::Result<Vec<User>> {
    let collection = db.collection::<User>("users");

    let mut cursor = collection.find(None, None).await?;

    let mut users: Vec<User> = vec![];

    while let Some(user) = cursor.try_next().await? {
        users.push(user);
    }

    Ok(users)
}

pub async fn find_users_with_ids(
    _collection: &Collection<User>,
    _user_ids: &Vec<String>,
) -> mongodb::error::Result<Vec<User>> {
    let mut cursor = _collection.find(None, None).await?;

    let mut users: Vec<User> = vec![];

    while let Some(user) = cursor.try_next().await? {
        if _user_ids.contains(&user._id.to_string()) {
            users.push(user);
        }
    }

    Ok(users)
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
    _user_id: &String,
    _new_name: &String,
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
        .await
        .unwrap();

    Ok(user)
}

pub async fn update_password(
    db: &Database,
    _user_id: &String,
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
        .await
        .unwrap();

    Ok(user)
}
