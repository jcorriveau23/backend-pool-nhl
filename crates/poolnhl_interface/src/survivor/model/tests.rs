use super::*;

const OWNER: &str = "owner";
const USER_2: &str = "user-2";
const USER_3: &str = "user-3";

const SEASON: u32 = 20262027;
// A Tuesday, so the first Saturday is not the start date itself.
const SEASON_START: &str = "2026-09-29";
const SEASON_END: &str = "2027-04-10";

fn settings() -> SurvivorSettings {
    SurvivorSettings::new()
}

fn pool() -> SurvivorPool {
    SurvivorPool::new(
        "survivor",
        OWNER,
        &settings(),
        SEASON,
        SEASON_START,
        SEASON_END,
    )
    .unwrap()
}

/// A pool with the three users joined and every week but the first settled away,
/// so tests act on week 1.
fn pool_with_participants() -> SurvivorPool {
    let mut pool = pool();
    for user in [OWNER, USER_2, USER_3] {
        pool.add_participant(user, user, 0).unwrap();
    }
    pool
}

fn used(entries: &[(u16, u32)]) -> Vec<UsedTeam> {
    entries
        .iter()
        .map(|(cycle, team_id)| UsedTeam {
            cycle: *cycle,
            team_id: *team_id,
        })
        .collect()
}

fn outcomes(entries: &[(&str, PickOutcome)]) -> HashMap<String, PickOutcome> {
    entries
        .iter()
        .map(|(user, outcome)| (user.to_string(), *outcome))
        .collect()
}

fn blocked(users: &[&str]) -> HashSet<String> {
    users.iter().map(|user| user.to_string()).collect()
}

// ---------------------------------------------------------------- pick dates

#[test]
fn saturdays_between_starts_on_the_first_saturday() {
    // 2026-09-29 is a Tuesday; the Saturday that follows is 2026-10-03.
    let saturdays = saturdays_between("2026-09-29", "2026-10-20").unwrap();

    assert_eq!(
        saturdays,
        vec![
            "2026-10-03".to_string(),
            "2026-10-10".to_string(),
            "2026-10-17".to_string(),
        ]
    );
}

#[test]
fn saturdays_between_includes_a_start_that_is_itself_a_saturday() {
    let saturdays = saturdays_between("2026-10-03", "2026-10-11").unwrap();

    assert_eq!(
        saturdays,
        vec!["2026-10-03".to_string(), "2026-10-10".to_string()]
    );
}

#[test]
fn saturdays_between_includes_an_end_that_is_itself_a_saturday() {
    let saturdays = saturdays_between("2026-10-04", "2026-10-10").unwrap();

    assert_eq!(saturdays, vec!["2026-10-10".to_string()]);
}

#[test]
fn saturdays_between_rejects_a_backwards_range() {
    assert!(saturdays_between("2026-10-10", "2026-10-03").is_err());
}

#[test]
fn saturdays_between_rejects_a_date_that_is_not_one() {
    assert!(saturdays_between("not-a-date", "2026-10-03").is_err());
}

#[test]
fn a_new_pool_has_a_week_per_saturday_of_the_season() {
    let pool = pool();
    let saturdays = saturdays_between(SEASON_START, SEASON_END).unwrap();

    assert_eq!(pool.weeks.len(), saturdays.len());
    assert_eq!(pool.weeks[0].week, 1);
    assert_eq!(pool.weeks[0].pick_date, saturdays[0]);
    assert!(matches!(pool.weeks[0].status, WeekStatus::Open));
    assert!(matches!(pool.status, SurvivorState::Created));
}

#[test]
fn a_pool_cannot_open_on_a_season_with_no_saturday_in_it() {
    // Monday to Friday of the same week.
    let error = SurvivorPool::new(
        "survivor",
        OWNER,
        &settings(),
        SEASON,
        "2026-10-05",
        "2026-10-09",
    );

    assert!(error.is_err());
}

// -------------------------------------------------------------- joining

