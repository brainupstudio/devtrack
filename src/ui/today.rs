//! Today: current session, a summary band and a full-width work ledger (list or timeline).
//! Editing happens in one dialog with a review flow;
//! past time is added from its own dialog and never touches the running session.

use super::i18n::{long_date, month, plural, tr, trf, weekday_title};
use super::i18n::trf_m;
use super::widgets::*;
use super::{page_header, App, Page};
use chrono::{Datelike, Duration as CDur, NaiveDate, Timelike};
use devtrack_core::edit::{self, RunningFields, TimeFields};
use devtrack_core::model::*;
use devtrack_core::report::{self, day_pieces, entries_in, flags, local_date, local_midnight, local_time_of, needs_completion, today, FlagKind, Period, PeriodKind};
use devtrack_core::store::{now_secs, validate};
use devtrack_core::timefmt::{clock, format_duration, parse_clock, parse_duration};
use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use std::collections::BTreeMap;


#[derive(Default, Clone)]
pub struct Draft {
    pub desc: String,
    pub project: Option<Id>,
    pub task: Option<Id>,
    pub billable: bool,
    /// An Azure DevOps work item chosen from the suggestions: it becomes a task on Start.
    pub remote: Option<String>,
}

#[derive(Clone)]
pub struct Past {
    pub desc: String,
    pub project: Option<Id>,
    pub task: Option<Id>,
    pub billable: bool,
    pub date: String,
    pub start: String,
    pub end_date: String,
    pub end: String,
    /// Typed duration; it wins over the end while `dur_edited`.
    pub dur: String,
    pub dur_edited: bool,
    pub errors: BTreeMap<&'static str, String>,
    /// The client shown: the project's, or one chosen to narrow the projects.
    pub client: Option<Id>,
}

impl Past {
    pub fn has_input(&self) -> bool {
        !self.desc.trim().is_empty() || self.project.is_some() || self.task.is_some()
    }
}

impl Default for Past {
    fn default() -> Self {
        Past { desc: String::new(), project: None, task: None, billable: false, date: today().format("%Y-%m-%d").to_string(), start: "09:00".into(), end_date: today().format("%Y-%m-%d").to_string(), end: "10:00".into(), dur: "1:00:00".into(), dur_edited: false, errors: BTreeMap::new(), client: None }
    }
}

pub struct TodayState {
    pub draft: Draft,
    pub past: Past,
    pub past_open: bool,
    pub past_focused: bool,
    pub period: Period,
    pub timeline: bool,
    /// The weekly grid (project × day) instead of the list or the calendar.
    pub timesheet: bool,
    /// Projects added to the grid as empty rows, until they have time.
    pub sheet_rows: Vec<Option<Id>>,
    /// The cell to put the cursor in once (a row just added: its first day to fill).
    pub sheet_focus: Option<(Option<Id>, NaiveDate)>,
    /// Review pictures: open the task menu of the session dialog once.
    pub preview_task_menu: bool,
    pub only_incomplete: bool,
    pub edit: Option<Edit>,
    pub run_desc: Option<(Id, String)>,
    /// Put the cursor in the description field on the next frame (menu bar "New session").
    pub focus_desc: bool,
    /// Where the period bar's "Today" button ends (last frame), for the timer row to line up.
    /// The views (List · Calendar · Timesheet) are on the summary row: the period bar had no room.
    pub views_in_summary: bool,
    /// The session last opened in the editor stays highlighted in the ledger.
    pub highlight: Option<Id>,
    /// Running session's start being edited in its popover: (entry, fields, fields as opened, error).
    pub run_time: Option<(Id, RunningFields, RunningFields, Option<String>)>,
    /// Open the running session's start popover on the next frame (review pictures).
    pub run_time_request: bool,
    /// Review pictures: open the conversions of the first row.
    pub conv_request: bool,
}

impl TodayState {
    pub fn new(period: Period) -> Self {
        TodayState { draft: Draft::default(), past: Past::default(), past_open: false, past_focused: false, period, timeline: false, timesheet: false, sheet_rows: vec![], sheet_focus: None, preview_task_menu: false, only_incomplete: false, edit: None, run_desc: None, highlight: None, run_time: None, run_time_request: false, conv_request: false, focus_desc: false, views_in_summary: false }
    }
}

#[derive(Clone)]
pub struct Edit {
    pub entry: TimeEntry,
    /// The time fields as typed; `orig` as first shown. Unchanged fields keep exact seconds.
    pub f: TimeFields,
    orig: TimeFields,
    pub errors: BTreeMap<&'static str, String>,
    pub attempted: bool,
    pub confirm_delete: bool,
    pub confirm_discard: bool,
    /// The result is longer than 24 hours: shown in full, saved only on a second Save.
    pub confirm_long: Option<(i64, i64)>,
    #[allow(dead_code)]
    pub focused_once: bool,
    pub focus_field: Option<&'static str>,
    original: TimeEntry,
    /// The client shown: the project's, or one chosen to narrow the projects.
    pub client: Option<Id>,
}

impl Edit {
    pub fn dirty(&self) -> bool {
        let o = &self.original;
        o.description != self.entry.description || o.project_id != self.entry.project_id || o.task_id != self.entry.task_id || o.billable != self.entry.billable || self.f != self.orig
    }
}

/// The fields as first shown (for review pictures).
pub fn orig_fields(ed: &Edit) -> TimeFields {
    ed.orig.clone()
}

fn time_fields(e: &TimeEntry) -> TimeFields {
    let end = e.stop.unwrap_or(e.start);
    TimeFields {
        start_date: local_date(e.start).format("%Y-%m-%d").to_string(),
        start: clock_text(e.start, true),
        end_date: local_date(end).format("%Y-%m-%d").to_string(),
        end: e.stop.map(|s| clock_text(s, true)).unwrap_or_default(),
        duration: format_duration(e.duration(now_secs()), DurationFormat::Improved),
        duration_edited: false,
    }
}

pub fn open_edit(app: &mut App, e: &TimeEntry, ctx: Option<&egui::Context>, _review: bool) {
    if let Some(ctx) = ctx {
        app.remember_focus(ctx);
    }
    let f = time_fields(e);
    app.today.highlight = Some(e.id.clone());
    app.today.edit = Some(Edit {
        entry: e.clone(),
        original: e.clone(),
        orig: f.clone(),
        f,
        errors: BTreeMap::new(),
        attempted: false,
        confirm_delete: false,
        confirm_discard: false,
        confirm_long: None,
        focused_once: false,
        focus_field: None,
        client: client_of(&app.store.db, &e.project_id),
    });
}

/// The client of a project, when it still exists.
fn client_of(db: &Db, project: &Option<Id>) -> Option<Id> {
    db.project(project).and_then(|p| p.client_id.clone()).filter(|c| db.client(&Some(c.clone())).is_some())
}

/// What a session was, in reading order: the task across the whole width, then Project and
/// Client side by side. The client belongs to the project, so it follows the project; chosen
/// first, it narrows the projects to its own (and picks the only one, if it has one). A client
/// can be created from its list by typing a name that isn't there.
#[allow(clippy::too_many_arguments)]
fn work_fields(app: &mut App, ui: &mut egui::Ui, salt: &str, w: f32, project: &mut Option<Id>, task: &mut Option<Id>, client: &mut Option<Id>, project_error: Option<&str>) {
    let rp = recent_projects(&app.store.db);
    let rt = recent_tasks(&app.store.db);
    let remote = RemoteSuggest::of(app);
    if std::mem::take(&mut app.today.preview_task_menu) {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("preview-open-task-picker"), true));
    }
    let b = field(ui, "Task", None, None, |ui| task_picker(ui, (salt, "task"), &app.store.db, remote.as_ref(), &rt, project, task, w));
    ui.add_space(ROW_GAP);
    let half = if w < 460.0 { w } else { (w - ROW_GAP) / 2.0 };
    let mut chosen = client.clone();
    let mut a = Pick::None;
    let mut c = Pick::None;
    ui.horizontal_wrapped(|ui| {
        a = field(ui, "Project", None, project_error, |ui| project_picker_in(ui, (salt, "project"), &app.store.db, &rp, project, Trigger::Field(half), chosen.as_ref()));
        if w < 460.0 { ui.end_row(); }
        c = field(ui, "Client", None, None, |ui| client_select(ui, (salt, "client"), &app.store.db, &mut chosen, half));
    });
    settle_work(app, b, a, c, project, Some(task), &mut chosen);
    *client = chosen;
}

/// What picking a task, a project or a client means for the other two: the client follows the
/// project; a client chosen (or created) first narrows the projects to its own and picks the
/// only one; a project created from a client's list belongs to it. True when anything changed.
fn settle_work(app: &mut App, task_pick: Pick, project_pick: Pick, client_pick: Pick, project: &mut Option<Id>, mut task: Option<&mut Option<Id>>, chosen: &mut Option<Id>) -> bool {
    let client_changed = match client_pick {
        Pick::Changed => true,
        Pick::Create(name) => {
            let cl = Client { id: new_id(), name: name.trim().to_string(), ..Default::default() };
            *chosen = Some(cl.id.clone());
            app.store.put_client(cl);
            app.notify(trf("Client “{}” created", &[&truncate(name.trim(), 40)]));
            true
        }
        _ => false,
    };
    let created = matches!(project_pick, Pick::Create(_));
    let task_changed = match task.as_deref_mut() {
        Some(t) => picked(app, task_pick, project, Some(t)),
        None => false,
    };
    let project_changed = picked(app, project_pick, project, None);
    if created {
        // created from a client's list: it belongs to that client
        if let (Some(pid), Some(cid)) = (project.clone(), chosen.clone()) {
            if let Some(mut pr) = app.store.db.projects.get(&pid).cloned() {
                pr.client = app.store.db.client(&Some(cid.clone())).map(|c| c.name.clone()).unwrap_or_default();
                pr.client_id = Some(cid);
                app.store.put_project(pr);
            }
        }
    }
    let mut changed = task_changed || project_changed;
    if client_changed {
        let db = &app.store.db;
        if client_of(db, project) != *chosen {
            let own: Vec<&Project> = chosen.as_ref().map(|c| db.projects_of_client(c).into_iter().filter(|p| !p.archived).collect()).unwrap_or_default();
            *project = if own.len() == 1 { Some(own[0].id.clone()) } else { None };
            // a task of another project no longer fits
            if let Some(t) = task {
                if db.task(t).map(|x| x.project_id.is_some() && x.project_id != *project).unwrap_or(false) {
                    *t = None;
                }
            }
            changed = true;
        }
    } else if changed && project.is_some() {
        *chosen = client_of(&app.store.db, project);
    }
    changed
}

pub(super) fn picked(app: &mut App, pick: Pick, project: &mut Option<Id>, task: Option<&mut Option<Id>>) -> bool {
    match pick {
        Pick::Changed => true,
        Pick::Create(name) => {
            match task {
                Some(t) => *t = Some(app.create_task(&name, project.clone())),
                None => *project = Some(app.create_project(&name)),
            }
            true
        }
        // a work item from Azure DevOps: it becomes a task now, in the session's project (or the
        // one its neighbours went to), and is the session's task
        Pick::Remote(n) => {
            let dest = project.clone().or_else(|| app.assigned.get(&n).and_then(|i| super::assigned::likely_project(&app.store.db, &i.project)));
            match (super::assigned::import(app, &n, dest), task) {
                (Some(tid), Some(t)) => {
                    if let Some(pid) = app.store.db.task(&Some(tid.clone())).and_then(|x| x.project_id.clone()) {
                        *project = Some(pid);
                    }
                    *t = Some(tid);
                    true
                }
                _ => false,
            }
        }
        Pick::LookUp(n) => {
            super::assigned::look_up(app, &n);
            false
        }
        Pick::None => false,
    }
}

pub fn missing_text(e: &TimeEntry) -> &'static str {
    match (e.task_id.is_none() && e.description.trim().is_empty(), e.project_id.is_none()) {
        (true, true) => "Needs project and task",
        (true, false) => "Needs a task",
        (false, true) => "Needs a project",
        _ => "",
    }
}

/// Incomplete, finished sessions in a period, oldest first.
pub fn incomplete_in(app: &App, period: &Period) -> Vec<TimeEntry> {
    let mut v: Vec<TimeEntry> = entries_in(&app.store.db, period, now_secs()).into_iter().filter(|e| needs_completion(e) && !e.is_running()).cloned().collect();
    v.sort_by_key(|e| e.start);
    v
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let now = now_secs();
    let period = app.today.period;
    let (title_txt, sub) = match period.kind {
        PeriodKind::Day if period.start == today() => (tr("Today"), long_date(period.start)),
        PeriodKind::Day => (weekday_title(period.start), long_date(period.start)),
        PeriodKind::Week => (tr("Week"), format!("{} – {}", long_date(period.start), long_date(period.end - CDur::days(1)))),
        PeriodKind::Month => (tr("Month"), format!("{} {}", month(period.start.month()), period.start.year())),
    };
    let mut open_past = false;
    page_header(ui, &title_txt, &sub, |ui| {
        if secondary(ui, "+ Add past time").on_hover_text(tr("Add a session that already happened")).clicked() {
            open_past = true;
        }
    });
    if open_past {
        app.remember_focus(ui.ctx());
        let day = period.start.format("%Y-%m-%d").to_string();
        app.today.past = Past { date: day.clone(), end_date: day, ..Past::default() };
        app.today.past_open = true;
    }
    // the current session stays on top whatever period is browsed, so the toolbar below never moves
    session_panel(app, ui);
    favorites_row(app, ui);
    ui.add_space(24.0 - ui.spacing().item_spacing.y);

    // the period toolbar: fixed positions; count and List/Timeline in its reserved status slot
    let monday = app.store.db.settings.week_starts_monday;
    let mut per = period;
    // three ways to see the same sessions: list, calendar, weekly grid
    let mut view = if app.today.timesheet { 2 } else if app.today.timeline { 1 } else { 0 };
    // List · Calendar · Timesheet say how the sessions below are shown: right of the period bar
    // when it fits, otherwise right on the summary row, just above what they change
    let views_w = segmented_w(ui, &VIEWS);
    let views_here = super::period::status_fits(ui, views_w);
    app.today.views_in_summary = !views_here;
    let bar = super::period::period_toolbar(ui, &mut per, monday, [true, true, true], if views_here { views_w } else { 0.0 }, |ui| {
        if let Some(i) = segmented(ui, &VIEWS, view) {
            view = i;
        }
    });
    super::period::record(app, "today", &bar);
    app.today.timeline = view == 1;
    app.today.timesheet = view == 2;
    // the grid is a week: choosing it, or being on it, keeps the period a week
    if app.today.timesheet && per.kind != PeriodKind::Week {
        per = Period::containing(PeriodKind::Week, per.start, monday);
    }
    if per != period {
        app.today.period = per;
    }
    let period = app.today.period;
    ui.add_space(GROUP_GAP - ROW_GAP);
    let entries: Vec<TimeEntry> = entries_in(&app.store.db, &period, now).into_iter().cloned().collect();
    let incomplete = incomplete_in(app, &period);
    if app.store.db.live_entries().next().is_none() {
        ui.add_space(16.0);
        first_use(app, ui);
        return;
    }
    ui.add_space(12.0);
    ledger_summary(app, ui, &entries, &period, &incomplete);
    ui.add_space(8.0);
    project_bar(app, ui, &entries, &period);
    let shown: Vec<TimeEntry> = entries.iter().filter(|e| !e.is_running() && (!app.today.only_incomplete || needs_completion(e))).cloned().collect();
    if app.today.timesheet {
        egui::ScrollArea::both().id_salt("timesheet").auto_shrink([false, false]).show(ui, |ui| {
            timesheet(app, ui, &entries, &period);
        });
    } else if app.today.timeline && period.kind != PeriodKind::Month {
        timeline(app, ui, &entries, &period);
    } else {
        let list = egui::ScrollArea::vertical().id_salt("ledger").auto_shrink([false, false]).show(ui, |ui| {
            ledger(app, ui, &shown, &period);

        });
        scroll_fades(ui, &list, p(ui).canvas);
    }
}

