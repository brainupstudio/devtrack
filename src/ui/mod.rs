//! Desktop shell: sidebar, top bar, shared navigation with the 200 ms content fade, compact
//! window mode, notices, keyboard shortcuts, sync scheduling, background jobs and the
//! review-picture options.

mod activities;
mod assigned;
mod assistants;
mod by_task;
mod compact;
mod dest;
mod gallery;
pub mod i18n;
mod i18n_it;
mod jobs;
mod monitors;
mod period;
mod report_time;
mod settings;
mod stats;
mod today;
pub mod widgets;

use devtrack_core::integrations::{org_of, ErrKind};
use devtrack_core::model::*;
use devtrack_core::report::{today as today_date, Period, PeriodKind};
use devtrack_core::store::{now_secs, OpenStatus, Store};
use devtrack_core::sync::{self, SyncOutcome};
use devtrack_core::timefmt::clock;
use eframe::egui::{self, Key, Modifiers, RichText, Sense, Stroke};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use i18n::trf_m;
use i18n::tr;
use widgets::*;

pub const SIDEBAR_W: f32 = 216.0;
pub const TOPBAR_H: f32 = 56.0;
/// macOS draws no title bar: the top strip of the sidebar is kept free for the three window
/// buttons, which the system paints over it. Zero where the system keeps its own title bar.
pub const TITLE_STRIP: f32 = if cfg!(target_os = "macos") { 32.0 } else { 0.0 };
/// Room the three window buttons take from the left edge of the window (macOS).
pub const LIGHTS_W: f32 = if cfg!(target_os = "macos") { 76.0 } else { 0.0 };
/// Width of the sidebar reduced to its icons: wide enough for the window buttons on macOS.
pub const RAIL_W: f32 = if cfg!(target_os = "macos") { 76.0 } else { 64.0 };

/// Makes `rect` move the window when dragged and zoom it on a double click, like a title bar.
/// Call it before the controls drawn in the same area, so they stay on top of it.
/// The compact window only moves: zooming a small floating window makes no sense.
pub fn window_drag(ui: &egui::Ui, rect: egui::Rect, id: &str) {
    if TITLE_STRIP == 0.0 {
        return;
    }
    let resp = ui.interact(rect, egui::Id::new(("window-drag", id)), Sense::click_and_drag());
    if resp.drag_started_by(egui::PointerButton::Primary) {
        // the system moves the window from here and keeps the button's release: forget any
        // click before, so the next press can't pair with it into a double click
        ui.data_mut(|d| d.remove::<(f64, egui::Pos2)>(egui::Id::new("window-drag-click")));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        return;
    }
    if id == "compact" || !resp.clicked() {
        return;
    }
    // a double click is two real clicks (press and release, no move) close in time and place;
    // egui's own counts presses, and a press that started a drag is one of them
    let now = ui.input(|i| i.time);
    let at = resp.interact_pointer_pos().unwrap_or_default();
    let key = egui::Id::new("window-drag-click");
    let last: Option<(f64, egui::Pos2)> = ui.data(|d| d.get_temp(key));
    if is_double_click(last, now, at) {
        ui.data_mut(|d| d.remove::<(f64, egui::Pos2)>(key));
        let max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
    } else {
        ui.data_mut(|d| d.insert_temp(key, (now, at)));
    }
}

/// A click at `at` and `now` completes a double click with the `last` one.
fn is_double_click(last: Option<(f64, egui::Pos2)>, now: f64, at: egui::Pos2) -> bool {
    last.is_some_and(|(t, p)| now - t <= 0.4 && p.distance(at) <= 4.0)
}
pub const FADE_MS: f32 = 200.0;
pub const COMPACT_SIZE: [f32; 2] = [430.0, 720.0];
pub const COMPACT_MIN: [f32; 2] = [380.0, 480.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Today,
    Activities,
    ReportTime,
    Statistics,
    Settings,
    Gallery,
}

