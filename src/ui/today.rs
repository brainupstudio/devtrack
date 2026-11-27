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
    /// Tags chosen before Start (from the link under the task, or typed as #tag).
    pub tags: Vec<String>,
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
    pub tags: Vec<String>,
}

impl Past {
    pub fn has_input(&self) -> bool {
        !self.desc.trim().is_empty() || self.project.is_some() || self.task.is_some()
    }
}

impl Default for Past {
    fn default() -> Self {
        Past { desc: String::new(), project: None, task: None, billable: false, date: today().format("%Y-%m-%d").to_string(), start: "09:00".into(), end_date: today().format("%Y-%m-%d").to_string(), end: "10:00".into(), dur: "1:00:00".into(), dur_edited: false, errors: BTreeMap::new(), client: None, tags: vec![] }
    }
}

pub struct TodayState {
    /// Review pictures: where the pointer rests over the project bar (`--page project-bar`).
    pub shot_pointer: std::cell::Cell<Option<egui::Pos2>>,
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
    /// Where the compact window's session popover points: the row (or button) that opened it.
    pub edit_anchor: Option<egui::Rect>,
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
    /// Task groups of the list showing their sessions (day and group key).
    pub open_groups: std::collections::HashSet<Id>,
    /// Only the sessions with this tag (or with none), in the list and the calendar; the grid
    /// always shows everything, since its cells are typed totals.
    pub tag_filter: Option<TagFilter>,
    /// Sessions ticked in the list, to tag or untag together.
    pub selected: std::collections::BTreeSet<Id>,
}

