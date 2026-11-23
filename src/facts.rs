//! Clean, structured facts about the person's time, for tools outside the app (the MCP server).
//!
//! Everything here is read-only and built on the same functions the app's own pages use
//! (`report::day_pieces`, `report::entries_in`, `report::timesheet`, `report::flags`), so every
//! number a tool returns matches what the person sees in DevTrack:
//!
//! - deleted records (kept for sync) are never shown;
//! - a session counts on each local day it overlaps, for exactly those seconds; a running one
//!   counts up to `now`;
//! - a task's estimate is the one that applies to it (`Task::planned_secs`): the source's for an
//!   imported work item, the person's own for a local task;
//! - missing links are named, never left empty: "(no project)", "(no client)", "(no task)".
//!
//! Durations are always given twice: exact seconds, for any arithmetic, and one form to read,
//! "4 h 18 min". Decimal hours (0,75) and clock-like h:mm (4:18) are left out on purpose: next to
//! each other they look like two different numbers, and h:mm reads as a time of day.
//! Secrets, integration settings and device details are never part of any fact.

use crate::model::*;
use crate::report::{self, Period, PeriodKind};
use chrono::{Datelike, Duration, Local, NaiveDate, Offset, TimeZone};
use serde::Serialize;
use std::collections::BTreeMap;

pub const NO_PROJECT: &str = "(no project)";
pub const NO_CLIENT: &str = "(no client)";
pub const NO_TASK: &str = "(no task)";

/// The longest period one question may cover, in days.
pub const MAX_DAYS: i64 = 366;
/// The most rows one answer carries; past it the answer says so and suggests grouping more.
pub const MAX_ROWS: usize = 500;

// ---------- units ----------

/// A duration to read: "45 min", "3 h", "4 h 18 min". Rounded down to the minute, like the app.
pub fn duration(secs: i64) -> String {
    let m = secs.max(0) / 60;
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02} min"),
    }
}

/// `part` of `whole` in whole percent.
fn percent(part: i64, whole: i64) -> i64 {
    if whole > 0 { ((part as f64) * 100.0 / whole as f64).round() as i64 } else { 0 }
}

fn local_iso(secs: i64) -> String {
    Local.timestamp_opt(secs, 0).single().map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)).unwrap_or_default()
}

fn zone_label(at: i64) -> String {
    let off = Local.timestamp_opt(at, 0).single().map(|d| d.offset().fix().local_minus_utc()).unwrap_or(0) / 60;
    let sign = if off < 0 { '-' } else { '+' };
    format!("UTC{}{:02}:{:02}", sign, off.abs() / 60, off.abs() % 60)
}

// ---------- periods ----------

/// Which days a question is about.
#[derive(Clone, Debug, PartialEq)]
pub enum PeriodSpec {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    /// Both days included.
    Range { from: NaiveDate, to: NaiveDate },
}

impl PeriodSpec {
    /// A preset name ("this_week") or "YYYY-MM-DD..YYYY-MM-DD" / one "YYYY-MM-DD".
    pub fn parse(s: &str) -> Result<PeriodSpec, String> {
        let s = s.trim();
        let preset = match s {
            "today" => Some(PeriodSpec::Today),
            "yesterday" => Some(PeriodSpec::Yesterday),
            "this_week" => Some(PeriodSpec::ThisWeek),
            "last_week" => Some(PeriodSpec::LastWeek),
            "this_month" => Some(PeriodSpec::ThisMonth),
            "last_month" => Some(PeriodSpec::LastMonth),
            _ => None,
        };
        if let Some(p) = preset {
            return Ok(p);
        }
        let date = |t: &str| NaiveDate::parse_from_str(t.trim(), "%Y-%m-%d").map_err(|_| format!("“{}” is not a date (YYYY-MM-DD)", t.trim()));
        match s.split_once("..") {
            Some((a, b)) => Ok(PeriodSpec::Range { from: date(a)?, to: date(b)? }),
            None => {
                let d = date(s)?;
                Ok(PeriodSpec::Range { from: d, to: d })
            }
        }
    }

    /// The days, given today and the person's first day of the week.
    pub fn resolve(&self, today: NaiveDate, monday: bool) -> Result<Period, String> {
        let p = match self {
            PeriodSpec::Today => Period::containing(PeriodKind::Day, today, monday),
            PeriodSpec::Yesterday => Period::containing(PeriodKind::Day, today - Duration::days(1), monday),
            PeriodSpec::ThisWeek => Period::containing(PeriodKind::Week, today, monday),
            PeriodSpec::LastWeek => Period::containing(PeriodKind::Week, today, monday).shift(-1, monday),
            PeriodSpec::ThisMonth => Period::containing(PeriodKind::Month, today, monday),
            PeriodSpec::LastMonth => Period::containing(PeriodKind::Month, today, monday).shift(-1, monday),
            PeriodSpec::Range { from, to } => {
                if to < from {
                    return Err("the period ends before it starts".into());
                }
                Period { kind: PeriodKind::Day, start: *from, end: *to + Duration::days(1) }
            }
        };
        let days = (p.end - p.start).num_days();
        if days > MAX_DAYS {
            return Err(format!("the period covers {} days; at most {} at a time", days, MAX_DAYS));
        }
        Ok(p)
    }
}

/// The days of an answer, both ends included.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PeriodInfo {
    pub from: String,
    pub to: String,
    pub days: i64,
    /// The ISO week number when the period is exactly one week.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week: Option<u32>,
}

impl PeriodInfo {
    pub fn of(p: &Period) -> PeriodInfo {
        let days = (p.end - p.start).num_days();
        PeriodInfo { from: p.start.to_string(), to: (p.end - Duration::days(1)).to_string(), days, week: (days == 7).then(|| week_number(p.start)) }
    }
}

/// The ISO week of a week starting on `start` (a Sunday start takes the week of its Monday).
pub fn week_number(start: NaiveDate) -> u32 {
    let monday = if start.weekday() == chrono::Weekday::Sun { start + Duration::days(1) } else { start };
    monday.iso_week().week()
}

/// What every answer says about itself, so numbers are never read without their frame.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Meta {
    /// When the answer was computed, local time.
    pub as_of: String,
    /// The zone days are counted in (the computer's).
    pub timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<PeriodInfo>,
    pub week_starts_on: &'static str,
    pub units: &'static str,
    /// A running session is counted up to `as_of`.
    pub running_counted_until_now: bool,
}

pub fn meta(db: &Db, period: Option<&Period>, now: i64) -> Meta {
    Meta {
        as_of: local_iso(now),
        timezone: zone_label(now),
        period: period.map(PeriodInfo::of),
        week_starts_on: if db.settings.week_starts_monday { "monday" } else { "sunday" },
        units: "seconds (exact, for arithmetic) and duration (to read, e.g. \"4 h 18 min\")",
        running_counted_until_now: true,
    }
}

