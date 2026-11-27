//! Activities: a searchable list of local projects and a project workspace with its tasks and,
//! next to them, the work items assigned to you in Azure DevOps (read only).
//! Credentials and diagnostics live in Settings › Connections; connecting from here returns to
//! the assigned tasks of the same project afterwards.

use super::jobs::JobOut;
use super::today::modal_frame;
use super::i18n::{plural, tr, trf};
use super::i18n::trf_m;
use super::widgets::*;
use super::{page_header, App, Page, ReturnTo};
use devtrack_core::integrations::{org_of, to_task, ErrKind, RemoteItem, RemoteProject, SourceError, PAGE};
use devtrack_core::model::*;
use devtrack_core::secrets;
use devtrack_core::store::now_secs;
use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub enum FinderState {
    NotLoaded,
    Loading,
    Loaded,
    NoResults,
    Offline(String),
    Error(String),
}

pub struct Finder {
    pub cfg: Option<Id>,
    pub searching: bool,
    pub query: String,
    /// Narrows the loaded list by title or number, without asking the source again.
    pub filter: String,
    pub remote_project: Option<String>,
    pub remote_projects: Vec<RemoteProject>,
    pub ids: Vec<String>,
    pub items: Vec<RemoteItem>,
    pub selected: BTreeSet<String>,
    pub state: FinderState,
    pub auto_tried: BTreeSet<Id>,
    pub refresh_msg: Option<(bool, String)>,
}

impl Default for Finder {
    fn default() -> Self {
        Finder {
            cfg: None,
            searching: false,
            query: String::new(),
            filter: String::new(),
            remote_project: None,
            remote_projects: vec![],
            ids: vec![],
            items: vec![],
            selected: BTreeSet::new(),
            state: FinderState::NotLoaded,
            auto_tried: BTreeSet::new(),
            refresh_msg: None,
        }
    }
}

impl Finder {
    pub fn set_error(&mut self, e: SourceError) {
        self.state = match e.kind {
            ErrKind::Offline => FinderState::Offline(e.message),
            _ => FinderState::Error(e.message),
        };
    }
}

/// What Azure DevOps says about the open task, read on demand and never stored.
#[derive(Clone, Debug, PartialEq)]
pub enum RemoteState {
    Loading,
    Loaded(devtrack_core::integrations::RemoteDetail),
    Failed(String),
}

/// Moving a task: one dialog, one decision about past sessions, one action.
#[derive(Clone)]
pub struct MoveDraft {
    pub task: Id,
    pub to: Option<Id>,
    pub also_sessions: bool,
    /// Sessions recorded on the task.
    pub sessions: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ActTab {
    #[default]
    Tasks,
    Projects,
    Clients,
    Tags,
}

/// What the Tags tab is doing with one tag: naming a new one, renaming one in its row, asking to
/// merge two (a rename to a name another tag has), or asking before a delete.
#[derive(Clone, PartialEq)]
pub enum TagEdit {
    New(String),
    Rename(Id, String),
    Merge { from: Id, into: Id },
    Delete(Id),
}

/// One step below a Work tab: a client or a project, in the order they were opened. The
/// breadcrumb shows them and Back removes the last one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Crumb {
    Client(Id),
    Project(Id),
}

/// Which tasks the estimate filter keeps: any, those past their planned hours, those without.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum EstimateFilter {
    #[default]
    All,
    Over,
    Missing,
}

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum ClientFilter {
    #[default]
    All,
    NoClient,
    Client(Id),
}

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub enum ProjectFilter {
    #[default]
    All,
    NoProject,
    Project(Id),
}

/// Where a task comes from: 0 any, 1 added in DevTrack, 2 Azure DevOps.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SourceFilter {
    #[default]
    All,
    Local,
    Azure,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum StatusFilter {
    #[default]
    Active,
    Archived,
    All,
}

/// Edit project form (explicit Save; nothing is stored while typing).
#[derive(Clone, PartialEq)]
pub struct ProjectForm {
    /// None = a new project.
    pub id: Option<Id>,
    pub name: String,
    pub client_id: Option<Id>,
    pub color: String,
    pub billable: bool,
    /// "Create client" subview inside the same dialog: the name being typed.
    pub new_client: Option<String>,
    pub attempted: bool,
    pub confirm_discard: bool,
    pub focused: bool,
    initial: (String, Option<Id>, String, bool),
}

impl ProjectForm {
    fn new(app: &App, id: Option<Id>, client_id: Option<Id>) -> ProjectForm {
        let (name, client_id, color, billable) = match id.as_ref().and_then(|i| app.store.db.projects.get(i)) {
            Some(p) => (p.name.clone(), p.client_id.clone(), p.color.clone(), p.billable_default),
            None => (String::new(), client_id, app.store.db.next_color(), false),
        };
        ProjectForm { id, initial: (name.clone(), client_id.clone(), color.clone(), billable), name, client_id, color, billable, new_client: None, attempted: false, confirm_discard: false, focused: false }
    }
    /// For review pictures: the form for an existing project.
    pub fn new_for(app: &App, id: Option<Id>) -> ProjectForm {
        ProjectForm::new(app, id, None)
    }
    pub fn dirty(&self) -> bool {
        (self.name.clone(), self.client_id.clone(), self.color.clone(), self.billable) != self.initial || self.new_client.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
    }
}

/// Client actions on the Clients tab (inline; no stacked dialogs).
#[derive(Clone, PartialEq, Debug)]
pub enum ClientEdit {
    New(String),
    Rename(Id, String),
    ConfirmDelete(Id),
}

#[derive(Default)]
pub struct ActState {
    pub tab: ActTab,
    /// What was opened below the tab, in order. `selected` and `client_open` mirror its last step.
    pub(crate) trail: Vec<Crumb>,
    /// The project whose tasks are open (None = the project browser). Read only: set through
    /// the navigation methods.
    pub selected: Option<Id>,
    pub search: String,
    pub client_filter: ClientFilter,
    pub status_filter: StatusFilter,
    pub new_task: String,
    /// Which tasks the list shows: 0 open, 1 done, 2 all.
    pub task_show: u8,
    /// Narrows the project's task list by title or #number.
    pub task_query: String,
    /// The "Add task" dialog is open, on a local task or on Azure DevOps work items.
    pub add_open: bool,
    pub add_devops: bool,
    /// The dialog was opened from the Tasks tab: the project is chosen in it (`add_project`)
    /// instead of being the open one.
    pub add_pick: bool,
    pub add_project: Option<Id>,
    /// "Add task" was pressed with no title: the field says so.
    pub add_attempted: bool,
    /// Closing would lose what was typed or picked: the footer asks first.
    pub add_confirm: bool,
    pub finder: Finder,
    pub pending_move: Option<MoveDraft>,
    /// The task whose sessions are listed under its row.
    pub task_open: Option<Id>,
    /// Source detail for a task (by task id), kept while the page is open.
    pub remote: Option<(Id, RemoteState)>,
    pub confirm_delete: Option<Id>,
    pub form: Option<ProjectForm>,
    pub client_search: String,
    pub client_open: Option<Id>,
    /// Narrows the open client's projects by name.
    pub client_projects_query: String,
    pub client_edit: Option<ClientEdit>,
    /// Rename task: the task and the name as typed (tasks added in DevTrack only).
    pub task_rename: Option<(Id, String)>,
    pub task_rename_attempted: bool,
    pub task_rename_focused: bool,
    pub task_rename_confirm: bool,
    pub client_attempted: bool,
    pub client_confirm: bool,
    pub client_focused: bool,
    /// The Tasks tab: every task, filtered by words and by open or done.
    pub all_query: String,
    pub all_show: u8,
    pub all_client: ClientFilter,
    pub all_project: ProjectFilter,
    pub all_source: SourceFilter,
    pub all_estimate: EstimateFilter,
    /// Tasks with a session carrying this tag, or with no tagged session at all.
    pub all_tag: Option<TagFilter>,
    /// The task whose sessions are listed under its row on the Tasks tab (apart from the
    /// project's, so each list keeps its own open row).
    pub all_open: Option<Id>,
    /// Bring the open row of the Tasks tab into view once (after opening a task from elsewhere).
    pub reveal: bool,
    /// The task whose every session is listed; others show the most recent ones.
    pub all_sessions: Option<Id>,
    /// Clients tab: active, archived or all.
    pub client_status: StatusFilter,
    /// Tags tab: the search, and what is being done to a tag.
    pub tag_search: String,
    pub tag_edit: Option<TagEdit>,
}

impl ActState {
    /// Opens the "Add task" dialog for the open project. The work items picked last time are
    /// never carried over: each opening starts with nothing selected.
    pub fn open_add(&mut self, devops: bool) {
        self.add_open = true;
        self.add_devops = devops;
        self.add_pick = false;
        self.add_attempted = false;
        self.add_confirm = false;
        self.new_task.clear();
        self.finder.selected.clear();
    }

    /// Opens the "Add task" dialog with a project to choose, starting from `project`.
    pub fn open_add_choose(&mut self, devops: bool, project: Option<Id>) {
        self.open_add(devops);
        self.add_pick = true;
        self.add_project = project;
    }

    pub fn close_add(&mut self) {
        self.add_open = false;
        self.finder.selected.clear();
    }

    /// Opens the client dialog (new or rename) fresh: no error, focus on the name.
    pub fn open_client_edit(&mut self, e: ClientEdit) {
        self.client_edit = Some(e);
        self.client_attempted = false;
        self.client_confirm = false;
        self.client_focused = false;
    }

    pub fn close_client_edit(&mut self) {
        self.client_edit = None;
        self.client_attempted = false;
        self.client_confirm = false;
    }

    /// The tab's first level: nothing opened below it.
    pub fn open_root(&mut self, tab: ActTab) {
        self.tab = tab;
        self.trail.clear();
        self.client_edit = None;
        self.sync();
    }

    /// Opens a project from where you are: its step follows the current ones, so Back and the
    /// breadcrumb return the same way. A project already on the way is returned to instead.
    pub fn open_project(&mut self, id: Id) {
        self.push(Crumb::Project(id));
        self.task_query.clear();
        self.close_add();
    }

    pub fn open_client(&mut self, id: Id) {
        self.push(Crumb::Client(id));
        self.client_edit = None;
        self.client_projects_query.clear();
    }

    /// A project as the only step of the Projects tab (links from other sections, previews).
    pub fn show_project(&mut self, id: Id) {
        self.open_root(ActTab::Projects);
        self.open_project(id);
    }

    /// A client as the only step of the Clients tab.
    pub fn show_client(&mut self, id: Id) {
        self.open_root(ActTab::Clients);
        self.open_client(id);
    }

    fn push(&mut self, c: Crumb) {
        if let Some(i) = self.trail.iter().position(|x| *x == c) {
            self.trail.truncate(i + 1);
        } else {
            self.trail.push(c);
        }
        self.sync();
    }

    /// The steps below the tab, oldest first.
    pub fn trail(&self) -> &[Crumb] {
        &self.trail
    }

    /// Returns to step `i` of the trail (0 = the first one opened).
    pub fn back_to(&mut self, i: usize) {
        self.trail.truncate(i + 1);
        self.close_add();
        self.client_edit = None;
        self.sync();
    }

    /// Return one level, retaining every list's search, filters and open row.
    pub fn back(&mut self) -> bool {
        if self.trail.pop().is_none() {
            return false;
        }
        self.close_add();
        self.client_edit = None;
        self.sync();
        true
    }

    fn sync(&mut self) {
        self.selected = match self.trail.last() { Some(Crumb::Project(id)) => Some(id.clone()), _ => None };
        self.client_open = match self.trail.last() { Some(Crumb::Client(id)) => Some(id.clone()), _ => None };
    }
}

const NO_PROJECT: &str = "__none__";

/// Opens a task where it lives: the Tasks tab, with its row open (source data for an imported
/// task, then its sessions) and brought into view. Filters that would hide it are cleared.
pub fn open_task(app: &mut App, task: &Task) {
    app.act.open_root(ActTab::Tasks);
    app.act.all_query.clear();
    app.act.all_client = ClientFilter::All;
    app.act.all_project = ProjectFilter::All;
    app.act.all_source = SourceFilter::All;
    app.act.all_estimate = EstimateFilter::All;
    app.act.all_tag = None;
    if task.done && app.act.all_show == 0 {
        app.act.all_show = 2;
    }
    app.act.all_open = Some(task.id.clone());
    app.act.reveal = true;
    if task.source == SOURCE_AZURE {
        app.act.remote = None;
        load_source_detail(app, task);
    }
    app.go(Page::Activities);
}

/// Colour names for the compact colour selector (same order as PROJECT_COLORS).
pub const COLOR_NAMES: [&str; 14] = ["Blue", "Purple", "Magenta", "Orange", "Amber", "Yellow", "Red", "Green", "Teal", "Peach", "Indigo", "Plum", "Olive", "Slate"];

fn color_name(hexs: &str) -> String {
    PROJECT_COLORS.iter().position(|c| c.eq_ignore_ascii_case(hexs)).map(|i| tr(COLOR_NAMES[i])).unwrap_or_else(|| hexs.to_string())
}

fn connect_azure(app: &mut App) {
    app.return_to = Some(ReturnTo { project: app.act.selected.clone() });
    app.settings.tab = super::settings::Tab::Connections;
    app.go(Page::Settings);
}

pub fn open_project_form(app: &mut App, ctx: &egui::Context, id: Option<Id>, client: Option<Id>) {
    app.remember_focus(ctx);
    app.act.form = Some(ProjectForm::new(app, id, client));
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    // a record opened below any tab: the trail decides which, the tab only names the way back
    if let Some(id) = app.act.selected.clone() { workspace(app, ui, id); return; }
    if let Some(id) = app.act.client_open.clone() { client_detail(app, ui, id); return; }
    let n_projects = app.store.db.live_projects().len();
    let n_clients = app.store.db.live_clients().len();
    let n_tasks = app.store.db.live_tasks().filter(|t| !t.done).count();
    page_header(ui, "Work", "Tasks, projects and clients: your own, or read from Azure DevOps.", |_| {});
    let projects_tab = format!("{} {}", tr("Projects"), n_projects);
    let tasks_tab = format!("{} {}", tr("Tasks"), n_tasks);
    let clients_tab = format!("{} {}", tr("Clients"), n_clients);
    let tags_tab = format!("{} {}", tr("Tags"), app.store.db.live_tags().len());
    let current = match app.act.tab { ActTab::Tasks => 0, ActTab::Projects => 1, ActTab::Clients => 2, ActTab::Tags => 3 };
    if let Some(i) = segmented_or_select(ui, "work-tabs", &[&tasks_tab, &projects_tab, &clients_tab, &tags_tab], current) {
        app.act.open_root(match i { 1 => ActTab::Projects, 2 => ActTab::Clients, 3 => ActTab::Tags, _ => ActTab::Tasks });
        app.fade_content();
    }
    ui.add_space(GROUP_GAP);
    match app.act.tab {
        ActTab::Projects => browser(app, ui),
        ActTab::Clients => clients_tab_ui(app, ui),
        ActTab::Tasks => tasks_tab_ui(app, ui),
        ActTab::Tags => tags_tab_ui(app, ui),
    }
}