impl TodayState {
    pub fn new(period: Period) -> Self {
        TodayState { shot_pointer: Default::default(), draft: Draft::default(), past: Past::default(), past_open: false, past_focused: false, period, timeline: false, timesheet: false, sheet_rows: vec![], sheet_focus: None, preview_task_menu: false, only_incomplete: false, edit: None, edit_anchor: None, run_desc: None, highlight: None, run_time: None, run_time_request: false, conv_request: false, focus_desc: false, views_in_summary: false, open_groups: Default::default(), tag_filter: None, selected: Default::default() }
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
pub(super) fn client_of(db: &Db, project: &Option<Id>) -> Option<Id> {
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
pub(super) fn settle_work(app: &mut App, task_pick: Pick, project_pick: Pick, client_pick: Pick, project: &mut Option<Id>, mut task: Option<&mut Option<Id>>, chosen: &mut Option<Id>) -> bool {
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
            // the task's own project when it is this client's, else the client's only project
            let task_project = task.as_deref().and_then(|t| db.task(t)).and_then(|x| x.project_id.clone());
            let own: Vec<&Project> = chosen.as_ref().map(|c| db.projects_of_client(c).into_iter().filter(|p| !p.archived).collect()).unwrap_or_default();
            *project = match task_project.filter(|tp| own.iter().any(|p| &p.id == tp)) {
                Some(tp) => Some(tp),
                None if own.len() == 1 => Some(own[0].id.clone()),
                None => None,
            };
            changed = true;
        }
    } else if changed && project.is_some() {
        *chosen = client_of(&app.store.db, project);
    }
    // A task belongs to one project: when the project changes to another one, the task goes,
    // and the notice says why. Nothing else in the form changes.
    if changed && !task_changed {
        if let Some(t) = task {
            if let Some(x) = app.store.db.task(t) {
                if x.project_id.is_some() && x.project_id != *project {
                    let owner = app.store.db.project(&x.project_id).map(|p| p.name.clone()).unwrap_or_default();
                    let title = truncate(&x.display(), 40);
                    *t = None;
                    app.notify(trf("“{}” belongs to {}: task removed", &[&title, &truncate(&owner, 40)]));
                }
            }
        }
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

/// What a tag picker's answer means for the tags it edits: a new name is created first (once,
/// spelled as the list will spell it) and added. True when the tags changed.
pub(super) fn tag_picked(app: &mut App, pick: Pick, tags: &mut Vec<String>) -> bool {
    match pick {
        Pick::Changed => true,
        Pick::Create(name) => match app.store.create_tag(&name) {
            Some(n) => {
                if !tags.iter().any(|t| t.eq_ignore_ascii_case(&n)) {
                    tags.push(n.clone());
                }
                app.notify(trf("Tag “{}” created", &[&truncate(&n, 40)]));
                true
            }
            None => false,
        },
        _ => false,
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

    // the period toolbar says which time is shown, and nothing else: how to look at it (the
    // views) sits with the total, just above what it changes
    let monday = app.store.db.settings.week_starts_monday;
    let mut per = period;
    // the views, as icons, in the bar's free slot on the right; on the summary row when it has none
    let views_w = segmented_icons_w(VIEWS.len());
    let views_here = super::period::status_fits(ui, views_w);
    app.today.views_in_summary = !views_here;
    let mut picked = None;
    let current = view_index(app);
    let bar = super::period::period_toolbar(ui, &mut per, monday, [true, true, true], if views_here { views_w } else { 0.0 }, |ui| {
        picked = segmented_icons(ui, &VIEWS, current);
    });
    super::period::record(app, "today", &bar);
    if let Some(i) = picked {
        set_view(app, i);
        if app.today.timesheet && per.kind != PeriodKind::Week {
            per = Period::containing(PeriodKind::Week, per.start, monday);
        }
        ui.ctx().request_repaint();
    }
    // the grid is a week: choosing it, or being on it, keeps the period a week
    if app.today.timesheet && per.kind != PeriodKind::Week {
        per = Period::containing(PeriodKind::Week, per.start, monday);
    }
    if per != period {
        app.today.period = per;
    }
    let period = app.today.period;
    ui.add_space(GROUP_GAP - ROW_GAP);
    let mut entries: Vec<TimeEntry> = entries_in(&app.store.db, &period, now).into_iter().cloned().collect();
    let incomplete = incomplete_in(app, &period);
    if app.store.db.live_entries().next().is_none() {
        ui.add_space(16.0);
        first_use(app, ui);
        return;
    }
    // the tag filter narrows everything below it (total, project bar, list, calendar) at once
    let filter = if app.today.timesheet { None } else { app.today.tag_filter.clone() };
    let unfiltered = entries.len();
    // the calendar keeps every session: one hidden by the filter would leave its hours looking
    // free, and a click or a drag there would overlap it. Those out of the filter are dimmed.
    let everything = entries.clone();
    let calendar = app.today.timeline && period.kind != PeriodKind::Month;
    if let Some(f) = &filter {
        // what still needs details is counted on the whole period: a filter must not hide work
        // to do
        entries.retain(|e| f.matches(e));
    }
    ui.add_space(12.0);
    ledger_summary(app, ui, &entries, &period, &incomplete);
    if let (Some(f), true, true, false) = (&filter, entries.is_empty(), unfiltered > 0, calendar) {
        ui.add_space(16.0);
        let (title, body) = match f {
            TagFilter::Tag(n) => (trf("No sessions with #{} in this period", &[n]), tr("Every session of this period has other tags. Show all sessions, or look at another period.")),
            TagFilter::Untagged => (tr("Every session in this period has a tag"), tr("Show all sessions, or look at another period.")),
        };
        let mut clear = false;
        empty_state(ui, Icon::Today, &title, &body, |ui| {
            clear = secondary(ui, "Show all sessions").clicked();
        });
        if clear { app.today.tag_filter = None; }
        return;
    }
    ui.add_space(8.0);
    project_bar(app, ui, &entries, &period);
    let shown: Vec<TimeEntry> = entries.iter().filter(|e| !e.is_running() && (!app.today.only_incomplete || needs_completion(e))).cloned().collect();
    if app.today.timesheet {
        egui::ScrollArea::both().id_salt("timesheet").auto_shrink([false, false]).show(ui, |ui| {
            timesheet(app, ui, &entries, &period);
        });
    } else if app.today.timeline && period.kind != PeriodKind::Month {
        timeline(app, ui, &everything, &period);
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
        let mut tag_pick = Pick::None;
        let mut tags = running.as_ref().map(|e| e.tags.clone()).unwrap_or_else(|| app.today.draft.tags.clone());
        ui.add_space(6.0);
        row(ui, 20.0, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(PAD_X);
            a = project_picker_in(ui, (salt, "project"), &app.store.db, &rp, &mut proj, Trigger::Link, chosen.as_ref());
            app.geometry.rows.push(("start row · project".into(), ui.min_rect()));
            ui.label(RichText::new("  ·  ").size(13.0).color(c.text2));
            cl = client_select_with(ui, (salt, "client"), &app.store.db, &mut chosen, Trigger::Link);
            ui.label(RichText::new("  ·  ").size(13.0).color(c.text2));
            tag_pick = tag_picker(ui, (salt, "tags"), &app.store.db, &mut tags, Trigger::Link);
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
        if tag_picked(app, tag_pick, &mut tags) {
            if running.is_some() {
                app.store.update_running(|x| x.tags = tags.clone());
            } else {
                app.today.draft.tags = tags;
            }
        }
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
/// Four ways to see the same sessions: every session, one line per task and day, the calendar,
/// the weekly grid. Icons, named in their tooltips: the switch stays narrow.
const VIEWS: [(Glyph, &str); 4] = [(Glyph::List, "List"), (Glyph::Group, "By task"), (Glyph::Calendar, "Timeline"), (Glyph::Grid, "Timesheet")];

fn view_index(app: &App) -> usize {
    if app.today.timesheet { 3 } else if app.today.timeline { 2 } else if app.store.local.list_by_task { 1 } else { 0 }
}

fn set_view(app: &mut App, i: usize) {
    app.today.timeline = i == 2;
    app.today.timesheet = i == 3;
    if i <= 1 && app.store.local.list_by_task != (i == 1) {
        app.store.local.list_by_task = i == 1;
        app.store.save_local();
    }
}
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
    /// A tag, for the word typed after # (the session gets it; the word leaves the text).
    Tag(String),
    /// A tag that doesn't exist yet: created when chosen.
    NewTag(String),
}

/// The text and the tag being typed at its end ("Login fix #fro" → ("Login fix", "fro")): the
/// last word, when it starts with # and isn't a work item's #number. None when there is none.
pub fn tag_token(text: &str) -> Option<(String, String)> {
    let t = text.trim_end();
    if text.ends_with(' ') && !t.is_empty() {
        return None;
    }
    let start = t.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    let word = &t[start..];
    let tag = word.strip_prefix('#')?;
    if !tag.is_empty() && tag.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((t[..start].trim_end().to_string(), tag.to_string()))
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
    // a #word at the end: tags only, the most recent first, then A–Z; a new name can be created
    if let Some((_, word)) = tag_token(q) {
        let w = word.to_lowercase();
        let recent = db.recent_tags(50);
        let rank = |n: &str| recent.iter().position(|r| r.eq_ignore_ascii_case(n)).unwrap_or(usize::MAX);
        let mut found: Vec<&Tag> = db.live_tags().into_iter().filter(|t| super::widgets::tag_matches(&t.name, &w)).collect();
        // names that start with the word before those that only contain it
        found.sort_by_key(|t| (!t.name.to_lowercase().starts_with(&w), rank(&t.name), t.name.to_lowercase()));
        let mut out: Vec<Suggest> = found.into_iter().take(8).map(|t| Suggest::Tag(t.name.clone())).collect();
        let clean = clean_tag(&word);
        // a near copy of a tag already there is not offered as new: that tag is in the list
        if !clean.is_empty() && db.tag_like(&clean).is_none() {
            out.push(Suggest::NewTag(clean));
        }
        return out;
    }
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
pub(super) fn description(app: &mut App, ui: &mut egui::Ui, running: Option<&TimeEntry>, width: f32) -> bool {
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
            // a #tag being typed is not part of the task's name
            let named = tag_token(&text).map(|(rest, _)| rest).unwrap_or_else(|| text.clone());
            if r.lost_focus() && named.trim() != current.trim() {
                // the name typed is the task: an existing one of the project, or a new one
                let name = named.trim().to_string();
                if !name.is_empty() {
                    let tid = app.task_named(e.project_id.clone(), &name);
                    app.store.update_running(|x| x.task_id = Some(tid.clone()));
                }
                app.today.run_desc = None;
            }
        }
        None => {
            if r.changed() {
                // typing another name leaves the chosen task: a new one is made on start (a
                // #tag typed after it doesn't count)
                let named = tag_token(&text).map(|(rest, _)| rest).unwrap_or_else(|| text.clone());
                if app.store.db.task(&app.today.draft.task).is_some_and(|t| t.display() != named.trim()) {
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
            (Suggest::Tag(name) | Suggest::NewTag(name), running) => {
                // the session gets the tag; the #word leaves the text, and typing goes on
                let name = if app.store.db.tag_named(&name).is_some() { Some(name) } else {
                    let n = app.store.create_tag(&name);
                    if let Some(n) = &n { app.notify(trf("Tag “{}” created", &[&truncate(n, 40)])); }
                    n
                };
                let rest = tag_token(&text).map(|(rest, _)| rest).unwrap_or_else(|| text.clone());
                if let Some(name) = name {
                    match running {
                        Some(e) => {
                            let mut tags = e.tags.clone();
                            if !tags.iter().any(|t| t.eq_ignore_ascii_case(&name)) { tags.push(name); }
                            app.store.update_running(|x| x.tags = tags.clone());
                            let current = app.store.db.task(&e.task_id).map(|t| t.display()).unwrap_or_else(|| e.description.clone());
                            app.today.run_desc = if rest.trim() == current.trim() { None } else { Some((e.id.clone(), rest.clone())) };
                        }
                        None => {
                            if !app.today.draft.tags.iter().any(|t| t.eq_ignore_ascii_case(&name)) { app.today.draft.tags.push(name); }
                            app.today.draft.desc = rest.clone();
                        }
                    }
                }
                app.today.focus_desc = true;
                r.request_focus();
            }
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
            // a #word being typed: Enter takes the first tag offered, never starts
            None if matches!(list.first(), Some(Suggest::Tag(..) | Suggest::NewTag(..))) => choice = list.first().cloned(),
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
                    let group = match s { Suggest::Task(..) => 0u8, Suggest::Remote(..) | Suggest::LookUp(..) | Suggest::Refresh => 1, Suggest::Project(..) => 2, Suggest::Tag(..) | Suggest::NewTag(..) => 3 };
                    if header != Some(group) {
                        if header.is_some() { ui.add_space(4.0); }
                        let (hr, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), Sense::hover());
                        let name = match group { 0 => tr("Tasks"), 1 => tr("From Azure DevOps · not in DevTrack yet"), 2 => tr("Projects"), _ => tr("Tags") };
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
                        Suggest::Tag(n) => pick_row(ui, hl == Some(i), None, &format!("#{}", n), "", false),
                        Suggest::NewTag(n) => pick_row(ui, hl == Some(i), None, &trf_m!("+ Create tag “{}”", truncate(n, 28)), "", false),
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
            Suggest::Tag(_) | Suggest::NewTag(_) => tag_token(&text).map(|(rest, _)| rest).unwrap_or_default(),
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
    let running = entries.iter().any(|e| e.is_running());
    let n = incomplete.len() as i64;
    // every session completed while only those were shown: back to the whole list
    if n == 0 && app.today.only_incomplete {
        app.today.only_incomplete = false;
    }
    // One row: what happened on the left (total, count, and the state of the sessions as words,
    // a button only when there is something to do), how to look at it on the right (the views).
    // A narrow window puts the views on their own row under it.
    let views_w = if app.today.views_in_summary { segmented_icons_w(VIEWS.len()) } else { 0.0 };
    let w = ui.available_width();
    let total = ui.painter().layout_job(mixed_job(&words(tracked), 20.0, c.text, medium(20.0), mono_medium(20.0)));
    let rest = ui.painter().layout_job(mixed_job(&trf_m!("tracked · {}", plural(done, "{} session", "{} sessions")), 14.0, c.text2, sans(14.0), mono_font(14.0)));
    // the state: (words, colour, an action when there is something to do)
    let state: Option<(String, Color32, Option<(&str, Glyph)>)> = if done == 0 && !app.today.only_incomplete {
        None
    } else if app.today.only_incomplete && !app.today.timeline {
        Some((format!("! {}", plural(n, "Showing {} session that needs details", "Showing {} sessions that need details")), c.amber, Some(("Show all sessions", Glyph::Close))))
    } else if n > 0 {
        Some((format!("! {}", plural(n, "{} session needs details", "{} sessions need details")), c.amber, if app.today.timeline { None } else { Some(("Show only these", Glyph::Next)) }))
    } else if running {
        Some(("● A session is still running".to_string(), c.text2, None))
    } else {
        Some(("✔ All sessions completed".to_string(), c.green, None))
    };
    let state_w = match &state {
        Some((t, _, Some(_))) => status_chip_w(ui, t, true) + GROUP_GAP,
        Some((t, _, None)) => ui.painter().layout_no_wrap(tr(t), sans(14.0), c.text).size().x + GROUP_GAP,
        None => 0.0,
    };
    let counts_w = if done == 0 && !app.today.only_incomplete { 0.0 } else { total.size().x + 8.0 + rest.size().x };
    // the tag filter sits on the right, with what it narrows: not on the grid (typed totals)
    let has_tags = !app.store.db.live_tags().is_empty();
    let filter_on = app.today.tag_filter.is_some();
    let show_filter = !app.today.timesheet && (has_tags || filter_on);
    let filter_w = if show_filter { TAG_FILTER_W + if filter_on { CTRL_H + 4.0 } else { 0.0 } + GROUP_GAP } else { 0.0 };
    let views_w = views_w + filter_w;
    let views_beside = counts_w + state_w + GROUP_GAP + views_w <= w;
    let state_beside = counts_w + state_w <= w;
    let baseline = |g: &egui::Galley| g.rows.first().and_then(|r| r.glyphs.first()).map(|gl| gl.pos.y).unwrap_or(g.size().y);
    let (b1, b2) = (baseline(&total), baseline(&rest));
    let mut clicked = false;
    let show_counts = counts_w > 0.0;
    ui.horizontal(|ui| {
        ui.set_min_height(CTRL_H);
        if show_counts {
            let h = total.size().y.max(b1 + rest.size().y - b2);
            let (r, _) = ui.allocate_exact_size(egui::vec2(counts_w, h), Sense::hover());
            ui.painter().galley(egui::pos2(r.left(), r.top()), total.clone(), c.text);
            ui.painter().galley(egui::pos2(r.right() - rest.size().x, r.top() + b1 - b2), rest.clone(), c.text2);
        }
        if let (Some((text, color, action)), true) = (&state, state_beside) {
            ui.add_space(GROUP_GAP - ui.spacing().item_spacing.x);
            clicked = state_line(ui, text, *color, *action);
        }
        if views_beside && (app.today.views_in_summary || show_filter) {
            // as high as the row: the tools never push the list down
            let room = egui::vec2(ui.available_width(), CTRL_H);
            ui.allocate_ui_with_layout(room, egui::Layout::right_to_left(egui::Align::Center), |ui| summary_tools(app, ui, show_filter));
        }
    });
    if let (Some((text, color, action)), false) = (&state, state_beside) {
        ui.add_space(ROW_GAP);
        clicked = state_line(ui, text, *color, *action);
    }
    if !views_beside && (app.today.views_in_summary || show_filter) {
        ui.add_space(ROW_GAP);
        let room = egui::vec2(ui.available_width(), CTRL_H);
        ui.allocate_ui_with_layout(room, egui::Layout::right_to_left(egui::Align::Center), |ui| summary_tools(app, ui, show_filter));
    }
    if app.today.timeline && period.kind == PeriodKind::Month {
        ui.add_space(ROW_GAP);
        meta(ui, "The timeline shows a day or a week; a month is shown as a list.");
    }
    if clicked {
        app.today.only_incomplete = !app.today.only_incomplete;
        // the sessions to complete may lie outside the tag filter: showing them lifts it
        if app.today.only_incomplete {
            app.today.tag_filter = None;
        }
    }
}

/// The state of the period's sessions beside the total: plain words when there is nothing to do
/// ("✔ All sessions completed"), the status button when there is. Returns its click.
fn state_line(ui: &mut egui::Ui, text: &str, color: Color32, action: Option<(&str, Glyph)>) -> bool {
    match action {
        Some(_) => status_chip_glyph(ui, text, color, action).clicked(),
        None => {
            ui.label(RichText::new(tr(text)).font(sans(14.0)).color(color));
            false
        }
    }
}

const TAG_FILTER_W: f32 = 168.0;

/// One step of a group's branch: the line down from `top`, across at `y` to `end`. With `more`
/// (where the next row starts) it goes on down; without, it is the last and turns with a
/// rounded corner. Shared by Today and the compact window, so both draw the same tree.
pub(super) fn branch(painter: &egui::Painter, x: f32, top: f32, y: f32, end: f32, more: Option<f32>, st: Stroke) {
    const R: f32 = 6.0;
    match more {
        Some(bottom) => {
            painter.line_segment([egui::pos2(x, top), egui::pos2(x, bottom)], st);
            painter.line_segment([egui::pos2(x, y), egui::pos2(end, y)], st);
        }
        None => {
            painter.line_segment([egui::pos2(x, top), egui::pos2(x, y - R)], st);
            let pts: Vec<egui::Pos2> = (0..=8).map(|i| {
                let t = std::f32::consts::FRAC_PI_2 * i as f32 / 8.0;
                egui::pos2(x + R - R * t.cos(), y - R + R * t.sin())
            }).collect();
            painter.add(egui::Shape::line(pts, st));
            painter.line_segment([egui::pos2(x + R, y), egui::pos2(end, y)], st);
        }
    }
}

/// What tells a session apart under its group: its note, without the task's name the group
/// already shows ("Migration: duplicates" under "Migration" reads "Duplicates"). None when the
/// note says nothing more than the task.
pub(super) fn nested_note(db: &Db, e: &TimeEntry) -> Option<String> {
    let note = e.description.trim();
    let task = db.task(&e.task_id).map(|t| t.title.trim().to_string()).unwrap_or_default();
    if note.is_empty() || note.eq_ignore_ascii_case(&task) || note == super::session_title(db, e) {
        return None;
    }
    let lower = note.to_lowercase();
    let rest = if !task.is_empty() && lower.starts_with(&task.to_lowercase()) {
        note[task.len()..].trim_start_matches([':', '-', '–', ',', ' ']).trim()
    } else {
        note
    };
    if rest.is_empty() {
        return None;
    }
    let mut ch = rest.chars();
    Some(ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default())
}

/// How far a session under an open group sits in from the group's own row.
const NEST_INDENT: f32 = 40.0;

/// In place of the list's header while sessions are ticked: a box for all of them, how many
/// are ticked, Add tag and Remove tag (each a menu that acts at once), and × to let them go.
fn selection_bar(app: &mut App, ui: &mut egui::Ui, hr: egui::Rect, ids: &std::collections::BTreeSet<Id>) {
    let c = p(ui);
    let n = app.today.selected.len();
    let all = n == ids.len();
    // the box sits where the rows have theirs
    let bx = egui::Rect::from_center_size(egui::pos2(hr.left() + 19.5, hr.center().y), egui::vec2(16.0, 16.0));
    let box_resp = ui.interact(bx.expand(6.0), ui.id().with("select-all"), Sense::click())
        .on_hover_text(tr(if all { "Clear selection" } else { "Select all sessions in the list" }));
    ui.painter().rect_filled(bx, 3.0, c.amber);
    if all {
        draw_glyph(ui.painter(), bx.center(), Glyph::Check, c.canvas);
    } else {
        ui.painter().hline(bx.left() + 4.0..=bx.right() - 4.0, bx.center().y, Stroke::new(1.8, c.canvas));
    }
    if box_resp.clicked() {
        app.today.selected = if all { Default::default() } else { ids.clone() };
    }
    // the tags on the ticked sessions, with how many carry each: what can be taken off
    let ticked: Vec<TimeEntry> = app.today.selected.iter().filter_map(|id| app.store.db.entries.get(id).cloned()).collect();
    let on: Vec<(String, Option<usize>)> = super::activities::tag_counts(&app.store.db, &ticked).into_iter().map(|(t, k)| (t, Some(k))).collect();
    let mut every: Vec<(String, Option<usize>)> = app.store.db.live_tags().into_iter().map(|t| (t.name.clone(), None)).collect();
    every.sort_by_key(|(t, _)| t.to_lowercase());
    let (mut add, mut remove, mut clear) = (None, None, false);
    let inner = egui::Rect::from_min_max(egui::pos2(hr.left() + 33.0, hr.top()), egui::pos2(hr.right() - 8.0, hr.bottom()));
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = GROUP_GAP;
        ui.label(RichText::new(plural(n as i64, "{} session selected", "{} sessions selected")).font(medium(13.0)).color(c.text));
        let a = link_btn(ui, "Add tag");
        app.geometry.rows.push(("selection · add tag".into(), a.rect));
        add = tag_menu(ui, ui.id().with("bulk-add"), &a, &every, true);
        if !on.is_empty() {
            let r = link_btn(ui, "Remove tag");
            app.geometry.rows.push(("selection · remove tag".into(), r.rect));
            remove = tag_menu(ui, ui.id().with("bulk-remove"), &r, &on, false);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            clear = icon_ghost(ui, Glyph::Close, "Clear selection").clicked();
        });
    });
    if clear {
        app.today.selected.clear();
    }
    if add.is_some() || remove.is_some() {
        let list: Vec<Id> = app.today.selected.iter().cloned().collect();
        let (changed, locked) = app.store.tag_entries(&list, add.as_deref(), remove.as_deref());
        let name = add.as_ref().or(remove.as_ref()).map(|t| app.store.db.tag_named(t).map(|x| x.name.clone()).unwrap_or(t.clone())).unwrap_or_default();
        let mut msg = match (&add, changed) {
            (_, 0) => tr("No session changed: they already were as asked"),
            (Some(_), 1) => trf("#{} added to 1 session", &[&name]),
            (Some(_), k) => trf("#{} added to {} sessions", &[&name, &k]),
            (None, 1) => trf("#{} taken off 1 session", &[&name]),
            (None, k) => trf("#{} taken off {} sessions", &[&name, &k]),
        };
        if locked > 0 {
            msg = format!("{} · {}", msg, plural(locked as i64, "1 in a locked period left as it was", "{} in a locked period left as they were"));
        }
        if changed > 0 { app.notify_undo(msg); } else { app.notify(msg); }
    }
}

/// The #tag under the pointer in a session's grey line, underlined, or None. `before` is the
/// text ahead of the first tag; tags cut off by the line's end do not count.
fn hovered_tag(ui: &egui::Ui, label: &egui::Response, before: &str, tags: &[String], color: Color32) -> Option<String> {
    let pos = ui.ctx().pointer_hover_pos()?;
    // the text is 12 px: the target reaches a little above, below and beside each tag
    if tags.is_empty() || !label.rect.expand2(egui::vec2(0.0, 6.0)).contains(pos) {
        return None;
    }
    let width = |t: &str| ui.painter().layout_no_wrap(t.to_string(), sans(12.0), color).size().x;
    let mut text = before.to_string();
    for (i, t) in tags.iter().enumerate() {
        if i > 0 { text.push(' '); }
        let x0 = label.rect.left() + width(&text);
        text.push_str(t);
        let x1 = label.rect.left() + width(&text);
        if x1 > label.rect.right() {
            return None;
        }
        // half the space between two tags on each side, so neighbours never overlap
        let pad = width(" ") / 2.0;
        if pos.x >= x0 - pad && pos.x <= x1 + pad {
            let y = label.rect.center().y + 7.0;
            ui.painter().hline(x0..=x1, y, Stroke::new(1.0, color));
            return Some(t.clone());
        }
    }
    None
}

/// The right end of the summary row, from the right: the views (when the period bar has no room
/// for them), then the tag filter with its clear button while it is on.
fn summary_tools(app: &mut App, ui: &mut egui::Ui, show_filter: bool) {
    if app.today.views_in_summary {
        summary_views(app, ui);
        ui.add_space(GROUP_GAP - ui.spacing().item_spacing.x);
    }
    if show_filter {
        if app.today.tag_filter.is_some() && icon_button(ui, Glyph::Close, "Show all sessions").clicked() {
            app.today.tag_filter = None;
        }
        let mut f = app.today.tag_filter.clone();
        let r = ui.scope(|ui| tag_filter(ui, "today-tag-filter", &app.store.db, &mut f, TAG_FILTER_W)).inner;
        if r {
            app.today.tag_filter = f;
        }
    }
}

/// List · By task · Calendar · Timesheet on the summary row, when the period bar has no room.
fn summary_views(app: &mut App, ui: &mut egui::Ui) {
    if let Some(i) = segmented_icons(ui, &VIEWS, view_index(app)) {
        set_view(app, i);
        // the grid is a week: choosing it keeps the period a week
        if app.today.timesheet && app.today.period.kind != PeriodKind::Week {
            app.today.period = Period::containing(PeriodKind::Week, app.today.period.start, app.store.db.settings.week_starts_monday);
        }
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
    // ticked sessions: only those still in the list count; Esc lets them all go
    let ids: std::collections::BTreeSet<Id> = entries.iter().filter(|e| !e.is_running()).map(|e| e.id.clone()).collect();
    app.today.selected.retain(|id| ids.contains(id));
    if !app.today.selected.is_empty() && ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.memory(|m| m.any_popup_open()) {
        app.today.selected.clear();
    }
    if app.today.selected.is_empty() {
        ui.painter().text(egui::pos2(hr.left() + 14.0, hr.center().y), egui::Align2::LEFT_CENTER, tr(if narrow { "Activity" } else { "Activity / project" }), sans(12.0), c.text2);
        if time_w > 0.0 {
            ui.painter().text(egui::pos2(hr.left() + x_time, hr.center().y), egui::Align2::LEFT_CENTER, tr("Time"), sans(12.0), c.text2);
        }
        ui.painter().text(egui::pos2(hr.left() + x_time + time_span, hr.center().y), egui::Align2::LEFT_CENTER, tr("Duration"), sans(12.0), c.text2);
    } else {
        // the table's header becomes the bar for what was ticked: nothing moves
        selection_bar(app, ui, hr, &ids);
    }
    let mut action: Option<(u8, TimeEntry)> = None;
    let mut copied: Option<String> = None;
    // grouped by task, every day gets its band: it carries the copy of the day's totals
    let by_task = app.store.local.list_by_task;
    let multi_day = by_day.len() > 1 || by_task;
    // rows are exactly 64 high: no extra spacing between a row and its divider
    ui.spacing_mut().item_spacing.y = 0.0;
    // one session's row (also a group's sessions when it is open)
    // `nest`: the session hangs from an open group — (the tags said once on the group, whether
    // it is the group's last). It then says only what is its own: its note, the tags the others
    // lack, its times; indented under the group's title, joined to it by a line.
    let draw_session = |app: &mut App, ui: &mut egui::Ui, e: &TimeEntry, a: &i64, b: &i64, action: &mut Option<(u8, TimeEntry)>, copied: &mut Option<String>, nest: Option<(&[String], bool)>| {
        let nested = nest.is_some();
        let shared: &[String] = nest.map(|n| n.0).unwrap_or(&[]);
        let mw = if nested { main_w - NEST_INDENT } else { main_w };
        let own_line = nested && missing_text(e).is_empty() && session_observations(&e.id, &fl).is_empty() && e.tags.iter().all(|t| shared.iter().any(|x| x.eq_ignore_ascii_case(t)));
        let mut action_rect = egui::Rect::NOTHING;
        let mut buttons_left = 0.0_f32;
        let mut tag_hit: Option<String> = None;
        let mut box_hit = false;
        let selecting = !app.today.selected.is_empty();
        let ticked = app.today.selected.contains(&e.id);
        let over_row = ui.ctx().read_response(egui::Id::new(("row", &e.id, *a))).is_some_and(|x| x.hovered());
        let show_box = selecting || over_row || ticked;
        let mut menu_tick = false;
        let row = ui.horizontal(|ui| {
            ui.set_min_height(64.0);
            ui.add_space(if nested { 14.0 + NEST_INDENT } else { 14.0 });
            col(ui, mw, 60.0, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                // a nested session with nothing of its own to add sits on the row's middle
                ui.add_space(if own_line { 21.0 } else { 12.0 });
                ui.horizontal(|ui| {
                    // the project's dot; while sessions are being picked (or under the pointer)
                    // a check box in its place
                    // nested: no dot of its own; its box sits on the group's line, in the column
                    // where every row has its box
                    let r = if nested {
                        let at = ui.cursor().min;
                        egui::Rect::from_center_size(egui::pos2(at.x - NEST_INDENT + 5.5, at.y + 9.0), egui::vec2(11.0, 9.0))
                    } else {
                        ui.allocate_exact_size(egui::vec2(11.0, 9.0), Sense::hover()).0
                    };
                    let painter = ui.painter().with_clip_rect(ui.ctx().screen_rect());
                    if show_box {
                        let bx = egui::Rect::from_center_size(r.center(), egui::vec2(16.0, 16.0));
                        let hot = egui::Rect::from_center_size(r.center(), egui::vec2(28.0, 28.0));
                        box_hit = ui.ctx().pointer_hover_pos().is_some_and(|p| hot.contains(p));
                        if ticked {
                            painter.rect_filled(bx, 3.0, c.amber);
                            draw_glyph(&painter, bx.center(), Glyph::Check, c.canvas);
                        } else {
                            painter.rect_filled(bx, 3.0, c.canvas);
                            painter.rect_stroke(bx, 3.0, Stroke::new(1.2, if box_hit { c.text } else { c.text2 }), egui::StrokeKind::Inside);
                        }
                    } else if !nested {
                        // nested: the group's dot stands for it
                        match app.store.db.project(&e.project_id).map(|p| hex(&p.color)) {
                            Some(col_) => { ui.painter().circle_filled(r.center(), 3.5, col_); }
                            None => { ui.painter().circle_stroke(r.center(), 3.5, Stroke::new(1.2, c.amber)); }
                        }
                    }
                    if nested {
                        // the group names the task; a session under it is told apart by its note
                        let (t, col_) = match nested_note(&app.store.db, e) {
                            Some(n) => (n, c.text),
                            None => (tr("No note"), c.text2),
                        };
                        ui.add(egui::Label::new(RichText::new(t).font(sans(14.0)).color(col_)).truncate());
                    } else {
                        let title_txt = super::session_title(&app.store.db, e);
                        ui.add(egui::Label::new(RichText::new(title_txt).font(medium(14.0))).truncate());
                    }
                });
                let observations = session_observations(&e.id, &fl);
                let miss = missing_text(e);
                // the tags close the line; each one filters the list to its sessions
                let tag_names: Vec<String> = if miss.is_empty() {
                    e.tags.iter().filter(|t| !shared.iter().any(|x| x.eq_ignore_ascii_case(t))).map(|t| format!("#{}", t)).collect()
                } else { vec![] };
                let mut before_tags = String::new();
                let context = if !miss.is_empty() {
                    tr(miss)
                } else {
                    // nested: project, client and note are on the group or in the title
                    let mut parts = if nested { vec![] } else { vec![super::session_context(&app.store.db, e)] };
                    parts.retain(|p| !p.is_empty());
                    if !tag_names.is_empty() {
                        before_tags = if parts.is_empty() { String::new() } else { format!("{} · ", parts.join(" · ")) };
                        parts.push(tag_names.join(" "));
                    }
                    parts.join(" · ")
                };
                let summary = if observations.is_empty() { context.clone() } else {
                    let count = if observations.len() == 1 { observations[0].clone() } else { plural(observations.len() as i64, "{} observation", "{} observations") };
                    if context.is_empty() { count } else { format!("{} · {}", context, count) }
                };
                let full = std::iter::once(context).chain(observations.iter().cloned()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n");
                // One bounded metadata line: arbitrary warning counts never grow this cell.
                let rect = ui.available_rect_before_wrap();
                ui.set_clip_rect(ui.clip_rect().intersect(egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() - 64.0), egui::pos2(rect.left() + mw, rect.bottom()))));
                // The whole row opens the editor (with the checks), so no separate Review link.
                if !own_line { ui.allocate_ui_with_layout(egui::vec2(mw, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_width(mw);
                    // the #tags, which filter on a click, a step brighter than the words around them:
                    // what can be clicked is told apart without a colour
                    let line: egui::WidgetText = if tag_names.is_empty() || !miss.is_empty() {
                        RichText::new(summary).size(12.0).color(if miss.is_empty() { c.text2 } else { c.amber }).into()
                    } else {
                        let tags_txt = tag_names.join(" ");
                        let rest = summary.get(before_tags.len() + tags_txt.len()..).unwrap_or("").to_string();
                        let fmt = |col| egui::TextFormat { font_id: sans(12.0), color: col, ..Default::default() };
                        let mut j = egui::text::LayoutJob::default();
                        j.append(&before_tags, 0.0, fmt(c.text2));
                        j.append(&tags_txt, 0.0, fmt(c.text2.lerp_to_gamma(c.text, 0.45)));
                        j.append(&rest, 0.0, fmt(c.text2));
                        egui::WidgetText::LayoutJob(j)
                    };
                    let lr = ui.add(egui::Label::new(line).truncate());
                    tag_hit = hovered_tag(ui, &lr, &before_tags, &tag_names, c.text2);
                    match &tag_hit {
                        Some(t) => { lr.on_hover_text(trf("Show only the sessions with {}", &[t])); }
                        None => { lr.on_hover_text(full); }
                    }
                }); }
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
                        *copied = Some(v);
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
                *action = Some((0, e.clone()));
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
                    *action = Some((0, e.clone()));
                }
                if menu_row(ui, "Edit…", false, false).clicked() {
                    *action = Some((1, e.clone()));
                }
                // the way into ticking several, also from the keyboard (Space ticks the row)
                if menu_row(ui, if ticked { "Deselect" } else { "Select" }, false, false).on_hover_text(tr("Tick it, to tag several sessions at once (Space on the row)")).clicked() {
                    menu_tick = true;
                }
                // the task's own page: source data (Azure DevOps) and every session on it
                if app.store.db.task(&e.task_id).is_some() && menu_row(ui, "Open task", false, false).clicked() {
                    *action = Some((3, e.clone()));
                }
                if menu_row(ui, "Duplicate", false, false).on_hover_text(tr("A copy with the same times, opened for editing")).clicked() {
                    *action = Some((4, e.clone()));
                }
                if e.duration(now) >= 120 && menu_row(ui, "Split in two", false, false).on_hover_text(tr("Two sessions meeting at the midpoint; adjust the time afterwards")).clicked() {
                    *action = Some((5, e.clone()));
                }
                let fav = favorite_of(e);
                let is_fav = app.store.local.favorites.contains(&fav);
                if menu_row(ui, if is_fav { "Remove from favorites" } else { "Add to favorites" }, false, false).clicked() {
                    *action = Some((6, e.clone()));
                }
                if menu_row(ui, "Delete", false, false).clicked() {
                    *action = Some((2, e.clone()));
                }
            });
        });
        // the branch to the group: down from the group's dot, then across to this session; the
        // last one ends the line
        if let Some((_, last)) = nest {
            let r = row.response.rect;
            let x = r.left() + 19.5;
            let y = r.top() + if own_line { 30.0 } else { 21.0 };
            let end = r.left() + 14.0 + NEST_INDENT - 6.0;
            let st = Stroke::new(1.5, c.border);
            if show_box {
                // the box takes the joint: the line stops short of it on both sides
                ui.painter().line_segment([egui::pos2(x, r.top()), egui::pos2(x, y - 11.0)], st);
                if !last { ui.painter().line_segment([egui::pos2(x, y + 11.0), egui::pos2(x, r.bottom())], st); }
            } else {
                branch(ui.painter(), x, r.top(), y, end, if last { None } else { Some(r.bottom()) }, st);
            }
        }
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
        // a click on a #tag filters by it; anywhere else on the row opens the editor
        // while picking, the whole row ticks; the box, ⌘/Ctrl-click, Space on the focused row and
        // the menu's Select tick at any time (Enter still opens it)
        let space = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Space));
        let toggle = menu_tick || space || box_hit || (resp.clicked() && (selecting || ui.input(|i| i.modifiers.command)));
        if resp.clicked() || space || menu_tick {
            if toggle {
                if !app.today.selected.remove(&e.id) {
                    app.today.selected.insert(e.id.clone());
                }
            } else {
                match &tag_hit {
                    Some(t) => app.today.tag_filter = Some(TagFilter::Tag(t.trim_start_matches('#').to_string())),
                    None => *action = Some((1, e.clone())),
                }
            }
        }
        // inside a group the divider steps in to the nested edge, until the last session
        match nest {
            Some((_, last)) => nest_divider(ui, !last),
            None => divider(ui),
        }
    };
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
        if by_task {
            for g in super::by_task::group(&list) {
                if g.pieces.len() == 1 {
                    let (e, a, b) = &g.pieces[0];
                    draw_session(app, ui, e, a, b, &mut action, &mut copied, None);
                    continue;
                }
                let gid = super::by_task::group_id(date, &g);
                let open = app.today.open_groups.contains(&gid);
                let lead = g.latest().clone();
                let (toggle, resume, conv_copy) = group_row(app, ui, &g, open, &profiles, GroupCols { main_w, time_w, dur_w, has_conv, play_btn });
                if let Some(v) = conv_copy {
                    copied = Some(v);
                }
                if resume {
                    action = Some((0, lead));
                }
                if toggle {
                    if open { app.today.open_groups.remove(&gid); } else { app.today.open_groups.insert(gid.clone()); }
                }
                if open {
                    // the tags on every session of the group are on its row already
                    let shared = group_shared_tags(&app.store.db, &g);
                    let n = g.pieces.len();
                    for (i, (e, a, b)) in g.pieces.iter().enumerate() {
                        draw_session(app, ui, e, a, b, &mut action, &mut copied, Some((&shared, i + 1 == n)));
                    }
                }
            }
            continue;
        }
        for (e, a, b) in &list {
            draw_session(app, ui, e, a, b, &mut action, &mut copied, None);
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

/// The columns of the list, shared by a group's line and its sessions.
struct GroupCols {
    main_w: f32,
    time_w: f32,
    dur_w: f32,
    has_conv: bool,
    play_btn: bool,
}

/// One task's line on a day: its sessions' total, from the first start to the last end, the
/// conversions of the total and Continue; the chevron (or a click on the line) shows its
/// sessions under it. Returns (toggle, continue, a conversion value to copy).
/// The tags on a group's sessions with how many carry each, most used first.
pub(super) fn group_tag_counts(db: &Db, g: &super::by_task::TaskGroup) -> Vec<(String, usize)> {
    let mut es: Vec<TimeEntry> = vec![];
    for (e, _, _) in &g.pieces {
        if !es.iter().any(|x| x.id == e.id) { es.push(e.clone()); }
    }
    super::activities::tag_counts(db, &es)
}

/// The tags every session of a group carries: said on the group's row, not on each session.
fn group_shared_tags(db: &Db, g: &super::by_task::TaskGroup) -> Vec<String> {
    let n = g.pieces.iter().map(|p| &p.0.id).collect::<std::collections::BTreeSet<_>>().len();
    group_tag_counts(db, g).into_iter().filter(|(_, k)| *k == n).map(|(t, _)| t).collect()
}

fn group_row(app: &mut App, ui: &mut egui::Ui, g: &super::by_task::TaskGroup, open: bool, profiles: &[TimesheetProfile], cols: GroupCols) -> (bool, bool, Option<String>) {
    let c = p(ui);
    let e = g.latest().clone();
    let mut toggle = false;
    let mut resume = false;
    let mut copied = None;
    let mut buttons_left = 0.0_f32;
    let n = g.pieces.len() as i64;
    let row = ui.horizontal(|ui| {
        ui.set_min_height(64.0);
        ui.add_space(14.0);
        col(ui, cols.main_w, 60.0, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if let Some(col_) = app.store.db.project(&e.project_id).map(|p| hex(&p.color)) {
                    dot(ui, col_, 3.5);
                }
                ui.add(egui::Label::new(RichText::new(super::session_title(&app.store.db, &e)).font(medium(14.0))).truncate());
            });
            // project and client only: a note belongs to one session, not to the group
            let context = app.store.db.project(&e.project_id).map(|p| {
                let client = app.store.db.client_name(p);
                if client.is_empty() { p.name.clone() } else { format!("{} · {}", p.name, client) }
            }).unwrap_or_default();
            // how many sessions is said in the time column
            let mut line = context;
            // the group's tags once, most used first
            let counts = group_tag_counts(&app.store.db, g);
            if !counts.is_empty() {
                let t = super::activities::tag_summary(&counts);
                line = if line.is_empty() { t } else { format!("{} · {}", line, t) };
            }
            ui.allocate_ui_with_layout(egui::vec2(cols.main_w, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(cols.main_w);
                // the tags behind "+N", on the pointer, in place of egui's own tip for a cut line
                let hover = super::activities::all_tags_hover(&line, &counts);
                cut_line(ui, mixed_job(&line, 12.0, c.text2, sans(12.0), mono_font(12.0)), hover);
            });
        });
        if cols.time_w > 0.0 {
            col(ui, cols.time_w, 60.0, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                // no times: first start to last end would read as one stretch of work, and the
                // sessions, opened, show their own
                ui.add_space(21.0);
                mixed(ui, &plural(n, "{} session", "{} sessions"), 13.0, Some(c.text2));
            });
        }
        col(ui, cols.dur_w, 60.0, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add_space(if cols.time_w == 0.0 { 12.0 } else { 21.0 });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(mono_m(super::shown_duration(&app.store.db, g.total), 14.0));
                if g.running {
                    dot(ui, c.green, 3.0);
                }
            });
            if cols.time_w == 0.0 {
                mixed(ui, &plural(n, "{} session", "{} sessions"), 12.0, Some(c.text2));
            }
        });
        ui.add_space(4.0);
        let (conv, play, more) = ui.vertical(|ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let conv = cols.has_conv.then(|| icon_ghost_quiet(ui, Glyph::Convert, "Conversions"));
                let play = cols.play_btn.then(|| icon_ghost(ui, Glyph::Play, "Continue now"));
                let more = icon_button(ui, if open { Glyph::Down } else { Glyph::Next }, if open { "Hide the sessions" } else { "Show the sessions" });
                (conv, play, more)
            }).inner
        }).inner;
        buttons_left = conv.as_ref().or(play.as_ref()).map(|r| r.rect.left()).unwrap_or(more.rect.left());
        if let Some(conv) = conv {
            let cid = ui.make_persistent_id(("group-conv", &g.key, g.first));
            if conv.clicked() {
                ui.memory_mut(|m| m.toggle_popup(cid));
            }
            let shown = ui.memory(|m| m.is_popup_open(cid));
            egui::popup_below_widget(ui, cid, &conv, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                if let Some(v) = conversions_card(ui, profiles, g.total, true) {
                    copied = Some(v);
                    ui.memory_mut(|m| m.close_popup());
                }
            });
            if !shown {
                conv.on_hover_ui(|ui| {
                    conversions_card(ui, profiles, g.total, false);
                });
            }
        }
        resume = play.is_some_and(|r| r.clicked());
        toggle = more.clicked();
    });
    let click_rect = egui::Rect::from_min_max(row.response.rect.min, egui::pos2(buttons_left - 4.0, row.response.rect.bottom()));
    let resp = ui.interact(click_rect, egui::Id::new(("group-row", &g.key, g.first)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, super::session_title(&app.store.db, &e)));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    toggle |= resp.clicked();
    if open {
        // its sessions hang from its dot: the line starts here, and the divider steps in to them
        let r = row.response.rect;
        ui.painter().line_segment([egui::pos2(r.left() + 19.5, r.bottom() - 6.0), egui::pos2(r.left() + 19.5, r.bottom())], Stroke::new(1.5, c.border));
        nest_divider(ui, true);
    } else {
        divider(ui);
    }
    (toggle, resume, copied)
}

