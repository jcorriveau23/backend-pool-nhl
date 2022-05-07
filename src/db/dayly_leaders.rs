// use chrono::prelude::*;
use mongodb::bson::{doc};
use mongodb::Database;
//use rocket::serde::json::Json;


use crate::models::dayly_leaders::DaylyLeaders;

pub async fn find_dayly_leaders(
    db: &Database,
    date: String,
) -> mongodb::error::Result<Option<DaylyLeaders>> {
    let collection = db.collection::<DaylyLeaders>("day_leaders");

    let dayly_leaders_doc = collection.find_one(doc! {"date": date}, None).await.unwrap();

    Ok(dayly_leaders_doc)
}