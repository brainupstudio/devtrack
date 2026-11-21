//! Azure DevOps, read only (REST api-version 7.1).
//! Calls used: GET projects, POST wiql (a query: it reads, it changes nothing), GET workitems.

use super::{RemoteDetail, RemoteUpdate};
use super::{allowed, record, ErrKind, RemoteItem, RemoteProject, SResult, SourceError, WorkSource, PAGE};
use base64::Engine;
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "api-version=7.1";
const FIELDS: &str = "System.Id,System.Title,System.State,System.WorkItemType,System.TeamProject,System.Description,System.AssignedTo,Microsoft.VSTS.Scheduling.OriginalEstimate";
/// WIQL returns ids only; this caps how many ids one query lists.
const MAX_IDS: usize = 5000;

pub struct AzureDevOps {
    org: String,
    base: String,
    auth: String,
    agent: ureq::Agent,
}

/// Accept "contoso", "https://dev.azure.com/contoso/...", or "https://contoso.visualstudio.com".
pub fn normalize_org(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    let s = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    let org = if let Some(rest) = s.strip_prefix("dev.azure.com/") {
        rest.split('/').next().unwrap_or("")
    } else if let Some(host) = s.split('/').next().filter(|h| h.ends_with(".visualstudio.com")) {
        host.trim_end_matches(".visualstudio.com")
    } else {
        s.split('/').next().unwrap_or("")
    };
    let org = org.trim();
    if org.is_empty() || org.contains(' ') {
        None
    } else {
        Some(org.to_string())
    }
}

pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let text = out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"");
    let collapsed: Vec<&str> = text.split_whitespace().collect();
    let mut t = collapsed.join(" ");
    if t.chars().count() > 600 {
        t = t.chars().take(600).collect::<String>() + "…";
    }
    t
}

fn wiql_text(s: &str) -> String {
    s.replace('\'', "''")
}

impl AzureDevOps {
    pub fn new(org_input: &str, token: &str) -> SResult<Self> {
        let org = normalize_org(org_input).ok_or_else(|| SourceError::new(ErrKind::NotFound, "Enter the organization name (as in dev.azure.com/<name>)"))?;
        let auth = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!(":{}", token.trim())));
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
        Ok(AzureDevOps { base: format!("https://dev.azure.com/{}", org), org, auth, agent })
    }

    fn check(resp: Result<ureq::Response, ureq::Error>) -> SResult<Value> {
        match resp {
            Ok(r) => {
                // A refused token can come back as 203 with a sign-in page instead of JSON.
                if r.status() == 203 {
                    return Err(SourceError::new(ErrKind::Auth, "Azure DevOps refused the token (expired or wrong?)"));
                }
                r.into_json::<Value>().map_err(|_| SourceError::new(ErrKind::Other, "Azure DevOps sent something that isn't data (is the organization right?)"))
            }
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                let msg = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
                    .unwrap_or_default();
                Err(match code {
                    401 => SourceError::new(ErrKind::Auth, "Azure DevOps refused the token (expired or wrong?)"),
                    403 => SourceError::new(ErrKind::Permission, "The token lacks permission: it needs Work Items (Read) and Project and Team (Read)"),
                    404 => SourceError::new(ErrKind::NotFound, "Organization or project not found"),
                    _ => SourceError::new(ErrKind::Other, format!("Azure DevOps answered {}: {}", code, msg)),
                })
            }
            Err(e) => Err(SourceError::new(ErrKind::Offline, format!("Can't reach Azure DevOps: {}", e))),
        }
    }

    fn get_json(&self, url: &str) -> SResult<Value> {
        if !allowed("GET", url) {
            record("GET", url, false);
            return Err(SourceError::new(ErrKind::Blocked, "Refused a request that isn't read-only"));
        }
        record("GET", url, true);
        Self::check(self.agent.get(url).set("Authorization", &self.auth).set("Accept", "application/json").call())
    }

    /// WIQL is sent with POST but only runs a query.
    fn wiql(&self, query: &str, project: Option<&str>) -> SResult<Vec<String>> {
        let scope = project.map(|p| format!("/{}", encode(p))).unwrap_or_default();
        let url = format!("{}{}/_apis/wit/wiql?{}&$top={}", self.base, scope, API, MAX_IDS);
        if !allowed("POST", &url) {
            record("POST", &url, false);
            return Err(SourceError::new(ErrKind::Blocked, "Refused a request that isn't read-only"));
        }
        record("POST", &url, true);
        let v = Self::check(
            self.agent
                .post(&url)
                .set("Authorization", &self.auth)
                .set("Accept", "application/json")
                .send_json(json!({ "query": query })),
        )?;
        Ok(v["workItems"]
            .as_array()
            .map(|a| a.iter().filter_map(|w| w["id"].as_i64().map(|i| i.to_string())).collect())
            .unwrap_or_default())
    }

    fn item_from(&self, v: &Value) -> Option<RemoteItem> {
        let f = &v["fields"];
        let id = v["id"].as_i64()?.to_string();
        let project = f["System.TeamProject"].as_str().unwrap_or("").to_string();
        Some(RemoteItem {
            url: format!("{}/{}/_workitems/edit/{}", self.base, encode(&project), id),
            id,
            title: f["System.Title"].as_str().unwrap_or("").to_string(),
            description: strip_html(f["System.Description"].as_str().unwrap_or("")),
            status: f["System.State"].as_str().unwrap_or("").to_string(),
            kind: f["System.WorkItemType"].as_str().unwrap_or("").to_string(),
            project,
            assigned_to: f["System.AssignedTo"]["displayName"].as_str().unwrap_or("").to_string(),
            estimate: f["Microsoft.VSTS.Scheduling.OriginalEstimate"].as_f64(),
        })
    }
}