// ---------- catalog ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ClientRow {
    pub client_id: Id,
    pub name: String,
    pub archived: bool,
    pub projects: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ProjectRow {
    pub project_id: Id,
    pub name: String,
    pub client_id: Option<Id>,
    pub client: String,
    pub archived: bool,
    pub billable_by_default: bool,
    pub estimate: Option<String>,
    pub estimate_seconds: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TaskRow {
    pub task_id: Id,
    pub title: String,
    pub project_id: Option<Id>,
    pub project: String,
    pub client: String,
    /// "manual" or "azure_devops", for filters.
    pub source: String,
    /// The same, to read: "Local" or "Azure DevOps".
    pub origin: String,
    /// "#4812" for a work item.
    pub work_item: Option<String>,
    pub work_item_type: Option<String>,
    /// The state in the source system ("Active", "Closed"…), empty for a local task.
    pub state: String,
    pub url: Option<String>,
    /// The estimate that applies: the source's for a work item, the person's for a local task.
    pub estimate: Option<String>,
    pub estimate_seconds: Option<i64>,
    pub done: bool,
    /// The source no longer returns it (deleted, moved or no access); its history stays.
    pub unavailable_in_source: bool,
}

fn client_name(db: &Db, project_id: &Option<Id>) -> String {
    db.project(project_id)
        .map(|p| db.client_name(p))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| NO_CLIENT.to_string())
}

fn project_name(db: &Db, project_id: &Option<Id>) -> String {
    db.project(project_id).map(|p| p.name.clone()).unwrap_or_else(|| NO_PROJECT.to_string())
}

pub fn clients(db: &Db, include_archived: bool) -> Vec<ClientRow> {
    db.live_clients()
        .into_iter()
        .filter(|c| include_archived || !c.archived)
        .map(|c| ClientRow { client_id: c.id.clone(), name: c.name.clone(), archived: c.archived, projects: db.projects_of_client(&c.id).len() })
        .collect()
}

pub fn projects(db: &Db, include_archived: bool) -> Vec<ProjectRow> {
    db.live_projects()
        .into_iter()
        .filter(|p| include_archived || !p.archived)
        .map(|p| ProjectRow {
            project_id: p.id.clone(),
            name: p.name.clone(),
            client_id: p.client_id.clone().filter(|id| db.client(&Some(id.clone())).is_some()),
            client: client_name(db, &Some(p.id.clone())),
            archived: p.archived,
            billable_by_default: p.billable_default,
            estimate: p.estimate_secs.map(duration),
            estimate_seconds: p.estimate_secs,
        })
        .collect()
}

pub fn task_row(db: &Db, t: &Task) -> TaskRow {
    TaskRow {
        task_id: t.id.clone(),
        title: t.title.clone(),
        project_id: t.project_id.clone().filter(|_| db.project(&t.project_id).is_some()),
        project: project_name(db, &t.project_id),
        client: client_name(db, &t.project_id),
        source: t.source.clone(),
        origin: source_label(&t.source).to_string(),
        work_item: t.external_id.as_ref().map(|x| format!("#{}", x)),
        work_item_type: t.external_type.clone(),
        state: t.status.clone(),
        url: t.url.clone(),
        estimate: t.planned_secs().map(duration),
        estimate_seconds: t.planned_secs(),
        done: t.done,
        unavailable_in_source: t.unavailable,
    }
}

pub fn tasks(db: &Db, include_done: bool) -> Vec<TaskRow> {
    let mut v: Vec<TaskRow> = db.live_tasks().filter(|t| include_done || !t.done).map(|t| task_row(db, t)).collect();
    v.sort_by(|a, b| (a.project.to_lowercase(), a.title.to_lowercase()).cmp(&(b.project.to_lowercase(), b.title.to_lowercase())));
    v
}

// ---------- time ----------

/// One session on one local day: the unit every total is a sum of.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TimeRow {
    pub date: String,
    pub entry_id: Id,
    /// Local time, ISO 8601, of the part of the session on this day.
    pub start: String,
    pub end: String,
    pub seconds: i64,
    pub duration: String,
    pub client_id: Option<Id>,
    pub client: String,
    pub project_id: Option<Id>,
    pub project: String,
    pub task_id: Option<Id>,
    pub task: String,
    pub work_item: Option<String>,
    /// "manual" or "azure_devops", for filters.
    pub source: String,
    /// The same, to read: "Local" or "Azure DevOps".
    pub origin: String,
    /// What the person wrote about the session.
    pub note: String,
    pub tags: Vec<String>,
    pub billable: bool,
    pub running: bool,
}

/// Which sessions a question is about. A name matches its record's name without regard to
/// case, or its id; `text` looks in the task title and the session's note.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub client: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    /// "manual" or "azure_devops".
    pub source: Option<String>,
    pub billable: Option<bool>,
    pub tag: Option<String>,
    pub text: Option<String>,
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

impl Filter {
    fn keeps(&self, r: &TimeRow) -> bool {
        let named = |want: &Option<String>, id: &Option<Id>, name: &str| match want {
            None => true,
            Some(w) => same(name, w) || id.as_deref().is_some_and(|i| i == w.trim()),
        };
        named(&self.client, &r.client_id, &r.client)
            && named(&self.project, &r.project_id, &r.project)
            && (named(&self.task, &r.task_id, &r.task) || self.task.as_deref().is_some_and(|w| r.work_item.as_deref().is_some_and(|x| same(x, w) || same(&x[1..], w))))
            && self.source.as_deref().is_none_or(|s| same(&r.source, s))
            && self.billable.is_none_or(|b| r.billable == b)
            && self.tag.as_deref().is_none_or(|t| r.tags.iter().any(|x| same(x, t)))
            && self.text.as_deref().is_none_or(|t| {
                let t = t.trim().to_lowercase();
                r.task.to_lowercase().contains(&t) || r.note.to_lowercase().contains(&t)
            })
    }
}

/// Every piece of every session in the period that passes the filter, oldest first.
pub fn time_rows(db: &Db, p: &Period, now: i64, filter: &Filter) -> Vec<TimeRow> {
    let mut out = vec![];
    for e in report::entries_in(db, p, now) {
        let task = db.task(&e.task_id);
        let project_id = e.project_id.clone().filter(|_| db.project(&e.project_id).is_some());
        let client_id = db.project(&project_id).and_then(|p| p.client_id.clone()).filter(|id| db.client(&Some(id.clone())).is_some());
        for (date, a, b) in report::day_pieces(e, now) {
            if !p.contains(date) {
                continue;
            }
            let row = TimeRow {
                date: date.to_string(),
                entry_id: e.id.clone(),
                start: local_iso(a),
                end: local_iso(b),
                seconds: b - a,
                duration: duration(b - a),
                client_id: client_id.clone(),
                client: client_name(db, &project_id),
                project_id: project_id.clone(),
                project: project_name(db, &project_id),
                task_id: task.map(|t| t.id.clone()),
                task: task.map(|t| t.title.clone()).unwrap_or_else(|| NO_TASK.to_string()),
                work_item: task.and_then(|t| t.external_id.as_ref()).map(|x| format!("#{}", x)),
                source: task.map(|t| t.source.clone()).unwrap_or_else(|| SOURCE_MANUAL.to_string()),
                origin: source_label(task.map(|t| t.source.as_str()).unwrap_or(SOURCE_MANUAL)).to_string(),
                note: e.description.trim().to_string(),
                tags: e.tags.clone(),
                billable: e.billable,
                running: e.is_running(),
            };
            if filter.keeps(&row) {
                out.push(row);
            }
        }
    }
    out.sort_by(|a, b| (&a.date, &a.start, &a.entry_id).cmp(&(&b.date, &b.start, &b.entry_id)));
    out
}

/// Where a task comes from, as a person says it.
pub fn source_label(code: &str) -> &str {
    match code {
        SOURCE_AZURE => "Azure DevOps",
        SOURCE_MANUAL => "Local",
        other => other,
    }
}

// ---------- pivot ----------

/// What time can be grouped by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dim {
    Day,
    Week,
    Month,
    Client,
    Project,
    Task,
    WorkItem,
    Source,
    Billable,
    Tag,
}

