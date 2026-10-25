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

fn date(date: &str) -> NaiveDate {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
}

/// The night in play when the pools below are created: before the season, so
/// they hold a week per Saturday of it and nothing here is read against
/// whatever day the suite happens to run on.
fn before_the_season() -> NaiveDate {
    date("2026-09-01")
}

fn pool() -> SurvivorPool {
    SurvivorPool::new_at(
        "survivor",
        OWNER,
        &settings(),
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
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
fn a_pool_opened_mid_season_starts_on_the_next_saturday() {
    // A Tuesday in November, three months into the season.
    let pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &settings(),
        SEASON,
        SEASON_START,
        SEASON_END,
        date("2026-11-10"),
    )
    .unwrap();

    assert_eq!(pool.weeks[0].week, 1);
    assert_eq!(pool.weeks[0].pick_date, "2026-11-14");
    // Every Saturday left of the season, and not one of the six already gone.
    assert_eq!(
        pool.weeks.len(),
        saturdays_between("2026-11-14", SEASON_END).unwrap().len()
    );
    assert_eq!(pool.weeks.last().unwrap().pick_date, "2027-04-10");
}

/// The night in play turns over at noon Eastern, so a pool created on Saturday
/// morning is in time to play that afternoon and one created once the day's
/// games are on is not.
#[test]
fn a_pool_created_on_a_saturday_plays_it_only_until_the_games_start() {
    // Before noon the night in play is still Friday's.
    assert_eq!(
        first_pick_date(SEASON_START, date("2026-11-13")).unwrap(),
        date("2026-11-14")
    );
    // From noon it is Saturday's own, and the pool opens on the one after.
    assert_eq!(
        first_pick_date(SEASON_START, date("2026-11-14")).unwrap(),
        date("2026-11-21")
    );
}

/// A pool opened in the summer still begins on the season's first Saturday:
/// the next one coming has no hockey on it.
#[test]
fn a_pool_opened_before_the_season_starts_on_its_first_saturday() {
    let saturdays = saturdays_between(SEASON_START, SEASON_END).unwrap();

    assert_eq!(pool().weeks[0].pick_date, saturdays[0]);
    assert_eq!(
        first_pick_date(SEASON_START, before_the_season()).unwrap(),
        date(&saturdays[0])
    );
}

/// The season's Saturdays all being gone is the same refusal as a season with
/// none in it: there is nothing left to play.
#[test]
fn a_pool_cannot_open_after_the_last_saturday_of_the_season() {
    let error = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &settings(),
        SEASON,
        SEASON_START,
        SEASON_END,
        date("2027-04-12"),
    );

    assert!(error.is_err());
    assert!(
        pick_dates_from(SEASON_START, SEASON_END, date("2027-04-12"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_pool_cannot_open_on_a_season_with_no_saturday_in_it() {
    // Monday to Friday of the same week.
    let error = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &settings(),
        SEASON,
        "2026-10-05",
        "2026-10-09",
        before_the_season(),
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
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
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
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
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

// --------------------------------------- spots kept on somebody's behalf

#[test]
fn the_organiser_can_add_a_spot_for_somebody_with_no_account() {
    let mut pool = pool();

    pool.add_managed_participant(OWNER, "generated-id", "Francis", 7)
        .unwrap();

    let managed = pool.participant("generated-id").unwrap();
    assert_eq!(managed.name, "Francis");
    assert!(managed.is_alive());
    // What tells it apart from somebody who signed themselves up, and so what
    // tells the UI the organiser has to pick for them.
    assert!(!managed.is_owned);
}

#[test]
fn somebody_who_signs_themselves_up_owns_their_spot() {
    let mut pool = pool();
    pool.add_participant(USER_2, "Raph", 0).unwrap();

    assert!(pool.participant(USER_2).unwrap().is_owned);
}

#[test]
fn an_assistant_may_add_a_spot_but_an_ordinary_participant_may_not() {
    let mut pool_settings = settings();
    pool_settings.assistants = vec![USER_2.to_string()];
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
    )
    .unwrap();

    pool.add_managed_participant(USER_2, "id-1", "Francis", 0)
        .unwrap();
    assert!(
        pool.add_managed_participant(USER_3, "id-2", "Davis", 0)
            .is_err()
    );
}

#[test]
fn a_managed_spot_goes_through_the_same_checks_as_a_join() {
    let mut pool_settings = settings();
    pool_settings.max_participants = 2;
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
    )
    .unwrap();
    pool.add_participant(OWNER, "jf", 0).unwrap();
    pool.add_managed_participant(OWNER, "id-1", "Francis", 0)
        .unwrap();

    // Full.
    assert!(
        pool.add_managed_participant(OWNER, "id-2", "Davis", 0)
            .is_err()
    );

    // And a name somebody already goes by is refused whichever way it arrives.
    let mut roomy = pool_with_participants();
    assert!(
        roomy
            .add_managed_participant(OWNER, "id-3", OWNER, 0)
            .is_err()
    );
}

// ------------------------------------------------------ renaming a spot

#[test]
fn the_owner_renames_a_participant_without_touching_its_id() {
    let mut pool = pool_with_participants();

    pool.rename_participant(OWNER, USER_2, "  Raph  ").unwrap();

    // The surrounding whitespace is dropped, and the id every pick of theirs is
    // keyed by is the one it was.
    let renamed = pool.participant(USER_2).unwrap();
    assert_eq!(renamed.name, "Raph");
    assert_eq!(renamed.id, USER_2);
}

#[test]
fn renaming_a_participant_is_the_owner_call_alone() {
    let mut pool = pool_with_participants();
    pool.settings.assistants = vec![USER_2.to_string()];

    // An assistant settles dates but does not rename: the names are how the
    // standings are read, like the settings they also may not touch.
    assert!(pool.rename_participant(USER_2, USER_3, "Raph").is_err());
    assert!(pool.rename_participant(USER_3, USER_3, "Raph").is_err());
    assert!(pool.rename_participant(OWNER, USER_3, "Raph").is_ok());
}

#[test]
fn a_rename_goes_through_the_same_name_checks_as_a_join() {
    let mut pool = pool_with_participants();

    assert!(pool.rename_participant(OWNER, USER_2, "   ").is_err());
    assert!(
        pool.rename_participant(OWNER, USER_2, &"x".repeat(MAX_PARTICIPANT_NAME_LENGTH + 1))
            .is_err()
    );
    // A name somebody else in the pool already goes by.
    assert!(pool.rename_participant(OWNER, USER_2, USER_3).is_err());
    // Their own name is not a clash with themselves.
    assert!(pool.rename_participant(OWNER, USER_2, USER_2).is_ok());
}

#[test]
fn a_participant_who_is_not_in_the_pool_cannot_be_renamed() {
    let mut pool = pool_with_participants();

    assert!(pool.rename_participant(OWNER, "stranger", "Raph").is_err());
}

#[test]
fn a_settled_date_does_not_close_renaming() {
    let mut pool = pool_with_participants();
    pool.weeks[0].status = WeekStatus::Settled;

    // Joining closes once a date has settled, because a latecomer would have
    // missed it. A rename moves a display name and nothing else, so the board
    // behind it reads exactly as it did.
    pool.rename_participant(OWNER, USER_2, "Raph").unwrap();
    assert_eq!(pool.participant(USER_2).unwrap().name, "Raph");
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
fn the_teams_left_in_a_cycle_count_down_as_they_are_spent() {
    // The pressure a participant actually feels deep into a season, and the
    // reason it is counted here rather than on the pick screen: that screen's
    // `used_team_ids` spans every pass through the league.
    assert_eq!(teams_left_in_cycle(&[], 32), 32);
    assert_eq!(teams_left_in_cycle(&used(&[(0, 8), (0, 6)]), 32), 30);
}

#[test]
fn the_teams_left_only_count_the_pass_being_played() {
    // Two passes, three picks: one team is spent in the pass they are on, so
    // counting every pick would say 29 when the answer is 31.
    let spent = used(&[(0, 8), (0, 6), (1, 10)]);

    assert_eq!(teams_left_in_cycle(&spent, 32), 31);
}

#[test]
fn an_exhausted_list_reads_as_the_whole_league_again() {
    // The same reset `available_team_ids` applies: the pick that follows opens
    // a new pass, so nothing is closed to them any more. Counting down would
    // have reached zero and said the season was over.
    let spent: Vec<UsedTeam> = (0..32)
        .map(|team_id| UsedTeam { cycle: 0, team_id })
        .collect();

    assert_eq!(teams_left_in_cycle(&spent, 32), 32);
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

    // A pool played under the old rule, where the owner closed a date by hand.
    // Nothing writes this status any more, and one that carries it still
    // refuses.
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

/// Filing somebody else's pick.
#[test]
fn the_organiser_may_pick_for_a_spot_they_keep_and_a_participant_may_not() {
    let mut pool = pool_with_participants();
    pool.add_managed_participant(OWNER, "francis-id", "Francis", 0)
        .unwrap();

    // The organiser files for the spot they keep.
    pool.validate_can_pick_for(OWNER, "francis-id", 1).unwrap();
    // And still for themselves.
    pool.validate_can_pick_for(OWNER, OWNER, 1).unwrap();
    // An ordinary participant files only their own.
    pool.validate_can_pick_for(USER_2, USER_2, 1).unwrap();
    assert!(pool.validate_can_pick_for(USER_2, "francis-id", 1).is_err());
    assert!(pool.validate_can_pick_for(USER_2, USER_3, 1).is_err());
}

#[test]
fn an_assistant_may_pick_for_anybody() {
    let mut pool_settings = settings();
    pool_settings.assistants = vec![USER_2.to_string()];
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
    )
    .unwrap();
    for user in [OWNER, USER_2, USER_3] {
        pool.add_participant(user, user, 0).unwrap();
    }

    pool.validate_can_pick_for(USER_2, USER_3, 1).unwrap();
}

#[test]
fn picking_for_a_spot_that_is_out_says_whose_it_is() {
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

    // The organiser is not the one who is out, so the message must not say
    // "you have been eliminated".
    let error = pool
        .validate_can_pick_for(OWNER, USER_2, 2)
        .expect_err("an eliminated participant cannot be picked for");
    let message = error.to_string();
    assert!(message.contains(USER_2), "{message}");
    assert!(!message.contains("You have been"), "{message}");
}

#[test]
fn picking_for_somebody_who_is_not_in_the_pool_is_refused() {
    let pool = pool_with_participants();

    assert!(pool.validate_can_pick_for(OWNER, "stranger", 1).is_err());
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
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
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
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
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
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        "2026-10-04",
        "2026-10-10",
        before_the_season(),
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
    let pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
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

// ----------------------------------------------------------- public picks

/// The dates, as a pool with nothing of the schedule to hand sees them: none
/// begun. The caller of `revealed_weeks` resolves the real answer against the
/// league's fixture list, so these tests hand in the two cases rather than
/// deriving either.
fn nothing_begun() -> HashSet<u16> {
    HashSet::new()
}

/// A pool whose first `settled` weeks are settled, the next one carrying the
/// old hand-locked status when `locked`, and the rest still taking picks.
fn pool_with_week_statuses(settled: usize, locked: bool, public_picks: bool) -> SurvivorPool {
    let mut pool_settings = settings();
    pool_settings.public_picks = public_picks;
    let mut pool = SurvivorPool::new_at(
        "survivor",
        OWNER,
        &pool_settings,
        SEASON,
        SEASON_START,
        SEASON_END,
        before_the_season(),
    )
    .unwrap();

    for week in pool.weeks.iter_mut().take(settled) {
        week.status = WeekStatus::Settled;
    }
    if locked {
        pool.weeks[settled].status = WeekStatus::Locked;
    }
    pool
}

#[test]
fn a_private_pool_reveals_nothing_until_a_date_closes() {
    let pool = pool_with_week_statuses(0, false, false);

    assert!(pool.revealed_weeks(&nothing_begun()).is_empty());
    assert!(!pool.picks_are_public(1, false));
}

#[test]
fn a_date_that_has_begun_is_revealed() {
    let pool = pool_with_week_statuses(0, false, false);

    // The reveal the owner used to press a button for, and at the same moment
    // they used to press it: the date's first puck drop. Nothing on the pool
    // changed — week 1 is still taking picks as far as this document knows,
    // and on the teams yet to play it really still is.
    assert!(pool.picks_are_public(1, true));
    assert_eq!(pool.revealed_weeks(&HashSet::from([1])), vec![1]);
}

#[test]
fn a_date_past_its_deadline_is_revealed_whatever_the_setting_says() {
    for public_picks in [false, true] {
        let pool = pool_with_week_statuses(2, true, public_picks);

        // Two settled and the third closed by hand under the old rule: its
        // picks are in either way, and without the schedule being consulted.
        assert!(pool.picks_are_public(1, false));
        assert!(pool.picks_are_public(3, false));
        assert_eq!(&pool.revealed_weeks(&nothing_begun())[..3], &[1, 2, 3]);
    }
}

#[test]
fn a_public_pool_also_reveals_the_date_being_played() {
    let pool = pool_with_week_statuses(2, false, true);

    // Week 3 is the one being played, and still taking picks.
    assert_eq!(pool.current_week().map(|week| week.week), Some(3));
    assert!(pool.picks_are_public(3, false));
    assert_eq!(pool.revealed_weeks(&nothing_begun()), vec![1, 2, 3]);
}

#[test]
fn a_public_pool_keeps_a_pick_filed_ahead_private() {
    let pool = pool_with_week_statuses(2, false, true);

    // Any date still taking picks may be picked for, so somebody can be a
    // month ahead. Their plans are nobody's business until that date is the
    // one being played.
    assert!(!pool.picks_are_public(4, false));
    assert!(!pool.picks_are_public(5, false));
}

#[test]
fn a_week_the_pool_does_not_have_is_not_public() {
    let pool = pool_with_week_statuses(0, false, true);

    // Not even told its deadline has passed, which a caller could only say of
    // a week that exists.
    assert!(!pool.picks_are_public(0, true));
    assert!(!pool.picks_are_public(999, true));
}

// ------------------------------------------------- handing a spot to an account

const INVITEE: &str = "vincent@example.com";
const VINCENTS_ACCOUNT: &str = "vincents-account";

/// A pool with a managed spot the organiser keeps for somebody, which is the
/// case the whole flow exists for: that person has no account on the spot, so
/// they cannot pick for themselves.
fn pool_with_a_managed_spot() -> SurvivorPool {
    let mut pool = pool();
    pool.add_participant(OWNER, "jf", 0).unwrap();
    pool.add_managed_participant(OWNER, "spot-id", "vincent", 0)
        .unwrap();
    pool
}

#[test]
fn the_owner_files_an_invitation_against_an_address() {
    let mut pool = pool_with_a_managed_spot();

    pool.request_participant_link(OWNER, "spot-id", INVITEE, 1234)
        .unwrap();

    let links = pool.pending_participant_links.as_ref().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].participant_id, "spot-id");
    assert_eq!(links[0].date_requested, 1234);
    // The address itself is never stored: the pool is readable by anybody in
    // it, and a hint is all the owner needs to recognize what they filed.
    assert_eq!(links[0].email_hash, hash_email(INVITEE));
    assert_eq!(links[0].email_hint, mask_email(INVITEE));
    assert!(!links[0].email_hint.contains("vincent@"));
}

#[test]
fn an_address_matches_however_it_was_typed() {
    let mut pool = pool_with_a_managed_spot();

    pool.request_participant_link(OWNER, "spot-id", "  Vincent@Example.COM ", 0)
        .unwrap();

    // The invitee signs in with the address Hanko holds, which need not match
    // the owner's typing character for character.
    pool.accept_participant_link(VINCENTS_ACCOUNT, INVITEE, "spot-id")
        .unwrap();

    assert!(pool.participant(VINCENTS_ACCOUNT).is_some());
}

#[test]
fn only_the_owner_files_or_withdraws_an_invitation() {
    let mut pool = pool_with_a_managed_spot();
    pool.settings.assistants = vec![USER_2.to_string()];

    // An assistant may enter a spot, but not decide which account it answers
    // to — the same line settings and deletion sit on.
    assert!(
        pool.request_participant_link(USER_2, "spot-id", INVITEE, 0)
            .is_err()
    );

    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();
    assert!(pool.cancel_participant_link(USER_2, "spot-id").is_err());
    pool.cancel_participant_link(OWNER, "spot-id").unwrap();
    assert!(pool.pending_participant_links.as_ref().unwrap().is_empty());
}

#[test]
fn an_invitation_on_a_spot_that_is_not_in_the_pool_is_refused() {
    let mut pool = pool_with_a_managed_spot();

    assert!(
        pool.request_participant_link(OWNER, "nobody", INVITEE, 0)
            .is_err()
    );
}

#[test]
fn filing_a_second_invitation_replaces_the_first() {
    let mut pool = pool_with_a_managed_spot();

    pool.request_participant_link(OWNER, "spot-id", "typo@example.com", 0)
        .unwrap();
    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();

    // The owner correcting the address they typed, not a second candidate.
    let links = pool.pending_participant_links.as_ref().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].email_hash, hash_email(INVITEE));
}

#[test]
fn accepting_moves_the_spot_to_the_account_and_marks_it_owned() {
    let mut pool = pool_with_a_managed_spot();
    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();

    pool.accept_participant_link(VINCENTS_ACCOUNT, INVITEE, "spot-id")
        .unwrap();

    assert!(pool.participant("spot-id").is_none());
    let taken = pool.participant(VINCENTS_ACCOUNT).unwrap();
    // It keeps its name, and with it everything the standings shows.
    assert_eq!(taken.name, "vincent");
    assert!(taken.is_owned);
    // The invitation is spent.
    assert!(pool.pending_participant_links.as_ref().unwrap().is_empty());
}

#[test]
fn accepting_is_what_lets_that_person_finally_pick() {
    let mut pool = pool_with_a_managed_spot();
    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();

    // The whole point of the flow: before the link their account is a stranger
    // to the pool and the pick screen has nothing for them.
    assert!(pool.validate_can_pick(VINCENTS_ACCOUNT, 1).is_err());

    pool.accept_participant_link(VINCENTS_ACCOUNT, INVITEE, "spot-id")
        .unwrap();

    pool.validate_can_pick(VINCENTS_ACCOUNT, 1).unwrap();
}

#[test]
fn an_invitation_filed_for_somebody_else_cannot_be_accepted_or_declined() {
    let mut pool = pool_with_a_managed_spot();
    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();

    // The address comes from the caller's own verified JWT, so this is somebody
    // else trying to claim the spot.
    assert!(
        pool.accept_participant_link(USER_3, "stranger@example.com", "spot-id")
            .is_err()
    );
    assert!(
        pool.decline_participant_link("stranger@example.com", "spot-id")
            .is_err()
    );

    // And the invitation is still standing for the person it was filed for.
    pool.accept_participant_link(VINCENTS_ACCOUNT, INVITEE, "spot-id")
        .unwrap();
}

#[test]
fn declining_drops_the_invitation_and_leaves_the_spot_alone() {
    let mut pool = pool_with_a_managed_spot();
    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();

    pool.decline_participant_link(INVITEE, "spot-id").unwrap();

    assert!(pool.pending_participant_links.as_ref().unwrap().is_empty());
    // The organiser keeps picking for them, as before.
    let spot = pool.participant("spot-id").unwrap();
    assert!(!spot.is_owned);
}

#[test]
fn accepting_without_an_invitation_is_refused() {
    let mut pool = pool_with_a_managed_spot();

    assert!(
        pool.accept_participant_link(VINCENTS_ACCOUNT, INVITEE, "spot-id")
            .is_err()
    );
    assert!(pool.decline_participant_link(INVITEE, "spot-id").is_err());
}

#[test]
fn an_account_already_in_the_pool_cannot_take_a_second_spot() {
    let mut pool = pool_with_a_managed_spot();
    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();

    // The organiser already plays their own spot; taking this one as well would
    // give them two in the same pool.
    assert!(
        pool.accept_participant_link(OWNER, INVITEE, "spot-id")
            .is_err()
    );
}

#[test]
fn a_spot_that_is_already_your_account_is_refused() {
    let mut pool = pool_with_a_managed_spot();
    pool.request_participant_link(OWNER, "spot-id", INVITEE, 0)
        .unwrap();

    assert!(
        pool.accept_participant_link("spot-id", INVITEE, "spot-id")
            .is_err()
    );
}

#[test]
fn the_pool_changes_hands_when_the_owner_moves_their_own_spot() {
    let mut pool = pool_with_a_managed_spot();
    pool.settings.assistants = vec![OWNER.to_string()];
    pool.winners = Some(vec![OWNER.to_string()]);

    pool.request_participant_link(OWNER, OWNER, INVITEE, 0)
        .unwrap();
    pool.accept_participant_link(VINCENTS_ACCOUNT, INVITEE, OWNER)
        .unwrap();

    // Everywhere the pool document keys anything by that id.
    assert_eq!(pool.owner, VINCENTS_ACCOUNT);
    assert_eq!(pool.settings.assistants, vec![VINCENTS_ACCOUNT.to_string()]);
    assert_eq!(pool.winners, Some(vec![VINCENTS_ACCOUNT.to_string()]));
    pool.validate_owner_rights(VINCENTS_ACCOUNT).unwrap();
}
