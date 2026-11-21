//! Formatting, parsing, conversion and rounding of durations.
//! Input is always whole seconds; nothing here changes stored data.

use crate::model::{DurationFormat, ReportFormat, RoundDir};

/// Display a duration in each style.
pub fn format_duration(secs: i64, style: DurationFormat) -> String {
    let neg = secs < 0;
    let a = secs.abs();
    let body = match style {
        // h:mm:ss of truncated seconds
        DurationFormat::Improved => format!("{}:{:02}:{:02}", a / 3600, (a % 3600) / 60, a % 60),
        // hours with two places, then " h"
        DurationFormat::Decimal => format!("{} h", fixed_half_up(a as f64 / 3600.0, 2, false)),
        // h:mm:ss from an hour, m:ss min from a minute, else NN sec
        DurationFormat::Classic => {
            if a >= 3600 {
                format!("{}:{:02}:{:02}", a / 3600, (a % 3600) / 60, a % 60)
            } else if a >= 60 {
                format!("{}:{:02} min", a / 60, a % 60)
            } else {
                format!("{:02} sec", a)
            }
        }
    };
    if neg {
        format!("-{}", body)
    } else {
        body
    }
}

/// Timer-style clock used in the timer bar: always h:mm:ss.
pub fn clock(secs: i64) -> String {
    format_duration(secs, DurationFormat::Improved)
}

/// Decimal number with `places` places, halves rounded up, with `.` or `,`.
pub fn fixed_half_up(v: f64, places: u8, comma: bool) -> String {
    let p = 10f64.powi(places as i32);
    // Small epsilon so 0.785 (stored as 0.78499999…) still rounds its half up.
    let r = ((v * p) + 1e-9).round() / p;
    let s = format!("{:.*}", places as usize, r);
    if comma {
        s.replace('.', ",")
    } else {
        s
    }
}

/// Round seconds to a step of minutes. Step 0 or 1 still works in whole seconds
/// (step 1 rounds to whole minutes). Returns seconds.
pub fn round_secs(secs: i64, step_min: u32, dir: RoundDir) -> i64 {
    if step_min == 0 {
        return secs;
    }
    let step = step_min as i64 * 60;
    let q = secs.div_euclid(step);
    let r = secs.rem_euclid(step);
    let down = q * step;
    match dir {
        RoundDir::Down => down,
        RoundDir::Up => {
            if r == 0 {
                down
            } else {
                down + step
            }
        }
        RoundDir::Closest => {
            if r * 2 >= step {
                down + step
            } else {
                down
            }
        }
    }
}

/// Reporting value in a destination format. `secs` is already rounded if rounding applies.
pub fn format_report(secs: i64, fmt: ReportFormat, places: u8, comma: bool) -> String {
    let neg = secs < 0;
    let a = secs.abs();
    let s = match fmt {
        ReportFormat::HoursMinutes => {
            // Whole minutes; leftover seconds are rounded to the closest minute so 1:29:40 shows 1:30.
            let m = (a + 30) / 60;
            format!("{}:{:02}", m / 60, m % 60)
        }
        ReportFormat::DecimalHours => fixed_half_up(a as f64 / 3600.0, places, comma),
        ReportFormat::Minutes => format!("{}", (a + 30) / 60),
    };
    if neg {
        format!("-{}", s)
    } else {
        s
    }
}

/// The number of seconds a written report value stands for (display precision only):
/// h:mm and minutes are whole minutes (half up), decimal hours are 1/10 or 1/100 of an hour.
pub fn shown_secs(secs: i64, fmt: ReportFormat, places: u8) -> i64 {
    let a = secs.abs();
    let v = match fmt {
        ReportFormat::HoursMinutes | ReportFormat::Minutes => (a + 30) / 60 * 60,
        ReportFormat::DecimalHours => {
            let unit = if places >= 2 { 36 } else { 360 }; // 0.01 h = 36 s, 0.1 h = 360 s
            (a + unit / 2) / unit * unit
        }
    };
    if secs < 0 {
        -v
    } else {
        v
    }
}

/// Read what a person types as a duration.
/// `1:30` → 1 h 30 min, `1:30:15`, `1.5` / `1,5` → hours, `90` → minutes,
/// `2h 15m`, `45m`, `30s` also work.
/// An estimate as typed: a bare number is hours ("8", "1,5"), anything else is a duration
/// ("1:30", "90m"). Empty or zero means no estimate.
pub fn parse_estimate(input: &str) -> Option<i64> {
    let s = input.trim().to_lowercase().replace(',', ".");
    let secs = match s.parse::<f64>() {
        Ok(h) if h.is_finite() && h >= 0.0 => (h * 3600.0).round() as i64,
        _ => parse_duration(&s)?,
    };
    if secs > 0 { Some(secs) } else { None }
}

/// "47% used · 4 h 13 min left" or "over by 1 h 20 min": where the tracked time stands against
/// an estimate. The second value is true once the estimate is passed.
pub fn estimate_state(tracked: i64, estimate: i64) -> (i64, bool) {
    (if estimate > 0 { tracked * 100 / estimate } else { 0 }, tracked > estimate)
}