impl WorkSource for AzureDevOps {
    fn test(&self) -> SResult<String> {
        let p = self.projects()?;
        Ok(format!("Connected to {}: {} project(s) readable", self.org, p.len()))
    }

    fn projects(&self) -> SResult<Vec<RemoteProject>> {
        let v = self.get_json(&format!("{}/_apis/projects?{}&$top=500", self.base, API))?;
        let mut out: Vec<RemoteProject> = v["value"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|p| RemoteProject { id: p["id"].as_str().unwrap_or("").to_string(), name: p["name"].as_str().unwrap_or("").to_string() })
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by_key(|p| p.name.to_lowercase());
        Ok(out)
    }

    fn assigned_ids(&self, project: Option<&str>) -> SResult<Vec<String>> {
        let mut q = String::from(
            "SELECT [System.Id] FROM WorkItems WHERE [System.AssignedTo] = @Me AND [System.State] NOT IN ('Closed','Removed','Done')",
        );
        if let Some(p) = project {
            q.push_str(&format!(" AND [System.TeamProject] = '{}'", wiql_text(p)));
        }
        q.push_str(" ORDER BY [System.ChangedDate] DESC");
        self.wiql(&q, project)
    }

    fn search_ids(&self, text: &str, project: Option<&str>) -> SResult<Vec<String>> {
        let t = text.trim().trim_start_matches('#');
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
            return Ok(vec![t.to_string()]);
        }
        let mut q = format!("SELECT [System.Id] FROM WorkItems WHERE [System.Title] CONTAINS '{}'", wiql_text(t));
        if let Some(p) = project {
            q.push_str(&format!(" AND [System.TeamProject] = '{}'", wiql_text(p)));
        }
        q.push_str(" ORDER BY [System.ChangedDate] DESC");
        self.wiql(&q, project)
    }

    fn get(&self, ids: &[String]) -> SResult<Vec<RemoteItem>> {
        let mut out = vec![];
        for chunk in ids.chunks(PAGE) {
            if chunk.is_empty() {
                continue;
            }
            let url = format!("{}/_apis/wit/workitems?ids={}&fields={}&errorPolicy=Omit&{}", self.base, chunk.join(","), FIELDS, API);
            let v = self.get_json(&url)?;
            if let Some(a) = v["value"].as_array() {
                out.extend(a.iter().filter_map(|w| self.item_from(w)));
            }
        }
        Ok(out)
    }

    fn detail(&self, id: &str, updates: usize) -> SResult<RemoteDetail> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            return Err(SourceError::new(ErrKind::NotFound, "This task has no Azure DevOps number"));
        }
        let url = format!("{}/_apis/wit/workitems/{}?fields={}&{}", self.base, id, WORK_FIELDS, API);
        let item = self.get_json(&url)?;
        let f = &item["fields"];
        let mut detail = RemoteDetail {
            remaining: f["Microsoft.VSTS.Scheduling.RemainingWork"].as_f64(),
            completed: f["Microsoft.VSTS.Scheduling.CompletedWork"].as_f64(),
            estimate: f["Microsoft.VSTS.Scheduling.OriginalEstimate"].as_f64(),
            updates: vec![],
        };
        // Updates come oldest first, 200 a page: read every page, keep the newest ones.
        let mut all = vec![];
        for page in 0..20 {
            let url = format!("{}/_apis/wit/workitems/{}/updates?$top=200&$skip={}&{}", self.base, id, page * 200, API);
            let v = self.get_json(&url)?;
            let n = v["value"].as_array().map(|a| a.len()).unwrap_or(0);
            all.extend(parse_updates(&v));
            if n < 200 {
                break;
            }
        }
        all.reverse();
        all.truncate(updates);
        detail.updates = all;
        Ok(detail)
    }

    fn org_key(&self) -> String {
        self.org.clone()
    }
}

const WORK_FIELDS: &str = "Microsoft.VSTS.Scheduling.RemainingWork,Microsoft.VSTS.Scheduling.CompletedWork,Microsoft.VSTS.Scheduling.OriginalEstimate";