/// Every task in one table, newest work first: the project and client under the title, the
/// last session, the time tracked and Start. A row opens the task in its project.
fn tasks_tab_ui(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let now = now_secs();
    let w = ui.available_width();
    if app.store.db.live_tasks().next().is_none() {
        let mut go = false;
        empty_state(ui, Icon::Activities, "No tasks yet", "Add a task of your own or bring work items from Azure DevOps. A session's task also becomes one.", |ui| {
            go = secondary(ui, "+ Add task").clicked();
        });
        if go {
            let project = recent_projects(&app.store.db).into_iter().next();
            app.remember_focus(ui.ctx());
            app.act.open_add_choose(false, project);
        }
        return;
    }
    // the options: clients, then the projects of the chosen client (archived ones last)
    let mut client_opts: Vec<(ClientFilter, String)> = vec![(ClientFilter::All, format!("{}: {}", tr("Client"), tr("All clients"))), (ClientFilter::NoClient, format!("{}: {}", tr("Client"), tr("No client")))];
    for cl in app.store.db.live_clients() {
        let arch = if cl.archived { format!(" ({})", tr("archived")) } else { String::new() };
        client_opts.push((ClientFilter::Client(cl.id.clone()), format!("{}: {}{}", tr("Client"), cl.name, arch)));
    }
    let mut project_opts: Vec<(ProjectFilter, String)> = vec![(ProjectFilter::All, format!("{}: {}", tr("Project"), tr("All projects"))), (ProjectFilter::NoProject, format!("{}: {}", tr("Project"), tr("No project")))];
    let mut projects: Vec<Project> = app.store.db.live_projects().into_iter().filter(|p| match &app.act.all_client {
        ClientFilter::All => true,
        ClientFilter::NoClient => app.store.db.client(&p.client_id).is_none(),
        ClientFilter::Client(id) => p.client_id.as_ref() == Some(id),
    }).cloned().collect();
    projects.sort_by_key(|p| (p.archived, p.name.to_lowercase()));
    for pr in &projects {
        let arch = if pr.archived { format!(" ({})", tr("archived")) } else { String::new() };
        project_opts.push((ProjectFilter::Project(pr.id.clone()), format!("{}: {}{}", tr("Project"), pr.name, arch)));
    }
    // a project outside the chosen client is no longer a choice
    if !project_opts.iter().any(|(v, _)| *v == app.act.all_project) {
        app.act.all_project = ProjectFilter::All;
    }
    let source_opts = vec![(SourceFilter::All, format!("{}: {}", tr("Source"), tr("All sources"))), (SourceFilter::Local, format!("{}: {}", tr("Source"), tr("Added in DevTrack"))), (SourceFilter::Azure, format!("{}: Azure DevOps", tr("Source")))];
    let show_opts = vec![(0u8, tr("Open tasks")), (1u8, tr("Done tasks")), (2u8, tr("All tasks"))];
    let estimate_opts = vec![(EstimateFilter::All, format!("{}: {}", tr("Estimates"), tr("All estimates"))), (EstimateFilter::Over, format!("{}: {}", tr("Estimates"), tr("Over the planned hours"))), (EstimateFilter::Missing, format!("{}: {}", tr("Estimates"), tr("No planned hours")))];
    // two rows: the words, which tasks and the action; then where they are, where they come
    // from and how their estimates stand
    let show_w = 170.0;
    let narrow = w < 560.0;
    let add_w = (text_w(ui, &tr("+ Add task"), &sans(14.0)) + PAD_X * 2.0).max(BTN_MIN_W);
    let mut add = false;
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(CTRL_H);
        ui.spacing_mut().item_spacing = egui::vec2(ROW_GAP, ROW_GAP);
        search_field(ui, &mut app.act.all_query, "Search tasks", "Looks in the task, its #number, project and client.", if narrow { w } else { w - show_w - add_w - ROW_GAP * 2.0 });
        app.geometry.rows.push(("tasks toolbar · search".into(), last_input_rect(ui)));
        select(ui, "all-task-show", &mut app.act.all_show, &show_opts, if narrow { w } else { show_w }, false);
        app.geometry.rows.push(("tasks toolbar · show".into(), last_trigger_rect(ui)));
        let r = button_w(ui, Kind::Secondary, "+ Add task", if narrow { w } else { add_w }, true);
        app.geometry.rows.push(("tasks toolbar · add".into(), r.rect));
        add = r.clicked();
    });
    if add {
        // the project filtered on, otherwise the one worked on last
        let project = match &app.act.all_project { ProjectFilter::Project(id) => Some(id.clone()), _ => recent_projects(&app.store.db).into_iter().next() };
        app.remember_focus(ui.ctx());
        app.act.open_add_choose(false, project);
    }
    ui.add_space(ROW_GAP);
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(CTRL_H);
        ui.spacing_mut().item_spacing = egui::vec2(ROW_GAP, ROW_GAP);
        // the tag menu only where there are tags (or one is still chosen)
        let tags = !app.store.db.live_tags().is_empty() || app.act.all_tag.is_some();
        let n = if tags { 5.0 } else { 4.0 };
        // a pixel short of an equal share: rounding must not push the last one to a new row
        let third = if narrow { w } else { ((w - ROW_GAP * (n - 1.0)) / n).floor() - 1.0 };
        select_searchable(ui, "all-task-client", &mut app.act.all_client, &client_opts, third);
        select_searchable(ui, "all-task-project", &mut app.act.all_project, &project_opts, third);
        select(ui, "all-task-source", &mut app.act.all_source, &source_opts, third, false);
        select(ui, "all-task-estimate", &mut app.act.all_estimate, &estimate_opts, third, false);
        if tags {
            let mut f = app.act.all_tag.clone();
            if tag_filter_with(ui, "all-task-tag", &app.store.db, &mut f, third, true) {
                app.act.all_tag = f;
            }
        }
    });
    ui.add_space(GROUP_GAP - ROW_GAP);
    let q = app.act.all_query.trim().to_string();
    let db = &app.store.db;
    let (fc, fp, fs, fe) = (app.act.all_client.clone(), app.act.all_project.clone(), app.act.all_source, app.act.all_estimate);
    let ft = app.act.all_tag.clone();
    let scope = db.live_tasks().filter(|t| match app.act.all_show { 0 => !t.done, 1 => t.done, _ => true }).count();
    let mut rows: Vec<(Task, String, String, Option<i64>, i64)> = db
        .live_tasks()
        .filter(|t| match app.act.all_show { 0 => !t.done, 1 => t.done, _ => true })
        .filter(|t| {
            let pr = db.project(&t.project_id);
            let client = pr.and_then(|p| p.client_id.clone()).filter(|c| db.client(&Some(c.clone())).is_some());
            let client_ok = match &fc { ClientFilter::All => true, ClientFilter::NoClient => client.is_none(), ClientFilter::Client(id) => client.as_ref() == Some(id) };
            let project_ok = match &fp { ProjectFilter::All => true, ProjectFilter::NoProject => pr.is_none(), ProjectFilter::Project(id) => pr.map(|p| &p.id) == Some(id) };
            let source_ok = match fs { SourceFilter::All => true, SourceFilter::Local => t.source != SOURCE_AZURE, SourceFilter::Azure => t.source == SOURCE_AZURE };
            let estimate_ok = match fe {
                EstimateFilter::All => true,
                EstimateFilter::Over => t.planned_secs().is_some_and(|est| db.task_total(&t.id, now) > est),
                EstimateFilter::Missing => t.planned_secs().is_none(),
            };
            // a task has the tags of its sessions
            let tag_ok = match &ft {
                None => true,
                Some(f @ TagFilter::Tag(_)) => db.live_entries().any(|e| e.task_id.as_deref() == Some(t.id.as_str()) && f.matches(e)),
                Some(TagFilter::Untagged) => db.live_entries().all(|e| e.task_id.as_deref() != Some(t.id.as_str()) || TagFilter::Untagged.matches(e)),
            };
            client_ok && project_ok && source_ok && estimate_ok && tag_ok
        })
        .map(|t| {
            let pr = db.project(&t.project_id);
            let place = match pr {
                Some(pr) => {
                    let cl = db.client_name(pr);
                    if cl.is_empty() { pr.name.clone() } else { format!("{} · {}", pr.name, cl) }
                }
                None => tr("No project"),
            };
            let color = pr.map(|p| p.color.clone()).unwrap_or_default();
            let last = db.live_entries().filter(|e| e.task_id.as_deref() == Some(t.id.as_str())).map(|e| e.start).max();
            (t.clone(), place, color, last, db.task_total(&t.id, now))
        })
        .filter(|(t, place, ..)| search_matches(&q, &[&t.title, place, t.external_id.as_deref().unwrap_or("")]))
        .collect();
    // most recent work first; tasks never tracked follow, by name
    rows.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.0.title.to_lowercase().cmp(&b.0.title.to_lowercase())));
    let filtered = !q.is_empty() || app.act.all_client != ClientFilter::All || app.act.all_project != ProjectFilter::All || app.act.all_source != SourceFilter::All || app.act.all_estimate != EstimateFilter::All || app.act.all_tag.is_some();
    let mut clear = false;
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        if rows.len() == scope {
            mixed(ui, &plural(rows.len() as i64, "{} task", "{} tasks"), 12.0, Some(c.text2));
        } else {
            mixed(ui, &trf("{} of {} tasks", &[&rows.len(), &scope]), 12.0, Some(c.text2));
        }
        // with no result the panel below carries the one "Clear filters"
        if filtered && !rows.is_empty() && link_btn(ui, "Clear filters").clicked() {
            clear = true;
        }
    });
    ui.add_space(4.0);
    if rows.is_empty() {
        let title = if filtered { "No task matches these filters" } else { match app.act.all_show { 0 => "No open tasks", 1 => "No done tasks", _ => "No tasks" } };
        empty_state(ui, Icon::Activities, title, "Change the search, client, project, source, estimates or tag, or which tasks to show.", |ui| {
            if filtered { clear = secondary(ui, "Clear filters").clicked(); }
        });
    }
    if clear {
        app.act.all_query.clear();
        app.act.all_client = ClientFilter::All;
        app.act.all_project = ProjectFilter::All;
        app.act.all_source = SourceFilter::All;
        app.act.all_estimate = EstimateFilter::All;
        app.act.all_tag = None;
    }
    if rows.is_empty() { return; }
    let tasks: Vec<Task> = rows.into_iter().map(|r| r.0).collect();
    let list = egui::ScrollArea::vertical().id_salt("all-tasks").auto_shrink([false, false]).show(ui, |ui| {
        task_rows(app, ui, &tasks, true);
    });
    scroll_fades(ui, &list, p(ui).canvas);
}

fn project_matches(app: &App, p: &Project) -> bool {
    let a = &app.act;
    let status_ok = match a.status_filter {
        StatusFilter::Active => !p.archived,
        StatusFilter::Archived => p.archived,
        StatusFilter::All => true,
    };
    let client_ok = match &a.client_filter {
        ClientFilter::All => true,
        ClientFilter::NoClient => app.store.db.client(&p.client_id).is_none(),
        ClientFilter::Client(id) => p.client_id.as_ref() == Some(id),
    };
    let text_ok = search_matches(&a.search, &[&p.name, &app.store.db.client_name(p)]);
    status_ok && client_ok && text_ok
}

fn filters_active(app: &App) -> bool {
    !app.act.search.trim().is_empty() || app.act.client_filter != ClientFilter::All || app.act.status_filter != StatusFilter::Active
}

fn clear_filters(app: &mut App) {
    app.act.search.clear();
    app.act.client_filter = ClientFilter::All;
    app.act.status_filter = StatusFilter::Active;
}

/// Project browser: one toolbar (search, client, status, New project) and consistent rows.
fn browser(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let now = now_secs();
    let total = app.store.db.live_projects().len();
    if total == 0 && app.store.db.live_tasks().next().is_none() {
        empty_projects(app, ui);
        return;
    }
    let available = ui.available_width();
    let mut opts: Vec<(ClientFilter, String)> = vec![(ClientFilter::All, format!("{}: {}", tr("Client"), tr("All clients"))), (ClientFilter::NoClient, format!("{}: {}", tr("Client"), tr("No client")))];
    for cl in app.store.db.live_clients() {
        let arch = if cl.archived { format!(" ({})", tr("archived")) } else { String::new() };
        opts.push((ClientFilter::Client(cl.id.clone()), format!("{}: {}{}", tr("Client"), cl.name, arch)));
    }
    let sopts: Vec<(StatusFilter, String)> = [(StatusFilter::Active, "Active"), (StatusFilter::Archived, "Archived"), (StatusFilter::All, "All")].iter().map(|(v, l)| (*v, format!("{}: {}", tr("Status"), tr(l)))).collect();
    let action_w = ui.painter().layout_no_wrap(tr("+ New project"), medium(14.0), c.text).size().x + PAD_X * 2.0;
    let one_row = available >= 260.0 + 230.0 + 170.0 + action_w + ROW_GAP * 3.0;
    let mut create = false;
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(CTRL_H);
        // the search takes the width the filters and the action leave free
        let search_w = if one_row { available - 230.0 - 170.0 - action_w - ROW_GAP * 3.0 } else if available >= 440.0 { available - action_w - ROW_GAP } else { available };
        search_field(ui, &mut app.act.search, "Search projects", "Looks in the project and its client.", search_w);
        app.geometry.rows.push(("activities toolbar · search".into(), last_input_rect(ui)));
        if one_row {
            select_searchable(ui, "act-client-filter", &mut app.act.client_filter, &opts, 230.0);
            app.geometry.rows.push(("activities toolbar · client".into(), last_trigger_rect(ui)));
            select(ui, "act-status-filter", &mut app.act.status_filter, &sopts, 170.0, false);
            app.geometry.rows.push(("activities toolbar · status".into(), last_trigger_rect(ui)));
        }
        if available < 440.0 { ui.end_row(); }
        let r = button_w(ui, Kind::Primary, "+ New project", action_w, true);
        app.geometry.rows.push(("activities toolbar · new project".into(), r.rect));
        create = r.clicked();
    });
    if !one_row {
        ui.horizontal_wrapped(|ui| {
            let client_w = if available >= 440.0 { (available - ROW_GAP) * 0.58 } else { available };
            select_searchable(ui, "act-client-filter", &mut app.act.client_filter, &opts, client_w);
            app.geometry.rows.push(("activities toolbar · client".into(), last_trigger_rect(ui)));
            if available < 440.0 { ui.end_row(); }
            let status_w = if available >= 440.0 { available - client_w - ROW_GAP } else { available };
            select(ui, "act-status-filter", &mut app.act.status_filter, &sopts, status_w, false);
            app.geometry.rows.push(("activities toolbar · status".into(), last_trigger_rect(ui)));
        });
    }
    if create {
        let client = match &app.act.client_filter { ClientFilter::Client(id) => Some(id.clone()), _ => None };
        open_project_form(app, ui.ctx(), None, client);
    }
    let mut list: Vec<Project> = app.store.db.live_projects().into_iter().filter(|p| project_matches(app, p)).cloned().collect();
    // the order chosen on the titles: name, client, open tasks, tracked time
    {
        let (col, asc) = work_sort(app, PROJECT_SORT.0, PROJECT_SORT.1);
        let db = &app.store.db;
        let open_tasks = |p: &Project| db.live_tasks().filter(|t| t.project_id.as_deref() == Some(p.id.as_str()) && !t.done).count();
        let tracked = |p: &Project| db.live_entries().filter(|e| e.project_id.as_deref() == Some(p.id.as_str())).map(|e| e.duration(now)).sum::<i64>();
        let name = |p: &Project| p.name.to_lowercase();
        let mut keyed: Vec<(Project, String, usize, i64)> = list.into_iter().map(|p| { let cl = db.client_name(&p).to_lowercase(); let (n, t) = (open_tasks(&p), tracked(&p)); (p, cl, n, t) }).collect();
        keyed.sort_by(|a, b| {
            let o = match col {
                1 => a.1.cmp(&b.1),
                2 => a.2.cmp(&b.2),
                3 => a.3.cmp(&b.3),
                _ => name(&a.0).cmp(&name(&b.0)),
            };
            let o = if asc { o } else { o.reverse() };
            // by client, those without one stay last either way
            let none_last = if col == 1 { a.1.is_empty().cmp(&b.1.is_empty()) } else { std::cmp::Ordering::Equal };
            none_last.then(o).then_with(|| name(&a.0).cmp(&name(&b.0)))
        });
        list = keyed.into_iter().map(|x| x.0).collect();
    }
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        // counted against the chosen status: archived projects are not "missing" from the
        // active list
        let scope = app.store.db.live_projects().into_iter().filter(|p| match app.act.status_filter {
            StatusFilter::Active => !p.archived,
            StatusFilter::Archived => p.archived,
            StatusFilter::All => true,
        }).count().min(total);
        if list.len() == scope {
            mixed(ui, &plural(scope as i64, "{} project", "{} projects"), 12.0, Some(c.text2));
        } else {
            mixed(ui, &trf("{} of {} projects", &[&list.len(), &scope]), 12.0, Some(c.text2));
        }
        // with no result the panel below carries the one "Clear filters"
        if filters_active(app) && !list.is_empty() && link_btn(ui, "Clear filters").clicked() {
            clear_filters(app);
        }
    });
    ui.add_space(4.0);
    if list.is_empty() {
        let mut clear = false;
        empty_state(ui, Icon::Activities, "No projects match these filters", "Change the search, client or status, or clear the filters.", |ui| {
            clear = secondary(ui, "Clear filters").clicked();
        });
        if clear {
            clear_filters(app);
        }
        return;
    }
    let w = ui.available_width();
    let (track_w, act_w) = (120.0, CTRL_H);
    let compact_rows = w < 600.0;
    // open tasks, between the client and the tracked time; in compact rows it joins the client line
    let tasks_w = if compact_rows { 0.0 } else { 88.0 };
    let client_w = if compact_rows { 0.0 } else { ((w - track_w - tasks_w - act_w - 48.0) * 0.4).clamp(120.0, 280.0) };
    let name_w = (w - client_w - track_w - tasks_w - act_w - 48.0).max(72.0);
    // header, 36 high, joined to the rows: one bordered table like Report Time's
    ui.spacing_mut().item_spacing.y = 0.0;
    let (hr, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), Sense::hover());
    ui.painter().rect_filled(hr, TABLE_TOP, c.nav);
    // The header stays put while the rows scroll, so its part of the table's border is drawn
    // here: the top and the two sides, joining the border the rows draw below.
    ui.painter().with_clip_rect(hr).rect_stroke(egui::Rect::from_min_max(hr.min, egui::pos2(hr.right(), hr.bottom() + 16.0)), 10.0, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
    let hy = hr.center().y;
    // each title orders the list: names A–Z, counts and time largest first
    let (key, def) = PROJECT_SORT;
    work_title(app, ui, key, def, 0, true, egui::pos2(hr.left() + 14.0, hy), false, "Project");
    if !compact_rows { work_title(app, ui, key, def, 1, true, egui::pos2(hr.left() + 14.0 + name_w + 8.0, hy), false, "Client"); }
    if !compact_rows { work_title(app, ui, key, def, 2, false, egui::pos2(hr.right() - CTRL_H - 16.0 - track_w, hy), true, "Tasks"); }
    work_title(app, ui, key, def, 3, false, egui::pos2(hr.right() - CTRL_H - 16.0, hy), true, "Tracked");
    let mut open: Option<Id> = None;
    let mut action: Option<(u8, Project)> = None;
    let list = egui::ScrollArea::vertical().id_salt("act-browser").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (ri, pr) in list.iter().enumerate() {
            let last = ri + 1 == list.len();
            let tracked: i64 = app.store.db.live_entries().filter(|e| e.project_id.as_deref() == Some(pr.id.as_str())).map(|e| e.duration(now)).sum();
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, if compact_rows { 68.0 } else { 52.0 }), Sense::click());
            app.geometry.rows.push((format!("project row · {}", pr.id), rect));
            // the fill is the whole row, edge to edge; the last one follows the table's corners
            ui.painter().rect_filled(rect, if last { TABLE_BOTTOM } else { egui::CornerRadius::ZERO }, hover_mix(ui, resp.id, resp.hovered(), Color32::TRANSPARENT, c.hover));
            if resp.has_focus() {
                ui.painter().rect_stroke(rect.shrink(1.0), RADIUS, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
            }
            let y = rect.center().y;
            ui.painter().circle_filled(egui::pos2(rect.left() + 18.0, y), 4.0, hex(&pr.color));
            let max = |px: f32| ((px - 16.0) / 7.6).max(6.0) as usize;
            let mut name_px = name_w - 18.0;
            if pr.archived {
                let g = ui.painter().layout_no_wrap(tr("Archived"), sans(12.0), c.text2);
                name_px -= g.size().x + 16.0;
                ui.painter().galley(egui::pos2(rect.left() + 14.0 + name_w - g.size().x - 8.0, y - g.size().y / 2.0), g, c.text2);
            }
            ui.painter().text(egui::pos2(rect.left() + 32.0, if compact_rows { y - 12.0 } else { y }), egui::Align2::LEFT_CENTER, truncate(&pr.name, max(name_px)), medium(14.0), if pr.archived { c.text2 } else { c.text });
            let cname = app.store.db.client_name(pr);
            let mut ctext = if cname.is_empty() { tr("No client") } else { cname.clone() };
            let open_tasks = app.store.db.live_tasks().filter(|t| t.project_id.as_deref() == Some(pr.id.as_str()) && !t.done).count();
            if compact_rows {
                ctext = format!("{} · {}", ctext, plural(open_tasks as i64, "{} task", "{} tasks"));
            } else {
                let col = if open_tasks == 0 { c.text2 } else { c.text };
                ui.painter().text(egui::pos2(rect.right() - CTRL_H - 16.0 - track_w, y), egui::Align2::RIGHT_CENTER, if open_tasks == 0 { "—".to_string() } else { open_tasks.to_string() }, mono_font(14.0), col);
            }
            ui.painter().text(egui::pos2(if compact_rows { rect.left() + 32.0 } else { rect.left() + 14.0 + name_w + 8.0 }, if compact_rows { y + 12.0 } else { y }), egui::Align2::LEFT_CENTER, truncate(&ctext, max(if compact_rows { w - 72.0 } else { client_w })), sans(13.0), c.text2);
            {
            // durations in words: digits in Mono, units in Sans
            let col = if tracked == 0 { c.text2 } else { c.text };
            let g = ui.painter().layout_job(mixed_job(&total_text(tracked), 14.0, col, sans(14.0), mono_font(14.0)));
            ui.painter().galley(egui::pos2(rect.right() - CTRL_H - 16.0 - g.size().x, (if compact_rows { y - 12.0 } else { y }) - g.size().y / 2.0), g, col);
        }
            let name = format!("{} · {}", pr.name, ctext);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone()));
            let more_rect = egui::Rect::from_center_size(egui::pos2(rect.right() - act_w / 2.0 - 4.0, y), egui::vec2(CTRL_H, CTRL_H));
            // in a child of its own: placing it must not move where the next row starts
            let more = {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(more_rect));
                icon_button(&mut child, Glyph::More, "Project actions")
            };
            let pid = ui.make_persistent_id(("project-row-more", &pr.id));
            if more.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pid));
            } else if resp.on_hover_text(tr("Open the project's tasks")).clicked() {
                open = Some(pr.id.clone());
            }
            egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(220.0);
                if menu_row(ui, "Open", false, false).clicked() {
                    open = Some(pr.id.clone());
                }
                if menu_row(ui, "Edit project…", false, false).clicked() {
                    action = Some((0, pr.clone()));
                }
                if menu_row(ui, if pr.archived { "Unarchive" } else { "Archive" }, false, false).clicked() {
                    action = Some((1, pr.clone()));
                }
                if menu_row(ui, "Delete project…", false, false).clicked() {
                    action = Some((2, pr.clone()));
                }
            });
            if app.act.confirm_delete.as_deref() == Some(pr.id.as_str()) {
                ui.add_space(4.0);
                row(ui, CTRL_H, |ui| {
                    ui.label(RichText::new(tr("Delete this project? Its sessions keep their time but lose the project.")).color(c.red));
                    if danger(ui, "Delete").clicked() {
                        app.store.delete_project(&pr.id);
                        app.act.confirm_delete = None;
                    }
                    if ghost(ui, "Keep").clicked() {
                        app.act.confirm_delete = None;
                    }
                });
                ui.add_space(4.0);
            }
            divider(ui);
        }
        table_border(ui, hr.top());
        let unassigned = app.store.db.live_tasks().filter(|t| app.store.db.project(&t.project_id).is_none()).count();
        if unassigned > 0 {
            ui.add_space(8.0);
            if link_btn(ui, &trf("Tasks without a project · {}", &[&unassigned])).clicked() {
                open = Some(NO_PROJECT.into());
            }
        }
    });
    scroll_fades(ui, &list, p(ui).canvas);
    if let Some(id) = open {
        app.act.open_project(id);
        app.fade_content();
    }
    if let Some((k, pr)) = action {
        match k {
            0 => open_project_form(app, ui.ctx(), Some(pr.id.clone()), None),
            1 => {
                let mut x = pr.clone();
                x.archived = !x.archived;
                app.store.put_project(x);
            }
            _ => app.act.confirm_delete = Some(pr.id.clone()),
        }
    }
}