impl Page {
    fn name(self) -> &'static str {
        match self {
            Page::Today => "Today",
            Page::Activities => "Work",
            Page::ReportTime => "Report Time",
            Page::Statistics => "Statistics",
            Page::Settings => "Settings",
            Page::Gallery => "Component gallery",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Page::Today => Icon::Today,
            Page::Activities => Icon::Activities,
            Page::ReportTime => Icon::Report,
            Page::Statistics => Icon::Stats,
            Page::Settings | Page::Gallery => Icon::Settings,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinMode {
    Full,
    Compact,
}

pub struct Screenshot {
    pub path: std::path::PathBuf,
    pub frames: u32,
    pub requested: bool,
}

pub struct Toast {
    pub msg: String,
    pub at: Instant,
    pub undo: bool,
}

#[derive(Default)]
pub struct SyncState {
    pub result: Option<Result<SyncOutcome, String>>,
    pub events: Vec<(i64, String)>,
    pub seen: BTreeMap<String, i64>,
    pub written_gen: u64,
}

/// Content fade between screens: from `from` opacity to 1 over 200 ms, ease-out cubic.
#[derive(Clone, Copy)]
pub struct Fade {
    pub start: Instant,
    pub from: f32,
}

pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl Fade {
    pub fn opacity(&self, now: Instant) -> f32 {
        let t = now.duration_since(self.start).as_secs_f32() * 1000.0 / FADE_MS;
        self.from + (1.0 - self.from) * ease_out(t)
    }
    pub fn done(&self, now: Instant) -> bool {
        now.duration_since(self.start).as_secs_f32() * 1000.0 >= FADE_MS
    }
}

/// Where to go back to after setting up a connection from Activities.
#[derive(Clone)]
pub struct ReturnTo {
    pub project: Option<Id>,
}

/// Measured control rectangles, for the geometry check (gallery and `--geometry-report`).
#[derive(Default, Clone)]
pub struct Geometry {
    pub rows: Vec<(String, egui::Rect)>,
}

/// Records each frame of a demonstration transition (`--transition-log`).
pub struct TransitionLog {
    pub path: std::path::PathBuf,
    pub frames: Vec<(f32, f32)>,
    pub started: Option<Instant>,
    pub written: bool,
}

/// The update check: running in the background, or how the last one went.
#[derive(Default)]
pub struct UpdateState {
    rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    /// The last check failed, with the reason (shown only after "Check now").
    pub error: Option<String>,
    /// "Check now" was pressed: its answer is shown, even when there is nothing new.
    pub asked: bool,
    /// Downloading and preparing this version (macOS, an app that can replace itself).
    download: Option<(String, std::sync::mpsc::Receiver<Result<std::path::PathBuf, String>>)>,
    /// This version is prepared and installs when DevTrack ends.
    pub ready: Option<String>,
    /// Why the last download or preparation failed: the download page is offered instead.
    pub install_error: Option<String>,
    /// The check at start has been made (or skipped): later ones follow `next_check`.
    started: bool,
    /// When to ask again while DevTrack stays open.
    next_check: Option<Instant>,
}

impl UpdateState {
    pub fn checking(&self) -> bool {
        self.rx.is_some()
    }
    /// The version being downloaded and prepared.
    pub fn downloading(&self) -> Option<&str> {
        self.download.as_ref().map(|(v, _)| v.as_str())
    }
}

pub struct App {
    pub store: Store,
    pub update: UpdateState,
    pub page: Page,
    pub today: today::TodayState,
    pub act: activities::ActState,
    pub rep: report_time::RepState,
    pub dest: Option<dest::DestDraft>,
    pub stats: stats::StatsState,
    pub settings: settings::SettingsState,
    /// Settings › AI assistants: the check in flight, the assistant picked, what was copied.
    pub assistants: assistants::AssistantsState,
    pub compact_ui: compact::CompactState,
    pub jobs: jobs::Jobs,
    /// Work items assigned to you in Azure DevOps, ready for the task field.
    pub assigned: assigned::Assigned,
    /// When the list was last asked for, so a failing connection is not asked every frame.
    pub assigned_tried: Option<i64>,
    /// Review pictures: open the period picker once.
    pub preview_period_picker: bool,
    pub conn_msgs: BTreeMap<Id, (bool, String)>,
    pub sync: SyncState,
    next_sync: Instant,
    synced_gen: u64,
    gen_seen: u64,
    gen_changed_at: Instant,
    pub toast: Option<Toast>,
    pub shot: Option<Screenshot>,
    theme: Option<bool>,
    /// The computer's appearance, read every frame (None when the system doesn't say).
    system_dark: Option<bool>,
    /// The window level last asked for the compact window (floating or not).
    on_top: Option<bool>,
    pub return_focus: Option<egui::Id>,
    /// Very narrow full window: icon rail (not the explicit compact window).
    pub narrow: bool,
    /// The sidebar shows icons only: the window is very narrow, or the person collapsed it.
    pub rail: bool,
    /// "Demo" or "Preview" when the window shows sample data, said next to the logo.
    pub data_note: Option<&'static str>,
    /// The menu bar item is installed (macOS): closing the window hides DevTrack instead of
    /// quitting, and the item's commands are carried out here.
    pub tray: bool,
    /// Hide the window on the first frame (opened at login).
    pub start_hidden: bool,
    pub fade: Option<Fade>,
    pub mode: WinMode,
    full_outer: Option<egui::Rect>,
    full_inner: Option<egui::Vec2>,
    full_page: Page,
    pub return_to: Option<ReturnTo>,
    pub geometry: Geometry,
    pub geometry_report: Option<std::path::PathBuf>,
    pub transition_log: Option<TransitionLog>,
    /// Sidebar destination waiting for "Leave without saving?".
    pub pending_nav: Option<Page>,
    /// Seconds since the last keyboard or mouse input on this computer, read in the background.
    idle_secs: std::sync::Arc<std::sync::atomic::AtomicI64>,
    /// When the current idle stretch began, once it passed the threshold.
    idle_since: Option<i64>,
    /// The idle stretch to ask about, once the person is back.
    pub idle_prompt: Option<i64>,
    /// When the last "you're not tracking" reminder was shown.
    last_reminder: i64,
    /// Tasks whose estimate was already announced as passed, so it is said once.
    estimate_alerted: std::collections::BTreeSet<Id>,
    frame: u64,
}

/// Seconds without keyboard or mouse input, system wide. macOS only for now; elsewhere 0, which
/// keeps idle detection quiet instead of guessing.
fn system_idle_secs() -> i64 {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("ioreg").args(["-c", "IOHIDSystem", "-d", "4"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = text.lines().find(|l| l.contains("\"HIDIdleTime\"")) {
                if let Some(n) = line.rsplit('=').next().and_then(|v| v.trim().parse::<i64>().ok()) {
                    return n / 1_000_000_000;
                }
            }
        }
    }
    0
}

/// A system notification for a reminder. macOS only for now; the in-app message always shows.
fn system_notify(title: &str, text: &str) {
    #[cfg(target_os = "macos")]
    {
        let clean = |s: &str| s.replace(['"', '\\'], "");
        let script = format!("display notification \"{}\" with title \"{}\"", clean(text), clean(title));
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (title, text);
}

/// A duration in every enabled conversion, in the order the conversions are listed:
/// "DevOps 0.75 · Zucchetti 0:45". Empty when there are no conversions.
pub fn conversions_text(db: &Db, secs: i64) -> String {
    db.live_profiles().iter().map(|p| format!("{} {}", widgets::truncate(&p.name, 14), devtrack_core::report::convert(p, secs))).collect::<Vec<_>>().join(" · ")
}

/// What a session is called wherever it is listed: its task, else its note, else "Untitled".
pub fn session_title(db: &Db, e: &TimeEntry) -> String {
    if let Some(t) = db.task(&e.task_id) {
        return t.display();
    }
    if !e.description.trim().is_empty() {
        return e.description.trim().to_string();
    }
    i18n::tr("Untitled session")
}

/// Under a session's title: its project and client, and its note when the title is the task.
pub fn session_context(db: &Db, e: &TimeEntry) -> String {
    let mut parts = vec![];
    if let Some(pr) = db.project(&e.project_id) {
        parts.push(pr.name.clone());
        let cl = db.client_name(pr);
        if !cl.is_empty() {
            parts.push(cl);
        }
    }
    // the note, unless it only repeats the task's name
    if let Some(t) = db.task(&e.task_id) {
        let note = e.description.trim();
        if !note.is_empty() && !note.eq_ignore_ascii_case(t.title.trim()) {
            parts.push(note.to_string());
        }
    }
    parts.join(" · ")
}

/// A tracked duration as the person chose to read it: to the second, or to the minute.
pub fn shown_duration(db: &Db, secs: i64) -> String {
    if db.settings.round_to_minute {
        let m = (secs + 30) / 60;
        format!("{}:{:02}", m / 60, m % 60)
    } else {
        devtrack_core::timefmt::format_duration(secs, db.settings.duration_format)
    }
}

impl App {
    pub fn new(store: Store, shot: Option<Screenshot>) -> App {
        let monday = store.db.settings.week_starts_monday;
        let t = today_date();
        let mut act = activities::ActState::default();
        act.finder.cfg = store.local.last_connection.clone();
        App {
            update: UpdateState::default(),
            today: today::TodayState::new(Period::containing(PeriodKind::Day, t, monday)),
            act,
            rep: report_time::RepState::new(Period::containing(PeriodKind::Day, t, monday)),
            dest: None,
            stats: stats::StatsState::new(Period::containing(PeriodKind::Week, t, monday)),
            settings: settings::SettingsState::from(&store),
            assistants: assistants::AssistantsState::default(),
            compact_ui: compact::CompactState::default(),
            jobs: jobs::Jobs::default(),
            assigned: assigned::Assigned::load(&store),
            assigned_tried: None,
            preview_period_picker: false,
            conn_msgs: BTreeMap::new(),
            sync: SyncState::default(),
            page: Page::Today,
            next_sync: Instant::now() + Duration::from_millis(300),
            synced_gen: u64::MAX,
            gen_seen: 0,
            gen_changed_at: Instant::now(),
            toast: None,
            shot,
            theme: None,
            system_dark: None,
            on_top: None,
            return_focus: None,
            narrow: false,
            rail: false,
            data_note: None,
            tray: false,
            start_hidden: false,
            fade: None,
            mode: WinMode::Full,
            full_outer: None,
            full_inner: None,
            full_page: Page::Today,
            return_to: None,
            geometry: Geometry::default(),
            geometry_report: None,
            transition_log: None,
            pending_nav: None,
            idle_secs: {
                let v = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
                // previews and review pictures never watch the computer
                if store.is_persistent() {
                    let w = v.clone();
                    std::thread::spawn(move || loop {
                        w.store(system_idle_secs(), std::sync::atomic::Ordering::Relaxed);
                        std::thread::sleep(Duration::from_secs(10));
                    });
                }
                v
            },
            idle_since: None,
            idle_prompt: None,
            last_reminder: now_secs(),
            estimate_alerted: Default::default(),
            frame: 0,
            store,
        }
    }

    // ---------- navigation (the only way pages change) ----------

    fn current_opacity(&self) -> f32 {
        self.fade.map(|f| f.opacity(Instant::now())).unwrap_or(1.0)
    }

    /// Fade the content in again without changing page (Settings sections use it).
    pub fn fade_content(&mut self) {
        if self.store.db.settings.reduce_motion {
            self.fade = None;
            return;
        }
        // continue from the current visual opacity instead of replaying a full fade
        let from = if self.fade.is_some() { self.current_opacity().min(0.999) } else { 0.0 };
        self.fade = Some(Fade { start: Instant::now(), from });
    }

    /// Unsaved input that leaving a section would lose.
    pub fn unsaved(&self) -> bool {
        self.today.edit.as_ref().is_some_and(|e| e.dirty())
            || (self.today.past_open && self.today.past.has_input())
            || self.dest.as_ref().is_some_and(|d| d.dirty())
            || self.act.form.as_ref().is_some_and(|f| f.dirty())
            || match &self.act.client_edit {
                Some(activities::ClientEdit::New(s)) => !s.trim().is_empty(),
                Some(activities::ClientEdit::Rename(id, s)) => self.store.db.clients.get(id).map(|c| c.name != *s).unwrap_or(false),
                _ => false,
            }
            || self.settings.has_input()
    }

    /// Sidebar and ⌘1–4: open the section at its top level. Unsaved input asks first.
    pub fn nav_to(&mut self, page: Page) {
        if self.unsaved() {
            self.pending_nav = Some(page);
            return;
        }
        self.leave_to(page);
    }

    /// Close every sub-level, dialog and filter, then show the section's top level.
    fn leave_to(&mut self, page: Page) {
        let monday = self.store.db.settings.week_starts_monday;
        let t = today_date();
        let day = Period::containing(PeriodKind::Day, t, monday);
        let td = &mut self.today;
        td.edit = None;
        td.past_open = false;
        td.past = today::Past::default();
        td.only_incomplete = false;
        td.highlight = None;
        td.run_time = None;
        td.period = day.clone();
        let finder = std::mem::take(&mut self.act.finder);
        self.act = activities::ActState { finder, ..Default::default() };
        self.rep.selected_row = None;
        self.rep.confirm_delete = false;
        self.rep.show_sessions = false;
        self.rep.period = day;
        self.dest = None;
        self.stats = stats::StatsState::new(Period::containing(PeriodKind::Week, t, monday));
        self.settings = settings::SettingsState::from(&self.store);
        self.pending_nav = None;
        if page != self.page {
            self.page = page;
        }
        self.fade_content();
    }

    /// "Leave without saving?" for a sidebar destination.
    fn leave_modal(&mut self, ctx: &egui::Context) {
        let Some(page) = self.pending_nav else { return };
        let dark = self.dark();
        let c = pal(dark);
        let mut stay = false;
        let mut leave = false;
        egui::Modal::new(egui::Id::new("leave-unsaved")).frame(today::modal_frame(&c)).backdrop_color(egui::Color32::from_black_alpha(if dark { 160 } else { 90 })).show(ctx, |ui| {
            ui.set_width(380.0);
            if modal_close(ui) { stay = true; }
            ui.label(RichText::new(tr("Leave without saving?")).font(semibold(20.0)));
            ui.add_space(ROW_GAP);
            ui.add(egui::Label::new(RichText::new(trf_m!("The changes you made will be lost if you open {}.", tr(page.name()))).size(14.0).color(c.text2)).wrap());
            ui.add_space(GROUP_GAP);
            row(ui, CTRL_H, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if danger(ui, "Leave without saving").clicked() { leave = true; }
                    if secondary(ui, "Keep editing").clicked() { stay = true; }
                });
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { stay = true; }
        });
        if leave { self.leave_to(page); }
        if stay { self.pending_nav = None; }
    }

    /// Change page: the selected navigation item updates at once, the new content fades in.
    pub fn go(&mut self, page: Page) {
        if page == self.page {
            return;
        }
        self.page = page;
        self.fade_content();
    }

    pub fn open_page(&mut self, name: &str) {
        let page = match name.to_lowercase().as_str() {
            "activities" => Page::Activities,
            "projects" => {
                self.act.open_root(activities::ActTab::Projects);
                Page::Activities
            }
            // the three create dialogs, side by side in review: one frame for all of them
            "task-new" => {
                let project = self.store.db.live_projects().first().map(|p| p.id.clone());
                self.act.open_root(activities::ActTab::Tasks);
                self.act.open_add_choose(false, project);
                Page::Activities
            }
            "project-new" => {
                self.act.open_root(activities::ActTab::Projects);
                self.act.form = Some(activities::ProjectForm::new_for(self, None));
                Page::Activities
            }
            "client-new" => {
                self.act.open_root(activities::ActTab::Clients);
                self.act.open_client_edit(activities::ClientEdit::New(String::new()));
                Page::Activities
            }
            // the task field with Azure DevOps work items not yet in DevTrack
            "timer-devops" => {
                if !self.store.is_persistent() {
                    let mut cfg = IntegrationConfig { id: "demo-azure".into(), kind: devtrack_core::model::SOURCE_AZURE.into(), name: "Azure DevOps · contoso".into(), enabled: true, ..Default::default() };
                    cfg.settings.insert("organization".into(), "contoso".into());
                    self.store.db.integrations.insert(cfg.id.clone(), cfg);
                    let item = |id: &str, title: &str, project: &str| assigned::Item { id: id.into(), title: title.into(), status: "Active".into(), kind: "Task".into(), project: project.into(), url: String::new(), estimate: None };
                    self.assigned.cfg = Some("demo-azure".into());
                    self.assigned.at = Some(now_secs() - 2 * 3600);
                    self.assigned.items = vec![item("7001", "Invoice PDF layout", "Portal"), item("7002", "Invoice reminders by email", "Billing"), item("7003", "Invoice archive export", "Elsewhere")];
                    if let Some(e) = self.store.db.running().cloned() { let _ = e; self.store.stop(); }
                    self.today.draft.desc = "invoice".into();
                    self.today.focus_desc = true;
                }
                Page::Today
            }
            // the Tasks tab filtered by a tag
            "tasks-tag" => {
                self.act.tab = activities::ActTab::Tasks;
                self.act.all_tag = Some(TagFilter::Tag("bug".into()));
                Page::Activities
            }
            // the Tasks tab with the most tracked task open in place: estimate, sessions
            "tasks-open" => {
                let db = &self.store.db;
                let task = db.live_tasks().max_by_key(|t| db.live_entries().filter(|e| e.task_id.as_deref() == Some(t.id.as_str())).count()).cloned();
                if let Some(t) = task {
                    activities::open_task(self, &t);
                }
                Page::Activities
            }
            "assigned" | "assigned-selected" => {
                // demo: a connection and a loaded list, two of them already imported
                if !self.store.is_persistent() {
                    use devtrack_core::integrations::RemoteItem;
                    let mut cfg = IntegrationConfig { id: "demo-azure".into(), kind: devtrack_core::model::SOURCE_AZURE.into(), name: "Azure DevOps · contoso".into(), enabled: true, ..Default::default() };
                    cfg.settings.insert("organization".into(), "contoso".into());
                    self.store.db.integrations.insert(cfg.id.clone(), cfg);
                    let rows = [("4812", "Login page times out on slow networks", "Bug", "Active"), ("4820", "Session refresh fails after password change", "Bug", "Active"), ("5103", "Migration script for the 2026 schema", "Task", "New"), ("5110", "Rivisitazione grafico funzionale dell'editor, con tutte le viste secondarie e i pannelli laterali", "Task", "New"), ("5121", "CSV export endpoint", "User Story", "Active"), ("5130", "Audit log retention", "Task", "New")];
                    self.act.finder.items = rows.iter().map(|(id, t, k, st)| RemoteItem { id: id.to_string(), title: t.to_string(), description: String::new(), status: st.to_string(), kind: k.to_string(), project: "Contoso.Portal".into(), url: String::new(), assigned_to: String::new(), estimate: None }).collect();
                    self.act.finder.ids = (0..9).map(|i| self.act.finder.items.get(i).map(|x| x.id.clone()).unwrap_or(format!("9{i}"))).collect();
                    self.act.finder.cfg = Some("demo-azure".into());
                    self.act.finder.state = activities::FinderState::Loaded;
                    if name == "assigned-selected" {
                        self.act.finder.selected = ["5110", "5121", "5130"].iter().map(|s| s.to_string()).collect();
                    }
                }
                if let Some(id) = self.store.db.live_projects().first().map(|p| p.id.clone()) { self.act.show_project(id); }
                let picked = std::mem::take(&mut self.act.finder.selected);
                self.act.open_add(true);
                self.act.finder.selected = picked;
                Page::Activities
            }
            "report" | "reporttime" | "timesheet" => Page::ReportTime,
            // the second or third destination, in the order the page lists them
            "report-2" | "report-3" => {
                let n = if name == "report-2" { 1 } else { 2 };
                self.rep.dest = self.store.db.live_profiles().get(n).map(|p| p.id.clone());
                Page::ReportTime
            }
            "report-detail" | "report-single" => {
                if !self.store.is_persistent() {
                    report_time::prepare_detail(self);
                    if name == "report-single" {
                        self.rep.hidden = self.store.db.live_profiles().into_iter().skip(1).map(|p| p.id.clone()).collect();
                    }
                }
                Page::ReportTime
            }
            "statistics" | "stats" | "dashboard" => Page::Statistics,
            // the tag filter on: Today's week with #sviluppo only, Statistics of the month likewise
            "today-tag" => {
                self.today.period = Period::containing(PeriodKind::Week, devtrack_core::report::today(), self.store.db.settings.week_starts_monday);
                self.today.tag_filter = Some(TagFilter::Tag("sviluppo".into()));
                Page::Today
            }
            // three sessions of the day ticked: the list's header becomes the selection bar
            "today-select" => {
                let period = self.today.period;
                let mut es: Vec<TimeEntry> = devtrack_core::report::entries_in(&self.store.db, &period, now_secs()).into_iter().filter(|e| !e.is_running()).cloned().collect();
                es.sort_by_key(|e| e.start);
                self.today.selected = es.iter().skip(1).take(3).map(|e| e.id.clone()).collect();
                Page::Today
            }
            // the calendar of the day with the filter on: the others faint, not gone
            "today-tag-cal" => {
                self.today.timeline = true;
                self.today.tag_filter = Some(TagFilter::Tag("sviluppo".into()));
                Page::Today
            }
            // the month, for its sections down to Tags
            "stats-month" => {
                self.stats.period = Period::containing(PeriodKind::Month, devtrack_core::report::today(), self.store.db.settings.week_starts_monday);
                Page::Statistics
            }
            "stats-tag" => {
                self.stats.period = Period::containing(PeriodKind::Month, devtrack_core::report::today(), self.store.db.settings.week_starts_monday);
                self.stats.tag = Some(TagFilter::Tag("sviluppo".into()));
                Page::Statistics
            }
            "settings" => Page::Settings,
            "update" => {
                // a newer version out, as a check would leave it
                self.store.local.update_latest = Some("0.2.0".into());
                self.store.local.update_checked_ms = now_secs() * 1000;
                Page::Settings
            }
            "settings-time" => {
                self.settings.tab = settings::Tab::Time;
                Page::Settings
            }
            "settings-data" => {
                self.settings.tab = settings::Tab::Data;
                Page::Settings
            }
            "sheet" | "sheet-added" => {
                self.today.timesheet = true;
                let monday = self.store.db.settings.week_starts_monday;
                self.today.period = Period::containing(PeriodKind::Week, today_date(), monday);
                // a project added to the week with no hours yet: its row shows ×
                if name == "sheet-added" {
                    let used: Vec<Option<Id>> = self.store.db.live_entries().map(|e| e.project_id.clone()).collect();
                    if let Some(p) = self.store.db.live_projects().into_iter().find(|p| !p.archived && !used.contains(&Some(p.id.clone()))) {
                        self.today.sheet_rows.push(Some(p.id.clone()));
                    }
                }
                Page::Today
            }
            "conversions" => {
                self.settings.tab = settings::Tab::Conversions;
                // a project added to the week with no hours yet: its row shows ×
                if name == "sheet-added" {
                    let used: Vec<Option<Id>> = self.store.db.live_entries().map(|e| e.project_id.clone()).collect();
                    if let Some(p) = self.store.db.live_projects().into_iter().find(|p| !p.archived && !used.contains(&Some(p.id.clone()))) {
                        self.today.sheet_rows.push(Some(p.id.clone()));
                    }
                }
                Page::Settings
            }
            "connections" => {
                self.settings.tab = settings::Tab::Connections;
                Page::Settings
            }
            // the AI assistants tab, turned on
            "assistants" | "assistants-code" | "assistants-other" | "assistants-confirm" => {
                self.settings.tab = settings::Tab::Assistants;
                self.store.local.mcp_enabled = true;
                self.assistants.client = Some(match name { "assistants-code" => 1, "assistants-other" => 2, _ => 0 });
                self.assistants.confirm_link = name == "assistants-confirm";
                Page::Settings
            }
            "sync" => {
                self.settings.tab = settings::Tab::Sync;
                Page::Settings
            }
            "run-time" => {
                self.today.run_time_request = true;
                Page::Today
            }
            "data" => {
                self.settings.tab = settings::Tab::Data;
                Page::Settings
            }
            "gallery" => Page::Gallery,
            "timeline" => {
                self.today.timeline = true;
                Page::Today
            }
            "week-conv" => {
                self.today.period = Period::containing(PeriodKind::Week, today_date(), self.store.db.settings.week_starts_monday);
                self.today.conv_request = true;
                Page::Today
            }
            "week-list" => {
                self.today.period = Period::containing(PeriodKind::Week, today_date(), self.store.db.settings.week_starts_monday);
                Page::Today
            }
            "week" => {
                self.today.period = Period::containing(PeriodKind::Week, today_date(), self.store.db.settings.week_starts_monday);
                self.today.timeline = true;
                Page::Today
            }
            // the session editor with its tag menu open
            "edit-tags" => {
                let mut list: Vec<TimeEntry> = self.store.db.live_entries().filter(|e| !e.is_running()).cloned().collect();
                list.sort_by_key(|e| std::cmp::Reverse(e.start));
                if let Some(mut e) = list.first().cloned() {
                    for t in ["meeting", "frontend", "review", "support", "urgent"] {
                        self.store.create_tag(t);
                    }
                    e.tags = vec!["frontend".into(), "review".into()];
                    self.store.put_entry(e.clone());
                    today::open_edit(self, &e, None, false);
                }
                widgets::SHOT_OPEN_TAGS.store(true, std::sync::atomic::Ordering::Relaxed);
                Page::Today
            }
            "edit" | "edit-review" => {
                let mut list: Vec<TimeEntry> = self.store.db.live_entries().filter(|e| !e.is_running()).cloned().collect();
                list.sort_by_key(|e| (!devtrack_core::report::needs_completion(e), e.start));
                if let Some(e) = list.first() {
                    today::open_edit(self, e, None, true);
                }
                Page::Today
            }
            "edit-invalid" => {
                let mut list: Vec<TimeEntry> = self.store.db.live_entries().filter(|e| !e.is_running()).cloned().collect();
                list.sort_by_key(|e| e.start);
                if let Some(e) = list.first() {
                    today::open_edit(self, e, None, false);
                    if let Some(ed) = &mut self.today.edit {
                        ed.f.start = "25:80".into();
                        ed.attempted = true;
                        today::validate_edit(ed);
                    }
                }
                Page::Today
            }
            "clients" => {
                self.act.tab = activities::ActTab::Clients;
                Page::Activities
            }
            // Work › Tags with a few tags on the sample sessions; "-rename" and "-merge" show a
            // tag being renamed in its row and the merge question
            "tags" | "tags-rename" | "tags-merge" | "tags-delete" => {
                let mut es: Vec<TimeEntry> = self.store.db.live_entries().filter(|e| !e.is_running()).cloned().collect();
                es.sort_by_key(|e| e.start);
                let sets: [&[&str]; 5] = [&["meeting"], &["frontend", "review"], &["support"], &["meeting", "client call"], &["frontend"]];
                for (i, mut e) in es.into_iter().enumerate() {
                    e.tags = sets[i % sets.len()].iter().map(|t| t.to_string()).collect();
                    self.store.put_entry(e);
                }
                self.store.create_tag("urgent");
                self.act.tab = activities::ActTab::Tags;
                let id = |s: &Self, n: &str| s.store.db.tag_named(n).map(|t| t.id.clone()).unwrap_or_default();
                self.act.tag_edit = match name {
                    "tags-rename" => Some(activities::TagEdit::Rename(id(self, "frontend"), "Front-end".into())),
                    "tags-merge" => Some(activities::TagEdit::Merge { from: id(self, "client call"), into: id(self, "meeting") }),
                    "tags-delete" => Some(activities::TagEdit::Delete(id(self, "support"))),
                    _ => None,
                };
                Page::Activities
            }
            "tasks" => {
                self.act.tab = activities::ActTab::Tasks;
                Page::Activities
            }
            "client-detail" => {
                self.act.tab = activities::ActTab::Clients;
                if let Some(id) = self.store.db.live_clients().first().map(|c| c.id.clone()) { self.act.show_client(id); }
                Page::Activities
            }
            "client-project" => {
                if let Some(project) = self.store.db.live_projects().into_iter().find(|p| p.client_id.is_some()) {
                    let (id, client) = (project.id.clone(), project.client_id.clone());
                    if let Some(client) = client { self.act.show_client(client); }
                    self.act.open_project(id);
                }
                Page::Activities
            }
            "ledger-stress" => {
                // Isolated reproduction only. Never inject sample records into a personal archive.
                if !self.store.is_persistent() {
                    let start = devtrack_core::report::local_midnight(devtrack_core::report::today());
                    self.store.db.entries.insert("stress-long".into(), TimeEntry { id: "stress-long".into(), project_id: Some("p-portal".into()), start, stop: Some(start + 86375), ..Default::default() });
                    for n in 0..3 {
                        let id = format!("stress-overlap-{}", n);
                        self.store.db.entries.insert(id.clone(), TimeEntry { id, description: format!("Overlapping session {}", n + 1), start: start + (10 + n) * 3600, stop: Some(start + (11 + n) * 3600), ..Default::default() });
                    }
                    self.today.highlight = Some("stress-long".into());
                }
                Page::Today
            }
            "client-filter" => {
                if let Some(c) = self.store.db.live_clients().iter().find(|c| c.name.starts_with("Northwind")) {
                    self.act.client_filter = activities::ClientFilter::Client(c.id.clone());
                }
                Page::Activities
            }
            "filter-empty" => {
                self.act.search = "zzz".into();
                Page::Activities
            }
            "project-edit" | "project-new-client" => {
                let id = self.store.db.live_projects().first().map(|p| p.id.clone());
                self.act.form = Some(activities::ProjectForm::new_for(self, id));
                if name.eq_ignore_ascii_case("project-new-client") {
                    if let Some(f) = &mut self.act.form {
                        f.new_client = Some("Contoso Pharmaceuticals".into());
                    }
                }
                Page::Activities
            }
            "workspace" => {
                if let Some(id) = self.store.db.live_projects().first().map(|p| p.id.clone()) { self.act.show_project(id); }
                Page::Activities
            }
            // an imported task open, with sample source data (previews never reach the network)
            "workspace-devops" => {
                use devtrack_core::integrations::{RemoteDetail, RemoteUpdate};
                let task = self.store.db.live_entries().filter(|e| !e.is_running()).filter_map(|e| e.task_id.clone()).filter_map(|id| self.store.db.tasks.get(&id).cloned()).find(|t| t.source == devtrack_core::model::SOURCE_AZURE);
                if let Some(mut t) = task {
                    // the scene also shows estimates: 8 h from Azure DevOps on the task, which its
                    // project adds up
                    t.source_estimate_secs = Some(8 * 3600);
                    self.store.db.tasks.insert(t.id.clone(), t.clone());
                    let day = |back: i64| Some(now_secs() - back * 86400);
                    if let Some(pid) = t.project_id.clone() { self.act.show_project(pid); }
                    self.act.task_open = Some(t.id.clone());
                    self.act.remote = Some((t.id.clone(), activities::RemoteState::Loaded(RemoteDetail {
                        remaining: Some(5.5), completed: Some(2.5), estimate: Some(8.0),
                        updates: vec![
                            RemoteUpdate { when: day(1), by: "Ada Rossi".into(), changes: vec![("Remaining".into(), "8".into(), "5.5".into()), ("Completed".into(), String::new(), "2.5".into())] },
                            RemoteUpdate { when: day(3), by: "Ada Rossi".into(), changes: vec![("State".into(), "New".into(), "Active".into()), ("Comment".into(), String::new(), "Started the work on the calendar view".into())] },
                            RemoteUpdate { when: day(9), by: "Bo Verdi".into(), changes: vec![("Assigned to".into(), "Bo Verdi".into(), "Ada Rossi".into()), ("Estimate".into(), String::new(), "8".into())] },
                        ],
                    })));
                }
                Page::Activities
            }
            // a project with tracked tasks, the first one showing its sessions
            "workspace-open" => {
                let task = self.store.db.live_entries().filter(|e| !e.is_running()).filter_map(|e| e.task_id.clone()).next();
                if let Some(t) = task.as_ref().and_then(|id| self.store.db.tasks.get(id)).cloned() {
                    if let Some(pid) = t.project_id.clone() { self.act.show_project(pid); }
                    self.act.task_open = Some(t.id.clone());
                }
                Page::Activities
            }
            "edit-long" => {
                let mut list: Vec<TimeEntry> = self.store.db.live_entries().filter(|e| !e.is_running()).cloned().collect();
                list.sort_by_key(|e| e.start);
                if let Some(e) = list.first() {
                    today::open_edit(self, e, None, false);
                    if let Some(ed) = &mut self.today.edit {
                        if let Ok(d) = chrono::NaiveDate::parse_from_str(&ed.f.end_date, "%Y-%m-%d") {
                            ed.f.end_date = (d + chrono::Duration::days(2)).format("%Y-%m-%d").to_string();
                        }
                        if let Ok(r) = devtrack_core::edit::resolve(&ed.f, &today::orig_fields(ed), ed.entry.start, ed.entry.stop) {
                            ed.confirm_long = Some((r.start, r.stop.unwrap_or(r.start)));
                        }
                    }
                }
                Page::Today
            }
            // the pointer over the project bar's second segment (review pictures only)
            "project-bar" => {
                SHOT_POINTER.store(true, std::sync::atomic::Ordering::Relaxed);
                Page::Today
            }
            "timer-project" => {
                // the timer's project menu, open (review pictures only)
                widgets::SHOT_OPEN_PROJECT.store(true, std::sync::atomic::Ordering::Relaxed);
                Page::Today
            }
            // the compact window's session popover, on a finished session (with --compact)
            "compact-edit" => {
                let e = self.store.db.live_entries().filter(|e| e.stop.is_some()).max_by_key(|e| e.start).cloned();
                if let Some(e) = e {
                    today::open_edit(self, &e, None, false);
                    self.today.edit_anchor = Some(egui::Rect::from_min_size(egui::pos2(16.0, 300.0), egui::vec2(380.0, 64.0)));
                }
                Page::Today
            }
            // the compact window's history scrolled part way, a day's band pinned (with --compact)
            // the list grouped by task; "-open" shows the sessions of the groups with more than one
            // the compact window's day calendar (with --compact)
            "compact-calendar" => {
                self.store.local.compact_calendar = true;
                Page::Today
            }
            "by-task" | "by-task-open" => {
                self.store.local.list_by_task = true;
                if name == "by-task-open" {
                    let now = now_secs();
                    // the day the page shows
                    let day = self.today.period.start;
                    let mut pieces = vec![];
                    for e in self.store.db.live_entries() {
                        for (d, a, b) in devtrack_core::report::day_pieces(e, now) {
                            if d == day { pieces.push((e.clone(), a, b)); }
                        }
                    }
                    for g in by_task::group(&pieces).into_iter().filter(|g| g.pieces.len() > 1) {
                        self.today.open_groups.insert(by_task::group_id(day, &g));
                    }
                }
                Page::Today
            }
            "compact-scrolled" => {
                compact::SHOT_SCROLLED.store(true, std::sync::atomic::Ordering::Relaxed);
                Page::Today
            }
            // the period field's picker open: a day, a week
            "period-picker" | "period-picker-week" => {
                self.today.timesheet = name == "period-picker-week";
                let monday = self.store.db.settings.week_starts_monday;
                let kind = if name == "period-picker-week" { PeriodKind::Week } else { PeriodKind::Day };
                self.today.period = Period::containing(kind, today_date(), monday);
                self.preview_period_picker = true;
                Page::Today
            }
            // the dialog over another page, as the top bar opens it from anywhere
            "past-work" => {
                self.today.past_open = true;
                Page::Activities
            }
            "past" | "past-task-menu" => {
                self.today.past_open = true;
                self.today.preview_task_menu = name == "past-task-menu";
                // a long list, as in real use: recent tasks on top, the menu fills its room
                if name == "past-task-menu" && !self.store.is_persistent() {
                    let pid = self.store.db.live_projects().first().map(|p| p.id.clone());
                    for i in 1..=14 {
                        self.create_task(&format!("Sample task {i}"), pid.clone());
                    }
                }
                Page::Today
            }
            "dest-new" => {
                self.dest = Some(dest::DestDraft::new_destination(self));
                Page::ReportTime
            }
            "dest-codes" => {
                let mut d = dest::DestDraft::new_destination(self);
                d.profile.name = "Company timesheet for the Northwind framework agreement, Q4 2026".into();
                d.step = dest::Step::Codes;
                self.dest = Some(d);
                Page::ReportTime
            }
            "dest-error" => {
                let mut d = dest::DestDraft::new_destination(self);
                d.profile.name.clear();
                d.attempted = true;
                self.dest = Some(d);
                Page::ReportTime
            }
            "dest-dirty" => {
                let mut d = dest::DestDraft::new_destination(self);
                d.profile.name = "Client hours".into();
                d.confirm_discard = true;
                self.dest = Some(d);
                Page::ReportTime
            }
            "dest-fix" | "dest-fix-project" | "dest-fix-exceptions" => {
                // the first line that really needs a code, as if picked from its row
                let now = now_secs();
                let want_project = name.eq_ignore_ascii_case("dest-fix-project") || name.eq_ignore_ascii_case("dest-fix-exceptions");
                let profiles: Vec<TimesheetProfile> = self.store.db.live_profiles().into_iter().cloned().collect();
                let mut pick: Option<(TimesheetProfile, Option<Id>)> = None;
                for pr in &profiles {
                    for l in devtrack_core::report::timesheet(&self.store.db, pr, &self.rep.period, now) {
                        if l.status == devtrack_core::report::LineStatus::NotReady && pick.is_none() {
                            pick = Some((pr.clone(), l.project_id.clone()));
                        }
                    }
                }
                if want_project {
                    if let (Some(pr), Some(p)) = (profiles.first(), self.store.db.live_projects().first().map(|p| p.id.clone())) {
                        let mut pr = pr.clone();
                        pr.mappings.retain(|m| m.project_id.as_ref() != Some(&p));
                        pick = Some((pr, Some(p)));
                    }
                }
                if let Some((pr, project)) = pick {
                    let mut d = dest::DestDraft::fix_code(self, &pr, project);
                    // `dest-fix-exceptions`: the same dialog with two task exceptions open
                    if name.eq_ignore_ascii_case("dest-fix-exceptions") {
                        for pc in d.codes.iter_mut() {
                            for _ in 0..2 { pc.exceptions.push(dest::Exception { task: None, code: String::new(), activity: String::new() }); }
                        }
                    }
                    self.dest = Some(d);
                }
                Page::ReportTime
            }
            "dest-settings" => {
                if let Some(pr) = self.store.db.live_profiles().first().map(|p| (*p).clone()) {
                    self.dest = Some(dest::DestDraft::edit(self, &pr, dest::Step::Settings));
                }
                Page::ReportTime
            }
            "move" => {
                if let Some(t) = self.store.db.live_tasks().find(|t| self.store.db.live_entries().any(|e| e.task_id.as_deref() == Some(t.id.as_str()))) {
                    let n = self.store.db.live_entries().filter(|e| e.task_id.as_deref() == Some(t.id.as_str())).count();
                    if let Some(pid) = t.project_id.clone() { self.act.show_project(pid); }
                    self.act.pending_move = Some(activities::MoveDraft { task: t.id.clone(), to: None, also_sessions: false, sessions: n });
                }
                Page::Activities
            }
            _ => Page::Today,
        };
        self.page = page;
    }

    /// Review pictures: the same period kind and date on Today, Report Time and Statistics.
    pub fn set_period(&mut self, kind: &str, date: Option<chrono::NaiveDate>) {
        let monday = self.store.db.settings.week_starts_monday;
        let k = match kind {
            "week" => PeriodKind::Week,
            "month" => PeriodKind::Month,
            _ => PeriodKind::Day,
        };
        let d = date.unwrap_or(self.today.period.start);
        let p = Period::containing(k, d, monday);
        self.today.period = p;
        self.rep.period = p;
        self.stats.period = p;
    }

    /// The most recent day before `before` that has a session, for empty pages to point at.
    pub fn last_day_with_sessions(&self, before: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
        self.store.db.live_entries().map(|e| devtrack_core::report::local_date(e.start)).filter(|d| *d < before).max()
    }

    pub fn show_last_working_day(&mut self) {
        let d = devtrack_core::sample::last_working_day();
        let monday = self.store.db.settings.week_starts_monday;
        self.today.period = Period::containing(PeriodKind::Day, d, monday);
        self.rep.period = Period::containing(PeriodKind::Day, d, monday);
    }

    pub fn dark(&self) -> bool {
        if self.store.db.settings.follow_system_theme {
            if let Some(d) = self.system_dark {
                return d;
            }
        }
        self.store.db.settings.dark
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some(Toast { msg: msg.into(), at: Instant::now(), undo: false });
    }

    pub fn notify_undo(&mut self, msg: impl Into<String>) {
        self.toast = Some(Toast { msg: msg.into(), at: Instant::now(), undo: true });
    }

    pub fn remember_focus(&mut self, ctx: &egui::Context) {
        self.return_focus = ctx.memory(|m| m.focused());
    }

    pub fn restore_focus(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.return_focus.take() {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }

    /// Something in Settings needs attention (shown as a small mark on the Settings item).
    pub fn settings_problem(&self) -> Option<String> {
        if let Some(b) = &self.store.blocked {
            return Some(b.clone());
        }
        if let Some(e) = &self.store.save_error {
            return Some(trf_m!("Last save failed: {}", e));
        }
        if let Some(Err(e)) = &self.sync.result {
            return Some(trf_m!("Sync: {}", e));
        }
        if let Some((_, (_, m))) = self.conn_msgs.iter().find(|(_, (ok, _))| !ok) {
            return Some(trf_m!("Connection: {}", m));
        }
        None
    }

    // ---------- compact window ----------

    pub fn modal_open(&self) -> bool {
        self.today.edit.is_some() || self.today.past_open || self.dest.is_some() || self.act.pending_move.is_some() || self.act.add_open || self.act.form.is_some() || self.idle_prompt.is_some()
    }

    /// Idle detection and the "you're not tracking" reminder, checked once a frame.
    fn watch_time(&mut self) {
        let now = now_secs();
        let idle = self.idle_secs.load(std::sync::atomic::Ordering::Relaxed);
        let threshold = self.store.local.idle_minutes as i64 * 60;
        if self.store.db.running().is_some() && threshold > 0 {
            if idle >= threshold {
                // the stretch began when input stopped, not when the threshold was reached
                self.idle_since.get_or_insert(now - idle);
            } else if idle < 30 {
                if let Some(since) = self.idle_since.take() {
                    self.idle_prompt = Some(since);
                }
            }
        } else {
            self.idle_since = None;
        }
        // estimate: say once when the running session takes its task past the estimate
        if let Some(task) = self.store.db.running().and_then(|e| self.store.db.task(&e.task_id)).cloned() {
            if let Some(est) = task.planned_secs() {
                if self.store.db.task_total(&task.id, now) > est && self.estimate_alerted.insert(task.id.clone()) {
                    let text = trf_m!("“{}” is past its planned {}", truncate(&task.title, 40), words(est));
                    system_notify("DevTrack", &text);
                    self.notify(text);
                }
            }
        }
        // reminder: nothing running or paused for a while, on a weekday between 9 and 18,
        // with the person at the computer
        let every = self.store.local.remind_minutes as i64 * 60;
        if every > 0 && self.store.db.running().is_none() && self.store.paused().is_none() && idle < 120 {
            use chrono::Timelike;
            let t = chrono::Local::now();
            let st = &self.store.db.settings;
            let minute = (t.hour() * 60 + t.minute()) as u16;
            let working = st.is_work_day(t.date_naive()) && minute >= st.work_start_min && minute < st.work_end_min;
            let last_stop = self.store.db.live_entries().filter_map(|e| e.stop).max().unwrap_or(0);
            if working && now - last_stop >= every && now - self.last_reminder >= every {
                self.last_reminder = now;
                system_notify("DevTrack", &tr("You're not tracking time."));
                self.notify(tr("You're not tracking time."));
            }
        }
    }

    /// Asks what to do with the time spent away while a session kept running.
    fn idle_modal(&mut self, ctx: &egui::Context) {
        let Some(since) = self.idle_prompt else { return };
        let Some(running) = self.store.db.running().cloned() else {
            self.idle_prompt = None;
            return;
        };
        let c = pal(self.dark());
        let h24 = self.store.db.settings.clock_24h;
        let mut choice = 0u8;
        let mut x_close = false;
        let modal = egui::Modal::new(egui::Id::new("idle-time")).frame(today::modal_frame(&c)).backdrop_color(egui::Color32::from_black_alpha(if self.dark() { 160 } else { 90 })).show(ctx, |ui| {
            ui.set_width(480.0f32.min(ctx.screen_rect().width() - 96.0));
            x_close = modal_close(ui);
            ui.label(RichText::new(tr("You were away while the session ran")).font(semibold(20.0)));
            ui.add_space(ROW_GAP);
            mixed(ui, &trf_m!("No keyboard or mouse activity since {} ({}). What should happen to that time?", clock_text(since, h24), words((now_secs() - since).max(0))), 14.0, Some(c.text2));
            ui.add_space(GROUP_GAP);
            divider(ui);
            ui.add_space(12.0);
            row_wrapped(ui, CTRL_H, |ui| {
                if primary(ui, "Discard it and continue").on_hover_text(tr("The session ends when you left; a new one starts now with the same details")).clicked() { choice = 1; }
                if secondary(ui, "Discard it and stop").on_hover_text(tr("The session ends when you left")).clicked() { choice = 2; }
                if secondary(ui, "Keep it").on_hover_text(tr("The time away stays in the session")).clicked() { choice = 3; }
            });
        });
        if modal.should_close() || x_close {
            choice = 3;
        }
        match choice {
            1 => {
                self.store.stop_idle(since);
                self.store.start(&running.description, running.project_id.clone(), running.task_id.clone(), running.billable);
                self.notify(tr("Idle time discarded; a new session is running"));
            }
            2 => {
                self.store.stop_idle(since);
                self.notify(tr("Idle time discarded; the session is stopped"));
            }
            _ => {}
        }
        if choice != 0 {
            self.idle_prompt = None;
        }
    }

    /// The logo's mark turns while a session runs, unless motion is reduced; it stays still in
    /// review pictures, so they come out the same every time.
    pub fn logo_spins(&self) -> bool {
        self.store.db.running().is_some() && !self.store.db.settings.reduce_motion && self.shot.is_none()
    }

    /// Clamp a window position so its size fits inside the screen (kept on screen).
    pub fn clamp_to_screen(pos: egui::Pos2, size: egui::Vec2, screen: Option<egui::Rect>) -> egui::Pos2 {
        let Some(m) = screen else { return pos };
        let x = pos.x.min(m.right() - size.x).max(m.left());
        let y = pos.y.min(m.bottom() - size.y).max(m.top());
        egui::pos2(x, y)
    }

    /// Where the window goes when it switches between full and compact: its top-left corner stays
    /// where it is, on the screen it is on, moved only as much as the new size needs to fit there.
    fn switch_position(info: &egui::ViewportInfo, size: egui::Vec2) -> Option<egui::Pos2> {
        let outer = info.outer_rect?;
        let screen = monitors::screen_at(outer.center(), info.monitor_size);
        Some(Self::clamp_to_screen(outer.min, size, screen))
    }

    pub fn enter_compact(&mut self, ctx: &egui::Context) {
        if self.mode == WinMode::Compact {
            return;
        }
        if self.modal_open() {
            // a dialog may hold unsaved changes: finish or cancel it first
            self.notify(tr("Finish or cancel the open dialog first"));
            return;
        }
        let info = ctx.input(|i| i.viewport().clone());
        self.full_outer = info.outer_rect;
        self.full_inner = info.inner_rect.map(|r| r.size());
        self.full_page = self.page;
        self.mode = WinMode::Compact;
        // the compact window keeps the size it had last time, and opens where the full one is
        let size = match self.store.local.compact_rect {
            Some([_, _, w, h]) => egui::vec2(w.max(COMPACT_MIN[0]), h.max(COMPACT_MIN[1])),
            None => egui::vec2(COMPACT_SIZE[0], COMPACT_SIZE[1]),
        };
        let pos = Self::switch_position(&info, size).unwrap_or(egui::pos2(40.0, 40.0));
        if self.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(COMPACT_MIN[0], COMPACT_MIN[1])));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }
    }

