//! Integration tests for the mongo-backed survivor service.
//!
//! They need a running mongo:
//!   docker compose up -d mongo
//!   cargo test -p poolnhl_infrastructure --test survivor_service_mongo -- --ignored
//!
//! Like the pool service's, they run against a dedicated `hockeypooltest`
//! database (never the seeded `hockeypool` one) and each test uses a uniquely
//! named pool, so they can run in parallel and leave the dev data alone.
//!
//! What they are here for is the part that cannot be unit tested: the two unique
//! indexes the game's rules are enforced with. A check in Rust would be
//! read-then-write and two picks racing each other would both pass it, so these
//! tests run the race.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use poolnhl_infrastructure::database_connection::{DatabaseConnection, DatabaseManager};
use poolnhl_infrastructure::services::nhl_schedule_service::{
    DayResults, NhlScheduleService, TeamResult,
};
use poolnhl_infrastructure::services::survivor_service::MongoSurvivorService;
use poolnhl_interface::survivor::model::{
    ParticipantStatus, PickOutcome, SurvivorPool, SurvivorSettings, SurvivorState, WeekStatus,
};
use poolnhl_interface::survivor::requests::{
    JoinSurvivorRequest, MakePickRequest, SettleWeekRequest, SurvivorCreationRequest,
    SurvivorDeletionRequest,
};
use poolnhl_interface::survivor::service::SurvivorService;

const TEST_DATABASE: &str = "hockeypooltest";

const OWNER: &str = "survivor-owner";

fn mongo_uri() -> String {
    std::env::var("TEST_MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string())
}

async fn database() -> DatabaseConnection {
    DatabaseManager::new_pool(&mongo_uri(), TEST_DATABASE)
        .await
        .expect("mongo is not reachable; start it with `docker compose up -d mongo`")
}

/// The service under test, with a schedule that answers from what a test
/// recorded rather than from the league's feed.
async fn service() -> (Arc<MongoSurvivorService>, NhlScheduleService) {
    let db = database().await;
    let schedule = NhlScheduleService::new();
    let service = Arc::new(MongoSurvivorService::new(db, schedule.clone()));

    // The rules under test *are* the indexes, so they have to exist.
    service
        .init_indexes()
        .await
        .expect("could not create the survivor indexes");

    (service, schedule)
}

fn unique_pool_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test-survivor-{prefix}-{nanos}")
}

fn settings(max_participants: u16) -> SurvivorSettings {
    let mut settings = SurvivorSettings::new();
    settings.max_participants = max_participants;
    settings
}

/// A finished day where `winners` won, `losers` lost and nobody else played.
fn day(winners: &[u32], losers: &[u32]) -> DayResults {
    let mut results = HashMap::new();
    let mut teams_playing = Vec::new();

    for team in winners {
        results.insert(*team, TeamResult::Won);
        teams_playing.push(*team);
    }
    for team in losers {
        results.insert(*team, TeamResult::Lost);
        teams_playing.push(*team);
    }

    teams_playing.sort_unstable();

    DayResults {
        teams_playing,
        results,
    }
}

/// A pool with its first pick date's games already known, so nothing in the
/// test reaches the network.
async fn pool_with_schedule(
    service: &MongoSurvivorService,
    schedule: &NhlScheduleService,
    prefix: &str,
    max_participants: u16,
    winners: &[u32],
    losers: &[u32],
) -> SurvivorPool {
    let pool = service
        .create_pool(
            OWNER,
            SurvivorCreationRequest {
                pool_name: unique_pool_name(prefix),
                settings: settings(max_participants),
                participant_name: "owner".to_string(),
            },
        )
        .await
        .expect("could not create the pool");

    schedule
        .record_day(&pool.weeks[0].pick_date, day(winners, losers))
        .expect("could not record the day's games");

    pool
}

async fn cleanup(service: &MongoSurvivorService, pool_name: &str) {
    let _ = service
        .delete_pool(
            OWNER,
            SurvivorDeletionRequest {
                pool_name: pool_name.to_string(),
            },
        )
        .await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn creating_a_pool_puts_its_owner_in_it_with_a_week_per_saturday() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "create", 10, &[10], &[8]).await;

    let stored = service.get_pool_by_name(&pool.name).await.unwrap();

    assert_eq!(stored.owner, OWNER);
    assert_eq!(stored.participants.len(), 1);
    assert_eq!(stored.participants[0].id, OWNER);
    assert!(matches!(stored.status, SurvivorState::Created));
    assert!(stored.weeks.len() > 20, "a season of Saturdays");
    assert_eq!(stored.weeks[0].week, 1);

    cleanup(&service, &pool.name).await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn a_pool_name_is_taken_only_once() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "dup-name", 10, &[10], &[8]).await;

    let again = service
        .create_pool(
            "somebody-else",
            SurvivorCreationRequest {
                pool_name: pool.name.clone(),
                settings: settings(10),
                participant_name: "other".to_string(),
            },
        )
        .await;

    assert!(again.is_err(), "the unique index on the name should refuse");

    cleanup(&service, &pool.name).await;
}

