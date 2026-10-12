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

use chrono::{DateTime, Utc};
use serde::Deserialize;

use poolnhl_interface::errors::{AppError, Result};

const SCOREBOARD_URL: &str = "https://api-web.nhle.com/v1/score";

/// Game states the league reports for a game that is over and will not change.
const FINISHED_STATES: [&str; 2] = ["OFF", "FINAL"];
/// A game the league has called off. Nobody who picked either side is penalised
/// for it.
const ABANDONED_STATES: [&str; 2] = ["PPD", "CNCL"];
/// Being played. `CRIT` is the league's name for a close game late on.
const LIVE_STATES: [&str; 2] = ["LIVE", "CRIT"];

#[derive(Debug, Deserialize)]
struct ScoreboardTeam {
    id: u32,
    #[serde(default)]
    score: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct ScoreboardClock {
    #[serde(rename = "timeRemaining", default)]
    time_remaining: Option<String>,
    #[serde(rename = "inIntermission", default)]
    in_intermission: bool,
}

#[derive(Debug, Deserialize, Default)]
struct ScoreboardPeriod {
    #[serde(default)]
    number: Option<u32>,
    /// `REG`, `OT` or `SO`. What makes "Final" and "Final (OT)" different.
    #[serde(rename = "periodType", default)]
    period_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScoreboardGame {
    #[serde(rename = "gameState", default)]
    game_state: String,
    #[serde(rename = "homeTeam")]
    home_team: ScoreboardTeam,
    #[serde(rename = "awayTeam")]
    away_team: ScoreboardTeam,
    #[serde(rename = "startTimeUTC", default)]
    start_time_utc: Option<String>,
    #[serde(default)]
    clock: ScoreboardClock,
    #[serde(rename = "periodDescriptor", default)]
    period_descriptor: ScoreboardPeriod,
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

/// Where a game stands, as the pick screen shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    /// Not started.
    Upcoming,
    /// Being played. Its score is worth showing and worth refetching, but it
    /// decides nothing yet.
    Live,
    /// Over. Its score will not change.
    Final,
    /// Postponed or cancelled. Costs whoever picked either side nothing.
    Abandoned,
}

/// One game of a date, as the pick screen shows it.
///
/// Display only, unlike [`TeamResult`]: who beat whom is decided by
/// [`outcome_of`] and nothing here is read by a settlement. A live score is
/// here on purpose, and `state` is what keeps it from reading as a result —
/// the pool has settled nothing while a game is still being played.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayGame {
    pub home_team_id: u32,
    pub away_team_id: u32,
    pub home_score: Option<u32>,
    pub away_score: Option<u32>,
    pub state: GameState,
    /// When the puck drops, as the league gave it (RFC 3339). Shown for a game
    /// still to come, and read by [`fixture_ttl`].
    pub start_time_utc: Option<String>,
    /// The period being played, or the one a finished game ended in.
    pub period: Option<u32>,
    /// `REG`, `OT` or `SO`, so a game settled past regulation can say so.
    pub period_type: Option<String>,
    /// Left in the period. Only meaningful while the game is live.
    pub time_remaining: Option<String>,
    pub in_intermission: bool,
}

impl DayGame {
    /// Whether the league should have started this game by now.
    ///
    /// A game whose start time has passed but which the feed still calls
    /// upcoming means our copy is behind, not that the puck has not dropped.
    fn should_have_started(&self, now: DateTime<Utc>) -> bool {
        self.start_time_utc
            .as_deref()
            .and_then(|start| DateTime::parse_from_rfc3339(start).ok())
            .is_some_and(|start| start.with_timezone(&Utc) <= now)
    }
}

/// A date's games, reduced to what a survivor pool asks of them.
#[derive(Debug, Clone, Default)]
pub struct DayResults {
    /// Every team with a game scheduled that day, whatever came of it. This is
    /// what a pick is validated against.
    pub teams_playing: Vec<u32>,
    /// What came of each team's game.
    pub results: HashMap<u32, TeamResult>,
    /// The same games unreduced, so the pick screen can show who plays whom.
    pub games: Vec<DayGame>,
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

/// How long a day's fixture list is reused when nothing on it has started.
///
/// Who plays on a date is settled well in advance and changes only when the
/// league moves a game, so an hour-old answer is fine — and it is the
/// difference between one upstream call on a Saturday morning and one per
/// person opening the pick screen.
const SCHEDULE_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// How long a day with a game under way is reused.
///
/// Short, because the scores on it are being shown as live — but only short
/// enough to be worth calling live. A survivor pool's pick screen is not a
/// play-by-play, and three minutes keeps a score current enough to follow at a
/// quarter of the upstream reads. It is still a cache: one read per interval
/// serves every participant watching, rather than one read each.
const LIVE_CACHE_TTL: Duration = Duration::from_secs(3 * 60);

/// How long `games` may be served before being read again.
///
/// Keyed off the games themselves rather than stored with them, so the answer
/// follows the day as it moves: an hour while it is still to come, a few
/// minutes once a puck has dropped. A game the feed still calls upcoming
/// though its start time has passed counts as under way — that is our copy
/// being behind, and serving it for another hour is how a date would sit at
/// 0-0 all evening.
fn fixture_ttl(games: &[DayGame], now: DateTime<Utc>) -> Duration {
    let in_play = games.iter().any(|game| {
        matches!(game.state, GameState::Live)
            || (matches!(game.state, GameState::Upcoming) && game.should_have_started(now))
    });

    if in_play {
        LIVE_CACHE_TTL
    } else {
        SCHEDULE_CACHE_TTL
    }
}

/// A date's fixture list and when it was read, keyed by date.
///
/// The games rather than the teams: the pick screen needs who plays whom, and
/// the team list a pick is validated against is derived from them, so one
/// upstream read serves both.
type FixtureCache = HashMap<String, (Instant, Vec<DayGame>)>;

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

