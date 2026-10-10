//! The survivor pool, on MongoDB.
//!
//! Two collections, and the split between them is what lets a pool hold
//! hundreds of people:
//!
//! - `survivor_pools` holds the pool — settings, participants, weeks. It is
//!   read on every request and written only when the pool itself changes:
//!   somebody joins, a date settles. Writes take the same optimistic lock on
//!   `date_updated` the roster pool uses.
//! - `survivor_picks` holds one document per (pool, participant, week). A pick
//!   is an insert that touches nothing anybody else is writing, so a Saturday
//!   morning of hundreds of concurrent picks is hundreds of independent inserts
//!   rather than hundreds of contenders for one document's lock.
//!
//! The two rules of the game are unique indexes on that second collection, not
//! checks in this file. A check here would be read-then-write, and two picks
//! racing each other would both pass it; an index cannot be raced.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::Utc;
use futures::stream::TryStreamExt;
use mongodb::bson::{Document, doc, to_bson};
use mongodb::options::{FindOneAndUpdateOptions, IndexOptions, ReturnDocument};
use mongodb::{Collection, IndexModel};
use uuid::Uuid;

use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::survivor::model::{
    PickOutcome, RevealedPick, SurvivorPool, SurvivorPoolShort, SurvivorStandingRow,
    SurvivorStandings, SurvivorState, UsedTeam, WeekStatus, available_team_ids, is_blocked,
    plan_pick,
};
use poolnhl_interface::survivor::picks::{SurvivorPick, SurvivorPickOptions, SurvivorPickView};
use poolnhl_interface::survivor::requests::{
    AddSurvivorParticipantRequest, JoinSurvivorRequest, LeaveSurvivorRequest, MakePickRequest,
    SettleWeekRequest, SurvivorCreationRequest, SurvivorDeletionRequest,
    UpdateSurvivorSettingsRequest,
};
use poolnhl_interface::survivor::service::SurvivorService;

use crate::database_connection::DatabaseConnection;
use crate::database_connection::{bson_err, mongo_err};
use crate::services::nhl_schedule_service::{NhlScheduleService, TeamResult};

/// Mongo's duplicate-key error. A pick that trips a unique index is a rule being
/// enforced, not a failure, so it is turned into the message the participant
/// should read rather than a 500.
const DUPLICATE_KEY: i32 = 11000;

#[derive(Clone)]
pub struct MongoSurvivorService {
    pools: Collection<SurvivorPool>,
    picks: Collection<SurvivorPick>,
    schedule: NhlScheduleService,
}

impl MongoSurvivorService {
    pub fn new(db: DatabaseConnection, schedule: NhlScheduleService) -> Self {
        Self {
            pools: db.collection::<SurvivorPool>("survivor_pools"),
            picks: db.collection::<SurvivorPick>("survivor_picks"),
            schedule,
        }
    }

    async fn pool_by_name(&self, name: &str) -> Result<SurvivorPool> {
        self.pools
            .find_one(doc! {"name": name}, None)
            .await
            .map_err(mongo_err)?
            .ok_or_else(|| AppError::NotFound {
                msg: format!("no survivor pool found with name '{name}'"),
            })
    }

    /// Apply `update` to the pool only if it has not changed since it was read
    /// at `expected_version`.
    ///
    /// The same optimistic lock the roster pool uses. It guards the pool
    /// document only — a pick never comes through here, which is the point.
    async fn update_pool(
        &self,
        mut update: Document,
        pool_name: &str,
        expected_version: i64,
    ) -> Result<SurvivorPool> {
        update
            .get_document_mut("$set")
            .map_err(|e| AppError::BsonError {
                msg: format!("a survivor pool update must carry a `$set` document: {e}"),
            })?
            .insert("date_updated", next_version(expected_version));

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        let updated = self
            .pools
            .find_one_and_update(
                doc! {"name": pool_name, "date_updated": expected_version},
                update,
                options,
            )
            .await
            .map_err(mongo_err)?;

        match updated {
            Some(pool) => Ok(pool),
            // The filter matched nothing: either the pool is gone, or its
            // version moved on. Tell those apart so the client gets 404 vs 409.
            None => match self
                .pools
                .find_one(doc! {"name": pool_name}, None)
                .await
                .map_err(mongo_err)?
            {
                Some(_) => Err(AppError::ConflictError {
                    msg: "This pool was modified by someone else while you were editing it. \
                          Refresh and try again."
                        .to_string(),
                }),
                None => Err(AppError::NotFound {
                    msg: format!("no survivor pool found with name '{pool_name}'"),
                }),
            },
        }
    }

