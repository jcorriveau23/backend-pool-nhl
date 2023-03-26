use mongodb::Database;

use rocket::State;

use crate::db::user;
use crate::errors::response::Result;
use rocket::serde::json::{json, Value};

/// Get user by _name
//  http://127.0.0.1:8000/api-rust/user/_name
#[get("/user/<_name>")]
pub async fn get_user_by_name(db: &State<Database>, _name: String) -> Result<Value> {
    user::find_user_with_name(db, &_name)
        .await
        .map(move |user| json!(&user))
}

/// Get all users
//  http://127.0.0.1:8000/users
#[get("/users")]
pub async fn get_users(db: &State<Database>) -> Result<Value> {
    user::find_users(db, &None)
        .await
        .map(move |users| json!(&users))
}

/// Get a specific list of users
//  http://127.0.0.1:8000/users/
#[get("/users?<names>")]
pub async fn get_users_with_id(db: &State<Database>, names: Vec<String>) -> Result<Value> {
    user::find_users(db, &Some(names))
        .await
        .map(move |users| json!(&users))
}