#[test]
fn joining_is_self_serve_and_records_an_alive_participant() {
    let mut pool = pool();
    pool.add_participant(USER_2, "  Raph  ", 42).unwrap();

    let participant = pool.participant(USER_2).unwrap();
    // The name is trimmed on the way in, so it matches what a later join is
    // checked for uniqueness against.
    assert_eq!(participant.name, "Raph");
    assert_eq!(participant.strikes, 0);
    assert_eq!(participant.date_joined, 42);
    assert!(participant.is_alive());
}

#[test]
fn the_same_user_cannot_join_twice() {
    let mut pool = pool();
    pool.add_participant(USER_2, "Raph", 0).unwrap();

    assert!(pool.add_participant(USER_2, "Raph again", 0).is_err());
}

#[test]
fn two_participants_cannot_share_a_name() {
    let mut pool = pool();
    pool.add_participant(USER_2, "Raph", 0).unwrap();

    assert!(pool.add_participant(USER_3, "Raph", 0).is_err());
}

#[test]
fn a_pool_does_not_take_more_than_its_maximum() {
    let mut pool_settings = settings();
    pool_settings.max_participants = 2;
    let mut pool = SurvivorPool::new(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
    )
    .unwrap();

    pool.add_participant(OWNER, OWNER, 0).unwrap();
    pool.add_participant(USER_2, USER_2, 0).unwrap();

    assert!(pool.add_participant(USER_3, USER_3, 0).is_err());
}

#[test]
fn a_pool_takes_the_hundreds_of_participants_it_is_built_for() {
    let mut pool_settings = settings();
    pool_settings.max_participants = 500;
    let mut pool = SurvivorPool::new(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
    )
    .unwrap();

    for index in 0..500 {
        pool.add_participant(&format!("user-{index}"), &format!("user-{index}"), 0)
            .unwrap();
    }

    assert_eq!(pool.participants.len(), 500);
    assert_eq!(pool.alive_participants().count(), 500);
}

#[test]
fn joining_closes_once_a_date_has_been_settled() {
    let mut pool = pool_with_participants();
    pool.apply_week_results(1, &outcomes(&[(OWNER, PickOutcome::Won)]), &blocked(&[]), 0)
        .unwrap();

    assert!(pool.add_participant("latecomer", "latecomer", 0).is_err());
}

#[test]
fn a_participant_may_leave_but_only_the_owner_may_remove_somebody_else() {
    let mut pool = pool_with_participants();

    assert!(pool.remove_participant(USER_2, USER_3).is_err());
    pool.remove_participant(USER_2, USER_2).unwrap();
    assert!(pool.participant(USER_2).is_none());

    pool.remove_participant(OWNER, USER_3).unwrap();
    assert!(pool.participant(USER_3).is_none());
}

#[test]
fn a_name_longer_than_the_maximum_is_refused() {
    let mut pool = pool();
    let too_long = "x".repeat(MAX_PARTICIPANT_NAME_LENGTH + 1);

    assert!(pool.add_participant(USER_2, &too_long, 0).is_err());
}

// --------------------------------------------------------------- settings

#[test]
fn settings_are_checked_before_a_pool_runs_with_them() {
    let mut pool_settings = settings();

    pool_settings.max_participants = 1;
    assert!(pool_settings.validate().is_err());

    pool_settings.max_participants = MAX_SURVIVOR_PARTICIPANTS + 1;
    assert!(pool_settings.validate().is_err());

    pool_settings.max_participants = 100;
    pool_settings.league_team_count = 1;
    assert!(pool_settings.validate().is_err());

    pool_settings.league_team_count = 32;
    assert!(pool_settings.validate().is_ok());
}

// ------------------------------------------------------- the used-team rule

#[test]
fn a_team_not_used_yet_is_picked_in_the_current_cycle() {
    let cycle = plan_pick(&used(&[(0, 8)]), &[8, 10, 6], 32, 10).unwrap();

    assert_eq!(cycle, 0);
}

#[test]
fn a_team_that_does_not_play_that_day_cannot_be_picked() {
    let error = plan_pick(&used(&[]), &[8, 10], 32, 6);

    assert!(error.is_err());
}

#[test]
fn a_used_team_is_closed_while_the_participant_still_has_others() {
    let error = plan_pick(&used(&[(0, 8)]), &[8, 10, 6], 32, 8);

    assert!(error.is_err());
}

