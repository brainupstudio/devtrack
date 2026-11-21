//! External systems as read-only sources of information.
//!
//! Every connector implements [`WorkSource`], which only has reading methods: there is
//! deliberately no way to write hours, change a status or update anything remotely. Every HTTP
//! request also passes [`allowed`], which only lets GET and the read-only WIQL query through,
//! and is recorded in [`request_log`] for the diagnostics screen.
//! Adding Jira later means one more module and one more entry in [`KINDS`].

pub mod azure;
pub mod entries_import;

use crate::model::{IntegrationConfig, Task};
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub struct RemoteProject {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct RemoteItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub kind: String,
    pub project: String,
    pub url: String,
    pub assigned_to: String,
    /// Original Estimate in hours, when the source has one.
    pub estimate: Option<f64>,
}

/// One change made to a work item at its source: when, by whom, and which fields changed
/// (field name, old value, new value). Read only.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteUpdate {
    /// Unix seconds; `None` when the source gave no usable date.
    pub when: Option<i64>,
    pub by: String,
    pub changes: Vec<(String, String, String)>,
}

/// What the source says about the work on an item: hours remaining, completed and estimated,
/// and its most recent updates, newest first. Read only, never written back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteDetail {
    pub remaining: Option<f64>,
    pub completed: Option<f64>,
    pub estimate: Option<f64>,
    pub updates: Vec<RemoteUpdate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrKind {
    /// No network or the server can't be reached.
    Offline,
    /// Token refused or expired.
    Auth,
    /// Token lacks a read permission.
    Permission,
    NotFound,
    /// A request DevTrack refuses to send because it could change remote data.
    Blocked,
    Other,
}

#[derive(Clone, Debug)]
pub struct SourceError {
    pub kind: ErrKind,
    pub message: String,
}