fn empty_projects(app: &mut App, ui: &mut egui::Ui) {
    let has_conn = app.store.db.integrations.values().any(|c| !c.deleted);
    let (mut create, mut import, mut connect) = (false, false, false);
    empty_state(ui, Icon::Activities, "No projects yet", "Projects group your sessions and decide how they're reported. Add tasks yourself, or bring work items assigned to you from Azure DevOps.", |ui| {
        create = primary(ui, "Create project").clicked();
        if has_conn {
            import = secondary(ui, "Import from Azure DevOps").clicked();
        } else {
            connect = secondary(ui, "Connect Azure DevOps").clicked();
        }
    });
    if create {
        open_project_form(app, ui.ctx(), None, None);
    }
    if import {
        app.act.open_project(NO_PROJECT.into());
        app.act.open_add(true);
    }
    if connect {
        connect_azure(app);
    }
}

/// Detail routes carry both their origin and record identity; root tabs are only on lists.
/// The steps' names for the breadcrumb ("Tasks without a project" for the unassigned list).
fn crumb_name(app: &App, c: &Crumb) -> String {
    match c {
        Crumb::Project(id) if id == NO_PROJECT => tr("Tasks without a project"),
        Crumb::Project(id) => app.store.db.projects.get(id).map(|p| p.name.clone()).unwrap_or_default(),
        Crumb::Client(id) => app.store.db.client(&Some(id.clone())).map(|c| c.name.clone()).unwrap_or_default(),
    }
}

/// One navigation row: a Back arrow named for where it goes, then the way here: the tab, each
/// step opened since (links), and the current record. Every link returns to that level.
fn detail_navigation(app: &mut App, ui: &mut egui::Ui, current: &str) -> bool {
    let trail: Vec<Crumb> = app.act.trail().to_vec();
    let root_name = match app.act.tab { ActTab::Tasks => "Tasks", ActTab::Projects => "Projects", ActTab::Clients => "Clients", ActTab::Tags => "Tags" };
    let back_label = match trail.len() {
        0 | 1 => tr(match app.act.tab { ActTab::Tasks => "Back to tasks", ActTab::Projects => "Back to projects", ActTab::Clients => "Back to clients", ActTab::Tags => "Back to tags" }),
        n => trf(match &trail[n - 2] { Crumb::Client(_) => "Back to client {}", Crumb::Project(_) => "Back to project {}" }, &[&truncate(&crumb_name(app, &trail[n - 2]), 32)]),
    };
    let mut back = false;
    let mut root = false;
    let mut step: Option<usize> = None;
    row_wrapped(ui, CTRL_H, |ui| {
        let r = icon_button(ui, Glyph::Prev, &back_label).on_hover_text(format!("{} (Alt+←)", back_label));
        app.geometry.rows.push(("activities navigation · back".into(), r.rect));
        back = r.clicked();
        ui.add_space(4.0);
        let r = link_btn(ui, root_name);
        app.geometry.rows.push(("activities navigation · root".into(), r.rect));
        root = r.clicked();
        let sep = |ui: &mut egui::Ui| { ui.add(egui::Label::new(RichText::new("/").size(13.0).color(p(ui).text2)).extend()); };
        for (i, c) in trail.iter().enumerate().take(trail.len().saturating_sub(1)) {
            sep(ui);
            let name = crumb_name(app, c);
            let r = link_btn(ui, &truncate(&name, 24)).on_hover_text(&name);
            app.geometry.rows.push((format!("activities navigation · step {}", i), r.rect));
            if r.clicked() { step = Some(i); }
        }
        sep(ui);
        ui.add(egui::Label::new(RichText::new(current).size(13.0).color(p(ui).text2)).wrap());
    });
    if root { let tab = app.act.tab; app.act.open_root(tab); }
    else if let Some(i) = step { app.act.back_to(i); }
    else if back { app.act.back(); }
    if root || back || step.is_some() { app.fade_content(); return true; }
    ui.add_space(GROUP_GAP);
    false
}

/// Record title gets its own bounded column; actions stack when the window gets narrow.
fn record_header(ui: &mut egui::Ui, name: &str, subtitle: &str, dot: Option<Color32>, actions: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let mut actions = Some(actions);
    // the project's colour dot, as in the list it was opened from, on the first line of the name
    let title = |ui: &mut egui::Ui| {
        ui.horizontal_top(|ui| {
            if let Some(col) = dot {
                let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 36.0), Sense::hover());
                ui.painter().circle_filled(egui::pos2(r.center().x, r.top() + 19.0), 6.0, col);
                ui.add_space(4.0);
            }
            ui.add(egui::Label::new(RichText::new(name).font(semibold(28.0))).wrap());
        });
    };
    if width >= 700.0 {
        ui.horizontal_top(|ui| {
            col(ui, width - 280.0, 0.0, |ui| {
                title(ui);
                if !subtitle.is_empty() { muted(ui, subtitle); }
            });
            ui.allocate_ui_with_layout(egui::vec2(272.0, CTRL_H), egui::Layout::right_to_left(egui::Align::Center), |ui| { if let Some(a) = actions.take() { a(ui); } });
        });
    } else {
        title(ui);
        if !subtitle.is_empty() { muted(ui, subtitle); }
        // the same order as in a wide window: secondary, then the primary action at the end
        ui.allocate_ui_with_layout(egui::vec2(width, CTRL_H), egui::Layout::right_to_left(egui::Align::Center), |ui| { if let Some(a) = actions.take() { a(ui); } });
    }
}

/// A project's task workspace: read-only header with one Edit project action, then tasks.
fn workspace(app: &mut App, ui: &mut egui::Ui, id: Id) {
    let c = p(ui);
    if id == NO_PROJECT {
        if detail_navigation(app, ui, &tr("Tasks without a project")) { return; }
        page_header(ui, "Tasks without a project", "Imported or created before a project was chosen. Move them with the row's actions.", |_| {});
        task_toolbar(app, ui);
        let list = egui::ScrollArea::vertical().id_salt("act-main").auto_shrink([false, false]).show(ui, |ui| task_table(app, ui, None));
        scroll_fades(ui, &list, p(ui).canvas);
        return;
    }
    let Some(pr) = app.store.db.projects.get(&id).filter(|p| !p.deleted).cloned() else {
        app.act.back();
        return;
    };
    if detail_navigation(app, ui, &pr.name) { return; }
    let cname = app.store.db.client_name(&pr);
    let mut edit = false;
    let mut start = false;
    record_header(ui, &pr.name, "", Some(hex(&pr.color)), |ui| {
        if !pr.archived { start = primary(ui, "Start").on_hover_text(tr("Start a session on this project")).clicked(); }
        edit = secondary(ui, "Edit project").clicked();
        if pr.archived { pill(ui, "Archived", c.text2); }
    });
    // The project's facts, each with its name: who it is for, how much time, how many sessions
    // and open tasks. Four across, two in a narrow window.
    let now = now_secs();
    let tracked: i64 = app.store.db.live_entries().filter(|e| e.project_id.as_deref() == Some(pr.id.as_str())).map(|e| e.duration(now)).sum();
    let sessions = app.store.db.live_entries().filter(|e| e.project_id.as_deref() == Some(pr.id.as_str())).count();
    let open_tasks = app.store.db.live_tasks().filter(|t| t.project_id.as_deref() == Some(pr.id.as_str()) && !t.done).count();
    let mut to_client = false;
    ui.add_space(GROUP_GAP - ROW_GAP);
    // the project's colour as the line that opens its facts: the same band as on Today
    let (band, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.0), Sense::hover());
    ui.painter().rect_filled(band, 1.5, hex(&pr.color));
    let available = ui.available_width();
    let columns = if available >= 860.0 { 5 } else if available >= 520.0 { 3 } else { 2 };
    let cell_w = (available - ROW_GAP * (columns - 1) as f32) / columns as f32;
    // planned hours: the sum of the tasks' own, never typed here. They are compared with the time
    // tracked on those same tasks, so tasks without an estimate don't make it look overrun.
    let project_tasks: Vec<Task> = app.store.db.live_tasks().filter(|t| t.project_id.as_deref() == Some(pr.id.as_str())).cloned().collect();
    let planned: Vec<&Task> = project_tasks.iter().filter(|t| t.planned_secs().is_some()).collect();
    let planned_sum: i64 = planned.iter().filter_map(|t| t.planned_secs()).sum();
    let planned_tracked: i64 = planned.iter().map(|t| app.store.db.task_total(&t.id, now)).sum();
    egui::Grid::new(("project-facts", &pr.id)).num_columns(columns).spacing([ROW_GAP, 12.0]).show(ui, |ui| {
        for i in 0..5 {
            ui.vertical(|ui| {
                ui.set_width(cell_w);
                ui.spacing_mut().item_spacing.y = 4.0;
                match i {
                    0 => {
                        label(ui, "Client");
                        if cname.is_empty() {
                            ui.label(RichText::new(tr("No client")).font(medium(16.0)).color(c.text2));
                        } else {
                            let r = ui.add(egui::Label::new(RichText::new(&cname).font(medium(16.0))).truncate().sense(Sense::click())).on_hover_text(tr("Open the client's projects"));
                            if r.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                            to_client = r.clicked();
                        }
                    }
                    1 => {
                        label(ui, "Tracked");
                        let (text, col) = if tracked == 0 { ("—".to_string(), c.text2) } else { (words(tracked), c.text) };
                        ui.label(mixed_job(&text, 16.0, col, medium(16.0), mono_medium(16.0)));
                    }
                    2 => {
                        label(ui, "Sessions");
                        ui.label(RichText::new(sessions.to_string()).font(mono_medium(16.0)).color(if sessions == 0 { c.text2 } else { c.text }));
                    }
                    3 => {
                        label(ui, "Open tasks");
                        ui.label(RichText::new(open_tasks.to_string()).font(mono_medium(16.0)).color(if open_tasks == 0 { c.text2 } else { c.text }));
                    }
                    _ => {
                        label(ui, "Planned hours");
                        // value, bar and words share the cell's width and left edge
                        ui.set_max_width(cell_w);
                        if planned.is_empty() {
                            ui.label(RichText::new("—").font(mono_medium(16.0)).color(c.text2));
                            meta(ui, "Sum of the planned hours of the tasks below.");
                        } else {
                            ui.label(mixed_job(&words(planned_sum), 16.0, c.text, medium(16.0), mono_medium(16.0)));
                            ui.add_space(2.0);
                            estimate_bar(ui, cell_w, planned_tracked, planned_sum, hex(&pr.color));
                            estimate_status(ui, planned_tracked, planned_sum);
                            meta(ui, &if planned.len() == project_tasks.len() { plural(planned.len() as i64, "From {} task", "Sum of {} tasks") } else { trf_m!("Sum of {} of {} tasks", planned.len(), project_tasks.len()) });
                        }
                    }
                }
            });
            if (i + 1) % columns == 0 { ui.end_row(); }
        }
    });
    divider(ui);
    if to_client {
        if let Some(cid) = pr.client_id.clone() {
            app.act.open_client(cid);
            app.fade_content();
            return;
        }
    }
    if edit { open_project_form(app, ui.ctx(), Some(pr.id.clone()), None); }
    if start {
        app.store.start("", Some(pr.id.clone()), None, pr.billable_default);
        app.notify(trf_m!("Tracking “{}”", truncate(&pr.name, 40)));
        app.go(Page::Today);
    }
    ui.add_space(GROUP_GAP + 8.0);
    task_toolbar(app, ui);
    let list = egui::ScrollArea::vertical().id_salt(("act-main", &pr.id)).auto_shrink([false, false]).show(ui, |ui| {
        task_table(app, ui, Some(pr.id.clone()));
    });
    scroll_fades(ui, &list, p(ui).canvas);
}

/// Searchable client selector backed by client ids, with No client and Create client.
/// Returns Some(name) when "Create client" was chosen.
fn client_picker(ui: &mut egui::Ui, db: &Db, value: &mut Option<Id>, width: f32) -> Option<String> {
    let mut create = None;
    let (txt, ph) = match db.client(value) {
        Some(cl) => (if cl.archived { format!("{} ({})", cl.name, tr("archived")) } else { cl.name.clone() }, false),
        None => (tr("No client"), true),
    };
    let pid = ui.make_persistent_id("client-popup");
    let sid = ui.make_persistent_id("client-q");
    let resp = select_trigger(ui, &txt, ph, width, true, false, false);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pid));
        ui.data_mut(|d| d.insert_temp(sid.with("initial-focus"), true));
    }
    egui::popup_below_widget(ui, pid, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(width);
        let mut q: String = ui.data_mut(|d| d.get_temp::<String>(sid).unwrap_or_default());
        let r = input(ui, &mut q, Field::new("Search clients", ui.available_width()).hint("Search clients"));
        if ui.data_mut(|d| d.remove_temp::<bool>(sid.with("initial-focus"))).unwrap_or(false) {
            r.request_focus();
        }
        ui.data_mut(|d| d.insert_temp(sid, q.clone()));
        let mut chosen: Option<Option<Id>> = None;
        if q.trim().is_empty() && menu_row(ui, "No client", value.is_none(), false).clicked() {
            chosen = Some(None);
        }
        let list: Vec<&Client> = db.live_clients().into_iter().filter(|c| !c.archived || value.as_ref() == Some(&c.id)).filter(|c| q.trim().is_empty() || c.name.to_lowercase().contains(&q.trim().to_lowercase())).collect();
        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            for cl in &list {
                if menu_row_raw(ui, &cl.name, value.as_ref() == Some(&cl.id), false).clicked() {
                    chosen = Some(Some(cl.id.clone()));
                }
            }
        });
        if list.is_empty() && !q.trim().is_empty() {
            meta(ui, trf("No client matches “{}”.", &[&q.trim()]));
        }
        ui.separator();
        let label_txt = if q.trim().is_empty() { tr("+ Create client") } else { trf("+ Create client “{}”", &[&truncate(q.trim(), 24)]) };
        if menu_row_raw(ui, &label_txt, false, false).clicked() {
            create = Some(q.trim().to_string());
            ui.data_mut(|d| d.remove::<String>(sid));
            ui.memory_mut(|m| m.close_popup());
        }
        if let Some(v) = chosen {
            *value = v;
            resp.request_focus();
            ui.data_mut(|d| d.remove::<String>(sid));
            ui.memory_mut(|m| m.close_popup());
        }
    });
    create
}

/// One compact swatch trigger; the palette opens in a popup (each swatch is focusable and named).
fn color_picker(ui: &mut egui::Ui, value: &mut String, width: f32) {
    let c = p(ui);
    let pid = ui.make_persistent_id("color-popup");
    let resp = select_trigger(ui, &format!("      {}", color_name(value)), false, width, true, false, false);
    ui.painter().circle_filled(egui::pos2(resp.rect.left() + PAD_X + 6.0, resp.rect.center().y), 6.0, hex(value));
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pid));
    }
    egui::popup_below_widget(ui, pid, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(7.0 * 36.0 + 12.0);
        for row in PROJECT_COLORS.chunks(7) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for col in row {
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, RADIUS, c.hover);
                    }
                    ui.painter().circle_filled(r.center(), 9.0, hex(col));
                    if value.eq_ignore_ascii_case(col) {
                        ui.painter().circle_stroke(r.center(), 12.5, Stroke::new(1.5, c.text));
                    }
                    if resp.has_focus() {
                        ui.painter().rect_stroke(r, RADIUS, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
                    }
                    let name = color_name(col);
                    let sel = value.eq_ignore_ascii_case(col);
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, sel, name.clone()));
                    if resp.on_hover_text(color_name(col)).clicked() {
                        *value = col.to_string();
                        ui.memory_mut(|m| m.close_popup());
                    }
                }
            });
        }
    });
}

/// Edit project / New project: content at most 480 wide; Create client is a subview with Back.
pub fn project_modal(app: &mut App, ctx: &egui::Context) {
    let Some(mut f) = app.act.form.take() else { return };
    let dark = app.dark();
    let mut close = false;
    let mut saved: Option<Project> = None;
    if let Some(mut name) = f.new_client.clone() {
        // ---- "Create client" inside the project dialog: the same frame, a step back to it ----
        let dialog = FormDialog {
            id: "project-form",
            title: "New client",
            context: Some(tr("The project draft is kept; you'll return to it with this client selected.")),
            width: FORM_W,
            primary: "Create client",
            primary_enabled: true,
            dirty: !name.trim().is_empty(),
            note: None,
            note_link: Some("‹ Back"),
        };
        let mut confirm = f.confirm_discard;
        let r = form_dialog(ctx, dark, dialog, &mut confirm, |ui, w| client_name_field(app, ui, w, &mut name, f.attempted, None, &mut f.focused));
        f.confirm_discard = confirm;
        f.new_client = Some(name.clone());
        if r.submit {
            f.attempted = true;
            if !name.trim().is_empty() {
                let cl = Client { id: new_id(), name: name.trim().to_string(), ..Default::default() };
                f.client_id = Some(cl.id.clone());
                app.store.put_client(cl);
                app.notify(trf("Client “{}” created", &[&truncate(name.trim(), 40)]));
                f.new_client = None;
                f.attempted = false;
            }
        } else if r.close || r.note_link {
            // back to the project form, never out of the dialog
            f.new_client = None;
            f.attempted = false;
            f.confirm_discard = false;
        }
        app.act.form = Some(f);
        return;
    }
    // ---- project form ----
    let editing = f.id.is_some();
    let dialog = FormDialog {
        id: "project-form",
        title: if editing { "Edit project" } else { "New project" },
        context: None,
        width: FORM_W,
        primary: if editing { "Save" } else { "Create project" },
        primary_enabled: true,
        dirty: f.dirty(),
        note: None,
        note_link: None,
    };
    let mut confirm = f.confirm_discard;
    let mut create_client: Option<String> = None;
    let r = form_dialog(ctx, dark, dialog, &mut confirm, |ui, w| {
        let err = if f.attempted && f.name.trim().is_empty() { Some(tr("Give the project a name")) } else { None };
        let r = field(ui, "Project name", None, err.as_deref(), |ui| input(ui, &mut f.name, Field::new("Project name", w).invalid(err.is_some())));
        if !f.focused {
            r.request_focus();
            f.focused = true;
        }
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(GROUP_GAP - ROW_GAP);
        let mut cid = f.client_id.clone();
        create_client = field(ui, "Client", None, None, |ui| client_picker(ui, &app.store.db, &mut cid, w));
        f.client_id = cid;
        ui.add_space(GROUP_GAP - ROW_GAP);
        field(ui, "Colour", None, None, |ui| color_picker(ui, &mut f.color, w));
        ui.add_space(GROUP_GAP - ROW_GAP);
        checkbox(ui, &mut f.billable, tr("Billable by default"));
        enter
    });
    f.confirm_discard = confirm;
    if let Some(n) = create_client {
        f.new_client = Some(n);
        f.focused = false;
        f.attempted = false;
    }
    if r.submit {
        f.attempted = true;
        if f.name.trim().is_empty() {
            f.focused = false; // focus the invalid name field
        } else {
            let mut pr = f.id.as_ref().and_then(|i| app.store.db.projects.get(i)).cloned().unwrap_or(Project { id: new_id(), ..Default::default() });
            pr.name = f.name.trim().to_string();
            pr.client_id = f.client_id.clone();
            pr.client = app.store.db.client(&f.client_id).map(|c| c.name.clone()).unwrap_or_default();
            pr.color = f.color.clone();
            pr.billable_default = f.billable;
            saved = Some(pr);
        }
    }
    close |= r.close;
    if let Some(pr) = saved {
        let new = f.id.is_none();
        let id = pr.id.clone();
        if Some(&pr) != app.store.db.projects.get(&id) {
            app.store.put_project(pr);
        }
        app.notify(tr(if new { "Project created" } else { "Project saved" }));
        if new && app.act.tab == ActTab::Projects && app.page == Page::Activities {
            app.act.open_project(id);
        }
        close = true;
    }
    if close {
        app.restore_focus(ctx);
    } else {
        app.act.form = Some(f);
    }
}

