use mongodb::bson::doc;
use mongodb::Database;

use crate::models::daily_leaders::DailyLeaders;

pub async fn find_daily_leaders(
    db: &Database,
    date: String,
) -> mongodb::error::Result<Option<DailyLeaders>> {
    let collection = db.collection::<DailyLeaders>("day_leaders");

    let daily_leaders_doc = collection
        .find_one(doc! {"date": date}, None)
        .await
        .unwrap();

    Ok(daily_leaders_doc)
}