impl SourceError {
    pub fn new(kind: ErrKind, message: impl Into<String>) -> Self {
        SourceError { kind, message: message.into() }
    }
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub type SResult<T> = Result<T, SourceError>;

/// Largest number of item details fetched in one go; more are loaded on request.
pub const PAGE: usize = 200;

/// What a connector can do. Read only.
pub trait WorkSource: Send {
    fn test(&self) -> SResult<String>;
    fn projects(&self) -> SResult<Vec<RemoteProject>>;
    /// Ids of all items assigned to the signed-in person (open ones), newest change first.
    fn assigned_ids(&self, project: Option<&str>) -> SResult<Vec<String>>;
    /// Ids of items matching a number or words in the title.
    fn search_ids(&self, text: &str, project: Option<&str>) -> SResult<Vec<String>>;
    /// Details of items (missing or inaccessible ones are simply absent).
    fn get(&self, ids: &[String]) -> SResult<Vec<RemoteItem>>;
    /// Remaining, completed and estimated work, plus the last `updates` changes.
    fn detail(&self, id: &str, updates: usize) -> SResult<RemoteDetail>;
    /// Stable organisation name, part of imported task ids.
    fn org_key(&self) -> String;
}

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The only requests DevTrack sends: GET, and POST to the WIQL query endpoint (a query that
/// reads; Azure DevOps uses POST only to carry the query text).
pub fn allowed(method: &str, url: &str) -> bool {
    let path = url.split('?').next().unwrap_or("");
    method == "GET" || (method == "POST" && path.ends_with("/_apis/wit/wiql"))
}

pub fn record(method: &str, url: &str, ok: bool) {
    let path = url.split('?').next().unwrap_or("").to_string();
    let line = format!("{} {} {} {}", chrono::Local::now().format("%H:%M:%S"), method, path, if ok { "" } else { "(refused)" });
    if let Ok(mut l) = LOG.lock() {
        l.push(line.trim_end().to_string());
        let n = l.len();
        if n > 200 {
            l.drain(0..n - 200);
        }
    }
}

/// Requests sent this session (method and path, no tokens or query values).
pub fn request_log() -> Vec<String> {
    LOG.lock().map(|l| l.clone()).unwrap_or_default()
}

pub struct Kind {
    pub id: &'static str,
    pub name: &'static str,
    pub available: bool,
    pub permissions: &'static str,
}

/// Connectors DevTrack knows about.
pub const KINDS: &[Kind] = &[
    Kind {
        id: crate::model::SOURCE_AZURE,
        name: "Azure DevOps",
        available: true,
        permissions: "Personal access token with Work Items (Read) and Project and Team (Read) only.",
    },
    Kind { id: "jira", name: "Jira", available: false, permissions: "Planned after the main workflow is reliable: API token with read scopes only." },
];

pub fn kind(id: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.id == id)
}

pub fn org_of(cfg: &IntegrationConfig) -> String {
    azure::normalize_org(cfg.settings.get("organization").map(|s| s.as_str()).unwrap_or("")).unwrap_or_default()
}

/// Build the connector for a configuration and its secret.
pub fn connect(cfg: &IntegrationConfig, secret: Option<String>) -> SResult<Box<dyn WorkSource>> {
    let secret = secret.ok_or_else(|| SourceError::new(ErrKind::Auth, "No access token saved on this device for this connection"))?;
    match cfg.kind.as_str() {
        crate::model::SOURCE_AZURE => {
            let org = cfg.settings.get("organization").cloned().unwrap_or_default();
            Ok(Box::new(azure::AzureDevOps::new(&org, &secret)?))
        }
        other => Err(SourceError::new(ErrKind::Other, format!("{} is not available yet", other))),
    }
}

/// Turn a remote item into a local task, or refresh an existing one.
/// Local choices are kept: an existing task keeps its local project; `target` only applies to
/// tasks that have none yet.
pub fn to_task(cfg: &IntegrationConfig, org: &str, item: &RemoteItem, existing: Option<&Task>, target: Option<String>) -> Task {
    let id = Task::external_key(&cfg.kind, org, &item.id);
    let mut t = existing.cloned().unwrap_or_else(|| Task { id: id.clone(), ..Default::default() });
    t.id = id;
    t.title = item.title.clone();
    t.description = item.description.clone();
    t.status = item.status.clone();
    t.source = cfg.kind.clone();
    t.external_id = Some(item.id.clone());
    t.external_project = Some(item.project.clone());
    t.external_type = Some(item.kind.clone());
    t.url = Some(item.url.clone());
    t.integration_id = Some(cfg.id.clone());
    t.source_estimate_secs = item.estimate.filter(|h| *h > 0.0).map(|h| (h * 3600.0).round() as i64);
    t.refreshed_at = Some(crate::store::now_secs());
    t.unavailable = false;
    t.deleted = false;
    if t.project_id.is_none() {
        t.project_id = target;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_reads_are_allowed() {
        assert!(allowed("GET", "https://dev.azure.com/o/_apis/projects?api-version=7.1"));
        assert!(allowed("POST", "https://dev.azure.com/o/P/_apis/wit/wiql?api-version=7.1&$top=1"));
        assert!(!allowed("PATCH", "https://dev.azure.com/o/_apis/wit/workitems/1?api-version=7.1"));
        assert!(!allowed("POST", "https://dev.azure.com/o/_apis/wit/workitems/$Task?api-version=7.1"));
        assert!(!allowed("DELETE", "https://dev.azure.com/o/_apis/wit/workitems/1"));
        assert!(!allowed("POST", "https://dev.azure.com/o/_apis/wit/wiql/../workitems"));
    }

    #[test]
    fn reimport_keeps_local_project() {
        let cfg = IntegrationConfig { id: "c".into(), kind: crate::model::SOURCE_AZURE.into(), ..Default::default() };
        let item = RemoteItem { id: "7".into(), title: "T".into(), description: String::new(), status: "Active".into(), kind: "Bug".into(), project: "X".into(), url: "u".into(), assigned_to: String::new(), estimate: Some(3.0) };
        let first = to_task(&cfg, "org", &item, None, Some("local-a".into()));
        assert_eq!(first.project_id.as_deref(), Some("local-a"));
        let again = to_task(&cfg, "org", &item, Some(&first), Some("local-b".into()));
        assert_eq!(again.project_id.as_deref(), Some("local-a"));
        assert_eq!(again.id, first.id);
        // the source's estimate comes with the item and is what the task plans
        assert_eq!(again.planned_secs(), Some(3 * 3600));
    }
}