/// The client's name, as every client dialog asks it: focused on opening, a note when the name
/// is already taken, an error once saving was tried without one. Returns true on Enter.
fn client_name_field(app: &App, ui: &mut egui::Ui, w: f32, name: &mut String, attempted: bool, except: Option<&Id>, focused: &mut bool) -> bool {
    let dup = !name.trim().is_empty() && app.store.db.live_clients().iter().any(|cl| cl.name.trim().eq_ignore_ascii_case(name.trim()) && Some(&cl.id) != except);
    let err = if attempted && name.trim().is_empty() { Some(tr("Give the client a name")) } else { None };
    let help = if dup { Some(tr("A client with this name already exists.")) } else { None };
    let r = field(ui, "Client name", help.as_deref(), err.as_deref(), |ui| input(ui, name, Field::new("Client name", w).invalid(err.is_some())));
    if !*focused {
        r.request_focus();
        *focused = true;
    }
    r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

/// Rename task: the name of a task added in DevTrack, in the same dialog as a client's. Its
/// sessions follow on their own (they point to the task); a name another task of the same
/// project already has is allowed but said.
pub fn task_rename_modal(app: &mut App, ctx: &egui::Context) {
    let Some((id, mut name)) = app.act.task_rename.clone() else { return };
    let Some(task) = app.store.db.tasks.get(&id).cloned() else {
        app.act.task_rename = None;
        return;
    };
    let dialog = FormDialog {
        id: "task-rename",
        title: "Rename task",
        context: None,
        width: FORM_W,
        primary: "Save",
        primary_enabled: true,
        dirty: name.trim() != task.title.trim(),
        note: None,
        note_link: None,
    };
    let dark = app.dark();
    let (mut confirm, mut focused, attempted) = (app.act.task_rename_confirm, app.act.task_rename_focused, app.act.task_rename_attempted);
    let db = &app.store.db;
    let r = form_dialog(ctx, dark, dialog, &mut confirm, |ui, w| {
        let dup = !name.trim().is_empty() && db.live_tasks().any(|t| t.id != id && t.project_id == task.project_id && t.title.trim().eq_ignore_ascii_case(name.trim()));
        let err = if attempted && name.trim().is_empty() { Some(tr("Give the task a name")) } else { None };
        let help = if dup { Some(tr("Another task of this project has this name.")) } else { None };
        let r = field(ui, "Task name", help.as_deref(), err.as_deref(), |ui| input(ui, &mut name, Field::new("Task name", w).invalid(err.is_some())));
        if !focused {
            r.request_focus();
            focused = true;
        }
        r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
    });
    app.act.task_rename_confirm = confirm;
    app.act.task_rename_focused = focused;
    let mut close = r.close;
    if r.submit {
        app.act.task_rename_attempted = true;
        if !name.trim().is_empty() {
            if name.trim() != task.title.trim() {
                app.store.put_task(Task { title: name.trim().to_string(), ..task.clone() });
                app.notify(tr("Task renamed"));
            }
            close = true;
        }
    }
    if close {
        app.act.task_rename = None;
        app.restore_focus(ctx);
    } else {
        app.act.task_rename = Some((id, name));
    }
}

/// The order of one of Work's lists: (column, smallest first), or the list's default.
fn work_sort(app: &App, list: &str, default: (u8, bool)) -> (u8, bool) {
    app.store.local.sorts.get(list).copied().unwrap_or(default)
}

/// A sortable column title of one of Work's lists: a click picks the column in its natural
/// direction (`natural_ascending`: names A–Z; counts, time and dates largest or latest first),
/// a second click turns it round. Kept on this device.
#[allow(clippy::too_many_arguments)]
fn work_title(app: &mut App, ui: &mut egui::Ui, list: &str, default: (u8, bool), col: u8, natural_ascending: bool, at: egui::Pos2, right: bool, label: &str) {
    let (cur, asc) = work_sort(app, list, default);
    let active = cur == col;
    let tip = if active { tr("Click to turn the order round") } else { trf("Order by {}", &[&tr(label).to_lowercase()]) };
    if sort_title(ui, ("work-sort", list, col), at, right, label, active, asc, natural_ascending).on_hover_text(tip).clicked() {
        let next = if active { (col, !asc) } else { (col, natural_ascending) };
        app.store.local.sorts.insert(list.to_string(), next);
        app.store.save_local();
    }
}

/// New client and Rename client: the same dialog as a project's.
pub fn client_modal(app: &mut App, ctx: &egui::Context) {
    let (mut name, renaming) = match app.act.client_edit.clone() {
        Some(ClientEdit::New(n)) => (n, None),
        Some(ClientEdit::Rename(id, n)) => (n, Some(id)),
        _ => return,
    };
    let original = renaming.as_ref().and_then(|id| app.store.db.clients.get(id)).map(|c| c.name.clone());
    let dirty = match &original { Some(o) => name != *o, None => !name.trim().is_empty() };
    let dialog = FormDialog {
        id: "client-form",
        title: if renaming.is_some() { "Rename client" } else { "New client" },
        context: if renaming.is_some() { None } else { Some(tr("A client groups projects for the same customer.")) },
        width: FORM_W,
        primary: if renaming.is_some() { "Save" } else { "Create client" },
        primary_enabled: true,
        dirty,
        note: None,
        note_link: None,
    };
    let dark = app.dark();
    let mut confirm = app.act.client_confirm;
    let mut focused = app.act.client_focused;
    let attempted = app.act.client_attempted;
    let r = form_dialog(ctx, dark, dialog, &mut confirm, |ui, w| client_name_field(app, ui, w, &mut name, attempted, renaming.as_ref(), &mut focused));
    app.act.client_confirm = confirm;
    app.act.client_focused = focused;
    let mut close = r.close;
    if r.submit {
        app.act.client_attempted = true;
        if !name.trim().is_empty() {
            match &renaming {
                Some(id) => {
                    app.store.rename_client(id, &name);
                    app.notify(tr("Client renamed"));
                }
                None => {
                    let cl = Client { id: new_id(), name: name.trim().to_string(), ..Default::default() };
                    app.store.put_client(cl);
                    app.notify(trf("Client “{}” created", &[&truncate(name.trim(), 40)]));
                }
            }
            close = true;
        }
    }
    if close {
        app.act.close_client_edit();
        app.restore_focus(ctx);
    } else {
        app.act.client_edit = Some(match renaming { Some(id) => ClientEdit::Rename(id, name), None => ClientEdit::New(name) });
    }
}

fn clients_tab_ui(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    if let Some(id) = app.act.client_open.clone() {
        client_detail(app, ui, id);
        return;
    }
    let all = app.store.db.live_clients().len();
    let available = ui.available_width();
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(CTRL_H);
        let new_w = ui.painter().layout_no_wrap(tr("+ New client"), medium(14.0), p(ui).text).size().x + PAD_X * 2.0;
        let status_w = 170.0;
        let search_w = if available < 480.0 { available } else { available - new_w - status_w - ROW_GAP * 2.0 };
        search_field(ui, &mut app.act.client_search, "Search clients", "Looks in the client's name.", search_w);
        if available < 480.0 { ui.end_row(); }
        let sopts: Vec<(StatusFilter, String)> = [(StatusFilter::Active, "Active"), (StatusFilter::Archived, "Archived"), (StatusFilter::All, "All")].iter().map(|(v, l)| (*v, format!("{}: {}", tr("Status"), tr(l)))).collect();
        select(ui, "client-status", &mut app.act.client_status, &sopts, if available < 480.0 { available } else { status_w }, false);
        if available < 480.0 { ui.end_row(); }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary(ui, "+ New client").clicked() {
                app.remember_focus(ui.ctx());
                app.act.open_client_edit(ClientEdit::New(String::new()));
            }
        });
    });
    ui.add_space(ROW_GAP);
    client_edit_panel(app, ui);
    for (a, b) in app.store.db.likely_duplicate_clients() {
        let (na, nb) = (app.store.db.clients[&a].name.clone(), app.store.db.clients[&b].name.clone());
        mixed(ui, &trf("Possible duplicates: “{}” and “{}”. Review them; nothing is merged automatically.", &[&na, &nb]), 12.0, Some(c.amber));
    }
    if all == 0 {
        col(ui, 480.0f32.min(ui.available_width()), 0.0, |ui| {
            panel(ui, |ui| {
                ui.label(RichText::new(tr("No clients yet")).font(medium(16.0)));
                muted(ui, "A client groups projects for the same customer. Projects can also have no client.");
            });
        });
        return;
    }
    let q = app.act.client_search.trim().to_lowercase();
    let status = app.act.client_status;
    let in_status = |cl: &&Client| match status { StatusFilter::Active => !cl.archived, StatusFilter::Archived => cl.archived, StatusFilter::All => true };
    let scope = app.store.db.live_clients().into_iter().filter(|cl| in_status(cl)).count();
    let list: Vec<Client> = app.store.db.live_clients().into_iter().filter(|cl| in_status(cl)).filter(|cl| search_matches(&q, &[&cl.name])).cloned().collect();
    let filtered = !q.is_empty() || status != StatusFilter::Active;
    let mut clear = false;
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        if list.len() == scope {
            mixed(ui, &plural(scope as i64, "{} client", "{} clients"), 12.0, Some(c.text2));
        } else {
            mixed(ui, &trf("{} of {} clients", &[&list.len(), &scope]), 12.0, Some(c.text2));
        }
        if filtered && !list.is_empty() && link_btn(ui, "Clear filters").clicked() {
            clear = true;
        }
    });
    ui.add_space(4.0);
    if list.is_empty() {
        empty_state(ui, Icon::Activities, "No clients match these filters", "Change the search or the status, or clear the filters.", |ui| {
            clear = secondary(ui, "Clear filters").clicked();
        });
    }
    if clear {
        app.act.client_search.clear();
        app.act.client_status = StatusFilter::Active;
    }
    if list.is_empty() { return; }
    // the order chosen on the titles: name A–Z, or the most projects first
    let list: Vec<Client> = {
        let (col, asc) = work_sort(app, CLIENT_SORT.0, CLIENT_SORT.1);
        let db = &app.store.db;
        let mut v: Vec<(Client, usize)> = list.into_iter().map(|cl| { let n = db.projects_of_client(&cl.id).len(); (cl, n) }).collect();
        v.sort_by(|a, b| {
            let o = if col == 1 { a.1.cmp(&b.1) } else { a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()) };
            let o = if asc { o } else { o.reverse() };
            o.then_with(|| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()))
        });
        v.into_iter().map(|x| x.0).collect()
    };
    let w = ui.available_width();
    let mut open = None;
    let list = egui::ScrollArea::vertical().id_salt("clients").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let table_top = ui.cursor().top();
        // titles, as the Projects table has: each one orders the list
        let (hr, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), Sense::hover());
        ui.painter().rect_filled(hr, TABLE_TOP, c.nav);
        let (key, def) = CLIENT_SORT;
        work_title(app, ui, key, def, 0, true, egui::pos2(hr.left() + 14.0, hr.center().y), false, "Client");
        work_title(app, ui, key, def, 1, false, egui::pos2(hr.right() - 14.0, hr.center().y), true, "Projects");
        for (ri, cl) in list.iter().enumerate() {
            let n = app.store.db.projects_of_client(&cl.id).len();
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 52.0), Sense::click());
            // under the titles: only the last row has rounded corners
            let round = egui::CornerRadius { nw: 0, ne: 0, sw: if ri + 1 == list.len() { 9 } else { 0 }, se: if ri + 1 == list.len() { 9 } else { 0 } };
            ui.painter().rect_filled(rect, round, hover_mix(ui, resp.id, resp.hovered(), Color32::TRANSPARENT, c.hover));
            if resp.has_focus() {
                ui.painter().rect_stroke(rect.shrink(1.0), RADIUS, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
            }
            app.geometry.rows.push((format!("client row · {}", cl.id), rect));
            let y = rect.center().y;
            ui.painter().text(egui::pos2(rect.left() + 14.0, y), egui::Align2::LEFT_CENTER, truncate(&cl.name, ((w - 320.0) / 7.6).max(10.0) as usize), medium(14.0), if cl.archived { c.text2 } else { c.text });
            let count = plural(n as i64, "{} project", "{} projects");
            let job = mixed_job(&count, 13.0, c.text2, sans(13.0), mono_font(13.0));
            let g = ui.painter().layout_job(job);
            ui.painter().galley(egui::pos2(rect.right() - 14.0 - g.size().x, y - g.size().y / 2.0), g, c.text2);
            if cl.archived {
                ui.painter().text(egui::pos2(rect.right() - 180.0, y), egui::Align2::RIGHT_CENTER, tr("Archived"), sans(12.0), c.text2);
            }
            let name = format!("{} · {}", cl.name, count);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone()));
            if resp.on_hover_text(tr("Open the client's projects")).clicked() {
                open = Some(cl.id.clone());
            }
            divider(ui);
        }
        table_border(ui, table_top);
    });
    scroll_fades(ui, &list, p(ui).canvas);
    if let Some(id) = open {
        app.act.open_client(id);
        app.fade_content();
    }
}

/// The Tags tab: every tag with how many sessions carry it and their time. A tag is renamed in
/// its row (a name another tag has asks to merge the two), deleted after a question that says
/// how many sessions it leaves (Undo brings it back), or created from "+ New tag".
fn tags_tab_ui(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let now = devtrack_core::store::now_secs();
    let available = ui.available_width();
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(CTRL_H);
        let new_w = ui.painter().layout_no_wrap(tr("+ New tag"), medium(14.0), c.text).size().x + PAD_X * 2.0;
        let search_w = if available < 480.0 { available } else { available - new_w - ROW_GAP };
        search_field(ui, &mut app.act.tag_search, "Search tags", "Looks in the tag's name.", search_w);
        if available < 480.0 { ui.end_row(); }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary(ui, "+ New tag").clicked() {
                app.act.tag_edit = Some(TagEdit::New(app.act.tag_search.trim().to_string()));
            }
        });
    });
    ui.add_space(ROW_GAP);
    tag_edit_panel(app, ui);
    let tags: Vec<Tag> = app.store.db.live_tags().into_iter().cloned().collect();
    if tags.is_empty() {
        if app.act.tag_edit.is_none() {
            col(ui, 480.0f32.min(ui.available_width()), 0.0, |ui| {
                panel(ui, |ui| {
                    ui.label(RichText::new(tr("No tags yet")).font(medium(16.0)));
                    muted(ui, "A tag says what kind of work a session was (meeting, review, support), whatever the project. Add them to sessions from the timer, by typing #tag, or here.");
                });
            });
        }
        return;
    }
    // how many sessions carry each tag, and their time
    let mut usage: std::collections::BTreeMap<String, (i64, i64)> = Default::default();
    for e in app.store.db.live_entries() {
        for t in &e.tags {
            let u = usage.entry(t.to_lowercase()).or_default();
            u.0 += 1;
            u.1 += e.duration(now);
        }
    }
    let q = app.act.tag_search.trim().to_lowercase();
    // as every tag search: "code-review" finds "Code review"
    let list: Vec<Tag> = tags.iter().filter(|t| q.is_empty() || tag_matches(&t.name, &q)).cloned().collect();
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        if list.len() == tags.len() {
            mixed(ui, &plural(tags.len() as i64, "{} tag", "{} tags"), 12.0, Some(c.text2));
        } else {
            mixed(ui, &trf("{} of {} tags", &[&list.len(), &tags.len()]), 12.0, Some(c.text2));
        }
        // the counts on the rows are not this week's or month's: say so once
        mixed(ui, &format!("·  {}", tr("sessions and time over everything tracked")), 12.0, Some(c.text2));
    });
    ui.add_space(4.0);
    if list.is_empty() {
        let mut clear = false;
        empty_state(ui, Icon::Activities, "No tags match this search", "Change the search, or create the tag with “+ New tag”.", |ui| {
            clear = secondary(ui, "Clear search").clicked();
        });
        if clear { app.act.tag_search.clear(); }
        return;
    }
    let w = ui.available_width();
    let mut action: Option<TagEdit> = None;
    let mut save: Option<(Id, String)> = None;
    let mut cancel = false;
    let editing = app.act.tag_edit.clone();
    // the order chosen on the titles: name A–Z, or the most used first
    let mut list = list;
    {
        let (col, asc) = work_sort(app, TAG_SORT.0, TAG_SORT.1);
        let n_of = |t: &Tag| usage.get(&t.name.to_lowercase()).copied().unwrap_or((0, 0));
        list.sort_by(|a, b| {
            let o = if col == 1 { n_of(a).cmp(&n_of(b)) } else { a.name.to_lowercase().cmp(&b.name.to_lowercase()) };
            let o = if asc { o } else { o.reverse() };
            o.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }
    let scroll = egui::ScrollArea::vertical().id_salt("tags").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let table_top = ui.cursor().top();
        // titles, as the other tables have; "Use" lines up with the counts, read from a row
        let (hr, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), Sense::hover());
        ui.painter().rect_filled(hr, TABLE_TOP, c.nav);
        let mut usage_right: Option<f32> = None;
        for (ri, t) in list.iter().enumerate() {
            let (n, secs) = usage.get(&t.name.to_lowercase()).copied().unwrap_or((0, 0));
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 52.0), Sense::hover());
            // under the titles: only the last row has rounded corners
            let round = egui::CornerRadius { nw: 0, ne: 0, sw: if ri + 1 == list.len() { 9 } else { 0 }, se: if ri + 1 == list.len() { 9 } else { 0 } };
            let hot = resp.hovered() || ui.rect_contains_pointer(rect);
            ui.painter().rect_filled(rect, round, hover_mix(ui, resp.id, hot, Color32::TRANSPARENT, c.hover));
            app.geometry.rows.push((format!("tag row · {}", t.id), rect));
            let y = rect.center().y;
            let mut row = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(14.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            row.spacing_mut().item_spacing.x = ROW_GAP;
            match &editing {
                Some(TagEdit::Rename(id, name)) if id == &t.id => {
                    // renamed in its row: the field, then Save and Cancel; Enter saves, Esc cancels
                    let mut v = name.clone();
                    let field_w = (rect.width() - 28.0 - 200.0).clamp(160.0, 360.0);
                    let r = input(&mut row, &mut v, Field::new("Tag name", field_w).hint("Tag name"));
                    if !r.has_focus() && !r.lost_focus() && v == t.name {
                        r.request_focus();
                    }
                    if v != *name {
                        action = Some(TagEdit::Rename(id.clone(), v.clone()));
                    }
                    let enter = r.lost_focus() && row.input(|i| i.key_pressed(egui::Key::Enter));
                    if primary(&mut row, "Save").clicked() || enter {
                        save = Some((id.clone(), v.clone()));
                    }
                    if ghost(&mut row, "Cancel").clicked() || row.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel = true;
                    }
                }
                _ => {
                    let name_job = mixed_job(&format!("#{}", t.name), 14.0, c.text, medium(14.0), medium(14.0));
                    row.add(egui::Label::new(name_job).truncate().selectable(false));
                    row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if ghost(ui, "Delete…").on_hover_text(tr("Remove this tag from its sessions")).clicked() {
                            action = Some(TagEdit::Delete(t.id.clone()));
                        }
                        if ghost(ui, "Rename").clicked() {
                            action = Some(TagEdit::Rename(t.id.clone(), t.name.clone()));
                        }
                        ui.add_space(GROUP_GAP);
                        if usage_right.is_none() { usage_right = Some(ui.cursor().max.x); }
                        let used = if n == 0 { tr("No sessions") } else { format!("{} · {}", plural(n, "{} session", "{} sessions"), words(secs)) };
                        ui.label(egui::WidgetText::from(mixed_job(&used, 13.0, c.text2, sans(13.0), mono_font(13.0))));
                    });
                }
            }
            let _ = y;
            divider(ui);
        }
        let (key, def) = TAG_SORT;
        work_title(app, ui, key, def, 0, true, egui::pos2(hr.left() + 14.0, hr.center().y), false, "Tags");
        work_title(app, ui, key, def, 1, false, egui::pos2(usage_right.unwrap_or(hr.right() - 14.0), hr.center().y), true, "Use");
        table_border(ui, table_top);
    });
    scroll_fades(ui, &scroll, p(ui).canvas);
    if let Some((id, name)) = save {
        match app.store.rename_tag(&id, &name) {
            Ok(n) => {
                app.act.tag_edit = None;
                if n > 0 {
                    app.notify(trf("Tag renamed on {} sessions", &[&n]));
                }
            }
            Err(into) => app.act.tag_edit = Some(TagEdit::Merge { from: id, into }),
        }
    } else if cancel {
        app.act.tag_edit = None;
    } else if let Some(a) = action {
        app.act.tag_edit = Some(a);
    }
}

