use mongodb::bson::doc;
use mongodb::Database;

use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;

use crate::models::daily_leaders::DailyLeaders;
use crate::db::daily_leaders;
use crate::errors::response::MyError;

/// get dailyLeaders document by _date
//  http://127.0.0.1:8000/daily_leaders/2022-04-29
#[openapi(tag = "DailyLeaders")]
#[get("/daily_leaders/<_date>")]
pub async fn get_daily_leaders_by_date(
    db: &State<Database>,
    _date: String,
) -> Result<Json<DailyLeaders>, MyError> {

    match daily_leaders::find_daily_leaders(db, _date).await {
        Ok(data) => {
            if data.is_none() {
                return Err(MyError::build(
                    400,
                    Some(format!("Data not found with date")),
                ));
            }
            
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