/// A divider under a row of an open group: it starts at the nested sessions' edge, so the group
/// reads as one block, and the branch line passes through it. `more`: another nested row follows.
fn nest_divider(ui: &mut egui::Ui, more: bool) {
    if !more {
        divider(ui);
        return;
    }
    let c = p(ui);
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(r.left() + 14.0 + NEST_INDENT, r.top()), r.max), 0.0, c.line);
    ui.painter().line_segment([egui::pos2(r.left() + 19.5, r.top()), egui::pos2(r.left() + 19.5, r.bottom())], Stroke::new(1.5, c.border));
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
    // The line stays thin, but the whole band reacts to the pointer: the names over it and a
    // margin under it, so it's easy to hit.
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 34.0), Sense::hover());
    let line_y = rect.top() + 22.0;
    let mut x = rect.left();
    let mut segs: Vec<(f32, f32, String, egui::Color32, i64)> = vec![];
    for (pid, secs) in &by {
        let pr = app.store.db.project(pid);
        let name = pr.map(|p| p.name.clone()).unwrap_or(tr("No project"));
        let color = pr.map(|p| hex(&p.color)).unwrap_or(c.text2);
        let seg = w * (*secs as f32 / total as f32);
        segs.push((x, x + seg, name, color, *secs));
        x += seg;
    }
    if super::SHOT_POINTER.load(std::sync::atomic::Ordering::Relaxed) {
        app.today.shot_pointer.set(Some(egui::pos2(rect.left() + w * 0.62, line_y)));
    }
    let hovered = resp.hover_pos().and_then(|pos| segs.iter().position(|s| pos.x >= s.0 && pos.x < s.1));
    for (i, (x0, x1, name, color, _)) in segs.iter().enumerate() {
        // 2 px between segments so two similar colours still read as two; the one under the
        // pointer a little thicker
        let grow = if hovered == Some(i) { 1.5 } else { 0.0 };
        let r = egui::Rect::from_min_max(egui::pos2(*x0, line_y - grow), egui::pos2((x1 - 2.0).max(x0 + 1.0), line_y + 3.0 + grow));
        ui.painter().rect_filled(r, 1.5, *color);
        let text_color = if hovered == Some(i) { c.text } else { c.text2 };
        let g = ui.painter().layout_no_wrap(name.clone(), sans(12.0), text_color);
        if g.size().x + 8.0 <= x1 - x0 {
            ui.painter().galley(egui::pos2(*x0, rect.top()), g, text_color);
        }
    }
    // the list of projects opens under the segment you are on, with its row picked out
    if let Some(h) = hovered {
        let tip_w = 280.0f32;
        let screen = ui.ctx().screen_rect();
        let left = segs[h].0.max(rect.left()).min(screen.right() - tip_w - 8.0).max(screen.left() + 8.0);
        egui::Area::new(ui.id().with("project-bar-tip"))
            .order(egui::Order::Tooltip)
            .fixed_pos(egui::pos2(left, line_y + 10.0))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(tip_w - 16.0);
                    for (i, (_, _, name, color, secs)) in segs.iter().enumerate() {
                        let on = i == h;
                        ui.horizontal(|ui| {
                            let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 4.0, *color);
                            let name_text = RichText::new(truncate(name, 28)).size(13.0);
                            ui.label(if on { name_text.color(c.text).font(semibold(13.0)) } else { name_text.color(c.text2) });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let v = RichText::new(format!("{} · {}%", words(*secs), secs * 100 / total)).size(13.0);
                                ui.label(if on { v.color(c.text) } else { v.color(c.text2) });
                            });
                        });
                    }
                });
            });
    }
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
    let mut days: Vec<NaiveDate> = period.days().into_iter().filter(|d| app.store.db.settings.is_work_day(*d) || worked.contains(d)).collect();
    // no working day in the week (all set as days off): show them all rather than no column
    if days.is_empty() {
        days = period.days();
    }
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
    // a day's header and total start where the hours start inside its cells: the field's own
    // padding in from the left edge, so a column reads down one line
    let day_col = |ui: &mut egui::Ui, width: f32, add: &mut dyn FnMut(&mut egui::Ui)| {
        ui.allocate_ui_with_layout(egui::vec2(width, CTRL_H), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_width(width); ui.add_space(PAD_X); add(ui); });
    };
    // header
    row(ui, 28.0, |ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        ui.allocate_ui_with_layout(egui::vec2(name_w, CTRL_H), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_width(name_w); label(ui, "Project"); });
        for d in &days {
            let text = format!("{} {}", super::i18n::weekday_short(*d), d.format("%d"));
            day_col(ui, cell_w, &mut |ui| { ui.label(RichText::new(&text).size(12.0).color(if *d == today() { c.text } else { c.text2 })); });
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
            day_col(ui, cell_w, &mut |ui| { ui.label(mono(if sum == 0 { "—".to_string() } else { cell_text(sum) }, 13.0).color(if sum == 0 { c.text2 } else { c.text })); });
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
    let dim = app.today.tag_filter.clone();
    timeline_in(app, ui, entries, period, true, dim.as_ref());
}