/// The question above the Tags table: a new tag's name, merging two tags, deleting one.
fn tag_edit_panel(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let Some(edit) = app.act.tag_edit.clone() else { return };
    if matches!(edit, TagEdit::Rename(..)) { return; }
    let count = |app: &App, name: &str| app.store.db.live_entries().filter(|e| e.tags.iter().any(|t| t.eq_ignore_ascii_case(name))).count();
    let name_of = |app: &App, id: &Id| app.store.db.tags.get(id).map(|t| t.name.clone()).unwrap_or_default();
    let w = 480.0f32.min(ui.available_width());
    let mut next: Option<Option<TagEdit>> = None;
    col(ui, w, 0.0, |ui| {
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            match &edit {
                TagEdit::Rename(..) => {}
                TagEdit::New(name) => {
                    ui.label(RichText::new(tr("New tag")).font(medium(16.0)));
                    ui.add_space(ROW_GAP);
                    let mut v = name.clone();
                    let r = input(ui, &mut v, Field::new("Tag name", ui.available_width()).hint("e.g. meeting"));
                    if !r.has_focus() && !r.lost_focus() && v.is_empty() {
                        r.request_focus();
                    }
                    let typed = clean_tag(&v);
                    let exists = app.store.db.tag_named(&typed).map(|t| t.name.clone());
                    if let Some(e) = &exists {
                        mixed(ui, &trf("“{}” is already a tag.", &[e]), 12.0, Some(c.amber));
                    }
                    ui.add_space(ROW_GAP);
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        let ok = !typed.is_empty() && exists.is_none();
                        if button_w(ui, Kind::Primary, "Add tag", 0.0, ok).clicked() || (enter && ok) {
                            if let Some(n) = app.store.create_tag(&typed) {
                                app.notify(trf("Tag “{}” created", &[&truncate(&n, 40)]));
                            }
                            next = Some(None);
                        }
                        if ghost(ui, "Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            next = Some(None);
                        }
                    });
                    if next.is_none() && v != *name {
                        next = Some(Some(TagEdit::New(v)));
                    }
                }
                TagEdit::Merge { from, into } => {
                    let (f, i) = (name_of(app, from), name_of(app, into));
                    ui.label(RichText::new(trf("“{}” is already a tag", &[&i])).font(medium(16.0)));
                    mixed(ui, &trf("Merge “{}” into it? Its {} session(s) take “{}”, and “{}” goes.", &[&f, &count(app, &f), &i, &f]), 13.0, Some(c.text2));
                    ui.add_space(ROW_GAP);
                    ui.horizontal(|ui| {
                        if primary(ui, "Merge").clicked() {
                            let n = app.store.merge_tags(from, into);
                            app.notify(trf("Tags merged: {} sessions now have “{}”", &[&n, &i]));
                            next = Some(None);
                        }
                        if ghost(ui, "Keep both").clicked() {
                            next = Some(None);
                        }
                    });
                }
                TagEdit::Delete(id) => {
                    let name = name_of(app, id);
                    let n = count(app, &name);
                    ui.label(RichText::new(trf("Delete “{}”?", &[&name])).font(medium(16.0)).color(c.red));
                    let body = if n == 0 { tr("No session has it.") } else { trf("It leaves {} session(s); the sessions stay.", &[&n]) };
                    mixed(ui, &body, 13.0, Some(c.text2));
                    ui.add_space(ROW_GAP);
                    ui.horizontal(|ui| {
                        if danger(ui, "Delete tag").clicked() {
                            app.store.delete_tag(id);
                            app.notify_undo(trf("Tag “{}” deleted", &[&truncate(&name, 40)]));
                            next = Some(None);
                        }
                        if ghost(ui, "Keep").clicked() {
                            next = Some(None);
                        }
                    });
                }
            }
        });
    });
    if let Some(n) = next {
        app.act.tag_edit = n;
    }
    ui.add_space(GROUP_GAP);
}

fn client_edit_panel(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let Some(edit) = app.act.client_edit.clone() else { return };
    if !matches!(edit, ClientEdit::ConfirmDelete(_)) { return; }
    let w = 480.0f32.min(ui.available_width());
    col(ui, w, 0.0, |ui| {
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            match edit {
                // new and rename are dialogs (client_modal)
                ClientEdit::New(_) | ClientEdit::Rename(..) => {}
                ClientEdit::ConfirmDelete(id) => {
                    let name = app.store.db.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
                    let n = app.store.db.projects_of_client(&id).len();
                    ui.label(RichText::new(trf("Delete “{}”?", &[&name])).font(medium(16.0)).color(c.red));
                    mixed(ui, &trf("Its {} project(s), their tasks and sessions are kept and set to No client. Archiving keeps everything as it is instead.", &[&n]), 13.0, Some(c.text2));
                    ui.add_space(ROW_GAP);
                    ui.horizontal(|ui| {
                        if secondary(ui, "Archive instead").clicked() {
                            if let Some(mut cl) = app.store.db.clients.get(&id).cloned() {
                                cl.archived = true;
                                app.store.put_client(cl);
                            }
                            app.act.client_edit = None;
                        }
                        if danger(ui, "Delete and unlink projects").clicked() {
                            app.store.delete_client_unlinking(&id);
                            app.act.client_edit = None;
                            if app.act.client_open.as_ref() == Some(&id) { app.act.back(); }
                            if app.act.client_filter == ClientFilter::Client(id.clone()) {
                                app.act.client_filter = ClientFilter::All;
                            }
                        }
                        if ghost(ui, "Keep").clicked() {
                            app.act.client_edit = None;
                        }
                    });
                }
            }
        });
    });
    ui.add_space(GROUP_GAP);
}

fn client_detail(app: &mut App, ui: &mut egui::Ui, id: Id) {
    let c = p(ui);
    let now = now_secs();
    let Some(cl) = app.store.db.client(&Some(id.clone())).cloned() else {
        app.act.back();
        return;
    };
    if detail_navigation(app, ui, &cl.name) { return; }
    record_header(ui, &cl.name, &tr("Client"), None, |ui| {
        if cl.archived { pill(ui, "Archived", c.text2); }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let more = icon_button(ui, Glyph::More, "Client actions");
            let pid = ui.make_persistent_id(("client-more", &cl.id));
            if more.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pid));
            }
            egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(220.0);
                if menu_row(ui, "Rename…", false, false).clicked() {
                    app.remember_focus(ui.ctx());
                    app.act.open_client_edit(ClientEdit::Rename(cl.id.clone(), cl.name.clone()));
                }
                if menu_row(ui, if cl.archived { "Unarchive" } else { "Archive" }, false, false).clicked() {
                    let mut x = cl.clone();
                    x.archived = !x.archived;
                    app.store.put_client(x);
                }
                if menu_row(ui, "Delete client…", false, false).clicked() {
                    app.act.client_edit = Some(ClientEdit::ConfirmDelete(cl.id.clone()));
                }
            });
            if primary(ui, "+ Add project").clicked() {
                open_project_form(app, ui.ctx(), None, Some(cl.id.clone()));
            }
        });
    });
    ui.add_space(GROUP_GAP);
    client_edit_panel(app, ui);
    let all_projects: Vec<Project> = app.store.db.projects_of_client(&cl.id).into_iter().cloned().collect();
    if !all_projects.is_empty() {
        search_field(ui, &mut app.act.client_projects_query, "Search projects", "Looks in the project's name.", ui.available_width());
        ui.add_space(ROW_GAP);
    }
    let projects: Vec<Project> = all_projects.iter().filter(|p| search_matches(&app.act.client_projects_query, &[&p.name])).cloned().collect();
    let mut clear = false;
    // the count on the left, the name of the value column on the right
    row(ui, 20.0, |ui| {
        if projects.len() == all_projects.len() {
            mixed(ui, &plural(projects.len() as i64, "{} project", "{} projects"), 12.0, Some(c.text2));
        } else {
            mixed(ui, &trf("{} of {} projects", &[&projects.len(), &all_projects.len()]), 12.0, Some(c.text2));
            clear = link_btn(ui, "Clear filters").clicked();
        }
        if !projects.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0 - ui.spacing().item_spacing.x);
                meta(ui, "Tracked");
            });
        }
    });
    ui.add_space(4.0);
    if clear { app.act.client_projects_query.clear(); }
    if all_projects.is_empty() {
        meta(ui, "No projects for this client yet. Add one, or assign an existing project from its Edit project form.");
    } else if projects.is_empty() {
        meta(ui, "No project of this client matches the search.");
    }
    let w = ui.available_width();
    let mut open = None;
    ui.spacing_mut().item_spacing.y = 0.0;
    let table_top = ui.cursor().top();
    for (ri, pr) in projects.iter().enumerate() {
        let tracked: i64 = app.store.db.live_entries().filter(|e| e.project_id.as_deref() == Some(pr.id.as_str())).map(|e| e.duration(now)).sum();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 52.0), Sense::click());
        let round = egui::CornerRadius { nw: if ri == 0 { 9 } else { 0 }, ne: if ri == 0 { 9 } else { 0 }, sw: if ri + 1 == projects.len() { 9 } else { 0 }, se: if ri + 1 == projects.len() { 9 } else { 0 } };
        ui.painter().rect_filled(rect, round, hover_mix(ui, resp.id, resp.hovered(), Color32::TRANSPARENT, c.hover));
        if resp.has_focus() {
            ui.painter().rect_stroke(rect.shrink(1.0), RADIUS, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
        }
        app.geometry.rows.push((format!("client project · {}", pr.id), rect));
        let y = rect.center().y;
        ui.painter().circle_filled(egui::pos2(rect.left() + 18.0, y), 4.0, hex(&pr.color));
        ui.painter().text(egui::pos2(rect.left() + 32.0, y), egui::Align2::LEFT_CENTER, truncate(&pr.name, ((w - 180.0) / 7.6).max(6.0) as usize), medium(14.0), if pr.archived { c.text2 } else { c.text });
        {
            // durations in words: digits in Mono, units in Sans
            let col = if tracked == 0 { c.text2 } else { c.text };
            let g = ui.painter().layout_job(mixed_job(&total_text(tracked), 14.0, col, sans(14.0), mono_font(14.0)));
            ui.painter().galley(egui::pos2(rect.right() - 14.0 - g.size().x, (y) - g.size().y / 2.0), g, col);
        }
        if pr.archived {
            // said in words, as in the clients list: grey alone is not a signal
            ui.painter().text(egui::pos2(rect.right() - 180.0, y), egui::Align2::RIGHT_CENTER, tr("Archived"), sans(12.0), c.text2);
        }
        let name = pr.name.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone()));
        if resp.on_hover_text(if tracked == 0 { tr("No time tracked yet") } else { tr("Time tracked on this project") }).clicked() {
            open = Some(pr.id.clone());
        }
        divider(ui);
    }
    if !projects.is_empty() {
        table_border(ui, table_top);
    }
    if let Some(pid) = open {
        app.act.open_project(pid);
        app.fade_content();
    }
}

/// Above the task list: a filter by title or number, which tasks to show, and the one way to
/// add a task (a dialog: a local task, or work items from Azure DevOps). The count of open tasks
/// is already in the project's facts, so it is not repeated here.
fn task_toolbar(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let w = ui.available_width();
    let add_label = tr("+ Add task");
    let add_w = (text_w(ui, &add_label, &sans(14.0)) + PAD_X * 2.0).max(BTN_MIN_W);
    let show_w = 170.0;
    let narrow = w < 560.0;
    let search_w = if narrow { w } else { (w - show_w - add_w - ROW_GAP * 2.0).max(160.0) };
    let mut add = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(ROW_GAP, ROW_GAP);
        search_field(ui, &mut app.act.task_query, "Search tasks", "Looks in the task and its #number.", search_w);
        let opts = vec![(0u8, tr("Open tasks")), (1u8, tr("Done tasks")), (2u8, tr("All tasks"))];
        select(ui, "task-show", &mut app.act.task_show, &opts, show_w, false);
        add = secondary(ui, "+ Add task").clicked();
    });
    let _ = c;
    if add {
        app.remember_focus(ui.ctx());
        app.act.open_add(false);
    }
    ui.add_space(GROUP_GAP - ROW_GAP);
}

fn task_table(app: &mut App, ui: &mut egui::Ui, project: Option<Id>) {
    let in_project: Vec<Task> = app
        .store
        .db
        .live_tasks()
        .filter(|t| match &project {
            Some(pid) => t.project_id.as_ref() == Some(pid),
            None => app.store.db.project(&t.project_id).is_none(),
        })
        .filter(|t| match app.act.task_show { 0 => !t.done, 1 => t.done, _ => true })
        .cloned()
        .collect();
    let mut tasks: Vec<Task> = in_project.iter().filter(|t| search_matches(&app.act.task_query, &[&t.title, t.external_id.as_deref().unwrap_or("")])).cloned().collect();
    tasks.sort_by_key(|t| (t.source.clone(), t.external_id.clone().unwrap_or_default().parse::<i64>().unwrap_or(0), t.title.to_lowercase()));
    if tasks.is_empty() {
        let (mut add, mut clear) = (false, false);
        if !app.act.task_query.trim().is_empty() {
            empty_state(ui, Icon::Activities, "No task matches the search", "Change the words, or clear the search.", |ui| {
                clear = secondary(ui, "Clear filters").clicked();
            });
        } else {
            let title = match app.act.task_show { 0 => "No open tasks", 1 => "No done tasks", _ => "No tasks" };
            empty_state(ui, Icon::Activities, title, "Add a task of your own or a work item from Azure DevOps. Sessions can also use the project alone.", |ui| {
                add = secondary(ui, "+ Add task").clicked();
            });
        }
        if clear { app.act.task_query.clear(); }
        if add { app.act.open_add(false); }
        return;
    }
    // the count, against the tasks the open/done choice keeps
    let mut clear = false;
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        if tasks.len() == in_project.len() {
            mixed(ui, &plural(tasks.len() as i64, "{} task", "{} tasks"), 12.0, Some(p(ui).text2));
        } else {
            mixed(ui, &trf("{} of {} tasks", &[&tasks.len(), &in_project.len()]), 12.0, Some(p(ui).text2));
            clear = link_btn(ui, "Clear filters").clicked();
        }
    });
    if clear { app.act.task_query.clear(); }
    ui.add_space(4.0);
    task_rows(app, ui, &tasks, false);
}

/// Most recent sessions listed under an open row; the rest behind "Show all".
const SESSIONS_SHOWN: usize = 10;

/// The task rows of a list, the same on the Tasks tab (`all`: every project, so each row names
/// its project and client) and in a project. A row opens in place: planned hours, source data
/// and the most recent sessions. Only one row is open per list.
/// The Tags tab: by name A–Z unless chosen otherwise (the most used first).
const TAG_SORT: (&str, (u8, bool)) = ("tags", (0, true));

/// The Clients tab: by name A–Z unless chosen otherwise (the most projects first).
const CLIENT_SORT: (&str, (u8, bool)) = ("clients", (0, true));

/// The Projects tab: by name A–Z unless chosen otherwise (client, open tasks, tracked time).
/// Projects without a client come last when ordered by client.
const PROJECT_SORT: (&str, (u8, bool)) = ("projects", (0, true));

/// Work's task lists (the Tasks tab, a project's tasks): by name A–Z, by latest session, or by
/// tracked time; the latest session first unless chosen otherwise on the titles.
const TASK_SORT: (&str, (u8, bool)) = ("tasks", (1, false));