#[test]
fn available_teams_are_the_playing_ones_the_participant_has_not_spent() {
    let available = available_team_ids(&used(&[(0, 8), (0, 6)]), &[8, 10, 6, 14], 32);

    assert_eq!(available, vec![10, 14]);
}

#[test]
fn going_through_the_whole_league_reopens_every_team_in_a_new_cycle() {
    // 32 teams used, so the list resets.
    let spent: Vec<UsedTeam> = (0..32)
        .map(|team_id| UsedTeam { cycle: 0, team_id })
        .collect();

    let available = available_team_ids(&spent, &[0, 1, 2], 32);
    assert_eq!(available, vec![0, 1, 2]);

    // And the pick that follows opens cycle 1, which is what reopens the unique
    // index on (participant, cycle, team).
    let cycle = plan_pick(&spent, &[0, 1, 2], 32, 1).unwrap();
    assert_eq!(cycle, 1);
}

#[test]
fn a_used_team_is_closed_again_once_the_new_cycle_has_spent_it() {
    let mut spent: Vec<UsedTeam> = (0..32)
        .map(|team_id| UsedTeam { cycle: 0, team_id })
        .collect();
    spent.push(UsedTeam {
        cycle: 1,
        team_id: 1,
    });

    // Cycle 1 is the current one now, and only team 1 is spent in it.
    assert_eq!(current_cycle(&spent), 1);
    assert!(plan_pick(&spent, &[0, 1, 2], 32, 1).is_err());
    assert_eq!(plan_pick(&spent, &[0, 1, 2], 32, 2).unwrap(), 1);
}

#[test]
fn a_participant_with_nothing_left_to_pick_that_day_is_blocked() {
    // Both teams playing are spent, but the league has more, so the list does
    // not reset — there is simply no legal pick this week.
    let spent = used(&[(0, 8), (0, 10)]);

    assert!(is_blocked(&spent, &[8, 10], 32));
    assert!(available_team_ids(&spent, &[8, 10], 32).is_empty());
}

#[test]
fn a_date_the_league_scheduled_nothing_on_blocks_everybody() {
    assert!(is_blocked(&used(&[]), &[], 32));
}

#[test]
fn a_participant_with_a_team_left_is_not_blocked() {
    assert!(!is_blocked(&used(&[(0, 8)]), &[8, 10], 32));
}

// ------------------------------------------------------------ pick windows

#[test]
fn only_an_alive_participant_of_an_open_week_may_pick() {
    let mut pool = pool_with_participants();

    pool.validate_can_pick(USER_2, 1).unwrap();
    // Not in the pool at all.
    assert!(pool.validate_can_pick("stranger", 1).is_err());
    // No such week.
    assert!(pool.validate_can_pick(USER_2, 9999).is_err());

    pool.week_mut(1).unwrap().status = WeekStatus::Locked;
    assert!(pool.validate_can_pick(USER_2, 1).is_err());
}

#[test]
fn an_eliminated_participant_may_not_pick() {
    let mut pool = pool_with_participants();
    pool.apply_week_results(
        1,
        &outcomes(&[
            (OWNER, PickOutcome::Won),
            (USER_2, PickOutcome::Lost),
            (USER_3, PickOutcome::Won),
        ]),
        &blocked(&[]),
        0,
    )
    .unwrap();

    assert!(!pool.participant(USER_2).unwrap().is_alive());
    assert!(pool.validate_can_pick(USER_2, 2).is_err());
    pool.validate_can_pick(OWNER, 2).unwrap();
}

// -------------------------------------------------------------- settlement

