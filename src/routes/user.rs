use mongodb::bson::doc;
use mongodb::Database;

use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;

use crate::models::user::User;
use crate::db::user;
use crate::errors::response::MyError;

/// get user document by _name
//  http://127.0.0.1:8000/user/_name
#[openapi(tag = "User")]
#[get("/user/<_name>")]
pub async fn get_user_by_name(
    db: &State<Database>,
    _name: String,
) -> Result<String, MyError> {

    match user::find_user(db, _name).await {
        Ok(data) => {
            if data.is_none() {
                return Err(MyError::build(
                    400,
                    Some(format!("User not found with name")),
                ));
            }
            let string = serde_json::to_string(&data.unwrap()).unwrap();
            Ok(string)
        }
        Err(e) => {
            println!("{}", e);
            return Err(MyError::build(
                400, 
                Some(format!("User not found with name")))
            );
        }
    }
}

/// get all users
//  http://127.0.0.1:8000/users
#[openapi(tag = "User")]
#[get("/users")]
pub async fn get_users(
    db: &State<Database>
) -> Result<String, MyError> {

    match user::find_users(db).await {
        Ok(data) => {

            let string = serde_json::to_string(&data).unwrap();
            Ok(string)
        }
        Err(e) => {
            println!("{}", e);
            return Err(MyError::build(
                400, 
                Some(format!("User not found with name")))
            );
        }
    }
}