//! Periods, summaries and timesheet preparation. Reads entries, never changes them.
//!
//! Time allocation: a session counts on every local day it overlaps, for exactly the
//! overlapping seconds. Day bounds are computed from local dates (local midnight to the next local
//! midnight), so days of 23 h or 25 h around daylight-saving changes are handled; nothing assumes
//! 86 400 s per day. The reporting zone is the device's local zone; confirmations record its offset.

use crate::model::*;
use crate::timefmt::{format_report, round_secs};
use chrono::{Datelike, Duration, Local, NaiveDate, Offset, TimeZone};
use std::collections::{BTreeMap, BTreeSet};

pub fn local_date(secs: i64) -> NaiveDate {
    Local.timestamp_opt(secs, 0).single().map(|d| d.date_naive()).unwrap_or_default()
}

pub fn local_midnight(date: NaiveDate) -> i64 {
    local_time_of(date, 0)
}

/// Local wall-clock time on a date → epoch seconds. In a DST gap the earliest valid time is used.
pub fn local_time_of(date: NaiveDate, secs_of_day: i64) -> i64 {
    let naive = date.and_hms_opt(0, 0, 0).unwrap() + Duration::seconds(secs_of_day);
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(d) => d.timestamp(),
        chrono::LocalResult::Ambiguous(a, _) => a.timestamp(),
        chrono::LocalResult::None => {
            // skipped hour (spring forward): move an hour on
            let later = naive + Duration::hours(1);
            Local.from_local_datetime(&later).earliest().map(|d| d.timestamp()).unwrap_or_else(|| naive.and_utc().timestamp())
        }
    }
}

pub fn day_bounds(d: NaiveDate) -> (i64, i64) {
    (local_midnight(d), local_midnight(d + Duration::days(1)))
}

/// Current offset of the reporting zone, in minutes east of UTC.
pub fn zone_offset_min() -> i32 {
    Local::now().offset().fix().local_minus_utc() / 60
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn week_start(d: NaiveDate, monday: bool) -> NaiveDate {
    let wd = if monday { d.weekday().num_days_from_monday() } else { d.weekday().num_days_from_sunday() };
    d - Duration::days(wd as i64)
}

/// Seconds of `e` inside [from, to).
pub fn overlap(e: &TimeEntry, from: i64, to: i64, now: i64) -> i64 {
    let end = e.stop.unwrap_or(now).max(e.start);
    (end.min(to) - e.start.max(from)).max(0)
}

/// The pieces of a session per local day: (day, piece start, piece end).
pub fn day_pieces(e: &TimeEntry, now: i64) -> Vec<(NaiveDate, i64, i64)> {
    let end = e.stop.unwrap_or(now).max(e.start);
    if end == e.start {
        return vec![(local_date(e.start), e.start, e.start)];
    }
    let mut out = vec![];
    let mut d = local_date(e.start);
    for _ in 0..400 {
        let (a, b) = day_bounds(d);
        let s = e.start.max(a);
        let t = end.min(b);
        if t > s {
            out.push((d, s, t));
        }
        if b >= end {
            break;
        }
        d += Duration::days(1);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeriodKind {
    Day,
    Week,
    Month,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    pub kind: PeriodKind,
    /// First day (inclusive).
    pub start: NaiveDate,
    /// Day after the last (exclusive).
    pub end: NaiveDate,
}

impl Period {
    pub fn containing(kind: PeriodKind, d: NaiveDate, monday: bool) -> Period {
        match kind {
            PeriodKind::Day => Period { kind, start: d, end: d + Duration::days(1) },
            PeriodKind::Week => {
                let s = week_start(d, monday);
                Period { kind, start: s, end: s + Duration::days(7) }
            }
            PeriodKind::Month => {
                let s = d.with_day(1).unwrap();
                let e = if s.month() == 12 {
                    NaiveDate::from_ymd_opt(s.year() + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(s.year(), s.month() + 1, 1).unwrap()
                };
                Period { kind, start: s, end: e }
            }
        }
    }

    pub fn shift(&self, n: i32, monday: bool) -> Period {
        let anchor = match self.kind {
            PeriodKind::Day => self.start + Duration::days(n as i64),
            PeriodKind::Week => self.start + Duration::days(7 * n as i64),
            PeriodKind::Month => {
                let m0 = self.start.year() * 12 + self.start.month0() as i32 + n;
                NaiveDate::from_ymd_opt(m0.div_euclid(12), m0.rem_euclid(12) as u32 + 1, 1).unwrap()
            }
        };
        Period::containing(self.kind, anchor, monday)
    }

    pub fn days(&self) -> Vec<NaiveDate> {
        let mut v = vec![];
        let mut d = self.start;
        while d < self.end {
            v.push(d);
            d += Duration::days(1);
        }
        v
    }

    pub fn contains(&self, d: NaiveDate) -> bool {
        d >= self.start && d < self.end
    }

    pub fn bounds(&self) -> (i64, i64) {
        (local_midnight(self.start), local_midnight(self.end))
    }

    pub fn label(&self) -> String {
        let t = today();
        match self.kind {
            PeriodKind::Day => {
                if self.start == t {
                    format!("Today · {}", self.start.format("%a %d %b"))
                } else if self.start == t - Duration::days(1) {
                    format!("Yesterday · {}", self.start.format("%a %d %b"))
                } else {
                    self.start.format("%a %d %b %Y").to_string()
                }
            }
            PeriodKind::Week => {
                let last = self.end - Duration::days(1);
                let base = format!("{} – {}", self.start.format("%d %b"), last.format("%d %b %Y"));
                if self.contains(t) {
                    format!("This week · {}", base)
                } else {
                    base
                }
            }
            PeriodKind::Month => self.start.format("%B %Y").to_string(),
        }
    }
}

/// Entries that overlap a period (newest first).
pub fn entries_in<'a>(db: &'a Db, p: &Period, now: i64) -> Vec<&'a TimeEntry> {
    let (a, b) = p.bounds();
    let mut v: Vec<&TimeEntry> = db
        .live_entries()
        .filter(|e| {
            let end = e.stop.unwrap_or(now).max(e.start);
            (end > a && e.start < b) || (e.start == end && e.start >= a && e.start < b)
        })
        .collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.start));
    v
}

