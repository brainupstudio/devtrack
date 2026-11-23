//! The Azure DevOps work items assigned to you, kept ready for the task field: read in the
//! background (at start, when Today opens and the list is older than 15 minutes), saved next to
//! the data so suggestions work at once and offline, and never asked for while typing. A
//! #number that is not in the list is looked up only when asked. Choosing one imports nothing:
//! the work item becomes a task when its session starts.

use super::i18n::{tr, trf_m};
use super::jobs::{JobOut, Jobs};
use super::App;
use devtrack_core::integrations::{org_of, to_task, RemoteItem};
use devtrack_core::model::*;
use devtrack_core::store::{now_secs, write_atomic, Store};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How old the list may get before Today reads it again.
pub const STALE_SECS: i64 = 15 * 60;
/// How many assigned work items are kept (one request for their details).
const KEEP: usize = 200;
const FILE: &str = "azure-assigned.json";

/// What the task field needs of a work item: no description, no history.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub status: String,
    pub kind: String,
    /// The Azure DevOps project it belongs to.
    pub project: String,
    pub url: String,
    pub estimate: Option<f64>,
}

impl Item {
    pub fn from_remote(r: &RemoteItem) -> Item {
        Item { id: r.id.clone(), title: r.title.clone(), status: r.status.clone(), kind: r.kind.clone(), project: r.project.clone(), url: r.url.clone(), estimate: r.estimate }
    }

    fn to_remote(&self) -> RemoteItem {
        RemoteItem { id: self.id.clone(), title: self.title.clone(), description: String::new(), status: self.status.clone(), kind: self.kind.clone(), project: self.project.clone(), url: self.url.clone(), assigned_to: String::new(), estimate: self.estimate }
    }

