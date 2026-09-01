use super::*;
use crate::draft::model::RoomUser;
use crate::pool::scoring::{DailyRosterPoints, DayScores, GoalyPoints, Roster, SkaterPoints};

const OWNER: &str = "owner";
const USER_2: &str = "user-2";
const USER_3: &str = "user-3";
const PARTICIPANTS: [&str; 3] = [OWNER, USER_2, USER_3];

fn player(id: u32, position: Position, salary_cap: Option<f64>) -> PlayerInfo {
    PlayerInfo {
        active: true,
        id,
        name: format!("player-{}", id),
        team: None,
        position,
        age: None,
        salary_cap,
        contract_expiration_season: None,
        game_played: None,
        goals: None,
        assists: None,
        points: None,
        points_per_game: None,
        goal_against_average: None,
        save_percentage: None,
        saves: None,
        shots: None,
        wins: None,
        ot: None,
    }
}

// A small roster format (2 forwards, 1 defender, 1 goalie, 1 reservist)
// keeps the fixtures readable.
fn small_settings() -> PoolSettings {
    let mut settings = PoolSettings::new();
    settings.number_poolers = 3;
    settings.number_forwards = 2;
    settings.number_defenders = 1;
    settings.number_goalies = 1;
    settings.number_reservists = 1;
    settings
}

fn pool_user(id: &str) -> PoolUser {
    PoolUser {
        id: id.to_string(),
        name: id.to_string(),
        is_owned: true,
    }
}

fn room_user(id: &str) -> RoomUser {
    RoomUser {
        id: id.to_string(),
        name: id.to_string(),
        email: Some(format!("{}@example.com", id)),
        is_ready: true,
    }
}

// An InProgress pool with full rosters. Participant number `i` (1-based)
// owns forwards i*100+1 and i*100+2, defender i*100+11, goalie i*100+21
// and reservist (a forward) i*100+31. Every player has a 1M$ salary.
fn in_progress_pool() -> Pool {
    let mut pool = Pool::new("test-pool", OWNER, &small_settings());
    pool.participants = PARTICIPANTS.iter().map(|id| pool_user(id)).collect();
    pool.status = PoolState::InProgress;

    let ids: Vec<String> = PARTICIPANTS.iter().map(|id| id.to_string()).collect();
    let mut context = PoolContext::new(&ids);
    for (i, participant) in PARTICIPANTS.iter().enumerate() {
        let base = (i as u32 + 1) * 100;
        let roster = context.pooler_roster.get_mut(*participant).unwrap();
        roster.chosen_forwards = vec![base + 1, base + 2];
        roster.chosen_defenders = vec![base + 11];
        roster.chosen_goalies = vec![base + 21];
        roster.chosen_reservists = vec![base + 31];
        for (id, position) in [
            (base + 1, Position::F),
            (base + 2, Position::F),
            (base + 11, Position::D),
            (base + 21, Position::G),
            (base + 31, Position::F),
        ] {
            context
                .players
                .insert(id.to_string(), player(id, position, Some(1_000_000.0)));
        }
    }
    pool.context = Some(context);
    pool
}

fn trade(proposed_by: &str, ask_to: &str, from_players: Vec<u32>, to_players: Vec<u32>) -> Trade {
    Trade {
        proposed_by: proposed_by.to_string(),
        ask_to: ask_to.to_string(),
        from_items: TradeItems {
            players: from_players,
            picks: Vec::new(),
        },
        to_items: TradeItems {
            players: to_players,
            picks: Vec::new(),
        },
        status: TradeStatus::NEW,
        id: 0,
        date_created: 0,
        date_accepted: 0,
    }
}

// The deadline day itself is still a valid trading day.
fn deadline() -> NaiveDate {
    NaiveDate::parse_from_str(TRADE_DEADLINE_DATE, "%Y-%m-%d").unwrap()
}

fn possesses(pool: &Pool, user_id: &str, player_id: u32) -> bool {
    pool.context.as_ref().unwrap().pooler_roster[user_id].validate_player_possession(player_id)
}

// -------------------------------------------------------------------
// Pool status validation
// -------------------------------------------------------------------

#[test]
fn validate_pool_status_rejects_a_different_status() {
    let pool = in_progress_pool();
    assert!(pool.validate_pool_status(&PoolState::InProgress).is_ok());
    assert!(pool.validate_pool_status(&PoolState::Created).is_err());
    assert!(pool.validate_pool_status(&PoolState::Final).is_err());
}

#[test]
fn validate_pool_status_any_accepts_every_listed_status() {
    let pool = in_progress_pool();
    assert!(
        pool.validate_pool_status_any(&[PoolState::InProgress, PoolState::Dynasty])
            .is_ok()
    );
    assert!(
        pool.validate_pool_status_any(&[PoolState::Created, PoolState::Dynasty])
            .is_err()
    );
}

// -------------------------------------------------------------------
// Settings update
// -------------------------------------------------------------------

#[test]
fn a_started_pool_updates_its_settings_but_not_its_roster_shape() {
    for status in [PoolState::InProgress, PoolState::Dynasty] {
        let mut pool = in_progress_pool();
        pool.status = status;

        // Scoring, salary cap, assistants and roster modification dates stay
        // open for the whole life of the pool.
        let mut settings = pool.settings.clone();
        settings.forwards_settings.points_per_goals = 5;
        settings.salary_cap = Some(90_000_000.0);
        settings.assistants = vec![USER_2.to_string()];
        settings.roster_modification_date = vec!["2024-01-01".to_string()];
        assert!(
            pool.can_update_started_pool_settings(OWNER, &settings)
                .is_ok()
        );

        // The roster shape is frozen, the rosters are already drafted.
        let mut settings = pool.settings.clone();
        settings.number_forwards += 1;
        assert!(
            pool.can_update_started_pool_settings(OWNER, &settings)
                .is_err()
        );

        // And so are the dynasty rules.
        let mut settings = pool.settings.clone();
        settings.dynasty_settings = Some(DynastySettings {
            next_season_number_players_protected: 8,
            tradable_picks: 3,
            past_season_pool_name: Vec::new(),
            next_season_pool_name: None,
        });
        assert!(
            pool.can_update_started_pool_settings(OWNER, &settings)
                .is_err()
        );
    }
}