        if !day.games.is_empty() {
            self.fixtures
                .write()
                .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
                .insert(date.to_string(), (Instant::now(), day.games.clone()));
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
        Ok(teams_of(&self.games_on(date).await?))
    }

    /// The date's games, for showing who plays whom and what the score was.
    ///
    /// Served from the same cache as [`Self::teams_playing_on`], so asking for
    /// both on one pick screen is still one call upstream.
    pub async fn games_on(&self, date: &str) -> Result<Vec<DayGame>> {
        if let Some(games) = self.cached_fixtures(date)? {
            return Ok(games);
        }

        Ok(self.day_results(date).await?.games)
    }

    fn cached(&self, date: &str) -> Result<Option<DayResults>> {
        Ok(self
            .settled_days
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .get(date)
            .cloned())
    }

    fn cached_fixtures(&self, date: &str) -> Result<Option<Vec<DayGame>>> {
        // A finished day's games are already final, so they never expire.
        if let Some(day) = self.cached(date)? {
            return Ok(Some(day.games));
        }

        Ok(self
            .fixtures
            .read()
            .map_err(|e| AppError::RwLockError { msg: e.to_string() })?
            .get(date)
            .filter(|(fetched_at, games)| fetched_at.elapsed() < fixture_ttl(games, Utc::now()))
            .map(|(_, games)| games.clone()))
    }
}

/// Turn the league's scoreboard into the per-team result a pool reads.
fn reduce(scoreboard: Scoreboard) -> DayResults {
    let mut day = DayResults::default();

    for game in scoreboard.games {
        let home = game.home_team.id;
        let away = game.away_team.id;

        let (home_result, away_result) = outcome_of(&game);
        day.results.insert(home, home_result);
        day.results.insert(away, away_result);

        day.games.push(DayGame {
            home_team_id: home,
            away_team_id: away,
            home_score: game.home_team.score,
            away_score: game.away_team.score,
            state: state_of(&game.game_state),
            start_time_utc: game.start_time_utc.clone(),
            period: game.period_descriptor.number,
            period_type: game.period_descriptor.period_type.clone(),
            time_remaining: game.clock.time_remaining.clone(),
            in_intermission: game.clock.in_intermission,
        });
    }

    day.teams_playing = teams_of(&day.games);

    day
}

/// Where a game stands, from the league's name for it.
///
/// An unknown state reads as upcoming: this decides what is displayed, and
/// showing a game as not started is the harmless way to be wrong. What a pick
/// is worth is decided by [`outcome_of`], which treats anything it does not
/// recognise as unresolved.
fn state_of(game_state: &str) -> GameState {
    if FINISHED_STATES.contains(&game_state) {
        return GameState::Final;
    }
    if ABANDONED_STATES.contains(&game_state) {
        return GameState::Abandoned;
    }
    if LIVE_STATES.contains(&game_state) {
        return GameState::Live;
    }
    GameState::Upcoming
}

/// Everybody with a game, once, sorted — what a pick is validated against.
fn teams_of(games: &[DayGame]) -> Vec<u32> {
    let mut teams: Vec<u32> = games
        .iter()
        .flat_map(|game| [game.home_team_id, game.away_team_id])
        .collect();

    teams.sort_unstable();
    teams.dedup();
    teams
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

    fn fixture(home: u32, away: u32) -> DayGame {
        DayGame {
            home_team_id: home,
            away_team_id: away,
            home_score: None,
            away_score: None,
            state: GameState::Upcoming,
            start_time_utc: None,
            period: None,
            period_type: None,
            time_remaining: None,
            in_intermission: false,
        }
    }

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
            start_time_utc: None,
            clock: ScoreboardClock::default(),
            period_descriptor: ScoreboardPeriod::default(),
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
        let day = reduce(Scoreboard {
            games: vec![game("OFF", (10, Some(4)), (8, Some(1)))],
        });
        schedule.record_day("2026-10-03", day.clone()).unwrap();

        // Served from the settled-day copy rather than the TTL'd one: a played
        // Saturday's fixture list cannot change, so ageing it out would only
        // buy a pointless refetch.
        assert_eq!(
            schedule.cached_fixtures("2026-10-03").unwrap(),
            Some(day.games.clone())
        );
        assert_eq!(day.games[0].state, GameState::Final);
    }