/// The season listing.
///
/// Worth its own test because the listing does not read pool documents back —
/// it counts the participants in mongo with `$size` so a listing of twenty
/// pools of three hundred does not put six thousand participant objects on the
/// wire. An earlier version used a `find` with a projection, which stripped the
/// settings fields the document needs to deserialize and answered 500.
#[tokio::test]
#[ignore = "needs a running mongo"]
async fn the_season_listing_carries_each_pools_participant_count() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "listing", 42, &[10], &[8]).await;

    service
        .join_pool(
            "rival",
            JoinSurvivorRequest {
                pool_name: pool.name.clone(),
                participant_name: "rival".to_string(),
            },
        )
        .await
        .unwrap();

    let listed = service.list_pools(pool.season).await.unwrap();
    let row = listed
        .iter()
        .find(|candidate| candidate.name == pool.name)
        .expect("the pool should be in its own season's listing");

    assert_eq!(row.owner, OWNER);
    assert_eq!(row.participant_count, 2);
    assert_eq!(row.max_participants, 42);
    assert!(matches!(row.status, SurvivorState::Created));

    // And it is not in another season's.
    let other_season = service.list_pools(pool.season + 1).await.unwrap();
    assert!(
        !other_season
            .iter()
            .any(|candidate| candidate.name == pool.name)
    );

    cleanup(&service, &pool.name).await;
}

// ------------------------------------------------- one pick per participant

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn submitting_a_pick_twice_changes_it_rather_than_adding_one() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "repick", 10, &[10, 6], &[8]).await;

    for team_id in [10, 6] {
        service
            .make_pick(
                OWNER,
                MakePickRequest {
                    pool_name: pool.name.clone(),
                    week: 1,
                    team_id,
                },
            )
            .await
            .unwrap();
    }

    let picks = service.get_my_picks(OWNER, &pool.name).await.unwrap();

    // One document, carrying the second team — not two picks for one date.
    assert_eq!(picks.len(), 1);
    assert_eq!(picks[0].team_id, 6);

    cleanup(&service, &pool.name).await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn changing_a_pick_does_not_count_its_own_team_as_already_used() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "rechoose", 10, &[10, 6], &[8]).await;

    let request = |team_id| MakePickRequest {
        pool_name: pool.name.clone(),
        week: 1,
        team_id,
    };

    service.make_pick(OWNER, request(10)).await.unwrap();
    service.make_pick(OWNER, request(6)).await.unwrap();
    // Back to the first team: it was only ever used by the pick being replaced,
    // so it must still be open.
    service.make_pick(OWNER, request(10)).await.unwrap();

    let picks = service.get_my_picks(OWNER, &pool.name).await.unwrap();
    assert_eq!(picks.len(), 1);
    assert_eq!(picks[0].team_id, 10);

    cleanup(&service, &pool.name).await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn a_team_already_used_is_closed_on_a_later_date() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "reuse", 10, &[10, 6], &[8]).await;

    // Week 2 plays the same teams.
    schedule
        .record_day(&pool.weeks[1].pick_date, day(&[10, 6], &[8]))
        .unwrap();

    service
        .make_pick(
            OWNER,
            MakePickRequest {
                pool_name: pool.name.clone(),
                week: 1,
                team_id: 10,
            },
        )
        .await
        .unwrap();

    let reused = service
        .make_pick(
            OWNER,
            MakePickRequest {
                pool_name: pool.name.clone(),
                week: 2,
                team_id: 10,
            },
        )
        .await;

    assert!(reused.is_err(), "a used team must be closed");

    // And a team they have not used is still open that same date.
    service
        .make_pick(
            OWNER,
            MakePickRequest {
                pool_name: pool.name.clone(),
                week: 2,
                team_id: 6,
            },
        )
        .await
        .unwrap();

    cleanup(&service, &pool.name).await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn two_picks_racing_for_the_same_team_leave_exactly_one() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "race-team", 10, &[10, 6], &[8]).await;
    schedule
        .record_day(&pool.weeks[1].pick_date, day(&[10, 6], &[8]))
        .unwrap();

    // The same participant picking the same team for two dates at once. Only
    // one may land: the unique index on (pool, participant, cycle, team) is
    // what decides it, and no amount of checking in Rust could.
    let first = {
        let service = service.clone();
        let pool_name = pool.name.clone();
        tokio::spawn(async move {
            service
                .make_pick(
                    OWNER,
                    MakePickRequest {
                        pool_name,
                        week: 1,
                        team_id: 10,
                    },
                )
                .await
        })
    };
    let second = {
        let service = service.clone();
        let pool_name = pool.name.clone();
        tokio::spawn(async move {
            service
                .make_pick(
                    OWNER,
                    MakePickRequest {
                        pool_name,
                        week: 2,
                        team_id: 10,
                    },
                )
                .await
        })
    };

    let (first, second) = (first.await.unwrap(), second.await.unwrap());
    let landed = [first.is_ok(), second.is_ok()]
        .iter()
        .filter(|ok| **ok)
        .count();

    assert_eq!(landed, 1, "exactly one of the two picks may land");

    let picks = service.get_my_picks(OWNER, &pool.name).await.unwrap();
    let on_that_team = picks.iter().filter(|pick| pick.team_id == 10).count();
    assert_eq!(on_that_team, 1, "the team is spent once, never twice");

    cleanup(&service, &pool.name).await;
}