/// The calendar: the sessions as blocks on the hours of each day. `header` draws each day's name
/// and total above its column (the compact window has its own day row instead).
/// `dim`: the tag filter on Today; sessions it leaves out are drawn faint but still there.
pub(super) fn timeline_in(app: &mut App, ui: &mut egui::Ui, entries: &[TimeEntry], period: &Period, header: bool, dim: Option<&TagFilter>) {
    let now = now_secs();
    let c = p(ui);
    let fmt = app.store.db.settings.duration_format;
    // days off are left out, unless a session falls on them
    let busy: std::collections::BTreeSet<NaiveDate> = entries.iter().flat_map(|e| day_pieces(e, now).into_iter().map(|x| x.0)).collect();
    let mut days: Vec<NaiveDate> = period.days().into_iter().filter(|d| app.store.db.settings.is_work_day(*d) || busy.contains(d)).collect();
    // a day off on its own (Day view on a Sunday) is still shown: there is always a column
    if days.is_empty() {
        days = period.days();
    }
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
    let head_h = if header { 38.0 } else { 0.0 };
    let room = ui.available_height() - head_h - 16.0;
    let hours = (last_h - first_h).max(1) as f32;
    let hour_h = if room.is_finite() && room > 0.0 && room < 10_000.0 { (room / hours).clamp(48.0, 140.0).floor() } else { 48.0 };
    let gutter = 52.0;
    let mut clicked: Option<(TimeEntry, egui::Rect)> = None;
    let mut create_at: Option<(NaiveDate, i64)> = None;
    let mut dragged: Option<(TimeEntry, i64, i64)> = None;
    let mut refused_future = false;
    let total_w = ui.available_width();
    let col_w = ((total_w - gutter - 8.0) / days.len() as f32).max(60.0);
    let (hr, _) = ui.allocate_exact_size(egui::vec2(gutter + col_w * days.len() as f32, head_h), Sense::hover());
    for (i, d) in days.iter().enumerate().filter(|_| header) {
        let x = hr.left() + gutter + i as f32 * col_w + 6.0;
        // with a filter, the day's total is the filter's, as the summary above says
        let tot: i64 = pieces.iter().filter(|x| x.0 == *d && dim.is_none_or(|f| f.matches(&x.1))).map(|x| x.3 - x.2).sum();
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
                if dim.is_some_and(|f| !f.matches(e)) {
                    // out of the filter: a veil of the page's colour over the whole block. Square and
                    // a hair larger: a rounded one left the colour bar's ends uncovered, and its
                    // corners fall on the page's own colour anyway
                    painter.rect_filled(r.expand(0.5), 0.0, c.canvas.gamma_multiply(0.7));
                }
                let resp = ui.interact(r, egui::Id::new(("tl", &e.id, *d)), Sense::click_and_drag());
                if resp.clicked() {
                    clicked = Some((e.clone(), r));
                }
                // Drag the block to move it, its upper edge to change its start, its lower edge
                // to change its end, in the calendar step. Only a finished session wholly inside
                // this day, outside the locked days.
                let movable = !e.is_running() && *a == e.start && Some(*b) == e.stop && !report::locked(&app.store.db, e.start);
                let drag_id = egui::Id::new("tl-drag");
                if movable {
                    // 0 moves, 1 the end, 2 the start; the edges are 8 px bands, never more than a
                    // third of a short block so it can still be moved
                    let edge = 8.0f32.min(r.height() / 3.0);
                    let part_at = |p: egui::Pos2| if p.y > r.bottom() - edge { 1u8 } else if p.y < r.top() + edge { 2 } else { 0 };
                    if let Some(p) = resp.hover_pos() {
                        ui.ctx().set_cursor_icon(if part_at(p) == 0 { egui::CursorIcon::Grab } else { egui::CursorIcon::ResizeVertical });
                    }
                    if resp.drag_started() {
                        // where the button went down, not where the pointer is once the drag is
                        // recognised (a few pixels on, often past an 8 px edge already); the
                        // distance covered so far counts too
                        let origin = ui.input(|i| i.pointer.press_origin());
                        let part = origin.map(part_at).unwrap_or(0);
                        let so_far = match (origin, ui.input(|i| i.pointer.interact_pos())) {
                            (Some(o), Some(p)) => p.y - o.y - resp.drag_delta().y,
                            _ => 0.0,
                        };
                        ui.data_mut(|x| x.insert_temp(drag_id, (e.id.clone(), part, so_far)));
                    }
                    let state = ui.data(|x| x.get_temp::<(Id, u8, f32)>(drag_id)).filter(|st| st.0 == e.id);
                    if let Some((_, part, dy)) = state {
                        let dy = dy + resp.drag_delta().y;
                        let step = app.store.db.settings.calendar_step.max(1) as f32;
                        let mins = ((dy / hour_h * 60.0) / step).round() as i64 * step as i64;
                        let shift = mins as f32 / 60.0 * hour_h;
                        // preview: where it will land, with the new times
                        let preview = match part {
                            1 => egui::Rect::from_min_max(r.min, egui::pos2(r.right(), (r.bottom() + shift).max(r.top() + 6.0))),
                            2 => egui::Rect::from_min_max(egui::pos2(r.left(), (r.top() + shift).min(r.bottom() - 6.0)), r.max),
                            _ => r.translate(egui::vec2(0.0, shift)),
                        };
                        let fg = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("tl-preview")));
                        fg.rect(preview, 5.0, c.focus.linear_multiply(0.15), Stroke::new(1.5, c.focus), egui::StrokeKind::Inside);
                        // a session keeps at least five minutes
                        let (ns, ne) = match part {
                            1 => (e.start, (b + mins * 60).max(e.start + 300)),
                            2 => ((a + mins * 60).min(b - 300), *b),
                            _ => (e.start + mins * 60, b + mins * 60),
                        };
                        // whole minutes once moved, as the editor leaves them
                        let (ns, ne) = (ns - ns.rem_euclid(60), ne - ne.rem_euclid(60));
                        fg.text(preview.left_top() + egui::vec2(9.0, 3.0), egui::Align2::LEFT_TOP, format!("{} → {}", clock_text(ns, true), clock_text(ne, true)), mono_font(12.0), c.text);
                        if resp.drag_stopped() {
                            ui.data_mut(|x| x.remove::<(Id, u8, f32)>(drag_id));
                            if mins != 0 && ne <= now {
                                dragged = Some((e.clone(), ns, ne));
                            } else if ne > now {
                                refused_future = true;
                            }
                        } else {
                            ui.data_mut(|x| x.insert_temp(drag_id, (e.id.clone(), part, dy)));
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
    if let Some((e, r)) = clicked {
        let ctx = ui.ctx().clone();
        let review = needs_completion(&e);
        open_edit(app, &e, Some(&ctx), review);
        // the compact window's editor opens by the block, as it does by a row
        if app.mode == super::WinMode::Compact {
            app.today.edit_anchor = Some(r);
        }
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

/// Save what the editor holds, after the same checks everywhere: closed days, a client without
/// one of its projects, the time fields, a session over 24 hours (saved on a second Save). True
/// when it was saved (or nothing changed) and the editor can close.
fn save_edit(app: &mut App, ed: &mut Edit) -> bool {
    ed.attempted = true;
    let moved_into_lock = edit::resolve(&ed.f, &ed.orig, ed.original.start, ed.original.stop).map(|r| devtrack_core::report::locked(&app.store.db, r.start)).unwrap_or(false);
    if devtrack_core::report::locked(&app.store.db, ed.original.start) || moved_into_lock {
        ed.errors.insert("general", locked_text(app));
    } else if ed.client.is_some() && ed.entry.project_id.is_none() {
        ed.errors.insert("project", "Choose one of this client's projects".into());
    } else if !validate_edit(ed) {
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
                    return true;
                }
                Err(m) => {
                    ed.errors.insert(if m.contains("future") { "start" } else { "end" }, tr(&m));
                }
            }
        }
    }
    false
}

pub fn edit_modal(app: &mut App, ctx: &egui::Context) {
    // the compact window edits a finished session in a small popover by its row
    if app.mode == super::WinMode::Compact && app.today.edit.as_ref().is_some_and(|e| e.entry.stop.is_some()) {
        compact_edit(app, ctx);
        return;
    }
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
        // tags: one list, a second way to classify across projects
        let pick = field(ui, "Tags", None, None, |ui| tag_picker(ui, ("edit-tags", &ed.entry.id), &app.store.db, &mut ed.entry.tags, Trigger::Field(w)));
        tag_picked(app, pick, &mut ed.entry.tags);
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
                if primary(ui, if ed.confirm_long.is_some() { "Save anyway" } else { "Save" }).clicked() && save_edit(app, &mut ed) {
                    close = true;
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
        let pick = field(ui, "Tags", None, None, |ui| tag_picker(ui, "past-tags", &app.store.db, &mut past.tags, Trigger::Field(w)));
        tag_picked(app, pick, &mut past.tags);
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
    let e = TimeEntry { id: new_id(), description: t.desc.trim().to_string(), project_id: t.project.clone(), task_id: t.task.clone(), start, stop: Some(stop), billable: t.billable, tags: t.tags.clone(), ..Default::default() };
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

    /// In the list, a click on a session's #tag shows only that tag's sessions and opens nothing;
    /// a click elsewhere on the row still opens the editor.
    #[test]
    fn a_click_on_a_tag_filters_the_list_and_opens_nothing() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Varied, false)), None);
        let e = app.store.db.live_entries().filter(|e| e.description == "Riunione giornaliera" && e.stop.is_some() && local_date(e.start) < today()).max_by_key(|e| e.start).cloned().expect("a daily meeting");
        let period = Period::containing(PeriodKind::Day, local_date(e.start), app.store.db.settings.week_starts_monday);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 1600.0));
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            let shown: Vec<TimeEntry> = entries_in(&app.store.db, &period, now_secs()).into_iter().filter(|e| !e.is_running()).cloned().collect();
            let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| ledger(app, ui, &shown, &period));
            });
        };
        let click = |app: &mut App, frame: &mut dyn FnMut(&mut App, Vec<egui::Event>), pos: egui::Pos2| {
            let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(app, vec![egui::Event::PointerMoved(pos)]);
            frame(app, vec![button(true)]);
            frame(app, vec![button(false)]);
        };
        frame(&mut app, vec![]);
        let row = ctx.read_response(egui::Id::new(("row", &e.id, e.start))).expect("the row is drawn").rect;
        // the title opens the editor
        click(&mut app, &mut frame, egui::pos2(row.left() + 60.0, row.top() + 22.0));
        assert!(app.today.edit.is_some(), "a click on the row opens the session");
        assert!(app.today.tag_filter.is_none());
        // along the grey line, the first spot that filters is the tag
        app.today.edit = None;
        let mut x = row.left() + 14.0;
        while app.today.tag_filter.is_none() && x < row.left() + 400.0 {
            click(&mut app, &mut frame, egui::pos2(x, row.top() + 46.0));
            app.today.edit = None;
            x += 3.0;
        }
        assert_eq!(app.today.tag_filter, Some(TagFilter::Tag("riunione".into())));
    }

    /// The box in a row's dot ticks it without opening it; while sessions are ticked a click on
    /// a row ticks it too, and Esc lets them all go.
    #[test]
    fn rows_tick_from_their_box_and_then_from_anywhere() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Varied, false)), None);
        let day = local_date(app.store.db.live_entries().filter(|e| e.description == "Riunione giornaliera" && e.stop.is_some() && local_date(e.start) < today()).map(|e| e.start).max().unwrap());
        let period = Period::containing(PeriodKind::Day, day, app.store.db.settings.week_starts_monday);
        let shown: Vec<TimeEntry> = {
            let mut v: Vec<TimeEntry> = entries_in(&app.store.db, &period, now_secs()).into_iter().filter(|e| !e.is_running()).cloned().collect();
            v.sort_by_key(|e| e.start);
            v
        };
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 1600.0));
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| ledger(app, ui, &shown, &period));
            });
        };
        let click = |app: &mut App, frame: &mut dyn FnMut(&mut App, Vec<egui::Event>), pos: egui::Pos2| {
            let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(app, vec![egui::Event::PointerMoved(pos)]);
            frame(app, vec![egui::Event::PointerMoved(pos)]);
            frame(app, vec![button(true)]);
            frame(app, vec![button(false)]);
        };
        frame(&mut app, vec![]);
        let rect = |id: &TimeEntry, a: i64| ctx.read_response(egui::Id::new(("row", &id.id, a))).expect("the row is drawn").rect;
        let (first, second) = (&shown[0], &shown[1]);
        let r1 = rect(first, first.start);
        click(&mut app, &mut frame, egui::pos2(r1.left() + 19.5, r1.top() + 21.0));
        assert!(app.today.selected.contains(&first.id), "the box ticks the row");
        assert!(app.today.edit.is_none(), "and opens nothing");
        let r2 = rect(second, second.start);
        click(&mut app, &mut frame, egui::pos2(r2.left() + 200.0, r2.top() + 22.0));
        assert!(app.today.selected.contains(&second.id) && app.today.edit.is_none(), "while ticking, the row ticks");
        // the bar takes a tag onto both at once
        let ids: Vec<Id> = app.today.selected.iter().cloned().collect();
        app.store.tag_entries(&ids, Some("da rivedere"), None);
        assert!(ids.iter().all(|id| app.store.db.entries[id].tags.iter().any(|t| t == "Da rivedere")), "the list's spelling");
        frame(&mut app, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        assert!(app.today.selected.is_empty());
    }

    /// Space on the focused row ticks it (Enter would open it); the filtered calendar still
    /// draws the sessions the filter leaves out, so their hours never look free.
    #[test]
    fn space_ticks_a_row_and_the_filtered_calendar_keeps_every_session() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Varied, false)), None);
        let e = app.store.db.live_entries().filter(|e| e.description == "Riunione giornaliera" && e.stop.is_some() && local_date(e.start) < today()).max_by_key(|e| e.start).cloned().unwrap();
        let day = local_date(e.start);
        let period = Period::containing(PeriodKind::Day, day, app.store.db.settings.week_starts_monday);
        let shown: Vec<TimeEntry> = entries_in(&app.store.db, &period, now_secs()).into_iter().filter(|e| !e.is_running()).cloned().collect();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 2400.0));
        let mut frame = |app: &mut App, events: Vec<egui::Event>, cal: bool| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if cal {
                        let f = Some(TagFilter::Tag("sviluppo".into()));
                        timeline_in(app, ui, &shown, &period, true, f.as_ref());
                    } else {
                        ledger(app, ui, &shown, &period);
                    }
                });
            });
        };
        frame(&mut app, vec![], false);
        let id = egui::Id::new(("row", &e.id, e.start));
        ctx.memory_mut(|m| m.request_focus(id));
        frame(&mut app, vec![], false);
        let key = |pressed| egui::Event::Key { key: egui::Key::Space, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
        frame(&mut app, vec![key(true)], false);
        frame(&mut app, vec![key(false)], false);
        assert!(app.today.selected.contains(&e.id), "Space ticks the focused row");
        assert!(app.today.edit.is_none(), "and opens nothing");
        // the meeting has no #sviluppo: still a block in the filtered calendar
        frame(&mut app, vec![], true);
        assert!(ctx.read_response(egui::Id::new(("tl", &e.id, day))).is_some(), "a session out of the filter is still drawn");
    }

    /// In the calendar, dragging a block's upper edge moves its start and keeps its end; its
    /// lower edge moves the end and keeps the start.
    #[test]
    fn calendar_blocks_stretch_from_either_edge() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = demo();
        let now = now_secs();
        // a finished session of an earlier day, at least an hour long and wholly inside it
        let e = app.store.db.live_entries().filter(|e| e.stop.is_some_and(|s| s - e.start >= 3600 && local_date(s) == local_date(e.start) && local_date(e.start) < today())).max_by_key(|e| e.start).cloned().expect("a long finished session");
        app.store.db.settings.lock_until = None;
        let day = local_date(e.start);
        let period = Period::containing(PeriodKind::Day, day, app.store.db.settings.week_starts_monday);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 2400.0));
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            let entries: Vec<TimeEntry> = entries_in(&app.store.db, &period, now).into_iter().cloned().collect();
            let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| timeline_in(app, ui, &entries, &period, true, None));
            });
        };
        let mut drag = |app: &mut App, from: egui::Pos2, dy: f32| {
            let to = from + egui::vec2(0.0, dy);
            let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(app, vec![egui::Event::PointerMoved(from)]);
            frame(app, vec![button(from, true)]);
            frame(app, vec![egui::Event::PointerMoved(from + egui::vec2(0.0, dy / 4.0))]);
            frame(app, vec![egui::Event::PointerMoved(to)]);
            frame(app, vec![button(to, false)]);
            frame(app, vec![]);
        };
        frame(&mut app, vec![]);
        let block = ctx.read_response(egui::Id::new(("tl", &e.id, day))).expect("the block is drawn").rect;
        // the upper edge, up by a good part of an hour
        drag(&mut app, egui::pos2(block.center().x, block.top() + 2.0), -40.0);
        let moved = app.store.db.live_entries().find(|x| x.id == e.id).cloned().unwrap();
        assert!(moved.start < e.start, "the start moved earlier");
        assert_eq!(moved.stop, e.stop.map(|s| s - s.rem_euclid(60)), "the end stays, in whole minutes");
        assert_eq!(moved.start % 60, 0);
        frame(&mut app, vec![]);
        let block = ctx.read_response(egui::Id::new(("tl", &e.id, day))).unwrap().rect;
        drag(&mut app, egui::pos2(block.center().x, block.bottom() - 2.0), -40.0);
        let shrunk = app.store.db.live_entries().find(|x| x.id == e.id).cloned().unwrap();
        assert_eq!(shrunk.start, moved.start, "the start stays");
        assert!(shrunk.stop < moved.stop, "the end moved earlier");
    }

    /// A day off with no sessions (Sunday, Day view) used to leave the calendar with no column,
    /// a width of NaN and a crash in egui; the timesheet the same with a week of days off.
    #[test]
    fn calendar_and_timesheet_draw_on_days_off() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = demo();
        let mut sunday = today();
        while sunday.weekday() != chrono::Weekday::Sun || app.store.db.live_entries().any(|e| local_date(e.start) == sunday) {
            sunday += CDur::days(1);
        }
        let monday = app.store.db.settings.week_starts_monday;
        for (timesheet, kind) in [(false, PeriodKind::Day), (true, PeriodKind::Week)] {
            app.today.timeline = !timesheet;
            app.today.timesheet = timesheet;
            app.today.period = Period::containing(kind, sunday, monday);
            if timesheet {
                // no working day at all this week
                let mut s = app.store.db.settings.clone();
                s.work_days = 0;
                app.store.db.settings = s;
            }
            for _ in 0..3 {
                let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() }, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| page(&mut app, ui));
                });
            }
        }
    }

    /// In the compact window a finished session opens in the small popover (not the dialog), the
    /// click that opened it does not close it, and a typed duration moves the end when saved.
    #[test]
    fn compact_window_edits_a_session_in_a_popover() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = demo();
        app.mode = super::super::WinMode::Compact;
        let e = app.store.db.live_entries().filter(|e| e.stop.is_some()).max_by_key(|e| e.start).cloned().expect("a finished session");
        open_edit(&mut app, &e, None, false);
        app.today.edit_anchor = Some(egui::Rect::from_min_size(egui::pos2(16.0, 200.0), egui::vec2(360.0, 60.0)));
        let click_at = egui::pos2(30.0, 220.0);
        // the opening click, still down on the row, then a plain frame
        for events in [vec![egui::Event::PointerButton { pos: click_at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }], vec![]] {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(430.0, 800.0))), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |_| {});
                edit_modal(&mut app, ctx);
            });
        }
        assert!(app.today.edit.is_some(), "the popover stays open after the click that opened it");
        assert!(ctx.memory(|m| m.layer_ids().any(|l| l.id == egui::Id::new("compact-edit"))), "drawn as the compact popover");
        let mut ed = app.today.edit.take().unwrap();
        ed.f.duration = "1:00".into();
        ed.f.duration_edited = true;
        assert!(save_edit(&mut app, &mut ed));
        let saved = app.store.db.live_entries().find(|x| x.id == e.id).cloned().unwrap();
        assert_eq!(saved.start, e.start);
        assert_eq!(saved.stop, Some(e.start + 3600));
    }

    /// Choosing the project after the task keeps the task when it is that project's, and takes
    /// it away (saying so) when it is another's; choosing the client keeps the task's project.
    #[test]
    fn choosing_the_project_after_the_task_keeps_what_fits() {
        let mut app = demo();
        let db = &app.store.db;
        let t = db.live_tasks().find(|t| t.project_id.is_some()).cloned().expect("a task with a project");
        let own = t.project_id.clone();
        let other = db.live_projects().into_iter().find(|p| Some(&p.id) != own.as_ref()).map(|p| p.id.clone()).expect("another project");
        // the task's own project: everything stays
        let (mut project, mut task, mut client) = (own.clone(), Some(t.id.clone()), None);
        settle_work(&mut app, Pick::None, Pick::Changed, Pick::None, &mut project, Some(&mut task), &mut client);
        assert_eq!(task.as_deref(), Some(t.id.as_str()));
        assert_eq!(client, client_of(&app.store.db, &own));
        // the task's client again, chosen by hand: its project stays the task's
        if let Some(c) = client.clone() {
            let mut chosen = Some(c);
            settle_work(&mut app, Pick::None, Pick::None, Pick::Changed, &mut project, Some(&mut task), &mut chosen);
            assert_eq!(project, own);
            assert_eq!(task.as_deref(), Some(t.id.as_str()));
        }
        // another project: the task goes, the project and the client are the new ones
        project = Some(other.clone());
        settle_work(&mut app, Pick::None, Pick::Changed, Pick::None, &mut project, Some(&mut task), &mut client);
        assert_eq!(project.as_deref(), Some(other.as_str()));
        assert!(task.is_none(), "a task of another project no longer fits");
        assert_eq!(client, client_of(&app.store.db, &Some(other)));
    }

    #[test]
    fn a_hash_word_at_the_end_asks_for_tags_and_a_number_stays_a_work_item() {
        assert_eq!(tag_token("Login fix #fro"), Some(("Login fix".into(), "fro".into())));
        assert_eq!(tag_token("#"), Some(("".into(), "".into())));
        assert_eq!(tag_token("#4812"), None, "a #number is a work item");
        assert_eq!(tag_token("#4812 Login page"), None);
        assert_eq!(tag_token("Login #fro "), None, "a space ends the tag word");
        let mut app = demo();
        for t in ["frontend", "review", "Front office"] {
            app.store.create_tag(t);
        }
        let list = suggestions(&app.store.db, "Login fix #fro", None);
        let names: Vec<String> = list.iter().map(|s| match s { Suggest::Tag(n) => format!("t:{n}"), Suggest::NewTag(n) => format!("new:{n}"), _ => "other".into() }).collect();
        assert_eq!(names, vec!["t:Front office", "t:frontend", "new:fro"], "tags only, then the word as a new tag");
        let exact = suggestions(&app.store.db, "#review", None);
        assert!(!exact.iter().any(|s| matches!(s, Suggest::NewTag(..))), "no new tag for a name that exists");
        // a near spelling finds the tag already there, and offers no near copy
        app.store.create_tag("Code review");
        let near = suggestions(&app.store.db, "#code-review", None);
        assert!(near.iter().any(|s| matches!(s, Suggest::Tag(n) if n == "Code review")), "code-review finds Code review");
        assert!(!near.iter().any(|s| matches!(s, Suggest::NewTag(..))), "and offers no new tag");
        assert_eq!(devtrack_core::model::tag_key("Code-Review "), devtrack_core::model::tag_key("code review"));
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
            let k = match s { Suggest::Task(id) => format!("t:{id}"), Suggest::Project(id) => format!("p:{id}"), Suggest::Remote(n) => format!("r:{n}"), Suggest::LookUp(n) => format!("l:{n}"), Suggest::Refresh => "refresh".into(), Suggest::Tag(n) | Suggest::NewTag(n) => format!("g:{n}") };
            assert!(!keys.contains(&k), "duplicate suggestion");
            keys.push(k);
        }
        assert!(suggestions(&app.store.db, "   ", None).is_empty());
    }

    #[test]
    fn suggestion_rows_stay_inside_a_narrow_window() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = demo();
        // a long project and client name, as real ones can be
        let pid = app.store.db.live_projects().into_iter().next().unwrap().id.clone();
        if let Some(pr) = app.store.db.projects.get_mut(&pid) {
            pr.name = "Customer portal website relaunch".into();
        }
        let name = "Customer portal website relaunch".to_string();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 900.0));
        let mut out = None;
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            app.geometry.rows.clear();
            out = Some(ctx.run(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| session_panel(app, ui));
            }));
        };
        frame(&mut app, vec![]);
        let at = app.geometry.rows.iter().find(|(n, _)| n == "start row · description").unwrap().1.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        frame(&mut app, vec![egui::Event::PointerMoved(at), press(true)]);
        frame(&mut app, vec![press(false)]);
        frame(&mut app, vec![egui::Event::Text(name[..5].to_string())]);
        frame(&mut app, vec![]);
        let popup = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("session-suggest"))).expect("suggestions open");
        let mut texts = 0;
        for cs in &out.unwrap().shapes {
            if let egui::Shape::Text(t) = &cs.shape {
                let r = t.visual_bounding_rect();
                if popup.contains(r.center()) {
                    texts += 1;
                    assert!(r.right() <= popup.right() + 0.5, "text past the list's edge: {r:?} in {popup:?}");
                }
            }
        }
        assert!(texts > 2, "rows were drawn");
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


