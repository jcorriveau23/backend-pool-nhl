use mongodb::bson::{doc, Document};
use mongodb::Database;
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use futures::stream::TryStreamExt;

use crate::models::user::{User, RegisterRequest, WalletLoginRegisterRequest};

pub async fn find_user_with_name(
    db: &Database,
    _name: &String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection.find_one(doc! {"name": _name}, None).await.unwrap();

    Ok(user)
}

pub async fn find_user_with_address(
    db: &Database,
    _addr: &String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");

    let user = collection.find_one(doc! {"addr": _addr}, None).await.unwrap();

    Ok(user)
}

pub async fn find_users(
    db: &Database,
) -> mongodb::error::Result<Vec<User>> {
    let collection = db.collection::<User>("users");

    let mut cursor = collection.find(None, None).await?;

    let mut users: Vec<User> = vec![];

    while let Some(user) = cursor.try_next().await? {
        users.push(user);
    }

    Ok(users)
}

pub async fn create_user_from_login(db: &Database, user: &RegisterRequest, password_hash: &String) -> mongodb::error::Result<Option<User>>{
    // this function needd to be call after calling find_user() and validate a user does not exist
    let collection = db.collection::<Document>("users");

    let d = doc! {
        "name": user.name.clone(),
        "password": password_hash,
        "email": user.email.clone(),
        "phone": user.phone.clone()
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
        pool_list: None
    };

    Ok(Some(new_user))
}

pub async fn create_user_from_wallet_login(db: &Database, user: WalletLoginRegisterRequest) -> mongodb::error::Result<Option<User>>{
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
        pool_list: None
    };

    Ok(Some(new_user))
}

// TODO: use this function to let the user edit their name.
pub async fn update_user_name(
    db: &Database,
    _user_id: &String,
    _new_name: &String,
) -> mongodb::error::Result<Option<User>> {
    let collection = db.collection::<User>("users");
    let find_one_and_update_options = FindOneAndUpdateOptions::builder()
        .return_document(ReturnDocument::After)
        .build();

    let filter = doc!{"_id": _user_id};

    let doc = doc! {
        "name": _new_name,
    };

    let user = collection.find_one_and_update(filter, doc, find_one_and_update_options).await.unwrap();

    Ok(user)
}