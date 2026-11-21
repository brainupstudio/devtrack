//! Local storage: every change is saved to disk at once, so entries (and a running timer)
//! survive closing the app, and work offline. Each change gets a new revision for sync.
//!
//! Safety: a missing file means first launch; a file that can't be read or decoded is
//! never treated as an empty account. It is copied aside, saving is blocked, and the person picks
//! "restore the previous copy" or "start fresh". Every save keeps the previous file as
//! `data.previous.json`.

use crate::model::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "DevTrack";

/// A saved session to start again with one click.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Favorite {
    pub description: String,
    #[serde(default)]
    pub project_id: Option<Id>,
    #[serde(default)]
    pub task_id: Option<Id>,
    #[serde(default)]
    pub billable: bool,
}

fn default_idle_minutes() -> u32 {
    10
}

/// Settings that belong to this device only (never synced).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalConfig {
    pub device_id: String,
    pub device_name: String,
    #[serde(default)]
    pub sync_folder: Option<PathBuf>,
    #[serde(default)]
    pub last_sync_ms: i64,
    /// When this device last wrote a sync file that carried new local changes.
    #[serde(default)]
    pub last_change_written_ms: i64,
    /// Entry paused on this device; Resume continues it as a new entry.
    #[serde(default)]
    pub paused_entry: Option<Id>,
    /// Last Azure DevOps connection used in Activities.
    #[serde(default)]
    pub last_connection: Option<Id>,
    /// Compact window bounds (x, y, width, height in points), remembered separately.
    #[serde(default)]
    pub compact_rect: Option<[f32; 4]>,
    /// The sidebar is reduced to its icons on this device.
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// Interface language on this device: "system", "en" or "it" (not synced).
    #[serde(default)]
    pub language: String,
    /// Sessions to start with one click, in the order they were added.
    #[serde(default)]
    pub favorites: Vec<Favorite>,
    /// Ask what to do with the time when the computer was idle this long while a session ran.
    /// 0 turns it off.
    #[serde(default = "default_idle_minutes")]
    pub idle_minutes: u32,
    /// Remind to start tracking after this many minutes without a session, on working hours.
    /// 0 turns it off.
    #[serde(default)]
    pub remind_minutes: u32,
    /// What sits beside the menu bar icon (macOS): 0 the running session's time, 1 today's
    /// total, 2 nothing.
    #[serde(default)]
    pub tray_shows: u8,
    /// The compact window floats above other windows.
    #[serde(default)]
    pub compact_on_top: bool,
    /// Ask once a day, at start, whether a newer DevTrack is out.
    #[serde(default = "default_true")]
    pub update_check: bool,
    /// When the last update check answered (ms).
    #[serde(default)]
    pub update_checked_ms: i64,
    /// The latest published version, as of that check.
    #[serde(default)]
    pub update_latest: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for LocalConfig {
    fn default() -> Self {
        let host = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "This computer".into());
        LocalConfig {
            device_id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
            device_name: host,
            sync_folder: None,
            last_sync_ms: 0,
            last_change_written_ms: 0,
            paused_entry: None,
            last_connection: None,
            compact_rect: None,
            sidebar_collapsed: false,
            language: String::new(),
            favorites: vec![],
            idle_minutes: default_idle_minutes(),
            remind_minutes: 0,
            tray_shows: 0,
            compact_on_top: false,
            update_check: true,
            update_checked_ms: 0,
            update_latest: None,
        }
    }
}

pub fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpenStatus {
    /// Preview / screenshot: nothing is read or written.
    InMemory,
    FirstLaunch,
    Opened,
    /// The file exists but couldn't be read (permissions, disk). Nothing was changed.
    Unreadable(String),
    /// The file couldn't be decoded. A copy was kept aside.
    Damaged { error: String, kept_as: PathBuf, previous_ok: bool },
}

