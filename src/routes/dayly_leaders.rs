use mongodb::bson::doc;
//use mongodb::bson::oid::ObjectId;
use mongodb::Database;
//use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;

use crate::models::dayly_leaders::DaylyLeaders;
use crate::models::response::MessageResponse;

// use crate::request_guards::basic::ApiKey;

use crate::db::dayly_leaders;

use crate::errors::response::MyError;
/// get daylyLeaders document by _date
//  http://127.0.0.1:8000/dayly_leaders/2022-04-29
#[openapi(tag = "DaylyLeaders")]
#[get("/dayly_leaders/<_date>")]
pub async fn get_dayly_leaders_by_date(
    db: &State<Database>,
    _date: String,
) -> Result<Json<DaylyLeaders>, MyError> {

    match dayly_leaders::find_dayly_leaders(db, _date).await {
        Ok(data) => {
            if data.is_none() {
                println!("data is null");
                return Err(MyError::build(
                    400,
                    Some(format!("Data not found with date")),
                ));
            }
            println!("data is valid");
            Ok(Json(data.unwrap()))
        }
        Err(e) => {
            println!("{}", e);
            return Err(MyError::build(
                400, 
                Some(format!("Data not found with date")))
            );
        }
    }
}