/*
The clock the pool's days are measured on.

Every rule that turns an instant into a pool day — the noon cutoff a roster move
is filed under, the day the live scoreboard is showing, the trade deadline — has
to agree on which day it is. `chrono::Local` cannot answer that: it reports the
timezone of whatever host the server happens to run on, and in production that
is a `debian-slim` container with no tzdata, so `Local` silently *is* `Utc`. A
waiver claim filed at 11:00 in Montreal reached the cutoff as 15:00, counted as
past noon, and was dated to the following day — on a screen that had just told
the pooler it would count for today.

The NHL schedules on Eastern time and so does this pool, so that is the zone the
days are cut on, named here once instead of inherited from the host. Nothing
outside this module should ask the host what day it is.
*/

use chrono::{DateTime, Duration, NaiveDate, Timelike, Utc};
use chrono_tz::{America::Toronto, Tz};

/// The zone every pool day is cut on. Eastern, with its DST rules, because that
/// is the clock the schedule — and every pooler reading it — runs on.
pub const POOL_TIMEZONE: Tz = Toronto;

/// The hour the pool's day turns over.
///
/// A pool day is not a calendar day. A west-coast game starting at 22:00 Eastern
/// finishes after midnight, and its points belong to the night it started on, so
/// the boundary sits at noon: by then the previous night is over and the next
/// one has not begun.
pub const DAY_CUTOFF_HOUR: u32 = 12;

/// Now, on the pool's clock.
pub fn pool_now() -> DateTime<Tz> {
    Utc::now().with_timezone(&POOL_TIMEZONE)
}

/// Today's calendar date on the pool's clock, for the rules that are about the
/// date on the wall rather than about a night of hockey — the trade deadline,
/// the end of the season.
pub fn pool_today() -> NaiveDate {
    pool_now().date_naive()
}

/// The night of hockey in progress at `at`: before noon, the games that finished
/// after midnight are still the previous day's.
pub fn game_day(at: DateTime<Tz>) -> NaiveDate {
    if at.hour() < DAY_CUTOFF_HOUR {
        at.date_naive() - Duration::days(1)
    } else {
        at.date_naive()
    }
}

/// The night of hockey in progress now.
pub fn current_game_day() -> NaiveDate {
    game_day(pool_now())
}

/// The day a roster move filed at `at` counts from: the next game day, since the
/// one in progress is already being scored off the lineup that was set for it.
pub fn roster_move_date(at: DateTime<Tz>) -> NaiveDate {
    game_day(at) + Duration::days(1)
}

/// The day a roster move filed now counts from.
pub fn current_roster_move_date() -> NaiveDate {
    roster_move_date(pool_now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// An instant given as a wall clock reading in Montreal, which is how every
    /// one of these rules is described to a pooler.
    fn eastern(date: &str, hour: u32, minute: u32) -> DateTime<Tz> {
        let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        POOL_TIMEZONE
            .from_local_datetime(&day.and_hms_opt(hour, minute, 0).unwrap())
            .single()
            .unwrap()
    }

    fn date(date: &str) -> NaiveDate {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn a_morning_move_counts_for_the_day_it_was_filed_on() {
        // The bug this module exists for: 11:00 in Montreal is 15:00 UTC, which
        // a host-timezone cutoff read as past noon and dated to the 13th.
        assert_eq!(
            roster_move_date(eastern("2026-10-12", 11, 0)),
            date("2026-10-12")
        );
    }

    #[test]
    fn an_afternoon_move_counts_for_the_next_day() {
        assert_eq!(
            roster_move_date(eastern("2026-10-12", 12, 0)),
            date("2026-10-13")
        );
        assert_eq!(
            roster_move_date(eastern("2026-10-12", 23, 59)),
            date("2026-10-13")
        );
    }

    #[test]
    fn the_last_minute_before_the_cutoff_still_counts_for_today() {
        assert_eq!(
            roster_move_date(eastern("2026-10-12", 11, 59)),
            date("2026-10-12")
        );
        assert_eq!(
            roster_move_date(eastern("2026-10-12", 0, 0)),
            date("2026-10-12")
        );
    }

    #[test]
    fn the_game_day_is_last_night_until_noon() {
        assert_eq!(game_day(eastern("2026-10-12", 0, 1)), date("2026-10-11"));
        assert_eq!(game_day(eastern("2026-10-12", 11, 59)), date("2026-10-11"));
        assert_eq!(game_day(eastern("2026-10-12", 12, 0)), date("2026-10-12"));
    }

    /// The cutoff is noon on the pooler's clock in both halves of the year, which
    /// is the whole reason the zone is named rather than an offset.
    #[test]
    fn the_cutoff_follows_daylight_saving() {
        // EDT, UTC-4: noon Eastern is 16:00 UTC.
        assert_eq!(eastern("2026-10-12", 12, 0).with_timezone(&Utc).hour(), 16);
        // EST, UTC-5: noon Eastern is 17:00 UTC.
        assert_eq!(eastern("2027-01-12", 12, 0).with_timezone(&Utc).hour(), 17);

        for day in ["2026-10-12", "2027-01-12"] {
            assert_eq!(roster_move_date(eastern(day, 11, 59)), date(day));
            assert_ne!(roster_move_date(eastern(day, 12, 0)), date(day));
        }
    }

    /// The two directions are one boundary: a move is filed for the night after
    /// the one being played.
    #[test]
    fn a_move_is_filed_for_the_night_after_the_one_in_progress() {
        for hour in 0..24 {
            let at = eastern("2026-12-05", hour, 30);
            assert_eq!(roster_move_date(at), game_day(at) + Duration::days(1));
        }
    }
}
