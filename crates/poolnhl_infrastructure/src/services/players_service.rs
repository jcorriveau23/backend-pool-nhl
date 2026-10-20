use async_trait::async_trait;

use futures::TryStreamExt;
use mongodb::Collection;
use mongodb::bson::{Bson, Document, doc, from_document};
use mongodb::options::FindOptions;
use mongodb::{IndexModel, options::IndexOptions};
use poolnhl_interface::errors::AppError;

use poolnhl_interface::errors::Result;
use poolnhl_interface::players::{
    model::{GetPlayerQuery, PlayerInfo, resolve_stats_season},
    service::PlayersService,
};

use crate::database_connection::DatabaseConnection;
use crate::database_connection::mongo_err;

#[derive(Clone)]
pub struct MongoPlayersService {
    collection: Collection<PlayerInfo>,
    season_stats: Collection<Document>,
}

// One document per (player, season), written by the scraper's cumulator. Only
// the counting stats live here; who the player is -- team, age, cap hit,
// contract -- stays on the single `players` document and is always current.
const PLAYER_SEASON_STATS: &str = "player_season_stats";

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

// Every stat field a `players` document carries. A past-season read blanks
// these before merging that season's row in, so a player with no row for the
// season asked for reads as "did not play" rather than carrying this season's
// numbers under last season's heading.
const STAT_FIELDS: [&str; 11] = [
    "game_played",
    "goals",
    "assists",
    "points",
    "points_per_game",
    "goal_against_average",
    "save_percentage",
    "saves",
    "shots",
    "wins",
    "ot",
];

fn blank_stats() -> Document {
    STAT_FIELDS
        .into_iter()
        .map(|field| (field.to_string(), Bson::Null))
        .collect()
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
        let season_stats = db.collection::<Document>(PLAYER_SEASON_STATS);
        Self {
            collection,
            season_stats,
        }
    }

    /// Read players with the stats recorded for `season` instead of the
    /// current ones.
    ///
    /// Identity is deliberately not swapped: a draft held after opening day
    /// wants each player's team, cap hit and contract as they are today, with
    /// the numbers he put up last season. Only the stat fields change.
    ///
    /// `players` is the collection driving the pipeline rather than
    /// `player_season_stats` so that the position and `active` filters, and a
    /// sort on an identity field like `salary_cap`, keep working on one
    /// document per player.
    async fn find_for_season(
        &self,
        filter: Document,
        season: u32,
        sort: Option<Document>,
        skip: u64,
        limit: i64,
    ) -> Result<Vec<PlayerInfo>> {
        let mut pipeline = vec![
            doc! { "$match": filter },
            doc! { "$lookup": {
                "from": PLAYER_SEASON_STATS,
                "let": { "player_id": "$id" },
                "pipeline": [
                    { "$match": { "$expr": { "$and": [
                        { "$eq": ["$id", "$$player_id"] },
                        { "$eq": ["$season", season as i64] },
                    ] } } },
                    // Left in, `_id`, `id` and `season` would be merged over
                    // the player's own fields below.
                    { "$project": { "_id": 0, "id": 0, "season": 0 } },
                ],
                "as": "season_stats",
            } },
            doc! { "$replaceWith": { "$mergeObjects": [
                "$$ROOT",
                blank_stats(),
                { "$ifNull": [{ "$first": "$season_stats" }, {}] },
            ] } },
            doc! { "$unset": "season_stats" },
        ];

        // Sorting after the merge, so a sort on `points` orders by the season
        // asked for and not by the current one.
        if let Some(sort) = sort {
            pipeline.push(doc! { "$sort": sort });
        }
        pipeline.push(doc! { "$skip": skip as i64 });
        pipeline.push(doc! { "$limit": limit });

        let mut cursor = self
            .collection
            .aggregate(pipeline, None)
            .await
            .map_err(mongo_err)?;

        let mut players = Vec::new();
        while let Some(document) = cursor.try_next().await.map_err(mongo_err)? {
            players.push(
                from_document(document).map_err(|e| AppError::BsonError { msg: e.to_string() })?,
            );
        }

        Ok(players)
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

        // One row per player per season, which is also the index the
        // past-season lookup matches on. Unique so a cumulator run that is
        // interrupted and restarted updates its rows instead of doubling them.
        let season_stats_index = IndexModel::builder()
            .keys(doc! { "id": 1, "season": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();

        self.season_stats
            .create_index(season_stats_index, None)
            .await
            .map_err(mongo_err)?;

        Ok(())
    }

    async fn get_players(&self, params: GetPlayerQuery) -> Result<Vec<PlayerInfo>> {
        // Rejects an unknown season before any of the work below.
        let stats_season = resolve_stats_season(params.season)?;

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

        if let Some(season) = stats_season {
            return self
                .find_for_season(filter, season, Some(sort_order), skip, limit)
                .await;
        }

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

    async fn get_players_with_name(
        &self,
        name: &str,
        season: Option<u32>,
    ) -> Result<Vec<PlayerInfo>> {
        let stats_season = resolve_stats_season(season)?;

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
        let limit = 10;

        if let Some(season) = stats_season {
            // Unsorted, like the plain read below: the five matches of a name
            // search are ordered by whatever the collection gives back.
            return self.find_for_season(filter, season, None, 0, limit).await;
        }

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