// ------------------------------------------------------------------- scale

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn two_hundred_participants_pick_at_once_and_every_pick_lands() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "scale", 300, &[10, 6, 14], &[8]).await;

    const PARTICIPANTS: usize = 200;

    // Joining is a pool-document write under the optimistic lock, and it is
    // meant to be: it is spread over the days before the pool starts.
    for index in 0..PARTICIPANTS {
        service
            .join_pool(
                &format!("picker-{index}"),
                JoinSurvivorRequest {
                    pool_name: pool.name.clone(),
                    participant_name: format!("picker-{index}"),
                },
            )
            .await
            .unwrap_or_else(|e| panic!("picker-{index} could not join: {e}"));
    }

    // Picking is the Saturday-morning stampede, and it is the thing that must
    // not contend: each of these is an insert into its own document.
    let mut picking = Vec::with_capacity(PARTICIPANTS);
    for index in 0..PARTICIPANTS {
        let service = service.clone();
        let pool_name = pool.name.clone();
        picking.push(tokio::spawn(async move {
            service
                .make_pick(
                    &format!("picker-{index}"),
                    MakePickRequest {
                        pool_name,
                        week: 1,
                        // They crowd onto the same few teams, the way a real
                        // field does behind the favourites.
                        team_id: [10, 6, 14][index % 3],
                    },
                )
                .await
        }));
    }

    let mut failures = Vec::new();
    for (index, handle) in picking.into_iter().enumerate() {
        if let Err(error) = handle.await.unwrap() {
            failures.push(format!("picker-{index}: {error}"));
        }
    }

    assert!(
        failures.is_empty(),
        "every concurrent pick should land, {} did not: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );

    // Locked first, since an open date shows a caller nothing but their own
    // pick — the owner, who never picked, would otherwise see none of these.
    service
        .lock_week(
            OWNER,
            SettleWeekRequest {
                pool_name: pool.name.clone(),
                week: 1,
            },
        )
        .await
        .unwrap();

    let picks = service.get_week_picks(OWNER, &pool.name, 1).await.unwrap();
    assert_eq!(picks.len(), PARTICIPANTS);

    // And every one of them is a distinct participant: the one-pick-per-date
    // index held under all 200 at once.
    let distinct: std::collections::HashSet<&str> = picks
        .iter()
        .map(|pick| pick.participant_id.as_str())
        .collect();
    assert_eq!(distinct.len(), PARTICIPANTS);

    cleanup(&service, &pool.name).await;
}

