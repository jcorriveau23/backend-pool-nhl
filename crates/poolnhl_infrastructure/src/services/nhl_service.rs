use async_trait::async_trait;

use mongodb::bson::doc;
use poolnhl_interface::errors::AppError;

use poolnhl_interface::errors::Result;
use poolnhl_interface::nhl::{
    model::{DailyGames, GameBoxScore, GameLanding},
    service::NhlService,
};

use crate::database_connection::DatabaseConnection;

#[derive(Clone)]
pub struct MongoNhlService {
    db: DatabaseConnection,
}

impl MongoNhlService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
#[async_trait]
impl NhlService for MongoNhlService {
    async fn get_daily_games(&self, date: &str) -> Result<DailyGames> {
        let collection = self.db.collection::<DailyGames>("daily_games");

        let daily_games = collection
            .find_one(doc! {"date": &date}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        daily_games.ok_or_else(move || AppError::CustomError {
            msg: format!("no daily games found with the date: {}", date),
        })
    }
    async fn get_game_landing(&self, id: u32) -> Result<GameLanding> {
        let collection = self.db.collection::<GameLanding>("games");

        let game_landing = collection
            .find_one(doc! {"id": id}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        game_landing.ok_or_else(move || AppError::CustomError {
            msg: format!("no game landing found with id: {}", id),
        })
    }

    async fn get_game_box_score(&self, id: u32) -> Result<GameBoxScore> {
        let collection = self.db.collection::<GameBoxScore>("boxscores");

        let game_box_score = collection
            .find_one(doc! {"id": id}, None)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        game_box_score.ok_or_else(move || AppError::CustomError {
            msg: format!("no game boxscore found with id: {}", id),
        })
    }
}
