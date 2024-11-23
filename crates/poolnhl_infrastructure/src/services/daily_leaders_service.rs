use async_trait::async_trait;

use chrono::{Duration, Local, Timelike};
use mongodb::bson::doc;
use poolnhl_interface::errors::AppError;

use poolnhl_interface::daily_leaders::{model::DailyLeaders, service::DailyLeadersService};
use poolnhl_interface::errors::Result;

use crate::database_connection::DatabaseConnection;

#[derive(Clone)]
pub struct MongoDailyLeadersService {
    db: DatabaseConnection,
}

impl MongoDailyLeadersService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
#[async_trait]
impl DailyLeadersService for MongoDailyLeadersService {
    async fn get_daily_leaders(&self, date: &str) -> Result<DailyLeaders> {
        let collection = self.db.collection::<DailyLeaders>("day_leaders");

        let mut formatted_date = date.to_string();

        if date == "now" {
            let mut today = Local::now().date_naive();

            let time = Local::now().time();

            // Before 12PM fetch games of yesterday.

            if time.hour() < 12 {
                today -= Duration::days(1);
            }
            formatted_date = today.format("%Y-%m-%d").to_string();
        }

        let daily_leaders = collection
            .find_one(doc! {"date": &formatted_date}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        daily_leaders.ok_or_else(move || AppError::CustomError {
            msg: format!("no daily leaders found for the date: {}", date),
        })
    }
}