#[test]
fn only_the_owner_and_the_assistants_update_the_settings() {
    let mut pool = in_progress_pool();
    let settings = pool.settings.clone();

    assert!(
        pool.can_update_started_pool_settings(USER_2, &settings)
            .is_err()
    );

    pool.settings.assistants = vec![USER_2.to_string()];
    let settings = pool.settings.clone();
    assert!(
        pool.can_update_started_pool_settings(USER_2, &settings)
            .is_ok()
    );
}

#[test]
fn a_pool_that_has_not_started_or_is_over_keeps_its_settings() {
    for status in [PoolState::Created, PoolState::Draft, PoolState::Final] {
        let mut pool = in_progress_pool();
        pool.status = status;
        let settings = pool.settings.clone();

        assert!(
            pool.can_update_started_pool_settings(OWNER, &settings)
                .is_err()
        );
    }
}

// -------------------------------------------------------------------
// Pooler names
// -------------------------------------------------------------------

#[test]
fn the_owner_renames_a_pooler_without_touching_its_id() {
    let mut pool = in_progress_pool();

    pool.update_pooler_name(OWNER, USER_2, "  Raph  ").unwrap();

    let renamed = pool
        .participants
        .iter()
        .find(|user| user.id == USER_2)
        .unwrap();
    // The surrounding whitespace is dropped, everything keyed by the id (the
    // rosters here) still finds the pooler.
    assert_eq!(renamed.name, "Raph");
    assert!(pool.context.unwrap().pooler_roster.contains_key(USER_2));
}

#[test]
fn renaming_a_pooler_is_the_owner_call_alone() {
    let mut pool = in_progress_pool();
    pool.settings.assistants = vec![USER_2.to_string()];

    assert!(pool.update_pooler_name(USER_2, USER_3, "Raph").is_err());
    assert!(pool.update_pooler_name(USER_3, USER_3, "Raph").is_err());
    assert!(pool.update_pooler_name(OWNER, USER_3, "Raph").is_ok());
}

#[test]
fn a_pooler_name_cannot_be_empty_or_too_long() {
    let mut pool = in_progress_pool();

    assert!(pool.update_pooler_name(OWNER, USER_2, "   ").is_err());
    assert!(
        pool.update_pooler_name(OWNER, USER_2, &"a".repeat(MAX_POOLER_NAME_LENGTH + 1))
            .is_err()
    );
    assert!(
        pool.update_pooler_name(OWNER, USER_2, &"a".repeat(MAX_POOLER_NAME_LENGTH))
            .is_ok()
    );
}

#[test]
fn two_poolers_of_a_pool_cannot_share_a_name() {
    let mut pool = in_progress_pool();

    assert!(pool.update_pooler_name(OWNER, USER_2, USER_3).is_err());
    // Renaming a pooler to the name it already carries is not a clash with
    // itself.
    assert!(pool.update_pooler_name(OWNER, USER_2, USER_2).is_ok());
}

#[test]
fn only_a_participant_of_the_pool_can_be_renamed() {
    let mut pool = in_progress_pool();

    assert!(pool.update_pooler_name(OWNER, "stranger", "Raph").is_err());
}

// -------------------------------------------------------------------
// Trades
// -------------------------------------------------------------------

#[test]
fn create_trade_registers_a_new_trade() {
    let mut pool = in_progress_pool();
    let mut new_trade = trade(OWNER, USER_2, vec![131], vec![231]);

    pool.create_trade_at(&mut new_trade, OWNER, deadline())
        .unwrap();

    let trades = pool.trades.as_ref().unwrap();
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].id, 0);
    assert!(matches!(trades[0].status, TradeStatus::NEW));
    assert!(trades[0].date_created > 0);
}

#[test]
fn create_trade_is_rejected_after_the_deadline() {
    let mut pool = in_progress_pool();
    let mut new_trade = trade(OWNER, USER_2, vec![131], vec![231]);

    let result = pool.create_trade_at(&mut new_trade, OWNER, deadline() + Duration::days(1));

    assert!(result.unwrap_err().to_string().contains("deadline"));
}

#[test]
fn create_trade_requires_an_in_progress_pool() {
    let mut pool = in_progress_pool();
    pool.status = PoolState::Created;
    let mut new_trade = trade(OWNER, USER_2, vec![131], vec![231]);

    assert!(
        pool.create_trade_at(&mut new_trade, OWNER, deadline())
            .is_err()
    );
}

#[test]
fn a_proposer_cannot_have_two_pending_trades() {
    let mut pool = in_progress_pool();
    pool.create_trade_at(
        &mut trade(OWNER, USER_2, vec![131], vec![231]),
        OWNER,
        deadline(),
    )
    .unwrap();

    let result = pool.create_trade_at(
        &mut trade(OWNER, USER_3, vec![131], vec![331]),
        OWNER,
        deadline(),
    );

    assert!(result.unwrap_err().to_string().contains("one active trade"));
}

#[test]
fn a_pending_trade_does_not_block_other_proposers() {
    let mut pool = in_progress_pool();
    pool.create_trade_at(
        &mut trade(OWNER, USER_2, vec![131], vec![231]),
        OWNER,
        deadline(),
    )
    .unwrap();

    // user-3 must still be able to propose his own trade.
    let mut other_trade = trade(USER_3, USER_2, vec![331], vec![231]);
    pool.create_trade_at(&mut other_trade, USER_3, deadline())
        .unwrap();

    assert_eq!(pool.trades.as_ref().unwrap().len(), 2);
    assert_eq!(other_trade.id, 1);
}

#[test]
fn creating_a_trade_for_someone_else_requires_privileges() {
    let mut pool = in_progress_pool();

    // user-2 cannot create a trade on behalf of user-3.
    let result = pool.create_trade_at(
        &mut trade(USER_3, OWNER, vec![331], vec![131]),
        USER_2,
        deadline(),
    );
    assert!(result.is_err());

    // The owner can.
    pool.create_trade_at(
        &mut trade(USER_3, OWNER, vec![331], vec![131]),
        OWNER,
        deadline(),
    )
    .unwrap();
}