pub struct Store {
    pub db: Db,
    pub local: LocalConfig,
    /// None = never write to disk (screenshot / preview mode).
    dir: Option<PathBuf>,
    last_rev: Rev,
    /// Increases on every local change; sync uses it to know there is something to send.
    pub generation: u64,
    pub save_error: Option<String>,
    pub status: OpenStatus,
    /// Set while the data file is unreadable or damaged: saving is refused so nothing is overwritten.
    pub blocked: Option<String>,
    /// Last deleted or discarded entry, for Undo.
    pub last_removed: Option<TimeEntry>,
}

pub fn data_root() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_dir() -> PathBuf {
    data_root().join(APP_NAME)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

enum Read {
    Missing,
    Ok(Db),
    Unreadable(String),
    Damaged(String),
}

fn read_db(path: &Path) -> Read {
    match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Read::Missing,
        Err(e) => Read::Unreadable(e.to_string()),
        Ok(b) => match serde_json::from_slice::<Db>(&b) {
            Ok(db) => Read::Ok(db),
            Err(e) => Read::Damaged(e.to_string()),
        },
    }
}

impl Store {
    pub fn in_memory(db: Db) -> Store {
        Store {
            db,
            local: LocalConfig { device_name: "Preview".into(), ..LocalConfig::default() },
            dir: None,
            last_rev: Rev::default(),
            generation: 0,
            save_error: None,
            status: OpenStatus::InMemory,
            blocked: None,
            last_removed: None,
        }
    }

    /// Open DevTrack's folder.
    pub fn open_default() -> Store {
        Store::open(default_dir())
    }

    pub fn open(dir: PathBuf) -> Store {
        let local: LocalConfig = std::fs::read(dir.join("local.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut s = Store {
            db: Db::default(),
            local,
            dir: Some(dir.clone()),
            last_rev: Rev::default(),
            generation: 0,
            save_error: None,
            status: OpenStatus::Opened,
            blocked: None,
            last_removed: None,
        };
        match read_db(&dir.join("data.json")) {
            Read::Missing => s.status = OpenStatus::FirstLaunch,
            Read::Ok(db) => s.db = db,
            Read::Unreadable(e) => {
                s.blocked = Some(format!("The data file can't be read: {}", e));
                s.status = OpenStatus::Unreadable(e);
            }
            Read::Damaged(e) => {
                let kept = dir.join(format!("data.damaged-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S")));
                let _ = std::fs::copy(dir.join("data.json"), &kept);
                let previous_ok = matches!(read_db(&dir.join("data.previous.json")), Read::Ok(_));
                s.blocked = Some(format!("The data file is damaged: {}", e));
                s.status = OpenStatus::Damaged { error: e, kept_as: kept, previous_ok };
            }
        }
        // versioned migration (schema 2: client records); backed up first, never on a blocked file
        if s.blocked.is_none() && matches!(s.status, OpenStatus::Opened) {
            s.migrate_clients();
        }
        // local.json carries no work data; keep it current unless the folder itself is unreadable
        if !matches!(s.status, OpenStatus::Unreadable(_)) {
            s.save_local();
        }
        s
    }