fn task_rows(app: &mut App, ui: &mut egui::Ui, tasks: &[Task], all: bool) {
    let now = now_secs();
    let c = p(ui);
    // the order chosen on the titles, the same on the Tasks tab and in a project
    let tasks: Vec<Task> = {
        let (col, asc) = work_sort(app, TASK_SORT.0, TASK_SORT.1);
        let db = &app.store.db;
        let last = |t: &Task| db.live_entries().filter(|e| e.task_id.as_deref() == Some(t.id.as_str())).map(|e| e.start).max();
        let mut v: Vec<(Task, Option<i64>, i64)> = tasks.iter().map(|t| (t.clone(), last(t), db.task_total(&t.id, now))).collect();
        v.sort_by(|a, b| {
            let o = match col {
                0 => a.0.title.to_lowercase().cmp(&b.0.title.to_lowercase()),
                2 => a.2.cmp(&b.2).then(a.1.cmp(&b.1)),
                _ => a.1.cmp(&b.1).then(a.2.cmp(&b.2)),
            };
            let o = if asc { o } else { o.reverse() };
            // ties (and never tracked) by name
            o.then_with(|| a.0.title.to_lowercase().cmp(&b.0.title.to_lowercase()))
        });
        v.into_iter().map(|x| x.0).collect()
    };
    let tasks = &tasks[..];
    let open_id = if all { app.act.all_open.clone() } else { app.act.task_open.clone() };
    let mut start: Option<Task> = None;
    let mut save: Vec<Task> = vec![];
    let mut delete: Option<Id> = None;
    let mut move_task: Option<Task> = None;
    let mut rename_task: Option<Task> = None;
    let mut copied: Option<String> = None;
    let profiles: Vec<devtrack_core::model::TimesheetProfile> = app.store.db.live_profiles().into_iter().cloned().collect();
    // the conversions icon sits between the total and Start, as on Today
    let conv_w = if profiles.is_empty() { 0.0 } else { CTRL_H + ROW_GAP };
    let task_table_top: Option<f32>;
    {
        // column header, like the other tables
        let w = ui.available_width();
        let (hr, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), Sense::hover());
        ui.painter().rect_filled(hr, TABLE_TOP, c.nav);
        task_table_top = Some(hr.top());
        // the rows follow the header with no gap: one bordered table
        ui.add_space(-ui.spacing().item_spacing.y);
        // after the disclosure column, on the same edge as the task titles
        // each title orders the list: name A–Z, the latest session, the most time
        let (key, def) = TASK_SORT;
        work_title(app, ui, key, def, 0, true, egui::pos2(hr.left() + CTRL_H + ROW_GAP * 3.0, hr.center().y), false, "Task");
        if w >= 560.0 {
            work_title(app, ui, key, def, 2, false, egui::pos2(hr.right() - BTN_MIN_W - CTRL_H - ROW_GAP * 3.0 - GROUP_GAP - conv_w, hr.center().y), true, "Tracked");
        }
        if w >= 720.0 {
            let x = hr.right() - BTN_MIN_W - CTRL_H - ROW_GAP * 4.0 - GROUP_GAP - if tasks.iter().any(|t| t.planned_secs().is_some()) { 170.0 } else { 110.0 } - conv_w;
            work_title(app, ui, key, def, 1, false, egui::pos2(x, hr.center().y), true, "Last session");
        }
    }
    let w = ui.available_width();
    let h24 = app.store.db.settings.clock_24h;
    let date_w = if w >= 720.0 { 110.0 + ROW_GAP } else { 0.0 };
    // "8 h 59 min / 8 h 00 min" needs more room than a plain total
    let track_w = if w >= 560.0 && tasks.iter().any(|t| t.planned_secs().is_some()) { 170.0 } else { 110.0 };
    let mut toggle: Option<Id> = None;
    let mut reload: Option<Task> = None;
    let _ = &task_table_top;
    let mut edit_entry: Option<TimeEntry> = None;
    let mut to_project: Option<Id> = None;
    let mut to_client: Option<Id> = None;
    let mut all_sessions: Option<Option<Id>> = None;
    for t in tasks {
        // on the Tasks tab a row says where the task belongs: project (with its colour) and client
        let project = app.store.db.project(&t.project_id).cloned();
        let client = project.as_ref().and_then(|p| p.client_id.clone()).and_then(|id| app.store.db.client(&Some(id)).cloned());
        // the task's finished sessions, newest first: the date column and the list under the row
        let mut sessions: Vec<TimeEntry> = app.store.db.live_entries().filter(|e| e.task_id.as_deref() == Some(t.id.as_str())).cloned().collect();
        sessions.sort_by_key(|e| std::cmp::Reverse(e.start));
        let tag_counts = tag_counts(&app.store.db, &sessions);
        let open = open_id.as_deref() == Some(t.id.as_str());
        let row_resp = ui.horizontal_wrapped(|ui| {
            ui.set_min_height(56.0);
            ui.spacing_mut().item_spacing.x = ROW_GAP;
            // 16 from the table's edge, as in Report Time: the disclosure's hover never touches
            // the border
            ui.add_space(GROUP_GAP);
            // an imported task always opens: it has source data even with no sessions yet
            let azure = t.source == SOURCE_AZURE && t.external_id.is_some();
            let _ = azure;
            ui.add_enabled_ui(true, |ui| {
                let r = icon_ghost(ui, if open { Glyph::Down } else { Glyph::Next }, if open { "Hide sessions" } else { "Show sessions" });
                app.geometry.rows.push((format!("task toggle · {}", t.id), r.rect));
                if r.clicked() { toggle = Some(t.id.clone()); }
            });
            col(ui, if w < 560.0 { w - CTRL_H - ROW_GAP * 4.0 } else { w - CTRL_H - ROW_GAP * 5.0 - date_w - track_w - BTN_MIN_W - CTRL_H - ROW_GAP * 4.0 - conv_w }, 48.0, |ui| {
                ui.add_space(4.0);
                let title = ui.horizontal(|ui| {
                    if all {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 18.0), Sense::hover());
                        match &project {
                            Some(pr) => { ui.painter().circle_filled(r.center(), 4.0, hex(&pr.color)); }
                            None => { ui.painter().circle_stroke(r.center(), 3.5, Stroke::new(1.0, c.text2)); }
                        }
                    }
                    let mut name = t.title.clone();
                    if let (Some(id), true) = (&t.external_id, all && t.source == SOURCE_AZURE) { name = format!("#{} {}", id, name); }
                    ui.add(egui::Label::new(RichText::new(name).font(medium(14.0)).color(if t.done { c.text2 } else { c.text })).truncate().sense(Sense::click())).on_hover_text(if t.description.is_empty() { t.title.clone() } else { format!("{}\n\n{}", t.title, t.description) })
                }).inner;
                if title.clicked() { toggle = Some(t.id.clone()); }
                let mut sub = vec![];
                if all {
                    sub.push(match (&project, &client) {
                        (Some(pr), Some(cl)) => format!("{} · {}", pr.name, cl.name),
                        (Some(pr), None) => pr.name.clone(),
                        _ => tr("No project"),
                    });
                }
                // the task's tags once, most used first, before the source details (which give
                // way first when the line is cut)
                if !tag_counts.is_empty() {
                    sub.push(tag_summary(&tag_counts));
                }
                match &t.external_id {
                    Some(id) if t.source == SOURCE_AZURE => {
                        sub.push(format!("DevOps #{}", id));
                        if let Some(ep) = &t.external_project {
                            sub.push(trf_m!("DevOps project {}", ep));
                        }
                        if !t.status.is_empty() {
                            sub.push(t.status.clone());
                        }
                    }
                    _ if all => { if t.done { sub.push(tr("done")); } }
                    _ => sub.push(tr(if t.done { "Local task · done" } else { "Local task" })),
                }
                if t.unavailable {
                    sub.push(tr("not accessible at last refresh"));
                }
                let line = sub.join(" · ");
                let job = mixed_job(&line, 12.0, if t.unavailable { c.amber } else { c.text2 }, sans(12.0), mono_font(12.0));
                // the tags behind "+N", on the pointer; it replaces egui's own tip for a cut line,
                // or the two would show one over the other
                let hover = all_tags_hover(&line, &tag_counts);
                cut_line(ui, job, hover);
            });
            if w < 560.0 { ui.end_row(); }
            if date_w > 0.0 {
                col_right(ui, 110.0, 48.0, |ui| {
                    ui.add_space(15.0);
                    match sessions.first() {
                        Some(e) => ui.label(mono(super::i18n::short_date(devtrack_core::report::local_date(e.start)), 13.0).color(c.text2)).on_hover_text(tr("Date of the most recent session")),
                        None => meta(ui, "—"),
                    };
                });
            }
            let total = app.store.db.task_total(&t.id, now);
            col_right(ui, track_w, 48.0, |ui| {
                ui.add_space(14.0);
                // "3 h 47 min / 8 h" when the task has an estimate; amber once it is passed
                match t.planned_secs() {
                    // past the planned hours: amber and a "!", so it does not rest on colour alone
                    Some(est) if total > est => mixed(ui, &format!("! {} / {}", words(total), words(est)), 14.0, Some(c.amber)).on_hover_text(trf_m!("{} over the planned hours", words(total - est))),
                    Some(est) => mixed(ui, &format!("{} / {}", if total == 0 { "0".to_string() } else { words(total) }, words(est)), 14.0, Some(c.text)).on_hover_text(tr("Tracked on this task, of its planned hours")),
                    None => mixed(ui, &total_text(total), 14.0, Some(if total > 0 { c.text } else { c.text2 })).on_hover_text(tr("Tracked on this task")),
                };
            });
            // the total as each platform writes it: behind one icon, as on Today; the place is
            // kept when there is nothing to convert, so Start stays in one column
            if !profiles.is_empty() && total > 0 {
                let (_, v) = super::today::conversions_icon(ui, ("task", &t.id), &profiles, total);
                if v.is_some() { copied = v; }
            } else if !profiles.is_empty() {
                ui.add_space(CTRL_H + ui.spacing().item_spacing.x);
            }
            if button_w(ui, Kind::Secondary, "Start", BTN_MIN_W, true).clicked() {
                start = Some(t.clone());
            }
            let more = icon_button(ui, Glyph::More, "Task actions");
            let pid = ui.make_persistent_id(("task-more", &t.id));
            if more.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pid));
            }
            egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(240.0);
                if let (true, Some(pr)) = (all, &project) {
                    if menu_row(ui, "Open project", false, false).clicked() {
                        to_project = Some(pr.id.clone());
                    }
                }
                if let Some(u) = &t.url {
                    if menu_row(ui, "Open original in Azure DevOps", false, false).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(u));
                    }
                }
                // a task added in DevTrack is named here; an imported one takes its title from the
                // source, which would overwrite a new name at the next refresh
                if t.source == SOURCE_MANUAL && menu_row(ui, "Rename…", false, false).clicked() {
                    rename_task = Some(t.clone());
                }
                if menu_row(ui, "Move task…", false, false).clicked() {
                    move_task = Some(t.clone());
                }
                if t.source == SOURCE_MANUAL && menu_row(ui, if t.done { "Mark as open" } else { "Mark as done" }, false, false).clicked() {
                    let mut x = t.clone();
                    x.done = !x.done;
                    x.status = if x.done { "Done".into() } else { "Open".into() };
                    save.push(x);
                }
                if menu_row(ui, "Remove from DevTrack", false, false).on_hover_text(tr("Nothing changes in the source; recorded time stays")).clicked() {
                    delete = Some(t.id.clone());
                }
            });
        }).response;
        app.geometry.rows.push((format!("task row · {}", t.id), row_resp.rect));
        if all && open && app.act.reveal {
            app.act.reveal = false;
            ui.scroll_to_rect(row_resp.rect, Some(egui::Align::Center));
        }
        if open {
            // the task's sessions: date · time · what · duration · Edit (the same dialog as Today)
            egui::Frame::new().fill(c.nav).corner_radius(RADIUS).inner_margin(egui::Margin { left: (CTRL_H + ROW_GAP) as i8, right: 12, top: 8, bottom: 8 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 2.0;
                let inner = ui.available_width();
                // on the Tasks tab: where the task belongs, each a way into it
                if all {
                    row(ui, CTRL_H, |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        label(ui, "Project");
                        match &project {
                            Some(pr) => {
                                let r = link_btn(ui, &truncate(&pr.name, 40)).on_hover_text(tr("Open the project"));
                                app.geometry.rows.push((format!("task project · {}", t.id), r.rect));
                                if r.clicked() { to_project = Some(pr.id.clone()); }
                            }
                            None => { meta(ui, "No project"); }
                        }
                        if let Some(cl) = &client {
                            ui.add_space(GROUP_GAP);
                            label(ui, "Client");
                            if link_btn(ui, &truncate(&cl.name, 40)).on_hover_text(tr("Open the client")).clicked() {
                                to_client = Some(cl.id.clone());
                            }
                        }
                    });
                }
                // the task's planned hours and where the tracked time stands: typed for a local
                // task, read from Azure DevOps (Original Estimate) for an imported one
                let from_source = t.source != SOURCE_MANUAL;
                row(ui, CTRL_H, |ui| {
                    ui.spacing_mut().item_spacing.x = GROUP_GAP;
                    label(ui, "Planned hours");
                    if from_source {
                        match t.planned_secs() {
                            Some(est) => {
                                // the value and where it comes from, read as one piece
                                ui.scope(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    ui.label(mixed_job(&words(est), 14.0, c.text, medium(14.0), mono_medium(14.0))).on_hover_text(tr("Original Estimate in Azure DevOps: change it there"));
                                    meta(ui, "from Azure DevOps");
                                });
                            }
                            None => { meta(ui, "No estimate in Azure DevOps"); }
                        }
                    } else if let Some(v) = estimate_field(ui, &t.id, t.estimate_secs, 110.0) {
                        let mut x = t.clone();
                        x.estimate_secs = v;
                        save.push(x);
                    }
                    match t.planned_secs() {
                        Some(est) => {
                            let done = app.store.db.task_total(&t.id, now);
                            let col = app.store.db.project(&t.project_id).map(|p| hex(&p.color)).unwrap_or(c.text2);
                            ui.vertical(|ui| { ui.add_space(CTRL_H / 2.0 - 2.0); estimate_bar(ui, 120.0, done, est, col); });
                            estimate_status(ui, done, est);
                        }
                        None if from_source => {}
                        None => { meta(ui, "How long you expect to spend on it. You get a notice once it is passed."); }
                    }
                });
                // every tag on the task's sessions, with how many sessions carry it
                if !tag_counts.is_empty() {
                    tag_count_row(ui, &tag_counts, sessions.len());
                }
                ui.add_space(4.0);
                if sessions.is_empty() && t.source != SOURCE_AZURE {
                    meta(ui, "No sessions on this task yet.");
                }
                if t.source == SOURCE_AZURE && t.external_id.is_some() {
                    if source_detail(app, ui, t, app.store.db.task_total(&t.id, now)) {
                        reload = Some(t.clone());
                    }
                    if !sessions.is_empty() {
                        ui.add_space(8.0);
                        label(ui, "Sessions in DevTrack");
                    }
                }
                let every = app.act.all_sessions.as_deref() == Some(t.id.as_str());
                let shown = if every { sessions.len() } else { sessions.len().min(SESSIONS_SHOWN) };
                for e in sessions.iter().take(shown) {
                    let r = row(ui, 28.0, |ui| {
                        ui.spacing_mut().item_spacing.x = GROUP_GAP;
                        ui.label(mono(super::i18n::short_date(devtrack_core::report::local_date(e.start)), 12.0).color(c.text2));
                        if inner >= 420.0 {
                            let end = e.stop.map(|s| clock_text(s, h24)).unwrap_or(tr("now"));
                            ui.label(range_job(&clock_text(e.start, h24), &end, 12.0, c.text2));
                        }
                        ui.scope(|ui| {
                            ui.set_max_width((ui.available_width() - 170.0).max(60.0));
                            // under its task, a session is told apart by its note
                            let what = if e.description.trim().is_empty() { tr("No note") } else { e.description.clone() };
                            // the task's tags are said once above: here only those that set
                            // this session apart
                            let job = note_with_own_tags(ui, &app.store.db, &what, e, &tag_counts, sessions.len());
                            ui.add(egui::Label::new(job).truncate()).on_hover_text(&what);
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let edit = ui.add_enabled_ui(true, |ui| link_btn(ui, "Edit")).inner;
                            // duration, then the conversions icon, as on Today
                            if !profiles.is_empty() {
                                let (_, v) = super::today::conversions_icon(ui, ("task-session", &e.id), &profiles, e.duration(now));
                                if v.is_some() { copied = v; }
                            }
                            ui.label(mono(super::shown_duration(&app.store.db, e.duration(now)), 13.0));
                            edit
                        }).inner
                    });
                    app.geometry.rows.push((format!("task session · {}", e.id), r.rect));
                    if r.on_hover_text(tr("Change the description, project or times of this session")).clicked() {
                        edit_entry = Some(e.clone());
                    }
                }
                // a long history stays short: the most recent sessions, the rest on request
                if sessions.len() > SESSIONS_SHOWN {
                    ui.add_space(4.0);
                    let label = if every { tr("Show fewer") } else { trf("Show all ({})", &[&sessions.len()]) };
                    let r = link_btn(ui, &label);
                    app.geometry.rows.push((format!("task sessions · all · {}", t.id), r.rect));
                    if r.clicked() { all_sessions = Some(if every { None } else { Some(t.id.clone()) }); }
                }
            });
            ui.add_space(4.0);
        }
        divider(ui);
    }
    if let Some(top) = task_table_top {
        table_border(ui, top);
    }
    if let Some(v) = all_sessions {
        app.act.all_sessions = v;
    }
    if let Some(id) = toggle {
        let slot = if all { &mut app.act.all_open } else { &mut app.act.task_open };
        *slot = if slot.as_ref() == Some(&id) { None } else { Some(id) };
        let now_open = slot.clone();
        // opening an imported task reads its source detail once
        if let Some(t) = now_open.as_ref().and_then(|id| app.store.db.tasks.get(id)).cloned() {
            if t.source == SOURCE_AZURE && app.act.remote.as_ref().map(|(id, _)| id != &t.id).unwrap_or(true) {
                reload = Some(t);
            }
        }
    }
    if let Some(t) = reload {
        load_source_detail(app, &t);
    }
    if let Some(e) = edit_entry {
        let ctx = ui.ctx().clone();
        super::today::open_edit(app, &e, Some(&ctx), false);
    }
    for t in save {
        app.store.put_task(t);
    }
    if let Some(id) = delete {
        app.store.delete_task(&id);
        app.notify(tr("Task removed from DevTrack"));
    }
    if let Some(t) = rename_task {
        app.remember_focus(ui.ctx());
        app.act.task_rename = Some((t.id.clone(), t.title.clone()));
        app.act.task_rename_attempted = false;
        app.act.task_rename_focused = false;
        app.act.task_rename_confirm = false;
    }
    if let Some(t) = move_task {
        let n = app.store.db.live_entries().filter(|e| e.task_id.as_deref() == Some(t.id.as_str())).count();
        app.remember_focus(ui.ctx());
        app.act.pending_move = Some(MoveDraft { task: t.id.clone(), to: None, also_sessions: false, sessions: n });
    }
    if let Some(v) = copied {
        ui.ctx().copy_text(v.clone());
        app.notify(trf_m!("Copied: {}", v));
    }
    if let Some(t) = start {
        let billable = app.store.db.project(&t.project_id).map(|p| p.billable_default).unwrap_or(false);
        app.store.start(&t.title, t.project_id.clone(), Some(t.id.clone()), billable);
        app.notify(trf_m!("Tracking “{}”", truncate(&t.display(), 40)));
        app.go(Page::Today);
    }
    // into the task's project or client: a step below the current list, so Back returns here
    if let Some(id) = to_project {
        app.act.open_project(id);
        app.fade_content();
    } else if let Some(id) = to_client {
        app.act.open_client(id);
        app.fade_content();
    }
}

/// An estimate as the field shows it: whole hours as "8", otherwise "1:30".
fn estimate_text(secs: Option<i64>) -> String {
    match secs {
        Some(s) if s % 3600 == 0 => (s / 3600).to_string(),
        Some(s) => format!("{}:{:02}", s / 3600, (s % 3600) / 60),
        None => String::new(),
    }
}

/// A small field for the hours the person plans to spend (the estimate), in hours or h:mm. Returns the new value when the person leaves the
/// field with something different in it (`Some(None)` clears the estimate).
fn estimate_field(ui: &mut egui::Ui, salt: impl std::hash::Hash, current: Option<i64>, width: f32) -> Option<Option<i64>> {
    let id = ui.make_persistent_id(("estimate", salt));
    let mut text = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| estimate_text(current));
    let r = input(ui, &mut text, Field::new("Planned hours", width).mono().hint("e.g. 2:45")).on_hover_text(tr("Whole hours or hours:minutes. Leave it empty for no estimate."));
    let mut out = None;
    if r.lost_focus() {
        let parsed = devtrack_core::timefmt::parse_estimate(&text);
        if parsed != current {
            out = Some(parsed);
        }
        // show the value as it was understood
        ui.data_mut(|d| d.remove::<String>(id));
    } else {
        ui.data_mut(|d| d.insert_temp(id, text));
    }
    out
}

/// Where the tracked time stands against the planned hours, as a bar in the project's colour;
/// amber once they are passed. The words beside it say the same, so colour is never alone.
fn estimate_bar(ui: &mut egui::Ui, w: f32, tracked: i64, estimate: i64, color: Color32) {
    let c = p(ui);
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, 4.0), Sense::hover());
    ui.painter().rect_filled(r, 2.0, c.line);
    let share = if estimate > 0 { (tracked as f32 / estimate as f32).clamp(0.0, 1.0) } else { 0.0 };
    if share > 0.0 {
        let fill = egui::Rect::from_min_size(r.min, egui::vec2((r.width() * share).max(4.0), r.height()));
        ui.painter().rect_filled(fill, 2.0, if tracked > estimate { c.amber } else { color });
    }
}

/// Where the tracked time stands against an estimate, in words; amber once it is passed.
fn estimate_status(ui: &mut egui::Ui, tracked: i64, estimate: i64) {
    let c = p(ui);
    let (pct, over) = devtrack_core::timefmt::estimate_state(tracked, estimate);
    if over {
        mixed(ui, &trf_m!("! {} over the planned hours", words(tracked - estimate)), 12.0, Some(c.amber));
    } else {
        mixed(ui, &trf_m!("{}% used · {} left", pct, words(estimate - tracked)), 12.0, Some(if pct >= 80 { c.amber } else { c.text2 }));
    }
}

/// Reads remaining, completed and the last three updates of an imported task from its source.
fn load_source_detail(app: &mut App, t: &Task) {
    let Some(ext) = t.external_id.clone() else { return };
    if !app.store.is_persistent() {
        return; // previews have no connection: their scenes fill the detail themselves
    }
    let conns = connections(app);
    let cfg = conns.iter().find(|c| t.integration_id.as_deref() == Some(c.id.as_str())).or_else(|| conns.iter().find(|c| t.id.starts_with(&format!("{}:{}:", SOURCE_AZURE, org_of(c).to_lowercase())))).cloned();
    let Some(cfg) = cfg else {
        app.act.remote = Some((t.id.clone(), RemoteState::Failed(tr("The Azure DevOps connection this task came from is no longer set up."))));
        return;
    };
    let task = t.id.clone();
    if app.jobs.spawn(&cfg, "Reading the work item…", move |c| JobOut::Detail { task, res: c.and_then(|c| c.detail(&ext, 3)) }) {
        app.act.remote = Some((t.id.clone(), RemoteState::Loading));
    }
}

/// The source's own numbers for a task, read only: remaining, completed and estimated hours
/// beside what DevTrack tracked, then the last updates. Returns true when a reload is asked.
fn source_detail(app: &App, ui: &mut egui::Ui, t: &Task, tracked: i64) -> bool {
    let c = p(ui);
    let mut reload = false;
    let state = app.act.remote.as_ref().filter(|(id, _)| id == &t.id).map(|(_, s)| s.clone());
    row(ui, 24.0, |ui| {
        ui.spacing_mut().item_spacing.x = GROUP_GAP;
        ui.label(RichText::new(trf_m!("Azure DevOps #{} · read only", t.external_id.clone().unwrap_or_default())).font(medium(13.0)));
        if matches!(state, Some(RemoteState::Loading)) {
            ui.spinner();
        } else if link_btn(ui, if state.is_some() { "Read again" } else { "Read from Azure DevOps" }).clicked() {
            reload = true;
        }
    });
    match state {
        None | Some(RemoteState::Loading) => {}
        Some(RemoteState::Failed(m)) => {
            ui.label(RichText::new(m).size(12.0).color(c.red));
        }
        Some(RemoteState::Loaded(d)) => {
            let hours = |v: Option<f64>| v.map(|x| format!("{} h", (x * 100.0).round() / 100.0)).unwrap_or("—".into());
            row_wrapped(ui, 40.0, |ui| {
                ui.spacing_mut().item_spacing.x = 32.0;
                for (name, value) in [("Remaining", hours(d.remaining)), ("Completed", hours(d.completed)), ("Tracked in DevTrack", if tracked == 0 { "—".into() } else { words(tracked) })] {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        label(ui, name);
                        ui.label(mixed_job(&value, 14.0, if value == "—" { c.text2 } else { c.text }, medium(14.0), mono_medium(14.0)));
                    });
                }
            });
            ui.add_space(6.0);
            label(ui, "Last updates");
            if d.updates.is_empty() {
                meta(ui, "No changes to state, hours, assignee or comments yet.");
            }
            for u in &d.updates {
                let when = u.when.map(|s| super::i18n::short_date(devtrack_core::report::local_date(s))).unwrap_or("—".into());
                let what = u.changes.iter().map(|(name, old, new)| {
                    let new = truncate(new, 60);
                    if old.is_empty() { format!("{}: {}", tr(name), new) } else { format!("{}: {} → {}", tr(name), truncate(old, 30), new) }
                }).collect::<Vec<_>>().join(" · ");
                row(ui, 22.0, |ui| {
                    ui.spacing_mut().item_spacing.x = GROUP_GAP;
                    ui.label(mono(when, 12.0).color(c.text2));
                    if !u.by.is_empty() && ui.available_width() > 420.0 {
                        ui.label(RichText::new(truncate(&u.by, 22)).size(12.0).color(c.text2));
                    }
                    ui.add(egui::Label::new(mixed_job(&what, 13.0, c.text, sans(13.0), mono_font(13.0))).truncate()).on_hover_text(&what);
                });
            }
        }
    }
    reload
}

