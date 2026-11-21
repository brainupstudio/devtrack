//! Sync between the person's own devices through a folder they already share
//! (OneDrive, iCloud Drive, Dropbox, a network share). Only DevTrack's own data is synced;
//! external systems are never touched.
//!
//! Three separate facts are reported: saved locally, exchanged with the shared folder, and
//! received by each other device (its own file says it has merged our latest changes).

use crate::model::*;
use crate::store::{now_ms, now_secs, write_atomic, Store, APP_NAME};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct DeviceFile {
    format: u32,
    device_id: String,
    device_name: String,
    written_ms: i64,
    /// For every other device: the `written_ms` of its file that this device has merged.
    #[serde(default)]
    seen: BTreeMap<String, i64>,
    db: Db,
}

#[derive(Clone, Debug)]
pub struct DeviceStatus {
    pub name: String,
    pub last_seen_ms: i64,
    /// It has merged everything this device changed up to our last write with changes.
    pub has_our_changes: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SyncOutcome {
    pub at_ms: i64,
    /// Records taken from other devices.
    pub received: usize,
    /// Records this device changed since the last sync that another device changed later.
    pub conflicts: usize,
    /// Extra running timers stopped to keep one active timer.
    pub timers_stopped: usize,
    pub devices: Vec<DeviceStatus>,
    /// Plain-language events worth reviewing (conflicts, timer adjustments).
    pub events: Vec<String>,
}

pub fn sync_dir(folder: &Path) -> PathBuf {
    folder.join(APP_NAME)
}

struct Ctx<'a> {
    me: &'a str,
    last_sync_ms: i64,
    out: &'a mut SyncOutcome,
    newest: Rev,
    from: &'a str,
}

fn merge_map<R: Record>(local: &mut BTreeMap<Id, R>, incoming: &BTreeMap<Id, R>, cx: &mut Ctx, what: &str, describe: &dyn Fn(&R) -> String) {
    for (id, theirs) in incoming {
        if theirs.rev() > &cx.newest {
            cx.newest = theirs.rev().clone();
        }
        match local.get(id) {
            Some(mine) if mine.rev() >= theirs.rev() => {}
            Some(mine) => {
                if mine.rev().device == cx.me && mine.rev().at_ms > cx.last_sync_ms && theirs.rev().device != cx.me {
                    cx.out.conflicts += 1;
                    cx.out.events.push(format!(
                        "Edited here and on {}: kept the newer version of {} “{}”",
                        cx.from,
                        what,
                        describe(theirs)
                    ));
                }
                local.insert(id.clone(), theirs.clone());
                cx.out.received += 1;
            }
            None => {
                local.insert(id.clone(), theirs.clone());
                cx.out.received += 1;
            }
        }
    }
}

/// Merge another device's data into ours (newest revision wins per record).
pub fn merge(store: &mut Store, other: &Db, from: &str, out: &mut SyncOutcome) {
    let me = store.local.device_id.clone();
    let mut cx = Ctx { me: &me, last_sync_ms: store.local.last_sync_ms, out, newest: Rev::default(), from };
    let db = &mut store.db;
    merge_map(&mut db.clients, &other.clients, &mut cx, "client", &|c: &Client| c.name.clone());
    merge_map(&mut db.projects, &other.projects, &mut cx, "project", &|p: &Project| p.name.clone());
    merge_map(&mut db.tasks, &other.tasks, &mut cx, "task", &|t: &Task| t.title.clone());
    merge_map(&mut db.entries, &other.entries, &mut cx, "time entry", &|e: &TimeEntry| {
        if e.description.is_empty() { "(no description)".into() } else { e.description.clone() }
    });
    merge_map(&mut db.profiles, &other.profiles, &mut cx, "profile", &|p: &TimesheetProfile| p.name.clone());
    merge_map(&mut db.marks, &other.marks, &mut cx, "report confirmation", &|m: &ReportedMark| m.date_label.clone());
    merge_map(&mut db.integrations, &other.integrations, &mut cx, "connection", &|i: &IntegrationConfig| i.name.clone());
    if other.settings.rev > db.settings.rev {
        db.settings = other.settings.clone();
        cx.out.received += 1;
    }
    let newest = cx.newest.clone();
    store.observe(&newest);
    // a project written by an older version carries only the client text: map it back
    store.migrate_clients();
}