    #[test]
    fn the_leagues_close_game_state_is_still_live() {
        // `CRIT` is what the league calls a close game late on; treating it as
        // anything else would stop the score updating exactly when it matters.
        let day = reduce(Scoreboard {
            games: vec![game("CRIT", (10, Some(2)), (8, Some(2)))],
        });

        assert_eq!(day.games[0].state, GameState::Live);
    }

    #[test]
    fn a_state_the_league_adds_later_reads_as_upcoming() {
        let day = reduce(Scoreboard {
            games: vec![game("SOMETHING_NEW", (10, None), (8, None))],
        });

        assert_eq!(day.games[0].state, GameState::Upcoming);
        // And it still decides nothing, which is the half that matters.
        assert_eq!(day.result_for(10), TeamResult::Unresolved);
    }

    #[test]
    fn a_live_games_clock_and_period_come_through() {
        let body = r#"{
            "games": [{
              "gameState": "LIVE",
              "awayTeam": {"id": 8, "score": 1},
              "homeTeam": {"id": 10, "score": 2},
              "startTimeUTC": "2026-10-10T23:00:00Z",
              "clock": {"timeRemaining": "12:34", "inIntermission": false},
              "periodDescriptor": {"number": 2, "periodType": "REG"}
            }]
        }"#;

        let day = reduce(serde_json::from_str(body).unwrap());
        let fixture = &day.games[0];