    /// As tasks show it: "#4812 Login page times out".
    pub fn display(&self) -> String {
        format!("#{} {}", self.id, self.title)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    Searching,
    Found,
    NotFound,
    Failed(String),
}

#[derive(Default, Serialize, Deserialize)]
pub struct Assigned {
    /// The connection the list was read from.
    pub cfg: Option<Id>,
    pub items: Vec<Item>,
    /// When the list was last read (seconds).
    pub at: Option<i64>,
    /// Work items found by their number, not assigned to you.
    #[serde(default)]
    pub extra: Vec<Item>,
    /// The last read failed: why (the saved list stays usable).
    #[serde(skip)]
    pub error: Option<String>,
    #[serde(skip)]
    pub loading: bool,
    /// A #number asked for and where its lookup stands; `lookup_sent` once the request left.
    #[serde(skip)]
    pub lookup: Option<(String, Lookup)>,
    #[serde(skip)]
    lookup_sent: bool,
    #[serde(skip)]
    jobs: Jobs,
}

impl Assigned {
    pub fn load(store: &Store) -> Assigned {
        store.dir().and_then(|d| std::fs::read(d.join(FILE)).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save(&self, store: &Store) {
        if let (Some(dir), Ok(b)) = (store.dir(), serde_json::to_vec(self)) {
            let _ = write_atomic(&dir.join(FILE), &b);
        }
    }

    /// Every work item known for the task field: assigned ones, then those looked up.
    pub fn all(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().chain(self.extra.iter().filter(|x| !self.items.iter().any(|i| i.id == x.id)))
    }

    pub fn get(&self, id: &str) -> Option<&Item> {
        self.all().find(|i| i.id == id)
    }

    /// "List from 2 h ago · Azure DevOps can't be reached": said under the suggestions when the
    /// list is old or the last read failed. None while it is fresh.
    pub fn status(&self, now: i64) -> Option<String> {
        let age = self.at.map(|a| now - a);
        let when = match age {
            Some(a) if a >= 3600 => Some(trf_m!("List from {} h ago", a / 3600)),
            Some(a) if a >= STALE_SECS => Some(trf_m!("List from {} min ago", a / 60)),
            _ => None,
        };
        match (&self.error, when) {
            (Some(_), Some(w)) => Some(format!("{} · {}", w, tr("Azure DevOps can't be reached"))),
            (Some(_), None) => Some(tr("Azure DevOps can't be reached")),
            (None, w) => w,
        }
    }
}

/// The connection suggestions come from: the one last used in Work, otherwise the first Azure
/// DevOps connection that is on.
pub fn connection(app: &App) -> Option<IntegrationConfig> {
    let live: Vec<&IntegrationConfig> = app.store.db.integrations.values().filter(|c| !c.deleted && c.enabled && c.kind == SOURCE_AZURE).collect();
    live.iter().find(|c| Some(&c.id) == app.store.local.last_connection.as_ref()).or(live.first()).map(|c| (*c).clone())
}

/// Reads the list again when it is older than 15 minutes (or `force`), in the background.
pub fn refresh(app: &mut App, force: bool) {
    let Some(cfg) = connection(app) else { return };
    let a = &app.assigned;
    if a.loading || a.jobs.busy() {
        return;
    }
    let same = a.cfg.as_ref() == Some(&cfg.id);
    let fresh = same && a.at.is_some_and(|t| now_secs() - t < STALE_SECS);
    // a failed read is tried again with the next stale check, not on every frame
    let failed_recently = a.error.is_some() && app.assigned_tried.is_some_and(|t| now_secs() - t < STALE_SECS);
    if !force && (fresh || failed_recently) {
        return;
    }
    let id = cfg.id.clone();
    if app.assigned.jobs.spawn(&cfg, "", move |c| {
        let res = c.and_then(|c| {
            let ids = c.assigned_ids(None)?;
            let first: Vec<String> = ids.into_iter().take(KEEP).collect();
            if first.is_empty() { Ok(vec![]) } else { c.get(&first) }
        });
        JobOut::Assigned { cfg: id, res }
    }) {
        app.assigned.loading = true;
        app.assigned_tried = Some(now_secs());
    }
}

/// Looks one work item up by its number (asked from the suggestions).
pub fn look_up(app: &mut App, number: &str) {
    app.assigned.lookup = Some((number.to_string(), Lookup::Searching));
    app.assigned.lookup_sent = false;
    send_lookup(app);
}

/// The lookup waits while the list is being read: one request at a time.
fn send_lookup(app: &mut App) {
    let Some((number, Lookup::Searching)) = app.assigned.lookup.clone() else { return };
    if app.assigned.lookup_sent {
        return;
    }
    let Some(cfg) = connection(app) else {
        app.assigned.lookup = Some((number, Lookup::Failed(tr("Azure DevOps isn't connected"))));
        return;
    };
    let n = number.clone();
    if app.assigned.jobs.spawn(&cfg, "", move |c| JobOut::Lookup { number: n.clone(), res: c.and_then(|c| c.get(&[n])) }) {
        app.assigned.lookup_sent = true;
    }
}

/// Each frame: collect what came back, send a waiting lookup.
pub fn poll(app: &mut App) {
    if let Some(out) = app.assigned.jobs.poll() {
        match out {
            JobOut::Assigned { cfg, res } => {
                app.assigned.loading = false;
                match res {
                    Ok(items) => {
                        let a = &mut app.assigned;
                        if a.cfg.as_ref() != Some(&cfg) { a.extra.clear(); }
                        a.cfg = Some(cfg);
                        a.items = items.iter().map(Item::from_remote).collect();
                        a.at = Some(now_secs());
                        a.error = None;
                        app.assigned.save(&app.store);
                    }
                    Err(e) => app.assigned.error = Some(e.message),
                }
            }
            JobOut::Lookup { number, res, .. } => {
                app.assigned.lookup_sent = false;
                let state = match res {
                    Ok(items) => match items.first() {
                        Some(it) => {
                            let item = Item::from_remote(it);
                            app.assigned.extra.retain(|x| x.id != item.id);
                            app.assigned.extra.push(item);
                            app.assigned.save(&app.store);
                            Lookup::Found
                        }
                        None => Lookup::NotFound,
                    },
                    Err(e) => Lookup::Failed(e.message),
                };
                if app.assigned.lookup.as_ref().is_some_and(|(n, _)| *n == number) {
                    app.assigned.lookup = Some((number, state));
                }
            }
            _ => {}
        }
    }
    send_lookup(app);
}

/// Where a work item of this Azure DevOps project most likely goes: the DevTrack project that
/// holds most of the work items already imported from it.
pub fn likely_project(db: &Db, devops_project: &str) -> Option<Id> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for t in db.live_tasks().filter(|t| t.source == SOURCE_AZURE && t.external_project.as_deref() == Some(devops_project)) {
        if let Some(p) = db.project(&t.project_id).filter(|p| !p.archived) {
            *count.entry(p.id.as_str()).or_insert(0) += 1;
        }
    }
    count.into_iter().max_by_key(|(id, n)| (*n, std::cmp::Reverse(*id))).map(|(id, _)| id.to_string())
}

/// Makes the work item a task (or finds the one it already is) in `project`, for a session.
pub fn import(app: &mut App, item_id: &str, project: Option<Id>) -> Option<Id> {
    let cfg = connection(app)?;
    let item = app.assigned.get(item_id)?.to_remote();
    let org = org_of(&cfg);
    let key = Task::external_key(&cfg.kind, &org, &item.id);
    let existing = app.store.db.tasks.get(&key).filter(|t| !t.deleted).cloned();
    let fresh = existing.is_none();
    let t = to_task(&cfg, &org, &item, existing.as_ref(), project);
    let id = t.id.clone();
    let place = app.store.db.project(&t.project_id).map(|p| p.name.clone());
    app.store.put_task(t);
    if fresh {
        app.notify(match place {
            Some(p) => trf_m!("#{} imported from Azure DevOps into “{}”", item.id, super::widgets::truncate(&p, 32)),
            None => trf_m!("#{} imported from Azure DevOps, with no project yet", item.id),
        });
    }
    Some(id)
}

/// The work items the task field may suggest: known here, not yet tasks in DevTrack.
pub fn candidates(app: &App) -> Vec<Item> {
    let Some(cfg) = connection(app) else { return vec![] };
    if app.assigned.cfg.as_ref() != Some(&cfg.id) && app.assigned.extra.is_empty() {
        return vec![];
    }
    let org = org_of(&cfg);
    app.assigned
        .all()
        .filter(|i| app.store.db.tasks.get(&Task::external_key(&cfg.kind, &org, &i.id)).map(|t| t.deleted).unwrap_or(true))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::today::{suggestions, RemoteSuggest, Suggest};
    use devtrack_core::sample::{build_state, DemoState};

    fn app_with_azure() -> App {
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Active, false)), None);
        let mut cfg = IntegrationConfig { id: "demo-azure".into(), kind: SOURCE_AZURE.into(), name: "Azure DevOps · contoso".into(), enabled: true, ..Default::default() };
        cfg.settings.insert("organization".into(), "contoso".into());
        app.store.db.integrations.insert(cfg.id.clone(), cfg);
        let item = |id: &str, title: &str, project: &str| Item { id: id.into(), title: title.into(), status: "Active".into(), kind: "Task".into(), project: project.into(), url: String::new(), estimate: Some(4.0) };
        app.assigned.cfg = Some("demo-azure".into());
        app.assigned.at = Some(now_secs());
        // 4812 is already a task in the sample; 7001 and 7002 are not
        app.assigned.items = vec![item("4812", "Login page times out on slow networks", "Portal"), item("7001", "Invoice PDF layout", "Portal"), item("7002", "Nightly export", "Elsewhere")];
        app
    }

    #[test]
    fn a_work_item_goes_where_its_project_mates_went() {
        let app = app_with_azure();
        assert_eq!(likely_project(&app.store.db, "Portal").as_deref(), Some("p-portal"));
        assert_eq!(likely_project(&app.store.db, "Elsewhere"), None);
    }

    #[test]
    fn suggestions_offer_work_items_not_yet_in_devtrack_and_a_lookup_for_unknown_numbers() {
        let app = app_with_azure();
        let ids: Vec<String> = candidates(&app).into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec!["7001", "7002"], "an imported work item is suggested as the task it already is");
        let remote = RemoteSuggest::of(&app).expect("connected");
        let found = suggestions(&app.store.db, "invoice", Some(&remote));
        assert!(found.iter().any(|s| matches!(s, Suggest::Remote(n) if n == "7001")));
        let unknown = suggestions(&app.store.db, "#9999", Some(&remote));
        assert!(unknown.iter().any(|s| matches!(s, Suggest::LookUp(n) if n == "9999")));
        let known = suggestions(&app.store.db, "#7001", Some(&remote));
        assert!(!known.iter().any(|s| matches!(s, Suggest::LookUp(..))), "a number in the list needs no lookup");
        // without a connection nothing from Azure DevOps is offered
        assert!(!suggestions(&app.store.db, "invoice", None).iter().any(|s| matches!(s, Suggest::Remote(..))));
    }

    #[test]
    fn starting_on_a_work_item_imports_it_into_its_likely_project() {
        let mut app = app_with_azure();
        if app.store.db.running().is_some() { app.store.stop(); }
        app.today.draft.desc = "#7001 Invoice PDF layout".into();
        app.today.draft.remote = Some("7001".into());
        app.today.draft.project = likely_project(&app.store.db, "Portal");
        app.start_draft();
        let run = app.store.db.running().cloned().expect("a running session");
        let task = app.store.db.task(&run.task_id).cloned().expect("its task");
        assert_eq!(task.external_id.as_deref(), Some("7001"));
        assert_eq!(task.source, SOURCE_AZURE);
        assert_eq!(task.project_id.as_deref(), Some("p-portal"));
        assert_eq!(run.project_id.as_deref(), Some("p-portal"));
        assert_eq!(task.source_estimate_secs, Some(4 * 3600));
        assert!(!candidates(&app).iter().any(|i| i.id == "7001"), "once imported it is a task, not a suggestion");
    }
}