/// One active timer: the latest started keeps running; the others stop where it started.
pub fn enforce_single_timer(store: &mut Store) -> Vec<String> {
    let mut running: Vec<TimeEntry> = store.db.entries.values().filter(|e| e.is_running()).cloned().collect();
    if running.len() < 2 {
        return vec![];
    }
    running.sort_by(|a, b| b.start.cmp(&a.start).then(b.rev.cmp(&a.rev)));
    let winner = running[0].clone();
    let now = now_secs();
    let mut events = vec![];
    for mut e in running.into_iter().skip(1) {
        e.stop = Some(winner.start.min(now).max(e.start));
        events.push(format!(
            "Two timers were running: “{}” was stopped where “{}” started",
            if e.description.is_empty() { "(no description)" } else { &e.description },
            if winner.description.is_empty() { "(no description)" } else { &winner.description }
        ));
        store.put_entry(e);
    }
    events
}

fn read_files(dir: &Path, me: &str, out: &mut BTreeMap<String, DeviceFile>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&p) else { continue };
        // A half-written or foreign file is skipped, never fatal.
        let Ok(f) = serde_json::from_slice::<DeviceFile>(&bytes) else { continue };
        if f.device_id == me || f.format > FORMAT {
            continue;
        }
        let newer = out.get(&f.device_id).map(|o| f.written_ms > o.written_ms).unwrap_or(true);
        if newer {
            out.insert(f.device_id.clone(), f);
        }
    }
}

