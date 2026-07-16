use async_trait::async_trait;

use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::FindOptions;
use mongodb::Collection;
use poolnhl_interface::errors::AppError;

use poolnhl_interface::errors::Result;
use poolnhl_interface::players::{
    model::{GetPlayerQuery, PlayerInfo},
    service::PlayersService,
};

use crate::database_connection::DatabaseConnection;

#[derive(Clone)]
pub struct MongoPlayersService {
    collection: Collection<PlayerInfo>,
}

impl MongoPlayersService {
    pub fn new(db: DatabaseConnection) -> Self {
        let collection = db.collection::<PlayerInfo>("players");
        Self { collection }
    }
}

pub async fn get_player_with_id(
    collection: &Collection<PlayerInfo>,
    player_id: i64,
) -> Result<PlayerInfo> {
    // Moved this part of the function here cause it is being reused in draft-service,
    // not only in player-services.
    let filter = doc! {"id": player_id};

return  collection
        .find_one(filter, None)
        .await
        .map_err(|e| AppError::MongoError { msg: e.to_string() })?
        .ok_or_else(|| AppError::CustomError {
            msg: format!("Player with id {} not found", player_id),
        });
}

#[async_trait]
impl PlayersService for MongoPlayersService {
    async fn get_players(&self, params: GetPlayerQuery) -> Result<Vec<PlayerInfo>> {
        let mut filter = doc! {};
        if let Some(active) = params.active {
            filter.insert("active", active);
        }
        if let Some(positions) = params.positions {
            filter.insert("position", doc! { "$in": positions });
        }

        // Sorting options: default to sorting by `total_points` descending
        let sort_field = params.sort.unwrap_or_else(|| "salary_cap".to_string());
        let sort_value = if params.descending.unwrap_or(true) {
            -1
        } else {
            1
        };
        let sort_order = doc! { sort_field: sort_value, "_id": 1 };

        // Pagination: skip and limit
        let skip = params.skip.unwrap_or(0);
        let limit = params.limit.unwrap_or(20);

        let find_options = FindOptions::builder()
            .sort(sort_order)
            .skip(Some(skip))
            .limit(limit)
            .build();

        let players = self
            .collection
            .find(filter, find_options)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?
            .try_collect()
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        Ok(players)
    }

    async fn get_players_with_name(&self, name: &str) -> Result<Vec<PlayerInfo>> {
        let mut filter = doc! {};
        filter.insert("name", doc! { "$regex": name, "$options": "i" });
        let limit = 5;

        let find_options = FindOptions::builder().limit(limit).build();

        let players = self
            .collection
            .find(filter, find_options)
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?
            .try_collect()
            .await
            .map_err(|e| AppError::MongoError { msg: e.to_string() })?;

        Ok(players)
    }

    async fn get_player_with_id(&self, player_id: i64) -> Result<PlayerInfo> {
        get_player_with_id(&self.collection, player_id).await
    }
}