/// An entry missing what reporting needs: a description or a project.
/// A session needs details when it has no project, or says neither what it was (a task) nor
/// anything about it (a note).
pub fn needs_completion(e: &TimeEntry) -> bool {
    e.project_id.is_none() || (e.task_id.is_none() && e.description.trim().is_empty())
}

#[derive(Clone, Debug, PartialEq)]
pub enum FlagKind {
    /// Overlaps another session (its id).
    Overlap(Id),
    Long,
    Short,
}

#[derive(Clone, Debug)]
pub struct Flag {
    pub entry_id: Id,
    pub kind: FlagKind,
    pub text: String,
}

pub const LONG_SECS: i64 = 12 * 3600;
pub const SHORT_SECS: i64 = 60;

/// Things worth a look: overlapping sessions and unusual durations. Reported, never corrected.
pub fn flags(db: &Db, p: &Period, now: i64) -> Vec<Flag> {
    let mut list = entries_in(db, p, now);
    list.sort_by_key(|e| e.start);
    let mut out = vec![];
    for (i, e) in list.iter().enumerate() {
        let d = e.duration(now);
        if d > LONG_SECS {
            out.push(Flag { entry_id: e.id.clone(), kind: FlagKind::Long, text: "longer than 12 hours".into() });
        } else if e.stop.is_some() && d < SHORT_SECS {
            out.push(Flag { entry_id: e.id.clone(), kind: FlagKind::Short, text: "shorter than a minute".into() });
        }
        let end = e.stop.unwrap_or(now);
        for o in list.iter().skip(i + 1) {
            if o.start >= end {
                break;
            }
            out.push(Flag { entry_id: e.id.clone(), kind: FlagKind::Overlap(o.id.clone()), text: "overlaps another session".into() });
            out.push(Flag { entry_id: o.id.clone(), kind: FlagKind::Overlap(e.id.clone()), text: "overlaps another session".into() });
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub total: i64,
    pub billable: i64,
    pub entries: usize,
    pub by_day: Vec<(NaiveDate, i64)>,
    pub by_project: Vec<(Option<Id>, i64)>,
    /// (project, task) → seconds
    pub by_task: Vec<(Option<Id>, Option<Id>, i64)>,
    pub by_day_project: BTreeMap<NaiveDate, Vec<(Option<Id>, i64)>>,
}

pub fn summarize(db: &Db, p: &Period, now: i64, project_filter: Option<&Id>) -> Summary {
    let mut s = Summary::default();
    let mut day: BTreeMap<NaiveDate, i64> = p.days().into_iter().map(|d| (d, 0)).collect();
    let mut proj: BTreeMap<Option<Id>, i64> = BTreeMap::new();
    let mut task: BTreeMap<(Option<Id>, Option<Id>), i64> = BTreeMap::new();
    let mut dp: BTreeMap<NaiveDate, BTreeMap<Option<Id>, i64>> = BTreeMap::new();
    for e in entries_in(db, p, now) {
        if let Some(f) = project_filter {
            if e.project_id.as_ref() != Some(f) {
                continue;
            }
        }
        s.entries += 1;
        for (date, a, b) in day_pieces(e, now) {
            if !p.contains(date) {
                continue;
            }
            let d = b - a;
            s.total += d;
            if e.billable {
                s.billable += d;
            }
            *day.entry(date).or_default() += d;
            *proj.entry(e.project_id.clone()).or_default() += d;
            *task.entry((e.project_id.clone(), e.task_id.clone())).or_default() += d;
            *dp.entry(date).or_default().entry(e.project_id.clone()).or_default() += d;
        }
    }
    s.by_day = day.into_iter().collect();
    s.by_project = proj.into_iter().collect();
    s.by_project.sort_by_key(|x| std::cmp::Reverse(x.1));
    s.by_task = task.into_iter().map(|((p, t), d)| (p, t, d)).collect();
    s.by_task.sort_by_key(|x| std::cmp::Reverse(x.2));
    s.by_day_project = dp.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect();
    s
}

/// Seconds worked on one local day (pieces of all sessions).
pub fn day_total(db: &Db, d: NaiveDate, now: i64) -> i64 {
    let (a, b) = day_bounds(d);
    db.live_entries().map(|e| overlap(e, a, b, now)).sum()
}

// ---------- timesheet ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStatus {
    /// A required job code is missing.
    NotReady,
    /// Contains a running session: can't be confirmed yet.
    Provisional,
    ToReport,
    Reported,
    ChangedAfterReporting,
}

impl LineStatus {
    pub fn label(self) -> &'static str {
        match self {
            LineStatus::NotReady => "Not ready: code missing",
            LineStatus::Provisional => "Provisional: still running",
            LineStatus::ToReport => "To report",
            LineStatus::Reported => "Reported",
            LineStatus::ChangedAfterReporting => "Changed after reporting",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionPiece {
    pub entry_id: Id,
    pub start: i64,
    pub end: i64,
    pub secs: i64,
    pub description: String,
    pub running: bool,
}

#[derive(Clone, Debug)]
pub struct SheetLine {
    /// Id of the confirmation for this line (profile|date|group).
    pub key: Id,
    pub date: NaiveDate,
    pub date_label: String,
    pub code: String,
    pub activity: String,
    pub project_id: Option<Id>,
    pub project: String,
    pub project_color: Option<String>,
    pub tasks: Vec<String>,
    /// Links to the external tasks behind this line.
    pub external: Vec<(String, Option<String>)>,
    pub sessions: Vec<SessionPiece>,
    pub actual: i64,
    pub reported: i64,
    pub value: String,
    pub actual_value: String,
    pub line: String,
    pub unmapped: bool,
    pub ready: bool,
    pub provisional: bool,
    pub fingerprint: String,
    pub mark: Option<ReportedMark>,
    pub status: LineStatus,
}

pub struct Mapped {
    pub code: String,
    pub activity: String,
    pub unmapped: bool,
}

/// Task mapping first, then project mapping, then the profile's default code.
pub fn map_code(profile: &TimesheetProfile, project_id: &Option<Id>, task_id: &Option<Id>) -> Mapped {
    if task_id.is_some() {
        if let Some(m) = profile.mappings.iter().find(|m| m.task_id.is_some() && &m.task_id == task_id) {
            return Mapped { code: m.code.trim().to_string(), activity: m.activity.trim().to_string(), unmapped: m.code.trim().is_empty() };
        }
    }
    if project_id.is_some() {
        if let Some(m) = profile.mappings.iter().find(|m| m.task_id.is_none() && &m.project_id == project_id) {
            return Mapped { code: m.code.trim().to_string(), activity: m.activity.trim().to_string(), unmapped: m.code.trim().is_empty() };
        }
    }
    Mapped { code: profile.default_code.trim().to_string(), activity: String::new(), unmapped: true }
}

/// Duplicate, contradictory and incomplete mappings.
pub fn mapping_issues(profile: &TimesheetProfile, db: &Db) -> Vec<String> {
    let mut out = vec![];
    let mut seen: BTreeMap<(Option<Id>, Option<Id>), (String, String)> = BTreeMap::new();
    let name = |m: &CodeMapping| -> String {
        match (db.task(&m.task_id), db.project(&m.project_id)) {
            (Some(t), _) => format!("task “{}”", t.display()),
            (None, Some(p)) => format!("project “{}”", p.name),
            _ => "a mapping without project or task".into(),
        }
    };
    for m in &profile.mappings {
        if m.project_id.is_none() && m.task_id.is_none() {
            out.push("A mapping has neither a project nor a task.".into());
            continue;
        }
        if m.code.trim().is_empty() {
            out.push(format!("{} has no job code.", name(m)));
        }
        let key = (if m.task_id.is_some() { None } else { m.project_id.clone() }, m.task_id.clone());
        let val = (m.code.trim().to_string(), m.activity.trim().to_string());
        match seen.get(&key) {
            Some(prev) if *prev == val => out.push(format!("{} is mapped twice.", name(m))),
            Some(prev) => out.push(format!("{} is mapped to both {} and {}.", name(m), show(prev), show(&val))),
            None => {
                seen.insert(key, val);
            }
        }
        if let (Some(t), Some(_)) = (db.task(&m.task_id), &m.project_id) {
            if t.project_id != m.project_id {
                out.push(format!("{} now belongs to another project than the one in its mapping.", name(m)));
            }
        }
    }
    out
}

fn show(v: &(String, String)) -> String {
    if v.1.is_empty() {
        v.0.clone()
    } else {
        format!("{} / {}", v.0, v.1)
    }
}

pub fn render_value(profile: &TimesheetProfile, secs: i64) -> String {
    let s = format_report(secs, profile.format, profile.decimal_places, profile.decimal_comma);
    if profile.trim_zeros && profile.format == ReportFormat::DecimalHours && s.contains(['.', ',']) {
        return s.trim_end_matches('0').trim_end_matches(['.', ',']).to_string();
    }
    s
}

/// True when a session starting at `secs` falls on or before the lock date.
pub fn locked(db: &Db, secs: i64) -> bool {
    db.settings.lock_until.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).map(|d| local_date(secs) <= d).unwrap_or(false)
}

/// The period's sessions as a CSV table for a spreadsheet: one row per session, with the
/// tracked time and a column for every conversion. `;` separates the fields, as Excel expects
/// with a decimal comma; text is quoted when it needs to be.
pub fn sessions_csv(db: &Db, p: &Period, now: i64) -> String {
    let cell = |s: &str| if s.contains([';', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let profiles = db.live_profiles();
    let mut head: Vec<String> = ["Date", "Start", "End", "Duration", "Hours", "Description", "Project", "Client", "Task", "Reference", "Tags", "Billable"].iter().map(|s| s.to_string()).collect();
    head.extend(profiles.iter().map(|p| p.name.clone()));
    let mut out = vec![head.iter().map(|h| cell(h)).collect::<Vec<_>>().join(";")];
    let mut list = entries_in(db, p, now);
    list.sort_by_key(|e| e.start);
    for e in list {
        let secs = e.duration(now);
        let clock = |t: i64| chrono::TimeZone::timestamp_opt(&chrono::Local, t, 0).single().map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default();
        let project = db.project(&e.project_id);
        let task = db.task(&e.task_id);
        let mut row = vec![
            local_date(e.start).format("%d/%m/%Y").to_string(),
            clock(e.start),
            e.stop.map(clock).unwrap_or_default(),
            crate::timefmt::clock(secs),
            format!("{:.2}", secs as f64 / 3600.0).replace('.', ","),
            e.description.clone(),
            project.map(|p| p.name.clone()).unwrap_or_default(),
            project.map(|p| db.client_name(p)).unwrap_or_default(),
            task.map(|t| t.title.clone()).unwrap_or_default(),
            task.and_then(|t| t.external_id.clone()).map(|id| format!("#{}", id)).unwrap_or_default(),
            e.tags.join(", "),
            if e.billable { "1".into() } else { "0".into() },
        ];
        row.extend(profiles.iter().map(|p| convert(p, secs)));
        out.push(row.iter().map(|c| cell(c)).collect::<Vec<_>>().join(";"));
    }
    out.join("\r\n") + "\r\n"
}

/// One tracked duration as this conversion writes it: its rounding first, then its format.
pub fn convert(profile: &TimesheetProfile, secs: i64) -> String {
    let rounded = if profile.rounds() { crate::timefmt::round_secs(secs, profile.round_step_min, profile.round_dir) } else { secs };
    render_value(profile, rounded)
}

/// Why a destination value differs from the actual time: the company rule (rounding) and the
/// display precision are reported separately and never written back to the sessions.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueExplained {
    pub actual: i64,
    /// After the destination's company rounding (equal to `actual` without rounding).
    pub after_rule: i64,
    /// What the written value stands for.
    pub shown: i64,
    pub rule_diff: i64,
    pub precision_diff: i64,
}

pub fn explain_value(profile: &TimesheetProfile, actual: i64, after_rule: i64) -> ValueExplained {
    let shown = crate::timefmt::shown_secs(after_rule, profile.format, profile.decimal_places);
    ValueExplained { actual, after_rule, shown, rule_diff: after_rule - actual, precision_diff: shown - after_rule }
}

/// FNV-1a, enough to notice that something behind a line changed.
fn fnv(parts: &[String]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in parts {
        for b in p.bytes().chain(std::iter::once(0x1f)) {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    format!("{:016x}", h)
}

pub fn timesheet(db: &Db, profile: &TimesheetProfile, p: &Period, now: i64) -> Vec<SheetLine> {
    struct Acc {
        date: NaiveDate,
        code: String,
        activity: String,
        project_id: Option<Id>,
        mixed: bool,
        tasks: Vec<String>,
        external: Vec<(String, Option<String>)>,
        sessions: Vec<SessionPiece>,
        parts: Vec<String>,
        actual: i64,
        rounded_pieces: i64,
        unmapped: bool,
    }
    let mut groups: BTreeMap<String, Acc> = BTreeMap::new();
    let monday = db.settings.week_starts_monday;
    let mut list = entries_in(db, p, now);
    list.reverse();
    for e in list {
        let m = map_code(profile, &e.project_id, &e.task_id);
        let pid = e.project_id.clone().unwrap_or_default();
        let tid = e.task_id.clone().unwrap_or_default();
        for (date, a, b) in day_pieces(e, now) {
            if !p.contains(date) {
                continue;
            }
            let d = b - a;
            // Code and activity are always part of the key, so different destinations never merge.
            let ca = format!("{}|{}", m.code, m.activity);
            let (gdate, gkey) = match profile.grouping {
                Grouping::DayCode => (date, ca),
                Grouping::DayProject => (date, format!("p:{}|{}", pid, ca)),
                Grouping::DayTask => (date, format!("p:{}|t:{}|{}", pid, tid, ca)),
                Grouping::WeekCode => (week_start(date, monday), ca),
            };
            let key = format!("{}|{}|{}", profile.id, gdate, gkey);
            let acc = groups.entry(key).or_insert_with(|| Acc {
                date: gdate,
                code: m.code.clone(),
                activity: m.activity.clone(),
                project_id: e.project_id.clone(),
                mixed: false,
                tasks: vec![],
                external: vec![],
                sessions: vec![],
                parts: vec![],
                actual: 0,
                rounded_pieces: 0,
                unmapped: false,
            });
            if acc.project_id != e.project_id {
                acc.mixed = true;
            }
            acc.actual += d;
            acc.rounded_pieces += round_secs(d, profile.round_step_min, profile.round_dir);
            acc.unmapped |= m.unmapped;
            acc.sessions.push(SessionPiece { entry_id: e.id.clone(), start: a, end: b, secs: d, description: e.description.clone(), running: e.is_running() });
            acc.parts.push(format!("{}@{}-{}:{}:{}:{}", e.id, a, if e.is_running() { "run".to_string() } else { b.to_string() }, pid, tid, e.billable));
            if let Some(t) = db.task(&e.task_id) {
                let label = t.display();
                if !acc.tasks.contains(&label) {
                    acc.tasks.push(label);
                    if t.source != SOURCE_MANUAL {
                        acc.external.push((format!("{} #{}", t.source_label(), t.external_id.clone().unwrap_or_default()), t.url.clone()));
                    }
                }
            } else if !e.description.is_empty() && !acc.tasks.contains(&e.description) {
                acc.tasks.push(e.description.clone());
            }
        }
    }
    let offset = zone_offset_min();
    let rule = profile.rule_signature();
    let mut out: Vec<SheetLine> = groups
        .into_iter()
        .map(|(key, a)| {
            let reported = match profile.round_scope {
                RoundScope::None => a.actual,
                RoundScope::PerEntry => a.rounded_pieces,
                RoundScope::PerGroup => round_secs(a.actual, profile.round_step_min, profile.round_dir),
            };
            let project_id = if a.mixed { None } else { a.project_id.clone() };
            let project = db.project(&project_id);
            let value = render_value(profile, reported);
            let actual_value = render_value(profile, a.actual);
            let date_label = if profile.grouping == Grouping::WeekCode {
                format!("Week of {}", a.date.format(&profile.date_format))
            } else {
                a.date.format(&profile.date_format).to_string()
            };
            let project_name = match project {
                Some(p) => p.name.clone(),
                None if a.mixed => "several projects".into(),
                None => "no project".into(),
            };
            let task_text = a.tasks.join(", ");
            let line = profile
                .line_template
                .replace("{date}", &date_label)
                .replace("{code}", &a.code)
                .replace("{activity}", &a.activity)
                .replace("{project}", &project_name)
                .replace("{task}", &task_text)
                .replace("{value}", &value)
                .replace("{actual}", &actual_value);
            let provisional = a.sessions.iter().any(|s| s.running);
            let ready = !a.code.is_empty();
            let mut parts = a.parts.clone();
            parts.sort();
            parts.push(format!("code={}", a.code));
            parts.push(format!("act={}", a.activity));
            parts.push(format!("val={}", value));
            parts.push(format!("rule={}", rule));
            parts.push(format!("tz={}", offset));
            let fingerprint = fnv(&parts);
            let mark = db.marks.get(&key).filter(|m| !m.deleted).cloned();
            let status = if !ready {
                LineStatus::NotReady
            } else if provisional {
                LineStatus::Provisional
            } else {
                match &mark {
                    None => LineStatus::ToReport,
                    Some(m) if m.fingerprint == fingerprint => LineStatus::Reported,
                    Some(_) => LineStatus::ChangedAfterReporting,
                }
            };
            SheetLine {
                key,
                date: a.date,
                date_label,
                code: a.code,
                activity: a.activity,
                project_id,
                project: project_name,
                project_color: project.map(|p| p.color.clone()),
                tasks: a.tasks,
                external: a.external,
                sessions: a.sessions,
                actual: a.actual,
                reported,
                value,
                actual_value,
                line,
                unmapped: a.unmapped,
                ready,
                provisional,
                fingerprint,
                mark,
                status,
            }
        })
        .collect();
    out.sort_by(|a, b| a.date.cmp(&b.date).then(a.code.cmp(&b.code)).then(a.project.cmp(&b.project)));
    out
}

/// Confirmation snapshot for a line. Refused for provisional or not-ready lines.
pub fn confirm(line: &SheetLine, profile: &TimesheetProfile, now: i64) -> Option<ReportedMark> {
    if line.provisional || !line.ready {
        return None;
    }
    Some(ReportedMark {
        id: line.key.clone(),
        profile_id: profile.id.clone(),
        reported_minutes: line.reported / 60,
        value: line.value.clone(),
        reported_secs: line.reported,
        actual_secs: line.actual,
        code: line.code.clone(),
        activity: line.activity.clone(),
        rule: profile.rule_signature(),
        fingerprint: line.fingerprint.clone(),
        date_label: line.date_label.clone(),
        marked_at: now,
        ..Default::default()
    })
}

/// Confirmations of this profile in the period whose line no longer exists
/// (e.g. the code changed): they need another review.
pub fn orphan_marks(db: &Db, profile: &TimesheetProfile, p: &Period, lines: &[SheetLine]) -> Vec<ReportedMark> {
    let keys: BTreeSet<&str> = lines.iter().map(|l| l.key.as_str()).collect();
    let prefix = format!("{}|", profile.id);
    db.marks
        .values()
        .filter(|m| !m.deleted && m.id.starts_with(&prefix) && !keys.contains(m.id.as_str()))
        .filter(|m| {
            m.id[prefix.len()..]
                .split('|')
                .next()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .map(|d| p.contains(d))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// Per-day comparison of actual time and each profile's values (alternatives, not sums).
pub fn compare(db: &Db, profiles: &[&TimesheetProfile], p: &Period, now: i64) -> Vec<(NaiveDate, i64, Vec<i64>)> {
    let s = summarize(db, p, now, None);
    let per: Vec<BTreeMap<NaiveDate, i64>> = profiles
        .iter()
        .map(|pr| {
            let mut m = BTreeMap::new();
            for l in timesheet(db, pr, p, now) {
                *m.entry(l.date).or_insert(0) += l.reported;
            }
            m
        })
        .collect();
    s.by_day
        .into_iter()
        .map(|(d, a)| (d, a, per.iter().map(|m| m.get(&d).copied().unwrap_or(0)).collect()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, start: i64, stop: Option<i64>) -> TimeEntry {
        TimeEntry { id: id.into(), start, stop, description: "x".into(), ..Default::default() }
    }

    #[test]
    fn actual_never_changes() {
        let mut db = Db::default();
        let d = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
        let start = local_time_of(d, 9 * 3600);
        db.entries.insert("e".into(), TimeEntry { id: "e".into(), start, stop: Some(start + 47 * 60 + 6), ..Default::default() });
        let mut prof = TimesheetProfile::new("Test", "X");
        prof.format = ReportFormat::DecimalHours;
        prof.round_scope = RoundScope::PerGroup;
        prof.round_step_min = 15;
        prof.round_dir = RoundDir::Up;
        prof.default_code = "GEN".into();
        let p = Period::containing(PeriodKind::Day, d, true);
        let lines = timesheet(&db, &prof, &p, start + 10_000);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].actual, 47 * 60 + 6);
        assert_eq!(lines[0].reported, 60 * 60);
        assert_eq!(lines[0].value, "1,00");
        assert_eq!(db.entries["e"].stop, Some(start + 47 * 60 + 6));
    }

    #[test]
    fn midnight_split() {
        let mut db = Db::default();
        let d = NaiveDate::from_ymd_opt(2026, 11, 22).unwrap(); // Sunday: week and... boundary
        let start = local_time_of(d, 23 * 3600 + 1800);
        db.entries.insert("e".into(), entry("e", start, Some(start + 3600)));
        let now = start + 10 * 86400;
        let s1 = summarize(&db, &Period::containing(PeriodKind::Day, d, true), now, None);
        let s2 = summarize(&db, &Period::containing(PeriodKind::Day, d + Duration::days(1), true), now, None);
        assert_eq!(s1.total, 1800);
        assert_eq!(s2.total, 1800);
        // across the week boundary (Monday start)
        let w1 = summarize(&db, &Period::containing(PeriodKind::Week, d, true), now, None);
        let w2 = summarize(&db, &Period::containing(PeriodKind::Week, d + Duration::days(1), true), now, None);
        assert_eq!((w1.total, w2.total), (1800, 1800));
        // across a month boundary
        let m = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap();
        let ms = local_time_of(m, 23 * 3600 + 1800);
        db.entries.insert("m".into(), entry("m", ms, Some(ms + 3600)));
        let nov = summarize(&db, &Period::containing(PeriodKind::Month, m, true), now, None);
        let dec = summarize(&db, &Period::containing(PeriodKind::Month, m + Duration::days(1), true), now, None);
        assert_eq!(nov.total, 3600 + 1800); // both halves of the 22nd + the 30th before midnight
        assert_eq!(dec.total, 1800);
        // the timesheet agrees with the daily totals
        let mut prof = TimesheetProfile::new("T", "X");
        prof.default_code = "G".into();
        let lines = timesheet(&db, &prof, &Period::containing(PeriodKind::Day, d + Duration::days(1), true), now);
        assert_eq!(lines.iter().map(|l| l.actual).sum::<i64>(), 1800);
        // the stored entry is untouched
        assert_eq!(db.entries["e"].stop, Some(start + 3600));
    }

    #[test]
    fn running_since_yesterday() {
        let mut db = Db::default();
        let y = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let start = local_time_of(y, 22 * 3600);
        db.entries.insert("r".into(), entry("r", start, None));
        let now = local_time_of(y + Duration::days(1), 3600);
        assert_eq!(day_total(&db, y, now), 2 * 3600);
        assert_eq!(day_total(&db, y + Duration::days(1), now), 3600);
        let mut prof = TimesheetProfile::new("T", "X");
        prof.default_code = "G".into();
        let lines = timesheet(&db, &prof, &Period::containing(PeriodKind::Day, y, true), now);
        assert_eq!(lines[0].status, LineStatus::Provisional);
        assert!(confirm(&lines[0], &prof, now).is_none());
    }

    #[test]
    fn different_codes_never_merge() {
        let mut db = Db::default();
        let d = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
        let s = local_time_of(d, 9 * 3600);
        db.projects.insert("p".into(), Project { id: "p".into(), name: "P".into(), ..Default::default() });
        for (id, t, off) in [("a", "ta", 0), ("b", "tb", 7200)] {
            db.tasks.insert(t.into(), Task { id: t.into(), project_id: Some("p".into()), title: t.into(), ..Default::default() });
            db.entries.insert(id.into(), TimeEntry { id: id.into(), project_id: Some("p".into()), task_id: Some(t.into()), start: s + off, stop: Some(s + off + 3600), ..Default::default() });
        }
        let mut prof = TimesheetProfile::new("T", "X");
        prof.grouping = Grouping::DayProject;
        prof.mappings = vec![
            CodeMapping { project_id: Some("p".into()), task_id: Some("ta".into()), code: "A".into(), activity: "dev".into() },
            CodeMapping { project_id: Some("p".into()), task_id: Some("tb".into()), code: "B".into(), activity: "ops".into() },
        ];
        let lines = timesheet(&db, &prof, &Period::containing(PeriodKind::Day, d, true), s + 86400);
        assert_eq!(lines.len(), 2);
        let codes: Vec<&str> = lines.iter().map(|l| l.code.as_str()).collect();
        assert_eq!(codes, vec!["A", "B"]);
        assert_eq!(lines.iter().map(|l| l.actual).sum::<i64>(), 7200);
        // contradictory mapping is reported
        prof.mappings.push(CodeMapping { project_id: Some("p".into()), task_id: Some("ta".into()), code: "C".into(), activity: String::new() });
        assert!(mapping_issues(&prof, &db).iter().any(|i| i.contains("both")));
    }

    #[test]
    fn reported_snapshots() {
        let mut db = Db::default();
        let d = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
        let s = local_time_of(d, 9 * 3600);
        db.entries.insert("e".into(), entry("e", s, Some(s + 5820)));
        let mut a = TimesheetProfile::new("A", "X");
        a.id = "A".into();
        a.default_code = "G".into();
        let mut b = a.clone();
        b.id = "B".into();
        b.format = ReportFormat::Minutes;
        let p = Period::containing(PeriodKind::Day, d, true);
        let now = s + 86400;
        let la = timesheet(&db, &a, &p, now);
        assert_eq!(la[0].status, LineStatus::ToReport);
        let m = confirm(&la[0], &a, now).unwrap();
        db.marks.insert(m.id.clone(), m);
        assert_eq!(timesheet(&db, &a, &p, now)[0].status, LineStatus::Reported);
        assert_eq!(timesheet(&db, &b, &p, now)[0].status, LineStatus::ToReport);
        // a time change needs review
        db.entries.get_mut("e").unwrap().stop = Some(s + 5880);
        assert_eq!(timesheet(&db, &a, &p, now)[0].status, LineStatus::ChangedAfterReporting);
        db.entries.get_mut("e").unwrap().stop = Some(s + 5820);
        assert_eq!(timesheet(&db, &a, &p, now)[0].status, LineStatus::Reported);
        // a rule change needs review
        a.round_scope = RoundScope::PerGroup;
        assert_eq!(timesheet(&db, &a, &p, now)[0].status, LineStatus::ChangedAfterReporting);
        a.round_scope = RoundScope::None;
        // a code change moves the line: the old confirmation is listed for review
        a.default_code = "H".into();
        let l2 = timesheet(&db, &a, &p, now);
        assert_eq!(l2[0].status, LineStatus::ToReport);
        assert_eq!(orphan_marks(&db, &a, &p, &l2).len(), 1);
    }

    #[test]
    fn quarter_hour_example() {
        // 1 h 37 min: up to quarter hours → 1:45 (+8 min); without rounding 1:37.
        let mut prof = TimesheetProfile::new("T", "X");
        prof.round_scope = RoundScope::PerGroup;
        prof.round_step_min = 15;
        prof.round_dir = RoundDir::Up;
        let secs = 97 * 60;
        let r = round_secs(secs, prof.round_step_min, prof.round_dir);
        assert_eq!(render_value(&prof, r), "1:45");
        assert_eq!(r - secs, 8 * 60);
        prof.round_scope = RoundScope::None;
        assert_eq!(render_value(&prof, secs), "1:37");
    }

    #[test]
    fn display_precision_is_explained_apart_from_rounding() {
        let mut hm = TimesheetProfile::new("hm", "");
        hm.format = ReportFormat::HoursMinutes;
        let e = explain_value(&hm, 5564, 5564); // 1:32:44
        assert_eq!((render_value(&hm, 5564).as_str(), e.rule_diff, e.precision_diff), ("1:33", 0, 16));
        let e = explain_value(&hm, 40115, 40115); // 11:08:35
        assert_eq!((render_value(&hm, 40115).as_str(), e.precision_diff), ("11:09", 25));
        let e = explain_value(&hm, 5400, 5400); // exact minutes
        assert_eq!((e.rule_diff, e.precision_diff), (0, 0));
        let mut dec = TimesheetProfile::new("dec", "");
        dec.format = ReportFormat::DecimalHours;
        dec.decimal_places = 2;
        dec.decimal_comma = false;
        let e = explain_value(&dec, 2826, 2826); // 47:06
        assert_eq!((render_value(&dec, 2826).as_str(), e.precision_diff), ("0.79", 18));
        let mut min = TimesheetProfile::new("min", "");
        min.format = ReportFormat::Minutes;
        let e = explain_value(&min, 2826, 2826);
        assert_eq!((render_value(&min, 2826).as_str(), e.precision_diff), ("47", -6));
        // company rounding up to 15 min, decimal: the whole difference is the rule
        dec.round_scope = RoundScope::PerGroup;
        dec.round_dir = RoundDir::Up;
        dec.round_step_min = 15;
        let r = crate::timefmt::round_secs(5564, 15, RoundDir::Up);
        let e = explain_value(&dec, 5564, r);
        assert_eq!((render_value(&dec, r).as_str(), e.rule_diff, e.precision_diff), ("1.75", 736, 0));
    }

    #[test]
    fn csv_has_a_row_per_session_and_a_column_per_conversion() {
        let mut db = Db::default();
        let day = today();
        let start = local_time_of(day, 9 * 3600);
        db.projects.insert("p".into(), Project { id: "p".into(), name: "Portal; v2".into(), ..Default::default() });
        db.entries.insert("a".into(), TimeEntry { id: "a".into(), description: "Say \"hi\"".into(), project_id: Some("p".into()), start, stop: Some(start + 44 * 60), billable: true, ..Default::default() });
        let conv = TimesheetProfile::conversion("DevOps");
        db.profiles.insert(conv.id.clone(), conv);
        let csv = sessions_csv(&db, &Period::containing(PeriodKind::Day, day, true), start + 7200);
        let lines: Vec<&str> = csv.trim_end().split("\r\n").collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with(";Billable;DevOps"));
        assert!(lines[1].contains(";09:00:00;09:44:00;0:44:00;0,73;"));
        assert!(lines[1].contains("\"Say \"\"hi\"\"\";\"Portal; v2\";"), "{}", lines[1]);
        assert!(lines[1].ends_with(";1;0.75"));
        // the lock covers its own day and the days before it
        assert!(!locked(&db, start));
        db.settings.lock_until = Some(day.format("%Y-%m-%d").to_string());
        assert!(locked(&db, start) && locked(&db, start - 86400 * 3) && !locked(&db, start + 86400 * 2));
    }

    #[test]
    fn conversions_round_then_format() {
        // 2 h 34 min 22 s: DevOps wants decimal hours, Zucchetti hours:minutes, both to 15 min
        let secs = 2 * 3600 + 34 * 60 + 22;
        let devops = TimesheetProfile::conversion("DevOps");
        assert_eq!(convert(&devops, secs), "2.5");
        assert_eq!(convert(&devops, 44 * 60), "0.75");
        let mut z = TimesheetProfile::conversion("Zucchetti");
        z.format = ReportFormat::HoursMinutes;
        assert_eq!(convert(&z, secs), "2:30");
        assert_eq!(convert(&z, 44 * 60), "0:45");
        // rounding off: the exact value in the same format
        let mut exact = TimesheetProfile::conversion("Exact");
        exact.round_scope = RoundScope::None;
        assert_eq!(convert(&exact, secs), "2.57");
    }
}