// -------------------------------------------------------------- the reveal

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn an_open_dates_picks_are_the_callers_own_and_nobody_elses() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "hidden", 10, &[10, 6], &[8]).await;

    service
        .join_pool(
            "rival",
            JoinSurvivorRequest {
                pool_name: pool.name.clone(),
                participant_name: "rival".to_string(),
            },
        )
        .await
        .unwrap();

    for (user, team_id) in [(OWNER, 10), ("rival", 6)] {
        service
            .make_pick(
                user,
                MakePickRequest {
                    pool_name: pool.name.clone(),
                    week: 1,
                    team_id,
                },
            )
            .await
            .unwrap();
    }

    // Still open: each of them sees only themselves. Seeing the field before
    // picking is the one thing that would take the game out of the game.
    let seen = service.get_week_picks(OWNER, &pool.name, 1).await.unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].participant_id, OWNER);

    // Locked: everybody's is public, which is most of what people come for.
    service
        .lock_week(
            OWNER,
            SettleWeekRequest {
                pool_name: pool.name.clone(),
                week: 1,
            },
        )
        .await
        .unwrap();

    let revealed = service.get_week_picks(OWNER, &pool.name, 1).await.unwrap();
    assert_eq!(revealed.len(), 2);

    cleanup(&service, &pool.name).await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn a_locked_date_takes_no_more_picks() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "locked", 10, &[10, 6], &[8]).await;

    service
        .lock_week(
            OWNER,
            SettleWeekRequest {
                pool_name: pool.name.clone(),
                week: 1,
            },
        )
        .await
        .unwrap();

    let late = service
        .make_pick(
            OWNER,
            MakePickRequest {
                pool_name: pool.name.clone(),
                week: 1,
                team_id: 10,
            },
        )
        .await;

    assert!(late.is_err());

    cleanup(&service, &pool.name).await;
}

// ---------------------------------------------------------------- settling

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn settling_a_date_eliminates_whoever_backed_a_loser() {
    let (service, schedule) = service().await;
    // Team 10 wins, team 8 loses.
    let pool = pool_with_schedule(&service, &schedule, "settle", 10, &[10], &[8]).await;

    for (user, team_id) in [(OWNER, 10), ("doomed", 8), ("also-doomed", 8)] {
        if user != OWNER {
            service
                .join_pool(
                    user,
                    JoinSurvivorRequest {
                        pool_name: pool.name.clone(),
                        participant_name: user.to_string(),
                    },
                )
                .await
                .unwrap();
        }
        service
            .make_pick(
                user,
                MakePickRequest {
                    pool_name: pool.name.clone(),
                    week: 1,
                    team_id,
                },
            )
            .await
            .unwrap();
    }

    let settled = service
        .settle_week(
            OWNER,
            SettleWeekRequest {
                pool_name: pool.name.clone(),
                week: 1,
            },
        )
        .await
        .unwrap();

    let by_id: HashMap<&str, &_> = settled
        .participants
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();

    assert!(matches!(by_id[OWNER].status, ParticipantStatus::Alive));
    assert!(matches!(
        by_id["doomed"].status,
        ParticipantStatus::Eliminated
    ));
    assert_eq!(by_id["doomed"].eliminated_week, Some(1));

    // One left standing, so the pool is decided.
    assert!(matches!(settled.status, SurvivorState::Final));
    assert_eq!(settled.winners, Some(vec![OWNER.to_string()]));
    assert!(matches!(
        settled.week(1).unwrap().status,
        WeekStatus::Settled
    ));

    // The outcome is written back onto each pick, so the standings never have
    // to re-read a scoreboard.
    let picks = service.get_week_picks(OWNER, &pool.name, 1).await.unwrap();
    let owner_pick = picks
        .iter()
        .find(|pick| pick.participant_id == OWNER)
        .unwrap();
    assert!(matches!(owner_pick.outcome, PickOutcome::Won));

    cleanup(&service, &pool.name).await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn settling_the_same_date_twice_changes_nothing() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "idempotent", 10, &[10], &[8]).await;

    service
        .join_pool(
            "rival",
            JoinSurvivorRequest {
                pool_name: pool.name.clone(),
                participant_name: "rival".to_string(),
            },
        )
        .await
        .unwrap();

    for (user, team_id) in [(OWNER, 10), ("rival", 10)] {
        // Two participants may back the same team; the used-team rule is
        // per-participant.
        service
            .make_pick(
                user,
                MakePickRequest {
                    pool_name: pool.name.clone(),
                    week: 1,
                    team_id,
                },
            )
            .await
            .unwrap();
    }

    let request = || SettleWeekRequest {
        pool_name: pool.name.clone(),
        week: 1,
    };

    let first = service.settle_week(OWNER, request()).await.unwrap();
    // The owner's button and a scheduled job must not care which ran first.
    let second = service.settle_week(OWNER, request()).await.unwrap();

    assert_eq!(first.participants.len(), second.participants.len());
    for (left, right) in first.participants.iter().zip(second.participants.iter()) {
        assert_eq!(left.strikes, right.strikes, "{}", left.id);
        assert_eq!(left.eliminated_week, right.eliminated_week, "{}", left.id);
    }

    cleanup(&service, &pool.name).await;
}