/// One layout for idle and running, so starting a session moves nothing:
/// description on top, project and task under it, the time and its action on the right.
fn session_panel(app: &mut App, ui: &mut egui::Ui) {
    let now = now_secs();
    let c = p(ui);
    let h24 = app.store.db.settings.clock_24h;
    let running = app.store.db.running().cloned();
    let paused = app.store.paused().cloned();
    let rp = recent_projects(&app.store.db);
    // 12 of padding, not the 20 of a content panel: this is a toolbar that stays on screen
    let frame = egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).inner_margin(egui::Margin::same(12));
    frame.show(ui, |ui| ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(ui.available_width());
        if running.is_none() {
            if let Some(pe) = &paused {
                row(ui, CTRL_H, |ui| {
                    dot(ui, c.amber, 3.5);
                    ui.label(RichText::new(tr("Paused")).size(13.0).color(c.amber));
                    ui.label(RichText::new(truncate(&super::session_title(&app.store.db, &pe), 40)).size(13.0));
                    mixed(ui, &words(pe.duration(now)), 12.0, Some(c.text2));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if secondary(ui, "Resume").on_hover_text(tr("Continue it as a new piece (⌘/Ctrl+P)")).clicked() {
                            app.store.resume();
                        }
                    });
                });
                ui.add_space(ROW_GAP);
            }
        }
        let w = ui.max_rect().width();
        // reserved for the running controls in both states, so the description never moves
        let actions_w = BTN_MIN_W * 2.0 + CTRL_H + ROW_GAP * 2.0;
        // fits a dot and "00:00:00" in Mono 28
        let timer_w = 168.0;
        // one line while the description keeps at least 260: a 1024 window still gets it
        let narrow = w < 260.0 + timer_w + actions_w + GROUP_GAP * 2.0;
        let desc_w = if narrow { w } else { w - timer_w - actions_w - GROUP_GAP * 2.0 };
        // line 1: description · time · action. The description keeps the same width idle and
        // running; the controls are right-aligned in the space left of it.
        let mut start = false;
        // line 1: the task takes the row; the time and the actions keep their place on the right
        if narrow {
            start |= description(app, ui, running.as_ref(), w);
            ui.add_space(ROW_GAP);
            ui.horizontal(|ui| {
                ui.set_min_height(DESC_H);
                let buttons = if running.is_some() { actions_w } else { 104.0 };
                session_controls(app, ui, running.as_ref(), now, (w - buttons - ROW_GAP).max(timer_w), false);
            });
        } else {
            ui.horizontal(|ui| {
                ui.set_min_height(DESC_H);
                ui.spacing_mut().item_spacing.x = GROUP_GAP;
                start |= description(app, ui, running.as_ref(), desc_w);
                let content = timer_w + ROW_GAP + if running.is_some() { actions_w } else { 104.0 };
                ui.add_space((ui.available_width() - content).max(0.0));
                session_controls(app, ui, running.as_ref(), now, timer_w, true);
            });
        }
        // line 2, under the task's text: its project and client as underlined words that open
        // the same menus as the session window, and while it runs, when it started
        let mut proj = running.as_ref().map(|e| e.project_id.clone()).unwrap_or_else(|| app.today.draft.project.clone());
        let salt = if running.is_some() { "run" } else { "draft" };
        let client_key = egui::Id::new(("timer-client", salt));
        // the client shown: the project's, or one chosen to narrow the projects
        let mut chosen = match (ui.data(|d| d.get_temp::<Option<Id>>(client_key)), proj.is_some()) {
            (Some(c), false) => c,
            _ => client_of(&app.store.db, &proj),
        };
        let mut started_link = None;
        let mut a = Pick::None;
        let mut cl = Pick::None;
        ui.add_space(6.0);
        row(ui, 20.0, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(PAD_X);
            a = project_picker_in(ui, (salt, "project"), &app.store.db, &rp, &mut proj, Trigger::Link, chosen.as_ref());
            app.geometry.rows.push(("start row · project".into(), ui.min_rect()));
            ui.label(RichText::new("  ·  ").size(13.0).color(c.text2));
            cl = client_select_with(ui, (salt, "client"), &app.store.db, &mut chosen, Trigger::Link);
            // a work item from Azure DevOps with no project: say it, in words and amber
            let from_devops = match &running {
                Some(e) => app.store.db.task(&e.task_id).is_some_and(|t| t.source == SOURCE_AZURE),
                None => app.today.draft.remote.is_some(),
            };
            if from_devops && proj.is_none() {
                ui.add_space(GROUP_GAP);
                ui.label(RichText::new(tr("! Choose a project for this work item")).size(13.0).color(c.amber));
            }
            if let Some(e) = &running {
                ui.add_space(GROUP_GAP + 8.0);
                started_link = Some(link_btn(ui, &trf("Started {}", &[&clock_text(e.start, h24)])).on_hover_text(tr("Change when it started")));
            }
        });
        let changed = settle_work(app, Pick::None, a, cl, &mut proj, None, &mut chosen);
        ui.data_mut(|d| d.insert_temp(client_key, chosen));
        let task = running.as_ref().map(|e| e.task_id.clone()).unwrap_or_else(|| app.today.draft.task.clone());
        if let Some(r) = started_link {
            // the shortcut and the menu open it here too, under the link that names the start
            if r.clicked() || std::mem::take(&mut app.today.run_time_request) { if let Some(e) = &running { open_run_time(app, ui, e, r.rect); } }
        }
        if changed {
            if running.is_some() {
                app.store.update_running(|x| { x.project_id = proj.clone(); x.task_id = task.clone(); });
            } else {
                if proj != app.today.draft.project {
                    app.today.draft.billable = app.store.db.project(&proj).map(|p| p.billable_default).unwrap_or(false);
                }
                app.today.draft.project = proj;
                app.today.draft.task = task;
            }
        }
        if start && running.is_none() {
            app.start_draft();
        }
    }));
}

// The description is a normal 36 control: the panel is one compact row when there is room.
const VIEWS: [&str; 3] = ["List", "Timeline", "Timesheet"];
const DESC_H: f32 = CTRL_H;

/// What a description suggestion fills in.
#[derive(Clone)]
pub enum Suggest {
    /// An existing task: choosing it fills the task and its project (and so the client).
    Task(Id),
    Project(Id),
    /// An Azure DevOps work item not yet in DevTrack (its number).
    Remote(String),
    /// Look a #number up in Azure DevOps.
    LookUp(String),
    /// Read the assigned work items again (offered when the list is old or unreachable).
    Refresh,
}

/// What Azure DevOps adds to the suggestions: work items not yet tasks, a lookup in progress,
/// and how current the list is.
#[derive(Default)]
pub struct RemoteSuggest {
    pub items: Vec<super::assigned::Item>,
    pub lookup: Option<(String, super::assigned::Lookup)>,
    pub status: Option<String>,
}

impl RemoteSuggest {
    pub fn of(app: &App) -> Option<RemoteSuggest> {
        super::assigned::connection(app)?;
        Some(RemoteSuggest { items: super::assigned::candidates(app), lookup: app.assigned.lookup.clone(), status: app.assigned.status(now_secs()) })
    }
}

/// Tasks, then Azure DevOps work items not yet in DevTrack, then projects, matching the text.
pub(super) fn suggestions(db: &Db, q: &str, remote: Option<&RemoteSuggest>) -> Vec<Suggest> {
    let raw = q.trim().to_string();
    let q = q.trim().trim_start_matches('#').to_lowercase();
    if q.is_empty() {
        return vec![];
    }
    // open tasks whose title, number, project or client match; the most recently used first
    let mut last_used: BTreeMap<&str, i64> = BTreeMap::new();
    for e in db.live_entries() {
        if let Some(t) = e.task_id.as_deref() {
            let v = last_used.entry(t).or_insert(0);
            *v = (*v).max(e.start);
        }
    }
    let mut tasks: Vec<&Task> = db
        .live_tasks()
        .filter(|t| !t.done)
        .filter(|t| {
            let (pname, client) = db.project(&t.project_id).map(|p| (p.name.clone(), db.client_name(p))).unwrap_or_default();
            format!("{} {} {} {}", t.title, t.external_id.clone().unwrap_or_default(), pname, client).to_lowercase().contains(&q)
        })
        .collect();
    tasks.sort_by_key(|t| (std::cmp::Reverse(last_used.get(t.id.as_str()).copied().unwrap_or(0)), t.title.to_lowercase()));
    let number_known = tasks.iter().any(|t| t.external_id.as_deref() == Some(q.as_str()));
    let mut out: Vec<Suggest> = tasks.into_iter().take(8).map(|t| Suggest::Task(t.id.clone())).collect();
    if let Some(r) = remote {
        let found: Vec<&super::assigned::Item> = r.items.iter().filter(|i| search_matches(&raw, &[&i.title, &i.id, &i.project])).take(5).collect();
        let number_found = found.iter().any(|i| i.id == q);
        out.extend(found.into_iter().map(|i| Suggest::Remote(i.id.clone())));
        // a number nobody here knows: offer to look it up (or say how the lookup went)
        if q.chars().all(|ch| ch.is_ascii_digit()) && !number_known && !number_found {
            out.push(Suggest::LookUp(q.clone()));
        }
        if r.status.is_some() && out.iter().any(|s| matches!(s, Suggest::Remote(..) | Suggest::LookUp(..))) {
            out.push(Suggest::Refresh);
        }
    }
    let mut projects: Vec<&Project> = db.live_projects().into_iter().filter(|p| !p.archived && format!("{} {}", p.name, db.client_name(p)).to_lowercase().contains(&q)).collect();
    projects.sort_by_key(|p| p.name.to_lowercase());
    out.extend(projects.into_iter().take(4).map(|p| Suggest::Project(p.id.clone())));
    out
}

/// The description field: large, framed, with suggestions from earlier work while typing.
/// Returns true when Enter asks to start (idle only, no suggestion highlighted).
fn description(app: &mut App, ui: &mut egui::Ui, running: Option<&TimeEntry>, width: f32) -> bool {
    let c = p(ui);
    // the field names the task: the chosen one's title, or what is typed for a new one
    let mut text = match running {
        Some(e) => match &app.today.run_desc {
            Some((id, t)) if *id == e.id => t.clone(),
            _ => app.store.db.task(&e.task_id).map(|t| t.display()).unwrap_or_else(|| e.description.clone()),
        },
        None => app.today.draft.task.as_ref().and(app.store.db.task(&app.today.draft.task)).map(|t| t.display()).unwrap_or_else(|| app.today.draft.desc.clone()),
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, DESC_H), Sense::hover());
    app.geometry.rows.push(("start row · description".into(), rect));
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Rect::from_min_max(rect.min + egui::vec2(PAD_X, 1.0), rect.max - egui::vec2(PAD_X, 1.0));
    let te = egui::TextEdit::singleline(&mut text)
        .id_salt("session-description")
        .frame(false)
        .font(sans(14.0))
        .margin(egui::Margin::ZERO)
        .desired_width(inner.width())
        .vertical_align(egui::Align::Center)
        .hint_text(RichText::new(tr("What are you working on? (task)")).font(sans(14.0)).color(c.text2));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let r = child.add_sized(inner.size(), te);
    if std::mem::take(&mut app.today.focus_desc) {
        r.request_focus();
    }
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, tr("Description")));
    let stroke = if r.has_focus() { Stroke::new(2.0, c.focus) } else if r.hovered() { Stroke::new(1.0, c.text2) } else { Stroke::new(1.0, c.border) };
    ui.painter().set(bg, egui::Shape::rect_filled(rect, RADIUS, c.field));
    ui.painter().rect_stroke(rect, RADIUS, stroke, egui::StrokeKind::Inside);

    // keep what is typed; a suggestion chosen below replaces it
    match running {
        Some(e) => {
            if r.changed() { app.today.run_desc = Some((e.id.clone(), text.clone())); }
            let current = app.store.db.task(&e.task_id).map(|t| t.display()).unwrap_or_else(|| e.description.clone());
            if r.lost_focus() && text.trim() != current.trim() {
                // the name typed is the task: an existing one of the project, or a new one
                let name = text.trim().to_string();
                if !name.is_empty() {
                    let tid = app.task_named(e.project_id.clone(), &name);
                    app.store.update_running(|x| x.task_id = Some(tid.clone()));
                }
                app.today.run_desc = None;
            }
        }
        None => {
            if r.changed() {
                // typing another name leaves the chosen task: a new one is made on start
                if app.store.db.task(&app.today.draft.task).is_some_and(|t| t.display() != text) {
                    app.today.draft.task = None;
                }
                // the same for a work item chosen from Azure DevOps
                if let Some(n) = app.today.draft.remote.clone() {
                    if app.assigned.get(&n).map(|i| i.display()) != Some(text.clone()) {
                        app.today.draft.remote = None;
                    }
                }
            }
            app.today.draft.desc = text.clone();
        }
    }
    suggest_under(app, ui, &r, rect, &text, running, width)
}

/// Suggestions under a description field: earlier sessions and projects matching the text.
/// Choosing one fills the draft (or the running session). Returns true when Enter asks to
/// start (no session running, nothing highlighted). Shared by Today and the compact window.
pub fn suggest_under(app: &mut App, ui: &mut egui::Ui, r: &egui::Response, rect: egui::Rect, text: &str, running: Option<&TimeEntry>, width: f32) -> bool {
    let text = text.to_string();
    let remote = RemoteSuggest::of(app);
    let (choice, enter_start) = suggest_pick(&app.store.db, remote.as_ref(), ui, "session-suggest", r, rect, &text, running.is_none(), width);
    if let Some(s) = choice {
        match (s, running) {
            (Suggest::LookUp(n), _) => {
                super::assigned::look_up(app, &n);
                r.request_focus();
            }
            (Suggest::Refresh, _) => {
                super::assigned::refresh(app, true);
                r.request_focus();
            }
            (Suggest::Remote(n), Some(e)) => {
                // while a session runs the choice is made now: the work item becomes its task
                let dest = e.project_id.clone().or_else(|| app.assigned.get(&n).and_then(|i| super::assigned::likely_project(&app.store.db, &i.project)));
                if let Some(tid) = super::assigned::import(app, &n, dest) {
                    let pid = app.store.db.task(&Some(tid.clone())).and_then(|t| t.project_id.clone());
                    app.store.update_running(|x| { x.task_id = Some(tid.clone()); if pid.is_some() { x.project_id = pid.clone(); } });
                }
                app.today.run_desc = None;
            }
            (Suggest::Remote(n), None) => {
                // nothing is imported yet: the draft names the work item and its likely project
                let item = app.assigned.get(&n).cloned();
                if app.today.draft.project.is_none() {
                    app.today.draft.project = item.as_ref().and_then(|i| super::assigned::likely_project(&app.store.db, &i.project));
                    app.today.draft.billable = app.store.db.project(&app.today.draft.project).map(|p| p.billable_default).unwrap_or(false);
                }
                app.today.draft.desc = item.map(|i| i.display()).unwrap_or_default();
                app.today.draft.task = None;
                app.today.draft.remote = Some(n);
            }
            (Suggest::Task(tid), Some(_)) => {
                let pid = app.store.db.task(&Some(tid.clone())).and_then(|t| t.project_id.clone());
                app.store.update_running(|x| { x.task_id = Some(tid.clone()); if pid.is_some() { x.project_id = pid.clone(); } });
                app.today.run_desc = None;
            }
            (Suggest::Project(id), Some(_)) => {
                // the project only: what is typed stays, and becomes the task on leaving the field
                app.store.update_running(|x| { x.project_id = Some(id.clone()); if x.task_id.is_some() { x.task_id = None; } });
            }
            (Suggest::Task(tid), None) => {
                let t = app.store.db.task(&Some(tid.clone())).cloned();
                let pid = t.as_ref().and_then(|t| t.project_id.clone());
                app.today.draft.billable = app.store.db.project(&pid).map(|p| p.billable_default).unwrap_or(false);
                app.today.draft.desc = t.map(|t| t.display()).unwrap_or_default();
                app.today.draft.project = pid;
                app.today.draft.task = Some(tid);
            }
            (Suggest::Project(id), None) => {
                // the project only: what is typed stays as the task's name
                app.today.draft.billable = app.store.db.project(&Some(id.clone())).map(|p| p.billable_default).unwrap_or(false);
                if app.store.db.task(&app.today.draft.task).map(|t| t.project_id.as_deref() != Some(id.as_str())).unwrap_or(false) {
                    app.today.draft.task = None;
                }
                app.today.draft.project = Some(id);
            }
        }
    }
    enter_start
}

