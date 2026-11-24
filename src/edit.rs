//! Turning the time fields of the session editor back into an interval.
//!
//! Rules:
//! - A field left as it was keeps its timestamp to the second: editing only the
//!   description, project, task or billability never changes start, stop or duration.
//! - The end has its own date, so an overnight or multi-day session is stated, never guessed.
//!   An end before the start is an error, not a jump to the next day.
//! - Changing the duration moves the end; changing the end recomputes the duration.
//! - A result longer than 24 hours needs an explicit second confirmation.

use crate::report::local_time_of;
use crate::timefmt::{parse_clock, parse_duration};
use chrono::NaiveDate;

pub const LONG_SESSION_SECS: i64 = 24 * 3600;

/// The editor's text fields, plus whether the duration was the last thing typed.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TimeFields {
    pub start_date: String,
    pub start: String,
    pub end_date: String,
    pub end: String,
    pub duration: String,
    pub duration_edited: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub start: i64,
    pub stop: Option<i64>,
    /// Longer than 24 hours: show the interval and ask again before saving.
    pub long: bool,
}

/// Field key and message (English; the interface translates it).
pub type FieldError = (&'static str, String);

fn date(s: &str, key: &'static str) -> Result<NaiveDate, FieldError> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|_| (key, "Enter a date as YYYY-MM-DD".to_string()))
}

fn clock(s: &str, key: &'static str) -> Result<i64, FieldError> {
    parse_clock(s).ok_or((key, "Enter a time from 00:00 to 23:59".to_string()))
}

/// `original` are the fields as first shown for (`orig_start`, `orig_stop`).
pub fn resolve(fields: &TimeFields, original: &TimeFields, orig_start: i64, orig_stop: Option<i64>) -> Result<Resolved, FieldError> {
    let start_same = fields.start_date.trim() == original.start_date.trim() && fields.start.trim() == original.start.trim();
    let start = if start_same {
        orig_start
    } else {
        local_time_of(date(&fields.start_date, "date")?, clock(&fields.start, "start")?)
    };
    let Some(old_stop) = orig_stop else {
        return Ok(Resolved { start, stop: None, long: false });
    };
    let stop = if fields.duration_edited && fields.duration.trim() != original.duration.trim() {
        let d = parse_duration(&fields.duration).ok_or(("dur", "Enter a duration like 1:30, 1.5 or 90".to_string()))?;
        start + d
    } else if fields.end_date.trim() == original.end_date.trim() && fields.end.trim() == original.end.trim() {
        old_stop
    } else {
        local_time_of(date(&fields.end_date, "end_date")?, clock(&fields.end, "end")?)
    };
    // Once the time is edited, the session is whole minutes: the part left as it was loses its
    // seconds too, so changing the start of 09:12:21–09:30:47 to 09:10 gives 09:10–09:30 and a
    // round 20 minutes, not 20:47. A session nothing was changed in keeps every second.
    let (start, stop) = if start == orig_start && stop == old_stop { (start, stop) } else { (start - start.rem_euclid(60), stop - stop.rem_euclid(60)) };
    if stop < start {
        return Err(("end", "The end is before the start. If the session ends on a later day, change the end date.".to_string()));
    }
    Ok(Resolved { start, stop: Some(stop), long: stop - start > LONG_SESSION_SECS })
}

/// The running session's start, edited while it keeps running ("now" is its end).
/// Typing the elapsed time sets the start to now minus that time; changing the start date or
/// time moves the start; untouched fields keep the start as it was, to the second. The start may
/// not be in the future.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RunningFields {
    pub start_date: String,
    pub start: String,
    pub elapsed: String,
    pub elapsed_edited: bool,
}

pub fn resolve_running(f: &RunningFields, original: &RunningFields, orig_start: i64, now: i64) -> Result<i64, FieldError> {
    let start = if f.elapsed_edited && f.elapsed.trim() != original.elapsed.trim() {
        let d = parse_duration(&f.elapsed).ok_or(("elapsed", "Enter a duration like 1:30, 1.5 or 90".to_string()))?;
        now - d
    } else if f.start_date.trim() == original.start_date.trim() && f.start.trim() == original.start.trim() {
        orig_start
    } else {
        local_time_of(date(&f.start_date, "date")?, clock(&f.start, "start")?)
    };
    if start > now {
        return Err(("start", "Start time cannot be in the future".to_string()));
    }
    Ok(start)
}

