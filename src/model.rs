//! Data model. One representation of time everywhere: start/stop are UTC epoch seconds,
//! durations are whole seconds. Formats, conversions and rounding never write back here.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Id = String;

pub fn new_id() -> Id {
    uuid::Uuid::new_v4().to_string()
}

/// Project colours, given in turn to new projects: distinct hues of similar strength, readable
/// on both themes.
pub const PROJECT_COLORS: [&str; 14] = [
    "#0B83D9", "#9E5BD9", "#D94182", "#E36A00", "#BF7000", "#C7AF14", "#D92B2B", "#2DA608",
    "#06A893", "#C9806B", "#465BB3", "#990099", "#566614", "#525266",
];

/// Revision of a record: newest wins when two devices disagree.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rev {
    pub at_ms: i64,
    pub seq: u32,
    pub device: String,
}

pub trait Record: Clone {
    fn id(&self) -> &str;
    fn rev(&self) -> &Rev;
    fn rev_mut(&mut self) -> &mut Rev;
    fn is_deleted(&self) -> bool;
}

macro_rules! record {
    ($t:ty) => {
        impl Record for $t {
            fn id(&self) -> &str {
                &self.id
            }
            fn rev(&self) -> &Rev {
                &self.rev
            }
            fn rev_mut(&mut self) -> &mut Rev {
                &mut self.rev
            }
            fn is_deleted(&self) -> bool {
                self.deleted
            }
        }
    };
}

/// A client: reusable, one client has many projects. Separate from external sources (an Azure
/// DevOps organisation or project is never a client).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Client {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
    /// The legacy project texts this client was created from (migration record, for recovery).
    #[serde(default)]
    pub migrated_from: Vec<String>,
    #[serde(default)]
    pub rev: Rev,
    #[serde(default)]
    pub deleted: bool,
}
record!(Client);

/// Stable id for a client created from a legacy project text, so two devices migrating on
/// their own produce the same record instead of duplicates.
pub fn legacy_client_id(name: &str) -> Id {
    format!("legacy-client:{}", name.trim())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub color: String,
    /// Zero or one client.
    #[serde(default)]
    pub client_id: Option<Id>,
    /// Legacy text field (schema 1). Kept equal to the client's name so older DevTrack versions
    /// still show it and a later migration can map it back; never the source of truth.
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub billable_default: bool,
    /// Time planned for the whole project, in seconds.
    #[serde(default)]
    pub estimate_secs: Option<i64>,
    #[serde(default)]
    pub rev: Rev,
    #[serde(default)]
    pub deleted: bool,
}
record!(Project);

/// Where a task came from. Kept as an open string so new integrations need no schema change.
pub const SOURCE_MANUAL: &str = "manual";
pub const SOURCE_AZURE: &str = "azure_devops";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: Id,
    pub project_id: Option<Id>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
    #[serde(default = "manual")]
    pub source: String,
    /// Identifier in the source system (work item number for Azure DevOps).
    #[serde(default)]
    pub external_id: Option<String>,
    /// Project or container in the source system.
    #[serde(default)]
    pub external_project: Option<String>,
    #[serde(default)]
    pub external_type: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub integration_id: Option<Id>,
    /// Time planned for a local task, in seconds, set by the person. Imported tasks use the
    /// source's estimate instead (`source_estimate_secs`); this is ignored for them.
    #[serde(default)]
    pub estimate_secs: Option<i64>,
    /// The source's own estimate (Azure DevOps Original Estimate), read at import, refresh and
    /// when the work item is read again. Never edited in DevTrack.
    #[serde(default)]
    pub source_estimate_secs: Option<i64>,
    #[serde(default)]
    pub refreshed_at: Option<i64>,
    /// The source did not return this item at the last refresh (deleted, moved or no access).
    /// The task and its history stay; only this flag changes.
    #[serde(default)]
    pub unavailable: bool,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub rev: Rev,
    #[serde(default)]
    pub deleted: bool,
}
record!(Task);

fn manual() -> String {
    SOURCE_MANUAL.into()
}