impl Dim {
    pub fn parse(s: &str) -> Result<Dim, String> {
        Ok(match s.trim() {
            "day" => Dim::Day,
            "week" => Dim::Week,
            "month" => Dim::Month,
            "client" => Dim::Client,
            "project" => Dim::Project,
            "task" => Dim::Task,
            "work_item" => Dim::WorkItem,
            "source" => Dim::Source,
            "billable" => Dim::Billable,
            "tag" => Dim::Tag,
            other => return Err(format!("“{}” can't be grouped by; use day, week, month, client, project, task, work_item, source, billable or tag", other)),
        })
    }

    pub const ALL: [&'static str; 10] = ["day", "week", "month", "client", "project", "task", "work_item", "source", "billable", "tag"];

    fn is_time(self) -> bool {
        matches!(self, Dim::Day | Dim::Week | Dim::Month)
    }

    /// (sort key, label, id) of a row on this dimension; a row may fall in several tags.
    fn keys(self, r: &TimeRow, monday: bool) -> Vec<(String, String, Option<Id>)> {
        let date = NaiveDate::parse_from_str(&r.date, "%Y-%m-%d").unwrap_or_default();
        match self {
            Dim::Day => vec![(r.date.clone(), r.date.clone(), None)],
            Dim::Week => {
                let s = report::week_start(date, monday);
                let label = format!("week {} ({} – {})", week_number(s), s, s + Duration::days(6));
                vec![(s.to_string(), label, None)]
            }
            Dim::Month => {
                let m = format!("{}-{:02}", date.year(), date.month());
                vec![(m.clone(), m, None)]
            }
            Dim::Client => vec![(r.client.to_lowercase(), r.client.clone(), r.client_id.clone())],
            Dim::Project => vec![(format!("{}\u{0}{}", r.client.to_lowercase(), r.project.to_lowercase()), r.project.clone(), r.project_id.clone())],
            Dim::Task => vec![(format!("{}\u{0}{}", r.project.to_lowercase(), r.task.to_lowercase()), r.task.clone(), r.task_id.clone())],
            Dim::WorkItem => {
                let w = r.work_item.clone().unwrap_or_else(|| "(no work item)".into());
                vec![(w.clone(), w, None)]
            }
            // readable label; the code ("manual", "azure_devops") stays as the key's id, for filters
            Dim::Source => vec![(r.source.clone(), source_label(&r.source).to_string(), Some(r.source.clone()))],
            Dim::Billable => {
                let b = if r.billable { "billable" } else { "not billable" };
                vec![(b.into(), b.into(), None)]
            }
            Dim::Tag => {
                if r.tags.is_empty() {
                    vec![("\u{10ffff}".into(), "(no tag)".into(), None)]
                } else {
                    r.tags.iter().map(|t| (t.to_lowercase(), t.clone(), None)).collect()
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Key {
    pub by: Dim,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GroupRow {
    pub keys: Vec<Key>,
    pub seconds: i64,
    pub duration: String,
    /// Distinct sessions with time in this group.
    pub sessions: usize,
    /// Share of the total, in whole percent.
    pub share_percent: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Totals {
    pub seconds: i64,
    pub duration: String,
    pub billable_seconds: i64,
    pub billable_duration: String,
    pub sessions: usize,
}

fn totals(rows: &[TimeRow]) -> Totals {
    let seconds: i64 = rows.iter().map(|r| r.seconds).sum();
    let billable: i64 = rows.iter().filter(|r| r.billable).map(|r| r.seconds).sum();
    let mut ids: Vec<&str> = rows.iter().map(|r| r.entry_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    Totals { seconds, duration: duration(seconds), billable_seconds: billable, billable_duration: duration(billable), sessions: ids.len() }
}

/// Time in a period, grouped on up to two dimensions: the one question behind every report.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TimeSummary {
    pub meta: Meta,
    pub group_by: Vec<Dim>,
    pub rows: Vec<GroupRow>,
    pub total: Totals,
    /// Things the reader must know to read the numbers right (rows left out, tags counted twice…).
    pub notes: Vec<String>,
    pub quality: Quality,
}

pub fn time_summary(db: &Db, p: &Period, now: i64, filter: &Filter, group_by: &[Dim]) -> Result<TimeSummary, String> {
    if group_by.len() > 2 {
        return Err("group by at most two dimensions".into());
    }
    if group_by.len() == 2 && group_by[0] == group_by[1] {
        return Err("the two dimensions must differ".into());
    }
    let monday = db.settings.week_starts_monday;
    let rows = time_rows(db, p, now, filter);
    let total = totals(&rows);
    // (sort keys) -> (keys, seconds, sessions)
    let mut groups: BTreeMap<Vec<String>, (Vec<Key>, i64, Vec<String>)> = BTreeMap::new();
    for r in &rows {
        let mut combos: Vec<(Vec<String>, Vec<Key>)> = vec![(vec![], vec![])];
        for d in group_by {
            let mut next = vec![];
            for (sk, ks) in &combos {
                for (s, label, id) in d.keys(r, monday) {
                    let mut sk2 = sk.clone();
                    sk2.push(s);
                    let mut ks2 = ks.clone();
                    ks2.push(Key { by: *d, label, id });
                    next.push((sk2, ks2));
                }
            }
            combos = next;
        }
        for (sk, ks) in combos {
            let g = groups.entry(sk).or_insert_with(|| (ks, 0, vec![]));
            g.1 += r.seconds;
            if !g.2.contains(&r.entry_id) {
                g.2.push(r.entry_id.clone());
            }
        }
    }
    let share = |s: i64| percent(s, total.seconds);
    let mut out: Vec<GroupRow> = groups
        .into_values()
        .map(|(keys, seconds, ids)| GroupRow { keys, seconds, duration: duration(seconds), sessions: ids.len(), share_percent: share(seconds) })
        .collect();
    // time dimensions read in calendar order; others, largest first
    if !group_by.first().is_some_and(|d| d.is_time()) {
        out.sort_by_key(|g| std::cmp::Reverse(g.seconds));
    }
    let mut notes = vec![];
    if group_by.contains(&Dim::Tag) {
        notes.push("A session with several tags counts under each of them, so the rows can add up to more than the total.".into());
    }
    if out.len() > MAX_ROWS {
        notes.push(format!("{} groups; only the first {} are listed. Group by less, or narrow the period or the filter.", out.len(), MAX_ROWS));
        out.truncate(MAX_ROWS);
    }
    if rows.iter().any(|r| r.running) {
        notes.push("A session is running: its time is counted up to now and will still grow.".into());
    }
    Ok(TimeSummary { meta: meta(db, Some(p), now), group_by: group_by.to_vec(), rows: out, total, notes, quality: quality(db, p, now) })
}

// ---------- estimates ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Task,
    Project,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EstimateRow {
    pub scope: Scope,
    pub id: Id,
    pub name: String,
    pub project: String,
    pub client: String,
    pub work_item: Option<String>,
    pub estimate: String,
    pub estimate_seconds: i64,
    /// All the time ever tracked on it, not only in a period.
    pub tracked: String,
    pub tracked_seconds: i64,
    /// What is left, or "over by …" when the estimate is exceeded.
    pub remaining: String,
    /// Negative when the estimate is exceeded.
    pub remaining_seconds: i64,
    /// Tracked against the estimate, in whole percent.
    pub progress_percent: i64,
    pub over_estimate: bool,
    pub done: bool,
}

/// Tasks or projects that have an estimate, with the time tracked against it, most used first.
pub fn estimates(db: &Db, scope: Scope, now: i64, filter: &Filter, only_over: bool) -> Vec<EstimateRow> {
    let row = |scope, id: &Id, name: String, project_id: &Option<Id>, work_item, est: i64, tracked: i64, done| EstimateRow {
        scope,
        id: id.clone(),
        name,
        project: project_name(db, project_id),
        client: client_name(db, project_id),
        work_item,
        estimate: duration(est),
        estimate_seconds: est,
        tracked: duration(tracked),
        tracked_seconds: tracked,
        remaining: if tracked > est { format!("over by {}", duration(tracked - est)) } else { duration(est - tracked) },
        remaining_seconds: est - tracked,
        progress_percent: percent(tracked, est),
        over_estimate: tracked > est,
        done,
    };
    let named = |want: &Option<String>, id: &str, name: &str| want.as_deref().is_none_or(|w| same(name, w) || id == w.trim());
    let mut out: Vec<EstimateRow> = match scope {
        Scope::Task => db
            .live_tasks()
            .filter_map(|t| {
                let est = t.planned_secs()?;
                let tr = task_row(db, t);
                let keep = named(&filter.project, tr.project_id.as_deref().unwrap_or(""), &tr.project)
                    && named(&filter.client, "", &tr.client)
                    && named(&filter.task, &t.id, &t.title)
                    && filter.source.as_deref().is_none_or(|s| same(&t.source, s));
                keep.then(|| row(Scope::Task, &t.id, t.title.clone(), &t.project_id, tr.work_item, est, db.task_total(&t.id, now), t.done))
            })
            .collect(),
        Scope::Project => db
            .live_projects()
            .into_iter()
            .filter_map(|p| {
                let est = p.estimate_secs?;
                let pid = Some(p.id.clone());
                let keep = named(&filter.project, &p.id, &p.name) && named(&filter.client, "", &client_name(db, &pid));
                let tracked: i64 = db.live_entries().filter(|e| e.project_id == pid).map(|e| e.duration(now)).sum();
                keep.then(|| row(Scope::Project, &p.id, p.name.clone(), &pid, None, est, tracked, p.archived))
            })
            .collect(),
    };
    if only_over {
        out.retain(|r| r.over_estimate);
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.tracked_seconds));
    out
}

// ---------- data quality ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SessionRef {
    pub entry_id: Id,
    pub date: String,
    pub start: String,
    pub duration: String,
    pub project: String,
    pub task: String,
    pub issue: String,
}

/// What would make a report wrong or incomplete. Reported, never corrected.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Quality {
    /// Sessions without a project, or with neither a task nor a note.
    pub needs_details: Vec<SessionRef>,
    pub running: Vec<SessionRef>,
    /// Overlapping, longer than 12 hours or shorter than a minute.
    pub worth_a_look: Vec<SessionRef>,
    /// Tasks with time in the period that the source no longer returns.
    pub unavailable_tasks: Vec<String>,
    /// True when nothing above needs attention.
    pub clean: bool,
}

fn session_ref(db: &Db, e: &TimeEntry, now: i64, issue: &str) -> SessionRef {
    SessionRef {
        entry_id: e.id.clone(),
        date: report::local_date(e.start).to_string(),
        start: local_iso(e.start),
        duration: duration(e.duration(now)),
        project: project_name(db, &e.project_id),
        task: db.task(&e.task_id).map(|t| t.title.clone()).unwrap_or_else(|| NO_TASK.into()),
        issue: issue.into(),
    }
}

pub fn quality(db: &Db, p: &Period, now: i64) -> Quality {
    let list = report::entries_in(db, p, now);
    let mut q = Quality::default();
    for e in &list {
        if report::needs_completion(e) {
            let why = if e.project_id.is_none() { "no project" } else { "neither a task nor a note" };
            q.needs_details.push(session_ref(db, e, now, why));
        }
        if e.is_running() {
            q.running.push(session_ref(db, e, now, "still running"));
        }
    }
    let mut seen: Vec<(Id, String)> = vec![];
    for f in report::flags(db, p, now) {
        if seen.iter().any(|(id, t)| id == &f.entry_id && t == &f.text) {
            continue;
        }
        seen.push((f.entry_id.clone(), f.text.clone()));
        if let Some(e) = db.entries.get(&f.entry_id) {
            q.worth_a_look.push(session_ref(db, e, now, &f.text));
        }
    }
    let mut gone: Vec<String> = list.iter().filter_map(|e| db.task(&e.task_id)).filter(|t| t.unavailable).map(|t| t.display()).collect();
    gone.sort();
    gone.dedup();
    q.unavailable_tasks = gone;
    q.needs_details.sort_by(|a, b| a.start.cmp(&b.start));
    q.clean = q.needs_details.is_empty() && q.running.is_empty() && q.worth_a_look.is_empty() && q.unavailable_tasks.is_empty();
    q
}

// ---------- report time ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct DestinationRow {
    pub profile_id: Id,
    pub name: String,
    pub destination: String,
    /// The rounding rule in words.
    pub rule: String,
    pub precision: String,
}

pub fn destinations(db: &Db) -> Vec<DestinationRow> {
    db.live_profiles()
        .into_iter()
        .map(|p| DestinationRow { profile_id: p.id.clone(), name: p.name.clone(), destination: p.destination.clone(), rule: p.rule_text(), precision: p.precision_text() })
        .collect()
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReportLine {
    pub date: String,
    pub code: String,
    pub activity: String,
    pub project: String,
    pub tasks: Vec<String>,
    /// The time really tracked behind the line.
    pub actual_seconds: i64,
    pub actual_duration: String,
    /// The value to enter in the destination, after its rules: never add it to tracked time.
    pub value: String,
    /// "not_ready", "provisional", "to_report", "reported" or "changed_after_reporting".
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReportTime {
    pub meta: Meta,
    pub destination: DestinationRow,
    pub lines: Vec<ReportLine>,
    pub actual_total_seconds: i64,
    pub actual_total_duration: String,
    pub lines_to_report: usize,
    pub lines_reported: usize,
}

/// The lines of one destination in a period, exactly as Report Time shows them.
pub fn report_time(db: &Db, profile: &str, p: &Period, now: i64) -> Result<ReportTime, String> {
    let prof = db
        .live_profiles()
        .into_iter()
        .find(|x| x.id == profile.trim() || same(&x.name, profile))
        .ok_or_else(|| format!("no destination “{}”; see the list of destinations", profile.trim()))?;
    let lines: Vec<ReportLine> = report::timesheet(db, prof, p, now)
        .into_iter()
        .map(|l| ReportLine {
            date: l.date.to_string(),
            code: l.code,
            activity: l.activity,
            project: if l.project.trim().is_empty() { NO_PROJECT.into() } else { l.project },
            tasks: l.tasks,
            actual_seconds: l.actual,
            actual_duration: duration(l.actual),
            value: l.value,
            status: match l.status {
                report::LineStatus::NotReady => "not_ready",
                report::LineStatus::Provisional => "provisional",
                report::LineStatus::ToReport => "to_report",
                report::LineStatus::Reported => "reported",
                report::LineStatus::ChangedAfterReporting => "changed_after_reporting",
            },
        })
        .collect();
    let actual: i64 = lines.iter().map(|l| l.actual_seconds).sum();
    Ok(ReportTime {
        meta: meta(db, Some(p), now),
        destination: destinations(db).into_iter().find(|d| d.profile_id == prof.id).expect("listed"),
        lines_to_report: lines.iter().filter(|l| matches!(l.status, "to_report" | "changed_after_reporting")).count(),
        lines_reported: lines.iter().filter(|l| l.status == "reported").count(),
        actual_total_seconds: actual,
        actual_total_duration: duration(actual),
        lines,
    })
}

// ---------- overview ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Overview {
    pub meta: Meta,
    /// First and last day with tracked time.
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    pub sessions: usize,
    pub clients: Vec<ClientRow>,
    pub projects: Vec<ProjectRow>,
    pub open_tasks: usize,
    pub destinations: Vec<DestinationRow>,
    /// Monday to Sunday.
    pub work_days: Vec<&'static str>,
    pub work_hours: String,
    pub running: Option<SessionRef>,
    pub this_week: Totals,
    pub last_week: Totals,
}

/// What there is to ask about: the first thing a tool reads.
pub fn overview(db: &Db, today: NaiveDate, now: i64) -> Overview {
    let monday = db.settings.week_starts_monday;
    let days: Vec<NaiveDate> = db.live_entries().map(|e| report::local_date(e.start)).collect();
    let week = |spec: PeriodSpec| spec.resolve(today, monday).map(|p| totals(&time_rows(db, &p, now, &Filter::default())));
    let names = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
    let s = &db.settings;
    Overview {
        meta: meta(db, None, now),
        first_day: days.iter().min().map(|d| d.to_string()),
        last_day: days.iter().max().map(|d| d.to_string()),
        sessions: days.len(),
        clients: clients(db, false),
        projects: projects(db, false),
        open_tasks: db.live_tasks().filter(|t| !t.done).count(),
        destinations: destinations(db),
        work_days: (0..7).filter(|i| s.work_days & (1 << i) != 0).map(|i| names[i]).collect(),
        work_hours: format!("{:02}:{:02}–{:02}:{:02}", s.work_start_min / 60, s.work_start_min % 60, s.work_end_min / 60, s.work_end_min % 60),
        running: db.running().map(|e| session_ref(db, e, now, "still running")),
        this_week: week(PeriodSpec::ThisWeek).unwrap_or_else(|_| totals(&[])),
        last_week: week(PeriodSpec::LastWeek).unwrap_or_else(|_| totals(&[])),
    }
}

// ---------- anonymizing ----------

/// A copy of the data to share outside the team: clients become "Client A", "Client B"… (in
/// alphabetical order, so the same data gives the same letters), their names are also replaced
/// where they appear inside project names and task titles, and sessions lose their notes.
/// Project and task names otherwise stay: they say what the work was.
pub fn anonymized(db: &Db) -> Db {
    let mut out = db.clone();
    let mut names: Vec<(Id, String)> = db.live_clients().into_iter().map(|c| (c.id.clone(), c.name.clone())).collect();
    names.sort_by_key(|(_, n)| n.to_lowercase());
    let alias = |i: usize| {
        let letters = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z"];
        if i < letters.len() { format!("Client {}", letters[i]) } else { format!("Client {}", i + 1) }
    };
    let pairs: Vec<(Id, String, String, Id)> = names.iter().enumerate().map(|(i, (id, n))| (id.clone(), n.clone(), alias(i), format!("client-{}", i + 1))).collect();
    let scrub = |text: &str| {
        let mut t = text.to_string();
        // longest names first, so "Northwind Traders" goes before "Northwind"
        let mut by_len: Vec<&(Id, String, String, Id)> = pairs.iter().filter(|p| !p.1.trim().is_empty()).collect();
        by_len.sort_by_key(|p| std::cmp::Reverse(p.1.len()));
        for (_, name, al, _) in by_len {
            let lower = t.to_lowercase();
            let needle = name.to_lowercase();
            let mut out = String::new();
            let mut last = 0;
            for (at, _) in lower.match_indices(&needle) {
                if at >= last && t.is_char_boundary(at) && t.is_char_boundary(at + needle.len()) {
                    out.push_str(&t[last..at]);
                    out.push_str(al);
                    last = at + needle.len();
                }
            }
            out.push_str(&t[last..]);
            t = out;
        }
        t
    };
    out.clients = pairs
        .iter()
        .map(|(id, _, al, new_id)| {
            let mut c = db.clients[id].clone();
            c.id = new_id.clone();
            c.name = al.clone();
            c.migrated_from.clear();
            (new_id.clone(), c)
        })
        .collect();
    let new_id = |old: &Option<Id>| old.as_ref().and_then(|o| pairs.iter().find(|p| &p.0 == o)).map(|p| p.3.clone());
    for p in out.projects.values_mut() {
        p.client_id = new_id(&p.client_id);
        p.client = p.client_id.as_ref().and_then(|id| pairs.iter().find(|x| &x.3 == id)).map(|x| x.2.clone()).unwrap_or_default();
        p.name = scrub(&p.name);
    }
    for t in out.tasks.values_mut() {
        t.title = scrub(&t.title);
        t.description.clear();
        t.external_project = t.external_project.as_deref().map(scrub);
        t.url = None;
    }
    for e in out.entries.values_mut() {
        e.description.clear();
    }
    for m in out.marks.values_mut() {
        m.activity = scrub(&m.activity);
    }
    for prof in out.profiles.values_mut() {
        prof.destination = scrub(&prof.destination);
        prof.name = scrub(&prof.name);
    }
    out.integrations.clear();
    out
}

/// The same filter for the anonymized copy: a client, project or task named as the person knows
/// it is turned into its id there, so "Northwind" still finds what is now "Client B".
pub fn anonymize_filter(db: &Db, f: &Filter) -> Filter {
    let mut clients: Vec<&Client> = db.live_clients();
    clients.sort_by_key(|c| c.name.to_lowercase());
    let client = f.client.as_ref().map(|w| {
        clients
            .iter()
            .position(|c| same(&c.name, w) || c.id == w.trim())
            .map(|i| format!("client-{}", i + 1))
            .unwrap_or_else(|| w.clone())
    });
    let project = f.project.as_ref().map(|w| db.live_projects().into_iter().find(|p| same(&p.name, w) || p.id == w.trim()).map(|p| p.id.clone()).unwrap_or_else(|| w.clone()));
    let task = f.task.as_ref().map(|w| db.live_tasks().find(|t| same(&t.title, w) || t.id == w.trim()).map(|t| t.id.clone()).unwrap_or_else(|| w.clone()));
    Filter { client, project, task, ..f.clone() }
}

// ---------- report tables ----------

/// A table ready to paste into a report: its numbers come from DevTrack, never from a model.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Table {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    /// The week (or any period) for a PM: by client and project, main tasks, estimates, quality.
    Weekly,
    /// One project: by task, then day by day (week by week past a month), with its estimates.
    Project,
    /// One client in a period: by project and by task.
    Client,
}

impl ReportKind {
    pub fn parse(s: &str) -> Result<ReportKind, String> {
        match s.trim() {
            "weekly" => Ok(ReportKind::Weekly),
            "project" => Ok(ReportKind::Project),
            "client" => Ok(ReportKind::Client),
            other => Err(format!("“{other}” is not a report; use weekly, project or client")),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ReportTables {
    pub meta: Meta,
    pub kind: ReportKind,
    /// The period as a report title says it: "week 47 · 2026-11-16 – 2026-11-22".
    pub period_title: String,
    pub total: Totals,
    pub tables: Vec<Table>,
    /// The tables in Markdown (one time column, "4 h 18 min", and the share), to paste as they are.
    pub markdown: String,
    /// The first table as CSV, with exact seconds, for a spreadsheet.
    pub csv: String,
    pub notes: Vec<String>,
    pub quality: Quality,
}

fn grouped_table(title: &str, s: &TimeSummary, heads: &[&str], limit: usize) -> Table {
    let mut columns: Vec<String> = heads.iter().map(|h| h.to_string()).collect();
    columns.extend(["Time", "Share", "Seconds"].map(String::from));
    let mut rows: Vec<Vec<String>> = s
        .rows
        .iter()
        .take(limit)
        .map(|g| {
            let mut r: Vec<String> = g.keys.iter().map(|k| k.label.clone()).collect();
            r.push(g.duration.clone());
            r.push(format!("{}%", g.share_percent));
            r.push(g.seconds.to_string());
            r
        })
        .collect();
    if s.rows.len() > limit {
        let rest: i64 = s.rows.iter().skip(limit).map(|g| g.seconds).sum();
        let mut r = vec![format!("{} more", s.rows.len() - limit)];
        r.extend(std::iter::repeat_n(String::new(), heads.len() - 1));
        r.push(duration(rest));
        r.push(format!("{}%", percent(rest, s.total.seconds)));
        r.push(rest.to_string());
        rows.push(r);
    }
    let mut total = vec!["Total".to_string()];
    total.extend(std::iter::repeat_n(String::new(), heads.len() - 1));
    total.push(s.total.duration.clone());
    total.push(if s.total.seconds > 0 { "100%".into() } else { String::new() });
    total.push(s.total.seconds.to_string());
    rows.push(total);
    Table { title: title.into(), columns, rows }
}

fn estimate_table(rows: &[EstimateRow]) -> Table {
    Table {
        title: "Estimates".into(),
        columns: ["Task", "Work item", "Estimate", "Tracked", "Remaining", "Progress"].map(String::from).to_vec(),
        rows: rows
            .iter()
            .map(|r| vec![r.name.clone(), r.work_item.clone().unwrap_or_default(), r.estimate.clone(), r.tracked.clone(), r.remaining.clone(), format!("{}%", r.progress_percent)])
            .collect(),
    }
}

/// A table in Markdown with one time column ("4 h 18 min") and the share, the total last in
/// bold. Exact seconds stay in the CSV.
pub fn markdown(t: &Table) -> String {
    let keep: Vec<usize> = (0..t.columns.len()).filter(|&i| t.columns[i] != "Seconds").collect();
    let cell = |s: &str| s.replace('|', "\\|");
    let row = |r: &Vec<String>, bold: bool| {
        let cells: Vec<String> = keep.iter().map(|&i| r.get(i).map(|c| cell(c)).unwrap_or_default()).map(|c| if bold && !c.is_empty() { format!("**{c}**") } else { c }).collect();
        format!("| {} |\n", cells.join(" | "))
    };
    let mut out = format!("**{}**\n\n", t.title);
    out.push_str(&format!("| {} |\n", keep.iter().map(|&i| cell(&t.columns[i])).collect::<Vec<_>>().join(" | ")));
    out.push_str(&format!("|{}|\n", keep.iter().map(|&i| if matches!(t.columns[i].as_str(), "Time" | "Share" | "Estimate" | "Tracked" | "Remaining" | "Progress" | "Completed (Azure)" | "Tracked (DevTrack)") { "---:" } else { "---" }).collect::<Vec<_>>().join("|")));
    for r in &t.rows {
        out.push_str(&row(r, r.first().is_some_and(|c| c == "Total")));
    }
    out
}

fn csv(t: &Table) -> String {
    let field = |s: &str| if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let mut out = t.columns.iter().map(|c| field(c)).collect::<Vec<_>>().join(",");
    out.push('\n');
    for r in &t.rows {
        out.push_str(&r.iter().map(|c| field(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

/// The tables of a report, computed here so the figures a PM reads are DevTrack's own.
pub fn report_tables(db: &Db, kind: ReportKind, p: &Period, now: i64, filter: &Filter) -> Result<ReportTables, String> {
    let summary = |dims: &[Dim]| time_summary(db, p, now, filter, dims);
    let mut tables = vec![];
    match kind {
        ReportKind::Weekly => {
            tables.push(grouped_table("Time by client and project", &summary(&[Dim::Client, Dim::Project])?, &["Client", "Project"], 40));
            tables.push(grouped_table("Main tasks", &summary(&[Dim::Task])?, &["Task"], 15));
            tables.push(grouped_table("By day", &summary(&[Dim::Day])?, &["Day"], 40));
        }
        ReportKind::Project => {
            if filter.project.is_none() {
                return Err("name the project (project)".into());
            }
            tables.push(grouped_table("By task", &summary(&[Dim::Task])?, &["Task"], 40));
            // the trend: day by day over a month or less, week by week beyond
            if (p.end - p.start).num_days() <= 31 {
                tables.push(grouped_table("By day", &summary(&[Dim::Day])?, &["Day"], 31));
            } else {
                tables.push(grouped_table("By week", &summary(&[Dim::Week])?, &["Week"], 60));
            }
        }
        ReportKind::Client => {
            if filter.client.is_none() {
                return Err("name the client (client)".into());
            }
            tables.push(grouped_table("By project", &summary(&[Dim::Project])?, &["Project"], 40));
            tables.push(grouped_table("By task", &summary(&[Dim::Project, Dim::Task])?, &["Project", "Task"], 40));
        }
    }
    // estimates of the tasks worked on in the period
    let worked: Vec<Id> = time_rows(db, p, now, filter).into_iter().filter_map(|r| r.task_id).collect();
    let est: Vec<EstimateRow> = estimates(db, Scope::Task, now, &Filter { client: filter.client.clone(), project: filter.project.clone(), ..Default::default() }, false)
        .into_iter()
        .filter(|r| worked.contains(&r.id))
        .collect();
    if !est.is_empty() {
        tables.push(estimate_table(&est));
    }
    let base = summary(&[])?;
    let md = tables.iter().map(markdown).collect::<Vec<_>>().join("\n");
    let first_csv = tables.first().map(csv).unwrap_or_default();
    let info = PeriodInfo::of(p);
    let span = if info.from == info.to { info.from.clone() } else { format!("{} – {}", info.from, info.to) };
    let period_title = match info.week {
        Some(w) => format!("week {w} · {span}"),
        None => span,
    };
    Ok(ReportTables { meta: base.meta, kind, period_title, total: base.total, markdown: md, csv: first_csv, tables, notes: base.notes, quality: base.quality })
}

// ---------- glossary ----------

/// The definitions behind every number, for a model to read before it writes a report.
pub const GLOSSARY: &str = "# DevTrack: how to read the numbers

- **Tracked time**: the real seconds of the sessions in the period. A running session counts up to the moment of the answer (`meta.as_of`).
- **Day**: a local day in the computer's time zone (`meta.timezone`). A session that crosses midnight counts on both days, for the part on each.
- **Week**: starts on `meta.week_starts_on`; `meta.period.week` is its number, to name it in a title (\"week 47\"). **Month**: the calendar month.
- **Durations**: `seconds` is exact, for any arithmetic; `duration` is the same time to read (\"4 h 18 min\"). In a report, write durations exactly as `duration` gives them: never as decimal hours (4,31) or as h:mm (4:18), which look like different numbers or a time of day. Shares are whole percentages.
- **Client → project → task**: a session belongs to a project (and through it to a client) and usually to a task. Missing links read \"(no project)\", \"(no client)\", \"(no task)\".
- **Work item**: a task imported from Azure DevOps (\"#4812\"). DevTrack only reads Azure DevOps, never changes it.
- **Estimate**: for a work item, its Original Estimate in Azure DevOps; for a local task or a project, the one set in DevTrack. **Progress** compares it with all the time ever tracked on it, not only the period's.
- **Value to enter** (Report Time): what a destination system (a company timesheet, a client's hours log) expects, after its own rounding and format (0,75 or 0.45). It is the only place for decimal hours. It differs from tracked time on purpose: report it as is and never add it to tracked time.
- **Quality**: sessions missing details, running, overlapping or unusual, and tasks gone from their source. Mention them in a report rather than leaving them out.
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{build_state, DemoState};

    fn now() -> i64 {
        crate::store::now_secs()
    }

    fn db() -> Db {
        build_state(now(), DemoState::Incomplete, true)
    }

    fn period(db: &Db, spec: PeriodSpec) -> Period {
        spec.resolve(report::today(), db.settings.week_starts_monday).unwrap()
    }

    #[test]
    fn totals_match_the_apps_own_summary() {
        let db = db();
        let n = now();
        let t = report::today();
        let last_30 = PeriodSpec::Range { from: t - Duration::days(29), to: t };
        for spec in [PeriodSpec::Today, PeriodSpec::ThisWeek, PeriodSpec::LastWeek, PeriodSpec::ThisMonth, PeriodSpec::LastMonth, last_30.clone()] {
            let p = period(&db, spec.clone());
            let s = report::summarize(&db, &p, n, None);
            if spec == last_30 {
                assert!(s.total > 0, "{spec:?} has time in the sample");
            }
            let t = time_summary(&db, &p, n, &Filter::default(), &[]).unwrap();
            assert_eq!(t.total.seconds, s.total, "{spec:?}");
            assert_eq!(t.total.billable_seconds, s.billable, "{spec:?}");
            // by project, as the app groups it
            let by_project = time_summary(&db, &p, n, &Filter::default(), &[Dim::Project]).unwrap();
            for (pid, secs) in &s.by_project {
                let row = by_project.rows.iter().find(|r| r.keys[0].id == *pid || (pid.is_none() && r.keys[0].label == NO_PROJECT)).expect("project row");
                assert_eq!(row.seconds, *secs, "{spec:?} {pid:?}");
            }
            assert_eq!(by_project.rows.iter().map(|r| r.seconds).sum::<i64>(), s.total);
            // by day, every day with time
            let by_day = time_summary(&db, &p, n, &Filter::default(), &[Dim::Day]).unwrap();
            for (d, secs) in s.by_day.iter().filter(|x| x.1 > 0) {
                let row = by_day.rows.iter().find(|r| r.keys[0].label == d.to_string()).expect("day row");
                assert_eq!(row.seconds, *secs);
            }
        }
    }

    #[test]
    fn two_dimensions_add_up_to_the_total() {
        let db = db();
        let n = now();
        let t0 = report::today();
        let p = period(&db, PeriodSpec::Range { from: t0 - Duration::days(29), to: t0 });
        let t = time_summary(&db, &p, n, &Filter::default(), &[Dim::Client, Dim::Project]).unwrap();
        assert!(t.rows.len() > 1, "several groups to add up");
        assert_eq!(t.rows.iter().map(|r| r.seconds).sum::<i64>(), t.total.seconds);
        assert!(t.rows.iter().all(|r| r.keys.len() == 2));
        let shares: i64 = t.rows.iter().map(|r| r.share_percent).sum();
        assert!(t.total.seconds == 0 || (shares - 100).abs() <= t.rows.len() as i64 / 2 + 1, "{shares}");
    }

    #[test]
    fn a_session_across_midnight_counts_on_both_days() {
        let mut db = db();
        let d = report::today() - Duration::days(2);
        let start = report::local_time_of(d, 23 * 3600);
        let e = TimeEntry { id: "night".into(), project_id: db.live_projects().first().map(|p| p.id.clone()), start, stop: Some(start + 2 * 3600), ..Default::default() };
        db.entries.insert(e.id.clone(), e);
        let p = PeriodSpec::Range { from: d, to: d + Duration::days(1) }.resolve(report::today(), true).unwrap();
        let rows: Vec<TimeRow> = time_rows(&db, &p, now(), &Filter::default()).into_iter().filter(|r| r.entry_id == "night").collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].seconds + rows[1].seconds, 2 * 3600);
        assert_eq!(rows[0].date, d.to_string());
    }

    #[test]
    fn deleted_records_never_show() {
        let mut db = db();
        let p = period(&db, PeriodSpec::ThisMonth);
        let before = time_summary(&db, &p, now(), &Filter::default(), &[]).unwrap().total.seconds;
        let id = time_rows(&db, &p, now(), &Filter::default()).iter().find(|r| !r.running).map(|r| r.entry_id.clone()).unwrap();
        db.entries.get_mut(&id).unwrap().deleted = true;
        let rows = time_rows(&db, &p, now(), &Filter::default());
        assert!(rows.iter().all(|r| r.entry_id != id));
        assert!(time_summary(&db, &p, now(), &Filter::default(), &[]).unwrap().total.seconds < before);
    }

    #[test]
    fn filters_match_names_without_case_and_ids() {
        let db = db();
        let n = now();
        let p = period(&db, PeriodSpec::ThisMonth);
        let all = time_rows(&db, &p, n, &Filter::default());
        let r = all.iter().find(|r| r.project_id.is_some()).unwrap();
        let by_name = time_rows(&db, &p, n, &Filter { project: Some(r.project.to_uppercase()), ..Default::default() });
        let by_id = time_rows(&db, &p, n, &Filter { project: r.project_id.clone(), ..Default::default() });
        assert!(!by_name.is_empty());
        assert_eq!(by_name, by_id);
        assert!(by_name.iter().all(|x| x.project == r.project));
    }

    #[test]
    fn estimates_use_the_estimate_that_applies() {
        let mut db = db();
        let n = now();
        // one local task with the person's estimate, one work item with the source's (and a
        // local one that must be ignored for it)
        let mut ids: Vec<Id> = db.live_tasks().filter(|t| db.task_total(&t.id, n) > 0).map(|t| t.id.clone()).collect();
        ids.sort();
        let (a, b) = (ids[0].clone(), ids[1].clone());
        {
            let t = db.tasks.get_mut(&a).unwrap();
            t.source = SOURCE_MANUAL.into();
            t.estimate_secs = Some(3600);
        }
        {
            let t = db.tasks.get_mut(&b).unwrap();
            t.source = SOURCE_AZURE.into();
            t.estimate_secs = Some(60);
            t.source_estimate_secs = Some(40 * 3600);
        }
        let rows = estimates(&db, Scope::Task, n, &Filter::default(), false);
        assert_eq!(rows.len(), 2);
        let wi = rows.iter().find(|r| r.id == b).unwrap();
        assert_eq!(wi.estimate_seconds, 40 * 3600, "a work item uses the source's estimate");
        assert_eq!(wi.estimate, "40 h");
        assert!(!wi.over_estimate);
        assert!(rows.iter().find(|r| r.id == a).unwrap().tracked_seconds > 0);
        assert_eq!(estimates(&db, Scope::Task, n, &Filter::default(), true).iter().all(|r| r.over_estimate), true);
        for row in rows {
            let t = db.tasks.get(&row.id).unwrap();
            assert_eq!(row.estimate_seconds, t.planned_secs().unwrap());
            assert_eq!(row.tracked_seconds, db.task_total(&t.id, n));
            assert_eq!(row.over_estimate, row.tracked_seconds > t.planned_secs().unwrap());
        }
    }

    #[test]
    fn report_time_lines_are_the_apps_lines() {
        let db = db();
        let n = now();
        let p = period(&db, PeriodSpec::ThisWeek);
        assert!(!db.live_profiles().is_empty(), "the sample has destinations");
        assert!(db.live_profiles().iter().any(|prof| !report::timesheet(&db, prof, &p, n).is_empty()), "and lines this week");
        for prof in db.live_profiles() {
            let app = report::timesheet(&db, prof, &p, n);
            let ours = report_time(&db, &prof.name.to_lowercase(), &p, n).unwrap();
            assert_eq!(ours.lines.len(), app.len());
            for (a, b) in ours.lines.iter().zip(&app) {
                assert_eq!(a.value, b.value);
                assert_eq!(a.actual_seconds, b.actual);
            }
        }
        assert!(report_time(&db, "no such destination", &p, n).is_err());
    }

    #[test]
    fn periods_parse_and_stay_bounded() {
        let t = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let p = PeriodSpec::parse("last_week").unwrap().resolve(t, true).unwrap();
        assert_eq!((p.start.to_string(), p.end.to_string()), ("2026-11-16".into(), "2026-11-23".into()));
        let r = PeriodSpec::parse("2026-11-01..2026-11-30").unwrap().resolve(t, true).unwrap();
        assert_eq!(PeriodInfo::of(&r).days, 30);
        assert_eq!(PeriodSpec::parse("2026-11-05").unwrap().resolve(t, true).unwrap().end.to_string(), "2026-11-06");
        assert!(PeriodSpec::parse("2025-01-01..2026-11-30").unwrap().resolve(t, true).is_err());
        assert!(PeriodSpec::parse("2026-11-30..2026-11-01").unwrap().resolve(t, true).is_err());
        assert!(PeriodSpec::parse("next_year").is_err());
    }

    #[test]
    fn quality_names_what_needs_details() {
        let db = db();
        let n = now();
        let p = period(&db, PeriodSpec::ThisWeek);
        let q = quality(&db, &p, n);
        let expected = report::entries_in(&db, &p, n).into_iter().filter(|e| report::needs_completion(e)).count();
        assert!(expected > 0, "the incomplete sample has sessions to complete");
        assert_eq!(q.needs_details.len(), expected);
        assert_eq!(q.running.len(), usize::from(db.running().is_some_and(|e| report::entries_in(&db, &p, n).iter().any(|x| x.id == e.id))));
        assert!(!q.clean || expected == 0);
    }

    #[test]
    fn nothing_secret_is_ever_serialized() {
        let mut db = db();
        let mut cfg = IntegrationConfig { id: "azure-1".into(), kind: SOURCE_AZURE.into(), name: "Azure".into(), enabled: true, ..Default::default() };
        cfg.settings.insert("organization".into(), "secret-org-name".into());
        db.integrations.insert(cfg.id.clone(), cfg);
        let n = now();
        let p = period(&db, PeriodSpec::ThisMonth);
        let json = serde_json::to_string(&(overview(&db, report::today(), n), time_summary(&db, &p, n, &Filter::default(), &[Dim::Task]).unwrap())).unwrap();
        assert!(!json.contains("secret-org-name"));
        assert!(!json.contains("device"));
    }

    #[test]
    fn units_are_consistent() {
        assert_eq!(duration(5400), "1 h 30 min");
        assert_eq!(duration(15480), "4 h 18 min");
        assert_eq!(duration(10800), "3 h");
        assert_eq!(duration(300), "5 min");
        assert_eq!(duration(59), "0 min");
        assert_eq!(duration(3600 + 5 * 60), "1 h 05 min");
        assert_eq!(week_number(NaiveDate::from_ymd_opt(2026, 11, 16).unwrap()), 47);
        assert_eq!(week_number(NaiveDate::from_ymd_opt(2026, 11, 15).unwrap()), 47, "a Sunday start counts as the week of its Monday");
    }
    #[test]
    fn anonymized_data_keeps_the_numbers_and_drops_the_names() {
        let db = db();
        let n = now();
        let anon = anonymized(&db);
        let p = period(&db, PeriodSpec::ThisWeek);
        let a = time_summary(&anon, &p, n, &Filter::default(), &[Dim::Client]).unwrap();
        let b = time_summary(&db, &p, n, &Filter::default(), &[Dim::Client]).unwrap();
        assert_eq!(a.total.seconds / 60, b.total.seconds / 60);
        assert_eq!(a.rows.len(), b.rows.len());
        let json = serde_json::to_string(&(overview(&anon, report::today(), n), time_rows(&anon, &p, n, &Filter::default()), report_tables(&anon, ReportKind::Weekly, &p, n, &Filter::default()).unwrap())).unwrap();
        for c in db.live_clients() {
            assert!(!json.to_lowercase().contains(&c.name.to_lowercase()), "{} leaked", c.name);
        }
        assert!(json.contains("Client A"));
        assert!(time_rows(&anon, &p, n, &Filter::default()).iter().all(|r| r.note.is_empty()));
    }

    #[test]
    fn report_tables_add_up_and_render() {
        let db = db();
        let n = now();
        let t0 = report::today();
        let p = PeriodSpec::Range { from: t0 - Duration::days(29), to: t0 }.resolve(t0, true).unwrap();
        let r = report_tables(&db, ReportKind::Weekly, &p, n, &Filter::default()).unwrap();
        let first = &r.tables[0];
        let total_row = first.rows.last().unwrap();
        assert_eq!(total_row[0], "Total");
        let secs_col = first.columns.iter().position(|c| c == "Seconds").unwrap();
        let sum: i64 = first.rows[..first.rows.len() - 1].iter().map(|row| row[secs_col].parse::<i64>().unwrap()).sum();
        assert_eq!(sum, r.total.seconds);
        assert_eq!(total_row[first.columns.iter().position(|c| c == "Time").unwrap()], r.total.duration);
        assert!(r.markdown.contains("**Time by client and project**"));
        assert!(r.markdown.contains(&format!("| **{}** |", r.total.duration)));
        assert!(r.markdown.contains("| Client | Project | Time | Share |"));
        assert!(!r.markdown.contains("Seconds"));
        // no decimal hours and no h:mm anywhere a person reads
        let text = r.markdown.clone();
        assert!(!text.chars().zip(text.chars().skip(1)).any(|(a, b)| a.is_ascii_digit() && b == ':'), "no h:mm in {text}");
        assert_eq!(r.csv.lines().count(), first.rows.len() + 1);
        assert!(report_tables(&db, ReportKind::Project, &p, n, &Filter::default()).is_err(), "a project report names its project");
    }


    #[test]
    fn sources_read_as_words() {
        let db = db();
        let p = period(&db, PeriodSpec::ThisMonth);
        let t = time_summary(&db, &p, now(), &Filter::default(), &[Dim::Source]).unwrap();
        let labels: Vec<&str> = t.rows.iter().map(|r| r.keys[0].label.as_str()).collect();
        assert!(labels.iter().all(|l| *l == "Azure DevOps" || *l == "Local"), "{labels:?}");
        assert!(t.rows.iter().all(|r| matches!(r.keys[0].id.as_deref(), Some("manual") | Some("azure_devops"))));
    }
}

