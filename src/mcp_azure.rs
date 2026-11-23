//! Azure DevOps work items for the MCP server, read only.
//!
//! - `assigned`: the work items assigned to the person, from the copy the app keeps
//!   (`azure-assigned.json`); no request leaves the computer.
//! - `work_item`: one work item read live through DevTrack's read-only client (GET and the
//!   read-only query only; anything else is refused before it is sent), with its remaining and
//!   completed work and its latest changes, next to the time DevTrack has on it.
//!
//! The access token is read from the system keychain to make the request and never appears in
//! an answer.

use crate::facts::{self, duration};
use crate::integrations::{RemoteDetail, RemoteItem};
use crate::model::{Db, Task, SOURCE_AZURE};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// The app's copy of the assigned list (`ui::assigned::Assigned`), as far as it is read here.
#[derive(Deserialize, Default)]
struct AssignedFile {
    #[serde(default)]
    items: Vec<AssignedItem>,
    #[serde(default)]
    at: Option<i64>,
}

#[derive(Deserialize)]
struct AssignedItem {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    estimate: Option<f64>,
}

fn secs_of_hours(h: Option<f64>) -> Option<i64> {
    h.map(|h| (h * 3600.0).round() as i64)
}

/// "28885", "#28885" or "28885 – Modulo…" → "28885".
pub fn number(input: &str) -> Option<String> {
    let t = input.trim().trim_start_matches('#');
    let n: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    (!n.is_empty()).then_some(n)
}

/// The DevTrack task for a work item: the imported one, or else a local task whose title starts
/// with its number ("28885 – Modulo C01+C03"), which is how such tasks are often written by hand.
pub fn task_for<'a>(db: &'a Db, id: &str) -> Option<(&'a Task, &'static str)> {
    if let Some(t) = db.live_tasks().find(|t| t.source == SOURCE_AZURE && t.external_id.as_deref() == Some(id)) {
        return Some((t, "imported"));
    }
    db.live_tasks().find(|t| number(&t.title).as_deref() == Some(id) && t.external_id.is_none()).map(|t| (t, "title"))
}

fn devtrack_side(db: &Db, id: &str, now: i64) -> Value {
    match task_for(db, id) {
        Some((t, how)) => {
            let tracked = db.task_total(&t.id, now);
            let last = db.live_entries().filter(|e| e.task_id.as_deref() == Some(t.id.as_str())).map(|e| e.start).max();
            json!({
                "task": t.title,
                "project": db.project(&t.project_id).map(|p| p.name.clone()).unwrap_or_else(|| facts::NO_PROJECT.into()),
                "client": db.project(&t.project_id).map(|p| db.client_name(p)).filter(|c| !c.is_empty()).unwrap_or_else(|| facts::NO_CLIENT.into()),
                "matched_by": if how == "imported" { "imported from Azure DevOps" } else { "a local task whose title starts with the number" },
                "tracked": duration(tracked),
                "tracked_seconds": tracked,
                "last_session": last.map(|s| crate::report::local_date(s).to_string()),
            })
        }
        None => Value::Null,
    }
}