/// The compact window's session editor, with only what a quick fix needs (task, project,
/// duration, start → end, day, tags), Delete and Save; notes, billable and the client are in the
/// full window's editor. Where the system lets an app place its windows it opens as a small window
/// of its own beside the compact one, as other trackers do; otherwise (Linux on Wayland, embedded
/// viewports, review pictures) as a popover by the row. Esc, ✕ and a click elsewhere close it,
/// asking first in its footer when something changed.
fn compact_edit(app: &mut App, ctx: &egui::Context) {
    let Some(mut ed) = app.today.edit.take() else { return };
    let beside = side_window_allowed(app, ctx);
    let close = if beside { compact_edit_window(app, ctx, &mut ed) } else { compact_edit_popover(app, ctx, &mut ed) };
    if close {
        app.today.edit_anchor = None;
        ctx.data_mut(|d| d.remove::<bool>(egui::Id::new("compact-edit-had-focus")));
        app.restore_focus(ctx);
    } else {
        app.today.edit = Some(ed);
    }
}

/// A window of its own needs a system that lets the app place it: not Linux on Wayland, not
/// embedded viewports, not review pictures (they capture one window).
fn side_window_allowed(app: &App, ctx: &egui::Context) -> bool {
    let wayland = cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some();
    !wayland && !ctx.embed_viewports() && app.shot.is_none() && ctx.input(|i| i.viewport().outer_rect.is_some())
}