/// Move task: choose the project, decide about existing sessions with one checkbox, one action.
pub fn move_modal(app: &mut App, ctx: &egui::Context) {
    let Some(mut d) = app.act.pending_move.clone() else { return };
    let dark = app.dark();
    let c = pal(dark);
    let Some(task) = app.store.db.tasks.get(&d.task).cloned() else {
        app.act.pending_move = None;
        return;
    };
    let rp = recent_projects(&app.store.db);
    let from_name = app.store.db.project(&task.project_id).map(|p| p.name.clone()).unwrap_or("No project".into());
    let mut close = false;
    let mut attempted = ctx.data(|m| m.get_temp::<bool>(egui::Id::new("move-attempted"))).unwrap_or(false);
    let w = 480.0f32.min(ctx.screen_rect().width() - 96.0);
    let mut x_close = false;
    let m = egui::Modal::new(egui::Id::new("move-task")).frame(modal_frame(&c)).backdrop_color(Color32::from_black_alpha(if dark { 160 } else { 90 })).show(ctx, |ui| {
        ui.set_width(w);
        x_close = modal_close(ui);
        ui.label(RichText::new(tr("Move task")).font(semibold(20.0)));
        ui.add_space(4.0);
        mixed(ui, &task.display(), 14.0, None);
        ui.horizontal(|ui| {
            meta(ui, "Now in");
            ui.label(RichText::new(truncate(&from_name, 40)).size(12.0).color(c.text2));
        });
        ui.add_space(GROUP_GAP);
        let same = d.to == task.project_id;
        let err = if attempted && same { Some("Choose a different project") } else { None };
        field(ui, "Move to project", None, err, |ui| {
            project_picker(ui, "move-to", &app.store.db, &rp, &mut d.to, w);
        });
        ui.add_space(GROUP_GAP - ROW_GAP);
        if d.sessions > 0 {
            let text = plural(d.sessions as i64, "Also move {} existing session", "Also move {} existing sessions");
            checkbox(ui, &mut d.also_sessions, mixed_job(&text, 14.0, c.text, sans(14.0), mono_font(14.0)));
            meta(ui, if d.also_sessions { "Their project changes too; values already marked as reported will ask to be confirmed again." } else { "Past sessions keep their current project." });
        } else {
            meta(ui, "No sessions are recorded on this task yet.");
        }
        ui.add_space(GROUP_GAP);
        divider(ui);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary(ui, "Move task").clicked() {
                    attempted = true;
                    if d.to != task.project_id {
                        let moved = app.store.move_task(&d.task, d.to.clone(), d.also_sessions);
                        let to_name = app.store.db.project(&d.to).map(|p| p.name.clone()).unwrap_or("no project".into());
                        app.notify(if d.also_sessions { trf_m!("Task and {} session(s) moved to {}", moved, truncate(&to_name, 30)) } else { trf_m!("Task moved to {}", truncate(&to_name, 30)) });
                        close = true;
                    }
                }
                if secondary(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
    });
    if m.should_close() || x_close {
        close = true;
    }
    if close {
        app.act.pending_move = None;
        ctx.data_mut(|m| m.remove::<bool>(egui::Id::new("move-attempted")));
        app.restore_focus(ctx);
    } else {
        ctx.data_mut(|m| m.insert_temp(egui::Id::new("move-attempted"), attempted));
        app.act.pending_move = Some(d);
    }
}

fn connections(app: &App) -> Vec<IntegrationConfig> {
    let mut v: Vec<IntegrationConfig> = app.store.db.integrations.values().filter(|c| !c.deleted && c.kind == SOURCE_AZURE).cloned().collect();
    v.sort_by_key(|c| c.name.to_lowercase());
    v
}

/// One query: ids, the first page of details and, the first time, the source's projects.
fn run_query(app: &mut App, cfg: &IntegrationConfig) {
    let searching = app.act.finder.searching;
    let q = app.act.finder.query.clone();
    let rp = app.act.finder.remote_project.clone();
    let want_projects = app.act.finder.remote_projects.is_empty();
    let id = cfg.id.clone();
    let label = if searching { "Searching…" } else { "Loading work items assigned to you…" };
    if app.jobs.spawn(cfg, label, move |c| {
        let res = c.and_then(|c| {
            let projects = if want_projects { c.projects().ok() } else { None };
            let ids = if searching { c.search_ids(&q, rp.as_deref())? } else { c.assigned_ids(rp.as_deref())? };
            let first: Vec<String> = ids.iter().take(PAGE).cloned().collect();
            let items = c.get(&first)?;
            Ok((ids, items, projects))
        });
        JobOut::Found { cfg: id, res }
    }) {
        app.act.finder.state = FinderState::Loading;
    }
}

fn refresh_imported(app: &mut App, cfg: &IntegrationConfig, imported: Vec<String>) {
    if imported.is_empty() {
        app.notify(tr("No imported tasks to refresh for this connection"));
        return;
    }
    let id = cfg.id.clone();
    app.jobs.spawn(cfg, "Refreshing imported tasks…", move |c| JobOut::Refresh { cfg: id, asked: imported.clone(), res: c.and_then(|c| c.get(&imported)) });
}

fn finder(app: &mut App, ui: &mut egui::Ui, target: Option<Id>) {
    let c = p(ui);
    let conns = connections(app);
    if conns.is_empty() {
        col(ui, 480.0f32.min(ui.available_width()), 0.0, |ui| {
            panel(ui, |ui| {
                ui.label(RichText::new(tr("Azure DevOps isn't connected")).font(medium(16.0)));
                muted(ui, "Connect with a read-only token to see the work items assigned to you here. You'll come back to this project afterwards.");
                ui.add_space(8.0);
                if primary(ui, "Connect Azure DevOps").clicked() {
                    connect_azure(app);
                }
            });
        });
        return;
    }
    // remember the last connection used; fall back to the first
    if app.act.finder.cfg.as_ref().map(|id| !conns.iter().any(|c| &c.id == id)).unwrap_or(true) {
        app.act.finder.cfg = Some(conns[0].id.clone());
    }
    let cfg = conns.iter().find(|c| Some(&c.id) == app.act.finder.cfg.as_ref()).cloned().unwrap();
    let has_token = !app.store.is_persistent() || secrets::has(&cfg.id);
    let org = org_of(&cfg);
    let imported: Vec<String> = app
        .store
        .db
        .live_tasks()
        .filter(|t| t.source == SOURCE_AZURE && (t.integration_id.as_deref() == Some(cfg.id.as_str()) || t.id.starts_with(&format!("{}:{}:", SOURCE_AZURE, org.to_lowercase()))))
        .filter_map(|t| t.external_id.clone())
        .collect();
    let mut run = false;
    // ---- one toolbar: a single row when it fits, otherwise mode · search · source on rows ----
    let avail = ui.available_width();
    let narrow = avail < 680.0;
    let action = if app.act.finder.searching { "Search" } else { "Refresh" };
    let action_w = ui.painter().layout_no_wrap(tr(action), sans(14.0), c.text).size().x + PAD_X * 2.0;
    let mode = |app: &mut App, ui: &mut egui::Ui, run: &mut bool| {
        if conns.len() > 1 {
            let opts: Vec<(Id, String)> = conns.iter().map(|c| (c.id.clone(), c.name.clone())).collect();
            let mut cur = cfg.id.clone();
            if select(ui, "finder-conn", &mut cur, &opts, 170.0, false) && cur != cfg.id {
                app.act.finder = super::activities::Finder { cfg: Some(cur.clone()), ..Default::default() };
                app.store.local.last_connection = Some(cur);
                app.store.save_local();
            }
        }
        if let Some(i) = segmented(ui, &["Assigned to me", "Search"], if app.act.finder.searching { 1 } else { 0 }) {
            app.act.finder.searching = i == 1;
            if i == 0 {
                *run = true;
            }
        }
    };
    let query = |app: &mut App, ui: &mut egui::Ui, w: f32, run: &mut bool| {
        let r = input(ui, &mut app.act.finder.query, Field::new("Search work items", w).hint("#4812 or words in the title"));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.act.finder.query.trim().is_empty() {
            *run = true;
        }
    };
    let source = |app: &mut App, ui: &mut egui::Ui, w: f32, run: &mut bool| {
        let mut opts: Vec<(Option<String>, String)> = vec![(None, tr("All DevOps projects"))];
        for rp in &app.act.finder.remote_projects {
            opts.push((Some(rp.name.clone()), rp.name.clone()));
        }
        if select(ui, "finder-proj", &mut app.act.finder.remote_project, &opts, w, false) {
            *run = true;
        }
        if button_w(ui, Kind::Secondary, action, 0.0, has_token && !app.jobs.busy()).on_hover_text(tr("Read the list again from Azure DevOps")).clicked() {
            *run = true;
        }
    };
    let more = |app: &mut App, ui: &mut egui::Ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let more = icon_button(ui, Glyph::More, "Source actions");
            let pid = ui.make_persistent_id("finder-more");
            if more.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pid));
            }
            egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(260.0);
                if menu_row(ui, &trf_m!("Refresh imported tasks · {}", imported.len()), false, false).on_hover_text(tr("Reads titles and statuses again; local projects and history stay")).clicked() {
                    refresh_imported(app, &cfg, imported.clone());
                }
                if menu_row(ui, "Connection settings", false, false).clicked() {
                    connect_azure(app);
                }
            });
            if let (Some(l), false, false) = (&app.jobs.label, app.act.finder.state == FinderState::Loading, narrow) {
                ui.spinner();
                meta(ui, l.clone());
            }
        });
    };
    // "Assigned to me" filters the loaded list as you type; "Search" asks Azure DevOps
    let local = |app: &mut App, ui: &mut egui::Ui, w: f32| {
        input(ui, &mut app.act.finder.filter, Field::new("Filter work items", w).hint("Filter by title or #number")).on_hover_text(tr("Narrows the list below. Use Search to look in all of Azure DevOps."));
    };
    if narrow {
        row(ui, CTRL_H, |ui| {
            mode(app, ui, &mut run);
            more(app, ui);
        });
        if app.act.finder.searching {
            row(ui, CTRL_H, |ui| query(app, ui, avail, &mut run));
        } else {
            row(ui, CTRL_H, |ui| local(app, ui, avail));
        }
        row(ui, CTRL_H, |ui| source(app, ui, (avail - action_w - ROW_GAP).max(120.0), &mut run));
    } else {
        row(ui, CTRL_H, |ui| {
            mode(app, ui, &mut run);
            if app.act.finder.searching {
                // leave room for the DevOps project, the action and "···"
                let q_w = (ui.available_width() - 190.0 - action_w - CTRL_H - ROW_GAP * 4.0).max(200.0);
                query(app, ui, q_w, &mut run);
            } else {
                let f_w = (ui.available_width() - 190.0 - action_w - CTRL_H - ROW_GAP * 4.0).clamp(160.0, 360.0);
                local(app, ui, f_w);
            }
            source(app, ui, 190.0, &mut run);
            more(app, ui);
        });
    }
    if run && has_token && !(app.act.finder.searching && app.act.finder.query.trim().is_empty()) {
        run_query(app, &cfg);
    }
    // load once on arrival (projects come with it)
    if has_token && !app.act.finder.searching && app.act.finder.state == FinderState::NotLoaded && !app.act.finder.auto_tried.contains(&cfg.id) && !app.jobs.busy() && app.store.is_persistent() {
        app.act.finder.auto_tried.insert(cfg.id.clone());
        run_query(app, &cfg);
    }
    ui.horizontal(|ui| {
        if let Some(t) = cfg.last_refresh {
            meta(ui, trf_m!("{} · imported tasks refreshed {}", cfg.name, ago(t * 1000)));
        } else {
            meta(ui, trf_m!("{} · read only", cfg.name));
        }
        if let Some((ok, m)) = &app.act.finder.refresh_msg {
            mixed(ui, m, 12.0, Some(if *ok { c.green } else { c.red }));
        }
    });
    ui.add_space(ROW_GAP);
    match app.act.finder.state.clone() {
        FinderState::NotLoaded => {
            if has_token {
                meta(ui, "Not loaded yet. Use Refresh to read the list.");
            } else {
                row(ui, CTRL_H, |ui| {
                    ui.label(RichText::new(tr("No access token on this device for this connection.")).color(c.amber));
                    if secondary(ui, "Add token").clicked() {
                        connect_azure(app);
                    }
                });
            }
        }
        FinderState::Loading => {
            row(ui, CTRL_H, |ui| {
                ui.spinner();
                meta(ui, "Loading…");
            });
        }
        FinderState::NoResults => {
            meta(ui, if app.act.finder.searching { "No results for this search." } else { "No open work items are assigned to you." });
        }
        FinderState::Offline(m) => {
            row(ui, CTRL_H, |ui| {
                ui.label(RichText::new(trf_m!("Offline: {}", m)).color(c.amber));
                if secondary(ui, "Try again").clicked() {
                    run_query(app, &cfg);
                }
            });
        }
        FinderState::Error(m) => {
            row(ui, CTRL_H, |ui| {
                ui.label(RichText::new(m).color(c.red));
                if secondary(ui, "Connection settings").clicked() {
                    connect_azure(app);
                }
            });
        }
        FinderState::Loaded => items_table(app, ui, &cfg, target),
    }
}

fn items_table(app: &mut App, ui: &mut egui::Ui, cfg: &IntegrationConfig, target: Option<Id>) {
    let c = p(ui);
    let org = org_of(cfg);
    let mut start_id: Option<String> = None;
    let mut import_id: Option<String> = None;
    let total = app.act.finder.ids.len();
    let shown = app.act.finder.items.len();
    let items = app.act.finder.items.clone();
    let target_name = app.store.db.project(&target).map(|p| p.name.clone());
    // where each shown item already lives in DevTrack, if it does
    let home: Vec<Option<String>> = items.iter().map(|it| {
        app.store.db.tasks.get(&Task::external_key(SOURCE_AZURE, &org, &it.id)).filter(|t| !t.deleted).map(|t| app.store.db.project(&t.project_id).map(|p| p.name.clone()).unwrap_or(tr("no project")))
    }).collect();
    // the filter narrows what is listed; counts and "select all" follow what is shown
    let q = app.act.finder.filter.trim().trim_start_matches('#').to_lowercase();
    let (items, home): (Vec<RemoteItem>, Vec<Option<String>>) = items.into_iter().zip(home).filter(|(it, _)| search_matches(&q, &[&it.title, &it.id])).unzip();
    let filtered = !q.is_empty();
    let fresh: Vec<String> = items.iter().zip(&home).filter(|(_, h)| h.is_none()).map(|(it, _)| it.id.clone()).collect();
    // count · load more · select what is new
    row(ui, 20.0, |ui| {
        ui.spacing_mut().item_spacing.x = GROUP_GAP;
        if filtered {
            mixed(ui, &trf_m!("{} of {} match “{}”", items.len(), shown, app.act.finder.filter.trim()), 12.0, Some(c.text2));
            if link_btn(ui, "Clear filter").clicked() {
                app.act.finder.filter.clear();
            }
        } else if shown == total {
            mixed(ui, &plural(total as i64, "{} work item", "{} work items"), 12.0, Some(c.text2));
        } else {
            mixed(ui, &trf_m!("Showing {} of {} work items", shown, total), 12.0, Some(c.text2));
        }
        if shown < total && link_btn(ui, &trf_m!("Load {} more", (total - shown).min(PAGE))).clicked() {
            let next: Vec<String> = app.act.finder.ids.iter().skip(shown).take(PAGE).cloned().collect();
            let id = cfg.id.clone();
            app.jobs.spawn(cfg, "Loading more…", move |c| JobOut::More { cfg: id, res: c.and_then(|c| c.get(&next)) });
        }
        let all_fresh_selected = !fresh.is_empty() && fresh.iter().all(|id| app.act.finder.selected.contains(id));
        if !fresh.is_empty() && !all_fresh_selected && link_btn(ui, &plural(fresh.len() as i64, "Select the {} not imported yet", "Select the {} not imported yet")).clicked() {
            app.act.finder.selected.extend(fresh.iter().cloned());
        }
    });
    ui.add_space(4.0);
    let w = ui.available_width();
    // the imported project gets its own column while there is room; otherwise it joins line 2
    let status_w = if w >= 640.0 { 200.0 } else { 0.0 };
    let actions_w = BTN_MIN_W + CTRL_H + ROW_GAP;
    for (it, home) in items.iter().zip(&home) {
        ui.horizontal(|ui| {
            ui.set_min_height(56.0);
            let mut on = app.act.finder.selected.contains(&it.id);
            if checkbox(ui, &mut on, "").on_hover_text(tr("Select to import")).changed() {
                if on {
                    app.act.finder.selected.insert(it.id.clone());
                } else {
                    app.act.finder.selected.remove(&it.id);
                }
            }
            col(ui, (ui.available_width() - status_w - actions_w - ROW_GAP * 2.0).max(160.0), 48.0, |ui| {
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new(&it.title).font(medium(14.0))).truncate()).on_hover_text(&it.title);
                let mut line = format!("#{} · {} · {} · {}", it.id, it.kind, it.project, it.status);
                if let (Some(h), true) = (home, status_w == 0.0) {
                    line.push_str(&format!(" · {}", trf_m!("✔ in {}", h)));
                }
                let job = mixed_job(&line, 12.0, c.text2, sans(12.0), mono_font(12.0));
                ui.add(egui::Label::new(job).truncate());
            });
            if status_w > 0.0 {
                // only what is already here is marked: an empty cell means "not imported"
                col(ui, status_w, 48.0, |ui| {
                    ui.add_space(14.0);
                    if let Some(h) = home {
                        ui.add(egui::Label::new(RichText::new(trf_m!("✔ in {}", h)).size(12.0).color(c.green)).truncate()).on_hover_text(tr("Already in DevTrack. Importing again updates it; it keeps its project."));
                    }
                });
            }
            // the actions end on the right edge, under the toolbar's own
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let more = icon_button(ui, Glyph::More, "Work item actions");
                let pid = ui.make_persistent_id(("item-more", &it.id));
                if more.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(pid));
                }
                egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(260.0);
                    let label_txt = match (home.is_some(), &target_name) {
                        (true, _) => tr("Update from Azure DevOps"),
                        (false, Some(t)) => trf_m!("Import into “{}”", truncate(t, 24)),
                        (false, None) => tr("Import (no project yet)"),
                    };
                    if menu_row(ui, &label_txt, false, false).clicked() {
                        import_id = Some(it.id.clone());
                    }
                    if menu_row(ui, "Open original in Azure DevOps", false, false).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(&it.url));
                    }
                });
                if button_w(ui, Kind::Secondary, "Start", BTN_MIN_W, true).on_hover_text(tr("Import if needed and start tracking")).clicked() {
                    start_id = Some(it.id.clone());
                }
            });
        });
        divider(ui);
    }
    ui.add_space(ROW_GAP);
    if let Some(id) = import_id {
        let done = import(app, cfg, &[id], target.clone());
        app.notify(trf_m!("{} task(s) imported; tasks already in DevTrack keep their project", done.len()));
    }
    if let Some(id) = start_id {
        if let Some(tid) = import(app, cfg, &[id], target).pop() {
            if let Some(t) = app.store.db.tasks.get(&tid).cloned() {
                let billable = app.store.db.project(&t.project_id).map(|p| p.billable_default).unwrap_or(false);
                app.store.start(&t.title, t.project_id.clone(), Some(t.id.clone()), billable);
                app.notify(trf_m!("Tracking “{}”", truncate(&t.display(), 40)));
                app.go(Page::Today);
            }
        }
    }
}

