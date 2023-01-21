use crate::errors::response::AppError;
use crate::errors::response::Result;
use mongodb::bson::doc;
use mongodb::Database;

use crate::models::daily_leaders::DailyLeaders;

pub async fn find_daily_leaders(db: &Database, _date: String) -> Result<DailyLeaders> {
    let collection = db.collection::<DailyLeaders>("day_leaders");

    let daily_leaders = collection.find_one(doc! {"date": &_date}, None).await?;

    daily_leaders.ok_or(AppError::CustomError {
        msg: format!("no daily leaders found for the date: {}", _date),
        code: 500,
    })
}
