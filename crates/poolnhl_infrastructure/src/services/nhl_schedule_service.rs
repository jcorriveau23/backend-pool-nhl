//! Who plays on a date, and who won.
//!
//! A survivor pool eliminates people, so it cannot take either from the client:
//! a browser that could report its own result could report a win. This reads the
//! league's own scoreboard instead, server-side, and is the only thing a
//! settlement trusts.
//!
//! Results for a date whose games have all finished never change, so those are
//! kept; a date still being played is re-read every time.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::Deserialize;

use poolnhl_interface::errors::{AppError, Result};

const SCOREBOARD_URL: &str = "https://api-web.nhle.com/v1/score";

/// Game states the league reports for a game that is over and will not change.
const FINISHED_STATES: [&str; 2] = ["OFF", "FINAL"];
/// A game the league has called off. Nobody who picked either side is penalised
/// for it.
const ABANDONED_STATES: [&str; 2] = ["PPD", "CNCL"];

#[derive(Debug, Deserialize)]
struct ScoreboardTeam {
    id: u32,
    #[serde(default)]
    score: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ScoreboardGame {
    #[serde(rename = "gameState", default)]
    game_state: String,
    #[serde(rename = "homeTeam")]
    home_team: ScoreboardTeam,
    #[serde(rename = "awayTeam")]
    away_team: ScoreboardTeam,
}

#[derive(Debug, Deserialize)]
struct Scoreboard {
    #[serde(default)]
    games: Vec<ScoreboardGame>,
}

/// How a team's game on a date turned out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamResult {
    Won,
    Lost,
    /// Called off, or still unplayed. Costs whoever picked the team nothing.
    Unresolved,
}

/// A date's games, reduced to what a survivor pool asks of them.
#[derive(Debug, Clone, Default)]
pub struct DayResults {
    /// Every team with a game scheduled that day, whatever came of it. This is
    /// what a pick is validated against.
    pub teams_playing: Vec<u32>,
    /// What came of each team's game.
    pub results: HashMap<u32, TeamResult>,
}

impl DayResults {
    /// Whether every game of the day has a result, which is what makes the date
    /// safe to settle and its answer safe to keep.
    pub fn is_complete(&self) -> bool {
        !self.results.is_empty()
            && self
                .results
                .values()
                .all(|result| !matches!(result, TeamResult::Unresolved))
    }

    pub fn result_for(&self, team_id: u32) -> TeamResult {
        self.results
            .get(&team_id)
            .copied()
            .unwrap_or(TeamResult::Unresolved)
    }
}

/// How long a not-yet-finished day's fixture list is reused.
///
/// Who plays on a date is settled well in advance and changes only when the
/// league moves a game, so an hour-old answer is fine — and it is the
/// difference between one upstream call on a Saturday morning and one per
/// person opening the pick screen.
const SCHEDULE_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// A date's fixture list and when it was read, keyed by date.
type FixtureCache = HashMap<String, (Instant, Vec<u32>)>;

#[derive(Clone)]
pub struct NhlScheduleService {
    client: reqwest::Client,
    /// Dates whose games have all finished, so their answer cannot change.
    settled_days: Arc<RwLock<HashMap<String, DayResults>>>,
    /// Who plays on a date, kept for [`SCHEDULE_CACHE_TTL`].
    ///
    /// Separate from `settled_days` because the two age differently: a result
    /// is final or it is nothing, while a fixture list is useful long before
    /// the games are played — which is exactly when the pick screen needs it.
    fixtures: Arc<RwLock<FixtureCache>>,
}

impl Default for NhlScheduleService {
    fn default() -> Self {
        Self::new()
    }
}

impl NhlScheduleService {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            settled_days: Arc::new(RwLock::new(HashMap::new())),
            fixtures: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a finished day's results without reading the league's feed.
    ///
    /// For driving a settlement over a known day — the integration tests do it
    /// to settle a date without reaching the network, and a scheduled job that
    /// pre-fetched a Saturday evening's results would use the same door. Only a
    /// day with every game resolved is accepted, since nothing else may be
    /// settled from.
    pub fn record_day(&self, date: &str, day: DayResults) -> Result<()> {
        if !day.is_complete() {
            return Err(AppError::CustomError {
                msg: format!("the games of {date} are not all finished"),
            });
        }

        self.settled_days
            .write()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .insert(date.to_string(), day);

        Ok(())
    }

