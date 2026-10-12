use async_trait::async_trait;

use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use poolnhl_interface::errors::AppError;

use poolnhl_interface::clock::current_game_day;
use poolnhl_interface::daily_leaders::{model::DailyLeaders, service::DailyLeadersService};
use poolnhl_interface::errors::Result;

use crate::database_connection::DatabaseConnection;
use crate::database_connection::mongo_err;

#[derive(Clone)]
pub struct MongoDailyLeadersService {
    collection: Collection<DailyLeaders>,
}

impl MongoDailyLeadersService {
    pub fn new(db: DatabaseConnection) -> Self {
        let collection = db.collection::<DailyLeaders>("day_leaders");
        Self { collection }
    }
}
#[async_trait]
impl DailyLeadersService for MongoDailyLeadersService {
    async fn init_indexes(&self) -> Result<()> {
        let index_model = IndexModel::builder()
            .keys(doc! { "date": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.collection
            .create_index(index_model, None)
            .await
            .map_err(mongo_err)?;
        Ok(())
    }

    async fn get_daily_leaders(&self, date: &str) -> Result<DailyLeaders> {
        let mut formatted_date = date.to_string();

        if date == "now" {
            // Before 12PM fetch games of yesterday.
            formatted_date = current_game_day().format("%Y-%m-%d").to_string();
        }

        let daily_leaders = self
            .collection
            .find_one(doc! {"date": &formatted_date}, None)
            .await
            .map_err(mongo_err)?;

        daily_leaders.ok_or_else(move || AppError::NotFound {
            msg: format!("no daily leaders found for the date: {}", date),
        })
    }
}
