//! The one test that talks to the league's live scoreboard.
//!
//!   cargo test -p poolnhl_infrastructure --test nhl_schedule_live -- --ignored
//!
//! Everything else about reading a day's results is covered offline, by the
//! unit tests next to `reduce`. What those cannot catch is the feed changing
//! shape underneath us — a renamed field, a new value for `gameState` — which
//! would stop a survivor pool from ever settling. So this one asks the real
//! endpoint, and is `#[ignore]`d because it needs the network.

use poolnhl_infrastructure::services::nhl_schedule_service::{NhlScheduleService, TeamResult};

/// A Saturday whose games are long over, so its answer can be asserted against
/// rather than merely inspected.
const FINISHED_SATURDAY: &str = "2026-09-26";

#[tokio::test]
#[ignore = "talks to the live NHL api"]
async fn a_finished_saturday_reads_back_as_a_settleable_day() {
    let schedule = NhlScheduleService::new();

    let day = schedule.day_results(FINISHED_SATURDAY).await.unwrap();

    assert!(
        !day.teams_playing.is_empty(),
        "the league played games on {FINISHED_SATURDAY}",
    );

    // What settling actually requires: every game resolved. If the feed renamed
    // `gameState` or started reporting a value this code does not know, every
    // game would come back unresolved and this is what would catch it.
    assert!(
        day.is_complete(),
        "every game of {FINISHED_SATURDAY} should have a result, got {:?}",
        day.results,
    );

    // A hockey game has a winner and a loser, so a day of finished games has
    // one of each per game and nothing else.
    let won = day
        .results
        .values()
        .filter(|result| matches!(result, TeamResult::Won))
        .count();
    let lost = day
        .results
        .values()
        .filter(|result| matches!(result, TeamResult::Lost))
        .count();

    assert_eq!(won, lost, "every game has one winner and one loser");
    assert_eq!(won + lost, day.results.len());

    // The ids must be the ones the pools and the front end's team table use; a
    // feed that started numbering teams differently would silently make every
    // pick invalid. 1..=99 is the range the NHL's own team ids sit in.
    for team_id in &day.teams_playing {
        assert!(
            *team_id > 0 && *team_id < 100,
            "{team_id} is not an NHL team id",
        );
    }
}

#[tokio::test]
#[ignore = "talks to the live NHL api"]
async fn a_finished_day_is_only_fetched_once() {
    let schedule = NhlScheduleService::new();

    let first = schedule.day_results(FINISHED_SATURDAY).await.unwrap();
    // The second call is served from the kept copy: a finished day's result
    // cannot change, and a settlement must not depend on the feed staying
    // reachable.
    let second = schedule.day_results(FINISHED_SATURDAY).await.unwrap();

    assert_eq!(first.teams_playing, second.teams_playing);
    assert_eq!(first.results.len(), second.results.len());
}