    /// The day's games. `date` is `yyyy-mm-dd`.
    pub async fn day_results(&self, date: &str) -> Result<DayResults> {
        if let Some(cached) = self.cached(date)? {
            return Ok(cached);
        }

        let url = format!("{SCOREBOARD_URL}/{date}");
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::ReqwestError { msg: e.to_string() })?;

        if !response.status().is_success() {
            return Err(AppError::ReqwestError {
                msg: format!("the scoreboard for {date} answered {}", response.status()),
            });
        }

        let scoreboard: Scoreboard = response
            .json()
            .await
            .map_err(|e| AppError::ReqwestError { msg: e.to_string() })?;

        let day = reduce(scoreboard);

        // Only a day that is wholly done is worth keeping as a result: anything
        // else still has one coming. Its fixture list is worth keeping either
        // way, and for the pick screen that is the whole point.
        if day.is_complete() {
            self.settled_days
                .write()
                .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
                .insert(date.to_string(), day.clone());
        }

        if !day.teams_playing.is_empty() {
            self.fixtures
                .write()
                .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
                .insert(
                    date.to_string(),
                    (Instant::now(), day.teams_playing.clone()),
                );
        }

        Ok(day)
    }

    /// The teams with a game on `date`, which is what a pick is validated
    /// against.
    ///
    /// Served from the fixture cache when it can be, so a pool of hundreds all
    /// opening the pick screen on the same morning is one call upstream rather
    /// than one each.
    pub async fn teams_playing_on(&self, date: &str) -> Result<Vec<u32>> {
        if let Some(teams) = self.cached_fixtures(date)? {
            return Ok(teams);
        }

        Ok(self.day_results(date).await?.teams_playing)
    }

    fn cached(&self, date: &str) -> Result<Option<DayResults>> {
        Ok(self
            .settled_days
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .get(date)
            .cloned())
    }

    fn cached_fixtures(&self, date: &str) -> Result<Option<Vec<u32>>> {
        // A finished day's list is already final, so it never expires.
        if let Some(day) = self.cached(date)? {
            return Ok(Some(day.teams_playing));
        }

        Ok(self
            .fixtures
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .get(date)
            .filter(|(fetched_at, _)| fetched_at.elapsed() < SCHEDULE_CACHE_TTL)
            .map(|(_, teams)| teams.clone()))
    }
}

/// Turn the league's scoreboard into the per-team result a pool reads.
fn reduce(scoreboard: Scoreboard) -> DayResults {
    let mut day = DayResults::default();

    for game in scoreboard.games {
        let home = game.home_team.id;
        let away = game.away_team.id;

        day.teams_playing.push(home);
        day.teams_playing.push(away);

        let (home_result, away_result) = outcome_of(&game);
        day.results.insert(home, home_result);
        day.results.insert(away, away_result);
    }

    day.teams_playing.sort_unstable();
    day.teams_playing.dedup();

    day
}