    /// Back to the full window: previous bounds and page (or `to` when given).
    pub fn exit_compact(&mut self, ctx: &egui::Context, to: Option<Page>) {
        if self.mode == WinMode::Full {
            return;
        }
        if self.modal_open() {
            self.notify(tr("Finish or cancel the open dialog first"));
            return;
        }
        let info = ctx.input(|i| i.viewport().clone());
        if let (Some(o), Some(i)) = (info.outer_rect, info.inner_rect) {
            self.store.local.compact_rect = Some([o.min.x, o.min.y, i.width(), i.height()]);
            self.store.save_local();
        }
        self.mode = WinMode::Full;
        let size = self.full_inner.unwrap_or(egui::vec2(1360.0, 860.0));
        // the full window keeps its size and opens where the compact one is
        let pos = Self::switch_position(&info, size).unwrap_or(egui::pos2(40.0, 40.0));
        if self.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(360.0, 480.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        }
        self.page = to.unwrap_or(self.full_page);
    }

    // ---------- sync ----------

    fn maybe_sync(&mut self) {
        if self.store.local.sync_folder.is_none() || !self.store.is_persistent() {
            return;
        }
        if self.store.generation != self.gen_seen {
            self.gen_seen = self.store.generation;
            self.gen_changed_at = Instant::now();
        }
        let due = Instant::now() >= self.next_sync
            || (self.store.generation != self.synced_gen && self.gen_changed_at.elapsed() >= Duration::from_secs(2));
        if due {
            self.sync_now();
        }
    }

