use mongodb::Database;

use rocket::State;

use crate::db::user;
use crate::errors::response::ResponseError;
use crate::routes::jwt::{return_token_error, ApiKeyError, UserToken};

/// Get user by _name
//  http://127.0.0.1:8000/api-rust/user/_name
#[get("/user/<_name>")]
pub async fn get_user_by_name(
    db: &State<Database>,
    _name: String,
) -> Result<String, ResponseError> {
    match user::find_user_with_name(db, &_name).await {
        Ok(data) => {
            let user_string = serde_json::to_string(&data).unwrap();
            Ok(user_string)
        }
        Err(e) => Err(ResponseError::build(400, Some(e.to_string()))),
    }
}

/// Get all users
//  http://127.0.0.1:8000/users
#[get("/users")]
pub async fn get_users(
    db: &State<Database>,
    token: Result<UserToken, ApiKeyError>,
) -> Result<String, ResponseError> {
    if let Err(e) = token {
        return Err(return_token_error(e));
    }

    match user::find_users(db).await {
        Ok(data) => {
            let string = serde_json::to_string(&data).unwrap();
            Ok(string)
        }
        Err(e) => Err(ResponseError::build(400, Some(e.to_string()))),
    }
}