#[test]
fn a_losing_pick_eliminates_when_no_strike_is_allowed() {
    let mut pool = pool_with_participants();

    pool.apply_week_results(
        1,
        &outcomes(&[
            (OWNER, PickOutcome::Won),
            (USER_2, PickOutcome::Lost),
            (USER_3, PickOutcome::Won),
        ]),
        &blocked(&[]),
        1_700_000_000,
    )
    .unwrap();

    let eliminated = pool.participant(USER_2).unwrap();
    assert!(!eliminated.is_alive());
    assert_eq!(eliminated.strikes, 1);
    assert_eq!(eliminated.eliminated_week, Some(1));

    assert!(pool.participant(OWNER).unwrap().is_alive());
    assert_eq!(pool.alive_participants().count(), 2);

    let week = pool.week(1).unwrap();
    assert!(matches!(week.status, WeekStatus::Settled));
    assert_eq!(week.settled_at, Some(1_700_000_000));
    assert!(matches!(pool.status, SurvivorState::InProgress));
}

#[test]
fn an_allowed_strike_is_survived_and_the_next_one_is_not() {
    let mut pool_settings = settings();
    pool_settings.strikes_allowed = 1;
    let mut pool = SurvivorPool::new(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
    )
    .unwrap();
    for user in [OWNER, USER_2, USER_3] {
        pool.add_participant(user, user, 0).unwrap();
    }

    pool.apply_week_results(
        1,
        &outcomes(&[(USER_2, PickOutcome::Lost)]),
        &blocked(&[]),
        0,
    )
    .unwrap();
    // Everybody else missed their pick, which is a strike by default too.
    assert_eq!(pool.participant(USER_2).unwrap().strikes, 1);
    assert!(pool.participant(USER_2).unwrap().is_alive());

    pool.apply_week_results(
        2,
        &outcomes(&[(USER_2, PickOutcome::Lost)]),
        &blocked(&[]),
        0,
    )
    .unwrap();
    assert_eq!(pool.participant(USER_2).unwrap().strikes, 2);
    assert!(!pool.participant(USER_2).unwrap().is_alive());
    assert_eq!(pool.participant(USER_2).unwrap().eliminated_week, Some(2));
}

#[test]
fn a_void_pick_costs_nothing() {
    let mut pool = pool_with_participants();

    pool.apply_week_results(
        1,
        &outcomes(&[
            (OWNER, PickOutcome::Void),
            (USER_2, PickOutcome::Won),
            (USER_3, PickOutcome::Pending),
        ]),
        &blocked(&[]),
        0,
    )
    .unwrap();

    assert_eq!(pool.alive_participants().count(), 3);
    for user in [OWNER, USER_2, USER_3] {
        assert_eq!(pool.participant(user).unwrap().strikes, 0);
    }
}

#[test]
fn a_missed_pick_costs_a_strike_by_default() {
    let mut pool = pool_with_participants();

    pool.apply_week_results(1, &outcomes(&[(OWNER, PickOutcome::Won)]), &blocked(&[]), 0)
        .unwrap();

    // Neither of the other two picked.
    assert_eq!(pool.participant(USER_2).unwrap().strikes, 1);
    assert!(!pool.participant(USER_2).unwrap().is_alive());
    assert_eq!(pool.participant(OWNER).unwrap().strikes, 0);
}

#[test]
fn a_missed_pick_can_be_set_to_eliminate_outright() {
    let mut pool_settings = settings();
    pool_settings.strikes_allowed = 2;
    pool_settings.missed_pick_is_strike = false;
    let mut pool = SurvivorPool::new(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
    )
    .unwrap();
    for user in [OWNER, USER_2] {
        pool.add_participant(user, user, 0).unwrap();
    }

    pool.apply_week_results(1, &outcomes(&[(OWNER, PickOutcome::Won)]), &blocked(&[]), 0)
        .unwrap();

    let missed = pool.participant(USER_2).unwrap();
    // Out despite two strikes being allowed, and without one being recorded.
    assert!(!missed.is_alive());
    assert_eq!(missed.strikes, 0);
    assert_eq!(missed.eliminated_week, Some(1));
}

#[test]
fn a_blocked_participant_is_not_penalised_for_not_picking() {
    let mut pool = pool_with_participants();

    pool.apply_week_results(
        1,
        &outcomes(&[(OWNER, PickOutcome::Won)]),
        &blocked(&[USER_2, USER_3]),
        0,
    )
    .unwrap();

    assert_eq!(pool.alive_participants().count(), 3);
    assert_eq!(pool.participant(USER_2).unwrap().strikes, 0);
}