/// The work items assigned to the person, as the app last read them, with what DevTrack has on
/// each. `query` matches the title, the number or the Azure DevOps project, `state` the state; `only_untracked` keeps those
/// with no time in DevTrack.
pub fn assigned(dir: &Path, db: &Db, now: i64, query: Option<&str>, state: Option<&str>, only_untracked: bool) -> Result<Value, String> {
    let file: AssignedFile = match std::fs::read(dir.join("azure-assigned.json")) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| format!("DevTrack's list of assigned work items can't be read ({e})."))?,
        Err(_) => return Err("DevTrack has no list of assigned work items yet: open Work › Add task › From Azure DevOps in the app once.".into()),
    };
    let q = query.map(|q| q.trim().trim_start_matches('#').to_lowercase());
    let mut rows = vec![];
    for it in &file.items {
        if q.as_deref().is_some_and(|q| !it.title.to_lowercase().contains(q) && !it.id.contains(q) && !it.project.to_lowercase().contains(q)) {
            continue;
        }
        if state.is_some_and(|s| !it.status.eq_ignore_ascii_case(s.trim())) {
            continue;
        }
        let side = devtrack_side(db, &it.id, now);
        let tracked = side.get("tracked_seconds").and_then(Value::as_i64).unwrap_or(0);
        if only_untracked && tracked > 0 {
            continue;
        }
        let est = secs_of_hours(it.estimate);
        rows.push(json!({
            "work_item": format!("#{}", it.id),
            "title": it.title,
            "type": it.kind,
            "state": it.status,
            "azure_project": it.project,
            "url": it.url,
            "original_estimate": est.map(duration),
            "original_estimate_seconds": est,
            "in_devtrack": side,
        }));
    }
    let read = file.at.map(|t| {
        let mins = (now - t).max(0) / 60;
        json!({ "at": chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&chrono::Local).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)), "minutes_ago": mins })
    });
    Ok(json!({
        "meta": facts::meta(db, None, now),
        "list_read": read,
        "note": "The list as DevTrack last read it from Azure DevOps (the app refreshes it every 15 minutes while it is open). For a work item's current state and work, use devtrack_work_item.",
        "count": rows.len(),
        "work_items": rows,
    }))
}

/// Reads one work item from Azure DevOps: the item and its work, with `updates` latest changes.
pub type Reader = dyn Fn(&Db, &str, usize) -> Result<(RemoteItem, RemoteDetail), String>;

/// The live reader: the connection the task came from, or the first Azure DevOps connection that
/// is on, with its token from the keychain.
pub fn live(db: &Db, id: &str, updates: usize) -> Result<(RemoteItem, RemoteDetail), String> {
    let from_task = task_for(db, id).and_then(|(t, _)| t.integration_id.clone());
    let mut conns: Vec<_> = db.integrations.values().filter(|c| !c.deleted && c.enabled && c.kind == SOURCE_AZURE).collect();
    conns.sort_by_key(|c| Some(&c.id) != from_task.as_ref());
    let cfg = conns.first().ok_or("There is no Azure DevOps connection in DevTrack (Settings › Connections).")?;
    let src = crate::integrations::connect(cfg, crate::secrets::get(&cfg.id)).map_err(|e| e.to_string())?;
    let item = src.get(&[id.to_string()]).map_err(|e| e.to_string())?.into_iter().next().ok_or_else(|| format!("#{id} isn't in Azure DevOps, or this connection can't see it."))?;
    let detail = src.detail(id, updates).map_err(|e| e.to_string())?;
    Ok((item, detail))
}

