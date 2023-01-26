use mongodb::Database;

use rocket::State;

use crate::db::user;
use crate::errors::response::AppError;
use crate::routes::jwt::UserToken;

/// Get user by _name
//  http://127.0.0.1:8000/api-rust/user/_name
#[get("/user/<_name>")]
pub async fn get_user_by_name(db: &State<Database>, _name: String) -> Result<String, AppError> {
    user::find_user_with_name(db, &_name)
        .await
        .map(move |user| {
            let user_string = serde_json::to_string(&user).unwrap();
            user_string
        })
}

/// Get all users
//  http://127.0.0.1:8000/users
#[get("/users")]
pub async fn get_users(
    db: &State<Database>,
    token: Result<UserToken, AppError>,
) -> Result<String, AppError> {
    user::find_users(db)
        .await
        .map(|data| serde_json::to_string(&data).unwrap())
}