    /// Every pick a participant has made in a pool.
    ///
    /// At most one per date, so this is a few dozen small documents off a
    /// covered index — which is why the used-team rule can be decided per
    /// request without the pool carrying a copy of it.
    async fn participant_picks(
        &self,
        pool_name: &str,
        participant_id: &str,
    ) -> Result<Vec<SurvivorPick>> {
        let cursor = self
            .picks
            .find(
                doc! {"pool_name": pool_name, "participant_id": participant_id},
                None,
            )
            .await
            .map_err(mongo_err)?;

        cursor.try_collect().await.map_err(mongo_err)
    }

    async fn week_picks(&self, pool_name: &str, week: u16) -> Result<Vec<SurvivorPick>> {
        let cursor = self
            .picks
            .find(doc! {"pool_name": pool_name, "week": week as i32}, None)
            .await
            .map_err(mongo_err)?;

        cursor.try_collect().await.map_err(mongo_err)
    }

    /// The teams playing on a week's date, reading them from the league the
    /// first time and keeping them on the week after that.
    ///
    /// A past Saturday's schedule never changes, and a settlement must not
    /// depend on the league's API being reachable months later, so the list is
    /// written back onto the pool once known.
    async fn eligible_teams(&self, pool: &SurvivorPool, week: u16) -> Result<Vec<u32>> {
        let week_entry = pool.week(week)?;

        // Once a date has been locked or settled its list is on the pool, and
        // that copy is the authority: a settlement months later must not depend
        // on the league's feed still being reachable, or still agreeing.
        if !week_entry.eligible_team_ids.is_empty() {
            return Ok(week_entry.eligible_team_ids.clone());
        }

        // Otherwise from the league, through the schedule service's own cache.
        // Deliberately without writing it back to the pool: this is the read
        // path, and hundreds of people opening the pick screen on the same
        // morning would be hundreds of writers contending for one document's
        // optimistic lock, almost all of them losing. `lock_week` persists the
        // list instead, once, as the owner's action.
        self.schedule.teams_playing_on(&week_entry.pick_date).await
    }

    /// Build the pick screen's data for a participant.
    async fn pick_options(
        &self,
        pool: &SurvivorPool,
        user_id: &str,
        week: u16,
    ) -> Result<SurvivorPickOptions> {
        let eligible = self.eligible_teams(pool, week).await?;
        self.pick_options_from(pool, user_id, week, eligible).await
    }

    /// The pick screen, built from a fixture list already in hand.
    ///
    /// `make_pick` resolves the teams to validate against and then answers with
    /// the screen; going through `pick_options` would resolve them a second
    /// time.
    async fn pick_options_from(
        &self,
        pool: &SurvivorPool,
        user_id: &str,
        week: u16,
        eligible: Vec<u32>,
    ) -> Result<SurvivorPickOptions> {
        let week_entry = pool.week(week)?;

        let picks = self.participant_picks(&pool.name, user_id).await?;
        let used: Vec<UsedTeam> = picks.iter().map(SurvivorPick::used_team).collect();

        let league_team_count = pool.settings.league_team_count;
        let current_pick = picks
            .iter()
            .find(|pick| pick.week == week)
            .map(|pick| pick.team_id);

        Ok(SurvivorPickOptions {
            week,
            pick_date: week_entry.pick_date.clone(),
            available_team_ids: available_team_ids(&used, &eligible, league_team_count),
            used_team_ids: used.iter().map(|team| team.team_id).collect(),
            is_blocked: is_blocked(&used, &eligible, league_team_count),
            can_pick: pool.validate_can_pick(user_id, week).is_ok(),
            eligible_team_ids: eligible,
            current_pick,
        })
    }
}

/// The next value of a pool's `date_updated` version stamp.
///
/// A wall-clock millisecond timestamp, forced to strictly increase so two
/// writes landing in the same millisecond still produce distinct versions.
fn next_version(current_version: i64) -> i64 {
    Utc::now().timestamp_millis().max(current_version + 1)
}