    /// The menu bar item: tells it what runs and what can be done, carries out what it asked
    /// for, and keeps DevTrack running when the window is closed.
    #[cfg(target_os = "macos")]
    fn menu_bar_item(&mut self, ctx: &egui::Context) {
        use crate::tray::{self, Cmd};
        if !self.tray {
            return;
        }
        let running = self.store.db.running().map(|e| e.start);
        let last = self.store.db.live_entries().filter(|e| !e.is_running()).max_by_key(|e| e.start).map(|e| e.id.clone());
        let can_continue = self.store.paused().is_some() || last.is_some();
        // today's total: finished time today, plus the running session from today's midnight
        let today = devtrack_core::report::Period::containing(devtrack_core::report::PeriodKind::Day, devtrack_core::report::today(), true);
        let (t0, t1) = today.bounds();
        let now = now_secs();
        let done_today: i64 = self.store.db.live_entries().filter(|e| !e.is_running()).map(|e| devtrack_core::report::overlap(e, t0, t1, now)).sum();
        tray::set_state(running, can_continue, self.store.local.sync_folder.is_some());
        tray::set_title_mode(self.store.local.tray_shows, done_today, running.map(|s| s.max(t0)));
        // the time beside the mark is counted by the item itself; a repaint now and then keeps
        // its state current while the window is hidden
        ctx.request_repaint_after(Duration::from_secs(5));
        match tray::take_cmd() {
            Cmd::None => {}
            Cmd::Show => ctx.send_viewport_cmd(egui::ViewportCommand::Focus),
            Cmd::New => {
                if self.mode == WinMode::Compact {
                    self.mode = WinMode::Full;
                }
                self.go(Page::Today);
                self.today.focus_desc = true;
            }
            Cmd::Continue => {
                if self.store.paused().is_some() {
                    self.store.resume();
                } else if let Some(id) = last {
                    self.store.continue_entry(&id);
                }
            }
            Cmd::Stop => {
                self.store.stop();
            }
            Cmd::Sync => self.sync_now(),
            Cmd::Report => self.go(Page::ReportTime),
            Cmd::Settings => self.go(Page::Settings),
        }
        if std::mem::take(&mut self.start_hidden) {
            tray::hide_app();
        }
        // closing the window keeps DevTrack running in the menu bar; ⌘Q and Quit end it
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            tray::hide_app();
        }
    }

    pub fn sync_now(&mut self) {
        let r = sync::run(&mut self.store, &mut self.sync.seen, &mut self.sync.written_gen);
        self.synced_gen = self.store.generation;
        self.gen_seen = self.store.generation;
        self.next_sync = Instant::now() + Duration::from_secs(30);
        if let Ok(o) = &r {
            for e in &o.events {
                self.sync.events.push((o.at_ms, e.clone()));
            }
            let n = self.sync.events.len();
            if n > 50 {
                self.sync.events.drain(0..n - 50);
            }
            if o.timers_stopped > 0 {
                self.notify(tr("Another device started a timer: the older one was stopped (Settings › Sync)"));
            }
        }
        self.sync.result = Some(r);
    }

    /// Gives sessions that only have a description the task of that name, once per data set,
    /// and says how many changed.
    pub fn convert_descriptions_once(&mut self) {
        if self.store.db.settings.descriptions_are_tasks {
            return;
        }
        let (tasks, changed) = devtrack_core::integrations::entries_import::descriptions_to_tasks(&self.store.db);
        let n = changed.len();
        for t in tasks { self.store.put_task(t); }
        for e in changed { self.store.put_entry(e); }
        let mut st = self.store.db.settings.clone();
        st.descriptions_are_tasks = true;
        self.store.put_settings(st);
        if n > 0 {
            self.notify(i18n::plural(n as i64, "{} session now has its task", "{} sessions now have their task"));
        }
    }

    /// The task called `name` in `project` (letter case aside), made as a local task when there is
    /// none. Used wherever a task is named by typing.
    pub fn task_named(&mut self, project: Option<Id>, name: &str) -> Id {
        let key = name.trim().to_lowercase();
        if let Some(t) = self.store.db.live_tasks().find(|t| t.project_id == project && !t.done && t.title.trim().to_lowercase() == key) {
            return t.id.clone();
        }
        let t = Task { id: new_id(), project_id: project, title: name.trim().to_string(), status: "Open".into(), source: devtrack_core::model::SOURCE_MANUAL.into(), ..Default::default() };
        let id = t.id.clone();
        self.store.put_task(t);
        id
    }

    pub fn start_draft(&mut self) {
        let d = self.today.draft.clone();
        let (desc, p, t, b) = (d.desc.clone(), d.project.clone(), d.task.clone(), d.billable);
        let d = &d;
        // no project chosen: the default one, when it still exists
        let p = p.or_else(|| self.store.db.settings.default_project.clone().filter(|id| self.store.db.projects.get(id).is_some_and(|x| !x.deleted && !x.archived)));
        let b = b || (d.project.is_none() && p.as_ref().and_then(|id| self.store.db.projects.get(id)).is_some_and(|x| x.billable_default));
        // the name typed is the task, not a note: an existing task of the project or a new one
        let t = match (t, &d.remote) {
            (Some(t), _) => Some(t),
            // a work item chosen from Azure DevOps becomes its task now, in the session's project
            (None, Some(n)) => assigned::import(self, n, p.clone()).or_else(|| Some(self.task_named(p.clone(), &desc))),
            (None, None) if !desc.trim().is_empty() => Some(self.task_named(p.clone(), &desc)),
            (None, None) => None,
        };
        // an imported work item that already had a project keeps it
        let p = p.or_else(|| self.store.db.task(&t).and_then(|x| x.project_id.clone()));
        let id = self.store.start("", p, t, b);
        // the tags chosen before Start go with the session
        if !d.tags.is_empty() {
            if let Some(mut e) = self.store.db.entries.get(&id).cloned() {
                e.tags = d.tags.clone();
                self.store.put_entry(e);
            }
        }
        self.today.draft = today::Draft::default();
    }

    pub fn create_project(&mut self, name: &str) -> Id {
        let p = Project { id: new_id(), name: name.trim().to_string(), color: self.store.db.next_color(), ..Default::default() };
        let id = p.id.clone();
        self.store.put_project(p);
        self.notify(trf_m!("Project “{}” created", truncate(name.trim(), 40)));
        id
    }

    pub fn create_task(&mut self, title: &str, project: Option<Id>) -> Id {
        let t = Task { id: new_id(), project_id: project, title: title.trim().to_string(), status: "Open".into(), source: SOURCE_MANUAL.into(), ..Default::default() };
        let id = t.id.clone();
        self.store.put_task(t);
        self.notify(trf_m!("Task “{}” created", truncate(title.trim(), 40)));
        id
    }

    fn handle_job(&mut self, out: jobs::JobOut) {
        use jobs::JobOut::*;
        match out {
            // read in the background for the task field (assigned.rs)
            Assigned { .. } | Lookup { .. } => {}
            Test { cfg, res } => {
                let v = match res {
                    Ok(m) => (true, m),
                    Err(e) => (false, e.message),
                };
                self.conn_msgs.insert(cfg, v);
            }
            Detail { task, res } => {
                // kept only if that task's detail is still the one being shown
                // the estimate read now is the task's, shown and added up without reading again
                if let Ok(d) = &res {
                    let secs = d.estimate.filter(|h| *h > 0.0).map(|h| (h * 3600.0).round() as i64);
                    if let Some(t) = self.store.db.tasks.get(&task).cloned() {
                        if t.source_estimate_secs != secs {
                            let mut x = t;
                            x.source_estimate_secs = secs;
                            self.store.put_task(x);
                        }
                    }
                }
                if self.act.remote.as_ref().map(|(id, _)| id == &task).unwrap_or(false) {
                    self.act.remote = Some((task, match res {
                        Ok(d) => activities::RemoteState::Loaded(d),
                        Err(e) => activities::RemoteState::Failed(e.message),
                    }));
                }
            }
            Found { cfg, .. } | More { cfg, .. } if self.act.finder.cfg.as_ref() != Some(&cfg) => {}
            Found { cfg: _, res } => match res {
                Ok((ids, items, projects)) => {
                    self.act.finder.selected.clear();
                    self.act.finder.state = if ids.is_empty() { activities::FinderState::NoResults } else { activities::FinderState::Loaded };
                    self.act.finder.ids = ids;
                    self.act.finder.items = items;
                    if let Some(p) = projects {
                        self.act.finder.remote_projects = p;
                    }
                }
                Err(e) => self.act.finder.set_error(e),
            },
            More { cfg: _, res } => match res {
                Ok(items) => {
                    self.act.finder.items.extend(items);
                    self.act.finder.state = activities::FinderState::Loaded;
                }
                Err(e) => self.act.finder.set_error(e),
            },
            Refresh { cfg, asked, res } => {
                let Some(mut c) = self.store.db.integrations.get(&cfg).cloned() else { return };
                match res {
                    Ok(items) => {
                        let org = org_of(&c);
                        let got: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
                        for it in &items {
                            let key = Task::external_key(&c.kind, &org, &it.id);
                            let existing = self.store.db.tasks.get(&key).cloned();
                            let t = devtrack_core::integrations::to_task(&c, &org, it, existing.as_ref(), None);
                            if existing.as_ref() != Some(&t) {
                                self.store.put_task(t);
                            }
                        }
                        let mut missing = 0;
                        for id in asked.iter().filter(|i| !got.contains(i)) {
                            let key = Task::external_key(&c.kind, &org, id);
                            if let Some(mut t) = self.store.db.tasks.get(&key).cloned() {
                                if !t.unavailable {
                                    t.unavailable = true;
                                    self.store.put_task(t);
                                }
                                missing += 1;
                            }
                        }
                        c.last_refresh = Some(now_secs());
                        self.store.put_integration(c);
                        let msg = if missing > 0 {
                            trf_m!("{} task(s) refreshed; {} not accessible (kept with their history)", got.len(), missing)
                        } else {
                            trf_m!("{} imported task(s) refreshed", got.len())
                        };
                        self.notify(msg.clone());
                        self.conn_msgs.insert(cfg, (true, msg));
                    }
                    Err(e) => {
                        let msg = match e.kind {
                            ErrKind::Auth => trf_m!("{} Imported tasks and their history are unchanged.", e.message),
                            _ => e.message,
                        };
                        self.conn_msgs.insert(cfg, (false, msg.clone()));
                        self.act.finder.refresh_msg = Some((false, msg));
                    }
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.modal_open() {
            return; // dialogs own the keyboard while open
        }
        if self.page == Page::Activities && !ctx.wants_keyboard_input() && !ctx.memory(|m| m.any_popup_open()) {
            if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowLeft)) && self.act.back() { self.fade_content(); }
        }
        let (toggle, pause, p1, p2, p3, p4, cm) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::Enter),
                i.consume_key(Modifiers::COMMAND, Key::P),
                i.consume_key(Modifiers::COMMAND, Key::Num1),
                i.consume_key(Modifiers::COMMAND, Key::Num2),
                i.consume_key(Modifiers::COMMAND, Key::Num3),
                i.consume_key(Modifiers::COMMAND, Key::Num4),
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::M),
            )
        });
        if self.mode == WinMode::Full && !self.narrow && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::B)) {
            self.toggle_sidebar();
        }
        if toggle {
            if self.store.db.running().is_some() {
                self.store.stop();
            } else {
                self.start_draft();
            }
        }
        if pause {
            if self.store.db.running().is_some() {
                self.store.pause();
            } else if self.store.paused().is_some() {
                self.store.resume();
            }
        }
        // single keys, only while nothing is being typed and no menu is open
        if !ctx.wants_keyboard_input() && !ctx.memory(|m| m.any_popup_open()) {
            let (s_key, c_key) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::S), i.consume_key(Modifiers::NONE, Key::C)));
            if s_key {
                if self.store.db.running().is_some() {
                    self.store.stop();
                } else {
                    self.start_draft();
                }
            }
            if c_key && self.store.db.running().is_none() {
                let last = self.store.db.live_entries().filter(|e| e.stop.is_some()).max_by_key(|e| e.start).map(|e| e.id.clone());
                if let Some(id) = last {
                    self.store.continue_entry(&id);
                    self.notify(tr("Continuing the last session"));
                }
            }
        }
        if cm {
            match self.mode {
                WinMode::Full => self.enter_compact(ctx),
                WinMode::Compact => self.exit_compact(ctx, None),
            }
        }
        if self.mode == WinMode::Full {
            if p1 {
                self.nav_to(Page::Today);
            }
            if p2 {
                self.nav_to(Page::Activities);
            }
            if p3 {
                self.nav_to(Page::ReportTime);
            }
            if p4 {
                self.nav_to(Page::Statistics);
            }
        }
    }

    fn nav_item(&mut self, ui: &mut egui::Ui, page: Page, shortcut: Option<usize>, count: usize, problem: Option<String>) {
        let c = p(ui);
        let selected = self.page == page || (page == Page::Settings && self.page == Page::Gallery);
        let h = 38.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
        if selected {
            ui.painter().rect_filled(rect, RADIUS, c.selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, RADIUS, c.hover);
        }
        if resp.has_focus() {
            ui.painter().rect_stroke(rect.expand(2.0), RADIUS + 2.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
        }
        let icon_c = if self.rail { rect.center() } else { rect.left_center() + egui::vec2(16.0, 0.0) };
        // the colour of the sidebar: every page icon is amber; the open page is told by its
        // filled row and heavier label
        draw_icon(ui.painter(), icon_c, page.icon(), c.amber);
        if !self.rail {
            let font = if selected { medium(14.0) } else { sans(14.0) };
            ui.painter().text(rect.left_center() + egui::vec2(34.0, 0.0), egui::Align2::LEFT_CENTER, tr(page.name()), font, c.text);
            let mut right = rect.right_center() - egui::vec2(10.0, 0.0);
            if let Some(n) = shortcut {
                let hint = if cfg!(target_os = "macos") { format!("⌘{}", n) } else { format!("Ctrl+{}", n) };
                let g = ui.painter().layout_no_wrap(hint, mono_font(12.0), c.text2);
                let w = g.size().x;
                ui.painter().galley(right - egui::vec2(w, g.size().y / 2.0), g, c.text2);
                right -= egui::vec2(w + 10.0, 0.0);
            }
            if count > 0 {
                let r = egui::Rect::from_center_size(right - egui::vec2(10.0, 0.0), egui::vec2(22.0, 18.0));
                // filled: a count of things to fix reads as a signal, not as another amber label
                ui.painter().rect_filled(r, 5.0, c.amber);
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{}", count), mono_medium(12.0), c.canvas);
            }
            if problem.is_some() {
                ui.painter().circle_filled(right - egui::vec2(4.0, 0.0), 4.0, c.amber);
            }
        }
        let tip = match page {
            Page::Today => "Current session, today's work log and what needs details".to_string(),
            Page::Activities => "Projects, tasks and Azure DevOps work items".to_string(),
            Page::ReportTime => "Actual time next to the values each destination needs".to_string(),
            Page::Statistics => "Totals per day, project and task".to_string(),
            _ => match &problem {
                Some(pr) => trf_m!("Settings · needs attention: {}", pr),
                None => "Display, sync, connections and data".to_string(),
            },
        };
        let name = tr(page.name());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name.clone()));
        if resp.on_hover_text(tip).clicked() {
            self.nav_to(page);
        }
    }

    /// A newer version is out: a quiet green row above Settings that opens the download page.
    fn update_item(&mut self, ui: &mut egui::Ui, version: &str) {
        let c = p(ui);
        let h = 38.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
        let hot = resp.hovered() || resp.has_focus();
        ui.painter().rect_filled(rect, RADIUS, c.green.gamma_multiply(if hot { 0.22 } else { 0.14 }));
        if resp.has_focus() {
            ui.painter().rect_stroke(rect.expand(2.0), RADIUS + 2.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
        }
        // ready: restart now (it also installs on its own when DevTrack ends); downloading: just
        // the state; otherwise the download page
        let ready = self.update.ready.as_deref() == Some(version);
        let downloading = self.update.downloading() == Some(version);
        let (label, short, tip) = if ready {
            (i18n::trf("Restart to update to {}", &[&version]), tr("Restart to update"), tr("DevTrack also updates on its own the next time you quit it. Your data stays where it is."))
        } else if downloading {
            (i18n::trf("Downloading DevTrack {}…", &[&version]), tr("Downloading…"), tr("It installs when you quit DevTrack, or at once with Restart."))
        } else {
            (i18n::trf("DevTrack {} is out", &[&version]), tr("Update available"), tr("Opens the download page in the browser. Your data stays where it is."))
        };
        let glyph = if ready { "↻" } else { "↓" };
        if self.rail {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, medium(16.0), c.green);
        } else {
            ui.painter().text(rect.left_center() + egui::vec2(16.0, 0.0), egui::Align2::CENTER_CENTER, glyph, medium(15.0), c.green);
            // the full words when they fit, the short ones when not, and never past the row
            let room = rect.width() - 34.0 - 12.0;
            let fits = |t: &str| ui.painter().layout_no_wrap(t.to_string(), sans(13.0), c.green).size().x <= room;
            let mut text = if fits(&label) { label.clone() } else { short };
            while !fits(&text) && text.chars().count() > 2 {
                let keep = text.chars().count() - 2;
                text = format!("{}…", text.chars().take(keep).collect::<String>().trim_end_matches('…'));
            }
            let g = ui.painter().layout_no_wrap(text, sans(13.0), c.green);
            ui.painter().galley(egui::pos2(rect.left() + 34.0, rect.center().y - g.size().y / 2.0), g, c.green);
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !downloading, label.clone()));
        if resp.on_hover_text(tip).clicked() {
            if ready {
                self.restart_to_update();
            } else if !downloading {
                ui.ctx().open_url(egui::OpenUrl::new_tab(devtrack_core::update::DOWNLOAD_PAGE));
            }
        }
    }

    /// The control that reduces the sidebar to its icons and brings it back.
    fn sidebar_toggle(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let c = p(ui);
        let resp = ui.interact(rect, egui::Id::new("sidebar-toggle"), Sense::click());
        let active = resp.hovered() || resp.is_pointer_button_down_on();
        if active {
            ui.painter().rect_filled(rect, RADIUS, c.hover);
        }
        draw_glyph(ui.painter(), rect.center(), Glyph::Sidebar, if active { c.text } else { c.text2 });
        if resp.has_focus() {
            focus_ring(ui, rect);
        }
        let k = if cfg!(target_os = "macos") { "⌘B" } else { "Ctrl+B" };
        let name = trf_m!(if self.store.local.sidebar_collapsed { "Show sidebar ({})" } else { "Hide sidebar ({})" }, k);
        let n = name.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, n.clone()));
        if resp.on_hover_text(name).clicked() {
            self.toggle_sidebar();
        }
    }

    fn toggle_sidebar(&mut self) {
        self.store.local.sidebar_collapsed = !self.store.local.sidebar_collapsed;
        self.store.save_local();
    }

    /// Ask whether a newer DevTrack is out, in the background. `asked`: from "Check now".
    pub fn check_update(&mut self, asked: bool) {
        if self.update.checking() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(devtrack_core::update::latest());
        });
        self.update.rx = Some(rx);
        self.update.asked = asked;
        self.update.error = None;
    }

    /// A newer version than this one, as of the last check.
    pub fn update_available(&self) -> Option<String> {
        self.store.local.update_latest.clone().filter(|v| devtrack_core::update::is_newer(v, &devtrack_core::update::current()))
    }

    /// Download and prepare `version` in the background, when this copy can replace itself.
    fn start_download(&mut self, version: String) {
        if self.update.download.is_some() || self.update.ready.as_deref() == Some(version.as_str()) || devtrack_core::update::can_update_itself().is_none() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let v = version.clone();
        std::thread::spawn(move || {
            let _ = tx.send(devtrack_core::update::download(&v));
        });
        self.update.download = Some((version, rx));
        self.update.install_error = None;
    }

    /// The prepared version waits for DevTrack to end: a script swaps it in then.
    fn arm_install(&mut self, version: &str, prepared: &std::path::Path) {
        match devtrack_core::update::can_update_itself() {
            Some(app) => match devtrack_core::update::install_when_closed(&app, prepared) {
                Ok(()) => self.update.ready = Some(version.to_string()),
                Err(e) => self.update.install_error = Some(e),
            },
            None => self.update.install_error = Some("DevTrack cannot replace itself in its folder.".into()),
        }
    }

    /// Install the prepared version now: DevTrack ends and the script opens the new one.
    pub fn restart_to_update(&mut self) {
        if self.update.ready.is_none() {
            return;
        }
        if let Err(e) = devtrack_core::update::request_restart() {
            self.update.install_error = Some(e);
            return;
        }
        self.store.save();
        self.store.save_local();
        std::process::exit(0);
    }

    fn poll_update(&mut self, ctx: &egui::Context) {
        // every time DevTrack opens, then every few hours while it stays open; never for
        // pictures or previews
        let every = Duration::from_millis(devtrack_core::update::CHECK_EVERY_MS as u64);
        if !self.update.started && self.frame >= 2 && self.shot.is_none() && self.store.is_persistent() {
            self.update.started = true;
            self.update.next_check = Some(Instant::now() + every);
            if self.store.local.update_check {
                // the answer decides what to download: the version remembered may be older
                self.check_update(false);
            } else if let Some(v) = self.update_available() {
                // a version found earlier: prepared already, it waits again for DevTrack to end
                if let (Some(prepared), Some(_)) = (devtrack_core::update::staged(&v), devtrack_core::update::can_update_itself()) {
                    self.arm_install(&v, &prepared);
                } else {
                    self.start_download(v);
                }
            }
        } else if self.update.started && self.store.local.update_check && self.update.next_check.is_some_and(|t| Instant::now() >= t) {
            self.update.next_check = Some(Instant::now() + every);
            self.check_update(false);
        }
        if let Some((v, rx)) = &self.update.download {
            match rx.try_recv() {
                Ok(res) => {
                    let v = v.clone();
                    self.update.download = None;
                    match res {
                        Ok(prepared) => self.arm_install(&v, &prepared),
                        Err(e) => self.update.install_error = Some(e),
                    }
                    // a newer one came out meanwhile: that is the one to install
                    if let Some(newer) = self.update_available().filter(|n| *n != v) {
                        self.start_download(newer);
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(500)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.update.download = None,
            }
        }
        let Some(rx) = &self.update.rx else { return };
        match rx.try_recv() {
            Ok(res) => {
                self.update.rx = None;
                match res {
                    Ok(v) => {
                        self.store.local.update_latest = Some(v);
                        self.store.local.update_checked_ms = now_secs() * 1000;
                        self.store.save_local();
                        if let Some(v) = self.update_available() {
                            if let (Some(prepared), Some(_)) = (devtrack_core::update::staged(&v), devtrack_core::update::can_update_itself()) {
                                self.arm_install(&v, &prepared);
                            } else {
                                self.start_download(v);
                            }
                        }
                    }
                    Err(e) => self.update.error = Some(e),
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(250)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.update.rx = None,
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let c = p(ui);
        if TITLE_STRIP > 0.0 {
            // the strip under the window buttons: it moves the window, and when the sidebar is
            // open it carries the control that collapses it, right after the buttons
            let (strip, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_STRIP), Sense::hover());
            let panel = ui.max_rect().expand2(egui::vec2(10.0, 0.0));
            window_drag(ui, egui::Rect::from_min_max(panel.min, egui::pos2(panel.right(), strip.bottom())), "sidebar");
            if !self.rail {
                let at = egui::pos2(panel.left() + LIGHTS_W + 2.0, strip.center().y - 14.0);
                self.sidebar_toggle(ui, egui::Rect::from_min_size(at, egui::vec2(28.0, 28.0)));
            }
            ui.add_space(10.0);
        } else {
            ui.add_space(18.0);
        }
        ui.horizontal(|ui| {
            // the mark stands over the column of the icons
            let lead = if self.rail { ((ui.available_width() - 28.0) / 2.0).floor() } else { 8.0 };
            ui.add_space(lead);
            let logo = brand_lockup(ui, 28.0, !self.rail, self.logo_spins());
            if let Some(note) = self.data_note {
                if !self.rail {
                    let right = egui::pos2(ui.max_rect().right() - 6.0, logo.bottom() - 2.0);
                    ui.painter().text(right, egui::Align2::RIGHT_BOTTOM, tr(note).to_uppercase(), medium(11.0), c.amber);
                }
            }
            if TITLE_STRIP == 0.0 && !self.rail && !self.narrow {
                // with a system title bar the control sits at the end of the logo's row
                let r = ui.max_rect();
                self.sidebar_toggle(ui, egui::Rect::from_min_size(egui::pos2(r.right() - 28.0, logo.center().y - 14.0), egui::vec2(28.0, 28.0)));
            }
        });
        if let (Some(note), true) = (self.data_note, self.rail) {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| ui.label(RichText::new(tr(note).to_uppercase()).font(medium(10.0)).color(c.amber)));
        }
        ui.add_space(22.0);
        ui.spacing_mut().item_spacing.y = 4.0;
        let pending = self.store.db.live_entries().filter(|e| devtrack_core::report::needs_completion(e) && !e.is_running()).count();
        self.nav_item(ui, Page::Today, Some(1), pending, None);
        self.nav_item(ui, Page::Activities, Some(2), 0, None);
        self.nav_item(ui, Page::ReportTime, Some(3), 0, None);
        self.nav_item(ui, Page::Statistics, Some(4), 0, None);
        let problem = self.settings_problem();
        let update = self.update_available();
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(16.0);
            self.nav_item(ui, Page::Settings, None, 0, problem);
            if let Some(v) = &update {
                ui.add_space(8.0);
                self.update_item(ui, v);
            }
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let c = p(ui);
        let now = now_secs();
        let ctx = ui.ctx().clone();
        // A full running-session toolbar cannot share a narrow header with breadcrumbs.
        // Keep one explicit route to the timer controls and reserve space for Compact.
        ui.ctx().data_mut(|d| d.remove::<egui::Rect>(egui::Id::new("topbar-free")));
        window_drag(ui, ui.max_rect().expand2(egui::vec2(32.0, 0.0)), "topbar");
        let reopen = self.rail && !self.narrow;
        if ui.available_width() < 700.0 {
            let running = if self.page != Page::Today { self.store.db.running().cloned() } else { None };
            ui.horizontal_centered(|ui| {
                let trailing = if running.is_some() { 160.0 } else { CTRL_H + ROW_GAP };
                let title_w = (ui.available_width() - trailing).max(24.0);
                let r = ui.add_sized([title_w, CTRL_H], egui::Label::new(RichText::new(tr(self.page.name())).size(13.0)).truncate().halign(egui::Align::Min));
                self.geometry.rows.push(("topbar · title".into(), r.rect));
                r.on_hover_text(tr(self.page.name()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let r = icon_button(ui, Glyph::Compact, "Compact window");
                    self.geometry.rows.push(("topbar · compact".into(), r.rect));
                    if r.clicked() { self.enter_compact(&ctx); }
                    if let Some(e) = running {
                        let text = clock(e.duration(now));
                        let r = ui.add_sized([104.0, CTRL_H], egui::Button::new(mono_m(text, 16.0)).stroke(Stroke::new(1.0, c.border)).corner_radius(RADIUS)).on_hover_text(tr("Open Today"));
                        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr("Open Today")));
                        self.geometry.rows.push(("topbar · running session".into(), r.rect));
                        if r.clicked() { self.nav_to(Page::Today); }
                    }
                });
            });
            return;
        }
        // Root pages are named by their heading and the selected sidebar item; the top bar
        // carries only the running session and the compact-window control.
        ui.horizontal_centered(|ui| {
            if reopen {
                // first thing in the bar, on the left edge the page's content starts from
                let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
                self.sidebar_toggle(ui, rect.translate(egui::vec2(-6.0, 0.0)));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, Glyph::Compact, "Compact window").clicked() {
                    self.enter_compact(&ctx);
                }
                ui.add_space(8.0);
                if self.page == Page::Today {
                    // Today has no session controls here: all the room is the page's
                    let free = ui.available_rect_before_wrap();
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("topbar-free"), free));
                    return;
                }
                if let Some(e) = self.store.db.running().cloned() {
                    if primary(ui, "Stop").clicked() {
                        self.store.stop();
                    }
                    if secondary(ui, "Pause").clicked() {
                        self.store.pause();
                    }
                    // the same start-time popover as on Today, from any page
                    let t = today::running_timer(ui, &e, now, 16.0);
                    if t.clicked() {
                        today::open_run_time(self, ui, &e, t.rect);
                    }
                    today::run_time_popover(self, ui, &e, &t);
                    let what = truncate(&session_title(&self.store.db, &e), 34);
                    if ui.add(egui::Label::new(RichText::new(what).font(medium(14.0))).sense(Sense::click())).on_hover_text(tr("Open Today")).clicked() {
                        self.go(Page::Today);
                    }
                    dot(ui, c.green, 3.5);
                } else if let Some(e) = self.store.paused().cloned() {
                    if secondary(ui, "Resume").clicked() {
                        self.store.resume();
                    }
                    ui.label(RichText::new(trf_m!("Paused · {}", truncate(&session_title(&self.store.db, &e), 30))).size(13.0).color(c.text2));
                } else if secondary(ui, "+ Add past time").on_hover_text(tr("Add a session that already happened")).clicked() {
                    // the same dialog as on Today, over the page you are on: you stay where you were
                    self.remember_focus(ui.ctx());
                    let day = today_date().format("%Y-%m-%d").to_string();
                    self.today.past = today::Past { date: day.clone(), end_date: day, ..today::Past::default() };
                    self.today.past_open = true;
                }
                // Inside a project or a client the page is titled by the record, so the bar
                // names the section it belongs to; a click returns to the section's first level.
                let inside_record = self.page == Page::Activities && (self.act.selected.is_some() || self.act.client_open.is_some());
                if !inside_record {
                    // what is left of the bar belongs to the page's title and actions
                    let free = ui.available_rect_before_wrap();
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("topbar-free"), free));
                }
                if inside_record {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let r = ui.add(egui::Label::new(RichText::new(tr(self.page.name())).font(medium(14.0))).sense(Sense::click())).on_hover_text(tr("Back to the list"));
                        if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        self.geometry.rows.push(("topbar · section".into(), r.rect));
                        if r.clicked() {
                            let tab = self.act.tab;
                            self.act.open_root(tab);
                            self.fade_content();
                        }
                    });
                }
            });
        });
    }

    fn notices(&mut self, ctx: &egui::Context) {
        let c = pal(self.dark());
        if let Some(reason) = self.store.blocked.clone() {
            egui::TopBottomPanel::top("recovery")
                .frame(egui::Frame::new().fill(c.red.linear_multiply(0.10)).inner_margin(egui::Margin::symmetric(32, 12)).stroke(Stroke::new(1.0, c.line)))
                .show(ctx, |ui| {
                    ui.label(RichText::new(tr("Your saved data couldn't be opened. Nothing has been overwritten.")).font(medium(14.0)).color(c.red));
                    ui.label(&reason);
                    let prev_ok = matches!(self.store.status, OpenStatus::Damaged { previous_ok: true, .. });
                    ui.horizontal(|ui| {
                        if prev_ok && primary(ui, "Restore the previous copy").clicked() {
                            match self.store.restore_previous() {
                                Ok(()) => self.notify(tr("Previous copy restored")),
                                Err(e) => self.notify(e),
                            }
                        }
                        if secondary(ui, "Try again").clicked() {
                            self.store.retry_open();
                        }
                        if matches!(self.store.status, OpenStatus::Damaged { .. }) && secondary(ui, "Start fresh (the damaged copy is kept)").clicked() {
                            self.store.start_fresh();
                        }
                    });
                });
        } else if let Some(err) = self.store.save_error.clone() {
            egui::TopBottomPanel::top("save-error")
                .frame(egui::Frame::new().fill(c.amber.linear_multiply(0.10)).inner_margin(egui::Margin::symmetric(32, 10)).stroke(Stroke::new(1.0, c.line)))
                .show(ctx, |ui| {
                    row(ui, CTRL_H, |ui| {
                        ui.label(RichText::new(tr("⚠ The last change wasn't saved.")).font(medium(14.0)).color(c.amber));
                        ui.label(err);
                        if secondary(ui, "Try saving again").clicked() {
                            self.store.save();
                        }
                    });
                });
        }
    }

    fn toast_area(&mut self, ctx: &egui::Context) {
        let c = pal(self.dark());
        if let Some(t) = &self.toast {
            if t.at.elapsed() > Duration::from_secs(if t.undo { 8 } else { 4 }) {
                self.toast = None;
            } else {
                let (msg, undo) = (t.msg.clone(), t.undo);
                let mut do_undo = false;
                egui::Area::new(egui::Id::new("toast"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        egui::Frame::new().fill(c.primary).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 9)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(msg).color(c.on_primary));
                                if undo && ui.add(egui::Button::new(RichText::new(tr("Undo")).font(medium(14.0)).color(c.on_primary)).frame(false)).clicked() {
                                    do_undo = true;
                                }
                            });
                        });
                    });
                if do_undo {
                    if self.store.undo_remove() {
                        self.notify(tr("Restored"));
                    } else {
                        self.toast = None;
                    }
                }
            }
        }
    }

    fn evidence(&mut self, ctx: &egui::Context) {
        // demonstration transition for `--transition-log`: navigate once, record every frame
        if let Some(log) = &mut self.transition_log {
            if self.frame == 30 && log.started.is_none() {
                log.started = Some(Instant::now());
                let target = if self.page == Page::ReportTime { Page::Activities } else { Page::ReportTime };
                // inline go() to keep the borrow simple
                self.page = target;
                if self.store.db.settings.reduce_motion {
                    self.fade = None;
                } else {
                    self.fade = Some(Fade { start: Instant::now(), from: 0.0 });
                }
            }
            if let (Some(st), false) = (log.started, log.written) {
                let ms = st.elapsed().as_secs_f32() * 1000.0;
                let op = self.fade.map(|f| f.opacity(Instant::now())).unwrap_or(1.0);
                log.frames.push((ms, op));
                if ms > FADE_MS + 120.0 {
                    let first_full = log.frames.iter().find(|(_, o)| *o >= 0.999).map(|(m, _)| *m).unwrap_or(0.0);
                    let mut out = format!(
                        "# DevTrack page transition log\n# reduce_motion={}\n# first frame at full opacity: {:.1} ms (target {} ms)\nelapsed_ms,opacity\n",
                        self.store.db.settings.reduce_motion, first_full, FADE_MS
                    );
                    for (m, o) in &log.frames {
                        out.push_str(&format!("{:.1},{:.3}\n", m, o));
                    }
                    let _ = std::fs::write(&log.path, out);
                    log.written = true;
                }
            }
        }
        if let Some(path) = &self.geometry_report {
            if self.frame == 110 && !self.geometry.rows.is_empty() {
                let mut out = String::from("{\n  \"unit\": \"logical points\",\n");
                out.push_str(&format!("  \"pixels_per_point\": {},\n  \"controls\": [\n", ctx.pixels_per_point()));
                let rows: Vec<String> = self
                    .geometry
                    .rows
                    .iter()
                    .map(|(n, r)| format!("    {{\"name\": \"{}\", \"left\": {:.1}, \"top\": {:.1}, \"width\": {:.1}, \"height\": {:.1}, \"bottom\": {:.1}}}", n, r.left(), r.top(), r.width(), r.height(), r.bottom()))
                    .collect();
                out.push_str(&rows.join(",\n"));
                out.push_str("\n  ]\n}\n");
                let _ = std::fs::write(path, out);
            }
        }
    }
}