/// One work item as Azure DevOps has it now, next to what DevTrack has tracked on it.
pub fn work_item(db: &Db, now: i64, input: &str, updates: usize, read: &Reader) -> Result<Value, String> {
    let id = number(input).ok_or_else(|| format!("“{}” is not a work item number.", input.trim()))?;
    let (item, detail) = read(db, &id, updates)?;
    let est = secs_of_hours(detail.estimate.or(item.estimate));
    let done = secs_of_hours(detail.completed);
    let left = secs_of_hours(detail.remaining);
    let side = devtrack_side(db, &id, now);
    let tracked = side.get("tracked_seconds").and_then(Value::as_i64);
    let comparison = match (tracked, done) {
        (Some(t), Some(d)) => Some(json!({
            "devtrack_minus_completed_seconds": t - d,
            "says": if (t - d).abs() < 15 * 60 {
                "DevTrack's tracked time and Azure's Completed Work agree (within 15 minutes).".to_string()
            } else if t > d {
                format!("DevTrack has {} more than Azure's Completed Work: hours not reported in Azure yet?", duration(t - d))
            } else {
                format!("Azure's Completed Work is {} more than DevTrack's tracked time: work done without the timer, or by someone else?", duration(d - t))
            },
        })),
        _ => None,
    };
    let changes: Vec<Value> = detail
        .updates
        .iter()
        .map(|u| {
            json!({
                "when": u.when.and_then(|w| chrono::DateTime::from_timestamp(w, 0)).map(|d| d.with_timezone(&chrono::Local).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
                "by": u.by,
                "changes": u.changes.iter().map(|(f, a, b)| json!({ "field": f, "from": a, "to": b })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({
        "meta": facts::meta(db, None, now),
        "work_item": format!("#{id}"),
        "title": item.title,
        "type": item.kind,
        "state": item.status,
        "assigned_to": item.assigned_to,
        "azure_project": item.project,
        "url": item.url,
        "azure_work": {
            "original_estimate": est.map(duration), "original_estimate_seconds": est,
            "completed_work": done.map(duration), "completed_work_seconds": done,
            "remaining_work": left.map(duration), "remaining_work_seconds": left,
        },
        "in_devtrack": side,
        "comparison": comparison,
        "latest_changes": changes,
        "note": "Read live from Azure DevOps, read only. Completed and Remaining Work are what is entered in Azure; DevTrack's tracked time is measured by the timer.",
    }))
}

// ---------- progress in a report ----------

/// Most work items read live for one report: each is two requests to Azure DevOps.
pub const MAX_PROGRESS_ITEMS: usize = 12;

/// The work items behind the time of a period: imported tasks, and local tasks whose title
/// starts with a number. Each once, most tracked first.
pub fn work_items_in(db: &Db, p: &crate::report::Period, now: i64, filter: &facts::Filter) -> Vec<String> {
    let mut secs: std::collections::BTreeMap<String, i64> = Default::default();
    for r in facts::time_rows(db, p, now, filter) {
        let Some(t) = db.task(&r.task_id) else { continue };
        let id = if t.source == SOURCE_AZURE { t.external_id.clone() } else { number(&t.title) };
        if let Some(id) = id {
            *secs.entry(id).or_default() += r.seconds;
        }
    }
    let mut v: Vec<(String, i64)> = secs.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    v.into_iter().map(|x| x.0).collect()
}

/// Where a work item stands, in words, from its estimate, Azure's Completed Work and the time
/// DevTrack tracked on it (seconds).
pub fn verdict(est: Option<i64>, done: Option<i64>, tracked: i64, state: &str) -> String {
    let closed = matches!(state.to_lowercase().as_str(), "closed" | "done" | "resolved" | "removed" | "completed");
    const SLACK: i64 = 15 * 60;
    let used = done.unwrap_or(0).max(tracked);
    if let Some(e) = est.filter(|e| *e > 0) {
        if used > e + SLACK {
            return format!("! Over the estimate by {}", duration(used - e));
        }
    }
    match done {
        Some(d) if tracked - d > SLACK => format!("! {} not reported in Azure yet", duration(tracked - d)),
        Some(d) if d - tracked > SLACK => format!("Azure has {} more than DevTrack", duration(d - tracked)),
        _ => match est.filter(|e| *e > 0) {
            Some(e) => {
                let pct = ((used as f64) * 100.0 / e as f64).round() as i64;
                if closed { format!("Done within the estimate ({pct}%)") } else { format!("On track ({pct}%)") }
            }
            None => "No estimate".into(),
        },
    }
}

/// The "Azure DevOps progress" table of a report: for each work item worked on in the period,
/// its state, estimate, Completed and Remaining Work read live, and DevTrack's tracked time
/// (all of it), with where it stands. Also the rows as data.
pub fn progress_table(db: &Db, p: &crate::report::Period, now: i64, filter: &facts::Filter, read: &Reader) -> (facts::Table, Vec<Value>, Vec<String>) {
    let ids = work_items_in(db, p, now, filter);
    let mut notes = vec![];
    if ids.len() > MAX_PROGRESS_ITEMS {
        notes.push(format!("{} work items in the period; the {} with most time are read from Azure DevOps.", ids.len(), MAX_PROGRESS_ITEMS));
    }
    let mut rows = vec![];
    let mut data = vec![];
    for id in ids.iter().take(MAX_PROGRESS_ITEMS) {
        let tracked = task_for(db, id).map(|(t, _)| db.task_total(&t.id, now)).unwrap_or(0);
        match read(db, id, 0) {
            Ok((item, detail)) => {
                let est = secs_of_hours(detail.estimate.or(item.estimate));
                let done = secs_of_hours(detail.completed);
                let left = secs_of_hours(detail.remaining);
                let v = verdict(est, done, tracked, &item.status);
                rows.push(vec![
                    format!("#{} {}", id, item.title),
                    item.status.clone(),
                    est.map(duration).unwrap_or_else(|| "–".into()),
                    done.map(duration).unwrap_or_else(|| "–".into()),
                    duration(tracked),
                    left.map(duration).unwrap_or_else(|| "–".into()),
                    v.clone(),
                ]);
                data.push(json!({
                    "work_item": format!("#{id}"), "title": item.title, "state": item.status,
                    "original_estimate": est.map(duration), "original_estimate_seconds": est,
                    "completed_work": done.map(duration), "completed_work_seconds": done,
                    "remaining_work": left.map(duration), "remaining_work_seconds": left,
                    "tracked": duration(tracked), "tracked_seconds": tracked,
                    "status": v,
                }));
            }
            Err(e) => {
                rows.push(vec![format!("#{id}"), "–".into(), "–".into(), "–".into(), duration(tracked), "–".into(), format!("Couldn't be read: {e}")]);
                data.push(json!({ "work_item": format!("#{id}"), "tracked": duration(tracked), "tracked_seconds": tracked, "error": e }));
            }
        }
    }
    let table = facts::Table {
        title: "Azure DevOps progress".into(),
        columns: ["Work item", "State", "Estimate", "Completed (Azure)", "Tracked (DevTrack)", "Remaining", "Status"].map(String::from).to_vec(),
        rows,
    };
    (table, data, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::RemoteUpdate;
    use crate::model::{TimeEntry, SOURCE_MANUAL};

    fn db_with_task(title: &str, source: &str, external: Option<&str>, secs: i64) -> Db {
        let mut db = Db::default();
        let t = Task { id: "t1".into(), title: title.into(), source: source.into(), external_id: external.map(String::from), ..Default::default() };
        db.tasks.insert(t.id.clone(), t);
        let e = TimeEntry { id: "e1".into(), task_id: Some("t1".into()), start: 1_800_000_000, stop: Some(1_800_000_000 + secs), ..Default::default() };
        db.entries.insert(e.id.clone(), e);
        db
    }

    fn fake(_: &Db, id: &str, n: usize) -> Result<(RemoteItem, RemoteDetail), String> {
        let item = RemoteItem { id: id.into(), title: "Modulo C01+C03".into(), status: "Active".into(), kind: "Task".into(), project: "TurismoFVG".into(), url: "https://dev.azure.com/x".into(), assigned_to: "Luca".into(), description: String::new(), estimate: None };
        let updates = vec![RemoteUpdate { when: Some(1_800_000_000), by: "Luca".into(), changes: vec![("State".into(), "New".into(), "Active".into())] }; n.min(1)];
        Ok((item, RemoteDetail { remaining: Some(4.0), completed: Some(3.0), estimate: Some(8.0), updates }))
    }

    #[test]
    fn numbers_and_matching() {
        assert_eq!(number("#28885").as_deref(), Some("28885"));
        assert_eq!(number("28885 – Modulo").as_deref(), Some("28885"));
        assert_eq!(number("Meeting"), None);
        let db = db_with_task("28885 – Modulo C01+C03", SOURCE_MANUAL, None, 3600);
        assert_eq!(task_for(&db, "28885").map(|(t, how)| (t.id.as_str(), how)), Some(("t1", "title")));
        let db = db_with_task("Modulo", SOURCE_AZURE, Some("28885"), 3600);
        assert_eq!(task_for(&db, "28885").map(|(_, how)| how), Some("imported"));
        assert!(task_for(&db, "999").is_none());
    }

    #[test]
    fn a_work_item_comes_with_devtrack_time_and_the_difference() {
        let db = db_with_task("28885 – Modulo C01+C03", SOURCE_MANUAL, None, 4 * 3600 + 18 * 60);
        let v = work_item(&db, 1_800_100_000, "#28885", 5, &fake).unwrap();
        assert_eq!(v["work_item"], "#28885");
        assert_eq!(v["azure_work"]["completed_work"], "3 h");
        assert_eq!(v["azure_work"]["remaining_work"], "4 h");
        assert_eq!(v["in_devtrack"]["tracked"], "4 h 18 min");
        assert_eq!(v["comparison"]["devtrack_minus_completed_seconds"], 78 * 60);
        assert!(v["comparison"]["says"].as_str().unwrap().contains("1 h 18 min more"));
        assert_eq!(v["latest_changes"][0]["changes"][0]["to"], "Active");
        assert!(work_item(&db, 0, "Meeting", 5, &fake).is_err());
    }

    #[test]
    fn the_assigned_list_says_what_devtrack_has() {
        let dir = std::env::temp_dir().join(format!("devtrack-assigned-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(assigned(&dir, &Db::default(), 0, None, None, false).is_err(), "no list yet");
        std::fs::write(
            dir.join("azure-assigned.json"),
            r#"{"cfg":"c","at":1800000000,"items":[{"id":"28885","title":"Modulo C01+C03","status":"Active","kind":"Task","project":"TurismoFVG","url":"u","estimate":8.0},{"id":"28999","title":"Footer","status":"New","kind":"Task","project":"TurismoFVG","url":"u","estimate":null}],"extra":[]}"#,
        )
        .unwrap();
        let db = db_with_task("28885 – Modulo C01+C03", SOURCE_MANUAL, None, 3600);
        let all = assigned(&dir, &db, 1_800_000_600, None, None, false).unwrap();
        assert_eq!(all["count"], 2);
        assert_eq!(all["list_read"]["minutes_ago"], 10);
        assert_eq!(all["work_items"][0]["original_estimate"], "8 h");
        assert_eq!(all["work_items"][0]["in_devtrack"]["tracked"], "1 h");
        assert!(all["work_items"][1]["in_devtrack"].is_null());
        let untracked = assigned(&dir, &db, 1_800_000_600, None, None, true).unwrap();
        assert_eq!(untracked["count"], 1);
        assert_eq!(untracked["work_items"][0]["work_item"], "#28999");
        assert_eq!(assigned(&dir, &db, 0, Some("#28885"), None, false).unwrap()["count"], 1);
        assert_eq!(assigned(&dir, &db, 0, None, Some("new"), false).unwrap()["count"], 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verdicts_say_where_a_work_item_stands() {
        let h = 3600;
        assert_eq!(verdict(Some(3 * h), Some(4 * h), 4 * h + 18 * 60, "Closed"), "! Over the estimate by 1 h 18 min");
        assert_eq!(verdict(Some(4 * h), Some(2 * h), 3 * h + 30 * 60, "Active"), "! 1 h 30 min not reported in Azure yet");
        assert_eq!(verdict(Some(6 * h), Some(3 * h), 3 * h + 10 * 60, "Active"), "On track (53%)");
        assert_eq!(verdict(Some(9 * h), Some(3 * h + 30 * 60), 3 * h + 30 * 60, "Closed"), "Done within the estimate (39%)");
        assert_eq!(verdict(Some(6 * h), Some(4 * h), 2 * h, "Active"), "Azure has 2 h more than DevTrack");
        assert_eq!(verdict(None, None, h, "New"), "No estimate");
    }

    #[test]
    fn the_progress_table_lists_the_work_items_of_the_period() {
        let db = db_with_task("28885 – Modulo C01+C03", SOURCE_MANUAL, None, 4 * 3600 + 18 * 60);
        let p = crate::report::Period { kind: crate::report::PeriodKind::Day, start: crate::report::local_date(1_800_000_000), end: crate::report::local_date(1_800_000_000) + chrono::Duration::days(1) };
        assert_eq!(work_items_in(&db, &p, 1_800_100_000, &facts::Filter::default()), vec!["28885".to_string()]);
        let (t, data, _) = progress_table(&db, &p, 1_800_100_000, &facts::Filter::default(), &fake);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0][0], "#28885 Modulo C01+C03");
        assert_eq!(t.rows[0][4], "4 h 18 min");
        assert_eq!(data[0]["completed_work"], "3 h");
        assert!(facts::markdown(&t).contains("| Work item | State | Estimate | Completed (Azure) |"));
    }
}

