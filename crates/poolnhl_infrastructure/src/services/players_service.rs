use async_trait::async_trait;

use futures::TryStreamExt;
use mongodb::Collection;
use mongodb::bson::doc;
use mongodb::options::FindOptions;
use mongodb::{IndexModel, options::IndexOptions};
use poolnhl_interface::errors::AppError;

use poolnhl_interface::errors::Result;
use poolnhl_interface::players::{
    model::{GetPlayerQuery, PlayerInfo},
    service::PlayersService,
};

use crate::database_connection::DatabaseConnection;
use crate::database_connection::mongo_err;

#[derive(Clone)]
pub struct MongoPlayersService {
    collection: Collection<PlayerInfo>,
}

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;
const DEFAULT_SORT_FIELD: &str = "salary_cap";
const MAX_NAME_SEARCH_LEN: usize = 64;

// Fields a client may sort on. Anything else is rejected rather than passed
// through to mongo as a sort key.
const SORTABLE_FIELDS: [&str; 12] = [
    "salary_cap",
    "name",
    "age",
    "points",
    "goals",
    "assists",
    "game_played",
    "points_per_game",
    "save_percentage",
    "goal_against_average",
    "wins",
    "ot",
];

fn sortable_field(field: &str) -> Result<&'static str> {
    SORTABLE_FIELDS
        .into_iter()
        .find(|allowed| *allowed == field)
        .ok_or_else(|| AppError::CustomError {
            msg: format!(
                "'{field}' is not a sortable field. Allowed: {}.",
                SORTABLE_FIELDS.join(", ")
            ),
        })
}

// Turn a search term into a regex that matches it literally.
fn escape_regex(term: &str) -> String {
    let mut escaped = String::with_capacity(term.len());
    for c in term.chars() {
        if r"\^$.|?*+()[]{}".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
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
    let filter = doc! {"id": player_id};

    return collection
        .find_one(filter, None)
        .await
        .map_err(mongo_err)?
        .ok_or_else(|| AppError::CustomError {
            msg: format!("Player with id {} not found", player_id),
        });
}

#[async_trait]
impl PlayersService for MongoPlayersService {
    async fn init_indexes(&self) -> Result<()> {
        let sort_indexes = [
            doc! { "salary_cap": -1, "_id": 1 },
            doc! { "points": -1, "_id": 1 },
            doc! { "position": 1, "salary_cap": -1, "_id": 1 },
            doc! { "position": 1, "points": -1, "_id": 1 },
        ];

        for keys in sort_indexes {
            let index_model = IndexModel::builder()
                .keys(keys)
                .options(IndexOptions::builder().build())
                .build();

            self.collection
                .create_index(index_model, None)
                .await
                .map_err(mongo_err)?;
        }
        Ok(())
    }

    async fn get_players(&self, params: GetPlayerQuery) -> Result<Vec<PlayerInfo>> {
        let mut filter = doc! {};
        if let Some(active) = params.active {
            filter.insert("active", active);
        }
        if let Some(positions) = params.positions {
            filter.insert("position", doc! { "$in": positions });
        }

        // Sorting options: default to `salary_cap` descending. The field is
        // checked against the allow-list so a caller cannot force a sort on an
        // arbitrary (unindexed) field and turn every query into a collection
        // scan.
        let sort_field = match params.sort.as_deref() {
            None => DEFAULT_SORT_FIELD,
            Some(field) => sortable_field(field)?,
        };
        let sort_value = if params.descending.unwrap_or(true) {
            -1
        } else {
            1
        };
        let sort_order = doc! { sort_field: sort_value, "_id": 1 };

        // Pagination: skip, and a limit capped so one request cannot ask for the
        // whole collection.
        let skip = params.skip.unwrap_or(0);
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

        let find_options = FindOptions::builder()
            .sort(sort_order)
            .skip(Some(skip))
            .limit(limit)
            .build();

        let players = self
            .collection
            .find(filter, find_options)
            .await
            .map_err(mongo_err)?
            .try_collect()
            .await
            .map_err(mongo_err)?;

        Ok(players)
    }

    async fn get_players_with_name(&self, name: &str) -> Result<Vec<PlayerInfo>> {
        if name.len() > MAX_NAME_SEARCH_LEN {
            return Err(AppError::CustomError {
                msg: format!("A player search is limited to {MAX_NAME_SEARCH_LEN} characters."),
            });
        }

        // The search term is escaped before it reaches `$regex`: mongo runs
        // PCRE, so an unescaped term like `(a+)+$` would let any caller trigger
        // catastrophic backtracking on the server.
        let mut filter = doc! {};
        filter.insert(
            "name",
            doc! { "$regex": escape_regex(name), "$options": "i" },
        );
        let limit = 5;

        let find_options = FindOptions::builder().limit(limit).build();

        let players = self
            .collection
            .find(filter, find_options)
            .await
            .map_err(mongo_err)?
            .try_collect()
            .await
            .map_err(mongo_err)?;

        Ok(players)
    }

    async fn get_player_with_id(&self, player_id: i64) -> Result<PlayerInfo> {
        get_player_with_id(&self.collection, player_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_term_is_matched_literally() {
        // The classic catastrophic-backtracking pattern becomes inert.
        assert_eq!(escape_regex("(a+)+$"), r"\(a\+\)\+\$");
        // An ordinary name is untouched.
        assert_eq!(escape_regex("McDavid"), "McDavid");
    }

    #[test]
    fn only_allow_listed_sort_fields_are_accepted() {
        assert_eq!(sortable_field("points").unwrap(), "points");
        // The goalie table defaults to `wins` and also sorts on `ot`, so both
        // have to be reachable or the whole goalie view answers 400.
        assert_eq!(sortable_field("wins").unwrap(), "wins");
        assert_eq!(sortable_field("ot").unwrap(), "ot");
        assert!(matches!(
            sortable_field("$where"),
            Err(AppError::CustomError { .. })
        ));
    }
}