/// One way to fill a session from its description, wherever a description is typed (the timer,
/// the edit and add-past dialogs): a previous session that matches fills description, project
/// (and so its client) and task at once; a matching project fills the project. `salt` keeps two
/// fields' lists apart. Returns the choice, and whether Enter asked to start (`can_start`, with
/// nothing highlighted).
pub fn suggest_pick(db: &Db, remote: Option<&RemoteSuggest>, ui: &mut egui::Ui, salt: &str, r: &egui::Response, rect: egui::Rect, text: &str, can_start: bool, width: f32) -> (Option<Suggest>, bool) {
    let c = p(ui);
    let text = text.to_string();
    let pop = egui::Id::new(salt);
    let hl_id = pop.with("highlight");
    let dismissed_id = pop.with("dismissed");
    if r.changed() {
        ui.data_mut(|d| { d.remove::<String>(dismissed_id); d.insert_temp(hl_id, None::<usize>); });
    }
    let dismissed = ui.data(|d| d.get_temp::<String>(dismissed_id)).is_some_and(|t| t == text);
    let over = ui.data(|d| d.get_temp::<egui::Rect>(pop)).zip(ui.ctx().pointer_latest_pos()).is_some_and(|(pr, pos)| pr.contains(pos));
    let list = if dismissed || !(r.has_focus() || r.lost_focus() || over) { vec![] } else { suggestions(db, &text, remote) };
    let mut hl: Option<usize> = ui.data(|d| d.get_temp(hl_id)).flatten();
    let mut choice: Option<Suggest> = None;
    let mut enter_start = false;
    if !list.is_empty() && r.has_focus() {
        let (down, up, esc) = ui.input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Escape)));
        if down { hl = Some(hl.map(|h| (h + 1).min(list.len() - 1)).unwrap_or(0)); }
        if up { hl = hl.and_then(|h| h.checked_sub(1)); }
        if esc { ui.data_mut(|d| d.insert_temp(dismissed_id, text.clone())); }
    }
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if enter {
        match hl.and_then(|h| list.get(h)) {
            Some(s) => choice = Some(s.clone()),
            None => enter_start = can_start,
        }
    }
    if !list.is_empty() && choice.is_none() {
        // above a dialog too: the list belongs to the field it opens from
        let area = egui::Area::new(pop).order(egui::Order::Tooltip).interactable(true).fixed_pos(rect.left_bottom() + egui::vec2(0.0, 4.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).inner_margin(egui::Margin::same(6)).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) }).show(ui, |ui| {
                ui.set_width(width - 12.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                let mut header: Option<u8> = None;
                for (i, s) in list.iter().enumerate() {
                    let group = match s { Suggest::Task(..) => 0u8, Suggest::Remote(..) | Suggest::LookUp(..) | Suggest::Refresh => 1, Suggest::Project(..) => 2 };
                    if header != Some(group) {
                        if header.is_some() { ui.add_space(4.0); }
                        let (hr, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), Sense::hover());
                        let name = match group { 0 => tr("Tasks"), 1 => tr("From Azure DevOps · not in DevTrack yet"), _ => tr("Projects") };
                        ui.painter().text(hr.left_center() + egui::vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, name, sans(12.0), c.text2);
                        header = Some(group);
                    }
                    let picked = match s {
                        Suggest::Task(tid) => {
                            let t = db.task(&Some(tid.clone()));
                            let pr = t.and_then(|t| db.project(&t.project_id));
                            let mut sub: Vec<String> = vec![];
                            if let Some(pr) = pr { sub.push(pr.name.clone()); let cl = db.client_name(pr); if !cl.is_empty() { sub.push(cl); } }
                            pick_row(ui, hl == Some(i), pr.map(|p| hex(&p.color)), &t.map(|t| t.display()).unwrap_or_default(), &sub.join(" · "), false)
                        }
                        Suggest::Project(id) => {
                            let pr = db.project(&Some(id.clone()));
                            pick_row(ui, hl == Some(i), pr.map(|p| hex(&p.color)), &pr.map(|p| p.name.clone()).unwrap_or_default(), &pr.map(|p| db.client_name(p)).unwrap_or_default(), false)
                        }
                        Suggest::Remote(n) => {
                            let it = remote.and_then(|r| r.items.iter().find(|x| &x.id == n));
                            let dest = it.and_then(|it| super::assigned::likely_project(db, &it.project)).and_then(|p| db.project(&Some(p)).cloned());
                            let mut sub = vec!["Azure DevOps".to_string()];
                            if let Some(it) = it {
                                if !it.project.is_empty() { sub.push(it.project.clone()); }
                                if !it.status.is_empty() { sub.push(it.status.clone()); }
                            }
                            sub.push(match &dest { Some(p) => trf_m!("goes to “{}”", truncate(&p.name, 28)), None => tr("no project yet") });
                            pick_row(ui, hl == Some(i), dest.as_ref().map(|p| hex(&p.color)), &it.map(|i| i.display()).unwrap_or_default(), &sub.join(" · "), false)
                        }
                        Suggest::LookUp(n) => {
                            use super::assigned::Lookup;
                            let state = remote.and_then(|r| r.lookup.as_ref()).filter(|(x, _)| x == n).map(|(_, st)| st.clone());
                            let (title, sub) = match &state {
                                Some(Lookup::Searching) => (trf_m!("Looking up #{}…", n), tr("Reading Azure DevOps")),
                                Some(Lookup::NotFound) => (trf_m!("#{} isn't in Azure DevOps", n), tr("Or you can't see it with this connection")),
                                Some(Lookup::Failed(m)) => (trf_m!("#{} couldn't be read", n), m.clone()),
                                _ => (trf_m!("Look up #{} in Azure DevOps", n), tr("Not assigned to you, or not read yet")),
                            };
                            let p = pick_row(ui, hl == Some(i), None, &title, &sub, false);
                            // only the offer acts: a lookup under way or done says where it stands
                            p && !matches!(state, Some(Lookup::Searching) | Some(Lookup::NotFound))
                        }
                        Suggest::Refresh => {
                            // how current the list is, said in words (and amber), with the way to fix it
                            let st = remote.and_then(|r| r.status.clone()).unwrap_or_default();
                            pick_row(ui, hl == Some(i), Some(c.amber), &st, &tr("Read the work items assigned to you again"), false)
                        }
                    };
                    if picked { choice = Some(s.clone()); }
                }
            });
        });
        ui.data_mut(|d| d.insert_temp(pop, area.response.rect));
    } else {
        ui.data_mut(|d| d.remove::<egui::Rect>(pop));
    }
    ui.data_mut(|d| d.insert_temp(hl_id, hl));

    if let Some(s) = &choice {
        // the list closes on what was chosen and stays closed until the text changes again
        let shown = match s {
            Suggest::Task(id) => db.task(&Some(id.clone())).map(|t| t.display()).unwrap_or_default(),
            Suggest::Remote(n) => remote.and_then(|r| r.items.iter().find(|x| &x.id == n)).map(|i| i.display()).unwrap_or_default(),
            // a lookup keeps the list open: its result shows in it
            Suggest::LookUp(_) | Suggest::Refresh => String::from("\u{0}"),
            Suggest::Project(_) => text.clone(),
        };
        ui.data_mut(|d| { d.remove::<egui::Rect>(pop); d.insert_temp(hl_id, None::<usize>); d.insert_temp(dismissed_id, shown); });
    }
    (choice, enter_start)
}

/// Time so far and the session's action: Start when idle; Pause, Stop and more while running.
fn session_controls(app: &mut App, ui: &mut egui::Ui, running: Option<&TimeEntry>, now: i64, timer_w: f32, hug_right: bool) {
    let c = p(ui);
    ui.spacing_mut().item_spacing.x = ROW_GAP;
    let layout = if hug_right { egui::Layout::right_to_left(egui::Align::Center) } else { egui::Layout::left_to_right(egui::Align::Center) };
    ui.allocate_ui_with_layout(egui::vec2(timer_w, DESC_H), layout, |ui| {
        ui.set_min_size(egui::vec2(timer_w, DESC_H));
        ui.spacing_mut().item_spacing.x = 10.0;
        match running {
            Some(e) => {
                if !hug_right { dot(ui, c.green, 4.0); }
                let t = running_timer(ui, e, now, 28.0);
                if t.clicked() {
                    open_run_time(app, ui, e, t.rect);
                }
                run_time_popover(app, ui, e, &t);
                if hug_right { dot(ui, c.green, 4.0); }
            }
            None => {
                ui.label(mono_m("0:00:00", 28.0).color(c.text2));
            }
        }
    });
    match running {
        Some(e) => {
            if button_w(ui, Kind::Secondary, "Pause", BTN_MIN_W, true).on_hover_text(tr("Stop now; Resume continues it as a new piece (⌘/Ctrl+P)")).clicked() {
                app.store.pause();
            }
            if button_w(ui, Kind::Primary, "Stop", BTN_MIN_W, true).on_hover_text(tr("Stop and save (⌘/Ctrl+Enter)")).clicked() {
                app.store.stop();
                app.notify(tr("Session saved"));
            }
            let more = icon_button(ui, Glyph::More, "More actions");
            let pid = ui.make_persistent_id("run-more");
            if more.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pid));
            }
            let mut edit = false;
            egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(200.0);
                if menu_row(ui, "Edit details…", false, false).clicked() {
                    edit = true;
                }
                if menu_row(ui, "Discard session", false, false).clicked() {
                    app.store.discard_running();
                    app.notify_undo(tr("Session discarded"));
                }
            });
            if edit {
                let ctx = ui.ctx().clone();
                open_edit(app, e, Some(&ctx), false);
            }
        }
        None => {
            let r = button_w(ui, Kind::Primary, "Start", 104.0, true).on_hover_text(tr("⌘/Ctrl+Enter"));
            app.geometry.rows.push(("start row · Start".into(), r.rect));
            if r.clicked() {
                app.start_draft();
            }
        }
    }
}

/// The live timer, clickable: opens the start-time popover.
pub fn running_timer(ui: &mut egui::Ui, e: &TimeEntry, now: i64, size: f32) -> egui::Response {
    let c = p(ui);
    let r = ui.add(egui::Label::new(mono_m(clock(e.duration(now)), size)).wrap_mode(egui::TextWrapMode::Extend).sense(Sense::click()));
    if r.hovered() || r.has_focus() {
        ui.painter().rect_stroke(r.rect.expand2(egui::vec2(8.0, 2.0)), RADIUS, Stroke::new(1.0, if r.has_focus() { c.focus } else { c.line }), egui::StrokeKind::Outside);
    }
    let name = tr("Running time: change the start");
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone()));
    r.on_hover_text(tr("Change when it started"))
}

fn run_fields(e: &TimeEntry, now: i64) -> RunningFields {
    RunningFields { start_date: local_date(e.start).format("%Y-%m-%d").to_string(), start: clock_text(e.start, true), elapsed: clock(e.duration(now)), elapsed_edited: false }
}

pub fn open_run_time(app: &mut App, ui: &egui::Ui, e: &TimeEntry, from: egui::Rect) {
    // remembered so the popover opens under the control that was clicked, not always the timer
    ui.data_mut(|d| d.insert_temp(egui::Id::new("run-time-anchor"), from));
    let f = run_fields(e, now_secs());
    app.today.run_time = Some((e.id.clone(), f.clone(), f, None));
    ui.memory_mut(|m| m.open_popup(egui::Id::new("run-time-popover")));
}

/// Start ▸ now, start date and time so far, editable while the session keeps running.
/// Enter or Apply saves; a click outside or Cancel leaves it unchanged.
pub fn run_time_popover(app: &mut App, ui: &mut egui::Ui, e: &TimeEntry, anchor: &egui::Response) {
    let pid = egui::Id::new("run-time-popover");
    if !ui.memory(|m| m.is_popup_open(pid)) {
        if app.today.run_time.as_ref().map(|r| r.0 == e.id).unwrap_or(false) {
            app.today.run_time = None;
        }
        return;
    }
    let Some((id, mut f, orig, mut err)) = app.today.run_time.clone() else { return };
    if id != e.id {
        return;
    }
    let now = now_secs();
    let h24 = app.store.db.settings.clock_24h;
    let mut apply = false;
    let mut cancel = false;
    // own area, left-aligned under what opened it and kept on screen
    let w = 300.0;
    let outer_w = w + 24.0;
    let screen = ui.ctx().screen_rect();
    let from = ui.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("run-time-anchor"))).unwrap_or(anchor.rect);
    let x = from.left().clamp(screen.left() + 8.0, (screen.right() - outer_w - 8.0).max(screen.left() + 8.0));
    let pos = egui::pos2(x, from.bottom() + 6.0);
    let pal_ = p(ui);
    let area = egui::Area::new(pid).order(egui::Order::Foreground).fixed_pos(pos).constrain(true).show(ui.ctx(), |ui| {
      egui::Frame::new().fill(pal_.raised).stroke(Stroke::new(1.0, pal_.line)).corner_radius(10.0).inner_margin(egui::Margin::same(12)).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) }).show(ui, |ui| {
        let c = p(ui);
        ui.set_width(w);
        ui.add_space(4.0);
        let half = (w - 24.0 - ROW_GAP * 2.0) / 2.0;
        ui.horizontal_top(|ui| {
            field(ui, "Start time", None, None, |ui| {
                let r = time_input(ui, &mut f.start, "Start time", half, err.as_deref().map(|_| true).unwrap_or(false) && !f.elapsed_edited);
                if r.changed() {
                    f.elapsed_edited = false;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    apply = true;
                }
            });
            field(ui, " ", None, None, |ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, CTRL_H), Sense::hover());
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "→", sans(16.0), c.text2);
            });
            // not a field: the end is "now" and can't be typed
            field(ui, "End", None, None, |ui| {
                row(ui, CTRL_H, |ui| {
                    ui.set_width(half);
                    mixed(ui, &format!("{} · {}", clock_text(now, h24), tr("now")), 14.0, Some(c.text2));
                });
            });
        });
        ui.add_space(GROUP_GAP - ROW_GAP);
        field(ui, "Start date", None, None, |ui| {
            let r = date_input(ui, &mut f.start_date, "Start date", w, false, app.store.db.settings.week_starts_monday).on_hover_text("YYYY-MM-DD");
            if r.changed() {
                f.elapsed_edited = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                apply = true;
            }
        });
        ui.add_space(GROUP_GAP - ROW_GAP);
        field(ui, "Time so far", None, None, |ui| {
            let r = input(ui, &mut f.elapsed, Field::new("Time so far", w).mono().hint("1:30 · 1.5 · 90")).on_hover_text(tr("Typing it moves the start; the session keeps running"));
            if r.changed() {
                f.elapsed_edited = true;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                apply = true;
            }
        });
        if let Some(m) = &err {
            ui.label(RichText::new(m).size(12.0).color(c.red));
        }
        ui.add_space(ROW_GAP);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary(ui, "Apply").clicked() {
                    apply = true;
                }
                if secondary(ui, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });
      });
    });
    // a click outside (not on the timer itself) or Escape closes without saving
    let clicked_outside = ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map(|p| !area.response.rect.contains(p) && !anchor.rect.contains(p) && !from.contains(p)).unwrap_or(false));
    if clicked_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if apply {
        match edit::resolve_running(&f, &orig, e.start, now) {
            Ok(start) => {
                if start != e.start {
                    app.store.update_running(|x| x.start = start);
                    app.notify(trf("Started at {}", &[&clock_text(start, h24)]));
                }
                app.today.run_time = None;
                ui.memory_mut(|m| m.close_popup());
                return;
            }
            Err((_, m)) => err = Some(tr(&m)),
        }
    }
    if cancel {
        app.today.run_time = None;
        ui.memory_mut(|m| m.close_popup());
        return;
    }
    app.today.run_time = Some((id, f, orig, err));
}

fn first_use(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(8.0);
    col(ui, 520.0f32.min(ui.available_width()), 0.0, |ui| {
        panel(ui, |ui| {
            section(ui, "Nothing recorded yet");
            muted(ui, "Type what you're working on above and press Start. Project and task can wait: sessions without details show up for review.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if secondary(ui, "Connect Azure DevOps").clicked() {
                    app.settings.tab = super::settings::Tab::Connections;
                    app.return_to = Some(super::ReturnTo { project: None });
                    app.go(Page::Settings);
                }
                if app.store.is_persistent() && ghost(ui, "Load sample data").clicked() {
                    app.store.load_sample();
                }
            });
        });
    });
}