#[test]
fn create_trade_validates_the_traded_items() {
    let mut pool = in_progress_pool();

    // One empty side.
    let result = pool.create_trade_at(
        &mut trade(OWNER, USER_2, vec![131], vec![]),
        OWNER,
        deadline(),
    );
    assert!(result.unwrap_err().to_string().contains("no items"));

    // More than 5 items on one side.
    let result = pool.create_trade_at(
        &mut trade(OWNER, USER_2, vec![101, 102, 111, 121, 131, 132], vec![231]),
        OWNER,
        deadline(),
    );
    assert!(result.is_err());

    // Proposing a player the proposer does not own.
    let result = pool.create_trade_at(
        &mut trade(OWNER, USER_2, vec![231], vec![201]),
        OWNER,
        deadline(),
    );
    assert!(result.unwrap_err().to_string().contains("possess"));
}

#[test]
fn delete_trade_is_limited_to_the_proposer_or_privileged_users() {
    let mut pool = in_progress_pool();
    let mut new_trade = trade(USER_2, USER_3, vec![231], vec![331]);
    pool.create_trade_at(&mut new_trade, USER_2, deadline())
        .unwrap();

    // Another participant cannot delete it.
    assert!(pool.delete_trade(USER_3, new_trade.id).is_err());

    // The proposer can.
    pool.delete_trade(USER_2, new_trade.id).unwrap();
    assert!(pool.trades.as_ref().unwrap().is_empty());

    // The owner can too.
    let mut second_trade = trade(USER_2, USER_3, vec![231], vec![331]);
    pool.create_trade_at(&mut second_trade, USER_2, deadline())
        .unwrap();
    pool.delete_trade(OWNER, second_trade.id).unwrap();
    assert!(pool.trades.as_ref().unwrap().is_empty());
}

#[test]
fn responding_a_trade_requires_a_24h_delay_for_regular_users() {
    let mut pool = in_progress_pool();
    let mut new_trade = trade(OWNER, USER_2, vec![131], vec![231]);
    pool.create_trade_at(&mut new_trade, OWNER, deadline())
        .unwrap();

    // The trade was just created: the asked user has to wait 24h.
    let result = pool.respond_trade(USER_2, true, new_trade.id);
    assert!(result.unwrap_err().to_string().contains("24h"));

    // Backdate the trade beyond 24h: the asked user can now accept it.
    pool.trades.as_mut().unwrap()[0].date_created = 0;
    pool.respond_trade(USER_2, true, new_trade.id).unwrap();

    // The players moved to the other participant's reservists.
    assert!(possesses(&pool, USER_2, 131));
    assert!(possesses(&pool, OWNER, 231));
    assert!(!possesses(&pool, OWNER, 131));
    assert!(!possesses(&pool, USER_2, 231));
    let trades = pool.trades.as_ref().unwrap();
    assert!(matches!(trades[0].status, TradeStatus::ACCEPTED));
    assert!(trades[0].date_accepted > 0);
}

#[test]
fn the_owner_can_refuse_a_trade_immediately() {
    let mut pool = in_progress_pool();
    let mut new_trade = trade(OWNER, USER_2, vec![131], vec![231]);
    pool.create_trade_at(&mut new_trade, OWNER, deadline())
        .unwrap();

    pool.respond_trade(OWNER, false, new_trade.id).unwrap();

    assert!(matches!(
        pool.trades.as_ref().unwrap()[0].status,
        TradeStatus::REFUSED
    ));
    // Rosters are untouched.
    assert!(possesses(&pool, OWNER, 131));
    assert!(possesses(&pool, USER_2, 231));
}

#[test]
fn only_the_asked_user_or_privileged_users_can_respond() {
    let mut pool = in_progress_pool();
    let mut new_trade = trade(OWNER, USER_2, vec![131], vec![231]);
    pool.create_trade_at(&mut new_trade, OWNER, deadline())
        .unwrap();
    pool.trades.as_mut().unwrap()[0].date_created = 0;

    assert!(pool.respond_trade(USER_3, true, new_trade.id).is_err());
}

#[test]
fn a_responded_trade_cannot_be_responded_again() {
    let mut pool = in_progress_pool();
    let mut new_trade = trade(OWNER, USER_2, vec![131], vec![231]);
    pool.create_trade_at(&mut new_trade, OWNER, deadline())
        .unwrap();
    pool.trades.as_mut().unwrap()[0].date_created = 0;
    pool.respond_trade(USER_2, true, new_trade.id).unwrap();

    assert!(pool.respond_trade(USER_2, true, new_trade.id).is_err());
    assert!(pool.delete_trade(OWNER, new_trade.id).is_err());
}

#[test]
fn an_accepted_trade_moves_picks_to_their_new_owner() {
    let mut pool = in_progress_pool();
    let round: HashMap<String, String> = PARTICIPANTS
        .iter()
        .map(|id| (id.to_string(), id.to_string()))
        .collect();
    pool.context.as_mut().unwrap().tradable_picks = Some(vec![round]);

    let mut new_trade = trade(OWNER, USER_2, vec![], vec![231]);
    new_trade.from_items.picks.push(Pick {
        round: 0,
        from: OWNER.to_string(),
    });
    pool.create_trade_at(&mut new_trade, OWNER, deadline())
        .unwrap();
    pool.trades.as_mut().unwrap()[0].date_created = 0;

    pool.respond_trade(USER_2, true, new_trade.id).unwrap();

    let context = pool.context.as_ref().unwrap();
    assert_eq!(context.tradable_picks.as_ref().unwrap()[0][OWNER], USER_2);
    assert!(possesses(&pool, OWNER, 231));
}

// -------------------------------------------------------------------
// Roster moves
// -------------------------------------------------------------------

#[test]
fn fill_spot_moves_a_reservist_into_a_free_spot() {
    let mut pool = in_progress_pool();
    // Free one forward spot.
    pool.context
        .as_mut()
        .unwrap()
        .pooler_roster
        .get_mut(OWNER)
        .unwrap()
        .chosen_forwards
        .pop();

    pool.fill_spot(OWNER, OWNER, 131).unwrap();

    let roster = &pool.context.as_ref().unwrap().pooler_roster[OWNER];
    assert!(roster.chosen_forwards.contains(&131));
    assert!(roster.chosen_reservists.is_empty());
}

