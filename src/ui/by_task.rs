//! A day's sessions grouped by task: one line per task (with its project) and the day's total on
//! it, for reading and copying the hours per task. Used by Today's list and the compact window.
//!
//! - Sessions of the same task and project go together. A session without a task but with a
//!   note goes with the others of the same project and note; one that still needs details stays
//!   alone, so the grouping never hides what has to be completed.
//! - Groups come largest first; the day's sessions keep their time order inside a group.

use devtrack_core::model::{Db, Id, TimeEntry, TimesheetProfile};
use devtrack_core::report::{convert, needs_completion};

/// One task on one day: its sessions (piece of each on that day: entry, start, end).
#[derive(Clone, Debug)]
pub struct TaskGroup {
    pub key: String,
    pub pieces: Vec<(TimeEntry, i64, i64)>,
    pub total: i64,
    pub first: i64,
    pub last: i64,
    pub running: bool,
}

impl TaskGroup {
    /// The session that stands for the group: the latest one (Continue, title, project).
    pub fn latest(&self) -> &TimeEntry {
        &self.pieces.iter().max_by_key(|p| p.1).expect("a group has a session").0
    }
}

fn key_of(e: &TimeEntry) -> String {
    let project = e.project_id.clone().unwrap_or_default();
    match &e.task_id {
        Some(t) => format!("t:{t}|p:{project}"),
        None if needs_completion(e) => format!("e:{}", e.id),
        None => format!("n:{}|p:{project}", e.description.trim().to_lowercase()),
    }
}

/// The groups of one day's pieces, largest first.
pub fn group(pieces: &[(TimeEntry, i64, i64)]) -> Vec<TaskGroup> {
    let mut out: Vec<TaskGroup> = vec![];
    let mut sorted: Vec<&(TimeEntry, i64, i64)> = pieces.iter().collect();
    sorted.sort_by_key(|p| p.1);
    for p in sorted {
        let key = key_of(&p.0);
        match out.iter_mut().find(|g| g.key == key) {
            Some(g) => {
                g.total += p.2 - p.1;
                g.first = g.first.min(p.1);
                g.last = g.last.max(p.2);
                g.running |= p.0.is_running();
                g.pieces.push(p.clone());
            }
            None => out.push(TaskGroup { key, total: p.2 - p.1, first: p.1, last: p.2, running: p.0.is_running(), pieces: vec![p.clone()] }),
        }
    }
    out.sort_by(|a, b| b.total.cmp(&a.total).then(a.first.cmp(&b.first)));
    out
}

/// The day's totals per task, ready to paste into a spreadsheet: a header, then one line per
/// task with its project, client, duration (h:mm) and the value for each platform.
pub fn day_text(db: &Db, groups: &[TaskGroup], profiles: &[TimesheetProfile], title: impl Fn(&TimeEntry) -> String) -> String {
    let clean = |s: String| s.replace(['\t', '\n'], " ");
    let mut head = vec![super::i18n::tr("Task"), super::i18n::tr("Project"), super::i18n::tr("Client"), super::i18n::tr("Duration")];
    head.extend(profiles.iter().map(|p| p.name.clone()));
    let mut lines = vec![head.join("\t")];
    for g in groups {
        let e = g.latest();
        let project = db.project(&e.project_id);
        let mut cells = vec![
            clean(title(e)),
            project.map(|p| clean(p.name.clone())).unwrap_or_default(),
            project.map(|p| clean(db.client_name(p))).unwrap_or_default(),
            devtrack_core::timefmt::format_report(g.total, devtrack_core::model::ReportFormat::HoursMinutes, 2, false),
        ];
        cells.extend(profiles.iter().map(|p| convert(p, g.total)));
        lines.push(cells.join("\t"));
    }
    let total: i64 = groups.iter().map(|g| g.total).sum();
    let mut last = vec![super::i18n::tr("Total"), String::new(), String::new(), devtrack_core::timefmt::format_report(total, devtrack_core::model::ReportFormat::HoursMinutes, 2, false)];
    last.extend(profiles.iter().map(|p| convert(p, total)));
    lines.push(last.join("\t"));
    lines.join("\n") + "\n"
}

/// Stable id of a group on a day, for its expanded state.
pub fn group_id(day: chrono::NaiveDate, g: &TaskGroup) -> Id {
    format!("{}|{}", day, g.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, task: Option<&str>, project: Option<&str>, desc: &str, start: i64, secs: i64) -> (TimeEntry, i64, i64) {
        let e = TimeEntry { id: id.into(), task_id: task.map(String::from), project_id: project.map(String::from), description: desc.into(), start, stop: Some(start + secs), ..Default::default() };
        (e, start, start + secs)
    }

    #[test]
    fn sessions_of_one_task_add_up_and_the_largest_comes_first() {
        let day = vec![
            entry("a", Some("t1"), Some("p"), "", 1000, 600),
            entry("b", Some("t2"), Some("p"), "", 2000, 3600),
            entry("c", Some("t1"), Some("p"), "", 9000, 1200),
            entry("d", Some("t1"), Some("p"), "", 5000, 300),
        ];
        let g = group(&day);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].pieces.len(), 1, "t2 has the most time");
        assert_eq!(g[1].total, 2100);
        assert_eq!(g[1].pieces.iter().map(|p| p.0.id.as_str()).collect::<Vec<_>>(), vec!["a", "d", "c"], "time order inside");
        assert_eq!((g[1].first, g[1].last), (1000, 10200));
        assert_eq!(g[1].latest().id, "c");
    }

    #[test]
    fn same_task_in_two_projects_stays_apart_and_incomplete_sessions_stay_alone() {
        let day = vec![
            entry("a", Some("t1"), Some("p1"), "", 0, 60),
            entry("b", Some("t1"), Some("p2"), "", 100, 60),
            entry("c", None, None, "", 200, 60),
            entry("d", None, None, "", 300, 60),
            entry("e", None, Some("p1"), "Call", 400, 60),
            entry("f", None, Some("p1"), "call ", 500, 60),
        ];
        let g = group(&day);
        assert_eq!(g.len(), 5, "t1/p1, t1/p2, c, d, call/p1");
        assert!(g.iter().any(|x| x.pieces.len() == 2 && x.pieces[0].0.description.trim().eq_ignore_ascii_case("call")));
    }

    #[test]
    fn the_day_copies_as_tab_separated_lines_with_a_total() {
        let mut db = Db::default();
        db.projects.insert("p".into(), devtrack_core::model::Project { id: "p".into(), name: "Portal".into(), ..Default::default() });
        let day = vec![entry("a", Some("t1"), Some("p"), "", 0, 5400), entry("b", Some("t1"), Some("p"), "", 9000, 1800)];
        let g = group(&day);
        let text = day_text(&db, &g, &[], |_| "Login".into());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "Login\tPortal\t\t2:00");
        assert!(lines[2].ends_with("\t2:00"));
    }
}
