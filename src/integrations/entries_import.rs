//! Import time entries from another time-tracking service: the person's own entries, read once
//! from the service address and API token they type in.
//!
//! One read-only request, `GET <address>/api/v9/me/time_entries` with a date range and
//! `meta=true` (so each entry carries its project, colour and client by name). Neither the
//! address nor the token is stored. Then [`plan`] works out, without touching anything, which
//! sessions are new, which are already in DevTrack, and which projects, clients and tasks would
//! be created; [`apply`] writes exactly that.
//!
//! An entry's description is what was worked on: "Meeting", "28913 – Home page". So each
//! description becomes a task of its project (one per name, not one per session), and a
//! description that starts with the number of an Azure DevOps work item already in DevTrack is
//! linked to it.

use super::{allowed, record};
use crate::model::{new_id, Client, Db, Id, Project, Task, TimeEntry, SOURCE_MANUAL};
use base64::Engine;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

/// The path of the entries on the service, after the address the person types.
const ENTRIES_PATH: &str = "/api/v9/me/time_entries";

/// The service address as typed, made into a base URL: "https://" added when missing, trailing
/// slashes removed. None when there is nothing usable.
pub fn base_url(address: &str) -> Option<String> {
    let a = address.trim().trim_end_matches('/');
    if a.is_empty() || a.contains(char::is_whitespace) {
        return None;
    }
    let url = if a.starts_with("https://") || a.starts_with("http://") { a.to_string() } else { format!("https://{}", a) };
    Some(url)
}

/// One finished entry as the service gives it, reduced to what DevTrack keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedEntry {
    pub id: i64,
    pub description: String,
    pub start: i64,
    pub stop: i64,
    pub project: Option<String>,
    pub project_color: Option<String>,
    pub client: Option<String>,
    pub billable: bool,
    pub tags: Vec<String>,
}

/// Reads the entries that start between `from` and `to` (YYYY-MM-DD, `to` included).
/// Running entries are left out: they are still changing on the service.
pub fn fetch(address: &str, token: &str, from: &str, to_inclusive: &str) -> Result<Vec<ImportedEntry>, String> {
    let base = base_url(address).ok_or_else(|| "Type the service's address".to_string())?;
    let token = token.trim();
    if token.is_empty() {
        return Err("Paste your API token".into());
    }
    // the service's end date is exclusive: one day more covers the last day
    let end = chrono::NaiveDate::parse_from_str(to_inclusive, "%Y-%m-%d").map_err(|_| "Invalid end date".to_string())? + chrono::Duration::days(1);
    let url = format!("{}{}?start_date={}&end_date={}&meta=true", base, ENTRIES_PATH, from, end.format("%Y-%m-%d"));
    if !allowed("GET", &url) {
        record("GET", &url, false);
        return Err("Refused a request that isn't read-only".into());
    }
    record("GET", &url, true);
    let auth = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{}:api_token", token)));
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let resp = agent.get(&url).set("Authorization", &auth).set("Accept", "application/json").call();
    let body: Value = match resp {
        Ok(r) => r.into_json().map_err(|_| "The service sent something that isn't data: check its address".to_string())?,
        Err(ureq::Error::Status(code, _)) => {
            return Err(match code {
                401 | 403 => "The service refused the token: copy it again from your profile on the service".into(),
                402 | 429 => "The service's request limit was reached: try again in an hour".into(),
                404 => "Nothing at that address: check the service's address".into(),
                _ => format!("The service answered {}", code),
            })
        }
        Err(e) => return Err(format!("Can't reach the service: {}", e)),
    };
    Ok(parse(&body))
}

/// The entries in the service's answer: finished ones only, oldest first.
pub fn parse(v: &Value) -> Vec<ImportedEntry> {
    let time = |k: &str, e: &Value| e[k].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.timestamp());
    let text = |k: &str, e: &Value| e[k].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut out: Vec<ImportedEntry> = v
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|e| {
            let start = time("start", e)?;
            let stop = time("stop", e)?;
            if stop <= start || e["duration"].as_i64().unwrap_or(0) < 0 {
                return None;
            }
            Some(ImportedEntry {
                id: e["id"].as_i64().unwrap_or(0),
                description: text("description", e).unwrap_or_default(),
                start,
                stop,
                project: text("project_name", e),
                project_color: text("project_color", e),
                client: text("client_name", e),
                billable: e["billable"].as_bool().unwrap_or(false),
                tags: e["tags"].as_array().map(|t| t.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
            })
        })
        .collect();
    out.sort_by_key(|e| e.start);
    out
}