#[test]
fn fill_spot_rejects_a_full_position() {
    let mut pool = in_progress_pool();

    let result = pool.fill_spot(OWNER, OWNER, 131);

    assert!(result.unwrap_err().to_string().contains("no space"));
}

#[test]
fn fill_spot_only_accepts_reservists() {
    let mut pool = in_progress_pool();

    let result = pool.fill_spot(OWNER, OWNER, 101);

    assert!(result.unwrap_err().to_string().contains("reservist"));
}

#[test]
fn fill_spot_enforces_the_salary_cap() {
    let mut pool = in_progress_pool();
    pool.settings.salary_cap = Some(4_000_000.0);
    let context = pool.context.as_mut().unwrap();
    context
        .pooler_roster
        .get_mut(OWNER)
        .unwrap()
        .chosen_forwards
        .pop();
    // The remaining alignment costs 3M$; a 2M$ reservist busts the 4M$ cap.
    context.players.get_mut("131").unwrap().salary_cap = Some(2_000_000.0);

    let result = pool.fill_spot(OWNER, OWNER, 131);

    assert!(result.unwrap_err().to_string().contains("salary cap"));
}

#[test]
fn add_player_requires_privileges() {
    let mut pool = in_progress_pool();
    let new_player = player(999, Position::F, None);

    assert!(pool.add_player(USER_2, USER_2, &new_player).is_err());
}

#[test]
fn add_player_rejects_a_player_already_owned() {
    let mut pool = in_progress_pool();
    let taken_player = player(231, Position::F, None);

    let result = pool.add_player(OWNER, USER_2, &taken_player);

    assert!(result.unwrap_err().to_string().contains("already picked"));
}

#[test]
fn add_player_puts_the_new_player_in_the_reservists() {
    let mut pool = in_progress_pool();
    let new_player = player(999, Position::F, None);

    pool.add_player(OWNER, USER_2, &new_player).unwrap();

    assert!(possesses(&pool, USER_2, 999));
    assert!(pool.context.as_ref().unwrap().players.contains_key("999"));
}

#[test]
fn remove_player_requires_privileges_and_possession() {
    let mut pool = in_progress_pool();

    // A regular participant cannot remove players, even his own.
    assert!(pool.remove_player(USER_2, USER_2, 231).is_err());

    // The owner cannot remove a player from someone who does not own it.
    assert!(pool.remove_player(OWNER, USER_3, 231).is_err());

    // The owner removes an owned player.
    pool.remove_player(OWNER, USER_2, 231).unwrap();
    assert!(!possesses(&pool, USER_2, 231));
}

// -------------------------------------------------------------------
// Roster modifications
// -------------------------------------------------------------------

fn modification_day() -> NaiveDate {
    NaiveDate::parse_from_str("2025-12-01", "%Y-%m-%d").unwrap()
}

#[test]
fn modify_roster_swaps_the_lineup_on_an_allowed_date() {
    let mut pool = in_progress_pool();
    pool.settings.roster_modification_date = vec!["2025-12-01".to_string()];

    pool.modify_roster_at(
        OWNER,
        OWNER,
        &[101, 131],
        &[111],
        &[121],
        &[102],
        modification_day(),
    )
    .unwrap();

    let roster = &pool.context.as_ref().unwrap().pooler_roster[OWNER];
    assert_eq!(roster.chosen_forwards, vec![101, 131]);
    assert_eq!(roster.chosen_reservists, vec![102]);
}

#[test]
fn modify_roster_is_rejected_outside_the_allowed_dates() {
    let mut pool = in_progress_pool();
    // The season must have started for the allowed-dates check to apply.
    pool.season_start = "2025-09-29".to_string();
    pool.settings.roster_modification_date = vec!["2025-12-01".to_string()];

    let result = pool.modify_roster_at(
        OWNER,
        OWNER,
        &[101, 131],
        &[111],
        &[121],
        &[102],
        modification_day() + Duration::days(1),
    );

    assert!(result.unwrap_err().to_string().contains("not allowed"));
}

#[test]
fn modify_roster_is_free_before_the_season_starts() {
    let mut pool = in_progress_pool();
    // No allowed date is configured, but the season has not started.
    let before_season = NaiveDate::parse_from_str("2025-09-01", "%Y-%m-%d").unwrap();

    pool.modify_roster_at(
        OWNER,
        OWNER,
        &[101, 131],
        &[111],
        &[121],
        &[102],
        before_season,
    )
    .unwrap();
}

// Participants arrange the players they have picked while the draft is still
// running, so the room sees the lineup they intend to open with.
#[test]
fn modify_roster_is_allowed_during_the_draft() {
    let mut pool = draft_pool();
    pool.draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();
    pool.draft_player(USER_2, &player(2, Position::F, None))
        .unwrap();
    pool.draft_player(USER_3, &player(3, Position::F, None))
        .unwrap();
    pool.draft_player(USER_3, &player(4, Position::F, None))
        .unwrap();
    pool.draft_player(USER_2, &player(5, Position::F, None))
        .unwrap();

    // user-2 benches the second forward he picked. The roster is still partial,
    // which is fine: the rules only compare it against itself.
    pool.modify_roster_at(USER_2, USER_2, &[2], &[], &[], &[5], modification_day())
        .unwrap();

    let roster = &pool.context.as_ref().unwrap().pooler_roster[USER_2];
    assert_eq!(roster.chosen_forwards, vec![2]);
    assert_eq!(roster.chosen_reservists, vec![5]);
}

#[test]
fn modify_roster_is_rejected_before_the_draft_starts() {
    // A pool still in Created has no roster to arrange.
    let mut pool = Pool::new("draft-pool", OWNER, &small_settings());
    let result = pool.modify_roster_at(OWNER, OWNER, &[], &[], &[], &[], modification_day());

    assert!(result.is_err());
}

// A lineup change made before the season starts does not switch anything
// mid-season, it redefines the lineup the pool opens with. Dating it in the
// past would leave it behind the opening event, and it would never apply.
#[test]
fn a_pre_season_lineup_change_lands_on_the_season_start() {
    let pool = in_progress_pool();

    assert_eq!(pool.lineup_effective_date("2025-09-01"), pool.season_start);
}