/// One line above the ledger: sessions and tracked total for the period, then the only
/// attention signal on Today (sessions that need details) with its filter.
fn ledger_summary(app: &mut App, ui: &mut egui::Ui, entries: &[TimeEntry], period: &Period, incomplete: &[TimeEntry]) {
    let c = p(ui);
    let now = now_secs();
    let (pa, pb) = period.bounds();
    let tracked: i64 = entries.iter().map(|e| report::overlap(e, pa, pb, now)).sum();
    // the count and the total cover the same sessions, the running one included
    let done = entries.len() as i64;
    if app.today.timeline && period.kind == PeriodKind::Month {
        meta(ui, "The timeline shows a day or a week; a month is shown as a list.");
    }
    // nothing recorded: the empty state below says it, so no "0 sessions · 0 s" line; the
    // views, when they live here, still get their row
    if done == 0 && !app.today.only_incomplete {
        if app.today.views_in_summary {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| summary_views(app, ui));
        }
        return;
    }
    // The total on the left, its label on the same baseline; the status of the sessions on the
    // right, as the same box Report Time uses (a button when there is something to do).
    let total = ui.painter().layout_job(mixed_job(&words(tracked), 20.0, c.text, medium(20.0), mono_medium(20.0)));
    let rest = ui.painter().layout_job(mixed_job(&trf_m!("tracked · {}", plural(done, "{} session", "{} sessions")), 14.0, c.text2, sans(14.0), mono_font(14.0)));
    let baseline = |g: &egui::Galley| g.rows.first().and_then(|r| r.glyphs.first()).map(|gl| gl.pos.y).unwrap_or(g.size().y);
    let (b1, b2) = (baseline(&total), baseline(&rest));
    let left_w = total.size().x + 8.0 + rest.size().x;
    let running = entries.iter().any(|e| e.is_running());
    let n = incomplete.len() as i64;
    // every session completed while only those were shown: back to the whole list
    if n == 0 && app.today.only_incomplete {
        app.today.only_incomplete = false;
    }
    // what the box says and what its click does: (text, colour, action)
    let (text, color, action): (String, Color32, Option<(&str, Glyph)>) = if app.today.only_incomplete && !app.today.timeline {
        (format!("! {}", plural(n, "Showing {} session that needs details", "Showing {} sessions that need details")), c.amber, Some(("Show all sessions", Glyph::Close)))
    } else if n > 0 {
        (format!("! {}", plural(n, "{} session needs details", "{} sessions need details")), c.amber, if app.today.timeline { None } else { Some(("Show only these", Glyph::Next)) })
    } else if running {
        ("● A session is still running".to_string(), c.text2, None)
    } else {
        ("✔ All sessions completed".to_string(), c.green, None)
    };
    let chip_w = status_chip_w(ui, &text, action.is_some());
    // with the views here: total · status on the left, views on the right
    let views = app.today.views_in_summary;
    let views_w = if views { segmented_w(ui, &VIEWS) } else { 0.0 };
    let w = ui.available_width();
    let (chip_beside, views_beside) = if views {
        let all = left_w + GROUP_GAP + chip_w + GROUP_GAP + views_w <= w;
        (all, all || left_w + GROUP_GAP + views_w <= w)
    } else {
        (left_w + GROUP_GAP + chip_w <= w, false)
    };
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.set_min_height(CTRL_H);
        let h = total.size().y.max(b1 + rest.size().y - b2);
        let (r, _) = ui.allocate_exact_size(egui::vec2(left_w, h), Sense::hover());
        let top = r.top();
        ui.painter().galley(egui::pos2(r.left(), top), total, c.text);
        ui.painter().galley(egui::pos2(r.left() + left_w - rest.size().x, top + b1 - b2), rest, c.text2);
        if views && views_beside {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                summary_views(app, ui);
                if chip_beside {
                    ui.add_space(GROUP_GAP - ui.spacing().item_spacing.x);
                    clicked = status_chip_glyph(ui, &text, color, action).clicked();
                }
            });
        } else if !views && chip_beside {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                clicked = status_chip_glyph(ui, &text, color, action).clicked();
            });
        }
    });
    if views && !views_beside {
        ui.add_space(ROW_GAP);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| summary_views(app, ui));
    }
    if !chip_beside {
        ui.add_space(ROW_GAP);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            clicked = status_chip_glyph(ui, &text, color, action).clicked();
        });
    }
    if clicked {
        app.today.only_incomplete = !app.today.only_incomplete;
    }
}

/// List · Calendar · Timesheet on the summary row, when the period bar has no room for them.
fn summary_views(app: &mut App, ui: &mut egui::Ui) {
    let view = if app.today.timesheet { 2 } else if app.today.timeline { 1 } else { 0 };
    if let Some(i) = segmented(ui, &VIEWS, view) {
        app.today.timeline = i == 1;
        app.today.timesheet = i == 2;
        ui.ctx().request_repaint();
    }
}

fn session_observations(id: &str, flags: &[report::Flag]) -> Vec<String> {
    let mut long = false;
    let mut short = false;
    let mut overlaps = std::collections::BTreeSet::new();
    for flag in flags.iter().filter(|f| f.entry_id == id) {
        match &flag.kind {
            FlagKind::Long => long = true,
            FlagKind::Short => short = true,
            FlagKind::Overlap(other) => { overlaps.insert(other.clone()); },
        }
    }
    let mut out = vec![];
    if long { out.push(tr("over 12 hours")); }
    if short { out.push(tr("under a minute")); }
    if !overlaps.is_empty() { out.push(plural(overlaps.len() as i64, "Overlaps {} other session", "Overlaps {} other sessions")); }
    out
}

fn piece_times(e: &TimeEntry, a: i64, b: i64, h24: bool) -> (String, String) {
    let s = if a > e.start { "00:00".to_string() } else { clock_text(a, h24) };
    let end_txt = if Some(b) != e.stop { "24:00".to_string() } else { clock_text(b, h24) };
    (s, end_txt)
}

/// "Conversions", then one line per platform: its name and this duration as it is written
/// there. With `pick` the lines are buttons and the chosen value comes back, to be copied.
pub fn conversions_card(ui: &mut egui::Ui, profiles: &[devtrack_core::model::TimesheetProfile], secs: i64, pick: bool) -> Option<String> {
    let c = p(ui);
    let mut out = None;
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.label(RichText::new(tr("Conversions")).font(medium(13.0)).color(c.text));
    ui.add_space(4.0);
    let rows: Vec<(String, String)> = profiles.iter().map(|pr| (truncate(&pr.name, 24), devtrack_core::report::convert(pr, secs))).collect();
    let width = |t: &str, f: egui::FontId| ui.painter().layout_no_wrap(t.to_string(), f, c.text).size().x.ceil();
    let name_w = rows.iter().map(|r| width(&r.0, sans(13.0))).fold(0.0_f32, f32::max);
    let val_w = rows.iter().map(|r| width(&r.1, mono_font(13.0))).fold(0.0_f32, f32::max);
    let h = if pick { 28.0 } else { 20.0 };
    // a card that stays open is wide enough for its hint on one line, and no wider
    let row_w = (name_w + GROUP_GAP + val_w).max(if pick { 208.0 } else { 0.0 });
    if pick {
        ui.set_width(row_w);
    }
    for (name, value) in &rows {
        let (r, resp) = ui.allocate_exact_size(egui::vec2(row_w, h), if pick { Sense::click() } else { Sense::hover() });
        if pick {
            let over = resp.hovered() || resp.has_focus();
            // title, names and hint share one left edge: the hover fill grows into the card's
            // margin instead of pushing the row's text inwards
            ui.painter().rect_filled(r.expand2(egui::vec2(5.0, 0.0)), RADIUS, hover_mix(ui, resp.id, over, Color32::TRANSPARENT, c.hover));
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let label = trf_m!("Copy {}", format!("{} {}", name, value));
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
            if resp.clicked() {
                out = Some(value.clone());
            }
        }
        ui.painter().text(egui::pos2(r.left(), r.center().y), egui::Align2::LEFT_CENTER, name, sans(13.0), c.text2);
        ui.painter().text(egui::pos2(r.left() + name_w + GROUP_GAP, r.center().y), egui::Align2::LEFT_CENTER, value, mono_font(13.0), c.text);
    }
    if pick {
        ui.add_space(4.0);
        meta(ui, "Click a value to copy it");
    }
    out
}

fn ledger(app: &mut App, ui: &mut egui::Ui, entries: &[TimeEntry], period: &Period) {
    let now = now_secs();
    let c = p(ui);
    let fmt = app.store.db.settings.duration_format;
    let h24 = app.store.db.settings.clock_24h;
    let fl = flags(&app.store.db, period, now);
    let mut by_day: BTreeMap<NaiveDate, Vec<(TimeEntry, i64, i64)>> = BTreeMap::new();
    for e in entries {
        for (d, a, b) in day_pieces(e, now) {
            if period.contains(d) {
                by_day.entry(d).or_default().push((e.clone(), a, b));
            }
        }
    }
    if by_day.is_empty() {
        let (mut show_all, mut add_past, mut go_back) = (false, false, None);
        if app.today.only_incomplete {
            empty_state(ui, Icon::Today, "Nothing needs details", "Every session in this period has what reporting needs.", |ui| {
                show_all = secondary(ui, "Show all sessions").clicked();
            });
        } else {
            let last = app.last_day_with_sessions(period.start);
            empty_state(ui, Icon::Today, "No completed sessions in this period", "Start the timer above while you work, or add a session that already happened.", |ui| {
                add_past = secondary(ui, "+ Add past time").clicked();
                if let Some(d) = last {
                    if link_btn(ui, "Show the last day with sessions").clicked() { go_back = Some(d); }
                }
            });
        }
        if show_all { app.today.only_incomplete = false; }
        if add_past {
            app.remember_focus(ui.ctx());
            let day = period.start.format("%Y-%m-%d").to_string();
            app.today.past = Past { date: day.clone(), end_date: day, ..Past::default() };
            app.today.past_open = true;
        }
        if let Some(d) = go_back {
            app.today.period = Period::containing(PeriodKind::Day, d, app.store.db.settings.week_starts_monday);
        }
        return;
    }
    let w = ui.available_width();
    let narrow = w < 560.0;
    // the conversions live behind one icon per row, between the duration and Continue: hovering
    // shows them, clicking keeps them open so a value can be copied
    let profiles: Vec<devtrack_core::model::TimesheetProfile> = app.store.db.live_profiles().into_iter().cloned().collect();
    let has_conv = !profiles.is_empty();
    let time_w = if narrow { 0.0 } else if h24 { 124.0 } else { 176.0 };
    let dur_w = if time_w == 0.0 { if h24 { 104.0 } else { 140.0 } } else { 96.0 };
    // column edges, shared by the header, the day bands and the rows
    let time_span = if time_w > 0.0 { time_w + ROW_GAP } else { 0.0 };
    // a narrow row has room for one of the two: Continue stays in the "···" menu
    let play_btn = !(narrow && has_conv);
    let conv_btn = if has_conv && play_btn { CTRL_H + 4.0 } else { 0.0 };
    let main_w = (w - 14.0 - ROW_GAP - time_span - dur_w - 106.0 - conv_btn).max(48.0);
    let x_time = 14.0 + main_w + ROW_GAP;
    // rows and bands touch: no spacing between a row and its divider
    ui.spacing_mut().item_spacing.y = 0.0;
    // header, 36 high
    let (hr, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), Sense::hover());
    ui.painter().rect_filled(hr, TABLE_TOP, c.nav);
    ui.painter().text(egui::pos2(hr.left() + 14.0, hr.center().y), egui::Align2::LEFT_CENTER, tr(if narrow { "Activity" } else { "Activity / project" }), sans(12.0), c.text2);
    if time_w > 0.0 {
        ui.painter().text(egui::pos2(hr.left() + x_time, hr.center().y), egui::Align2::LEFT_CENTER, tr("Time"), sans(12.0), c.text2);
    }
    ui.painter().text(egui::pos2(hr.left() + x_time + time_span, hr.center().y), egui::Align2::LEFT_CENTER, tr("Duration"), sans(12.0), c.text2);
    let mut action: Option<(u8, TimeEntry)> = None;
    let mut copied: Option<String> = None;
    let multi_day = by_day.len() > 1;
    // rows are exactly 64 high: no extra spacing between a row and its divider
    ui.spacing_mut().item_spacing.y = 0.0;
    for (date, mut list) in by_day.into_iter().rev() {
        list.sort_by_key(|x| x.1);
        if multi_day {
            // the day is a band of the table: same inset as the rows, its total in the duration column
            let (dr, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), Sense::hover());
            ui.painter().rect_filled(dr, 0.0, c.nav);
            if dr.top() - hr.bottom() < 1.0 {
                // the first band follows the header: one line keeps the two apart
                ui.painter().rect_filled(egui::Rect::from_min_max(dr.min, egui::pos2(dr.right(), dr.top() + 1.0)), 0.0, c.line);
            }
            let name = ui.painter().text(egui::pos2(dr.left() + 14.0, dr.center().y), egui::Align2::LEFT_CENTER, super::i18n::weekday(date), medium(13.0), c.text);
            ui.painter().text(egui::pos2(name.right() + ROW_GAP, dr.center().y), egui::Align2::LEFT_CENTER, super::i18n::short_date(date), mono_font(12.0), c.text2);
            let total: i64 = list.iter().map(|x| x.2 - x.1).sum();
            // under its column; a narrow list has no room there beside the date, so it goes to the right edge
            let (tx, ta) = if narrow { (dr.right() - 14.0, egui::Align2::RIGHT_CENTER) } else { (dr.left() + x_time + time_span, egui::Align2::LEFT_CENTER) };
            ui.painter().text(egui::pos2(tx, dr.center().y), ta, super::shown_duration(&app.store.db, total), mono_medium(13.0), c.text);
            divider(ui);
        }
        for (e, a, b) in &list {
            let mut action_rect = egui::Rect::NOTHING;
            let mut buttons_left = 0.0_f32;
            let row = ui.horizontal(|ui| {
                ui.set_min_height(64.0);
                ui.add_space(14.0);
                col(ui, main_w, 60.0, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        match app.store.db.project(&e.project_id).map(|p| hex(&p.color)) {
                            Some(col_) => dot(ui, col_, 3.5),
                            None => {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(11.0, 9.0), Sense::hover());
                                ui.painter().circle_stroke(r.center(), 3.5, Stroke::new(1.2, c.amber));
                            }
                        }
                        let title_txt = super::session_title(&app.store.db, e);
                        ui.add(egui::Label::new(RichText::new(title_txt).font(medium(14.0))).truncate());
                    });
                    let observations = session_observations(&e.id, &fl);
                    let miss = missing_text(e);
                    let context = if !miss.is_empty() {
                        tr(miss)
                    } else {
                        let mut parts = vec![super::session_context(&app.store.db, e)];
                        parts.retain(|p| !p.is_empty());
                        if !e.tags.is_empty() { parts.push(e.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")); }
                        parts.join(" · ")
                    };
                    let summary = if observations.is_empty() { context.clone() } else {
                        let count = if observations.len() == 1 { observations[0].clone() } else { plural(observations.len() as i64, "{} observation", "{} observations") };
                        if context.is_empty() { count } else { format!("{} · {}", context, count) }
                    };
                    let full = std::iter::once(context).chain(observations.iter().cloned()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n");
                    // One bounded metadata line: arbitrary warning counts never grow this cell.
                    let rect = ui.available_rect_before_wrap();
                    ui.set_clip_rect(ui.clip_rect().intersect(egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() - 64.0), egui::pos2(rect.left() + main_w, rect.bottom()))));
                    // The whole row opens the editor (with the checks), so no separate Review link.
                    ui.allocate_ui_with_layout(egui::vec2(main_w, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_width(main_w);
                        ui.add(egui::Label::new(RichText::new(summary).size(12.0).color(if miss.is_empty() { c.text2 } else { c.amber })).truncate()).on_hover_text(full);
                    });
                });
                let (from, to) = piece_times(e, *a, *b, h24);
                let note = if *a > e.start { "from the day before" } else if Some(*b) != e.stop { "continues next day" } else { "" };
                if time_w > 0.0 {
                    col(ui, time_w, 60.0, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        // one line sits on the row's middle; with a note it takes the two-line rhythm
                        ui.add_space(if note.is_empty() { 21.0 } else { 12.0 });
                        ui.label(range_job(&from, &to, 13.0, c.text2));
                        if !note.is_empty() {
                            meta(ui, note);
                        }
                    });
                }
                col(ui, dur_w, 60.0, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let part = time_w > 0.0 && b - a != e.duration(now);
                    ui.add_space(if time_w == 0.0 || part { 12.0 } else { 21.0 });
                    ui.label(mono_m(super::shown_duration(&app.store.db, b - a), 14.0));
                    if time_w == 0.0 {
                        ui.label(range_job(&from, &to, 12.0, c.text2));
                    } else if part {
                        mixed(ui, &trf_m!("of {}", format_duration(e.duration(now), fmt)), 12.0, Some(c.text2));
                    }
                });
                ui.add_space(4.0);
                let (conv, resume, more) = ui.vertical(|ui| {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let conv = has_conv.then(|| icon_ghost_quiet(ui, Glyph::Convert, "Conversions"));
                        let resume = play_btn.then(|| icon_ghost(ui, Glyph::Play, "Continue now"));
                        (conv, resume, icon_button(ui, Glyph::More, "Session actions"))
                    }).inner
                }).inner;
                // the row's own click area stops before the first button, or it would take their hover
                buttons_left = conv.as_ref().or(resume.as_ref()).map(|r| r.rect.left()).unwrap_or(more.rect.left());
                if let Some(conv) = conv {
                    let cid = ui.make_persistent_id(("row-conv", &e.id, *a));
                    if conv.clicked() || std::mem::take(&mut app.today.conv_request) {
                        ui.memory_mut(|m| m.toggle_popup(cid));
                    }
                    let open = ui.memory(|m| m.is_popup_open(cid));
                    egui::popup_below_widget(ui, cid, &conv, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                        if let Some(v) = conversions_card(ui, &profiles, b - a, true) {
                            copied = Some(v);
                            ui.memory_mut(|m| m.close_popup());
                        }
                    });
                    if !open {
                        conv.on_hover_ui(|ui| {
                            conversions_card(ui, &profiles, b - a, false);
                        });
                    }
                }
                if resume.is_some_and(|r| r.clicked()) {
                    action = Some((0, e.clone()));
                }
                action_rect = more.rect;
                app.geometry.rows.push((format!("ledger action · {}", e.id), more.rect));
                let pid = ui.make_persistent_id(("row-more", &e.id, *a));
                if more.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(pid));
                }
                egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(180.0);
                    if menu_row(ui, "Continue now", false, false).clicked() {
                        action = Some((0, e.clone()));
                    }
                    if menu_row(ui, "Edit…", false, false).clicked() {
                        action = Some((1, e.clone()));
                    }
                    // the task's own page: source data (Azure DevOps) and every session on it
                    if app.store.db.task(&e.task_id).is_some() && menu_row(ui, "Open task", false, false).clicked() {
                        action = Some((3, e.clone()));
                    }
                    if menu_row(ui, "Duplicate", false, false).on_hover_text(tr("A copy with the same times, opened for editing")).clicked() {
                        action = Some((4, e.clone()));
                    }
                    if e.duration(now) >= 120 && menu_row(ui, "Split in two", false, false).on_hover_text(tr("Two sessions meeting at the midpoint; adjust the time afterwards")).clicked() {
                        action = Some((5, e.clone()));
                    }
                    let fav = favorite_of(e);
                    let is_fav = app.store.local.favorites.contains(&fav);
                    if menu_row(ui, if is_fav { "Remove from favorites" } else { "Add to favorites" }, false, false).clicked() {
                        action = Some((6, e.clone()));
                    }
                    if menu_row(ui, "Delete", false, false).clicked() {
                        action = Some((2, e.clone()));
                    }
                });
            });
            if app.today.highlight.as_deref() == Some(e.id.as_str()) {
                let r = row.response.rect.expand2(egui::vec2(0.0, 0.0));
                ui.painter().rect_filled(egui::Rect::from_min_max(r.min, egui::pos2(r.left() + 3.0, r.max.y)), 1.5, c.focus);
            }
            app.geometry.rows.push((format!("ledger row · {}", e.id), row.response.rect));
            let click_rect = egui::Rect::from_min_max(row.response.rect.min, egui::pos2(buttons_left - 4.0, row.response.rect.bottom()));
            let resp = ui.interact(click_rect, egui::Id::new(("row", &e.id, *a)), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, super::session_title(&app.store.db, e)));
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                action = Some((1, e.clone()));
            }
            divider(ui);
        }
    }
    if !entries.is_empty() {
        table_border(ui, hr.top());
    }
    if let Some(v) = copied {
        ui.ctx().copy_text(v.clone());
        app.notify(trf_m!("Copied: {}", v));
    }
    if let Some((kind, e)) = action {
        // a locked session can be continued, opened or made a favorite, never changed
        if matches!(kind, 2 | 4 | 5) && devtrack_core::report::locked(&app.store.db, e.start) {
            let m = locked_text(app);
            app.notify(m);
            return;
        }
        match kind {
            0 => app.store.continue_entry(&e.id),
            1 => {
                let ctx = ui.ctx().clone();
                let review = needs_completion(&e);
                open_edit(app, &e, Some(&ctx), review);
            }
            3 => {
                if let Some(t) = app.store.db.task(&e.task_id).cloned() {
                    super::activities::open_task(app, &t);
                }
            }
            4 => {
                let mut copy = e.clone();
                copy.id = new_id();
                app.store.put_entry(copy.clone());
                let ctx = ui.ctx().clone();
                open_edit(app, &copy, Some(&ctx), false);
                app.notify(tr("Session duplicated: adjust the copy"));
            }
            5 => {
                if let Some(stop) = e.stop {
                    // the midpoint, on a whole minute
                    let mid = (e.start + (stop - e.start) / 2) / 60 * 60;
                    if app.store.split_entry(&e.id, mid) {
                        app.notify(trf_m!("Session split at {}", clock_text(mid, app.store.db.settings.clock_24h)));
                    }
                }
            }
            6 => {
                let fav = favorite_of(&e);
                if let Some(i) = app.store.local.favorites.iter().position(|f| f == &fav) {
                    app.store.local.favorites.remove(i);
                    app.notify(tr("Removed from favorites"));
                } else {
                    app.store.local.favorites.push(fav);
                    app.notify(tr("Added to favorites"));
                }
                app.store.save_local();
            }
            _ => {
                app.store.delete_entry(&e.id);
                app.notify_undo(tr("Session deleted"));
            }
        }
    }
}