pub fn parse_duration(input: &str) -> Option<i64> {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        let nums: Option<Vec<i64>> = parts.iter().map(|p| p.trim().parse::<i64>().ok()).collect();
        let n = nums?;
        if n.iter().any(|x| *x < 0) {
            return None;
        }
        return match n.len() {
            2 => Some(n[0] * 3600 + n[1] * 60),
            3 => Some(n[0] * 3600 + n[1] * 60 + n[2]),
            _ => None,
        };
    }
    if s.chars().any(|c| c == 'h' || c == 'm' || c == 's') {
        let mut total = 0f64;
        let mut num = String::new();
        let mut any = false;
        for c in s.chars() {
            if c.is_ascii_digit() || c == '.' || c == ',' {
                num.push(if c == ',' { '.' } else { c });
            } else if c == 'h' || c == 'm' || c == 's' {
                let v: f64 = num.parse().ok()?;
                num.clear();
                any = true;
                total += match c {
                    'h' => v * 3600.0,
                    'm' => v * 60.0,
                    _ => v,
                };
            } else if c.is_whitespace() || c.is_alphabetic() {
                // "min", "hrs", "sec": the first letter already counted.
                continue;
            } else {
                return None;
            }
        }
        if !num.is_empty() {
            return None;
        }
        return if any { Some(total.round() as i64) } else { None };
    }
    let t = s.replace(',', ".");
    if t.contains('.') {
        let h: f64 = t.parse().ok()?;
        if h < 0.0 {
            return None;
        }
        return Some((h * 3600.0).round() as i64);
    }
    let m: i64 = t.parse().ok()?;
    if m < 0 {
        return None;
    }
    Some(m * 60)
}

/// Parse "HH:MM" (or "H:MM", "HHMM") as a time of day → seconds since midnight.
pub fn parse_clock(input: &str) -> Option<i64> {
    let s = input.trim();
    let (h, m) = if let Some((a, b)) = s.split_once(':') {
        (a.trim().parse::<i64>().ok()?, b.trim().parse::<i64>().ok()?)
    } else if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) {
        (s[..2].parse().ok()?, s[2..].parse().ok()?)
    } else {
        (s.parse::<i64>().ok()?, 0)
    };
    if !(0..24).contains(&h) || !(0..60).contains(&m) {
        return None;
    }
    Some(h * 3600 + m * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_examples() {
        let s = 47 * 60 + 6;
        assert_eq!(format_duration(s, DurationFormat::Classic), "47:06 min");
        assert_eq!(format_duration(s, DurationFormat::Improved), "0:47:06");
        assert_eq!(format_duration(s, DurationFormat::Decimal), "0.79 h");
        assert_eq!(format_duration(5, DurationFormat::Classic), "05 sec");
        assert_eq!(format_duration(3725, DurationFormat::Classic), "1:02:05");
        assert_eq!(format_duration(-3725, DurationFormat::Improved), "-1:02:05");
    }

    #[test]
    fn rounding() {
        assert_eq!(round_secs(7 * 60 + 29, 15, RoundDir::Closest), 0);
        assert_eq!(round_secs(7 * 60 + 30, 15, RoundDir::Closest), 15 * 60);
        assert_eq!(round_secs(61, 15, RoundDir::Up), 15 * 60);
        assert_eq!(round_secs(14 * 60 + 59, 15, RoundDir::Down), 0);
        assert_eq!(round_secs(90 * 60, 0, RoundDir::Up), 90 * 60);
    }

    #[test]
    fn report_formats() {
        assert_eq!(format_report(5400, ReportFormat::HoursMinutes, 2, false), "1:30");
        assert_eq!(format_report(5400, ReportFormat::DecimalHours, 2, false), "1.50");
        assert_eq!(format_report(5400, ReportFormat::DecimalHours, 2, true), "1,50");
        assert_eq!(format_report(5400, ReportFormat::Minutes, 2, false), "90");
    }

    #[test]
    fn parsing() {
        assert_eq!(parse_duration("1:30"), Some(5400));
        assert_eq!(parse_duration("1.5"), Some(5400));
        assert_eq!(parse_duration("1,5"), Some(5400));
        assert_eq!(parse_duration("90"), Some(5400));
        assert_eq!(parse_duration("1h 30m"), Some(5400));
        assert_eq!(parse_duration("45 min"), Some(2700));
        assert_eq!(parse_clock("9:05"), Some(9 * 3600 + 300));
        assert_eq!(parse_clock("1730"), Some(17 * 3600 + 1800));
    }

    #[test]
    fn estimates_read_bare_numbers_as_hours() {
        assert_eq!(parse_estimate("8"), Some(8 * 3600));
        assert_eq!(parse_estimate("1,5"), Some(5400));
        assert_eq!(parse_estimate("1:30"), Some(5400));
        assert_eq!(parse_estimate("90m"), Some(5400));
        assert_eq!(parse_estimate(""), None);
        assert_eq!(parse_estimate("0"), None);
        assert_eq!(estimate_state(1800, 3600), (50, false));
        assert_eq!(estimate_state(4000, 3600), (111, true));
    }
}