fn outcome_of(game: &ScoreboardGame) -> (TeamResult, TeamResult) {
    let state = game.game_state.as_str();

    if ABANDONED_STATES.contains(&state) {
        return (TeamResult::Unresolved, TeamResult::Unresolved);
    }

    if !FINISHED_STATES.contains(&state) {
        return (TeamResult::Unresolved, TeamResult::Unresolved);
    }

    // A finished game the league reported without a score is not something to
    // guess at; leaving it unresolved costs nobody their pool.
    let (Some(home_score), Some(away_score)) = (game.home_team.score, game.away_team.score) else {
        return (TeamResult::Unresolved, TeamResult::Unresolved);
    };

    match home_score.cmp(&away_score) {
        std::cmp::Ordering::Greater => (TeamResult::Won, TeamResult::Lost),
        std::cmp::Ordering::Less => (TeamResult::Lost, TeamResult::Won),
        // Hockey settles a tie in overtime or a shootout, so a finished game
        // level on the scoreboard means the feed is not telling the whole
        // story. Treated as unresolved rather than as a loss for both.
        std::cmp::Ordering::Equal => (TeamResult::Unresolved, TeamResult::Unresolved),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(state: &str, home: (u32, Option<u32>), away: (u32, Option<u32>)) -> ScoreboardGame {
        ScoreboardGame {
            game_state: state.to_string(),
            home_team: ScoreboardTeam {
                id: home.0,
                score: home.1,
            },
            away_team: ScoreboardTeam {
                id: away.0,
                score: away.1,
            },
        }
    }

    #[test]
    fn a_finished_game_gives_its_winner_and_its_loser() {
        let day = reduce(Scoreboard {
            games: vec![game("OFF", (10, Some(4)), (8, Some(1)))],
        });

        assert_eq!(day.teams_playing, vec![8, 10]);
        assert_eq!(day.result_for(10), TeamResult::Won);
        assert_eq!(day.result_for(8), TeamResult::Lost);
        assert!(day.is_complete());
    }

    #[test]
    fn the_leagues_other_name_for_a_finished_game_counts_too() {
        let day = reduce(Scoreboard {
            games: vec![game("FINAL", (6, Some(2)), (10, Some(3)))],
        });

        assert_eq!(day.result_for(10), TeamResult::Won);
        assert_eq!(day.result_for(6), TeamResult::Lost);
    }

    #[test]
    fn a_game_still_to_come_or_under_way_has_no_result_yet() {
        for state in ["FUT", "PRE", "LIVE", "CRIT"] {
            let day = reduce(Scoreboard {
                games: vec![game(state, (10, None), (8, None))],
            });

            assert_eq!(day.result_for(10), TeamResult::Unresolved, "{state}");
            assert!(!day.is_complete(), "{state}");
        }
    }

    #[test]
    fn a_postponed_game_costs_neither_side_anything() {
        let day = reduce(Scoreboard {
            games: vec![game("PPD", (10, None), (8, None))],
        });

        assert_eq!(day.result_for(10), TeamResult::Unresolved);
        assert_eq!(day.result_for(8), TeamResult::Unresolved);
        // Both teams are still "playing" that day as far as a pick is
        // concerned: the pick was legal when it was made.
        assert_eq!(day.teams_playing, vec![8, 10]);
    }

    #[test]
    fn a_finished_game_with_no_score_reported_is_not_guessed_at() {
        let day = reduce(Scoreboard {
            games: vec![game("OFF", (10, None), (8, Some(2)))],
        });

        assert_eq!(day.result_for(10), TeamResult::Unresolved);
        assert_eq!(day.result_for(8), TeamResult::Unresolved);
    }

    #[test]
    fn a_finished_game_level_on_the_scoreboard_is_not_a_loss_for_both() {
        let day = reduce(Scoreboard {
            games: vec![game("OFF", (10, Some(2)), (8, Some(2)))],
        });

        assert_eq!(day.result_for(10), TeamResult::Unresolved);
        assert_eq!(day.result_for(8), TeamResult::Unresolved);
    }

    #[test]
    fn a_day_is_only_complete_once_every_game_has_a_result() {
        let day = reduce(Scoreboard {
            games: vec![
                game("OFF", (10, Some(4)), (8, Some(1))),
                game("LIVE", (6, None), (14, None)),
            ],
        });

        assert!(!day.is_complete());
        assert_eq!(day.result_for(10), TeamResult::Won);
        assert_eq!(day.result_for(6), TeamResult::Unresolved);
    }

    // The fixture cache is what keeps a Saturday morning of hundreds of people
    // opening the pick screen to one call upstream instead of one each, so what
    // it will and will not answer from is worth pinning down.
    #[tokio::test]
    async fn a_recorded_day_answers_both_the_fixtures_and_the_results() {
        let schedule = NhlScheduleService::new();
        let day = reduce(Scoreboard {
            games: vec![game("OFF", (10, Some(4)), (8, Some(1)))],
        });

        schedule.record_day("2026-10-03", day).unwrap();

        // Neither call reaches the network; a failure here would be a hang or
        // an error, not a wrong answer.
        assert_eq!(
            schedule.teams_playing_on("2026-10-03").await.unwrap(),
            vec![8, 10]
        );
        assert_eq!(
            schedule
                .day_results("2026-10-03")
                .await
                .unwrap()
                .result_for(10),
            TeamResult::Won
        );
    }

    #[test]
    fn a_finished_days_fixtures_never_expire() {
        let schedule = NhlScheduleService::new();
        schedule
            .record_day(
                "2026-10-03",
                reduce(Scoreboard {
                    games: vec![game("OFF", (10, Some(4)), (8, Some(1)))],
                }),
            )
            .unwrap();

        // Served from the settled-day copy rather than the TTL'd one: a played
        // Saturday's fixture list cannot change, so ageing it out would only
        // buy a pointless refetch.
        assert_eq!(
            schedule.cached_fixtures("2026-10-03").unwrap(),
            Some(vec![8, 10])
        );
    }

    #[test]
    fn a_date_never_read_has_no_cached_fixtures() {
        let schedule = NhlScheduleService::new();

        assert_eq!(schedule.cached_fixtures("2026-10-03").unwrap(), None);
    }

    #[test]
    fn a_stale_fixture_list_is_not_served() {
        let schedule = NhlScheduleService::new();

        // Older than the TTL, so it must be refetched rather than returned.
        schedule.fixtures.write().unwrap().insert(
            "2026-10-03".to_string(),
            (
                Instant::now() - SCHEDULE_CACHE_TTL - Duration::from_secs(1),
                vec![8, 10],
            ),
        );

        assert_eq!(schedule.cached_fixtures("2026-10-03").unwrap(), None);
    }

    #[test]
    fn a_fresh_fixture_list_is_served() {
        let schedule = NhlScheduleService::new();

        schedule
            .fixtures
            .write()
            .unwrap()
            .insert("2026-10-03".to_string(), (Instant::now(), vec![8, 10]));

        assert_eq!(
            schedule.cached_fixtures("2026-10-03").unwrap(),
            Some(vec![8, 10])
        );
    }

    #[test]
    fn an_unfinished_day_cannot_be_recorded() {
        let schedule = NhlScheduleService::new();
        let unfinished = reduce(Scoreboard {
            games: vec![game("LIVE", (10, None), (8, None))],
        });

        // Nothing may be settled from a day still being played, so it is
        // refused rather than stored and trusted later.
        assert!(schedule.record_day("2026-10-03", unfinished).is_err());
        assert_eq!(schedule.cached_fixtures("2026-10-03").unwrap(), None);
    }

    #[test]
    fn a_date_with_no_games_is_not_complete_and_has_nobody_playing() {
        let day = reduce(Scoreboard { games: vec![] });

        assert!(day.teams_playing.is_empty());
        assert!(!day.is_complete());
    }

    #[test]
    fn a_team_playing_twice_in_a_day_is_listed_once() {
        let day = reduce(Scoreboard {
            games: vec![
                game("OFF", (10, Some(4)), (8, Some(1))),
                game("OFF", (6, Some(0)), (10, Some(2))),
            ],
        });

        assert_eq!(day.teams_playing, vec![6, 8, 10]);
    }

    #[test]
    fn the_scoreboard_shape_is_read_off_the_leagues_own_json() {
        // Trimmed from api-web.nhle.com/v1/score/{date}; the fields the pool
        // does not read are left in on purpose, a shape change in them must not
        // break a settlement.
        let body = r#"{
            "currentDate": "2026-10-10",
            "games": [{
              "id": 2026020123,
              "season": 20262027,
              "gameState": "OFF",
              "gameScheduleState": "OK",
              "awayTeam": {"id": 8, "abbrev": "MTL", "score": 2, "sog": 28},
              "homeTeam": {"id": 10, "abbrev": "TOR", "score": 5, "sog": 31},
              "periodDescriptor": {"number": 3, "periodType": "REG"}
            }]
        }"#;

        let scoreboard: Scoreboard = serde_json::from_str(body).unwrap();
        let day = reduce(scoreboard);

        assert_eq!(day.teams_playing, vec![8, 10]);
        assert_eq!(day.result_for(10), TeamResult::Won);
        assert_eq!(day.result_for(8), TeamResult::Lost);
    }
}
