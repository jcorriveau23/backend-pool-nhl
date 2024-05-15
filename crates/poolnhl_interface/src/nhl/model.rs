use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Name {
    pub default: String,
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

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub enum PeriodType {
    REG,
    OT,
    SO,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PeriodDescriptor {
    pub number: u32,
    pub periodType: PeriodType,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PeriodScoring {
    pub periodDescriptor: PeriodDescriptor,
    pub goals: Vec<Goal>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ShootoutResult {
    save,
    goal,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct ShootoutInfo {
    pub sequence: u32,
    pub playerId: u32,
    pub teamAbbrev: String,
    pub firstName: String,
    pub lastName: String,
    pub result: ShootoutResult,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PeriodScore {
    pub away: u32,
    pub home: u32,
    pub periodDescriptor: PeriodDescriptor,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct TotalScore {
    pub away: u32,
    pub home: u32,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Linescore {
    pub byPeriod: Vec<PeriodScore>,
    pub totals: TotalScore,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PeriodShots {
    pub periodDescriptor: PeriodDescriptor,
    pub away: u32,
    pub home: u32,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct GameSummary {
    pub linescore: Linescore,
    pub scoring: Vec<PeriodScoring>,
    pub shootout: Vec<ShootoutInfo>,
    pub teamGameStats: Vec<TeamGameStats>,
    pub shotsByPeriod: Vec<PeriodShots>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct TeamGameStats {
    pub category: String,
    // pub awayValue: StringOrU32,
    // pub homeValue: StringOrU32,
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
    pub powerPlayGoals: u32,
    pub shots: u32,
    pub faceoffWinningPctg: f32,
    pub toi: String,
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
pub struct GameBoxScore {
    pub id: u32,
    pub awayTeam: TeamInfo,
    pub homeTeam: TeamInfo,

    pub playerByGameStats: PlayerByGameStats,
}
