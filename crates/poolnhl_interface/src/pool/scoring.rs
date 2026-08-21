//! Scoring lines and per-day rosters.
//!
//! These types are the currency between the shared `day_leaders` collection and
//! a pool's ranking: [`DayScores`] is the compact projection of one day's stats,
//! and [`DailyRosterPoints`] applies a pool's [`PoolSettings`] to the lineup a
//! participant iced that day.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::daily_leaders::model::DailyLeaders;
use crate::pool::model::{GoaliesSettings, PoolSettings, SkaterSettings};

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DailyRosterPoints {
    pub roster: Roster,
    pub is_cumulated: bool,
}

impl DailyRosterPoints {
    /// Whether no rostered player has a scoring line for that day.
    ///
    /// True for a day the league did not play (breaks, off days): every slot is
    /// `None`, so the day adds nothing to the participant's points or games.
    pub fn is_scoreless(&self) -> bool {
        self.roster.F.values().all(Option::is_none)
            && self.roster.D.values().all(Option::is_none)
            && self.roster.G.values().all(Option::is_none)
    }

    pub fn get_total_points(
        &self,
        pool_settings: &PoolSettings,
        forwards_points: &mut HashMap<String, (u16, u16)>,
        defenders_points: &mut HashMap<String, (u16, u16)>,
        goalies_points: &mut HashMap<String, (u16, u16)>,
    ) -> (u16, u16) {
        let mut total_points = 0;
        let mut number_of_games = 0;

        // Forwards
        for (player_id, skater_points) in &self.roster.F {
            if let Some(skater_points) = skater_points {
                let daily_points = skater_points.get_total_points(&pool_settings.forwards_settings);
                total_points += daily_points;
                number_of_games += 1;
                if let Some((points, number_of_games)) = forwards_points.get_mut(player_id) {
                    *points += daily_points;
                    *number_of_games += 1;
                } else {
                    forwards_points.insert(player_id.clone(), (daily_points, 1));
                }
            }
        }

        // Defenders
        for (player_id, skater_points) in &self.roster.D {
            if let Some(skater_points) = skater_points {
                let daily_points = skater_points.get_total_points(&pool_settings.defense_settings);
                total_points += daily_points;
                number_of_games += 1;

                if let Some((points, number_of_games)) = defenders_points.get_mut(player_id) {
                    *points += daily_points;
                    *number_of_games += 1;
                } else {
                    defenders_points.insert(player_id.clone(), (daily_points, 1));
                }
            }
        }

        // Goalies
        for (player_id, goalie_points) in &self.roster.G {
            if let Some(goalie_points) = goalie_points {
                let daily_points = goalie_points.get_total_points(&pool_settings.goalies_settings);
                total_points += daily_points;
                number_of_games += 1;

                if let Some((points, number_of_games)) = goalies_points.get_mut(player_id) {
                    *points += daily_points;
                    *number_of_games += 1;
                } else {
                    goalies_points.insert(player_id.clone(), (daily_points, 1));
                }
            }
        }

        (total_points, number_of_games)
    }
}
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Roster {
    pub F: HashMap<String, Option<SkaterPoints>>,
    pub D: HashMap<String, Option<SkaterPoints>>,
    pub G: HashMap<String, Option<GoalyPoints>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct SkaterPoints {
    pub G: u8,
    pub A: u8,
    pub SOG: Option<u8>,
}

impl SkaterPoints {
    pub fn get_total_points(&self, skater_settings: &SkaterSettings) -> u16 {
        let mut total_points = 0;

        total_points += self.G as u16 * skater_settings.points_per_goals as u16
            + self.A as u16 * skater_settings.points_per_assists as u16;

        if let Some(shootout_goal) = self.SOG {
            total_points += shootout_goal as u16 * skater_settings.points_per_shootout_goals as u16;
        }

        if self.G >= 3 {
            total_points += skater_settings.points_per_hattricks as u16;
        }

        total_points
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct GoalyPoints {
    pub G: u8,
    pub A: u8,
    pub W: bool,
    pub SO: bool,
    pub OT: bool,
}

impl GoalyPoints {
    pub fn get_total_points(&self, goalies_settings: &GoaliesSettings) -> u16 {
        let mut total_points = 0;
        total_points += self.G as u16 * goalies_settings.points_per_goals as u16
            + self.A as u16 * goalies_settings.points_per_assists as u16;

        if self.W {
            total_points += goalies_settings.points_per_wins as u16;
        }

        if self.SO {
            total_points += goalies_settings.points_per_shutouts as u16;
        }

        if self.OT {
            total_points += goalies_settings.points_per_overtimes as u16;
        }

        total_points
    }
}

/// Per-player scoring lines for a single date, derived from the shared
/// `day_leaders` collection. This is the compact, scoring-only projection that
/// replaces the per-player breakdown previously duplicated in every pool's
/// `score_by_day`: points are computed on demand from here instead of being
/// stored per pool. See the pool score redesign.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DayScores {
    pub skaters: HashMap<u32, SkaterPoints>,
    pub goalies: HashMap<u32, GoalyPoints>,
    /// Every player that dressed that day, scoring or not. `day_leaders` only
    /// lists a player in `skaters`/`goalies` once they registered a stat, so
    /// this is the only way to tell "played and did not score" apart from "did
    /// not play" — which is what the games-played tally counts.
    pub played: HashSet<u32>,
}

impl DayScores {
    /// Project a raw `day_leaders` document into the compact scoring lines.
    ///
    /// The goalie bonuses are derived exactly as the rest of the stack does: a
    /// win is `decision == "W"`, a shutout is a win with a perfect save
    /// percentage, and an overtime/shootout loss is `decision == "O"`.
    pub fn from_daily_leaders(daily_leaders: &DailyLeaders) -> Self {
        let skaters = daily_leaders
            .skaters
            .iter()
            .map(|s| {
                (
                    s.id,
                    SkaterPoints {
                        G: s.stats.goals,
                        A: s.stats.assists,
                        SOG: Some(s.stats.shootoutGoals),
                    },
                )
            })
            .collect();

        let goalies = daily_leaders
            .goalies
            .iter()
            .map(|g| {
                let is_win = g.stats.decision.as_deref() == Some("W");
                let is_shutout = is_win && matches!(g.stats.savePercentage, Some(sp) if sp >= 1.0);
                let is_overtime = g.stats.decision.as_deref() == Some("O");
                (
                    g.id,
                    GoalyPoints {
                        G: g.stats.goals,
                        A: g.stats.assists,
                        W: is_win,
                        SO: is_shutout,
                        OT: is_overtime,
                    },
                )
            })
            .collect();

        Self {
            skaters,
            goalies,
            played: daily_leaders.played.iter().copied().collect(),
        }
    }

    /// Build a participant's per-day [`Roster`] for the given lineup, sourcing
    /// each player's points from these day scores. A rostered player that did
    /// not play maps to `None`; one that played without registering a stat maps
    /// to an all-zero line, matching the shape the legacy `score_by_day`
    /// produced so the existing scoring and ranking logic
    /// ([`DailyRosterPoints::get_total_points`], [`PoolContext::get_final_rank`])
    /// can be reused verbatim. That distinction is what makes games played add
    /// up: a scoreless game still counts as a game.
    pub fn roster_for(&self, forwards: &[u32], defense: &[u32], goalies: &[u32]) -> Roster {
        Roster {
            F: forwards.iter().map(|id| self.skater_line(id)).collect(),
            D: defense.iter().map(|id| self.skater_line(id)).collect(),
            G: goalies.iter().map(|id| self.goaly_line(id)).collect(),
        }
    }

    fn skater_line(&self, id: &u32) -> (String, Option<SkaterPoints>) {
        let points = self.skaters.get(id).cloned().or_else(|| {
            self.played.contains(id).then_some(SkaterPoints {
                G: 0,
                A: 0,
                SOG: Some(0),
            })
        });
        (id.to_string(), points)
    }

    fn goaly_line(&self, id: &u32) -> (String, Option<GoalyPoints>) {
        let points = self.goalies.get(id).cloned().or_else(|| {
            self.played.contains(id).then_some(GoalyPoints {
                G: 0,
                A: 0,
                W: false,
                SO: false,
                OT: false,
            })
        });
        (id.to_string(), points)
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SkaterPoolPoints {
    pub G: u8,
    pub A: u8,
    pub HT: u8,
    pub SOG: u8,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoalyPoolPoints {
    pub G: u8,
    pub A: u8,
    pub W: u8,
    pub SO: u8,
    pub OT: u8,
}