impl Task {
    /// The hours planned for this task: the source's estimate for an imported task, the
    /// person's own for a local one.
    pub fn planned_secs(&self) -> Option<i64> {
        if self.source == SOURCE_MANUAL { self.estimate_secs } else { self.source_estimate_secs }
    }
    /// Fixed id for an imported item, so importing it on two devices never duplicates it.
    pub fn external_key(source: &str, org: &str, external_id: &str) -> Id {
        format!("{}:{}:{}", source, org.to_lowercase(), external_id)
    }
    pub fn source_label(&self) -> &str {
        match self.source.as_str() {
            SOURCE_AZURE => "Azure DevOps",
            SOURCE_MANUAL => "Manual",
            other => other,
        }
    }
    pub fn display(&self) -> String {
        match &self.external_id {
            Some(x) => format!("#{} {}", x, self.title),
            None => self.title.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TimeEntry {
    pub id: Id,
    #[serde(default)]
    pub description: String,
    pub project_id: Option<Id>,
    pub task_id: Option<Id>,
    /// UTC epoch seconds.
    pub start: i64,
    /// None while running.
    pub stop: Option<i64>,
    #[serde(default)]
    pub billable: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub rev: Rev,
    #[serde(default)]
    pub deleted: bool,
}
record!(TimeEntry);

impl TimeEntry {
    pub fn is_running(&self) -> bool {
        self.stop.is_none() && !self.deleted
    }
    /// Exact recorded duration in seconds; a running entry counts up to `now`.
    pub fn duration(&self, now: i64) -> i64 {
        (self.stop.unwrap_or(now) - self.start).max(0)
    }
}

/// How a duration is written. The variant names are stored in the data file, so they stay as
/// they are; what people read is `label`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum DurationFormat {
    /// 0:47:06
    #[default]
    Improved,
    /// 47:06 min
    Classic,
    /// 0.79 h
    Decimal,
}

impl DurationFormat {
    pub const ALL: [DurationFormat; 3] =
        [DurationFormat::Improved, DurationFormat::Classic, DurationFormat::Decimal];
    pub fn label(self) -> &'static str {
        match self {
            DurationFormat::Improved => "Hours, minutes and seconds (0:47:06)",
            DurationFormat::Classic => "Minutes and seconds (47:06 min)",
            DurationFormat::Decimal => "Decimal hours (0.79 h)",
        }
    }
}

/// Settings shared by all of the person's devices.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub duration_format: DurationFormat,
    pub clock_24h: bool,
    pub week_starts_monday: bool,
    pub dark: bool,
    /// Turn off interface transitions.
    #[serde(default)]
    pub reduce_motion: bool,
    /// Show tracked time rounded to the minute instead of to the second. Display only.
    #[serde(default)]
    pub round_to_minute: bool,
    /// Sessions that start on or before this day (YYYY-MM-DD) can't be changed or deleted.
    #[serde(default)]
    pub lock_until: Option<String>,
    /// Sessions that only had a description were given the task of that name, once (the task
    /// now says what a session was).
    #[serde(default)]
    pub descriptions_are_tasks: bool,
    /// Working days, Monday (bit 0) to Sunday (bit 6). Other days are left out of the calendar,
    /// the weekly grid and the per-day chart unless they hold time, and reminders skip them.
    #[serde(default = "default_work_days")]
    pub work_days: u8,
    /// Working hours, in minutes from midnight: reminders only fire inside them, and an empty
    /// calendar shows them.
    #[serde(default = "default_work_start")]
    pub work_start_min: u16,
    #[serde(default = "default_work_end")]
    pub work_end_min: u16,
    /// Follow the computer's light or dark appearance instead of `dark`.
    #[serde(default)]
    pub follow_system_theme: bool,
    /// Project given to a new session started without one.
    #[serde(default)]
    pub default_project: Option<Id>,
    /// Minutes a session snaps to when it is created, moved or stretched in the calendar.
    #[serde(default = "default_calendar_step")]
    pub calendar_step: u8,
    #[serde(default)]
    pub rev: Rev,
}

fn default_work_days() -> u8 {
    0b001_1111
}
fn default_work_start() -> u16 {
    9 * 60
}
fn default_work_end() -> u16 {
    18 * 60
}
fn default_calendar_step() -> u8 {
    5
}