/// The message shown when something tries to change a locked session.
pub fn locked_text(app: &App) -> String {
    let day = app.store.db.settings.lock_until.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).map(super::i18n::short_date).unwrap_or_default();
    trf_m!("Sessions up to {} are locked. Change the lock in Settings › Display.", day)
}

/// Saves the period's sessions as a CSV file chosen by the person.
/// The sessions of a period as a CSV file (Settings › Data).
pub fn export_csv(app: &mut App, period: &Period) {
    if !app.store.is_persistent() {
        app.notify(tr("This is a preview: nothing is written to disk."));
        return;
    }
    let last = period.end - CDur::days(1);
    let name = if last == period.start { format!("devtrack-{}.csv", period.start.format("%Y-%m-%d")) } else { format!("devtrack-{}_{}.csv", period.start.format("%Y-%m-%d"), last.format("%Y-%m-%d")) };
    let Some(path) = rfd::FileDialog::new().set_file_name(name).add_filter("CSV", &["csv"]).save_file() else { return };
    // the byte-order mark makes Excel read accents correctly
    let text = format!("\u{feff}{}", devtrack_core::report::sessions_csv(&app.store.db, period, now_secs()));
    match std::fs::write(&path, text) {
        Ok(()) => app.notify(trf_m!("Exported to {}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
        Err(e) => app.notify(trf_m!("Couldn't save the file: {}", e)),
    }
}

/// How the period's time divides among projects: one bar, a segment per project in its colour,
/// with the names above the widest segments. Hidden when everything is on one project.
fn project_bar(app: &App, ui: &mut egui::Ui, entries: &[TimeEntry], period: &Period) {
    let c = p(ui);
    let now = now_secs();
    let (pa, pb) = period.bounds();
    let mut by: Vec<(Option<Id>, i64)> = vec![];
    for e in entries {
        let d = report::overlap(e, pa, pb, now);
        match by.iter_mut().find(|x| x.0 == e.project_id) {
            Some(x) => x.1 += d,
            None => by.push((e.project_id.clone(), d)),
        }
    }
    let total: i64 = by.iter().map(|x| x.1).sum();
    if by.len() < 2 || total <= 0 {
        return;
    }
    by.sort_by_key(|x| std::cmp::Reverse(x.1));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), Sense::hover());
    let mut x = rect.left();
    let mut tip = vec![];
    for (pid, secs) in &by {
        let pr = app.store.db.project(pid);
        let name = pr.map(|p| p.name.clone()).unwrap_or(tr("No project"));
        let color = pr.map(|p| hex(&p.color)).unwrap_or(c.text2);
        let seg = w * (*secs as f32 / total as f32);
        // 2 px between segments so two similar colours still read as two
        let r = egui::Rect::from_min_max(egui::pos2(x, rect.bottom() - 4.0), egui::pos2((x + seg - 2.0).max(x + 1.0), rect.bottom() - 1.0));
        ui.painter().rect_filled(r, 1.5, color);
        let g = ui.painter().layout_no_wrap(name.clone(), sans(12.0), c.text2);
        if g.size().x + 8.0 <= seg {
            ui.painter().galley(egui::pos2(x, rect.top()), g, c.text2);
        }
        tip.push(format!("{} · {} · {}%", name, words(*secs), secs * 100 / total));
        x += seg;
    }
    resp.on_hover_text(tip.join("\n"));
    ui.add_space(ROW_GAP);
}

/// "1:30" for a cell, empty for no time.
fn cell_text(secs: i64) -> String {
    if secs <= 0 { String::new() } else { let m = (secs + 30) / 60; format!("{}:{:02}", m / 60, m % 60) }
}

/// The week as a grid: a row per project, a column per day, one editable total in each cell.
/// Typing a time adds, lengthens or shortens that day's sessions on the project.
fn timesheet(app: &mut App, ui: &mut egui::Ui, entries: &[TimeEntry], period: &Period) {
    let c = p(ui);
    let now = now_secs();
    // days off are left out, unless something was recorded on them
    let worked: std::collections::BTreeSet<NaiveDate> = entries.iter().map(|e| local_date(e.start)).collect();
    let days: Vec<NaiveDate> = period.days().into_iter().filter(|d| app.store.db.settings.is_work_day(*d) || worked.contains(d)).collect();
    // time per project and day, from finished sessions (the grid edits those)
    let mut rows: Vec<(Option<Id>, Vec<i64>)> = vec![];
    let mut running_on: Vec<(Option<Id>, NaiveDate)> = vec![];
    for e in entries {
        let day = local_date(e.start);
        let Some(i) = days.iter().position(|d| *d == day) else { continue };
        if e.is_running() {
            running_on.push((e.project_id.clone(), day));
            continue;
        }
        let secs = e.stop.unwrap_or(e.start) - e.start;
        match rows.iter_mut().find(|r| r.0 == e.project_id) {
            Some(r) => r.1[i] += secs,
            None => {
                let mut v = vec![0; days.len()];
                v[i] = secs;
                rows.push((e.project_id.clone(), v));
            }
        }
    }
    for extra in app.today.sheet_rows.clone() {
        if !rows.iter().any(|r| r.0 == extra) {
            rows.push((extra, vec![0; days.len()]));
        }
    }
    let name_of = |app: &App, pid: &Option<Id>| app.store.db.project(pid).map(|p| p.name.clone()).unwrap_or(tr("No project"));
    rows.sort_by_key(|r| (r.0.is_none(), name_of(app, &r.0).to_lowercase()));

    // seven day columns need room: under 780 the grid keeps that width and scrolls sideways
    let w = ui.available_width().max(780.0);
    ui.set_min_width(w);
    let cell_w = 72.0f32.min(((w - 180.0 - 84.0) / days.len() as f32 - ROW_GAP).max(52.0));
    let total_w = 84.0;
    // the last column: × on a row that has no hours (one added, or emptied)
    let remove_w = CTRL_H;
    let name_w = (w - (cell_w + ROW_GAP) * days.len() as f32 - total_w - ROW_GAP - remove_w - ROW_GAP).max(120.0);
    let right = |ui: &mut egui::Ui, width: f32, add: &mut dyn FnMut(&mut egui::Ui)| {
        ui.allocate_ui_with_layout(egui::vec2(width, CTRL_H), egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.set_width(width); add(ui); });
    };
    // header
    row(ui, 28.0, |ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        ui.allocate_ui_with_layout(egui::vec2(name_w, CTRL_H), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_width(name_w); label(ui, "Project"); });
        for d in &days {
            let text = format!("{} {}", super::i18n::weekday_short(*d), d.format("%d"));
            right(ui, cell_w, &mut |ui| { ui.label(RichText::new(&text).size(12.0).color(if *d == today() { c.text } else { c.text2 })); });
        }
        right(ui, total_w, &mut |ui| { label(ui, "Total"); });
        ui.add_space(remove_w);
    });
    divider(ui);
    if rows.is_empty() {
        ui.add_space(ROW_GAP);
        meta(ui, "No time this week. Add a project below and type its hours, or fill from last week.");
    }
    let mut change: Option<(Option<Id>, NaiveDate, i64)> = None;
    let mut refused = false;
    let mut remove: Option<Option<Id>> = None;
    for (pid, cells) in &rows {
        row(ui, 44.0, |ui| {
            ui.spacing_mut().item_spacing.x = ROW_GAP;
            ui.allocate_ui_with_layout(egui::vec2(name_w, CTRL_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(name_w);
                let pr = app.store.db.project(pid);
                dot(ui, pr.map(|p| hex(&p.color)).unwrap_or(c.text2), 3.5);
                let client = pr.map(|p| app.store.db.client_name(p)).unwrap_or_default();
                let name = name_of(app, pid);
                let text = if client.is_empty() { name.clone() } else { format!("{} · {}", name, client) };
                ui.add(egui::Label::new(RichText::new(text.clone()).font(medium(14.0)).color(if pid.is_none() { c.text2 } else { c.text })).truncate()).on_hover_text(text);
            });
            for (i, d) in days.iter().enumerate() {
                let id = ui.make_persistent_id(("sheet-cell", pid, *d));
                let mut text = ui.data(|x| x.get_temp::<String>(id)).unwrap_or_else(|| cell_text(cells[i]));
                let future = *d > today();
                let live = running_on.iter().any(|(p, day)| p == pid && day == d);
                let r = ui.add_enabled_ui(!future, |ui| input(ui, &mut text, Field::new("Tracked", cell_w).mono())).inner;
                if app.today.sheet_focus.as_ref().is_some_and(|(fp, fd)| fp == pid && fd == d) {
                    app.today.sheet_focus = None;
                    r.request_focus();
                }
                let tip = if live { tr("A session is running here: its time is added when it stops.") } else { tr("Hours (8, 1,5) or hours:minutes (1:30). Empty removes the time.") };
                let r = r.on_hover_text(tip);
                if r.lost_focus() {
                    let new = if text.trim().is_empty() { Some(0) } else { devtrack_core::timefmt::parse_estimate(&text) };
                    match new {
                        // compared to the minute: retyping what is shown changes nothing
                        Some(secs) if cell_text(secs) != cell_text(cells[i]) => change = Some((pid.clone(), *d, secs)),
                        Some(_) => {}
                        None => refused = true,
                    }
                    ui.data_mut(|x| x.remove::<String>(id));
                } else if r.has_focus() {
                    ui.data_mut(|x| x.insert_temp(id, text));
                }
            }
            let sum: i64 = cells.iter().sum();
            right(ui, total_w, &mut |ui| { mixed(ui, &if sum == 0 { "—".to_string() } else { words(sum) }, 14.0, Some(if sum == 0 { c.text2 } else { c.text })); });
            // a row with hours goes away when its cells are emptied; one without is removed here
            let busy = running_on.iter().any(|(p, _)| p == pid);
            if sum == 0 && !busy {
                let r = icon_ghost(ui, Glyph::Close, "Remove this row");
                app.geometry.rows.push((format!("sheet row · remove · {}", pid.clone().unwrap_or_default()), r.rect));
                if r.clicked() { remove = Some(pid.clone()); }
            } else {
                ui.add_space(remove_w);
            }
        });
        divider(ui);
    }
    // totals
    row(ui, 36.0, |ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        ui.allocate_ui_with_layout(egui::vec2(name_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_width(name_w); ui.label(RichText::new(tr("Total")).font(medium(13.0))); });
        let mut grand = 0;
        for i in 0..days.len() {
            let sum: i64 = rows.iter().map(|r| r.1[i]).sum();
            grand += sum;
            right(ui, cell_w, &mut |ui| { ui.label(mono(if sum == 0 { "—".to_string() } else { cell_text(sum) }, 13.0).color(if sum == 0 { c.text2 } else { c.text })); });
        }
        right(ui, total_w, &mut |ui| { mixed_medium(ui, &if grand == 0 { "—".to_string() } else { words(grand) }, 14.0); });
        ui.add_space(remove_w);
    });
    ui.add_space(GROUP_GAP);
    // add a row · copy last week: each with its label above and what it does below, as every
    // field; the copy says beforehand how many cells it fills, and why it can't
    let prev = Period { kind: PeriodKind::Week, start: period.start - CDur::days(7), end: period.start };
    let last: Vec<TimeEntry> = entries_in(&app.store.db, &prev, now).into_iter().filter(|e| e.stop.is_some()).cloned().collect();
    let mut wanted: Vec<(Option<Id>, NaiveDate, i64)> = vec![];
    for e in &last {
        let day = local_date(e.start) + CDur::days(7);
        let secs = e.stop.unwrap_or(e.start) - e.start;
        match wanted.iter_mut().find(|x| x.0 == e.project_id && x.1 == day) {
            Some(x) => x.2 += secs,
            None => wanted.push((e.project_id.clone(), day, secs)),
        }
    }
    let fillable: Vec<(Option<Id>, NaiveDate, i64)> = wanted
        .iter()
        .filter(|(pid, day, _)| {
            let has = rows.iter().find(|r| &r.0 == pid).and_then(|r| days.iter().position(|d| d == day).map(|i| r.1[i] > 0)).unwrap_or(false);
            !has && *day <= today() && !report::locked(&app.store.db, local_time_of(*day, 12 * 3600))
        })
        .cloned()
        .collect();
    let rp = recent_projects(&app.store.db);
    let mut add: Option<Id> = None;
    let mut copy = false;
    let col_w = 340.0f32.min(ui.available_width());
    // two fields side by side, top-aligned; they stack when the window is narrow
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true), |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(GROUP_GAP * 2.0, GROUP_GAP);
        col(ui, col_w, 0.0, |ui| {
            field_info(ui, "Add a project to the week", "Its row appears in the grid: type the hours of each day in it. A row with no hours is removed with ×.", |ui| {
                let mut pick: Option<Id> = None;
                if matches!(project_picker(ui, "sheet-add-row", &app.store.db, &rp, &mut pick, col_w), Pick::Changed) {
                    add = pick;
                }
            });
        });
        col(ui, col_w, 0.0, |ui| {
            // why the button can't act, said where its explanation is
            let help = if !fillable.is_empty() {
                plural(fillable.len() as i64, "Copies last week's hours into {} empty cell, project by project. Only empty cells up to today: filled cells and closed days stay as they are.", "Copies last week's hours into {} empty cells, project by project. Only empty cells up to today: filled cells and closed days stay as they are.")
            } else if wanted.is_empty() {
                tr("Last week has no hours to copy.")
            } else {
                tr("The days last week had hours on are already filled here, or closed.")
            };
            field_info(ui, "Fill from last week", &help, |ui| {
                copy = button_w(ui, Kind::Secondary, "Copy last week", 0.0, !fillable.is_empty()).on_disabled_hover_text(&help).clicked();
            });
        });
    });
    if refused {
        app.notify(tr("That isn't a time. Use hours (8, 1,5) or hours:minutes (1:30)."));
    }
    if let Some(pid) = remove {
        app.today.sheet_rows.retain(|r| *r != pid);
    }
    if let Some(pid) = add {
        if !app.today.sheet_rows.contains(&Some(pid.clone())) {
            app.today.sheet_rows.push(Some(pid.clone()));
        }
        // the cursor goes where the hours go: today, or the week's last day that can be filled
        let day = days.iter().copied().filter(|d| *d <= today()).last().or(days.first().copied());
        if let Some(day) = day {
            app.today.sheet_focus = Some((Some(pid), day));
        }
    }
    if let Some((pid, day, secs)) = change {
        if report::locked(&app.store.db, local_time_of(day, 12 * 3600)) {
            let m = locked_text(app);
            app.notify(m);
        } else if app.store.set_day_total(&pid, day, secs) {
            app.notify(trf_m!("{} · {}: {}", name_of(app, &pid), super::i18n::short_date(day), if secs == 0 { tr("time removed") } else { words(secs) }));
        }
    }
    if copy {
        let mut filled = 0;
        for (pid, day, secs) in fillable {
            if app.store.set_day_total(&pid, day, secs) {
                filled += 1;
            }
        }
        app.notify(if filled == 0 { tr("Nothing to copy: last week is empty, or these days already have time.") } else { plural(filled as i64, "{} cell filled from last week", "{} cells filled from last week") });
    }
}