/// The position of a week in the pool's `weeks` array, for a positional `$set`.
fn week_index(pool: &SurvivorPool, week: u16) -> Result<usize> {
    pool.weeks
        .iter()
        .position(|candidate| candidate.week == week)
        .ok_or_else(|| AppError::NotFound {
            msg: format!("'{}' has no week {week}.", pool.name),
        })
}

/// Whether a Mongo error is a unique index rejecting a write.
///
/// The index is how the rules of the game are enforced, so hitting one is an
/// expected outcome with a message for the participant — not a 500.
fn is_duplicate_key(error: &mongodb::error::Error) -> bool {
    use mongodb::error::{ErrorKind, WriteFailure};

    match error.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(write_error)) => {
            write_error.code == DUPLICATE_KEY
        }
        ErrorKind::BulkWrite(bulk) => bulk
            .write_errors
            .as_ref()
            .is_some_and(|errors| errors.iter().any(|e| e.code == DUPLICATE_KEY)),
        _ => false,
    }
}

#[async_trait]
impl SurvivorService for MongoSurvivorService {
    async fn init_indexes(&self) -> Result<()> {
        let unique = || IndexOptions::builder().unique(true).build();

        self.pools
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"name": 1})
                    .options(unique())
                    .build(),
                None,
            )
            .await
            .map_err(mongo_err)?;

        // Not unique: many pools share a season. Covers the pool listing.
        self.pools
            .create_index(IndexModel::builder().keys(doc! {"season": 1}).build(), None)
            .await
            .map_err(mongo_err)?;

        // One pick per participant per date. This is what makes a double submit
        // — two taps, two tabs, a retried request — impossible to turn into two
        // picks, without this file having to read before it writes.
        self.picks
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"pool_name": 1, "participant_id": 1, "week": 1})
                    .options(unique())
                    .build(),
                None,
            )
            .await
            .map_err(mongo_err)?;

        // A team a participant has used is closed to them for the rest of that
        // pass through the league. `cycle` is in the key so the whole set
        // reopens when they have been through every team, rather than the index
        // having to be torn down.
        self.picks
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"pool_name": 1, "participant_id": 1, "cycle": 1, "team_id": 1})
                    .options(unique())
                    .build(),
                None,
            )
            .await
            .map_err(mongo_err)?;

        // The reveal and the settlement both read a whole date at once.
        self.picks
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"pool_name": 1, "week": 1})
                    .build(),
                None,
            )
            .await
            .map_err(mongo_err)?;

        Ok(())
    }

    async fn get_pool_by_name(&self, name: &str) -> Result<SurvivorPool> {
        self.pool_by_name(name).await
    }

    async fn list_pools(&self, season: u32) -> Result<Vec<SurvivorPoolShort>> {
        // Counted by mongo with `$size` rather than by reading the participant
        // arrays back: a listing of twenty pools of three hundred people would
        // otherwise put six thousand participant objects on the wire to arrive
        // at twenty numbers.
        //
        // It is also why this is an aggregation and not a `find` with a
        // projection. A projection narrow enough to be worth doing strips the
        // settings fields `SurvivorSettings` requires, and the document then
        // fails to deserialize.
        let pipeline = vec![
            doc! {"$match": {"season": season}},
            doc! {"$project": {
                "_id": 0,
                "name": 1,
                "owner": 1,
                "status": 1,
                "season": 1,
                "participant_count": {"$size": {"$ifNull": ["$participants", []]}},
                "max_participants": "$settings.max_participants",
            }},
        ];

        let cursor = self
            .pools
            .aggregate(pipeline, None)
            .await
            .map_err(mongo_err)?;

        let documents: Vec<Document> = cursor.try_collect().await.map_err(mongo_err)?;

        documents
            .into_iter()
            .map(|document| {
                mongodb::bson::from_document::<SurvivorPoolShort>(document).map_err(|e| {
                    AppError::BsonError {
                        msg: format!("a survivor pool listing row did not deserialize: {e}"),
                    }
                })
            })
            .collect()
    }

    async fn create_pool(
        &self,
        user_id: &str,
        req: SurvivorCreationRequest,
    ) -> Result<SurvivorPool> {
        let season_info = poolnhl_interface::pool::model::SeasonInfo::current();

        let mut pool = SurvivorPool::new(
            &req.pool_name,
            user_id,
            &req.settings,
            season_info.season,
            &season_info.start_season_date,
            &season_info.end_season_date,
        )?;

        // The owner is in their own pool from the start: one who was not would
        // have no pick to make in the pool they run.
        pool.add_participant(
            user_id,
            &req.participant_name,
            Utc::now().timestamp_millis(),
        )?;

        self.pools.insert_one(&pool, None).await.map_err(|e| {
            if is_duplicate_key(&e) {
                return AppError::CustomError {
                    msg: format!("A pool named '{}' already exists.", req.pool_name),
                };
            }
            mongo_err(e)
        })?;

        Ok(pool)
    }

    async fn delete_pool(&self, user_id: &str, req: SurvivorDeletionRequest) -> Result<()> {
        let pool = self.pool_by_name(&req.pool_name).await?;
        pool.validate_owner_rights(user_id)?;

        self.pools
            .delete_one(doc! {"name": &req.pool_name}, None)
            .await
            .map_err(mongo_err)?;

        // The picks are their own documents, so deleting the pool leaves them
        // behind unless they go too.
        self.picks
            .delete_many(doc! {"pool_name": &req.pool_name}, None)
            .await
            .map_err(mongo_err)?;

        Ok(())
    }

    async fn update_settings(
        &self,
        user_id: &str,
        req: UpdateSurvivorSettingsRequest,
    ) -> Result<SurvivorPool> {
        let pool = self.pool_by_name(&req.pool_name).await?;
        pool.validate_owner_rights(user_id)?;
        req.settings.validate()?;

        // Lowering the cap below the people already in would leave the pool
        // over its own maximum.
        if (req.settings.max_participants as usize) < pool.participants.len() {
            return Err(AppError::CustomError {
                msg: format!(
                    "'{}' already has {} participants.",
                    pool.name,
                    pool.participants.len()
                ),
            });
        }

        // Changing what a pick costs once results are in would rewrite history:
        // the strikes already recorded were counted under the old rules.
        let has_settled = pool
            .weeks
            .iter()
            .any(|week| matches!(week.status, WeekStatus::Settled));

        if has_settled
            && (req.settings.strikes_allowed != pool.settings.strikes_allowed
                || req.settings.missed_pick_is_strike != pool.settings.missed_pick_is_strike
                || req.settings.league_team_count != pool.settings.league_team_count)
        {
            return Err(AppError::CustomError {
                msg: "The pool has started; how a pick is scored can no longer change.".to_string(),
            });
        }

        self.update_pool(
            doc! {"$set": doc!{"settings": to_bson(&req.settings).map_err(bson_err)?}},
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn join_pool(&self, user_id: &str, req: JoinSurvivorRequest) -> Result<SurvivorPool> {
        let mut pool = self.pool_by_name(&req.pool_name).await?;

        pool.add_participant(
            user_id,
            &req.participant_name,
            Utc::now().timestamp_millis(),
        )?;

        // The whole list is written back rather than `$push`ed: the checks above
        // (the cap, the name nobody else has) read it, so the write has to be
        // the one that lock covers. Hundreds of joins are spread over the days
        // before the pool starts, unlike the picks, so this is not a hot path.
        self.update_pool(
            doc! {"$set": doc!{"participants": to_bson(&pool.participants).map_err(bson_err)?}},
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn add_participant(
        &self,
        user_id: &str,
        req: AddSurvivorParticipantRequest,
    ) -> Result<SurvivorPool> {
        let mut pool = self.pool_by_name(&req.pool_name).await?;

        // Generated here, never taken from the request: an id the organiser
        // could name would let them attach a pool to somebody else's account.
        let id = Uuid::new_v4().to_string();

        pool.add_managed_participant(
            user_id,
            &id,
            &req.participant_name,
            Utc::now().timestamp_millis(),
        )?;

        self.update_pool(
            doc! {"$set": doc!{"participants": to_bson(&pool.participants).map_err(bson_err)?}},
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn leave_pool(&self, user_id: &str, req: LeaveSurvivorRequest) -> Result<SurvivorPool> {
        let mut pool = self.pool_by_name(&req.pool_name).await?;

        pool.remove_participant(user_id, &req.participant_id)?;

        let updated = self
            .update_pool(
                doc! {"$set": doc!{"participants": to_bson(&pool.participants).map_err(bson_err)?}},
                &req.pool_name,
                pool.date_updated,
            )
            .await?;

        // Their picks go with them, which also reopens the teams they had spent
        // should they rejoin before the pool starts.
        self.picks
            .delete_many(
                doc! {"pool_name": &req.pool_name, "participant_id": &req.participant_id},
                None,
            )
            .await
            .map_err(mongo_err)?;

        Ok(updated)
    }

    async fn get_pick_options(
        &self,
        user_id: &str,
        pool_name: &str,
        week: u16,
        participant_id: Option<&str>,
    ) -> Result<SurvivorPickOptions> {
        let pool = self.pool_by_name(pool_name).await?;
        let target = participant_id.unwrap_or(user_id);

        // Same rule as filing the pick, and for a sharper reason: this screen
        // carries `current_pick`, so letting anybody read anybody's would hand
        // them the field's picks before the date locks.
        if target != user_id && !pool.has_assistant_rights(user_id) {
            return Err(AppError::ForbiddenError {
                msg: format!(
                    "Only the owner of '{}' can see somebody else's picks.",
                    pool.name
                ),
            });
        }

        self.pick_options(&pool, target, week).await
    }

    async fn make_pick(&self, user_id: &str, req: MakePickRequest) -> Result<SurvivorPickOptions> {
        let pool = self.pool_by_name(&req.pool_name).await?;

        // Whose pick this is. A participant sends nothing and files their own;
        // the organiser names one of the spots they keep on somebody's behalf,
        // which have no account to file with.
        let participant_id = req.participant_id.as_deref().unwrap_or(user_id);

        pool.validate_can_pick_for(user_id, participant_id, req.week)?;

        let eligible = self.eligible_teams(&pool, req.week).await?;
        let existing = self.participant_picks(&pool.name, participant_id).await?;

        let current = existing.iter().find(|pick| pick.week == req.week);

        if let Some(current) = current {
            if !pool.settings.allow_pick_change {
                return Err(AppError::CustomError {
                    msg: "This pool does not allow a pick to be changed.".to_string(),
                });
            }
            if current.team_id == req.team_id {
                // Already the pick on file; nothing to write.
                return self
                    .pick_options_from(&pool, participant_id, req.week, eligible)
                    .await;
            }
        }

        // The team is weighed against every pick but the one being replaced —
        // otherwise changing a pick would see its own team as already spent.
        let used: Vec<UsedTeam> = existing
            .iter()
            .filter(|pick| pick.week != req.week)
            .map(SurvivorPick::used_team)
            .collect();

        let cycle = plan_pick(
            &used,
            &eligible,
            pool.settings.league_team_count,
            req.team_id,
        )?;

        let now = Utc::now().timestamp_millis();

        // Upsert on the (pool, participant, week) key the unique index covers,
        // so a change replaces the pick and a first pick creates it — one round
        // trip, and a double submit cannot produce two documents. The key
        // fields themselves come from the filter, which mongo applies to the
        // document it inserts.
        let mut set = doc! {
            "cycle": cycle as i32,
            "team_id": req.team_id,
            "outcome": to_bson(&PickOutcome::Pending).map_err(bson_err)?,
        };
        // `date_picked` is left to `$setOnInsert` alone: naming a path in both
        // operators is a conflict mongo refuses outright.
        if current.is_some() {
            set.insert("date_modified", now);
        }

        self.picks
            .find_one_and_update(
                doc! {
                    "pool_name": &pool.name,
                    "participant_id": participant_id,
                    "week": req.week as i32,
                },
                doc! {"$set": set, "$setOnInsert": doc!{"date_picked": now}},
                FindOneAndUpdateOptions::builder()
                    .upsert(true)
                    .return_document(ReturnDocument::After)
                    .build(),
            )
            .await
            .map_err(|e| {
                // The other unique index: somebody's concurrent pick took this
                // team first. A rule, not a fault.
                if is_duplicate_key(&e) {
                    return AppError::ConflictError {
                        msg: "You have already used that team. Refresh and pick another."
                            .to_string(),
                    };
                }
                mongo_err(e)
            })?;

        self.pick_options_from(&pool, participant_id, req.week, eligible)
            .await
    }

    async fn get_week_picks(
        &self,
        user_id: &str,
        pool_name: &str,
        week: u16,
    ) -> Result<Vec<SurvivorPickView>> {
        let pool = self.pool_by_name(pool_name).await?;
        let week_entry = pool.week(week)?;

        let picks = self.week_picks(pool_name, week).await?;

        // Until the date locks, the only pick anybody may see is their own.
        // Knowing what the field is on before picking is the one thing that
        // would take the guesswork — and so the game — out of a survivor pool.
        if matches!(week_entry.status, WeekStatus::Open) {
            return Ok(picks
                .iter()
                .filter(|pick| pick.participant_id == user_id)
                .map(SurvivorPickView::from)
                .collect());
        }

        Ok(picks.iter().map(SurvivorPickView::from).collect())
    }

    async fn get_my_picks(&self, user_id: &str, pool_name: &str) -> Result<Vec<SurvivorPickView>> {
        let picks = self.participant_picks(pool_name, user_id).await?;
        Ok(picks.iter().map(SurvivorPickView::from).collect())
    }

    async fn lock_week(&self, user_id: &str, req: SettleWeekRequest) -> Result<SurvivorPool> {
        let pool = self.pool_by_name(&req.pool_name).await?;
        pool.validate_assistant_rights(user_id)?;

        match pool.week(req.week)?.status {
            WeekStatus::Open => {}
            // Both are already closed to picks; nothing to do.
            WeekStatus::Locked | WeekStatus::Settled => return Ok(pool),
        }

        // The fixture list is written down here, as one owner action, rather
        // than by every reader of the pick screen. From this point the pool
        // carries its own copy and a settlement does not depend on the league's
        // feed still being reachable, or still agreeing about who played.
        let eligible = self.eligible_teams(&pool, req.week).await?;

        let index = week_index(&pool, req.week)?;
        self.update_pool(
            doc! {"$set": doc!{
                format!("weeks.{index}.status"): to_bson(&WeekStatus::Locked).map_err(bson_err)?,
                format!("weeks.{index}.eligible_team_ids"): to_bson(&eligible).map_err(bson_err)?,
                "status": to_bson(&SurvivorState::InProgress).map_err(bson_err)?,
            }},
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn settle_week(&self, user_id: &str, req: SettleWeekRequest) -> Result<SurvivorPool> {
        let mut pool = self.pool_by_name(&req.pool_name).await?;
        pool.validate_assistant_rights(user_id)?;

        let week_entry = pool.week(req.week)?;

        // Idempotent: the owner's button and a scheduled job should not care
        // which of them ran first.
        if matches!(week_entry.status, WeekStatus::Settled) {
            return Ok(pool);
        }

        let pick_date = week_entry.pick_date.clone();
        let day = self.schedule.day_results(&pick_date).await?;

        if !day.is_complete() {
            return Err(AppError::CustomError {
                msg: format!("The games of {pick_date} are not all finished yet."),
            });
        }

        let picks = self.week_picks(&req.pool_name, req.week).await?;

        // What each pick did, and the same answer written back onto the pick so
        // the standings do not have to re-derive it from a scoreboard that will
        // not be fetched again.
        let mut outcomes: HashMap<String, PickOutcome> = HashMap::new();
        for pick in &picks {
            let outcome = match day.result_for(pick.team_id) {
                TeamResult::Won => PickOutcome::Won,
                TeamResult::Lost => PickOutcome::Lost,
                TeamResult::Unresolved => PickOutcome::Void,
            };
            outcomes.insert(pick.participant_id.clone(), outcome);

            self.picks
                .update_one(
                    doc! {
                        "pool_name": &req.pool_name,
                        "participant_id": &pick.participant_id,
                        "week": req.week as i32,
                    },
                    doc! {"$set": doc!{"outcome": to_bson(&outcome).map_err(bson_err)?}},
                    None,
                )
                .await
                .map_err(mongo_err)?;
        }

        // Whoever had no legal pick that day: they are not penalised for not
        // making one. Decided from the same teams the picks were validated
        // against, per participant, since what is left to each of them differs.
        let eligible = if day.teams_playing.is_empty() {
            pool.week(req.week)?.eligible_team_ids.clone()
        } else {
            day.teams_playing.clone()
        };

        let mut blocked: HashSet<String> = HashSet::new();
        for participant in pool.participants.iter().filter(|p| p.is_alive()) {
            if outcomes.contains_key(&participant.id) {
                continue;
            }

            let used: Vec<UsedTeam> = self
                .participant_picks(&req.pool_name, &participant.id)
                .await?
                .iter()
                .filter(|pick| pick.week != req.week)
                .map(SurvivorPick::used_team)
                .collect();

            if is_blocked(&used, &eligible, pool.settings.league_team_count) {
                blocked.insert(participant.id.clone());
            }
        }

        pool.apply_week_results(req.week, &outcomes, &blocked, Utc::now().timestamp_millis())?;

        let index = week_index(&pool, req.week)?;
        self.update_pool(
            doc! {"$set": doc!{
                "participants": to_bson(&pool.participants).map_err(bson_err)?,
                format!("weeks.{index}.status"): to_bson(&WeekStatus::Settled).map_err(bson_err)?,
                format!("weeks.{index}.settled_at"): pool.week(req.week)?.settled_at,
                format!("weeks.{index}.eligible_team_ids"): to_bson(&eligible).map_err(bson_err)?,
                "status": to_bson(&pool.status).map_err(bson_err)?,
                "winners": to_bson(&pool.winners).map_err(bson_err)?,
            }},
            &req.pool_name,
            pool.date_updated,
        )
        .await
    }

    async fn get_standings(&self, pool_name: &str) -> Result<SurvivorStandings> {
        let pool = self.pool_by_name(pool_name).await?;

        // Only the dates whose picks are public. An open date's picks are not
        // in the standings at all, rather than in it blanked out — a payload
        // that carried them would reveal them to anybody reading the response.
        let revealed_weeks: Vec<u16> = pool
            .weeks
            .iter()
            .filter(|week| !matches!(week.status, WeekStatus::Open))
            .map(|week| week.week)
            .collect();

        let mut picks_by_participant: HashMap<String, HashMap<u16, RevealedPick>> = HashMap::new();
        let mut wins: HashMap<String, u16> = HashMap::new();

        if !revealed_weeks.is_empty() {
            let weeks: Vec<i32> = revealed_weeks.iter().map(|week| *week as i32).collect();
            let cursor = self
                .picks
                .find(
                    doc! {"pool_name": pool_name, "week": doc! {"$in": weeks}},
                    None,
                )
                .await
                .map_err(mongo_err)?;

            let picks: Vec<SurvivorPick> = cursor.try_collect().await.map_err(mongo_err)?;

            for pick in picks {
                if matches!(pick.outcome, PickOutcome::Won) {
                    *wins.entry(pick.participant_id.clone()).or_default() += 1;
                }
                picks_by_participant
                    .entry(pick.participant_id.clone())
                    .or_default()
                    .insert(
                        pick.week,
                        RevealedPick {
                            team_id: pick.team_id,
                            outcome: pick.outcome,
                        },
                    );
            }
        }

        let mut rows: Vec<SurvivorStandingRow> = pool
            .participants
            .iter()
            .map(|participant| SurvivorStandingRow {
                participant_id: participant.id.clone(),
                name: participant.name.clone(),
                status: participant.status.clone(),
                strikes: participant.strikes,
                eliminated_week: participant.eliminated_week,
                wins: wins.get(&participant.id).copied().unwrap_or(0),
                picks: picks_by_participant
                    .remove(&participant.id)
                    .unwrap_or_default(),
            })
            .collect();

        // Still standing first, then by dates survived; among those already out,
        // whoever lasted longest. The order the page reads top to bottom, done
        // here so hundreds of rows are not sorted in the browser.
        rows.sort_by(|left, right| {
            let alive = right
                .status
                .eq(&poolnhl_interface::survivor::model::ParticipantStatus::Alive)
                .cmp(
                    &left
                        .status
                        .eq(&poolnhl_interface::survivor::model::ParticipantStatus::Alive),
                );

            alive
                .then(right.wins.cmp(&left.wins))
                .then(right.eliminated_week.cmp(&left.eliminated_week))
                .then(left.name.cmp(&right.name))
        });

        let alive_count = pool.alive_participants().count() as u16;

        Ok(SurvivorStandings {
            pool_name: pool.name.clone(),
            status: pool.status.clone(),
            alive_count,
            eliminated_count: pool.participants.len() as u16 - alive_count,
            revealed_weeks,
            rows,
            winners: pool.winners.clone(),
        })
    }
}