#[test]
fn a_lineup_change_during_the_season_keeps_its_own_date() {
    let mut pool = in_progress_pool();
    pool.season_start = "2025-09-29".to_string();

    assert_eq!(pool.lineup_effective_date("2025-12-01"), "2025-12-01");
}

#[test]
fn modify_roster_validates_the_lineup() {
    let mut pool = in_progress_pool();
    pool.settings.roster_modification_date = vec!["2025-12-01".to_string()];
    let today = modification_day();

    // Too many forwards.
    let result = pool.modify_roster_at(OWNER, OWNER, &[101, 102, 131], &[111], &[121], &[], today);
    assert!(result.unwrap_err().to_string().contains("forwards"));

    // Not the same amount of players as before.
    let result = pool.modify_roster_at(OWNER, OWNER, &[101, 102], &[111], &[121], &[], today);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not the same as before")
    );

    // A duplicated player.
    let result = pool.modify_roster_at(OWNER, OWNER, &[101, 101], &[111], &[121], &[131], today);
    assert!(result.unwrap_err().to_string().contains("dupplicated"));

    // A player owned by someone else.
    let result = pool.modify_roster_at(OWNER, OWNER, &[101, 201], &[111], &[121], &[131], today);
    assert!(result.unwrap_err().to_string().contains("possess"));
}

#[test]
fn modify_roster_enforces_the_salary_cap() {
    let mut pool = in_progress_pool();
    pool.settings.roster_modification_date = vec!["2025-12-01".to_string()];
    // 4 aligned players at 1M$ each against a 3.5M$ cap.
    pool.settings.salary_cap = Some(3_500_000.0);

    let result = pool.modify_roster_at(
        OWNER,
        OWNER,
        &[101, 102],
        &[111],
        &[121],
        &[131],
        modification_day(),
    );

    assert!(result.unwrap_err().to_string().contains("salary cap"));
}

// -------------------------------------------------------------------
// Draft
// -------------------------------------------------------------------

fn draft_pool() -> Pool {
    let mut pool = Pool::new("draft-pool", OWNER, &small_settings());
    let room_users: Vec<RoomUser> = PARTICIPANTS.iter().map(|id| room_user(id)).collect();
    let draft_order: Vec<String> = PARTICIPANTS.iter().map(|id| id.to_string()).collect();
    pool.start_draft(OWNER, &room_users, &draft_order).unwrap();
    pool
}

#[test]
fn start_draft_initializes_the_pool() {
    let pool = draft_pool();

    assert_eq!(pool.status, PoolState::Draft);
    assert_eq!(pool.participants.len(), 3);
    assert_eq!(pool.context.as_ref().unwrap().pooler_roster.len(), 3);
    assert_eq!(pool.draft_order.as_ref().unwrap().len(), 3);
}

#[test]
fn start_draft_validates_the_participants_and_the_order() {
    let draft_order: Vec<String> = PARTICIPANTS.iter().map(|id| id.to_string()).collect();

    // Not enough poolers compared to the settings.
    let mut pool = Pool::new("draft-pool", OWNER, &small_settings());
    let two_users = vec![room_user(OWNER), room_user(USER_2)];
    assert!(pool.start_draft(OWNER, &two_users, &draft_order).is_err());

    // A draft order that references an unknown user.
    let mut pool = Pool::new("draft-pool", OWNER, &small_settings());
    let room_users: Vec<RoomUser> = PARTICIPANTS.iter().map(|id| room_user(id)).collect();
    let bad_order = vec![
        OWNER.to_string(),
        USER_2.to_string(),
        "stranger".to_string(),
    ];
    assert!(pool.start_draft(OWNER, &room_users, &bad_order).is_err());

    // A draft order that gives a pooler two spots and leaves another one out.
    let mut pool = Pool::new("draft-pool", OWNER, &small_settings());
    let duplicated_order = vec![OWNER.to_string(), USER_2.to_string(), USER_2.to_string()];
    assert!(
        pool.start_draft(OWNER, &room_users, &duplicated_order)
            .is_err()
    );

    // A draft order that does not cover every pooler of the room.
    let mut pool = Pool::new("draft-pool", OWNER, &small_settings());
    let short_order = vec![OWNER.to_string(), USER_2.to_string()];
    assert!(pool.start_draft(OWNER, &room_users, &short_order).is_err());

    // Only a pool in the Created state can start a draft.
    let mut pool = in_progress_pool();
    assert!(pool.start_draft(OWNER, &room_users, &draft_order).is_err());
}

#[test]
fn draft_player_follows_the_serpentine_order() {
    let mut pool = draft_pool();

    pool.draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();

    // It is now user-2's turn; user-3 cannot draft.
    let result = pool.draft_player(USER_3, &player(2, Position::F, None));
    assert!(result.unwrap_err().to_string().contains(USER_2));

    pool.draft_player(USER_2, &player(2, Position::F, None))
        .unwrap();
    pool.draft_player(USER_3, &player(3, Position::F, None))
        .unwrap();

    // Second round is reversed: user-3 drafts first.
    pool.draft_player(USER_3, &player(4, Position::F, None))
        .unwrap();

    assert!(possesses(&pool, USER_3, 3));
    assert!(possesses(&pool, USER_3, 4));
}

#[test]
fn the_owner_can_draft_on_behalf_of_the_next_drafter() {
    let mut pool = draft_pool();
    pool.draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();

    // It is user-2's turn, but the owner picks for him.
    pool.draft_player(OWNER, &player(2, Position::F, None))
        .unwrap();

    assert!(possesses(&pool, USER_2, 2));
}

#[test]
fn draft_player_rejects_an_already_picked_player() {
    let mut pool = draft_pool();
    pool.draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();

    let result = pool.draft_player(USER_2, &player(1, Position::F, None));

    assert!(result.unwrap_err().to_string().contains("already picked"));
}

// The rounds of a full draft under `small_settings`: one position per round,
// the last forward round overflowing into the reservist spot.
const DRAFT_ROUNDS: [Position; 5] = [
    Position::F,
    Position::F,
    Position::D,
    Position::G,
    Position::F,
];