/// What a favorite keeps of a session: what it was, not when.
fn favorite_of(e: &TimeEntry) -> devtrack_core::store::Favorite {
    devtrack_core::store::Favorite { description: e.description.trim().to_string(), project_id: e.project_id.clone(), task_id: e.task_id.clone(), billable: e.billable }
}

/// Favorites: saved sessions, each a button that starts it now. Shown only when there are some
/// and nothing is running.
fn favorites_row(app: &mut App, ui: &mut egui::Ui) {
    if app.store.local.favorites.is_empty() || app.store.db.running().is_some() {
        return;
    }
    let favorites = app.store.local.favorites.clone();
    let mut start = None;
    ui.add_space(ROW_GAP);
    row_wrapped(ui, CTRL_H, |ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        label(ui, "Favorites");
        for (i, f) in favorites.iter().enumerate() {
            let project = app.store.db.project(&f.project_id).map(|p| p.name.clone());
            let task = app.store.db.task(&f.task_id).map(|t| t.title.clone());
            let name = if !f.description.is_empty() { f.description.clone() } else { task.clone().or(project.clone()).unwrap_or(tr("Untitled session")) };
            let tip = [Some(name.clone()), project, task].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            if button_w(ui, Kind::Secondary, &truncate(&name, 28), 0.0, true).on_hover_text(format!("{}\n{}", tip, tr("Start it now. Remove it from a session's menu."))).clicked() {
                start = Some(i);
            }
        }
    });
    if let Some(f) = start.and_then(|i| favorites.get(i)) {
        app.store.start(&f.description, f.project_id.clone(), f.task_id.clone(), f.billable);
    }
}

fn timeline(app: &mut App, ui: &mut egui::Ui, entries: &[TimeEntry], period: &Period) {
    let now = now_secs();
    let c = p(ui);
    let fmt = app.store.db.settings.duration_format;
    // days off are left out, unless a session falls on them
    let busy: std::collections::BTreeSet<NaiveDate> = entries.iter().flat_map(|e| day_pieces(e, now).into_iter().map(|x| x.0)).collect();
    let days: Vec<NaiveDate> = period.days().into_iter().filter(|d| app.store.db.settings.is_work_day(*d) || busy.contains(d)).collect();
    // the hours shown run from the first session to the last (at least four hours, 8–18 when
    // there is nothing to show), not from a fixed working day
    let mut first_h = 24i64;
    let mut last_h = 0i64;
    let mut pieces: Vec<(NaiveDate, TimeEntry, i64, i64)> = vec![];
    for e in entries {
        for (d, a, b) in day_pieces(e, now) {
            if period.contains(d) {
                let day0 = local_midnight(d);
                first_h = first_h.min((a - day0) / 3600);
                last_h = last_h.max(((b - day0 + 3599) / 3600).min(24));
                pieces.push((d, e.clone(), a, b));
            }
        }
    }
    if pieces.is_empty() {
        // nothing recorded: the working hours
        let st = &app.store.db.settings;
        first_h = (st.work_start_min / 60) as i64;
        last_h = ((st.work_end_min as i64 + 59) / 60).min(24);
    }
    // today in view: the hours always reach the present, so the "now" line is on screen
    if period.contains(today()) {
        let h = chrono::Local::now().hour() as i64;
        first_h = first_h.min(h);
        last_h = last_h.max((h + 1).min(24));
    }
    if last_h - first_h < 4 {
        last_h = (first_h + 4).min(24);
        first_h = (last_h - 4).max(0);
    }
    // the hours fill the height the window gives the calendar (day header and margins taken out),
    // between 48 and 140 per hour: taller windows show more of each session, shorter ones scroll
    let room = ui.available_height() - 38.0 - 16.0;
    let hours = (last_h - first_h).max(1) as f32;
    let hour_h = if room.is_finite() && room > 0.0 && room < 10_000.0 { (room / hours).clamp(48.0, 140.0).floor() } else { 48.0 };
    let gutter = 52.0;
    let mut clicked: Option<TimeEntry> = None;
    let mut create_at: Option<(NaiveDate, i64)> = None;
    let mut dragged: Option<(TimeEntry, i64, i64)> = None;
    let mut refused_future = false;
    let total_w = ui.available_width();
    let col_w = ((total_w - gutter - 8.0) / days.len() as f32).max(60.0);
    let (hr, _) = ui.allocate_exact_size(egui::vec2(gutter + col_w * days.len() as f32, 38.0), Sense::hover());
    for (i, d) in days.iter().enumerate() {
        let x = hr.left() + gutter + i as f32 * col_w + 6.0;
        let tot: i64 = pieces.iter().filter(|x| x.0 == *d).map(|x| x.3 - x.2).sum();
        // the day number follows the name at a fixed 6, whatever the name's width
        let name = ui.painter().text(egui::pos2(x, hr.top() + 2.0), egui::Align2::LEFT_TOP, super::i18n::weekday_short(*d), medium(14.0), if *d == today() { c.text } else { c.text2 });
        ui.painter().text(egui::pos2(name.right() + 6.0, hr.top() + 3.0), egui::Align2::LEFT_TOP, d.format("%d").to_string(), mono_font(12.0), c.text2);
        // an empty day shows no total rather than "0 s"
        if tot > 0 {
            let g = ui.painter().layout_job(mixed_job(&words(tot), 12.0, c.text2, sans(12.0), mono_font(12.0)));
            ui.painter().galley(egui::pos2(x, hr.top() + 21.0), g, c.text2);
        }
    }
    let list = egui::ScrollArea::vertical().id_salt("timeline").auto_shrink([false, false]).show(ui, |ui| {
        let height = (last_h - first_h) as f32 * hour_h;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(gutter + col_w * days.len() as f32, height + 8.0), Sense::hover());
        let painter = ui.painter_at(rect);
        let top = rect.top() + 4.0;
        for h in first_h..=last_h {
            let y = top + (h - first_h) as f32 * hour_h;
            painter.line_segment([egui::pos2(rect.left() + gutter, y), egui::pos2(rect.right(), y)], Stroke::new(1.0, c.line));
            painter.text(egui::pos2(rect.left() + gutter - 8.0, y), egui::Align2::RIGHT_CENTER, format!("{:02}:00", h % 24), mono_font(12.0), c.text2);
        }
        for (i, d) in days.iter().enumerate() {
            let x0 = rect.left() + gutter + i as f32 * col_w;
            painter.line_segment([egui::pos2(x0, top), egui::pos2(x0, top + height)], Stroke::new(1.0, c.line));
            let day0 = local_midnight(*d);
            // a click on an empty part of the day opens "Add session" at that time (15 min steps)
            let col_rect = egui::Rect::from_min_size(egui::pos2(x0, top), egui::vec2(col_w, height));
            let bg = ui.interact(col_rect, egui::Id::new(("tl-col", *d)), Sense::click());
            if bg.clicked() && *d <= today() {
                if let Some(pos) = bg.interact_pointer_pos() {
                    let step = (app.store.db.settings.calendar_step.max(1)) as i64;
                    let mins = (((pos.y - top) / hour_h * 60.0) as i64 + first_h * 60) / step * step;
                    create_at = Some((*d, mins.clamp(0, 23 * 60)));
                }
            }
            // sessions that overlap sit side by side: each takes the first free lane
            let mut dayp: Vec<&(NaiveDate, TimeEntry, i64, i64)> = pieces.iter().filter(|x| x.0 == *d).collect();
            dayp.sort_by_key(|x| x.2);
            let mut lane_end: Vec<i64> = vec![];
            let lanes: Vec<usize> = dayp.iter().map(|x| {
                let l = lane_end.iter().position(|end| *end <= x.2).unwrap_or(lane_end.len());
                if l == lane_end.len() { lane_end.push(x.3); } else { lane_end[l] = x.3; }
                l
            }).collect();
            let lane_w = (col_w - 6.0) / lane_end.len().max(1) as f32;
            for (k, (_, e, a, b)) in dayp.iter().map(|x| (&x.0, &x.1, &x.2, &x.3)).enumerate() {
                let s = (a - day0) as f32 / 3600.0;
                let en = (b - day0) as f32 / 3600.0;
                let y0 = top + (s - first_h as f32) * hour_h;
                let y1 = (top + (en - first_h as f32) * hour_h).max(y0 + 20.0); // a short session still shows its title and is easy to click
                // a session nothing overlaps keeps the whole day's width
                let alone = !dayp.iter().enumerate().any(|(j, y)| j != k && y.2 < *b && *a < y.3);
                let (lx, lw) = if alone { (x0 + 3.0, col_w - 6.0) } else { (x0 + 3.0 + lanes[k] as f32 * lane_w, lane_w - 1.0) };
                let r = egui::Rect::from_min_max(egui::pos2(lx, y0 + 1.0), egui::pos2(lx + lw, y1 - 1.0));
                let col_w = lw + 6.0; // the text below fits the block, not the whole day
                let colr = app.store.db.project(&e.project_id).map(|p| hex(&p.color)).unwrap_or(c.text2);
                painter.rect_filled(r, 5.0, c.raised);
                painter.rect_stroke(r, 5.0, Stroke::new(1.0, if e.is_running() { c.green } else { c.line }), egui::StrokeKind::Inside);
                painter.rect_filled(egui::Rect::from_min_max(r.min, egui::pos2(r.left() + 3.0, r.bottom())), 2.0, colr);
                if r.height() > 16.0 {
                    // as much as the block's height allows, line by line: the title (wrapping to
                    // a second line in a tall block), then the time or what is missing, then the
                    // project; each line only when it fits whole
                    let title_txt = super::session_title(&app.store.db, e);
                    let max_chars = ((col_w - 24.0) / 7.0) as usize;
                    let text_w = (r.width() - 14.0).max(20.0);
                    let line_h = 15.0;
                    let room_lines = ((r.height() - 6.0) / line_h).floor().max(1.0) as usize;
                    let title_rows = if room_lines >= 4 { 2 } else { 1 };
                    let mut job = egui::text::LayoutJob::simple(title_txt.clone(), sans(12.0), c.text, text_w);
                    job.wrap.max_rows = title_rows;
                    job.wrap.break_anywhere = false;
                    job.wrap.overflow_character = Some('…');
                    let g = painter.layout_job(job);
                    let used = g.rows.len().max(1);
                    painter.galley(r.left_top() + egui::vec2(9.0, 3.0), g, c.text);
                    let mut y = r.top() + 3.0 + used as f32 * line_h;
                    if room_lines > used {
                        if needs_completion(e) {
                            painter.text(egui::pos2(r.left() + 9.0, y), egui::Align2::LEFT_TOP, truncate(missing_text(e), max_chars.max(4)), sans(12.0), c.amber);
                        } else {
                            painter.text(egui::pos2(r.left() + 9.0, y), egui::Align2::LEFT_TOP, format_duration(b - a, fmt), mono_font(12.0), c.text2);
                        }
                        y += line_h;
                    }
                    if room_lines > used + 1 {
                        if let Some(p) = app.store.db.project(&e.project_id) {
                            painter.text(egui::pos2(r.left() + 9.0, y), egui::Align2::LEFT_TOP, truncate(&p.name, max_chars.max(4)), sans(12.0), c.text2);
                        }
                    }
                }
                let resp = ui.interact(r, egui::Id::new(("tl", &e.id, *d)), Sense::click_and_drag());
                if resp.clicked() {
                    clicked = Some(e.clone());
                }
                // Drag the block to move it, its lower edge to change its end: 5-minute steps.
                // Only a finished session wholly inside this day, outside the locked days.
                let movable = !e.is_running() && *a == e.start && Some(*b) == e.stop && !report::locked(&app.store.db, e.start);
                let drag_id = egui::Id::new("tl-drag");
                if movable {
                    let near_edge = resp.hover_pos().map(|p| p.y > r.bottom() - 7.0).unwrap_or(false);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(if near_edge { egui::CursorIcon::ResizeVertical } else { egui::CursorIcon::Grab });
                    }
                    if resp.drag_started() {
                        ui.data_mut(|x| x.insert_temp(drag_id, (e.id.clone(), near_edge, 0.0f32)));
                    }
                    let state = ui.data(|x| x.get_temp::<(Id, bool, f32)>(drag_id)).filter(|st| st.0 == e.id);
                    if let Some((_, resize, dy)) = state {
                        let dy = dy + resp.drag_delta().y;
                        let step = app.store.db.settings.calendar_step.max(1) as f32;
                        let mins = ((dy / hour_h * 60.0) / step).round() as i64 * step as i64;
                        let shift = mins as f32 / 60.0 * hour_h;
                        // preview: where it will land, with the new times
                        let preview = if resize { egui::Rect::from_min_max(r.min, egui::pos2(r.right(), (r.bottom() + shift).max(r.top() + 6.0))) } else { r.translate(egui::vec2(0.0, shift)) };
                        let fg = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("tl-preview")));
                        fg.rect(preview, 5.0, c.focus.linear_multiply(0.15), Stroke::new(1.5, c.focus), egui::StrokeKind::Inside);
                        let (ns, ne) = if resize { (e.start, (b + mins * 60).max(e.start + 300)) } else { (e.start + mins * 60, b + mins * 60) };
                        fg.text(preview.left_top() + egui::vec2(9.0, 3.0), egui::Align2::LEFT_TOP, format!("{} → {}", clock_text(ns, true), clock_text(ne, true)), mono_font(12.0), c.text);
                        if resp.drag_stopped() {
                            ui.data_mut(|x| x.remove::<(Id, bool, f32)>(drag_id));
                            if mins != 0 && ne <= now {
                                dragged = Some((e.clone(), ns, ne));
                            } else if ne > now {
                                refused_future = true;
                            }
                        } else {
                            ui.data_mut(|x| x.insert_temp(drag_id, (e.id.clone(), resize, dy)));
                        }
                    }
                }
                resp.on_hover_text(trf_m!("{}\n{} this day", super::session_title(&app.store.db, e), words(b - a)));
            }
            if *d == today() {
                let t = chrono::Local::now();
                let hrs = t.hour() as f32 + t.minute() as f32 / 60.0;
                if hrs >= first_h as f32 && hrs <= last_h as f32 {
                    let y = top + (hrs - first_h as f32) * hour_h;
                    // "now": a red line across today with a dot at its start, above the blocks
                    let fg = ui.painter_at(rect);
                    fg.line_segment([egui::pos2(x0, y), egui::pos2(x0 + col_w, y)], Stroke::new(2.0, c.red));
                    fg.circle_filled(egui::pos2(x0, y), 4.0, c.red);
                }
            }
        }
    });
    scroll_fades(ui, &list, p(ui).canvas);
    if refused_future {
        app.notify(tr("A session can't end in the future."));
    }
    if let Some((mut e, start, stop)) = dragged {
        e.start = start;
        e.stop = Some(stop);
        let h24 = app.store.db.settings.clock_24h;
        app.store.put_entry(e);
        app.notify(trf_m!("Session now {} → {}", clock_text(start, h24), clock_text(stop, h24)));
    }
    if let Some((day, mins)) = create_at {
        let d = day.format("%Y-%m-%d").to_string();
        let end = (mins + 60).min(23 * 60 + 59);
        app.remember_focus(ui.ctx());
        app.today.past = Past { date: d.clone(), end_date: d, start: format!("{:02}:{:02}", mins / 60, mins % 60), end: format!("{:02}:{:02}", end / 60, end % 60), ..Past::default() };
        app.today.past_open = true;
    }
    if let Some(e) = clicked {
        let ctx = ui.ctx().clone();
        let review = needs_completion(&e);
        open_edit(app, &e, Some(&ctx), review);
    }
}

