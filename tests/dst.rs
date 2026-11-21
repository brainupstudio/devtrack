//! Daylight-saving days, checked in their own process with a fixed zone (Europe/Rome).

use chrono::NaiveDate;
use devtrack_core::model::*;
use devtrack_core::report::*;

#[test]
fn dst_days_are_not_86400_seconds() {
    std::env::set_var("TZ", "Europe/Rome");
    // 29 Mar 2026: clocks go forward (23 h day); 25 Oct 2026: back (25 h day).
    let spring = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap();
    let autumn = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
    let (a, b) = day_bounds(spring);
    assert_eq!(b - a, 23 * 3600);
    let (a2, b2) = day_bounds(autumn);
    assert_eq!(b2 - a2, 25 * 3600);

    // A session from 22:00 the evening before to 02:00 local on the autumn day:
    // 2 h before midnight, and 3 real hours after it (02:00 happens after the repeated hour).
    let mut db = Db::default();
    let start = local_time_of(autumn.pred_opt().unwrap(), 22 * 3600);
    let stop = local_time_of(autumn, 2 * 3600 + 3600); // 03:00 local, after the repeated 02:00–03:00
    db.entries.insert("e".into(), TimeEntry { id: "e".into(), start, stop: Some(stop), ..Default::default() });
    let now = stop + 86400 * 3;
    let before = day_total(&db, autumn.pred_opt().unwrap(), now);
    let after = day_total(&db, autumn, now);
    assert_eq!(before, 2 * 3600);
    assert_eq!(before + after, stop - start);
}