// The serpentine order of every pick of a full draft, as (round, drafter).
fn full_draft_turns(draft_order: &[String]) -> Vec<(usize, String)> {
    let mut turns = Vec::new();
    for round in 0..DRAFT_ROUNDS.len() {
        let mut drafters: Vec<String> = draft_order.to_vec();
        if round % 2 == 1 {
            drafters.reverse();
        }
        turns.extend(drafters.into_iter().map(|drafter| (round, drafter)));
    }
    turns
}

#[test]
fn the_draft_completes_once_every_roster_is_full() {
    let mut pool = draft_pool();
    let draft_order = pool.draft_order.as_ref().unwrap().clone();
    let turns = full_draft_turns(&draft_order);
    let total_picks = turns.len();

    // 3 participants x 5 roster spots.
    assert_eq!(total_picks, 15);

    for (pick, (round, drafter)) in turns.into_iter().enumerate() {
        let player_id = pick as u32 + 1;
        let outcome = pool
            .draft_player(
                &drafter,
                &player(player_id, DRAFT_ROUNDS[round].clone(), None),
            )
            .unwrap();

        // The player went to whoever's turn it was, and the pick was recorded.
        assert_eq!(outcome.drafter, drafter);
        assert_eq!(outcome.appended_picks, vec![player_id]);

        let is_last_pick = pick + 1 == total_picks;
        assert_eq!(
            outcome.is_done,
            is_last_pick,
            "pick {} of {total_picks} reported is_done={}",
            pick + 1,
            outcome.is_done
        );

        // The pool must stay in Draft for every pick but the last: flipping
        // early would open roster moves on incomplete rosters.
        let expected = if is_last_pick {
            PoolState::InProgress
        } else {
            PoolState::Draft
        };
        assert_eq!(
            pool.status,
            expected,
            "wrong status after pick {} of {total_picks}",
            pick + 1
        );
    }

    let context = pool.context.as_ref().unwrap();
    for participant in PARTICIPANTS {
        assert_eq!(context.get_roster_count(participant).unwrap(), 5);
        assert_eq!(context.get_reservists_count(participant).unwrap(), 1);
    }
    // Every pick made it into the draft history, in order.
    assert_eq!(
        context.players_name_drafted,
        (1..=total_picks as u32).collect::<Vec<_>>()
    );
}

// Once the draft is over the pool has left the Draft status, so further picks
// (a late socket message, a double click) must be refused rather than
// overfilling a roster.
#[test]
fn a_pick_after_the_draft_is_complete_is_refused() {
    let mut pool = draft_pool();
    let draft_order = pool.draft_order.as_ref().unwrap().clone();

    for (pick, (round, drafter)) in full_draft_turns(&draft_order).into_iter().enumerate() {
        pool.draft_player(
            &drafter,
            &player(pick as u32 + 1, DRAFT_ROUNDS[round].clone(), None),
        )
        .unwrap();
    }
    assert_eq!(pool.status, PoolState::InProgress);

    let result = pool.draft_player(&draft_order[0], &player(99, Position::F, None));

    assert!(result.is_err(), "a pick after completion must be refused");
    let context = pool.context.as_ref().unwrap();
    assert_eq!(context.get_roster_count(&draft_order[0]).unwrap(), 5);
    assert_eq!(context.players_name_drafted.len(), 15);
}

#[test]
fn undo_draft_player_removes_the_last_pick() {
    let mut pool = draft_pool();
    pool.draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();
    pool.draft_player(USER_2, &player(2, Position::F, None))
        .unwrap();

    // Only the owner can undo.
    assert!(pool.undo_draft_player(USER_2).is_err());

    pool.undo_draft_player(OWNER).unwrap();

    let context = pool.context.as_ref().unwrap();
    assert!(!possesses(&pool, USER_2, 2));
    assert!(!context.players.contains_key("2"));
    assert_eq!(context.players_name_drafted, vec![1]);
}

// The draft broadcasts the outcome instead of the whole pool, so the outcome
// has to name the participant the pick landed on and everything the pick
// appended to `players_name_drafted`.
#[test]
fn draft_player_reports_the_pick_delta() {
    let mut pool = draft_pool();

    let outcome = pool
        .draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();

    assert_eq!(outcome.drafter, OWNER);
    assert_eq!(outcome.appended_picks, vec![1]);
    assert!(!outcome.is_done);

    // The owner drafting for someone else reports that someone else, not the
    // caller: this is the id the clients apply the roster update to.
    let outcome = pool
        .draft_player(OWNER, &player(2, Position::F, None))
        .unwrap();

    assert_eq!(outcome.drafter, USER_2);
    assert_eq!(outcome.appended_picks, vec![2]);
}

#[test]
fn draft_player_reports_the_last_pick_as_done() {
    let mut pool = draft_pool();
    let positions = [
        Position::F,
        Position::F,
        Position::D,
        Position::G,
        Position::F,
    ];
    let draft_order = pool.draft_order.as_ref().unwrap().clone();

    let mut player_id = 0;
    let mut last_outcome = None;
    for (round, position) in positions.iter().enumerate() {
        let mut drafters: Vec<String> = draft_order.clone();
        if round % 2 == 1 {
            drafters.reverse();
        }
        for drafter in drafters {
            player_id += 1;
            last_outcome = Some(
                pool.draft_player(&drafter, &player(player_id, position.clone(), None))
                    .unwrap(),
            );
        }
    }

    assert!(last_outcome.unwrap().is_done);
    assert_eq!(pool.status, PoolState::InProgress);
}

#[test]
fn undo_draft_player_reports_the_reverted_pick() {
    let mut pool = draft_pool();
    pool.draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();
    pool.draft_player(USER_2, &player(2, Position::F, None))
        .unwrap();

    let outcome = pool.undo_draft_player(OWNER).unwrap();

    assert_eq!(outcome.drafter, USER_2);
    assert_eq!(outcome.player_id, 2);
}