/// Closing asked by ✕, Esc or a click elsewhere: at once when nothing changed, otherwise the
/// footer asks first (a second request closes).
fn request_close(ed: &mut Edit) -> bool {
    if ed.dirty() && !ed.confirm_discard {
        ed.confirm_discard = true;
        false
    } else {
        true
    }
}

/// Beside the compact window: a borderless window that stays on top, on the right of it (on the
/// left when the screen ends there), level with the row that opened it. It follows the compact
/// window when that moves. Losing focus counts as a click elsewhere.
fn compact_edit_window(app: &mut App, ctx: &egui::Context, ed: &mut Edit) -> bool {
    let w = 380.0;
    let outer_w = w + 24.0;
    let info = ctx.input(|i| i.viewport().clone());
    let (Some(outer), Some(inner)) = (info.outer_rect, info.inner_rect) else { return compact_edit_popover(app, ctx, ed) };
    // the screen the compact window is on, wherever it sits among side-by-side screens
    let screen = super::monitors::screen_at(outer.center(), info.monitor_size).unwrap_or(egui::Rect::EVERYTHING);
    let h_id = egui::Id::new("compact-edit-h");
    let h: f32 = ctx.data(|d| d.get_temp(h_id)).unwrap_or(440.0);
    let x = super::monitors::side_x(outer, outer_w, 8.0, screen);
    let anchor = app.today.edit_anchor.unwrap_or(egui::Rect::from_min_size(egui::pos2(0.0, 120.0), egui::vec2(10.0, 10.0)));
    let y = (inner.top() + anchor.top() - 12.0).min(screen.bottom() - h - 8.0).max(outer.top()).max(screen.top());
    let builder = egui::ViewportBuilder::default()
        .with_title(tr("Edit session"))
        .with_position(egui::pos2(x, y))
        .with_inner_size(egui::vec2(outer_w, h))
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_active(true)
        // see-through, so the editor's own rounded frame is the window's shape
        .with_transparent(true)
        .with_window_level(egui::WindowLevel::AlwaysOnTop);
    let dark = app.dark();
    ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("compact-edit-window"), builder, |cctx, _class| {
        let c = pal(dark);
        let mut out = (false, false);
        // the panel is clear; the frame inside it is the editor, rounded like the popover, and its
        // measured height (not the window's) sets the window's
        let panel = egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::TRANSPARENT)).show(cctx, |ui| {
            egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing.y = ROW_GAP;
                out = compact_edit_body(app, ui, ed, w);
            }).response.rect.height()
        });
        // the window is as tall as what it holds, and taller while a menu or the calendar is open
        let menus = cctx.memory(|m| m.any_popup_open());
        let want = (panel.inner + 1.0).max(if menus { 600.0 } else { 0.0 }).ceil();
        if (want - h).abs() > 1.0 {
            cctx.data_mut(|d| d.insert_temp(h_id, want));
            cctx.request_repaint();
        }
        let (mut close, mut ask) = out;
        let focused = cctx.input(|i| i.viewport().focused);
        let had = cctx.data(|d| d.get_temp::<bool>(egui::Id::new("compact-edit-had-focus"))).unwrap_or(false);
        if focused == Some(true) && !had {
            cctx.data_mut(|d| d.insert_temp(egui::Id::new("compact-edit-had-focus"), true));
        }
        // clicked elsewhere: on the compact window or another app
        if had && focused == Some(false) && !menus {
            ask = true;
        }
        if !menus && cctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ask = true;
        }
        if ask && !close {
            close = request_close(ed);
        }
        close
    })
}

