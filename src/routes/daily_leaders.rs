use mongodb::bson::doc;
use mongodb::Database;

use rocket::serde::json::Json;
use rocket::State;

use crate::db::daily_leaders;
use crate::errors::response::AppError;
use crate::models::daily_leaders::DailyLeaders;

/// get dailyLeaders document by _date
//  http://127.0.0.1:8000/daily_leaders/2022-04-29
#[get("/daily_leaders/<_date>")]
pub async fn get_daily_leaders_by_date(
    db: &State<Database>,
    _date: String,
) -> Result<Json<DailyLeaders>, AppError> {
    match daily_leaders::find_daily_leaders(db, _date).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err(e),
    }
}