// A dynasty draft pushes a 0 onto `players_name_drafted` for every drafter it
// skips because its roster is already full. Those zeros are part of the pick
// delta, otherwise the clients drift out of sync with the draft board.
#[test]
fn dynasty_draft_reports_the_skipped_picks() {
    let mut pool = dynasty_pool();
    pool.protect_players(OWNER, OWNER, &[101, 111]).unwrap();
    pool.protect_players(USER_2, USER_2, &[201, 211]).unwrap();
    pool.protect_players(USER_3, USER_3, &[301, 311]).unwrap();
    pool.complete_protection(OWNER).unwrap();
    pool.draft_order = Some(PARTICIPANTS.iter().map(|id| id.to_string()).collect());

    // Fill user-2's roster (max 5) so the draft has to skip his turn.
    let roster = pool
        .context
        .as_mut()
        .unwrap()
        .pooler_roster
        .get_mut(USER_2)
        .unwrap();
    roster.chosen_forwards.push(202);
    roster.chosen_goalies.push(221);
    roster.chosen_reservists.push(231);

    // The owner drafts first, then user-2's turn is skipped.
    let outcome = pool
        .draft_player(OWNER, &player(1, Position::F, None))
        .unwrap();

    assert_eq!(outcome.drafter, OWNER);
    assert_eq!(outcome.appended_picks, vec![1, 0]);
    assert_eq!(
        pool.context.as_ref().unwrap().players_name_drafted,
        vec![1, 0]
    );
}

// -------------------------------------------------------------------
// Dynasty protection
// -------------------------------------------------------------------

fn dynasty_pool() -> Pool {
    let mut pool = in_progress_pool();
    pool.status = PoolState::Dynasty;
    pool.settings.dynasty_settings = Some(DynastySettings {
        next_season_number_players_protected: 2,
        tradable_picks: 1,
        past_season_pool_name: Vec::new(),
        next_season_pool_name: None,
    });
    pool
}

#[test]
fn protect_players_stores_the_protected_list() {
    let mut pool = dynasty_pool();

    pool.protect_players(OWNER, OWNER, &[101, 111]).unwrap();

    let context = pool.context.as_ref().unwrap();
    assert_eq!(
        context.protected_players.as_ref().unwrap()[OWNER],
        vec![101, 111]
    );
}

#[test]
fn protect_players_validates_the_request() {
    let mut pool = dynasty_pool();

    // Wrong amount of protected players.
    assert!(pool.protect_players(OWNER, OWNER, &[101]).is_err());

    // A player the participant does not own.
    assert!(pool.protect_players(OWNER, OWNER, &[101, 201]).is_err());

    // Only a Dynasty pool accepts protections.
    let mut pool = in_progress_pool();
    assert!(pool.protect_players(OWNER, OWNER, &[101, 111]).is_err());
}

#[test]
fn complete_protection_rebuilds_the_rosters_and_moves_to_draft() {
    let mut pool = dynasty_pool();
    pool.protect_players(OWNER, OWNER, &[101, 111]).unwrap();

    // The protection phase is not complete yet.
    assert!(pool.complete_protection(OWNER).is_err());

    pool.protect_players(USER_2, USER_2, &[201, 211]).unwrap();
    pool.protect_players(USER_3, USER_3, &[301, 311]).unwrap();

    // Only the owner can complete the protection phase.
    assert!(pool.complete_protection(USER_2).is_err());

    pool.complete_protection(OWNER).unwrap();

    assert_eq!(pool.status, PoolState::Draft);
    let context = pool.context.as_ref().unwrap();
    // Rosters only hold the protected players; the others left the pool.
    assert_eq!(context.get_roster_count(OWNER).unwrap(), 2);
    assert!(possesses(&pool, OWNER, 101));
    assert!(possesses(&pool, OWNER, 111));
    assert_eq!(context.players.len(), 6);
}

// -------------------------------------------------------------------
// Points and final ranking
// -------------------------------------------------------------------

#[test]
fn skater_points_include_hattricks_and_shootout_goals() {
    let settings = SkaterSettings {
        points_per_goals: 2,
        points_per_assists: 1,
        points_per_hattricks: 3,
        points_per_shootout_goals: 1,
    };
    let points = SkaterPoints {
        G: 3,
        A: 1,
        SOG: Some(1),
    };

    // 3 goals * 2 + 1 assist + 1 shootout goal + hattrick bonus.
    assert_eq!(points.get_total_points(&settings), 11);
}

#[test]
fn goalie_points_cumulate_every_bonus() {
    let settings = GoaliesSettings {
        points_per_wins: 2,
        points_per_shutouts: 3,
        points_per_overtimes: 1,
        points_per_goals: 3,
        points_per_assists: 2,
    };
    let points = GoalyPoints {
        G: 1,
        A: 1,
        W: true,
        SO: false,
        OT: true,
    };

    // 1 goal * 3 + 1 assist * 2 + win + overtime.
    assert_eq!(points.get_total_points(&settings), 8);
}

fn skater_day(scores: &[(u32, u8, u8)]) -> DailyRosterPoints {
    DailyRosterPoints {
        roster: Roster {
            F: scores
                .iter()
                .map(|(id, goals, assists)| {
                    (
                        id.to_string(),
                        Some(SkaterPoints {
                            G: *goals,
                            A: *assists,
                            SOG: None,
                        }),
                    )
                })
                .collect(),
            D: HashMap::new(),
            G: HashMap::new(),
        },
        is_cumulated: true,
    }
}

// One cumulated day: user-2 leads with 4 points, the owner and user-3 are
// tied at 2 points but the owner played fewer games.
fn pool_with_scores() -> Pool {
    let mut pool = in_progress_pool();
    let mut day = HashMap::new();
    day.insert(OWNER.to_string(), skater_day(&[(101, 1, 0)]));
    day.insert(USER_2.to_string(), skater_day(&[(201, 2, 0)]));
    day.insert(USER_3.to_string(), skater_day(&[(301, 0, 1), (302, 0, 1)]));
    pool.context.as_mut().unwrap().score_by_day =
        Some(HashMap::from([("2025-12-01".to_string(), day)]));
    pool
}

#[test]
fn final_rank_orders_by_points_then_fewer_games() {
    let pool = pool_with_scores();

    let rank = pool
        .context
        .as_ref()
        .unwrap()
        .get_final_rank(&pool.settings)
        .unwrap();

    assert_eq!(rank, vec![USER_2, OWNER, USER_3]);
}

// A day that scored but was not cumulated is still being written: ranking on it
// would produce the wrong final order.
#[test]
fn final_rank_requires_every_scoring_day_to_be_cumulated() {
    let mut pool = pool_with_scores();
    let context = pool.context.as_mut().unwrap();
    context
        .score_by_day
        .as_mut()
        .unwrap()
        .get_mut("2025-12-01")
        .unwrap()
        .get_mut(OWNER)
        .unwrap()
        .is_cumulated = false;

    assert!(context.get_final_rank(&pool.settings).is_err());
}