impl Settings {
    pub fn is_work_day(&self, d: chrono::NaiveDate) -> bool {
        use chrono::Datelike;
        self.work_days & (1 << d.weekday().num_days_from_monday()) != 0
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            duration_format: DurationFormat::Improved,
            clock_24h: true,
            week_starts_monday: true,
            dark: true,
            reduce_motion: false,
            round_to_minute: false,
            lock_until: None,
            descriptions_are_tasks: false,
            work_days: default_work_days(),
            work_start_min: default_work_start(),
            work_end_min: default_work_end(),
            follow_system_theme: false,
            default_project: None,
            calendar_step: default_calendar_step(),
            rev: Rev::default(),
        }
    }
}

// ---------- timesheet profiles ----------

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportFormat {
    /// 1:30
    #[default]
    HoursMinutes,
    /// 1.50 (places and separator in the profile)
    DecimalHours,
    /// 90
    Minutes,
}

impl ReportFormat {
    pub const ALL: [ReportFormat; 3] =
        [ReportFormat::HoursMinutes, ReportFormat::DecimalHours, ReportFormat::Minutes];
    pub fn label(self) -> &'static str {
        match self {
            ReportFormat::HoursMinutes => "Hours and minutes (1:30)",
            ReportFormat::DecimalHours => "Decimal hours (1.50)",
            ReportFormat::Minutes => "Total minutes (90)",
        }
    }
}

/// Rounding directions: to the closest step, always up, or always down.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoundDir {
    #[default]
    Closest,
    Up,
    Down,
}