        assert_eq!(fixture.state, GameState::Live);
        assert_eq!(fixture.period, Some(2));
        assert_eq!(fixture.time_remaining, Some("12:34".to_string()));
        assert!(!fixture.in_intermission);
        assert_eq!(fixture.home_score, Some(2));
    }

    #[test]
    fn a_game_settled_in_overtime_says_so() {
        let body = r#"{
            "games": [{
              "gameState": "OFF",
              "awayTeam": {"id": 8, "score": 3},
              "homeTeam": {"id": 10, "score": 4},
              "periodDescriptor": {"number": 4, "periodType": "OT"}
            }]
        }"#;

        let day = reduce(serde_json::from_str(body).unwrap());

        assert_eq!(day.games[0].period_type, Some("OT".to_string()));
        assert_eq!(day.result_for(10), TeamResult::Won);
    }

    #[test]
    fn an_intermission_is_reported_as_one() {
        let body = r#"{
            "games": [{
              "gameState": "LIVE",
              "awayTeam": {"id": 8, "score": 1},
              "homeTeam": {"id": 10, "score": 1},
              "clock": {"timeRemaining": "15:00", "inIntermission": true},
              "periodDescriptor": {"number": 1, "periodType": "REG"}
            }]
        }"#;

        let day = reduce(serde_json::from_str(body).unwrap());

        // The clock is counting down the intermission, not the period, so the
        // screen has to be able to tell.
        assert!(day.games[0].in_intermission);
    }

    #[test]
    fn a_scoreboard_without_a_clock_or_a_period_still_parses() {
        // The league omits both on a game far enough out, and a shape change
        // in them must not take the pick screen down.
        let body = r#"{
            "games": [{
              "gameState": "FUT",
              "awayTeam": {"id": 8},
              "homeTeam": {"id": 10}
            }]
        }"#;

        let day = reduce(serde_json::from_str(body).unwrap());

        assert_eq!(day.games[0].state, GameState::Upcoming);
        assert_eq!(day.games[0].period, None);
        assert_eq!(day.games[0].time_remaining, None);
    }

    /*
    The cache TTL is what decides whether a "live" score is actually live.

    An hour-old score shown as live would be worse than no score at all, so
    which TTL a day gets is worth pinning down.
    */
    mod fixture_ttl {
        use super::*;

        fn at(state: GameState, start: Option<&str>) -> Vec<DayGame> {
            vec![DayGame {
                state,
                start_time_utc: start.map(str::to_string),
                ..fixture(10, 8)
            }]
        }

        fn now() -> DateTime<Utc> {
            DateTime::parse_from_rfc3339("2026-10-10T23:30:00Z")
                .unwrap()
                .with_timezone(&Utc)
        }

        #[test]
        fn a_day_still_to_come_is_kept_for_the_hour() {
            let games = at(GameState::Upcoming, Some("2026-10-11T23:00:00Z"));

            assert_eq!(fixture_ttl(&games, now()), SCHEDULE_CACHE_TTL);
        }

        #[test]
        fn a_day_with_a_game_under_way_is_kept_for_seconds() {
            let games = at(GameState::Live, Some("2026-10-10T23:00:00Z"));

            assert_eq!(fixture_ttl(&games, now()), LIVE_CACHE_TTL);
        }

        #[test]
        fn a_game_past_its_start_time_counts_as_under_way() {
            // The feed still calls it upcoming, which means our copy is behind.
            // Holding it for an hour is how a date sits at 0-0 all evening.
            let games = at(GameState::Upcoming, Some("2026-10-10T23:00:00Z"));

            assert_eq!(fixture_ttl(&games, now()), LIVE_CACHE_TTL);
        }

        #[test]
        fn a_finished_day_does_not_need_watching() {
            let games = at(GameState::Final, Some("2026-10-10T17:00:00Z"));

            assert_eq!(fixture_ttl(&games, now()), SCHEDULE_CACHE_TTL);
        }

        #[test]
        fn one_live_game_is_enough_to_watch_the_whole_day() {
            let mut games = at(GameState::Final, Some("2026-10-10T17:00:00Z"));
            games.extend(at(GameState::Live, Some("2026-10-10T23:00:00Z")));

            assert_eq!(fixture_ttl(&games, now()), LIVE_CACHE_TTL);
        }

        #[test]
        fn a_game_with_no_start_time_is_taken_at_the_feeds_word() {
            let games = at(GameState::Upcoming, None);

            assert_eq!(fixture_ttl(&games, now()), SCHEDULE_CACHE_TTL);
        }

        #[test]
        fn an_unparseable_start_time_is_taken_at_the_feeds_word() {
            let games = at(GameState::Upcoming, Some("soon"));

            assert_eq!(fixture_ttl(&games, now()), SCHEDULE_CACHE_TTL);
        }
    }

    #[test]
    fn a_live_days_fixtures_go_stale_in_minutes_not_hours() {
        let schedule = NhlScheduleService::new();

        // Cached ten minutes ago: well inside the hour a day still to come
        // would get, well past the few minutes a live one does.
        schedule.fixtures.write().unwrap().insert(
            "2026-10-10".to_string(),
            (
                Instant::now() - Duration::from_secs(10 * 60),
                vec![DayGame {
                    state: GameState::Live,
                    ..fixture(10, 8)
                }],
            ),
        );

        assert_eq!(schedule.cached_fixtures("2026-10-10").unwrap(), None);
    }

    #[test]
    fn a_live_days_fixtures_are_served_inside_the_interval() {
        let schedule = NhlScheduleService::new();

        // The other half of the rule: a live day is refetched often, not on
        // every single request. One read per interval is what a pool of
        // hundreds watching the same game costs upstream.
        schedule.fixtures.write().unwrap().insert(
            "2026-10-10".to_string(),
            (
                Instant::now() - Duration::from_secs(30),
                vec![DayGame {
                    state: GameState::Live,
                    ..fixture(10, 8)
                }],
            ),
        );

        assert!(schedule.cached_fixtures("2026-10-10").unwrap().is_some());
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
                vec![fixture(10, 8)],
            ),
        );

        assert_eq!(schedule.cached_fixtures("2026-10-03").unwrap(), None);
    }

    #[test]
    fn a_fresh_fixture_list_is_served() {
        let schedule = NhlScheduleService::new();

        schedule.fixtures.write().unwrap().insert(
            "2026-10-03".to_string(),
            (Instant::now(), vec![fixture(10, 8)]),
        );

        assert_eq!(
            schedule.cached_fixtures("2026-10-03").unwrap(),
            Some(vec![fixture(10, 8)])
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
    fn a_finished_game_keeps_the_matchup_and_the_score_it_was_read_from() {
        let day = reduce(Scoreboard {
            games: vec![game("OFF", (10, Some(4)), (8, Some(1)))],
        });

        assert_eq!(day.games.len(), 1);
        assert_eq!(day.games[0].home_team_id, 10);
        assert_eq!(day.games[0].away_team_id, 8);
        assert_eq!(day.games[0].home_score, Some(4));
        assert_eq!(day.games[0].away_score, Some(1));
        assert_eq!(day.games[0].state, GameState::Final);
    }

    #[test]
    fn a_game_still_to_come_keeps_the_matchup_without_a_score() {
        let day = reduce(Scoreboard {
            games: vec![game("FUT", (10, None), (8, None))],
        });

        let fixture = &day.games[0];
        assert_eq!((fixture.home_team_id, fixture.away_team_id), (10, 8));
        assert_eq!(fixture.home_score, None);
        assert_eq!(fixture.state, GameState::Upcoming);
    }

    #[test]
    fn a_game_under_way_is_not_reported_as_final() {
        // The league scores a live game, and the screen must not show that as
        // the result: the pool has not settled anything yet.
        let day = reduce(Scoreboard {
            games: vec![game("LIVE", (10, Some(2)), (8, Some(1)))],
        });

        assert_eq!(day.games[0].home_score, Some(2));
        assert_eq!(day.games[0].state, GameState::Live);
        assert_eq!(day.result_for(10), TeamResult::Unresolved);
    }

    #[test]
    fn a_postponed_game_is_still_a_matchup_but_not_a_final_one() {
        let day = reduce(Scoreboard {
            games: vec![game("PPD", (10, None), (8, None))],
        });

        assert_eq!(day.games.len(), 1);
        assert_eq!(day.games[0].state, GameState::Abandoned);
    }

    #[test]
    fn a_days_games_answer_the_team_list_a_pick_is_validated_against() {
        // The screen's matchups and the list a pick is checked against are the
        // same read, so they cannot disagree about who is playing.
        let day = reduce(Scoreboard {
            games: vec![
                game("OFF", (10, Some(4)), (8, Some(1))),
                game("FUT", (6, None), (14, None)),
            ],
        });

        assert_eq!(day.teams_playing, teams_of(&day.games));
        assert_eq!(day.teams_playing, vec![6, 8, 10, 14]);
    }

    #[tokio::test]
    async fn a_recorded_day_answers_its_matchups_too() {
        let schedule = NhlScheduleService::new();
        schedule
            .record_day(
                "2026-10-03",
                reduce(Scoreboard {
                    games: vec![game("OFF", (10, Some(4)), (8, Some(1)))],
                }),
            )
            .unwrap();

        let games = schedule.games_on("2026-10-03").await.unwrap();

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].home_score, Some(4));
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