// ---------- dialogs ----------

pub fn modal_frame(c: &Pal) -> egui::Frame {
    egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).inner_margin(egui::Margin::same(24))
}

/// Validate the edit fields; fills `errors` (does not change the entry).
pub fn validate_edit(ed: &mut Edit) -> bool {
    ed.errors.clear();
    if NaiveDate::parse_from_str(ed.f.start_date.trim(), "%Y-%m-%d").is_err() {
        ed.errors.insert("date", tr("Enter a date as YYYY-MM-DD"));
    }
    if parse_clock(&ed.f.start).is_none() {
        ed.errors.insert("start", tr("Enter a time from 00:00 to 23:59"));
    }
    if ed.entry.stop.is_some() {
        if ed.f.duration_edited {
            if parse_duration(&ed.f.duration).is_none() {
                ed.errors.insert("dur", tr("Enter a duration like 1:30, 1.5 or 90"));
            }
        } else {
            if NaiveDate::parse_from_str(ed.f.end_date.trim(), "%Y-%m-%d").is_err() {
                ed.errors.insert("end_date", tr("Enter a date as YYYY-MM-DD"));
            }
            if parse_clock(&ed.f.end).is_none() {
                ed.errors.insert("end", tr("Enter a time from 00:00 to 23:59"));
            }
        }
    }
    if ed.errors.is_empty() {
        if let Err((k, m)) = edit::resolve(&ed.f, &ed.orig, ed.original.start, ed.original.stop) {
            ed.errors.insert(k, tr(&m));
        }
    }
    ed.errors.is_empty()
}

/// The time of a session as the two dialogs edit it.
struct TimeBlock<'a> {
    start_date: &'a mut String,
    start: &'a mut String,
    end_date: &'a mut String,
    end: &'a mut String,
    dur: &'a mut String,
    /// start date, start time, end date, end time, duration
    bad: [bool; 5],
    running: bool,
    /// The duration these fields give, when they can be read: for the conversions.
    secs: Option<i64>,
    monday: bool,
    focus: Option<&'static str>,
}

struct TimeOut {
    end_changed: bool,
    dur_changed: bool,
    blurred: bool,
    rects: Vec<(&'static str, egui::Rect)>,
}

/// Start and End are what the person sets; the Duration is what they add up to. So the block is
/// one frame: the two rows on the left as ordinary fields, the duration on the right as a large
/// figure with the conversions under it. The figure is still editable (an underline says so):
/// typing it moves the end.
fn time_block(ui: &mut egui::Ui, db: &Db, w: f32, t: TimeBlock) -> TimeOut {
    let c = p(ui);
    let mut out = TimeOut { end_changed: false, dur_changed: false, blurred: false, rects: vec![] };
    let narrow = w < 460.0;
    let inner = w - 26.0;
    let dur_w = if narrow { inner } else { 176.0 };
    let left_w = if narrow { inner } else { inner - dur_w - 33.0 };
    let lab_w = if narrow { 0.0 } else { 48.0 };
    let fields_w = left_w - if narrow { ROW_GAP } else { lab_w + ROW_GAP * 2.0 };
    let date_w = (fields_w * 0.56).floor();
    let time_w = fields_w - date_w;
    let TimeBlock { start_date, start, end_date, end, dur, bad, running, secs, monday, focus } = t;
    egui::Frame::new().stroke(Stroke::new(1.0, c.line)).corner_radius(RADIUS + 2.0).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
        ui.set_width(inner);
        let mut body = |ui: &mut egui::Ui, out: &mut TimeOut| {
            let line = |ui: &mut egui::Ui, out: &mut TimeOut, name: &str, names: [&str; 2], keys: [&'static str; 2], date: &mut String, time: &mut String, bads: [bool; 2], geo: [&'static str; 2], live: bool| -> bool {
                let mut changed = false;
                if narrow {
                    ui.label(RichText::new(tr(name)).size(12.0).color(c.text2));
                    ui.add_space(LABEL_GAP);
                }
                row(ui, CTRL_H, |ui| {
                    ui.spacing_mut().item_spacing.x = ROW_GAP;
                    if !narrow {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(lab_w, CTRL_H), Sense::hover());
                        ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, tr(name), sans(13.0), c.text2);
                    }
                    if live {
                        // a running session has no end yet
                        readonly(ui, "—", date_w, true);
                        readonly(ui, &tr("running"), time_w, false);
                        out.rects.push((geo[1], last_input_rect(ui)));
                        return;
                    }
                    let r = date_input(ui, date, names[0], date_w, bads[0], monday).on_hover_text("YYYY-MM-DD");
                    out.rects.push((geo[0], last_input_rect(ui)));
                    if focus == Some(keys[0]) {
                        r.request_focus();
                    }
                    out.blurred |= r.lost_focus();
                    changed |= r.changed();
                    let r = time_input(ui, time, names[1], time_w, bads[1]).on_hover_text(tr("HH:MM · ↑↓ on hours or minutes, Shift: 10 min"));
                    out.rects.push((geo[1], last_input_rect(ui)));
                    if focus == Some(keys[1]) {
                        r.request_focus();
                    }
                    out.blurred |= r.lost_focus();
                    changed |= r.changed();
                });
                changed
            };
            ui.vertical(|ui| {
                ui.set_width(left_w);
                ui.spacing_mut().item_spacing.y = 0.0;
                line(ui, out, "Session start", ["Start date", "Start time"], ["date", "start"], start_date, start, [bad[0], bad[1]], ["start date", "start"], false);
                ui.add_space(ROW_GAP);
                if line(ui, out, "Session end", ["End date", "End time"], ["end_date", "end"], end_date, end, [bad[2], bad[3]], ["end date", "end"], running) {
                    out.end_changed = true;
                }
            });
        };
        let mut result = |ui: &mut egui::Ui, out: &mut TimeOut| {
            ui.vertical(|ui| {
                ui.set_width(dur_w);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new(tr("Duration")).size(12.0).color(c.text2));
                ui.add_space(2.0);
                if running {
                    let r = ui.label(RichText::new(clock(secs.unwrap_or(0))).font(mono_medium(22.0)));
                    out.rects.push(("duration", r.rect));
                } else {
                    let r = ui.add(egui::TextEdit::singleline(dur).font(mono_medium(22.0)).frame(false).margin(egui::Margin::ZERO).desired_width(dur_w).text_color(if bad[4] { c.red } else { c.text }));
                    // no field box: the underline alone says the figure can be typed
                    let line_c = if bad[4] { c.red } else if r.has_focus() { c.focus } else { hover_mix(ui, r.id.with("u"), r.hovered(), c.border, c.text2) };
                    let y = r.rect.bottom() + 1.0;
                    ui.painter().line_segment([egui::pos2(r.rect.left(), y), egui::pos2(r.rect.right(), y)], Stroke::new(if r.has_focus() { 2.0 } else { 1.0 }, line_c));
                    out.rects.push(("duration", r.rect));
                    if focus == Some("dur") {
                        r.request_focus();
                    }
                    out.blurred |= r.lost_focus();
                    out.dur_changed |= r.changed();
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, tr("Duration")));
                    r.on_hover_text(tr("1:30 · 1.5 · 90 — changing it moves the end time"));
                }
                ui.add_space(8.0);
                // the same time as each platform writes it: follows the fields as they change
                let conv = secs.filter(|s| *s > 0).map(|s| super::conversions_text(db, s)).unwrap_or_default();
                row(ui, 16.0, |ui| {
                    if !conv.is_empty() {
                        ui.add(egui::Label::new(mixed_job(&conv, 12.0, c.text2, sans(12.0), mono_font(12.0))).truncate()).on_hover_text(conv.replace(" · ", "\n"));
                    }
                });
            });
        };
        if narrow {
            body(ui, &mut out);
            ui.add_space(12.0);
            divider(ui);
            ui.add_space(12.0);
            result(ui, &mut out);
        } else {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                body(ui, &mut out);
                ui.add_space(16.0);
                let (sr, _) = ui.allocate_exact_size(egui::vec2(1.0, CTRL_H * 2.0 + ROW_GAP), Sense::hover());
                ui.painter().rect_filled(sr, 0.0, c.line);
                ui.add_space(16.0);
                result(ui, &mut out);
            });
        }
    });
    out
}

pub fn edit_modal(app: &mut App, ctx: &egui::Context) {
    let Some(mut ed) = app.today.edit.take() else { return };
    let dark = app.dark();
    let c = pal(dark);
    let mut close = false;
    let w = 600.0f32.min(ctx.screen_rect().width() - 96.0);
    let running = ed.entry.stop.is_none();
    let mut x_close = false;
    let modal = egui::Modal::new(egui::Id::new("edit-session")).frame(modal_frame(&c)).backdrop_color(Color32::from_black_alpha(if dark { 160 } else { 90 })).show(ctx, |ui| {
        ui.set_width(w);
        x_close = modal_close(ui);
        ui.label(RichText::new(tr(if running { "Edit running session" } else { "Edit session" })).font(semibold(20.0)));
        ui.add_space(GROUP_GAP);
        egui::ScrollArea::vertical().id_salt("edit-body").max_height((ctx.screen_rect().height() - if w < 460.0 { 280.0 } else { 220.0 }).max(80.0)).show(ui, |ui| {
        ui.set_width(w);
        // what the session was: the task first, then its client and project
        let project_error = ed.errors.get("project").cloned();
        work_fields(app, ui, "edit", w, &mut ed.entry.project_id, &mut ed.entry.task_id, &mut ed.client, project_error.as_deref());
        if ed.entry.project_id.is_some() || ed.client.is_none() {
            ed.errors.remove("project");
        }
        if let Some(r) = app.store.db.task(&ed.entry.task_id).and_then(source_ref) {
            ui.horizontal(|ui| {
                meta(ui, "Linked to");
                ui.label(mono(r, 12.0).color(c.text2));
            });
        }
        ui.add_space(GROUP_GAP - ROW_GAP);
        // One framed block for the time: Start and End as two rows on the left, and the Duration
        // on the right as their result (large, no field box). It can still be typed: that moves
        // the end.
        let mut blurred = false;
        let before_date = ed.f.start_date.clone();
        let resolved = if running { None } else { edit::resolve(&ed.f, &ed.orig, ed.original.start, ed.original.stop).ok() };
        let consequence = resolved.as_ref().and_then(|res| {
            let days = res.stop.map(|s| (local_date(s) - local_date(res.start)).num_days()).unwrap_or(0);
            if days == 1 { Some(tr("Ends next day")) } else { None }
        });
        let secs = if running { Some(ed.entry.duration(now_secs())) } else { resolved.as_ref().and_then(|r| r.stop.map(|s| s - r.start)) };
        let bad = |k: &str| ed.errors.contains_key(k);
        let bads = [bad("date"), bad("start"), bad("end_date"), bad("end"), bad("dur")];
        let out = time_block(ui, &app.store.db, w, TimeBlock { start_date: &mut ed.f.start_date, start: &mut ed.f.start, end_date: &mut ed.f.end_date, end: &mut ed.f.end, dur: &mut ed.f.duration, bad: bads, running, secs, monday: app.store.db.settings.week_starts_monday, focus: ed.focus_field });
        if out.end_changed {
            ed.f.duration_edited = false;
        }
        if out.dur_changed {
            ed.f.duration_edited = true;
        }
        blurred |= out.blurred;
        app.geometry.rows.extend(out.rects.iter().map(|(n, r)| (format!("edit · {}", n), *r)));
        // one line, always there: the first time error, or what the interval implies
        let time_error = ["date", "start", "end_date", "end", "dur"].into_iter().find_map(|k| ed.errors.get(k).cloned());
        ui.add_space(LABEL_GAP);
        row(ui, 18.0, |ui| {
            if let Some(m) = &time_error {
                ui.add(egui::Label::new(RichText::new(tr(m)).size(12.0).color(c.red)).truncate()).on_hover_text(tr(m));
            } else if let Some(text) = &consequence {
                meta(ui, text.clone());
            }
        });
        // checks about the saved session sit with the time they describe: amber, worded
        let checks = session_observations(&ed.entry.id, &flags(&app.store.db, &app.today.period, now_secs()));
        if !checks.is_empty() {
            ui.add_space(ROW_GAP);
            notice(ui, c.amber, |ui| {
                for check in &checks {
                    mixed(ui, &format!("! {}", check), 13.0, Some(c.amber));
                }
            }).response.on_hover_text(tr("These observations describe the saved session. They do not change its duration."));
        }
        // moving the start date carries an untouched end date along
        if ed.f.start_date != before_date {
            if let (Ok(a), Ok(b), Ok(e)) = (NaiveDate::parse_from_str(before_date.trim(), "%Y-%m-%d"), NaiveDate::parse_from_str(ed.f.start_date.trim(), "%Y-%m-%d"), NaiveDate::parse_from_str(ed.f.end_date.trim(), "%Y-%m-%d")) {
                ed.f.end_date = (e + (b - a)).format("%Y-%m-%d").to_string();
            }
        }
        ed.focus_field = None;
        // the duration follows the start and end unless it is what's being typed
        if !running && !ed.f.duration_edited {
            if let Ok(res) = edit::resolve(&ed.f, &ed.orig, ed.original.start, ed.original.stop) {
                if let Some(stop) = res.stop {
                    let d = format_duration(stop - res.start, DurationFormat::Improved);
                    if d != ed.f.duration {
                        ed.f.duration = d;
                    }
                }
            } else if ed.f.duration != "—" {
                // start or end can't be read: no stale duration next to them
                ed.f.duration = "—".into();
            }
        }
        if blurred && ed.attempted {
            validate_edit(&mut ed);
        }
        // a note, optional: a detail about this session; the task says what it was
        field(ui, "Note", None, None, |ui| input(ui, &mut ed.entry.description, Field::new("Note", w).hint("Optional: a detail about this session")));
        ui.add_space(GROUP_GAP - ROW_GAP);
        // tags: free words, separated by commas; a second way to classify, across projects
        let tags_id = egui::Id::new(("edit-tags", &ed.entry.id));
        let mut tags = ui.data(|d| d.get_temp::<String>(tags_id)).unwrap_or_else(|| ed.entry.tags.join(", "));
        field(ui, "Tags", None, None, |ui| input(ui, &mut tags, Field::new("Tags", w).hint("e.g. meeting, urgent — separated by commas")));
        ed.entry.tags = tags.split(',').map(|t| t.trim().trim_start_matches('#').to_string()).filter(|t| !t.is_empty()).collect();
        ui.data_mut(|d| d.insert_temp(tags_id, tags));
        ui.add_space(ROW_GAP);
        checkbox(ui, &mut ed.entry.billable, tr("Billable"));
        if let Some(g) = ed.errors.get("general") {
            ui.label(RichText::new(g).size(12.0).color(c.red));
        }
        if let Some((a, b)) = ed.confirm_long {
            ui.add_space(ROW_GAP);
            notice(ui, c.amber, |ui| {
                mixed(ui, &trf("! This session would last {}. Check the dates, or press “Save anyway”.", &[&words(b - a)]), 13.0, Some(c.amber));
            });
        }
        }); // scrollable editor body
        ui.add_space(GROUP_GAP);
        divider(ui);
        ui.add_space(12.0);
        if ed.confirm_discard {
            row(ui, CTRL_H, |ui| {
                ui.label(tr("Discard your changes?"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if danger(ui, "Discard").clicked() {
                        close = true;
                    }
                    if secondary(ui, "Keep editing").clicked() {
                        ed.confirm_discard = false;
                    }
                });
            });
            return;
        }
        row_wrapped(ui, CTRL_H, |ui| {
            if ed.confirm_delete {
                if danger(ui, "Delete session").clicked() {
                    if devtrack_core::report::locked(&app.store.db, ed.original.start) {
                        ed.errors.insert("general", locked_text(app));
                        ed.confirm_delete = false;
                    } else {
                        app.store.delete_entry(&ed.entry.id);
                        app.notify_undo(tr("Session deleted"));
                        close = true;
                    }
                }
                if ghost(ui, "Keep").clicked() {
                    ed.confirm_delete = false;
                }
            } else if danger(ui, "Delete…").clicked() {
                ed.confirm_delete = true;
            }
            if w < 460.0 { ui.end_row(); }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // after the long-session notice the same button says what it will do
                if primary(ui, if ed.confirm_long.is_some() { "Save anyway" } else { "Save" }).clicked() {
                    ed.attempted = true;
                    let moved_into_lock = edit::resolve(&ed.f, &ed.orig, ed.original.start, ed.original.stop).map(|r| devtrack_core::report::locked(&app.store.db, r.start)).unwrap_or(false);
                    if devtrack_core::report::locked(&app.store.db, ed.original.start) || moved_into_lock {
                        ed.errors.insert("general", locked_text(app));
                    } else if ed.client.is_some() && ed.entry.project_id.is_none() {
                        ed.errors.insert("project", "Choose one of this client's projects".into());
                    } else if !validate_edit(&mut ed) {
                        ed.focus_field = ["date", "start", "end_date", "end", "dur"].into_iter().find(|k| ed.errors.contains_key(k));
                    } else if let Ok(res) = edit::resolve(&ed.f, &ed.orig, ed.original.start, ed.original.stop) {
                        let confirmed = ed.confirm_long == Some((res.start, res.stop.unwrap_or(res.start)));
                        if res.long && !confirmed {
                            ed.confirm_long = Some((res.start, res.stop.unwrap_or(res.start)));
                        } else {
                            let mut e = ed.entry.clone();
                            e.start = res.start;
                            e.stop = res.stop;
                            match validate(&e) {
                                Ok(()) => {
                                    if e != ed.original {
                                        app.store.put_entry(e);
                                        app.notify(tr("Session saved"));
                                    }
                                    close = true;
                                }
                                Err(m) => {
                                    ed.errors.insert(if m.contains("future") { "start" } else { "end" }, tr(&m));
                                }
                            }
                        }
                    }
                }
                if secondary(ui, "Cancel").clicked() {
                    if ed.dirty() {
                        ed.confirm_discard = true;
                    } else {
                        close = true;
                    }
                }
            });
        });
    });
    if (modal.should_close() || x_close) && !close {
        if ed.dirty() {
            ed.confirm_discard = true;
        } else {
            close = true;
        }
    }
    if close {
        ctx.data_mut(|d| d.remove::<String>(egui::Id::new(("edit-tags", &ed.entry.id))));
        // back to where the editor was opened: same period, filter, scroll and highlighted row
        app.restore_focus(ctx);
    } else {
        app.today.edit = Some(ed);
    }
}