/// Review pictures: a pointer resting over the project bar on Today (`--page project-bar`).
pub static SHOT_POINTER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if SHOT_POINTER.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(at) = self.today.shot_pointer.get() {
                raw_input.events.push(egui::Event::PointerMoved(at));
            }
        }
    }

    /// Fully clear: the compact editor's own window is see-through so its rounded frame is its
    /// shape. Every other window is covered edge to edge by its panels.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        self.poll_update(ctx);
        // interface language: applied every frame, so a change never resets timers or drafts
        i18n::set_lang(i18n::LangPref::parse(&self.store.local.language).resolve());
        self.system_dark = ctx.system_theme().map(|t| t == egui::Theme::Dark);
        // tooltips wrap inside the window: egui's 500 px is wider than the compact window, and a
        // long one (a cut line shown whole) ran off its edge
        let tip = (ctx.screen_rect().width() - 32.0).clamp(160.0, 500.0);
        if ctx.style().spacing.tooltip_width != tip {
            ctx.style_mut(|s| s.spacing.tooltip_width = tip);
        }
        let dark = self.dark();
        // the window is see-through (for the compact editor's own window): its whole area gets the
        // app's background first, under the panels, so no hairline between panels shows through
        ctx.layer_painter(egui::LayerId::background()).rect_filled(ctx.screen_rect(), 0.0, pal(dark).canvas);
        if self.theme != Some(dark) {
            apply_theme(ctx, dark);
            // the window itself too: macOS draws its edge from the window's appearance, a light
            // grey line around a dark app when it is not told the app is dark
            let follow = self.store.db.settings.follow_system_theme;
            let theme = if follow { egui::SystemTheme::SystemDefault } else if dark { egui::SystemTheme::Dark } else { egui::SystemTheme::Light };
            ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(theme));
            self.theme = Some(dark);
        }
        // the compact window floats above the others when the person asked for it
        let on_top = self.mode == WinMode::Compact && self.store.local.compact_on_top;
        if self.on_top != Some(on_top) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal }));
            self.on_top = Some(on_top);
        }
        self.narrow = self.mode == WinMode::Full && ctx.screen_rect().width() < 760.0;
        self.rail = self.narrow || (self.mode == WinMode::Full && self.store.local.sidebar_collapsed);
        self.maybe_sync();
        if std::mem::take(&mut self.preview_period_picker) {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("preview-open-period-picker"), true));
        }
        // the assigned work items: read again when the task field (Today, the compact window) or
        // a session dialog shows them
        assigned::poll(self);
        if self.mode == WinMode::Compact || self.page == Page::Today || self.today.edit.is_some() || self.today.past_open {
            assigned::refresh(self, false);
        }
        if let Some(out) = self.jobs.poll() {
            self.handle_job(out);
        }
        self.watch_time();
        #[cfg(target_os = "macos")]
        self.menu_bar_item(ctx);
        self.shortcuts(ctx);
        self.geometry.rows.clear();
        let c = pal(dark);

        if self.mode == WinMode::Compact {
            compact::show(self, ctx);
        } else {
            egui::SidePanel::left("nav")
                .exact_width(if self.rail { RAIL_W } else { SIDEBAR_W })
                .resizable(false)
                .frame(egui::Frame::new().fill(c.nav).inner_margin(egui::Margin::symmetric(10, 0)).stroke(Stroke::new(1.0, c.line)))
                .show(ctx, |ui| self.sidebar(ui));
            egui::TopBottomPanel::top("topbar")
                .exact_height(TOPBAR_H)
                .frame(egui::Frame::new().fill(c.canvas).inner_margin(egui::Margin::symmetric(if self.narrow { 16 } else { 32 }, 0)).stroke(Stroke::new(1.0, c.line)))
                .show(ctx, |ui| self.top_bar(ui));
            self.notices(ctx);
            let now = Instant::now();
            let opacity = match self.fade {
                Some(f) if !self.store.db.settings.reduce_motion => {
                    if f.done(now) {
                        self.fade = None;
                        1.0
                    } else {
                        ctx.request_repaint();
                        f.opacity(now)
                    }
                }
                _ => {
                    self.fade = None;
                    1.0
                }
            };
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(c.canvas).inner_margin(if self.narrow { egui::Margin::symmetric(16, 16) } else { egui::Margin::symmetric(32, 24) }))
                .show(ctx, |ui| {
                    // only the destination page is drawn and hit-tested; it fades in
                    ui.set_opacity(opacity);
                    let scroll_page = self.narrow || ctx.screen_rect().height() < 680.0;
                    let mut draw = |ui: &mut egui::Ui| match self.page {
                        Page::Today => today::page(self, ui),
                        Page::Activities => activities::page(self, ui),
                        Page::ReportTime => report_time::page(self, ui),
                        Page::Statistics => stats::page(self, ui),
                        Page::Settings => settings::page(self, ui),
                        Page::Gallery => gallery::page(self, ui),
                    };
                    if scroll_page {
                        let list = egui::ScrollArea::vertical().id_salt("page-scroll").auto_shrink([false, false]).show(ui, |ui| draw(ui));
                        scroll_fades(ui, &list, p(ui).canvas);
                    } else { draw(ui); }
                });
        }

        today::edit_modal(self, ctx);
        today::past_modal(self, ctx);
        dest::modal(self, ctx);
        activities::move_modal(self, ctx);
        activities::add_task_modal(self, ctx);
        activities::project_modal(self, ctx);
        activities::client_modal(self, ctx);
        self.leave_modal(ctx);
        self.idle_modal(ctx);
        self.toast_area(ctx);
        self.evidence(ctx);

        if self.store.db.running().is_some() || self.jobs.busy() || self.toast.is_some() || self.transition_log.is_some() {
            ctx.request_repaint_after(Duration::from_millis(if self.transition_log.is_some() { 1 } else { 250 }));
        } else {
            ctx.request_repaint_after(Duration::from_secs(5));
        }

        if let Some(shot) = &mut self.shot {
            ctx.request_repaint();
            shot.frames += 1;
            if shot.frames >= 120 && !shot.requested {
                shot.requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let mut done = false;
            ctx.input(|i| {
                for ev in &i.raw.events {
                    if let egui::Event::Screenshot { image, .. } = ev {
                        let [w, h] = image.size;
                        let mut buf = Vec::with_capacity(w * h * 4);
                        for px in &image.pixels {
                            buf.extend_from_slice(&px.to_srgba_unmultiplied());
                        }
                        if let Some(parent) = shot.path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match image::save_buffer(&shot.path, &buf, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                            Ok(_) => eprintln!("saved {}", shot.path.display()),
                            Err(e) => eprintln!("could not save screenshot: {}", e),
                        }
                        done = true;
                    }
                }
            });
            if done {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

/// Page header: heading and actions on one row, subtitle 8 units below, 24 before the next section.
/// The page's title, subtitle and actions. In a wide window they live in the top bar, in the
/// room it leaves free, so the page itself starts with its content; in a narrow window the top
/// bar already names the page and the actions get a row of their own.
pub fn page_header(ui: &mut egui::Ui, title_text: &str, subtitle: &str, actions: impl FnOnce(&mut egui::Ui)) {
    let free = ui.ctx().data(|d| d.get_temp::<egui::Rect>(egui::Id::new("topbar-free"))).filter(|r| r.width() >= 320.0);
    let Some(free) = free else {
        // narrow: no big title (the bar has it); one row with the subtitle on the left and the
        // actions on the right (in the same order as in the top bar), then the content
        ui.horizontal(|ui| {
            ui.set_min_height(CTRL_H);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = ROW_GAP;
                actions(ui);
                if !subtitle.is_empty() {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let c = p(ui).text2;
                        let t = tr(subtitle);
                        ui.add(egui::Label::new(RichText::new(&t).size(13.0).color(c)).truncate()).on_hover_text(&t);
                    });
                }
            });
        });
        ui.add_space(GROUP_GAP - ui.spacing().item_spacing.y);
        return;
    };
    let c = p(ui);
    let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(free).layout(egui::Layout::right_to_left(egui::Align::Center)));
    bar.set_clip_rect(free.expand(1.0));
    bar.spacing_mut().item_spacing.x = ROW_GAP;
    actions(&mut bar);
    bar.add_space(GROUP_GAP - ROW_GAP);
    bar.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.add(egui::Label::new(RichText::new(tr(title_text)).font(semibold(17.0))).extend());
        if !subtitle.is_empty() {
            let t = tr(subtitle);
            ui.add(egui::Label::new(RichText::new(&t).size(13.0).color(c.text2)).truncate()).on_hover_text(&t);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_takes_200_ms_with_ease_out() {
        let start = Instant::now();
        let f = Fade { start, from: 0.0 };
        assert!(f.opacity(start) < 0.01);
        let half = f.opacity(start + Duration::from_millis(100));
        assert!((half - 0.875).abs() < 0.01); // 1-(1-0.5)^3
        assert!(f.opacity(start + Duration::from_millis(200)) >= 0.999);
        assert!(f.done(start + Duration::from_millis(200)));
        assert!(!f.done(start + Duration::from_millis(180)));
        // rapid navigation continues from the current opacity, never back to zero
        let g = Fade { start, from: 0.6 };
        assert!(g.opacity(start) >= 0.6);
    }

    #[test]
    fn only_two_quick_clicks_in_one_place_zoom_the_window() {
        let p = egui::pos2(100.0, 10.0);
        assert!(is_double_click(Some((1.0, p)), 1.3, p));
        assert!(!is_double_click(Some((1.0, p)), 1.6, p));
        assert!(!is_double_click(Some((1.0, p)), 1.2, egui::pos2(130.0, 10.0)));
        assert!(!is_double_click(None, 1.2, p));
    }

    #[test]
    fn compact_window_stays_on_screen() {
        let m = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0)));
        let p = App::clamp_to_screen(egui::pos2(1300.0, 800.0), egui::vec2(430.0, 720.0), m);
        assert_eq!(p, egui::pos2(1010.0, 180.0));
        let p2 = App::clamp_to_screen(egui::pos2(-50.0, -20.0), egui::vec2(430.0, 720.0), m);
        assert_eq!(p2, egui::pos2(0.0, 0.0));
        // a second screen to the right: the window stays on it, not pulled back onto the first
        let right = Some(egui::Rect::from_min_size(egui::pos2(1440.0, 0.0), egui::vec2(1920.0, 1080.0)));
        let p3 = App::clamp_to_screen(egui::pos2(1600.0, 100.0), egui::vec2(1360.0, 860.0), right);
        assert_eq!(p3, egui::pos2(1600.0, 100.0));
        let p4 = App::clamp_to_screen(egui::pos2(3000.0, 100.0), egui::vec2(1360.0, 860.0), right);
        assert_eq!(p4, egui::pos2(2000.0, 100.0));
    }
    #[test]
    fn task_search_show_and_add_never_overlap() {
        for width in [264.0, 344.0, 500.0, 740.0, 1140.0] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let mut app = App::new(Store::in_memory(devtrack_core::sample::build_state(now_secs(), devtrack_core::sample::DemoState::Incomplete, true)), None);
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 1800.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| activities::page(&mut app, ui));
            });
            let controls: Vec<_> = app.geometry.rows.iter().filter(|(name, _)| name.starts_with("tasks toolbar")).collect();
            assert_eq!(controls.len(), 3);
            for (_, rect) in &controls { assert!(rect.left() >= -0.5 && rect.right() <= width + 0.5, "{width}: {rect:?}"); }
            for (i, (_, a)) in controls.iter().enumerate() {
                for (_, b) in &controls[i + 1..] { assert!(!a.shrink(0.5).intersects(*b), "{width}: {a:?} overlaps {b:?}"); }
            }
        }
    }

    #[test]
    fn project_filters_and_create_action_never_overlap() {
        for width in [264.0, 344.0, 500.0, 740.0, 1140.0] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let mut app = App::new(Store::in_memory(devtrack_core::sample::build_state(now_secs(), devtrack_core::sample::DemoState::Incomplete, true)), None);
            app.act.open_root(activities::ActTab::Projects);
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 1800.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| activities::page(&mut app, ui));
            });
            let controls: Vec<_> = app.geometry.rows.iter().filter(|(name, _)| name.starts_with("activities toolbar")).collect();
            assert_eq!(controls.len(), 4);
            for (_, rect) in &controls { assert!(rect.left() >= -0.5 && rect.right() <= width + 0.5, "{width}: {rect:?}"); }
            for (i, (_, a)) in controls.iter().enumerate() {
                for (_, b) in &controls[i + 1..] { assert!(!a.shrink(0.5).intersects(*b), "{width}: {a:?} overlaps {b:?}"); }
            }
        }
    }

    #[test]
    fn running_header_remains_usable_on_small_windows() {
        for width in [232.0, 264.0, 344.0, 520.0] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let mut app = App::new(Store::in_memory(devtrack_core::sample::build_state(now_secs(), devtrack_core::sample::DemoState::Incomplete, true)), None);
            app.page = Page::Activities;
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 56.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| app.top_bar(ui));
            });
            assert_eq!(app.geometry.rows.len(), 3);
            for (_, rect) in &app.geometry.rows { assert!(rect.left() >= -0.5 && rect.right() <= width + 0.5, "{width}: {rect:?}"); }
            for (i, (_, a)) in app.geometry.rows.iter().enumerate() {
                for (_, b) in &app.geometry.rows[i + 1..] { assert!(!a.shrink(0.5).intersects(*b), "{width}: {a:?} overlaps {b:?}"); }
            }
        }
    }

}