/// Read every other device's file, merge, then write ours. Errors mean "offline": the
/// local data is safe and the next run catches up.
pub fn run(store: &mut Store, seen: &mut BTreeMap<String, i64>, last_written_gen: &mut u64) -> Result<SyncOutcome, String> {
    if store.blocked.is_some() {
        return Err("Sync paused until the local data file is recovered".into());
    }
    let folder = store.local.sync_folder.clone().ok_or("No shared folder chosen")?;
    if !folder.is_dir() {
        return Err(format!("Shared folder not reachable: {}", folder.display()));
    }
    let dir = sync_dir(&folder);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut out = SyncOutcome { at_ms: now_ms(), ..Default::default() };
    let me = store.local.device_id.clone();
    let local_changes = store.generation != *last_written_gen;
    let mut files = BTreeMap::new();
    read_files(&dir, &me, &mut files);
    for f in files.values() {
        merge(store, &f.db, &f.device_name, &mut out);
        seen.insert(f.device_id.clone(), f.written_ms);
    }
    if out.received > 0 {
        store.generation += 1;
        store.save();
    }
    let ev = enforce_single_timer(store);
    out.timers_stopped = ev.len();
    out.events.extend(ev);
    let written = now_ms();
    // Only our own changes (and timer adjustments) need confirming by other devices.
    if local_changes || out.timers_stopped > 0 {
        store.local.last_change_written_ms = written;
    }
    *last_written_gen = store.generation;
    let mine = DeviceFile {
        format: FORMAT,
        device_id: me.clone(),
        device_name: store.local.device_name.clone(),
        written_ms: written,
        seen: seen.clone(),
        db: store.db.clone(),
    };
    let bytes = serde_json::to_vec(&mine).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(format!("{}.json", me)), &bytes).map_err(|e| e.to_string())?;
    for f in files.values() {
        let theirs = f.seen.get(&me).copied().unwrap_or(0);
        out.devices.push(DeviceStatus {
            name: f.device_name.clone(),
            last_seen_ms: f.written_ms,
            has_our_changes: store.local.last_change_written_ms == 0 || theirs >= store.local.last_change_written_ms,
        });
    }
    store.local.last_sync_ms = out.at_ms;
    store.save_local();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dev: &str) -> Store {
        let mut s = Store::in_memory(Db::default());
        s.local.device_id = dev.into();
        s
    }

    #[test]
    fn start_on_one_stop_on_other() {
        let mut a = store("a");
        let mut b = store("b");
        a.start("Write report", None, None, false);
        let mut out = SyncOutcome::default();
        merge(&mut b, &a.db.clone(), "A", &mut out);
        assert!(b.db.running().is_some());
        b.stop();
        let mut out = SyncOutcome::default();
        merge(&mut a, &b.db.clone(), "B", &mut out);
        assert!(a.db.running().is_none());
        assert_eq!(a.db.entries.len(), 1);
    }

    #[test]
    fn two_timers_become_one() {
        let mut a = store("a");
        let mut b = store("b");
        a.start("A", None, None, false);
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.start("B", None, None, false);
        b.update_running(|e| e.start += 10);
        let mut out = SyncOutcome::default();
        merge(&mut a, &b.db.clone(), "B", &mut out);
        assert_eq!(enforce_single_timer(&mut a).len(), 1);
        assert_eq!(a.db.entries.values().filter(|e| e.is_running()).count(), 1);
        assert_eq!(a.db.running().unwrap().description, "B");
    }

    #[test]
    fn deletes_travel() {
        let mut a = store("a");
        let mut b = store("b");
        let id = a.start("X", None, None, false);
        a.stop();
        let mut out = SyncOutcome::default();
        merge(&mut b, &a.db.clone(), "A", &mut out);
        b.delete_entry(&id);
        merge(&mut a, &b.db.clone(), "B", &mut out);
        assert!(a.db.entries[&id].deleted);
    }

    #[test]
    fn concurrent_offline_edits_are_reported() {
        let mut a = store("a");
        let mut b = store("b");
        let id = a.start("X", None, None, false);
        a.stop();
        let mut out = SyncOutcome::default();
        merge(&mut b, &a.db.clone(), "A", &mut out);
        a.local.last_sync_ms = crate::store::now_ms();
        b.local.last_sync_ms = a.local.last_sync_ms;
        std::thread::sleep(std::time::Duration::from_millis(3));
        let mut ea = a.db.entries[&id].clone();
        ea.description = "edited on A".into();
        a.put_entry(ea);
        std::thread::sleep(std::time::Duration::from_millis(3));
        let mut eb = b.db.entries[&id].clone();
        eb.description = "edited on B".into();
        b.put_entry(eb);
        let mut out = SyncOutcome::default();
        merge(&mut a, &b.db.clone(), "B", &mut out);
        assert_eq!(out.conflicts, 1);
        assert_eq!(a.db.entries[&id].description, "edited on B");
        assert!(!out.events.is_empty());
    }

    #[test]
    fn folder_round_trip_and_receipt() {
        let root = std::env::temp_dir().join(format!("devtrack-sync-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&root).unwrap();
        let mut a = store("a");
        let mut b = store("b");
        a.local.sync_folder = Some(root.clone());
        b.local.sync_folder = Some(root.clone());
        a.local.device_name = "A".into();
        b.local.device_name = "B".into();
        let (mut sa, mut sb) = (BTreeMap::new(), BTreeMap::new());
        let (mut ga, mut gb) = (0u64, 0u64);
        a.start("on A", None, None, false);
        run(&mut a, &mut sa, &mut ga).unwrap();
        run(&mut b, &mut sb, &mut gb).unwrap();
        assert!(b.db.running().is_some());
        b.stop(); // stop on the second device
        run(&mut b, &mut sb, &mut gb).unwrap();
        let o = run(&mut a, &mut sa, &mut ga).unwrap();
        assert!(a.db.running().is_none());
        assert!(o.devices[0].has_our_changes);
        // unreachable folder → error, local data intact
        a.local.sync_folder = Some(root.join("missing"));
        assert!(run(&mut a, &mut sa, &mut ga).is_err());
        assert_eq!(a.db.entries.len(), 1);
    }

    #[test]
    fn clients_travel_and_older_writers_are_mapped_back() {
        let mut a = store("a");
        let mut b = store("b");
        a.put_client(Client { id: "c1".into(), name: "Northwind".into(), ..Default::default() });
        a.put_project(Project { id: "p".into(), name: "Portal".into(), client_id: Some("c1".into()), client: "Northwind".into(), ..Default::default() });
        let da = a.db.clone();
        merge(&mut b, &da, "A", &mut SyncOutcome::default());
        assert_eq!(b.db.client_name(&b.db.projects["p"]), "Northwind");
        // an older DevTrack rewrote the project knowing only the client text
        let mut old = b.db.projects["p"].clone();
        old.client_id = None;
        old.name = "Portal 2".into();
        old.rev = Rev { at_ms: i64::MAX / 2, seq: 0, device: "old".into() };
        let mut odb = Db::default();
        odb.projects.insert("p".into(), old);
        merge(&mut a, &odb, "old", &mut SyncOutcome::default());
        assert_eq!(a.db.projects["p"].name, "Portal 2");
        assert_eq!(a.db.projects["p"].client_id, Some("c1".into()), "association restored, no duplicate client");
        assert_eq!(a.db.live_clients().len(), 1);
    }
}