/// Step a clock text ("09:00") by minutes, wrapping round midnight; None when it isn't a time.
pub fn step_clock(s: &str, minutes: i64) -> Option<String> {
    let secs = parse_clock(s)?;
    let m = ((secs / 60 + minutes) % 1440 + 1440) % 1440;
    Some(format!("{:02}:{:02}", m / 60, m % 60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{local_date, local_time_of};
    use crate::model::DurationFormat;
    use crate::timefmt::format_duration;
    use chrono::{Local, TimeZone};

    fn fields_for(start: i64, stop: Option<i64>) -> TimeFields {
        let f = |s: i64| Local.timestamp_opt(s, 0).single().unwrap().format("%H:%M").to_string();
        let end = stop.unwrap_or(start);
        TimeFields {
            start_date: local_date(start).format("%Y-%m-%d").to_string(),
            start: f(start),
            end_date: local_date(end).format("%Y-%m-%d").to_string(),
            end: f(end),
            duration: format_duration(end - start, DurationFormat::Improved),
            duration_edited: false,
        }
    }

    fn at(d: (i32, u32, u32), h: i64, m: i64, s: i64) -> i64 {
        local_time_of(NaiveDate::from_ymd_opt(d.0, d.1, d.2).unwrap(), h * 3600 + m * 60 + s)
    }

    #[test]
    fn an_edited_time_leaves_no_seconds_on_either_end() {
        // 09:12:21 to 09:30:47, the start moved to 09:10 (as ↑/↓ would): 09:10 to 09:30
        let day = Local.with_ymd_and_hms(2026, 11, 24, 0, 0, 0).single().unwrap().timestamp();
        let (a, b) = (day + 9 * 3600 + 12 * 60 + 21, day + 9 * 3600 + 30 * 60 + 47);
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.start = "09:10".into();
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert_eq!(r.start, day + 9 * 3600 + 10 * 60);
        assert_eq!(r.stop, Some(day + 9 * 3600 + 30 * 60));
        // the end moved instead: the start loses its seconds too
        let mut f = orig.clone();
        f.end = "09:40".into();
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert_eq!(r.start, day + 9 * 3600 + 12 * 60);
        assert_eq!(r.stop, Some(day + 9 * 3600 + 40 * 60));
        // back to what it was: nothing changed, every second kept
        let r = resolve(&orig, &orig, a, Some(b)).unwrap();
        assert_eq!((r.start, r.stop), (a, Some(b)));
    }

    #[test]
    fn same_minute_session_with_seconds_is_kept() {
        // 12:51:25–12:51:52 (27 s) once became 23:59:35
        let (a, b) = (at((2026, 11, 17), 12, 51, 25), at((2026, 11, 17), 12, 51, 52));
        let orig = fields_for(a, Some(b));
        let r = resolve(&orig.clone(), &orig, a, Some(b)).unwrap();
        assert_eq!((r.start, r.stop), (a, Some(b)));
        assert_eq!(r.stop.unwrap() - r.start, 27);
        assert!(!r.long);
    }

    #[test]
    fn metadata_only_edits_keep_every_second() {
        for (a, b) in [
            (at((2026, 11, 9), 9, 2, 17), at((2026, 11, 9), 9, 19, 45)),
            (at((2026, 11, 16), 23, 30, 11), at((2026, 11, 17), 0, 30, 59)),
            (at((2026, 11, 14), 8, 0, 1), at((2026, 11, 16), 18, 0, 2)),
            (at((2026, 3, 29), 1, 15, 33), at((2026, 3, 29), 3, 45, 7)),
            (at((2026, 10, 25), 1, 15, 33), at((2026, 10, 25), 3, 45, 7)),
        ] {
            let orig = fields_for(a, Some(b));
            let r = resolve(&orig.clone(), &orig, a, Some(b)).unwrap();
            assert_eq!((r.start, r.stop), (a, Some(b)), "{:?}", orig);
        }
    }

    #[test]
    fn running_session_keeps_its_start() {
        let a = at((2026, 11, 17), 15, 16, 41);
        let orig = fields_for(a, None);
        let r = resolve(&orig.clone(), &orig, a, None).unwrap();
        assert_eq!((r.start, r.stop), (a, None));
    }

    #[test]
    fn normal_same_day_change() {
        let (a, b) = (at((2026, 11, 9), 9, 2, 17), at((2026, 11, 9), 9, 19, 45));
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.end = "10:00".into();
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert_eq!(r.start, at((2026, 11, 9), 9, 2, 0), "the untouched start loses its seconds: whole minutes");
        assert_eq!(r.stop, Some(at((2026, 11, 9), 10, 0, 0)));
    }

    #[test]
    fn end_before_start_is_an_error_not_a_day() {
        let (a, b) = (at((2026, 11, 9), 12, 51, 25), at((2026, 11, 9), 12, 51, 52));
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.end = "12:40".into();
        let e = resolve(&f, &orig, a, Some(b)).unwrap_err();
        assert_eq!(e.0, "end");
    }

    #[test]
    fn real_overnight_session_uses_the_end_date() {
        let (a, b) = (at((2026, 11, 9), 22, 0, 0), at((2026, 11, 9), 23, 0, 0));
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.end_date = "2026-11-10".into();
        f.end = "01:30".into();
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert_eq!(r.stop, Some(at((2026, 11, 10), 1, 30, 0)));
        assert!(!r.long);
    }

    #[test]
    fn multi_day_result_asks_for_confirmation() {
        let (a, b) = (at((2026, 11, 9), 9, 0, 0), at((2026, 11, 9), 10, 0, 0));
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.end_date = "2026-11-11".into();
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert!(r.long);
    }

    #[test]
    fn explicit_duration_moves_the_end() {
        let (a, b) = (at((2026, 11, 9), 9, 2, 17), at((2026, 11, 9), 9, 19, 45));
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.duration = "1:30".into();
        f.duration_edited = true;
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert_eq!((r.start, r.stop), (at((2026, 11, 9), 9, 2, 0), Some(at((2026, 11, 9), 10, 32, 0))), "a round 1:30");
    }

    #[test]
    fn retyping_the_same_duration_changes_nothing() {
        let (a, b) = (at((2026, 11, 9), 9, 2, 17), at((2026, 11, 9), 9, 19, 45));
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.duration_edited = true;
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert_eq!(r.stop, Some(b));
    }

    #[test]
    fn moving_the_start_keeps_an_untouched_end_to_the_minute() {
        let (a, b) = (at((2026, 11, 9), 9, 2, 17), at((2026, 11, 9), 9, 19, 45));
        let orig = fields_for(a, Some(b));
        let mut f = orig.clone();
        f.start = "09:00".into();
        let r = resolve(&f, &orig, a, Some(b)).unwrap();
        assert_eq!((r.start, r.stop), (at((2026, 11, 9), 9, 0, 0), Some(at((2026, 11, 9), 9, 19, 0))));
    }

    #[test]
    fn running_start_moves_by_time_date_or_elapsed() {
        let now = at((2026, 11, 17), 17, 52, 38);
        let a = at((2026, 11, 17), 17, 0, 41);
        let orig = RunningFields { start_date: "2026-11-17".into(), start: "17:00".into(), elapsed: "0:51:57".into(), elapsed_edited: false };
        assert_eq!(resolve_running(&orig.clone(), &orig, a, now).unwrap(), a, "untouched keeps seconds");
        let mut f = orig.clone();
        f.start = "16:30".into();
        assert_eq!(resolve_running(&f, &orig, a, now).unwrap(), at((2026, 11, 17), 16, 30, 0));
        let mut f = orig.clone();
        f.start_date = "2026-11-16".into();
        f.start = "22:00".into();
        assert_eq!(resolve_running(&f, &orig, a, now).unwrap(), at((2026, 11, 16), 22, 0, 0));
        let mut f = orig.clone();
        f.elapsed = "1:30".into();
        f.elapsed_edited = true;
        assert_eq!(resolve_running(&f, &orig, a, now).unwrap(), now - 5400);
        let mut f = orig.clone();
        f.start = "18:30".into();
        assert_eq!(resolve_running(&f, &orig, a, now).unwrap_err().0, "start");
    }

    #[test]
    fn clock_steps_wrap_round_midnight() {
        assert_eq!(step_clock("09:00", 1).as_deref(), Some("09:01"));
        assert_eq!(step_clock("00:05", -15).as_deref(), Some("23:50"));
        assert_eq!(step_clock("23:59", 1).as_deref(), Some("00:00"));
        assert_eq!(step_clock("nope", 1), None);
    }
}

/// The editor's logic before this fix, kept only to document what went wrong (see VERIFICATION.md).
#[cfg(test)]
mod before_fix {
    use crate::report::local_time_of;
    use chrono::{Duration, NaiveDate};

    /// Same-minute end: the minute-precision end (12:51:00) fell before the start with seconds
    /// (12:51:25), moved to the next day, and the "same minute as before" check then failed.
    fn old_stop(date: NaiveDate, start: i64, end_secs_of_day: i64, old: i64) -> i64 {
        let mut stop = local_time_of(date, end_secs_of_day);
        if stop < start {
            stop = local_time_of(date + Duration::days(1), end_secs_of_day);
        }
        if stop / 60 == old / 60 {
            stop = old;
        }
        stop
    }

    #[test]
    fn old_logic_turned_27_seconds_into_almost_a_day() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let (a, b) = (local_time_of(d, 12 * 3600 + 51 * 60 + 25), local_time_of(d, 12 * 3600 + 51 * 60 + 52));
        let stop = old_stop(d, a, 12 * 3600 + 51 * 60, b);
        assert_eq!(stop - a, 23 * 3600 + 59 * 60 + 35);
    }
}