// The league's off days (all-star / olympic breaks, playoff gaps) are left
// uncumulated by the ingest because there are no games to cumulate. They add
// nothing to any tally, so they must not make the pool impossible to finalize.
#[test]
fn a_day_without_games_does_not_block_the_final_rank() {
    let mut pool = pool_with_scores();
    let context = pool.context.as_mut().unwrap();
    let score_by_day = context.score_by_day.as_mut().unwrap();

    // An olympic-break day: every participant is rostered, nobody played, and
    // the ingest never flipped it to cumulated.
    let mut break_day = HashMap::new();
    for participant in PARTICIPANTS {
        let mut scoreless = skater_day(&[]);
        scoreless
            .roster
            .F
            .insert(101.to_string(), Option::<SkaterPoints>::None);
        scoreless.is_cumulated = false;
        break_day.insert(participant.to_string(), scoreless);
    }
    score_by_day.insert("2026-02-21".to_string(), break_day);

    let rank = context.get_final_rank(&pool.settings).unwrap();

    // The ranking is unchanged: the empty day contributed nothing.
    assert_eq!(rank, vec![USER_2, OWNER, USER_3]);
}

#[test]
fn day_scores_derive_from_daily_leaders_and_reuse_scoring() {
    use crate::daily_leaders::model::{
        DailyGoaly, DailyLeaders, DailySkater, GoalyStats, SkaterStats,
    };

    let daily_leaders = DailyLeaders {
        date: "2025-12-01".to_string(),
        skaters: vec![DailySkater {
            name: "Hat Trick".to_string(),
            id: 101,
            team: 1,
            stats: SkaterStats {
                goals: 3,
                assists: 1,
                shootoutGoals: 1,
            },
        }],
        goalies: vec![DailyGoaly {
            name: "Perfect Night".to_string(),
            id: 501,
            team: 2,
            stats: GoalyStats {
                goals: 0,
                assists: 0,
                decision: Some("W".to_string()),
                savePercentage: Some(1.0),
                OT: None,
            },
        }],
        played: vec![101, 501],
    };

    let day = DayScores::from_daily_leaders(&daily_leaders);

    // A shutout is derived from a win with a perfect save percentage.
    let goalie = day.goalies.get(&501).unwrap();
    assert!(goalie.W && goalie.SO && !goalie.OT);

    // Building the lineup roster lets the existing scoring run untouched.
    let daily = DailyRosterPoints {
        roster: day.roster_for(&[101], &[], &[501]),
        is_cumulated: true,
    };

    let (mut f, mut d, mut g) = (HashMap::new(), HashMap::new(), HashMap::new());
    let (points, games) = daily.get_total_points(&PoolSettings::new(), &mut f, &mut d, &mut g);

    // Forward: 3*2 + 1*1 + 1 shootout + 3 hattrick = 11. Goalie: win 2 + shutout 3 = 5.
    assert_eq!(points, 16);
    assert_eq!(games, 2);
}

#[test]
fn scoreless_players_that_dressed_still_count_as_a_game() {
    use crate::daily_leaders::model::DailyLeaders;

    // `day_leaders` only lists a player under skaters/goalies once they
    // registered a stat; 102 and 502 dressed and were held off the board,
    // while 103/503 did not play at all.
    let daily_leaders = DailyLeaders {
        date: "2025-12-01".to_string(),
        skaters: vec![],
        goalies: vec![],
        played: vec![102, 502],
    };

    let day = DayScores::from_daily_leaders(&daily_leaders);
    let roster = day.roster_for(&[102, 103], &[], &[502, 503]);

    let skater = roster.F["102"].as_ref().unwrap();
    assert_eq!((skater.G, skater.A), (0, 0));
    assert!(roster.F["103"].is_none());

    let goalie = roster.G["502"].as_ref().unwrap();
    assert!(!goalie.W && !goalie.SO && !goalie.OT);
    assert!(roster.G["503"].is_none());

    let daily = DailyRosterPoints {
        roster,
        is_cumulated: true,
    };
    let (mut f, mut d, mut g) = (HashMap::new(), HashMap::new(), HashMap::new());
    let (points, games) = daily.get_total_points(&PoolSettings::new(), &mut f, &mut d, &mut g);

    // No points, but the two players that dressed each played a game.
    assert_eq!(points, 0);
    assert_eq!(games, 2);
}

#[test]
fn record_lineup_change_appends_only_on_change() {
    let mut context = PoolContext::new(&["u1".to_string()]);
    {
        let roster = context.pooler_roster.get_mut("u1").unwrap();
        roster.chosen_forwards = vec![10, 11];
        roster.chosen_goalies = vec![30];
    }

    assert!(context.record_lineup_change("u1", "2025-10-01"));
    // Same lineup on a later day: nothing appended.
    assert!(!context.record_lineup_change("u1", "2025-10-05"));
    // A real change is appended.
    context.pooler_roster.get_mut("u1").unwrap().chosen_forwards = vec![10, 12];
    assert!(context.record_lineup_change("u1", "2025-10-08"));

    let events = context.lineup_events.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].effective_date, "2025-10-01");
    assert_eq!(events[0].forwards, vec![10, 11]);
    assert_eq!(events[1].effective_date, "2025-10-08");
    assert_eq!(events[1].forwards, vec![10, 12]);
}

#[test]
fn mark_as_final_waits_for_the_season_to_end() {
    let mut pool = pool_with_scores();
    let end_of_season = NaiveDate::parse_from_str(END_SEASON_DATE, "%Y-%m-%d").unwrap();

    // Cannot be finalized during the season.
    assert!(pool.mark_as_final_at(OWNER, end_of_season).is_err());

    pool.mark_as_final_at(OWNER, end_of_season + Duration::days(1))
        .unwrap();

    assert_eq!(pool.status, PoolState::Final);
    assert_eq!(
        pool.final_rank.as_ref().unwrap(),
        &[USER_2.to_string(), OWNER.to_string(), USER_3.to_string()]
    );
}