#[test]
fn settling_a_date_twice_is_refused() {
    let mut pool = pool_with_participants();
    pool.apply_week_results(1, &outcomes(&[(OWNER, PickOutcome::Won)]), &blocked(&[]), 0)
        .unwrap();

    assert!(
        pool.apply_week_results(1, &outcomes(&[(OWNER, PickOutcome::Won)]), &blocked(&[]), 0)
            .is_err()
    );
}

#[test]
fn the_last_one_standing_wins_and_the_pool_ends() {
    let mut pool = pool_with_participants();

    pool.apply_week_results(
        1,
        &outcomes(&[
            (OWNER, PickOutcome::Won),
            (USER_2, PickOutcome::Lost),
            (USER_3, PickOutcome::Lost),
        ]),
        &blocked(&[]),
        0,
    )
    .unwrap();

    assert!(matches!(pool.status, SurvivorState::Final));
    assert_eq!(pool.winners, Some(vec![OWNER.to_string()]));
}

#[test]
fn a_date_that_takes_out_everybody_left_is_shared_between_them() {
    let mut pool = pool_with_participants();

    pool.apply_week_results(
        1,
        &outcomes(&[
            (OWNER, PickOutcome::Lost),
            (USER_2, PickOutcome::Lost),
            (USER_3, PickOutcome::Lost),
        ]),
        &blocked(&[]),
        0,
    )
    .unwrap();

    // Nobody is left, so the pool does not end with no winner at all — the
    // three who went into the date share it.
    assert!(matches!(pool.status, SurvivorState::Final));
    let mut winners = pool.winners.clone().unwrap();
    winners.sort();
    assert_eq!(
        winners,
        vec![OWNER.to_string(), USER_2.to_string(), USER_3.to_string()]
    );
}

#[test]
fn the_field_still_standing_when_the_weeks_run_out_shares_the_pool() {
    let mut pool_settings = settings();
    pool_settings.missed_pick_is_strike = false;
    // A one-Saturday season, so the weeks run out immediately.
    let mut pool = SurvivorPool::new(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        "2026-10-04",
        "2026-10-10",
    )
    .unwrap();
    assert_eq!(pool.weeks.len(), 1);

    for user in [OWNER, USER_2] {
        pool.add_participant(user, user, 0).unwrap();
    }

    pool.apply_week_results(
        1,
        &outcomes(&[(OWNER, PickOutcome::Won), (USER_2, PickOutcome::Won)]),
        &blocked(&[]),
        0,
    )
    .unwrap();

    assert!(matches!(pool.status, SurvivorState::Final));
    let mut winners = pool.winners.clone().unwrap();
    winners.sort();
    assert_eq!(winners, vec![OWNER.to_string(), USER_2.to_string()]);
}

#[test]
fn current_week_is_the_first_one_not_settled() {
    let mut pool = pool_with_participants();
    assert_eq!(pool.current_week().unwrap().week, 1);

    pool.apply_week_results(1, &outcomes(&[(OWNER, PickOutcome::Won)]), &blocked(&[]), 0)
        .unwrap();

    assert_eq!(pool.current_week().unwrap().week, 2);
}

// ------------------------------------------------------------------ rights

#[test]
fn the_owner_and_the_assistants_may_settle_a_date_and_nobody_else() {
    let mut pool_settings = settings();
    pool_settings.assistants = vec![USER_2.to_string()];
    let pool = SurvivorPool::new(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
    )
    .unwrap();

    pool.validate_assistant_rights(OWNER).unwrap();
    pool.validate_assistant_rights(USER_2).unwrap();
    assert!(pool.validate_assistant_rights(USER_3).is_err());

    // Owner-only actions do not open up to an assistant.
    pool.validate_owner_rights(OWNER).unwrap();
    assert!(pool.validate_owner_rights(USER_2).is_err());
}

#[test]
fn a_pool_name_too_short_or_too_long_is_refused() {
    assert!(validate_pool_name("ab").is_err());
    assert!(validate_pool_name(&"x".repeat(MAX_SURVIVOR_POOL_NAME_LENGTH + 1)).is_err());
    validate_pool_name("my survivor pool").unwrap();
}