/// What an import would do, worked out before anything is written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// Entries that become new sessions.
    pub new: Vec<ImportedEntry>,
    /// Entries already in DevTrack (same start, end and description), left alone.
    pub already: usize,
    /// Projects to create, by name, with the client they belong to.
    pub new_projects: Vec<(String, Option<String>)>,
    /// Clients to create, by name.
    pub new_clients: Vec<String>,
    /// Tasks to create: (project name, title).
    pub new_tasks: Vec<(Option<String>, String)>,
    /// Sessions linked to an Azure DevOps work item already in DevTrack.
    pub linked: usize,
}

/// The work item number a description starts with: "28913 – Home page" → "28913".
fn work_item_number(desc: &str) -> Option<String> {
    let n: String = desc.trim().trim_start_matches('#').chars().take_while(|c| c.is_ascii_digit()).collect();
    (n.len() >= 3).then_some(n)
}

/// An imported Azure DevOps task with that number.
fn devops_task<'a>(db: &'a Db, desc: &str) -> Option<&'a Task> {
    let n = work_item_number(desc)?;
    db.live_tasks().find(|t| t.source != SOURCE_MANUAL && t.external_id.as_deref() == Some(n.as_str()))
}

fn key(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Matches projects and clients by name (letter case aside) and finds sessions already there.
pub fn plan(db: &Db, entries: &[ImportedEntry]) -> Plan {
    let mut p = Plan::default();
    let projects: BTreeMap<String, &Project> = db.live_projects().into_iter().map(|x| (key(&x.name), x)).collect();
    let clients: BTreeMap<String, &Client> = db.live_clients().into_iter().map(|x| (key(&x.name), x)).collect();
    for e in entries {
        // the same session already here: same times, and its note or its task says the same
        let dup = db.live_entries().any(|x| {
            x.start == e.start && x.stop == Some(e.stop) && (x.description.trim() == e.description.trim() || db.task(&x.task_id).is_some_and(|t| t.title.trim() == e.description.trim()))
        });
        if dup {
            p.already += 1;
            continue;
        }
        if let Some(name) = &e.project {
            if !projects.contains_key(&key(name)) && !p.new_projects.iter().any(|(n, _)| key(n) == key(name)) {
                p.new_projects.push((name.clone(), e.client.clone()));
                if let Some(c) = &e.client {
                    if !clients.contains_key(&key(c)) && !p.new_clients.iter().any(|n| key(n) == key(c)) {
                        p.new_clients.push(c.clone());
                    }
                }
            }
        }
        if !e.description.is_empty() {
            if devops_task(db, &e.description).is_some() {
                p.linked += 1;
            } else {
                let exists_here = db.live_tasks().any(|t| {
                    t.title.trim().to_lowercase() == key(&e.description) && db.project(&t.project_id).map(|pr| key(&pr.name)) == e.project.as_deref().map(key)
                });
                let planned = p.new_tasks.iter().any(|(pn, title)| key(title) == key(&e.description) && pn.as_deref().map(key) == e.project.as_deref().map(key));
                if !exists_here && !planned {
                    p.new_tasks.push((e.project.clone(), e.description.clone()));
                }
            }
        }
        p.new.push(e.clone());
    }
    p
}

/// What to write: the clients, projects and sessions of a plan, as records. `next_color` gives a
/// colour to a project the service didn't colour.
pub fn apply(db: &Db, plan: &Plan, mut next_color: impl FnMut() -> String) -> (Vec<Client>, Vec<Project>, Vec<Task>, Vec<TimeEntry>) {
    let mut clients: BTreeMap<String, Id> = db.live_clients().into_iter().map(|x| (key(&x.name), x.id.clone())).collect();
    let mut new_clients = vec![];
    for name in &plan.new_clients {
        let c = Client { id: new_id(), name: name.trim().to_string(), ..Default::default() };
        clients.insert(key(name), c.id.clone());
        new_clients.push(c);
    }
    let mut projects: BTreeMap<String, Id> = db.live_projects().into_iter().map(|x| (key(&x.name), x.id.clone())).collect();
    let colors: BTreeMap<String, String> = plan.new.iter().filter_map(|e| Some((key(e.project.as_ref()?), e.project_color.clone()?))).collect();
    let mut new_projects = vec![];
    for (name, client) in &plan.new_projects {
        let client_id = client.as_ref().and_then(|c| clients.get(&key(c)).cloned());
        let client_name = client.clone().unwrap_or_default();
        let color = colors.get(&key(name)).cloned().filter(|c| c.starts_with('#') && c.len() == 7).unwrap_or_else(&mut next_color);
        let pr = Project { id: new_id(), name: name.trim().to_string(), color, client_id, client: client_name, ..Default::default() };
        projects.insert(key(name), pr.id.clone());
        new_projects.push(pr);
    }
    // tasks: (project id, title) → task id, existing ones first, then the new ones
    let mut tasks: BTreeMap<(Option<Id>, String), Id> = db.live_tasks().map(|t| ((t.project_id.clone(), key(&t.title)), t.id.clone())).collect();
    let mut new_tasks = vec![];
    for (pname, title) in &plan.new_tasks {
        let pid = pname.as_ref().and_then(|n| projects.get(&key(n)).cloned());
        let k = (pid.clone(), key(title));
        if tasks.contains_key(&k) {
            continue;
        }
        let t = Task { id: new_id(), project_id: pid, title: title.trim().to_string(), status: "Open".into(), source: SOURCE_MANUAL.into(), ..Default::default() };
        tasks.insert(k, t.id.clone());
        new_tasks.push(t);
    }
    let entries = plan
        .new
        .iter()
        .map(|e| {
            let mut project_id = e.project.as_ref().and_then(|n| projects.get(&key(n)).cloned());
            // the description is the task: a work item already here, or the project's task
            let task_id = if e.description.is_empty() {
                None
            } else if let Some(t) = devops_task(db, &e.description) {
                if project_id.is_none() {
                    project_id = t.project_id.clone();
                }
                Some(t.id.clone())
            } else {
                tasks.get(&(project_id.clone(), key(&e.description))).cloned()
            };
            TimeEntry {
                id: new_id(),
                description: String::new(),
                project_id,
                task_id,
                start: e.start,
                stop: Some(e.stop),
                billable: e.billable,
                tags: e.tags.clone(),
                ..Default::default()
            }
        })
        .collect();
    (new_clients, new_projects, new_tasks, entries)
}

/// Sessions already in DevTrack whose description is really their task (imported before, or
/// typed before tasks came first): each description becomes a task of the session's project,
/// one per name, or the Azure DevOps work item it starts with. Returns the tasks to create and
/// the sessions to update (their task set, their description cleared).
pub fn descriptions_to_tasks(db: &Db) -> (Vec<Task>, Vec<TimeEntry>) {
    let mut tasks: BTreeMap<(Option<Id>, String), Id> = db.live_tasks().filter(|t| !t.done).map(|t| ((t.project_id.clone(), key(&t.title)), t.id.clone())).collect();
    let mut new_tasks = vec![];
    let mut changed = vec![];
    for e in db.live_entries() {
        if e.task_id.is_some() || e.description.trim().is_empty() || e.is_running() {
            continue;
        }
        let mut x = e.clone();
        if let Some(t) = devops_task(db, &e.description) {
            x.task_id = Some(t.id.clone());
            if x.project_id.is_none() {
                x.project_id = t.project_id.clone();
            }
        } else {
            let k = (e.project_id.clone(), key(&e.description));
            let id = match tasks.get(&k) {
                Some(id) => id.clone(),
                None => {
                    let t = Task { id: new_id(), project_id: e.project_id.clone(), title: e.description.trim().to_string(), status: "Open".into(), source: SOURCE_MANUAL.into(), ..Default::default() };
                    let id = t.id.clone();
                    tasks.insert(k, id.clone());
                    new_tasks.push(t);
                    id
                }
            };
            x.task_id = Some(id);
        }
        x.description = String::new();
        changed.push(x);
    }
    (new_tasks, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!([
            {"id": 1, "description": "Stand-up", "start": "2026-11-16T08:00:00+00:00", "stop": "2026-11-16T08:15:00Z", "duration": 900,
             "project_name": "Home banking", "project_color": "#2da608", "client_name": "Banca Aurora", "billable": true, "tags": ["meeting"]},
            {"id": 2, "description": "Login", "start": "2026-11-16T09:00:00Z", "stop": "2026-11-16T10:30:00Z", "duration": 5400,
             "project_name": "home banking", "client_name": "Banca Aurora", "billable": false, "tags": null},
            {"id": 3, "description": "Running", "start": "2026-11-17T09:00:00Z", "stop": null, "duration": -1},
            {"id": 4, "description": "", "start": "2026-11-17T11:00:00Z", "stop": "2026-11-17T11:30:00Z", "duration": 1800}
        ])
    }

    #[test]
    fn the_address_becomes_a_base_url() {
        assert_eq!(base_url("api.example.com/").as_deref(), Some("https://api.example.com"));
        assert_eq!(base_url(" https://api.example.com ").as_deref(), Some("https://api.example.com"));
        assert_eq!(base_url(""), None);
        assert_eq!(base_url("not an address"), None);
    }

    #[test]
    fn running_entries_are_left_out_and_the_rest_sorted() {
        let e = parse(&sample());
        assert_eq!(e.iter().map(|x| x.id).collect::<Vec<_>>(), vec![1, 2, 4]);
        assert_eq!(e[0].stop - e[0].start, 900);
        assert_eq!(e[0].tags, vec!["meeting".to_string()]);
        assert_eq!(e[2].project, None);
    }

    #[test]
    fn projects_match_by_name_and_a_second_import_adds_nothing() {
        let mut db = Db::default();
        let entries = parse(&sample());
        let p = plan(&db, &entries);
        // "Home banking" and "home banking" are one project, created once with its client
        assert_eq!(p.new.len(), 3);
        assert_eq!(p.new_projects, vec![("Home banking".to_string(), Some("Banca Aurora".to_string()))]);
        assert_eq!(p.new_clients, vec!["Banca Aurora".to_string()]);
        assert_eq!(p.new_tasks.len(), 2, "Stand-up and Login in Home banking, nothing for the empty one");
        let (clients, projects, tasks, sessions) = apply(&db, &p, || "#888888".into());
        assert_eq!(tasks.len(), 2, "one task per name and project");
        assert!(sessions[0].task_id.is_some() && sessions[0].description.is_empty());
        assert_eq!(projects[0].color, "#2da608");
        assert_eq!(projects[0].client_id.as_ref(), Some(&clients[0].id));
        assert!(sessions[0].project_id.is_some() && sessions[1].project_id == sessions[0].project_id);
        for c in clients { db.clients.insert(c.id.clone(), c); }
        for x in projects { db.projects.insert(x.id.clone(), x); }
        for t in tasks { db.tasks.insert(t.id.clone(), t); }
        for s in sessions { db.entries.insert(s.id.clone(), s); }
        let again = plan(&db, &entries);
        assert_eq!((again.new.len(), again.already), (0, 3));
        assert!(again.new_projects.is_empty() && again.new_clients.is_empty() && again.new_tasks.is_empty());
    }

    #[test]
    fn descriptions_become_one_task_per_name_and_work_items_are_linked() {
        let mut db = Db::default();
        let pr = Project { id: "p".into(), name: "Sito".into(), ..Default::default() };
        db.projects.insert("p".into(), pr);
        let wi = Task { id: "az".into(), project_id: Some("p".into()), title: "Home page".into(), source: "azure".into(), external_id: Some("28913".into()), ..Default::default() };
        db.tasks.insert("az".into(), wi);
        for (i, d) in ["Meeting", "meeting", "28913 – Home page", ""].iter().enumerate() {
            let e = TimeEntry { id: format!("e{i}"), project_id: Some("p".into()), description: d.to_string(), start: i as i64 * 100, stop: Some(i as i64 * 100 + 50), ..Default::default() };
            db.entries.insert(e.id.clone(), e);
        }
        let (tasks, changed) = descriptions_to_tasks(&db);
        assert_eq!(tasks.len(), 1, "\"Meeting\" and \"meeting\" are one task");
        assert_eq!(changed.len(), 3, "the session without a description is left alone");
        assert_eq!(changed.iter().find(|e| e.id == "e2").and_then(|e| e.task_id.clone()), Some("az".to_string()));
        assert!(changed.iter().all(|e| e.description.is_empty()));
    }
}