/// The fields worth a line in the update list, with the name shown for each.
const WATCHED: &[(&str, &str)] = &[
    ("System.State", "State"),
    ("Microsoft.VSTS.Scheduling.RemainingWork", "Remaining"),
    ("Microsoft.VSTS.Scheduling.CompletedWork", "Completed"),
    ("Microsoft.VSTS.Scheduling.OriginalEstimate", "Estimate"),
    ("System.AssignedTo", "Assigned to"),
    ("System.Title", "Title"),
    ("System.IterationPath", "Iteration"),
    ("System.History", "Comment"),
];

fn field_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => strip_html(s).trim().to_string(),
        Value::Number(n) => n.as_f64().map(|x| format!("{}", (x * 100.0).round() / 100.0)).unwrap_or_default(),
        Value::Object(o) => o.get("displayName").and_then(|d| d.as_str()).unwrap_or("").to_string(),
        other => other.to_string(),
    }
}

/// One page of the updates answer, oldest first. An update that touched none of the watched
/// fields is left out: it would be an empty line.
pub fn parse_updates(v: &Value) -> Vec<RemoteUpdate> {
    let mut out = vec![];
    for u in v["value"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let f = &u["fields"];
        let mut changes = vec![];
        for (key, name) in WATCHED {
            let c = &f[*key];
            if c.is_null() {
                continue;
            }
            let (old, new) = (field_text(&c["oldValue"]), field_text(&c["newValue"]));
            // a title or iteration that is set for the first time is the item being created,
            // not news about the work
            if old != new && !(old.is_empty() && matches!(*name, "Title" | "Iteration")) {
                changes.push((name.to_string(), old, new));
            }
        }
        if changes.is_empty() {
            continue;
        }
        // the newest update carries a placeholder revisedDate (year 9999): the change date is reliable
        let date = f["System.ChangedDate"]["newValue"].as_str().or_else(|| u["revisedDate"].as_str()).unwrap_or("");
        let when = chrono::DateTime::parse_from_rfc3339(date).ok().map(|d| d.timestamp()).filter(|t| *t < 4_000_000_000);
        out.push(RemoteUpdate { when, by: u["revisedBy"]["displayName"].as_str().unwrap_or("").to_string(), changes });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn org_forms() {
        assert_eq!(normalize_org("contoso").as_deref(), Some("contoso"));
        assert_eq!(normalize_org("https://dev.azure.com/contoso/Proj").as_deref(), Some("contoso"));
        assert_eq!(normalize_org("https://contoso.visualstudio.com/").as_deref(), Some("contoso"));
        assert_eq!(encode("My Project"), "My%20Project");
    }

    #[test]
    fn updates_keep_the_watched_fields_and_their_dates() {
        let v: Value = serde_json::from_str(r#"{"count":3,"value":[
            {"id":1,"revisedBy":{"displayName":"Ada"},"revisedDate":"2026-11-10T09:00:00Z","fields":{
                "System.ChangedDate":{"newValue":"2026-11-10T08:59:00Z"},
                "System.State":{"newValue":"New"},
                "Microsoft.VSTS.Scheduling.OriginalEstimate":{"newValue":8}}},
            {"id":2,"revisedBy":{"displayName":"Bo"},"revisedDate":"2026-11-12T09:00:00Z","fields":{
                "System.ChangedDate":{"oldValue":"2026-11-10T08:59:00Z","newValue":"2026-11-12T08:00:00Z"},
                "System.Rev":{"oldValue":1,"newValue":2}}},
            {"id":3,"revisedBy":{"displayName":"Ada"},"revisedDate":"9999-01-01T00:00:00Z","fields":{
                "System.ChangedDate":{"oldValue":"2026-11-12T08:00:00Z","newValue":"2026-11-18T16:30:00Z"},
                "System.State":{"oldValue":"New","newValue":"Active"},
                "Microsoft.VSTS.Scheduling.RemainingWork":{"oldValue":8,"newValue":5.5},
                "Microsoft.VSTS.Scheduling.CompletedWork":{"newValue":2.5},
                "System.AssignedTo":{"oldValue":{"displayName":"Bo"},"newValue":{"displayName":"Ada"}},
                "System.History":{"newValue":"<div>Started the work</div>"}}}
        ]}"#).unwrap();
        let u = parse_updates(&v);
        // the update that only bumped the revision is left out
        assert_eq!(u.len(), 2);
        assert_eq!(u[0].by, "Ada");
        assert_eq!(u[0].changes, vec![("State".to_string(), String::new(), "New".to_string()), ("Estimate".to_string(), String::new(), "8".to_string())]);
        let last = &u[1];
        assert!(last.when.is_some(), "the placeholder revisedDate is not used");
        assert_eq!(last.changes[0], ("State".to_string(), "New".to_string(), "Active".to_string()));
        assert!(last.changes.contains(&("Remaining".to_string(), "8".to_string(), "5.5".to_string())));
        assert!(last.changes.contains(&("Completed".to_string(), String::new(), "2.5".to_string())));
        assert!(last.changes.contains(&("Assigned to".to_string(), "Bo".to_string(), "Ada".to_string())));
        assert!(last.changes.contains(&("Comment".to_string(), String::new(), "Started the work".to_string())));
    }
}