    /// The copy kept before the last save (`data.previous.json`), for reviewing changes.
    pub fn read_previous(&self) -> Option<Db> {
        match read_db(&self.dir.as_ref()?.join("data.previous.json")) {
            Read::Ok(db) => Some(db),
            _ => None,
        }
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    pub fn is_persistent(&self) -> bool {
        self.dir.is_some()
    }

    /// Recovery: use `data.previous.json` (the copy before the last save).
    pub fn restore_previous(&mut self) -> Result<(), String> {
        let dir = self.dir.clone().ok_or("Preview mode")?;
        match read_db(&dir.join("data.previous.json")) {
            Read::Ok(db) => {
                self.db = db;
                self.blocked = None;
                self.status = OpenStatus::Opened;
                self.generation += 1;
                self.save();
                Ok(())
            }
            _ => Err("The previous copy can't be used either".into()),
        }
    }

    /// Recovery: start with empty data. The damaged file stays where it was copied.
    pub fn start_fresh(&mut self) {
        if let OpenStatus::Unreadable(_) = self.status {
            return; // the folder itself is the problem; nothing would be saved
        }
        self.db = Db::default();
        self.blocked = None;
        self.status = OpenStatus::FirstLaunch;
        self.generation += 1;
        self.save();
    }

    pub fn retry_open(&mut self) {
        if let Some(dir) = self.dir.clone() {
            *self = Store::open(dir);
        }
    }

    pub fn save(&mut self) {
        if let Some(reason) = &self.blocked {
            self.save_error = Some(format!("Not saved: {}", reason));
            return;
        }
        if let Some(dir) = &self.dir {
            let path = dir.join("data.json");
            if path.exists() {
                let _ = std::fs::copy(&path, dir.join("data.previous.json"));
            }
            let res = serde_json::to_vec_pretty(&self.db)
                .map_err(|e| e.to_string())
                .and_then(|b| write_atomic(&path, &b).map_err(|e| e.to_string()));
            self.save_error = res.err();
        }
    }

    pub fn save_local(&mut self) {
        if let Some(dir) = &self.dir {
            if let Ok(b) = serde_json::to_vec_pretty(&self.local) {
                let _ = write_atomic(&dir.join("local.json"), &b);
            }
        }
    }

    /// A new revision, strictly newer than any this device made before.
    pub fn tick(&mut self) -> Rev {
        let now = now_ms();
        let rev = if now > self.last_rev.at_ms {
            Rev { at_ms: now, seq: 0, device: self.local.device_id.clone() }
        } else {
            Rev { at_ms: self.last_rev.at_ms, seq: self.last_rev.seq + 1, device: self.local.device_id.clone() }
        };
        self.last_rev = rev.clone();
        rev
    }

    /// Make sure our next revision is newer than anything seen from other devices.
    pub fn observe(&mut self, rev: &Rev) {
        if rev > &self.last_rev {
            self.last_rev = Rev { at_ms: rev.at_ms, seq: rev.seq, device: self.local.device_id.clone() };
        }
    }

    fn changed(&mut self) {
        self.generation += 1;
        self.save();
    }

    // ---------- generic saves ----------

    pub fn put_client(&mut self, mut c: Client) {
        c.rev = self.tick();
        self.db.clients.insert(c.id.clone(), c);
        self.changed();
    }

    /// Rename a client once: every project shows the new name (their ids, sessions and codes stay).
    pub fn rename_client(&mut self, id: &Id, name: &str) {
        let Some(mut c) = self.db.clients.get(id).cloned() else { return };
        c.name = name.trim().to_string();
        let projects: Vec<Project> = self.db.projects.values().filter(|p| p.client_id.as_ref() == Some(id) && p.client != c.name).cloned().collect();
        let new_name = c.name.clone();
        self.put_client(c);
        for mut p in projects {
            p.client = new_name.clone(); // legacy mirror for older versions
            self.put_project(p);
        }
    }

    /// Delete a client after confirmation: its projects are kept and become "No client".
    pub fn delete_client_unlinking(&mut self, id: &Id) -> usize {
        let projects: Vec<Project> = self.db.projects.values().filter(|p| p.client_id.as_ref() == Some(id)).cloned().collect();
        let n = projects.len();
        for mut p in projects {
            p.client_id = None;
            p.client.clear();
            self.put_project(p);
        }
        if let Some(mut c) = self.db.clients.get(id).cloned() {
            c.deleted = true;
            self.put_client(c);
        }
        n
    }

    /// Schema 1 → 2: clients from the legacy project texts (repeatable, no duplicates).
    /// Exact names after trimming outer spaces share one client; names differing by case or
    /// punctuation stay separate (see `Db::likely_duplicate_clients`). Blank stays "No client".
    /// The first run backs up data.json as `data.before-clients.json`.
    pub fn migrate_clients(&mut self) -> usize {
        let pending: Vec<Project> = self.db.projects.values().filter(|p| p.client_id.is_none() && !p.client.trim().is_empty()).cloned().collect();
        if pending.is_empty() {
            if self.db.schema < 2 && self.blocked.is_none() {
                self.db.schema = 2;
            }
            return 0;
        }
        if self.blocked.is_some() {
            return 0;
        }
        if let Some(dir) = &self.dir {
            let backup = dir.join("data.before-clients.json");
            let src = dir.join("data.json");
            if src.exists() && !backup.exists() {
                let _ = std::fs::copy(&src, &backup);
            }
        }
        let mut n = 0;
        for mut p in pending {
            let name = p.client.trim().to_string();
            // an existing client with exactly this name (or created from it) wins
            let existing = self
                .db
                .clients
                .values()
                .find(|c| !c.deleted && (c.name.trim() == name || c.migrated_from.iter().any(|m| m.trim() == name)))
                .map(|c| c.id.clone());
            let id = match existing {
                Some(id) => id,
                None => {
                    let id = legacy_client_id(&name);
                    let c = match self.db.clients.get(&id) {
                        // a deleted legacy client comes back rather than being duplicated
                        Some(c) => Client { deleted: false, ..c.clone() },
                        None => Client { id: id.clone(), name: name.clone(), migrated_from: vec![p.client.clone()], ..Default::default() },
                    };
                    self.put_client(c);
                    id
                }
            };
            p.client_id = Some(id);
            p.client = name;
            self.put_project(p);
            n += 1;
        }
        self.db.schema = 2;
        self.changed();
        n
    }

    pub fn put_project(&mut self, mut p: Project) {
        p.rev = self.tick();
        self.db.projects.insert(p.id.clone(), p);
        self.changed();
    }
    pub fn put_task(&mut self, mut t: Task) {
        t.rev = self.tick();
        self.db.tasks.insert(t.id.clone(), t);
        self.changed();
    }
    pub fn put_entry(&mut self, mut e: TimeEntry) {
        e.rev = self.tick();
        self.db.entries.insert(e.id.clone(), e);
        self.changed();
    }
    pub fn put_profile(&mut self, mut p: TimesheetProfile) {
        p.rev = self.tick();
        self.db.profiles.insert(p.id.clone(), p);
        self.changed();
    }
    pub fn put_mark(&mut self, mut m: ReportedMark) {
        m.rev = self.tick();
        self.db.marks.insert(m.id.clone(), m);
        self.changed();
    }
    pub fn put_integration(&mut self, mut i: IntegrationConfig) {
        i.rev = self.tick();
        self.db.integrations.insert(i.id.clone(), i);
        self.changed();
    }
    pub fn put_settings(&mut self, mut s: Settings) {
        s.rev = self.tick();
        self.db.settings = s;
        self.changed();
    }

    pub fn delete_entry(&mut self, id: &str) {
        if let Some(mut e) = self.db.entries.get(id).cloned() {
            self.last_removed = Some(e.clone());
            e.deleted = true;
            self.put_entry(e);
        }
    }
    /// Bring back the last deleted or discarded entry.
    pub fn undo_remove(&mut self) -> bool {
        if let Some(mut e) = self.last_removed.take() {
            if e.stop.is_none() {
                // it was running: it keeps running only if nothing else runs now
                if self.db.running().is_some() {
                    e.stop = Some(now_secs().max(e.start));
                }
            }
            e.deleted = false;
            self.put_entry(e);
            true
        } else {
            false
        }
    }
    pub fn delete_project(&mut self, id: &str) {
        if let Some(mut p) = self.db.projects.get(id).cloned() {
            p.deleted = true;
            self.put_project(p);
        }
    }
    pub fn delete_task(&mut self, id: &str) {
        if let Some(mut t) = self.db.tasks.get(id).cloned() {
            t.deleted = true;
            self.put_task(t);
        }
    }

    /// Move a task to another project. Past sessions keep their project unless `move_history`.
    pub fn move_task(&mut self, task_id: &str, project: Option<Id>, move_history: bool) -> usize {
        let Some(mut t) = self.db.tasks.get(task_id).cloned() else { return 0 };
        t.project_id = project.clone();
        self.put_task(t);
        let mut n = 0;
        if move_history {
            let list: Vec<TimeEntry> = self.db.live_entries().filter(|e| e.task_id.as_deref() == Some(task_id)).cloned().collect();
            for mut e in list {
                e.project_id = project.clone();
                self.put_entry(e);
                n += 1;
            }
        }
        n
    }

    // ---------- timer ----------

    /// Start a timer. Only one runs at a time: a running one is stopped first.
    pub fn start(&mut self, description: &str, project_id: Option<Id>, task_id: Option<Id>, billable: bool) -> Id {
        let now = now_secs();
        self.stop_at(now);
        self.local.paused_entry = None;
        self.save_local();
        let e = TimeEntry {
            id: new_id(),
            description: description.trim().to_string(),
            project_id,
            task_id,
            start: now,
            stop: None,
            billable,
            tags: vec![],
            rev: Rev::default(),
            deleted: false,
        };
        let id = e.id.clone();
        self.put_entry(e);
        id
    }

    fn stop_at(&mut self, at: i64) -> Option<Id> {
        let running: Vec<TimeEntry> = self.db.entries.values().filter(|e| e.is_running()).cloned().collect();
        let mut last = None;
        for mut e in running {
            e.stop = Some(at.max(e.start));
            last = Some(e.id.clone());
            self.put_entry(e);
        }
        last
    }

    pub fn stop(&mut self) -> Option<Id> {
        let id = self.stop_at(now_secs());
        self.local.paused_entry = None;
        self.save_local();
        id
    }

    /// Pause = stop now and remember the entry, so Resume can continue it.
    /// Stops the running session at an earlier moment (the start of an idle stretch). A moment
    /// at or before its start discards it instead. Returns the stopped entry.
    pub fn stop_idle(&mut self, when: i64) -> Option<TimeEntry> {
        let running = self.db.running().cloned()?;
        if when <= running.start {
            self.discard_running();
            return None;
        }
        let mut e = running;
        e.stop = Some(when.min(now_secs()));
        self.put_entry(e.clone());
        Some(e)
    }

    /// Sets how much time a project has on a day, as a timesheet cell does. More time lengthens
    /// the day's last session on that project, or adds one (after the day's last session, from
    /// 9:00 at the earliest); less time shortens the last sessions, removing those used up.
    /// Running sessions are never touched. Returns false when nothing changed.
    pub fn set_day_total(&mut self, project: &Option<Id>, day: chrono::NaiveDate, secs: i64) -> bool {
        use crate::report::{local_date, local_time_of};
        let mut mine: Vec<TimeEntry> = self.db.live_entries().filter(|e| e.stop.is_some() && &e.project_id == project && local_date(e.start) == day).cloned().collect();
        mine.sort_by_key(|e| e.start);
        let current: i64 = mine.iter().map(|e| e.stop.unwrap_or(e.start) - e.start).sum();
        let secs = secs.max(0);
        if secs == current {
            return false;
        }
        if secs > current {
            match mine.last().cloned() {
                Some(mut last) => {
                    last.stop = last.stop.map(|s| s + (secs - current));
                    self.put_entry(last);
                }
                None => {
                    let day_end = self.db.live_entries().filter(|e| local_date(e.start) == day).filter_map(|e| e.stop).max().unwrap_or(0);
                    let start = day_end.max(local_time_of(day, 9 * 3600));
                    let billable = self.db.project(project).map(|p| p.billable_default).unwrap_or(false);
                    self.put_entry(TimeEntry { id: new_id(), project_id: project.clone(), start, stop: Some(start + secs), billable, ..Default::default() });
                }
            }
        } else {
            let mut cut = current - secs;
            for mut e in mine.into_iter().rev() {
                if cut <= 0 {
                    break;
                }
                let d = e.stop.unwrap_or(e.start) - e.start;
                if d > cut {
                    e.stop = e.stop.map(|s| s - cut);
                    self.put_entry(e);
                    cut = 0;
                } else {
                    cut -= d;
                    self.delete_entry(&e.id);
                }
            }
        }
        true
    }

    /// Divides a finished session in two at `at`; nothing happens outside its interval.
    pub fn split_entry(&mut self, id: &str, at: i64) -> bool {
        let Some(e) = self.db.entries.get(id).filter(|e| !e.deleted).cloned() else { return false };
        let Some(stop) = e.stop else { return false };
        if at <= e.start || at >= stop {
            return false;
        }
        let mut first = e.clone();
        first.stop = Some(at);
        let mut second = e;
        second.id = new_id();
        second.start = at;
        self.put_entry(first);
        self.put_entry(second);
        true
    }

    pub fn pause(&mut self) {
        let id = self.stop_at(now_secs());
        self.local.paused_entry = id;
        self.save_local();
    }

    pub fn paused(&self) -> Option<&TimeEntry> {
        if self.db.running().is_some() {
            return None;
        }
        self.local.paused_entry.as_ref().and_then(|i| self.db.entries.get(i)).filter(|e| !e.deleted)
    }

    pub fn resume(&mut self) {
        if let Some(e) = self.paused().cloned() {
            self.start(&e.description, e.project_id.clone(), e.task_id.clone(), e.billable);
        }
    }

    /// Start again with the same details as an earlier entry.
    pub fn continue_entry(&mut self, id: &str) {
        if let Some(e) = self.db.entries.get(id).cloned() {
            self.start(&e.description, e.project_id, e.task_id, e.billable);
        }
    }

    pub fn discard_running(&mut self) {
        if let Some(e) = self.db.running().cloned() {
            self.delete_entry(&e.id);
        }
    }

    /// Change what the running timer is about (description, project, task, billable, start).
    pub fn update_running<F: FnOnce(&mut TimeEntry)>(&mut self, f: F) {
        if let Some(mut e) = self.db.running().cloned() {
            f(&mut e);
            self.put_entry(e);
        }
    }

    /// Add past time. Never touches the running entry.
    pub fn add_manual(&mut self, e: TimeEntry) -> Result<(), String> {
        validate(&e)?;
        if e.stop.is_none() {
            return Err("Past time needs an end".into());
        }
        self.put_entry(e);
        Ok(())
    }

    /// Add sample content (asked for by the person).
    pub fn load_sample(&mut self) {
        let sample = crate::sample::build(now_secs());
        for c in sample.clients.into_values() {
            self.put_client(c);
        }
        for p in sample.projects.into_values() {
            self.put_project(p);
        }
        for t in sample.tasks.into_values() {
            self.put_task(t);
        }
        for e in sample.entries.into_values() {
            self.put_entry(e);
        }
        for p in sample.profiles.into_values() {
            self.put_profile(p);
        }
    }

    /// Recent distinct activities (description, project, task), newest first.
    pub fn recent(&self, n: usize) -> Vec<TimeEntry> {
        let mut list: Vec<&TimeEntry> = self.db.live_entries().filter(|e| !e.is_running()).collect();
        list.sort_by_key(|e| std::cmp::Reverse(e.start));
        let mut seen = std::collections::BTreeSet::new();
        let mut out = vec![];
        for e in list {
            if e.description.is_empty() && e.project_id.is_none() && e.task_id.is_none() {
                continue;
            }
            let k = (e.description.clone(), e.project_id.clone(), e.task_id.clone());
            if seen.insert(k) {
                out.push(e.clone());
                if out.len() >= n {
                    break;
                }
            }
        }
        out
    }
}

/// A session's rules: stop must not be before start; start cannot be in the future.
pub fn validate(e: &TimeEntry) -> Result<(), String> {
    if e.start > now_secs() + 60 {
        return Err("Start time cannot be in the future".into());
    }
    if let Some(stop) = e.stop {
        if stop < e.start {
            return Err("End time must be after start time".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("devtrack-test-{}-{}", name, uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn damaged_file_is_never_overwritten() {
        let d = tmp("damaged");
        std::fs::write(d.join("data.json"), b"{ this is not json").unwrap();
        let mut s = Store::open(d.clone());
        assert!(matches!(s.status, OpenStatus::Damaged { .. }));
        s.start("x", None, None, false);
        assert_eq!(std::fs::read(d.join("data.json")).unwrap(), b"{ this is not json");
        assert!(s.save_error.is_some());
        s.start_fresh();
        assert!(s.blocked.is_none());
        let kept: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("data.damaged-")).collect();
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn previous_copy_restores() {
        let d = tmp("prev");
        let mut s = Store::open(d.clone());
        assert_eq!(s.status, OpenStatus::FirstLaunch);
        s.start("first", None, None, false);
        s.stop();
        std::fs::write(d.join("data.json"), b"garbage").unwrap();
        let mut s2 = Store::open(d.clone());
        assert!(matches!(s2.status, OpenStatus::Damaged { previous_ok: true, .. }));
        s2.restore_previous().unwrap();
        assert!(s2.db.live_entries().any(|e| e.description == "first"));
    }

    #[test]
    fn manual_add_leaves_running_entry() {
        let mut s = Store::in_memory(Db::default());
        let a = s.start("A", Some("pa".into()), Some("ta".into()), true);
        let now = now_secs();
        let b = TimeEntry { id: new_id(), description: "B".into(), project_id: Some("pb".into()), task_id: Some("tb".into()), start: now - 7200, stop: Some(now - 3600), ..Default::default() };
        s.add_manual(b).unwrap();
        let r = s.db.running().unwrap();
        assert_eq!((r.id.as_str(), r.description.as_str(), r.project_id.as_deref(), r.task_id.as_deref()), (a.as_str(), "A", Some("pa"), Some("ta")));
        // discard + undo
        s.discard_running();
        assert!(s.db.running().is_none());
        assert!(s.undo_remove());
        assert_eq!(s.db.running().unwrap().id, a);
    }

    #[test]
    fn clients_migrate_once_from_project_texts() {
        let d = tmp("clients");
        let mut db = Db::default();
        for (id, client) in [("p1", "Northwind"), ("p2", "  Northwind "), ("p3", "northwind"), ("p4", ""), ("p5", "Fabrikam")] {
            db.projects.insert(id.into(), Project { id: id.into(), name: id.into(), client: client.into(), ..Default::default() });
        }
        db.entries.insert("e1".into(), TimeEntry { id: "e1".into(), project_id: Some("p1".into()), start: 1000, stop: Some(1027), ..Default::default() });
        std::fs::write(d.join("data.json"), serde_json::to_vec(&db).unwrap()).unwrap();
        let s = Store::open(d.clone());
        assert!(d.join("data.before-clients.json").exists(), "backup before migrating");
        assert_eq!(s.db.schema, 2);
        let names: Vec<String> = s.db.live_clients().iter().map(|c| c.name.clone()).collect();
        assert_eq!(names, vec!["Fabrikam", "Northwind", "northwind"]); // case differences are not merged
        assert_eq!(s.db.projects["p1"].client_id, s.db.projects["p2"].client_id);
        assert_ne!(s.db.projects["p1"].client_id, s.db.projects["p3"].client_id);
        assert_eq!(s.db.projects["p4"].client_id, None);
        assert_eq!(s.db.likely_duplicate_clients().len(), 1);
        // recorded time untouched
        assert_eq!((s.db.entries["e1"].start, s.db.entries["e1"].stop), (1000, Some(1027)));
        // repeatable: opening again creates nothing new
        let mut s2 = Store::open(d.clone());
        assert_eq!(s2.db.live_clients().len(), 3);
        assert_eq!(s2.migrate_clients(), 0);
    }

    #[test]
    fn clients_are_shared_renamed_and_unlinked_safely() {
        let mut s = Store::in_memory(Db::default());
        let c = Client { id: new_id(), name: "Northwind".into(), ..Default::default() };
        s.put_client(c.clone());
        for id in ["a", "b"] {
            s.put_project(Project { id: id.into(), name: id.into(), client_id: Some(c.id.clone()), client: "Northwind".into(), ..Default::default() });
        }
        s.put_entry(TimeEntry { id: "e".into(), project_id: Some("a".into()), start: 100, stop: Some(200), ..Default::default() });
        s.rename_client(&c.id, "Northwind Traders");
        assert_eq!(s.db.client_name(&s.db.projects["a"]), "Northwind Traders");
        assert_eq!(s.db.client_name(&s.db.projects["b"]), "Northwind Traders");
        assert_eq!(s.db.projects["a"].client_id, Some(c.id.clone()), "ids never change");
        // reassign one project without recreating it
        let mut b = s.db.projects["b"].clone();
        b.client_id = None;
        s.put_project(b);
        assert_eq!(s.db.projects_of_client(&c.id).len(), 1);
        // deleting unlinks, never deletes projects or sessions
        assert_eq!(s.delete_client_unlinking(&c.id), 1);
        assert!(s.db.projects.values().all(|p| !p.deleted && p.client_id.is_none()));
        assert_eq!(s.db.entries["e"].stop, Some(200));
        assert!(s.db.client(&Some(c.id.clone())).is_none());
    }

    #[test]
    fn a_timesheet_cell_adds_lengthens_and_shortens() {
        use crate::report::{local_time_of, today};
        let mut s = Store::in_memory(Db::default());
        let day = today() - chrono::Duration::days(3);
        let p = Some("p".to_string());
        let total = |s: &Store| -> i64 { s.db.live_entries().filter(|e| e.project_id == p).map(|e| e.stop.unwrap() - e.start).sum() };
        assert!(s.set_day_total(&p, day, 3600), "an empty cell adds a session");
        assert_eq!(s.db.live_entries().next().unwrap().start, local_time_of(day, 9 * 3600));
        assert!(s.set_day_total(&p, day, 5400), "more time lengthens it");
        assert_eq!((s.db.live_entries().count(), total(&s)), (1, 5400));
        // a second session on another project the same day starts after the first
        let q = Some("q".to_string());
        s.set_day_total(&q, day, 1800);
        assert_eq!(s.db.live_entries().filter(|e| e.project_id == q).next().unwrap().start, local_time_of(day, 9 * 3600) + 5400);
        assert!(s.set_day_total(&p, day, 600), "less time shortens it");
        assert_eq!(total(&s), 600);
        assert!(s.set_day_total(&p, day, 0), "zero removes it");
        assert_eq!(total(&s), 0);
        assert!(!s.set_day_total(&p, day, 0), "the same value changes nothing");
    }

    #[test]
    fn split_and_stop_at_keep_the_total_time() {
        let mut s = Store::in_memory(Db::default());
        let t0 = now_secs() - 7200;
        s.put_entry(TimeEntry { id: "a".into(), description: "Work".into(), start: t0, stop: Some(t0 + 3600), ..Default::default() });
        assert!(!s.split_entry("a", t0), "the start is not a split point");
        assert!(s.split_entry("a", t0 + 1200));
        let mut parts: Vec<_> = s.db.live_entries().cloned().collect();
        parts.sort_by_key(|e| e.start);
        assert_eq!(parts.len(), 2);
        assert_eq!((parts[0].start, parts[0].stop), (t0, Some(t0 + 1200)));
        assert_eq!((parts[1].start, parts[1].stop), (t0 + 1200, Some(t0 + 3600)));
        assert_eq!(parts[1].description, "Work");
        // a running session stopped where the idle time began
        let id = s.start("Idle", None, None, false);
        let start = s.db.entries[&id].start;
        let stopped = s.stop_idle(start);
        assert!(stopped.is_none() && s.db.running().is_none(), "idle from the very start discards it");
    }
}
