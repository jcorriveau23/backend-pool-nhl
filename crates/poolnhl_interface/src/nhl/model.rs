use serde::{Deserialize, Serialize};


#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Name {
    pub default: String
}
// Game Landing information

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct TeamInfo {
    pub id: u32,
    pub logo: String,
    pub score: Option<u32>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Assist {
    pub playerId: u32,
    pub firstName: Name,
    pub lastName: Name,
    pub assistsToDate: u32,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Goal {
    pub strength: String,
    pub playerId: u32,
    pub firstName: Name,
    pub lastName: Name,
    pub headshot: String,
    pub teamAbbrev: Name,
    pub goalsToDate: u32,
    pub awayScore: u32,
    pub homeScore: u32,
    pub timeInPeriod: String,
    pub assists: Vec<Assist>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PeriodScoring {
    pub period: u32,
    pub goals: Vec<Goal>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct ShootoutInfo {
    pub sequence: u32,
    pub playerId: u32,
    pub teamAbbrev: String,
    pub firstName: String,
    pub lastName: String,
    pub result: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GameSummary {
    pub scoring: Vec<PeriodScoring>,
    pub shootout: Vec<ShootoutInfo>,
    pub teamGameStats: Vec<TeamGameStats>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct TeamGameStats {
    pub category: String,
    pub awayValue: String,
    pub homeValue: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GameLanding {
    pub id: u32,
    pub awayTeam: TeamInfo,
    pub homeTeam: TeamInfo,
    pub summary: GameSummary,
}

// Daily Games information
#[derive(Debug, Deserialize, Serialize)]
pub enum GameState {
    OFF,
    LIVE,
    FUT,
    PPD,
    PRE,
    CRIT,
    FINAL,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct TimeRemaining {
    pub timeRemaining: String,
    pub running: bool,
    pub inIntermission: bool,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PeriodDescriptor {
    pub number: u32,
    pub periodType: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Game {
    pub id: u32,
    pub startTimeUTC: String,
    pub gameState: GameState,
    pub awayTeam: TeamInfo,
    pub homeTeam: TeamInfo,

    pub period: Option<u32>,
    pub periodDescriptor: Option<PeriodDescriptor>,
    pub clock: Option<TimeRemaining>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DailyGames {
    pub date: String,
    pub games: Vec<Game>,
}

// Box score information
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PlayerName {
    pub default: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct SkaterStats {
    pub playerId: u32,
    pub sweaterNumber: u32,
    pub name: PlayerName,
    pub position: String,
    pub goals: u32,
    pub assists: u32,
    pub points: u32,
    pub plusMinus: i32,
    pub pim: Option<u32>,
    pub hits: u32,
    pub blockedShots: u32,
    pub powerPlayGoals: u32,
    pub powerPlayPoints: u32,
    pub shorthandedGoals: u32,
    pub shPoints: u32,
    pub shots: u32,
    pub faceoffs: String,
    pub faceoffWinningPctg: f32,
    pub toi: String,
    pub powerPlayToi: String,
    pub shorthandedToi: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GoalieStats {
    pub playerId: u32,
    pub sweaterNumber: u32,
    pub name: PlayerName,
    pub position: String,
    pub evenStrengthShotsAgainst: String,
    pub powerPlayShotsAgainst: String,
    pub shorthandedShotsAgainst: String,
    pub saveShotsAgainst: String,
    pub savePctg: Option<String>,
    pub evenStrengthGoalsAgainst: u32,
    pub powerPlayGoalsAgainst: u32,
    pub shorthandedGoalsAgainst: u32,
    pub pim: Option<u32>,
    pub goalsAgainst: u32,
    pub toi: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct TeamBoxScore {
    pub forwards: Vec<SkaterStats>,
    pub defense: Vec<SkaterStats>,
    pub goalies: Vec<GoalieStats>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PlayerByGameStats {
    pub awayTeam: TeamBoxScore,
    pub homeTeam: TeamBoxScore,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct BoxScore {
    pub playerByGameStats: PlayerByGameStats,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GameBoxScore {
    pub id: u32,
    pub awayTeam: TeamInfo,
    pub homeTeam: TeamInfo,

    pub boxscore: BoxScore,
}