/// Inside the compact window, by the row: under it when it fits, otherwise over it.
fn compact_edit_popover(app: &mut App, ctx: &egui::Context, ed: &mut Edit) -> bool {
    let c = pal(app.dark());
    let screen = ctx.screen_rect();
    let w = (screen.width() - 48.0).clamp(260.0, 380.0);
    let outer = w + 24.0;
    let anchor = app.today.edit_anchor.unwrap_or(egui::Rect::from_center_size(screen.center(), egui::vec2(10.0, 10.0)));
    let est_h = 420.0;
    let x = (screen.center().x - outer / 2.0).max(screen.left() + 8.0);
    let below = anchor.bottom() + 6.0 + est_h <= screen.bottom() - 8.0;
    let (pos, pivot) = if below { (egui::pos2(x, anchor.bottom() + 6.0), egui::Align2::LEFT_TOP) } else { (egui::pos2(x, (anchor.top() - 6.0).max(screen.top() + est_h + 8.0).min(screen.bottom() - 8.0)), egui::Align2::LEFT_BOTTOM) };
    let mut out = (false, false);
    let area = egui::Area::new(egui::Id::new("compact-edit")).order(egui::Order::Foreground).pivot(pivot).fixed_pos(pos).constrain(true).show(ctx, |ui| {
        egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).inner_margin(egui::Margin::same(12)).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing.y = ROW_GAP;
            out = compact_edit_body(app, ui, ed, w);
        });
    });
    let (close, mut ask) = out;
    let menus_open = ctx.memory(|m| m.any_popup_open());
    // only a click on the page under it counts: the pickers' menus and the calendar are layers of
    // their own; and not the click that opened it, on its row
    let first_frame = !std::mem::replace(&mut ed.focused_once, true);
    let on_page = |p: egui::Pos2| ctx.layer_id_at(p).map(|l| l.order == egui::Order::Background).unwrap_or(true);
    let outside = !first_frame && ctx.input(|i| i.pointer.any_click()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p) && on_page(p));
    if outside || (!menus_open && ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
        ask = true;
    }
    close || (ask && request_close(ed))
}