/// A day with a game still to resolve cannot be recorded as settleable, which
/// is the near end of the guard that stops a date being settled mid-evening.
/// The far end — `settle_week` refusing an incomplete day — is covered by the
/// `is_complete` unit tests, since reaching it here would mean calling the
/// league's live feed from a test.
#[tokio::test]
#[ignore = "needs a running mongo"]
async fn a_day_with_a_game_left_to_play_cannot_be_recorded_as_finished() {
    let (_, schedule) = service().await;

    let unfinished = DayResults {
        teams_playing: vec![8, 10],
        results: HashMap::from([(10, TeamResult::Unresolved), (8, TeamResult::Won)]),
    };

    assert!(!unfinished.is_complete());
    assert!(schedule.record_day("2026-10-03", unfinished).is_err());
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn only_the_owner_or_an_assistant_may_settle_a_date() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "rights", 10, &[10], &[8]).await;

    service
        .join_pool(
            "nosy",
            JoinSurvivorRequest {
                pool_name: pool.name.clone(),
                participant_name: "nosy".to_string(),
            },
        )
        .await
        .unwrap();

    let refused = service
        .settle_week(
            "nosy",
            SettleWeekRequest {
                pool_name: pool.name.clone(),
                week: 1,
            },
        )
        .await;

    assert!(refused.is_err());

    cleanup(&service, &pool.name).await;
}

// --------------------------------------------------------------- standings

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn the_standings_count_the_field_and_hide_an_open_dates_picks() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "standings", 10, &[10], &[8]).await;

    service
        .join_pool(
            "rival",
            JoinSurvivorRequest {
                pool_name: pool.name.clone(),
                participant_name: "rival".to_string(),
            },
        )
        .await
        .unwrap();

    for (user, team_id) in [(OWNER, 10), ("rival", 8)] {
        service
            .make_pick(
                user,
                MakePickRequest {
                    pool_name: pool.name.clone(),
                    week: 1,
                    team_id,
                },
            )
            .await
            .unwrap();
    }

    // While week 1 is open, the standings carry no picks at all — a payload
    // that carried them would reveal them to whoever read the response.
    let open = service.get_standings(&pool.name).await.unwrap();
    assert!(open.revealed_weeks.is_empty());
    assert!(open.rows.iter().all(|row| row.picks.is_empty()));
    assert_eq!(open.alive_count, 2);

    service
        .settle_week(
            OWNER,
            SettleWeekRequest {
                pool_name: pool.name.clone(),
                week: 1,
            },
        )
        .await
        .unwrap();

    let settled = service.get_standings(&pool.name).await.unwrap();
    assert_eq!(settled.revealed_weeks, vec![1]);
    assert_eq!(settled.alive_count, 1);
    assert_eq!(settled.eliminated_count, 1);

    // Still standing first, which is the order the page reads top to bottom.
    assert_eq!(settled.rows[0].participant_id, OWNER);
    assert_eq!(settled.rows[0].wins, 1);
    assert_eq!(settled.rows[0].picks[&1].team_id, 10);
    assert!(matches!(
        settled.rows[1].status,
        ParticipantStatus::Eliminated
    ));

    cleanup(&service, &pool.name).await;
}

// ------------------------------------------------------------------ joining

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn a_pool_does_not_take_more_than_its_maximum() {
    let (service, schedule) = service().await;
    // Room for the owner and one more.
    let pool = pool_with_schedule(&service, &schedule, "full", 2, &[10], &[8]).await;

    service
        .join_pool(
            "second",
            JoinSurvivorRequest {
                pool_name: pool.name.clone(),
                participant_name: "second".to_string(),
            },
        )
        .await
        .unwrap();

    let refused = service
        .join_pool(
            "third",
            JoinSurvivorRequest {
                pool_name: pool.name.clone(),
                participant_name: "third".to_string(),
            },
        )
        .await;

    assert!(refused.is_err());

    cleanup(&service, &pool.name).await;
}

#[tokio::test]
#[ignore = "needs a running mongo"]
async fn deleting_a_pool_takes_its_picks_with_it() {
    let (service, schedule) = service().await;
    let pool = pool_with_schedule(&service, &schedule, "delete", 10, &[10], &[8]).await;

    service
        .make_pick(
            OWNER,
            MakePickRequest {
                pool_name: pool.name.clone(),
                week: 1,
                team_id: 10,
            },
        )
        .await
        .unwrap();

    service
        .delete_pool(
            OWNER,
            SurvivorDeletionRequest {
                pool_name: pool.name.clone(),
            },
        )
        .await
        .unwrap();

    assert!(service.get_pool_by_name(&pool.name).await.is_err());
    // The picks are their own documents, so they would otherwise be left
    // behind — and would close those teams to the same person in a new pool of
    // the same name.
    let orphans = service.get_my_picks(OWNER, &pool.name).await.unwrap();
    assert!(orphans.is_empty());
}