impl RoundDir {
    pub const ALL: [RoundDir; 3] = [RoundDir::Closest, RoundDir::Up, RoundDir::Down];
    pub fn label(self) -> &'static str {
        match self {
            RoundDir::Closest => "To closest",
            RoundDir::Up => "Up",
            RoundDir::Down => "Down",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoundScope {
    /// Never round.
    None,
    /// Round every entry, then add.
    PerEntry,
    /// Add first, round each reported line.
    #[default]
    PerGroup,
}

impl RoundScope {
    pub const ALL: [RoundScope; 3] = [RoundScope::None, RoundScope::PerEntry, RoundScope::PerGroup];
    pub fn label(self) -> &'static str {
        match self {
            RoundScope::None => "No rounding",
            RoundScope::PerEntry => "Each time entry",
            RoundScope::PerGroup => "Each reported line",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Grouping {
    /// One line per day and job code.
    #[default]
    DayCode,
    DayProject,
    DayTask,
    /// One line per week and job code.
    WeekCode,
}

impl Grouping {
    pub const ALL: [Grouping; 4] =
        [Grouping::DayCode, Grouping::DayProject, Grouping::DayTask, Grouping::WeekCode];
    pub fn label(self) -> &'static str {
        match self {
            Grouping::DayCode => "Day + job code",
            Grouping::DayProject => "Day + project",
            Grouping::DayTask => "Day + task",
            Grouping::WeekCode => "Week + job code",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CodeMapping {
    pub project_id: Option<Id>,
    /// When set, overrides the project's mapping for this task.
    pub task_id: Option<Id>,
    pub code: String,
    pub activity: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TimesheetProfile {
    pub id: Id,
    pub name: String,
    /// The destination system, for the person's reference (e.g. Zucchetti).
    pub destination: String,
    pub format: ReportFormat,
    pub decimal_places: u8,
    pub decimal_comma: bool,
    pub round_step_min: u32,
    pub round_dir: RoundDir,
    pub round_scope: RoundScope,
    pub grouping: Grouping,
    pub default_code: String,
    pub mappings: Vec<CodeMapping>,
    /// Copy line, with {date} {code} {activity} {project} {task} {value} {actual}.
    pub line_template: String,
    /// chrono date format for {date}.
    pub date_format: String,
    /// Decimal hours without trailing zeros: 2.5 rather than 2.50.
    #[serde(default)]
    pub trim_zeros: bool,
    #[serde(default)]
    pub rev: Rev,
    #[serde(default)]
    pub deleted: bool,
}
record!(TimesheetProfile);

impl TimesheetProfile {
    pub fn new(name: &str, destination: &str) -> Self {
        TimesheetProfile {
            id: new_id(),
            name: name.into(),
            destination: destination.into(),
            format: ReportFormat::HoursMinutes,
            decimal_places: 2,
            decimal_comma: true,
            // No company rounding until the person explicitly chooses one.
            round_step_min: 15,
            round_dir: RoundDir::Up,
            round_scope: RoundScope::None,
            grouping: Grouping::DayCode,
            default_code: String::new(),
            mappings: vec![],
            line_template: "{date}\t{code}\t{activity}\t{value}".into(),
            date_format: "%d/%m/%Y".into(),
            trim_zeros: false,
            rev: Rev::default(),
            deleted: false,
        }
    }

    /// A conversion as created from Settings: decimal hours with a point and no trailing zeros,
    /// rounded to the closest 15 minutes.
    pub fn conversion(name: &str) -> Self {
        let mut p = TimesheetProfile::new(name, "");
        p.format = ReportFormat::DecimalHours;
        p.decimal_comma = false;
        p.trim_zeros = true;
        p.round_scope = RoundScope::PerGroup;
        p.round_step_min = 15;
        p.round_dir = RoundDir::Closest;
        p
    }

    pub fn rounds(&self) -> bool {
        self.round_scope != RoundScope::None && self.round_step_min >= 1
    }

    /// Everything that changes a reported value; part of each confirmation snapshot.
    pub fn rule_signature(&self) -> String {
        format!(
            "fmt={:?};dp={};comma={};round={:?}/{}/{:?};group={:?};date={}",
            self.format,
            self.decimal_places,
            self.decimal_comma,
            self.round_scope,
            self.round_step_min,
            self.round_dir,
            self.grouping,
            self.date_format
        )
    }

    /// Rounding rule in words (separate from display precision).
    pub fn rule_text(&self) -> String {
        if !self.rounds() {
            "no rounding".into()
        } else {
            format!(
                "{} to {} min, {}",
                self.round_dir.label().to_lowercase(),
                self.round_step_min,
                self.round_scope.label().to_lowercase()
            )
        }
    }

    /// Display precision in words.
    pub fn precision_text(&self) -> String {
        match self.format {
            ReportFormat::HoursMinutes => "shown to the minute (h:mm)".into(),
            ReportFormat::Minutes => "shown in whole minutes".into(),
            ReportFormat::DecimalHours => format!(
                "shown with {} decimal{} (1 decimal ≈ 6 min, 2 ≈ 36 s)",
                self.decimal_places,
                if self.decimal_places == 1 { "" } else { "s" }
            ),
        }
    }
}

/// "Already reported" confirmation for one line of one profile, kept locally (and on the
/// person's other devices), never sent anywhere. It is a snapshot: when the line's value,
/// code, activity, rule or underlying sessions change, the line shows "Changed after reporting".
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ReportedMark {
    /// profile|date|group key
    pub id: Id,
    pub profile_id: Id,
    /// Old (0.1) marks only had whole minutes; kept so they can be shown, never trusted as current.
    #[serde(default)]
    pub reported_minutes: i64,
    /// The value exactly as confirmed (e.g. "1,50").
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub reported_secs: i64,
    #[serde(default)]
    pub actual_secs: i64,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub activity: String,
    /// Format + rounding + grouping in force when confirmed.
    #[serde(default)]
    pub rule: String,
    /// Fingerprint of the sessions behind the line (ids, times, project, task, zone offset).
    #[serde(default)]
    pub fingerprint: String,
    /// Date label of the line, for listing marks whose line no longer exists.
    #[serde(default)]
    pub date_label: String,
    pub marked_at: i64,
    #[serde(default)]
    pub rev: Rev,
    #[serde(default)]
    pub deleted: bool,
}
record!(ReportedMark);

/// One configured external system. Secrets live in the system keychain, keyed by `id`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct IntegrationConfig {
    pub id: Id,
    pub kind: String,
    pub name: String,
    pub settings: BTreeMap<String, String>,
    pub enabled: bool,
    #[serde(default)]
    pub last_refresh: Option<i64>,
    #[serde(default)]
    pub rev: Rev,
    #[serde(default)]
    pub deleted: bool,
}
record!(IntegrationConfig);

/// Everything that syncs between devices. Tombstones (`deleted`) are kept so deletions travel.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Db {
    /// Data layout version: 1 = client as project text, 2 = client records.
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub clients: BTreeMap<Id, Client>,
    #[serde(default)]
    pub projects: BTreeMap<Id, Project>,
    #[serde(default)]
    pub tasks: BTreeMap<Id, Task>,
    #[serde(default)]
    pub entries: BTreeMap<Id, TimeEntry>,
    #[serde(default)]
    pub profiles: BTreeMap<Id, TimesheetProfile>,
    #[serde(default)]
    pub marks: BTreeMap<Id, ReportedMark>,
    #[serde(default)]
    pub integrations: BTreeMap<Id, IntegrationConfig>,
    #[serde(default)]
    pub settings: Settings,
}

impl Db {
    pub fn running(&self) -> Option<&TimeEntry> {
        self.entries.values().filter(|e| e.is_running()).max_by_key(|e| e.start)
    }
    /// A client by id, including archived ones (history stays identifiable); not deleted ones.
    pub fn client(&self, id: &Option<Id>) -> Option<&Client> {
        id.as_ref().and_then(|i| self.clients.get(i)).filter(|c| !c.deleted)
    }
    pub fn live_clients(&self) -> Vec<&Client> {
        let mut v: Vec<&Client> = self.clients.values().filter(|c| !c.deleted).collect();
        v.sort_by_key(|c| (c.name.to_lowercase(), c.id.clone()));
        v
    }
    /// The client name shown for a project ("" for no client).
    pub fn client_name(&self, p: &Project) -> String {
        self.client(&p.client_id).map(|c| c.name.clone()).unwrap_or_default()
    }
    pub fn projects_of_client(&self, client: &Id) -> Vec<&Project> {
        self.live_projects().into_iter().filter(|p| p.client_id.as_ref() == Some(client)).collect()
    }
    /// Clients whose names differ only by case, spacing or punctuation: offered for review,
    /// never merged automatically.
    pub fn likely_duplicate_clients(&self) -> Vec<(Id, Id)> {
        let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        let list = self.live_clients();
        let mut out = vec![];
        for (i, a) in list.iter().enumerate() {
            for b in list.iter().skip(i + 1) {
                if a.name.trim() != b.name.trim() && !key(&a.name).is_empty() && key(&a.name) == key(&b.name) {
                    out.push((a.id.clone(), b.id.clone()));
                }
            }
        }
        out
    }
    pub fn project(&self, id: &Option<Id>) -> Option<&Project> {
        id.as_ref().and_then(|i| self.projects.get(i)).filter(|p| !p.deleted)
    }
    pub fn task(&self, id: &Option<Id>) -> Option<&Task> {
        id.as_ref().and_then(|i| self.tasks.get(i)).filter(|t| !t.deleted)
    }
    pub fn live_projects(&self) -> Vec<&Project> {
        let mut v: Vec<&Project> = self.projects.values().filter(|p| !p.deleted).collect();
        v.sort_by_key(|p| p.name.to_lowercase());
        v
    }
    pub fn live_tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values().filter(|t| !t.deleted)
    }
    pub fn live_entries(&self) -> impl Iterator<Item = &TimeEntry> {
        self.entries.values().filter(|e| !e.deleted)
    }
    pub fn live_profiles(&self) -> Vec<&TimesheetProfile> {
        let mut v: Vec<&TimesheetProfile> = self.profiles.values().filter(|p| !p.deleted).collect();
        v.sort_by_key(|p| p.name.to_lowercase());
        v
    }
    pub fn next_color(&self) -> String {
        let n = self.projects.values().filter(|p| !p.deleted).count();
        PROJECT_COLORS[n % PROJECT_COLORS.len()].to_string()
    }
    /// Total recorded seconds per task, over all time.
    pub fn task_total(&self, task_id: &str, now: i64) -> i64 {
        self.live_entries()
            .filter(|e| e.task_id.as_deref() == Some(task_id))
            .map(|e| e.duration(now))
            .sum()
    }
}