pub fn past_modal(app: &mut App, ctx: &egui::Context) {
    if !app.today.past_open {
        return;
    }
    let dark = app.dark();
    let c = pal(dark);
    let mut close = false;
    let mut past = app.today.past.clone();
    // the same width and the same time rows as "Edit session"
    let w = 600.0f32.min(ctx.screen_rect().width() - 96.0);
    let mut x_close = false;
    let modal = egui::Modal::new(egui::Id::new("add-past")).frame(modal_frame(&c)).backdrop_color(Color32::from_black_alpha(if dark { 160 } else { 90 })).show(ctx, |ui| {
        ui.set_width(w);
        x_close = modal_close(ui);
        ui.label(RichText::new(tr("Add past time")).font(semibold(20.0)));
        meta(ui, if app.store.db.running().is_some() { "A separate entry: the running session keeps going, unchanged." } else { "A separate entry for work that already happened." });
        ui.add_space(GROUP_GAP);
        egui::ScrollArea::vertical().id_salt("past-body").max_height((ctx.screen_rect().height() - 240.0).max(100.0)).show(ui, |ui| {
        ui.set_width(w);
        let project_error = past.errors.get("project").cloned();
        work_fields(app, ui, "past", w, &mut past.project, &mut past.task, &mut past.client, project_error.as_deref());
        if past.project.is_some() || past.client.is_none() {
            past.errors.remove("project");
        }
        ui.add_space(GROUP_GAP - ROW_GAP);
        let bad = |k: &str| past.errors.contains_key(k);
        let bads = [bad("date"), bad("start"), bad("end_date"), bad("end"), bad("dur")];
        let before_date = past.date.clone();
        let secs = build_past(&past).ok().and_then(|e| e.stop.map(|s| s - e.start));
        let out = time_block(ui, &app.store.db, w, TimeBlock { start_date: &mut past.date, start: &mut past.start, end_date: &mut past.end_date, end: &mut past.end, dur: &mut past.dur, bad: bads, running: false, secs, monday: app.store.db.settings.week_starts_monday, focus: None });
        if out.end_changed {
            past.dur_edited = false;
        }
        if out.dur_changed {
            past.dur_edited = true;
        }
        // moving the start date carries the end date along
        if past.date != before_date {
            if let (Ok(a), Ok(b), Ok(e)) = (NaiveDate::parse_from_str(before_date.trim(), "%Y-%m-%d"), NaiveDate::parse_from_str(past.date.trim(), "%Y-%m-%d"), NaiveDate::parse_from_str(past.end_date.trim(), "%Y-%m-%d")) {
                past.end_date = (e + (b - a)).format("%Y-%m-%d").to_string();
            }
        }
        // end and duration follow each other: whichever was typed last decides
        let built = build_past(&past);
        if let Ok(e) = &built {
            if let Some(stop) = e.stop {
                if past.dur_edited {
                    past.end_date = local_date(stop).format("%Y-%m-%d").to_string();
                    past.end = clock_text(stop, true);
                } else {
                    past.dur = format_duration(stop - e.start, DurationFormat::Improved);
                }
            }
        }
        // one line, always there: the first time error, or what the interval implies
        let time_error = ["date", "start", "end_date", "end", "dur"].into_iter().find_map(|k| past.errors.get(k).cloned());
        ui.add_space(LABEL_GAP);
        row(ui, 18.0, |ui| {
            if let Some(m) = &time_error {
                ui.add(egui::Label::new(RichText::new(tr(m)).size(12.0).color(c.red)).truncate());
            } else if let Ok(e) = &built {
                if e.stop.map(|s| (local_date(s) - local_date(e.start)).num_days()) == Some(1) {
                    meta(ui, "Ends next day");
                }
            }
        });
        ui.add_space(ROW_GAP);
        // a note, optional: the task says what it was
        field(ui, "Note", None, None, |ui| input(ui, &mut past.desc, Field::new("Note", w).hint("Optional: a detail about this session")));
        ui.add_space(ROW_GAP);
        checkbox(ui, &mut past.billable, tr("Billable"));
        if let Some(g) = past.errors.get("general") {
            ui.label(RichText::new(g).size(12.0).color(c.red));
        }
        ui.add_space(GROUP_GAP);
        });
        divider(ui);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary(ui, "Add session").clicked() {
                    if past.client.is_some() && past.project.is_none() {
                        past.errors.insert("project", "Choose one of this client's projects".into());
                    } else { match build_past(&past) {
                        Ok(entry) if devtrack_core::report::locked(&app.store.db, entry.start) => {
                            past.errors.insert("general", locked_text(app));
                        }
                        Ok(entry) => match app.store.add_manual(entry) {
                            Ok(()) => {
                                app.notify(tr("Past time added"));
                                close = true;
                            }
                            Err(e) => {
                                past.errors.insert("general", e);
                            }
                        },
                        Err((k, e)) => {
                            past.errors.clear();
                            past.errors.insert(k, e);
                        }
                    } }
                }
                if secondary(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
    });
    if modal.should_close() || x_close {
        close = true;
    }
    if close {
        app.today.past_open = false;
        app.today.past_focused = false;
        app.today.past = Past::default();
        app.restore_focus(ctx);
    } else {
        app.today.past = past;
    }
}

pub fn build_past(t: &Past) -> Result<TimeEntry, (&'static str, String)> {
    let date = NaiveDate::parse_from_str(t.date.trim(), "%Y-%m-%d").map_err(|_| ("date", "Enter a date as YYYY-MM-DD".to_string()))?;
    let s = parse_clock(&t.start).ok_or(("start", "Enter a time from 00:00 to 23:59".to_string()))?;
    let start = local_time_of(date, s);
    let stop = if t.dur_edited {
        start + parse_duration(&t.dur).ok_or(("dur", "Enter a duration like 1:30, 1.5 or 90".to_string()))?
    } else {
        let end_date = NaiveDate::parse_from_str(t.end_date.trim(), "%Y-%m-%d").map_err(|_| ("end_date", "Enter a date as YYYY-MM-DD".to_string()))?;
        let e = parse_clock(&t.end).ok_or(("end", "Enter a time from 00:00 to 23:59".to_string()))?;
        local_time_of(end_date, e)
    };
    let e = TimeEntry { id: new_id(), description: t.desc.trim().to_string(), project_id: t.project.clone(), task_id: t.task.clone(), start, stop: Some(stop), billable: t.billable, ..Default::default() };
    validate(&e).map_err(|m| ("general", m))?;
    Ok(e)
}

#[cfg(test)]
mod ledger_regression_tests {
    use super::*;
    use devtrack_core::{sample::{build_state, DemoState}, store::Store};

    #[test]
    fn multiple_observations_do_not_push_the_action_outside_the_ledger() {
        for width in [264.0, 600.0, 800.0] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Incomplete, false)), None);
            let start = local_midnight(today());
            app.store.db.entries.clear();
            app.store.db.entries.insert("long".into(), TimeEntry { id: "long".into(), project_id: Some("p-portal".into()), start, stop: Some(start + 86375), ..Default::default() });
            for n in 0..5 {
                let id = format!("overlap-{n}");
                app.store.db.entries.insert(id.clone(), TimeEntry { id, description: "Another overlapping record".repeat(10), start: start + (10 + n) * 3600, stop: Some(start + (11 + n) * 3600), ..Default::default() });
            }
            let entries: Vec<_> = app.store.db.live_entries().cloned().collect();
            let per = Period::containing(PeriodKind::Day, today(), true);
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 1200.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| ledger(&mut app, ui, &entries, &per));
            });
            for (name, rect) in &app.geometry.rows {
                if name.starts_with("ledger") { assert!(rect.left() >= -0.5 && rect.right() <= width + 1.0, "{width}: {name} {rect:?}"); }
            }
            assert!(app.geometry.rows.iter().any(|(n, _)| n == "ledger action · long"));
            let checks = session_observations("long", &flags(&app.store.db, &per, now_secs()));
            assert_eq!(checks.len(), 2, "one long-duration check and one grouped overlap check");
        }
    }
}

#[cfg(test)]
mod session_panel_tests {
    use super::*;
    use devtrack_core::{sample::{build_state, DemoState}, store::Store};

    fn demo() -> App {
        App::new(Store::in_memory(build_state(now_secs(), DemoState::Ready, false)), None)
    }

    #[test]
    fn suggestions_match_project_names_and_never_repeat() {
        let app = demo();
        let pr = app.store.db.live_projects().into_iter().next().cloned().expect("a demo project");
        let q = pr.name[..3.min(pr.name.len())].to_string();
        let list = suggestions(&app.store.db, &q, None);
        assert!(!list.is_empty(), "typing part of a project name suggests something");
        let mut keys = vec![];
        for s in &list {
            let k = match s { Suggest::Task(id) => format!("t:{id}"), Suggest::Project(id) => format!("p:{id}"), Suggest::Remote(n) => format!("r:{n}"), Suggest::LookUp(n) => format!("l:{n}"), Suggest::Refresh => "refresh".into() };
            assert!(!keys.contains(&k), "duplicate suggestion");
            keys.push(k);
        }
        assert!(suggestions(&app.store.db, "   ", None).is_empty());
    }

    #[test]
    fn typing_in_the_description_opens_suggestions() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = demo();
        let name = app.store.db.live_projects().into_iter().next().unwrap().name.clone();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 900.0));
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            app.geometry.rows.clear();
            let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| session_panel(app, ui));
            });
        };
        frame(&mut app, vec![]);
        let at = app.geometry.rows.iter().find(|(n, _)| n == "start row · description").unwrap().1.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, vec![egui::Event::PointerMoved(at), press(true)]);
        frame(&mut app, vec![press(false)]);
        frame(&mut app, vec![egui::Event::Text(name[..3].to_string())]);
        frame(&mut app, vec![]);
        assert_eq!(app.today.draft.desc, name[..3]);
        let popup = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("session-suggest")));
        assert!(popup.is_some_and(|r| r.height() > 30.0), "suggestions open under the field");
    }

    #[test]
    fn sidebar_asks_before_losing_changes_and_opens_the_top_level() {
        let mut app = demo();
        let id = app.store.db.live_projects().into_iter().next().unwrap().id.clone();
        app.act.open_project(id);
        app.today.only_incomplete = true;
        app.nav_to(Page::Activities);
        assert!(app.act.selected.is_none(), "a section opens at its top level");
        assert!(!app.today.only_incomplete);
        app.today.past_open = true;
        app.today.past.desc = "typed".into();
        app.nav_to(Page::Statistics);
        assert_eq!(app.pending_nav, Some(Page::Statistics), "unsaved input asks first");
        assert_eq!(app.page, Page::Activities);
    }
}

/// The conversions icon of a row: hovering shows the duration as each platform writes it,
/// clicking keeps the card open so a value can be copied. Returns the copied value.
pub fn conversions_icon(ui: &mut egui::Ui, salt: impl std::hash::Hash, profiles: &[TimesheetProfile], secs: i64) -> (egui::Response, Option<String>) {
    let conv = icon_ghost_quiet(ui, Glyph::Convert, "Conversions");
    let cid = ui.make_persistent_id(("row-conv", salt));
    if conv.clicked() {
        ui.memory_mut(|m| m.toggle_popup(cid));
    }
    let open = ui.memory(|m| m.is_popup_open(cid));
    let mut copied = None;
    egui::popup_below_widget(ui, cid, &conv, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        if let Some(v) = conversions_card(ui, profiles, secs, true) {
            copied = Some(v);
            ui.memory_mut(|m| m.close_popup());
        }
    });
    let conv = if open { conv } else { conv.on_hover_ui(|ui| { conversions_card(ui, profiles, secs, false); }) };
    (conv, copied)
}