/// The import action for the selected work items: pinned to the bottom of the page, so it is in
/// reach wherever the list is scrolled. Drawn before the page's scroll area.
/// "Add task": one dialog for a task of the person's own or work items from Azure DevOps (more
/// sources would join here). Opened from the task list; it adds to the project that is open.
pub fn add_task_modal(app: &mut App, ctx: &egui::Context) {
    if !app.act.add_open {
        return;
    }
    let picked = if app.act.add_pick { app.act.add_project.clone() } else { app.act.selected.clone() };
    let target: Option<Id> = picked.filter(|id| id != NO_PROJECT && app.store.db.projects.get(id).is_some_and(|p| !p.deleted));
    let target_name = app.store.db.project(&target).map(|p| p.name.clone());
    let rp = recent_projects(&app.store.db);
    let devops = app.act.add_devops;
    let n = app.act.finder.selected.len();
    let ready = n > 0 && app.act.finder.state == FinderState::Loaded;
    let primary_label = if devops {
        match &target_name {
            Some(t) => trf_m!("Import {} into “{}”", n, truncate(t, 28)),
            None => trf_m!("Import {} (no project yet)", n),
        }
    } else {
        tr("Add task")
    };
    // opened inside a project the place is fixed and said under the title; from the Tasks tab
    // it is the dialog's first field
    let context = if app.act.add_pick { None } else {
        Some(match &target_name {
            Some(n) => trf_m!("In “{}”", truncate(n, 48)),
            None => tr("Without a project: move it to one later from the row's actions."),
        })
    };
    let dialog = FormDialog {
        id: "add-task",
        title: "Add task",
        context,
        // the work items are a table: that view alone is wider
        width: if devops { 820.0 } else { FORM_W },
        primary: &primary_label,
        primary_enabled: if devops { ready } else { true },
        dirty: !app.act.new_task.trim().is_empty() || n > 0,
        note: if devops && n > 0 { Some(plural(n as i64, "{} work item selected", "{} work items selected")) } else { None },
        note_link: if devops && n > 0 { Some("Deselect") } else { None },
    };
    let dark = app.dark();
    let mut confirm = app.act.add_confirm;
    let mut pick = Pick::None;
    let screen_h = ctx.screen_rect().height();
    let r = form_dialog(ctx, dark, dialog, &mut confirm, |ui, w| {
        let mut enter = false;
        if app.act.add_pick {
            field(ui, "Project", Some("Where the task belongs. Without a project, move it to one later from the row's actions."), None, |ui| {
                pick = project_picker(ui, "add-task-project", &app.store.db, &rp, &mut app.act.add_project, w);
            });
            ui.add_space(GROUP_GAP - ROW_GAP);
        }
        if let Some(i) = segmented(ui, &["Local task", "From Azure DevOps"], if app.act.add_devops { 1 } else { 0 }) {
            app.act.add_devops = i == 1;
        }
        ui.add_space(GROUP_GAP);
        if app.act.add_devops {
            egui::ScrollArea::vertical().id_salt("add-task-devops").max_height((screen_h * 0.55).max(220.0)).auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(w);
                finder(app, ui, target.clone());
            });
        } else {
            let err = if app.act.add_attempted && app.act.new_task.trim().is_empty() { Some(tr("Give the task a title")) } else { None };
            let r = field(ui, "Title", Some("A task of your own, for work that has no work item."), err.as_deref(), |ui| {
                input(ui, &mut app.act.new_task, Field::new("Title", w).hint("e.g. Weekly client meeting").invalid(err.is_some()))
            });
            if !ui.memory(|mm| mm.focused().is_some()) {
                r.request_focus();
            }
            enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        }
        enter
    });
    app.act.add_confirm = confirm;
    if let Pick::Create(name) = pick {
        app.act.add_project = Some(app.create_project(&name));
    }
    if r.note_link {
        app.act.finder.selected.clear();
    }
    let mut done = r.close;
    if r.submit {
        if app.act.add_devops {
            if let Some(cfg) = connections(app).into_iter().find(|c| Some(&c.id) == app.act.finder.cfg.as_ref()) {
                let ids: Vec<String> = app.act.finder.selected.iter().cloned().collect();
                let imported = import(app, &cfg, &ids, target.clone());
                app.notify(trf_m!("{} task(s) imported; tasks already in DevTrack keep their project", imported.len()));
                done = true;
            }
        } else if app.act.new_task.trim().is_empty() {
            app.act.add_attempted = true;
        } else {
            let t = Task { id: new_id(), project_id: target.clone(), title: app.act.new_task.trim().to_string(), status: "Open".into(), source: SOURCE_MANUAL.into(), ..Default::default() };
            app.notify(trf_m!("Task added: {}", truncate(&t.title, 40)));
            app.store.put_task(t);
            done = true;
        }
    }
    if done {
        app.act.close_add();
        app.restore_focus(ctx);
    }
}

fn import(app: &mut App, cfg: &IntegrationConfig, ids: &[String], target: Option<Id>) -> Vec<Id> {
    let org = org_of(cfg);
    let mut out = vec![];
    let items: Vec<RemoteItem> = app.act.finder.items.iter().filter(|i| ids.contains(&i.id)).cloned().collect();
    for it in items {
        let key = Task::external_key(&cfg.kind, &org, &it.id);
        let existing = app.store.db.tasks.get(&key).filter(|t| !t.deleted).cloned();
        let t = to_task(cfg, &org, &it, existing.as_ref(), target.clone());
        out.push(t.id.clone());
        app.store.put_task(t);
    }
    out
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    use devtrack_core::{sample::{build_state, DemoState}, store::Store};

    /// Work's lists order by their titles: Projects by name first, then a click on Tracked
    /// puts the most tracked first, and a second click turns it round; the choice is kept.
    #[test]
    fn work_lists_order_by_their_column_titles() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Varied, false)), None);
        app.act.tab = ActTab::Projects;
        let now = now_secs();
        let tracked = |app: &App, id: &str| app.store.db.live_entries().filter(|e| e.project_id.as_deref() == Some(id)).map(|e| e.duration(now)).sum::<i64>();
        let order = |app: &App| -> Vec<Id> {
            let mut rows: Vec<(f32, Id)> = app.geometry.rows.iter().filter_map(|(n, r)| n.strip_prefix("project row · ").map(|id| (r.top(), id.to_string()))).collect();
            rows.sort_by(|a, b| a.0.total_cmp(&b.0));
            rows.into_iter().map(|x| x.1).collect()
        };
        draw(&ctx, &mut app, vec![]);
        draw(&ctx, &mut app, vec![]);
        assert!(order(&app).len() > 3, "the rows are drawn");
        let names: Vec<String> = order(&app).iter().map(|id| app.store.db.projects[id].name.to_lowercase()).collect();
        assert!(names.windows(2).all(|w| w[0] <= w[1]), "by name, A to Z: {names:?}");
        let title = egui::Id::new(("work-sort", "projects", 3u8));
        for _ in 0..2 {
            let r = ctx.read_response(title).expect("the Tracked title").rect;
            let button = |pressed| egui::Event::PointerButton { pos: r.center(), button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            draw(&ctx, &mut app, vec![egui::Event::PointerMoved(r.center())]);
            draw(&ctx, &mut app, vec![button(true)]);
            draw(&ctx, &mut app, vec![button(false)]);
            draw(&ctx, &mut app, vec![]);
            let t: Vec<i64> = order(&app).iter().map(|id| tracked(&app, id)).collect();
            if app.store.local.sorts["projects"] == (3, false) {
                assert!(t.windows(2).all(|w| w[0] >= w[1]), "the most tracked first: {t:?}");
            } else {
                assert_eq!(app.store.local.sorts["projects"], (3, true), "a second click turns it round");
                assert!(t.windows(2).all(|w| w[0] <= w[1]), "the least tracked first: {t:?}");
            }
        }
    }

    fn draw(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        app.geometry.rows.clear();
        let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 1000.0))), events, ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let bounds = ui.max_rect();
                page(app, ui);
                assert!(ui.min_rect().right() <= bounds.right() + 1.0 && ui.min_rect().bottom() <= bounds.bottom() + 1.0, "detail layout escaped its viewport: {:?}", ui.min_rect());
            });
        });
    }

    fn click(ctx: &egui::Context, app: &mut App, control: &str) {
        let point = app.geometry.rows.iter().find(|(n, _)| n == control).unwrap_or_else(|| panic!("Missing {control}")).1.center();
        draw(ctx, app, vec![egui::Event::PointerMoved(point), egui::Event::PointerButton { pos: point, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        draw(ctx, app, vec![egui::Event::PointerButton { pos: point, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
        draw(ctx, app, vec![]);
    }

    #[test]
    fn client_project_back_buttons_preserve_the_origin_and_search() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Incomplete, false)), None);
        app.act.open_root(ActTab::Clients);
        app.act.client_search = "Northwind".into();
        draw(&ctx, &mut app, vec![]);
        draw(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, "client row · c-northwind");
        assert_eq!(app.act.client_open.as_deref(), Some("c-northwind"));
        click(&ctx, &mut app, "client project · p-portal");
        assert_eq!(app.act.selected.as_deref(), Some("p-portal"));
        assert_eq!(app.act.trail(), &[Crumb::Client("c-northwind".into()), Crumb::Project("p-portal".into())]);
        assert_eq!(app.act.tab, ActTab::Clients);
        click(&ctx, &mut app, "activities navigation · back");
        assert!(app.act.selected.is_none());
        assert_eq!(app.act.client_open.as_deref(), Some("c-northwind"));
        click(&ctx, &mut app, "activities navigation · back");
        assert!(app.act.client_open.is_none());
        assert_eq!(app.act.client_search, "Northwind");
    }

    #[test]
    fn project_browser_back_and_root_keep_filters_but_clear_detail_routes() {
        let mut state = ActState::default();
        state.search = "portal".into();
        state.client_filter = ClientFilter::Client("c-northwind".into());
        state.status_filter = StatusFilter::All;
        state.open_root(ActTab::Projects);
        state.open_project("p-portal".into());
        assert!(state.back());
        assert!(state.selected.is_none());
        assert_eq!(state.search, "portal");
        assert_eq!(state.client_filter, ClientFilter::Client("c-northwind".into()));
        assert_eq!(state.status_filter, StatusFilter::All);
        state.open_client("c-northwind".into());
        state.open_project("p-portal".into());
        state.open_root(ActTab::Projects);
        assert!(state.selected.is_none() && state.client_open.is_none() && state.trail().is_empty());
        assert!(!state.back());
    }

    #[test]
    fn work_opens_on_tasks_and_the_trail_returns_the_way_it_came() {
        let mut state = ActState::default();
        assert_eq!(state.tab, ActTab::Tasks, "Work opens on Tasks");
        // Tasks › project › its client › another project of that client
        state.open_project("p-portal".into());
        state.open_client("c-northwind".into());
        state.open_project("p-api".into());
        assert_eq!(state.selected.as_deref(), Some("p-api"));
        assert!(state.back());
        assert_eq!(state.client_open.as_deref(), Some("c-northwind"));
        assert!(state.selected.is_none());
        // a step already on the way is returned to, not added again
        state.open_project("p-portal".into());
        assert_eq!(state.trail(), &[Crumb::Project("p-portal".into())]);
        state.open_client("c-northwind".into());
        state.back_to(0);
        assert_eq!(state.selected.as_deref(), Some("p-portal"));
        assert_eq!(state.tab, ActTab::Tasks, "the tab names the way back and never changes below it");
        assert!(state.back());
        assert!(!state.back());
    }

    #[test]
    fn one_search_rule_everywhere() {
        assert!(search_matches("", &["anything"]));
        assert!(search_matches("design lib", &["Design – Library"]));
        assert!(search_matches("library immersiva", &["Design – Library", "Design · IMMERSIVA"]));
        assert!(search_matches("#28885", &["Modulo C01", "28885"]));
        assert!(!search_matches("design turismo", &["Design – Library", "Design · IMMERSIVA"]));
    }

    #[test]
    fn a_task_row_on_the_tasks_tab_opens_in_place_and_its_project_is_one_step_below() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Incomplete, false)), None);
        app.act.open_root(ActTab::Tasks);
        app.act.all_show = 2;
        let task = app.store.db.live_tasks().find(|t| app.store.db.project(&t.project_id).is_some()).cloned().expect("a demo task in a project");
        let project = task.project_id.clone().unwrap();
        app.act.all_query = task.title.clone();
        draw(&ctx, &mut app, vec![]);
        draw(&ctx, &mut app, vec![]);
        // the row opens where it is: no project page, no other task of the project
        click(&ctx, &mut app, &format!("task toggle · {}", task.id));
        assert_eq!(app.act.all_open.as_deref(), Some(task.id.as_str()));
        assert!(app.act.selected.is_none());
        // its project: one step below the Tasks tab, and Back returns to the open row
        click(&ctx, &mut app, &format!("task project · {}", task.id));
        assert_eq!(app.act.selected.as_deref(), Some(project.as_str()));
        assert_eq!(app.act.tab, ActTab::Tasks);
        click(&ctx, &mut app, "activities navigation · back");
        assert!(app.act.selected.is_none());
        assert_eq!(app.act.all_open.as_deref(), Some(task.id.as_str()));
        assert_eq!(app.act.all_query, task.title);
    }

    #[test]
    fn create_dialogs_ask_before_losing_what_was_typed() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Incomplete, false)), None);
        let esc = || vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }];
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), events, ..Default::default() }, |ctx| {
                client_modal(app, ctx);
                add_task_modal(app, ctx);
            });
        };
        // untouched: Esc closes at once
        app.act.open_client_edit(ClientEdit::New(String::new()));
        frame(&mut app, vec![]);
        frame(&mut app, esc());
        assert!(app.act.client_edit.is_none());
        // typed: Esc asks first, a second Esc closes
        app.act.open_client_edit(ClientEdit::New("Acme".into()));
        frame(&mut app, vec![]);
        frame(&mut app, esc());
        assert!(app.act.client_edit.is_some() && app.act.client_confirm);
        frame(&mut app, esc());
        assert!(app.act.client_edit.is_none());
        // the task dialog behaves the same
        app.act.open_add_choose(false, None);
        app.act.new_task = "Write the report".into();
        frame(&mut app, vec![]);
        frame(&mut app, esc());
        assert!(app.act.add_open && app.act.add_confirm);
    }

    /// Rename task: Enter saves the new name on the task; its sessions keep pointing to it.
    #[test]
    fn a_task_added_in_devtrack_can_be_renamed() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Varied, false)), None);
        let task = app.store.db.live_tasks().find(|t| t.source == SOURCE_MANUAL && app.store.db.live_entries().any(|e| e.task_id.as_deref() == Some(t.id.as_str()))).cloned().unwrap();
        let sessions = app.store.db.live_entries().filter(|e| e.task_id.as_deref() == Some(task.id.as_str())).count();
        let mut frame = |app: &mut App, events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), events, ..Default::default() }, |ctx| task_rename_modal(app, ctx));
        };
        app.act.task_rename = Some((task.id.clone(), "Corso Rust avanzato".into()));
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        frame(&mut app, vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
        frame(&mut app, vec![]);
        assert!(app.act.task_rename.is_none(), "Enter saves and closes");
        assert_eq!(app.store.db.tasks[&task.id].title, "Corso Rust avanzato");
        assert_eq!(app.store.db.live_entries().filter(|e| e.task_id.as_deref() == Some(task.id.as_str())).count(), sessions, "its sessions stay on it");
    }

    #[test]
    fn opening_a_task_from_elsewhere_shows_its_row_on_the_tasks_tab() {
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Incomplete, false)), None);
        let task = app.store.db.live_tasks().next().cloned().unwrap();
        app.act.show_project("p-portal".into());
        app.act.all_query = "nothing matches this".into();
        app.act.all_estimate = EstimateFilter::Over;
        open_task(&mut app, &task);
        assert_eq!(app.act.tab, ActTab::Tasks);
        assert!(app.act.selected.is_none() && app.act.trail().is_empty());
        assert_eq!(app.act.all_open.as_deref(), Some(task.id.as_str()));
        assert!(app.act.all_query.is_empty() && app.act.all_estimate == EstimateFilter::All);
        assert!(app.act.reveal);
    }
}

/// Project and task totals are aggregates: shown like other totals ("7 h 25 min"), with a dash
/// for nothing tracked yet rather than a zero clock.
fn total_text(secs: i64) -> String {
    if secs == 0 { "—".into() } else { words(secs) }
}

/// A tag name as the list spells it (the name as written when it is not in the list).
pub(super) fn tag_spelling(db: &devtrack_core::model::Db, name: &str) -> String {
    db.tag_named(name).map(|t| t.name.clone()).unwrap_or_else(|| name.trim().to_string())
}

/// The tags on these sessions with how many sessions carry each, most used first, then A–Z.
/// Spellings that differ only by capitals count as one tag.
pub(super) fn tag_counts(db: &devtrack_core::model::Db, sessions: &[TimeEntry]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = vec![];
    for e in sessions {
        let mut seen: Vec<String> = vec![];
        for n in &e.tags {
            let name = tag_spelling(db, n);
            if name.is_empty() || seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                continue;
            }
            seen.push(name.clone());
            match counts.iter_mut().find(|(m, _)| m.eq_ignore_ascii_case(&name)) {
                Some((_, k)) => *k += 1,
                None => counts.push((name, 1)),
            }
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    counts
}

/// The first tags of a summary for a grey line: "#sviluppo #iOS #PDF +2".
pub(super) fn tag_summary(counts: &[(String, usize)]) -> String {
    let mut names: Vec<String> = counts.iter().take(3).map(|(n, _)| format!("#{}", n)).collect();
    if counts.len() > 3 {
        names.push(format!("+{}", counts.len() - 3));
    }
    names.join(" ")
}

/// A line that shows `tag_summary` ("#a #b #c +2"), with every tag in its place: what the
/// pointer shows over the line when some are behind "+N". None when nothing is hidden.
pub(super) fn all_tags_hover(line: &str, counts: &[(String, usize)]) -> Option<String> {
    (counts.len() > 3).then(|| {
        let all = counts.iter().map(|(n, _)| format!("#{}", n)).collect::<Vec<_>>().join(" ");
        line.replacen(&tag_summary(counts), &all, 1)
    })
}

/// A "Tags" row: every tag with how many of the sessions carry it.
/// With a single session the counts would all be 1, so they are left out.
pub(super) fn tag_count_row(ui: &mut egui::Ui, counts: &[(String, usize)], total: usize) {
    let c = p(ui);
    row(ui, CTRL_H, |ui| {
        ui.spacing_mut().item_spacing.x = GROUP_GAP;
        label(ui, "Tags");
        let text = counts.iter().map(|(n, k)| if total > 1 { format!("#{} {}", n, k) } else { format!("#{}", n) }).collect::<Vec<_>>().join("  ·  ");
        ui.add(egui::Label::new(RichText::new(text).size(13.0).color(c.text)).wrap()).on_hover_text(tr("Sessions with each tag"));
    });
}

/// A session's note followed by the tags that set it apart from the others in the list (those
/// not on every session); the shared ones are said once above the list.
pub(super) fn note_with_own_tags(ui: &egui::Ui, db: &devtrack_core::model::Db, what: &str, e: &TimeEntry, counts: &[(String, usize)], total: usize) -> egui::WidgetText {
    let c = p(ui);
    let own: Vec<String> = e.tags.iter()
        .map(|n| tag_spelling(db, n))
        .filter(|n| counts.iter().any(|(m, k)| m.eq_ignore_ascii_case(n) && *k < total))
        .map(|n| format!("#{}", n))
        .collect();
    if own.is_empty() {
        return RichText::new(what).size(13.0).into();
    }
    let mut j = egui::text::LayoutJob::default();
    j.append(what, 0.0, egui::TextFormat { font_id: sans(13.0), color: c.text, ..Default::default() });
    j.append(&format!("  {}", own.join(" ")), 0.0, egui::TextFormat { font_id: sans(12.0), color: c.text2, ..Default::default() });
    egui::WidgetText::LayoutJob(j)
}