/// What the compact editor shows, wherever it is drawn. Returns (closed, asked to close).
fn compact_edit_body(app: &mut App, ui: &mut egui::Ui, ed: &mut Edit, w: f32) -> (bool, bool) {
    let c = p(ui);
    let monday = app.store.db.settings.week_starts_monday;
    let mut close = false;
    let mut ask_close = false;
    row(ui, 28.0, |ui| {
        ui.label(RichText::new(tr("Edit session")).font(medium(15.0)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_ghost(ui, Glyph::Close, "Close (Esc)").clicked() {
                ask_close = true;
            }
        });
    });
    // what it was: the task across the width, then its project
    let remote = RemoteSuggest::of(app);
    let rt = recent_tasks(&app.store.db);
    let rp = recent_projects(&app.store.db);
    let b = task_picker(ui, "compact-edit-task", &app.store.db, remote.as_ref(), &rt, &mut ed.entry.project_id, &mut ed.entry.task_id, w);
    let a = project_picker(ui, "compact-edit-project", &app.store.db, &rp, &mut ed.entry.project_id, w);
    let mut client = ed.client.clone();
    settle_work(app, b, a, Pick::None, &mut ed.entry.project_id, Some(&mut ed.entry.task_id), &mut client);
    ed.client = client;
    if let Some(m) = ed.errors.get("project").cloned() {
        ui.label(RichText::new(tr(&m)).size(12.0).color(c.red));
    }
    // its tags, the same field as the full editor's
    let tp = tag_picker(ui, "compact-edit-tags", &app.store.db, &mut ed.entry.tags, Trigger::Field(w));
    tag_picked(app, tp, &mut ed.entry.tags);
    ui.add_space(4.0);
    // the time: the duration first (as it reads in the row), then start → end and the day
    let bad = |k: &str| ed.errors.contains_key(k);
    let (bd, bs, be, bdate) = (bad("dur"), bad("start"), bad("end") || bad("end_date"), bad("date"));
    field(ui, "Duration", None, None, |ui| {
        let r = input(ui, &mut ed.f.duration, Field::new("Duration", w).mono().invalid(bd).hint("1:30 · 1.5 · 90"));
        if r.changed() {
            ed.f.duration_edited = true;
        }
    });
    let half = (w - 24.0 - ROW_GAP * 2.0) / 2.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        if time_input(ui, &mut ed.f.start, "Start time", half, bs).changed() {
            ed.f.duration_edited = false;
        }
        let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, CTRL_H), Sense::hover());
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "→", sans(16.0), c.text2);
        if time_input(ui, &mut ed.f.end, "End time", half, be).changed() {
            ed.f.duration_edited = false;
        }
    });
    let before = ed.f.start_date.clone();
    if date_input(ui, &mut ed.f.start_date, "Start date", w, bdate, monday).changed() {
        ed.f.duration_edited = false;
    }
    // the end's day moves with the start's: a session over midnight stays over midnight
    if ed.f.start_date != before {
        if let (Ok(a), Ok(b), Ok(e)) = (NaiveDate::parse_from_str(before.trim(), "%Y-%m-%d"), NaiveDate::parse_from_str(ed.f.start_date.trim(), "%Y-%m-%d"), NaiveDate::parse_from_str(ed.f.end_date.trim(), "%Y-%m-%d")) {
            ed.f.end_date = (e + (b - a)).format("%Y-%m-%d").to_string();
        }
    }
    // the other side follows what was typed: the duration from start and end, or the end
    // from the duration
    if let Ok(res) = edit::resolve(&ed.f, &ed.orig, ed.original.start, ed.original.stop) {
        if let Some(stop) = res.stop {
            if ed.f.duration_edited {
                let end = clock_text(stop, true);
                if end != ed.f.end { ed.f.end = end; }
                let end_date = local_date(stop).format("%Y-%m-%d").to_string();
                if end_date != ed.f.end_date { ed.f.end_date = end_date; }
            } else {
                let d = format_duration(stop - res.start, DurationFormat::Improved);
                if d != ed.f.duration { ed.f.duration = d; }
            }
        }
    }
    if ed.attempted {
        validate_edit(ed);
    }
    let err = ["dur", "date", "start", "end_date", "end", "general"].into_iter().find_map(|k| ed.errors.get(k).cloned());
    if let Some(m) = err {
        ui.add(egui::Label::new(RichText::new(tr(&m)).size(12.0).color(c.red)).wrap());
    }
    if let Some((a, b)) = ed.confirm_long {
        mixed(ui, &trf("! This session would last {}. Check the dates, or press “Save anyway”.", &[&words(b - a)]), 12.0, Some(c.amber));
    }
    ui.add_space(4.0);
    divider(ui);
    ui.add_space(4.0);
    row(ui, CTRL_H, |ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        if ed.confirm_discard {
            ui.label(tr("Discard your changes?"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if danger(ui, "Discard").clicked() { close = true; }
                if secondary(ui, "Keep editing").clicked() { ed.confirm_discard = false; }
            });
            return;
        }
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
            if ghost(ui, "Keep").clicked() { ed.confirm_delete = false; }
            return;
        }
        if danger(ui, "Delete…").clicked() {
            ed.confirm_delete = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary(ui, if ed.confirm_long.is_some() { "Save anyway" } else { "Save" }).clicked() && save_edit(app, ed) {
                close = true;
            }
        });
    });
    (close, ask_close)
}
