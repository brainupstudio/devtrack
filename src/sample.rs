//! Sample content for previews, review pictures and "Load sample data". Names are invented.

use crate::model::*;
use crate::report::{local_time_of, today};
use chrono::{Datelike, Duration, Weekday};

/// The four states asked for in the review pictures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoState {
    /// Nothing yet.
    First,
    /// A session is running, the day is classified.
    Active,
    /// Some sessions lack a description or project; one crosses midnight; two overlap.
    Incomplete,
    /// Yesterday fully classified and mapped, part of it confirmed.
    Ready,
    /// A wider, mixed data set: five clients, twelve projects, work items from two Azure DevOps
    /// organizations (one number in both) and local tasks, with two conversions (Azure DevOps
    /// in decimal hours, Zucchetti in hours:minutes).
    Varied,
}

impl DemoState {
    pub fn parse(s: &str) -> Option<DemoState> {
        match s {
            "first" => Some(DemoState::First),
            "active" => Some(DemoState::Active),
            "incomplete" => Some(DemoState::Incomplete),
            "ready" => Some(DemoState::Ready),
            "varied" | "seed" => Some(DemoState::Varied),
            _ => None,
        }
    }
}

pub fn build(now: i64) -> Db {
    build_state(now, DemoState::Ready, false)
}

pub fn build_state(now: i64, state: DemoState, running: bool) -> Db {
    let mut db = Db::default();
    if state == DemoState::First {
        return db;
    }
    if state == DemoState::Varied {
        return build_varied(now);
    }
    db.schema = 2;
    for (id, name) in [("c-northwind", "Northwind Traders"), ("c-fabrikam", "Fabrikam")] {
        db.clients.insert(id.into(), Client { id: id.into(), name: name.into(), ..Default::default() });
    }
    let projects = [
        ("p-portal", "Client portal", "#0B83D9", "c-northwind", false),
        ("p-tools", "Internal tooling", "#9E5BD9", "", false),
        ("p-admin", "Meetings & admin", "#D94182", "", false),
        ("p-migr", "Data migration", "#2DA608", "c-northwind", false),
        ("p-brand", "Brand refresh", "#E36A00", "c-fabrikam", false),
        ("p-legacy", "Legacy website", "#525266", "c-fabrikam", true),
    ];
    for (id, name, color, client, archived) in projects {
        let client_id = if client.is_empty() { None } else { Some(client.to_string()) };
        let client_name = client_id.as_ref().map(|c| db.clients[c].name.clone()).unwrap_or_default();
        db.projects.insert(id.into(), Project { id: id.into(), name: name.into(), color: color.into(), client_id, client: client_name, archived, ..Default::default() });
    }
    let org = "contoso";
    let ado = |n: &str, title: &str, state: &str, kind: &str, proj: &str, local: &str| Task {
        id: Task::external_key(SOURCE_AZURE, org, n),
        project_id: Some(local.into()),
        title: title.into(),
        description: format!("Sample work item {}.", n),
        status: state.into(),
        source: SOURCE_AZURE.into(),
        external_id: Some(n.into()),
        external_project: Some(proj.into()),
        external_type: Some(kind.into()),
        url: Some(format!("https://dev.azure.com/{}/{}/_workitems/edit/{}", org, proj, n)),
        ..Default::default()
    };
    // Client portal holds items from two Azure DevOps projects (Portal and Billing).
    let tasks = vec![
        ado("4812", "Login page times out on slow networks", "Active", "Bug", "Portal", "p-portal"),
        ado("4820", "Export invoices to CSV", "Active", "User Story", "Billing", "p-portal"),
        ado("5103", "Migrate customer table", "New", "Task", "Platform", "p-migr"),
        ado("5110", "Validate migrated addresses", "Active", "Task", "Platform", "p-migr"),
        Task { id: "t-plan".into(), project_id: Some("p-admin".into()), title: "Weekly planning".into(), status: "Open".into(), source: SOURCE_MANUAL.into(), ..Default::default() },
        Task { id: "t-review".into(), project_id: Some("p-tools".into()), title: "Code review".into(), status: "Open".into(), source: SOURCE_MANUAL.into(), ..Default::default() },
    ];
    for t in tasks {
        db.tasks.insert(t.id.clone(), t);
    }
    let k = |n: &str| Some(Task::external_key(SOURCE_AZURE, org, n));
    // (start minute of day, minutes, description, project, task, billable)
    let day_plan: Vec<(i64, i64, &str, &str, Option<String>, bool)> = vec![
        (9 * 60, 20, "Stand-up", "p-admin", Some("t-plan".into()), false),
        (9 * 60 + 30, 98, "Reproduce timeout", "p-portal", k("4812"), true),
        (11 * 60 + 20, 48, "Review pull requests", "p-tools", Some("t-review".into()), false),
        (14 * 60, 126, "CSV export endpoint", "p-portal", k("4820"), true),
        (16 * 60 + 20, 71, "Migration script", "p-migr", k("5103"), true),
    ];
    let t0 = today();
    let mut n = 0;
    let mut add = |db: &mut Db, start: i64, stop: Option<i64>, desc: &str, pid: Option<&str>, tid: Option<String>, bill: bool| {
        n += 1;
        let id = format!("sample-{}", n);
        db.entries.insert(
            id.clone(),
            TimeEntry { id, description: desc.to_string(), project_id: pid.map(|s| s.to_string()), task_id: tid, start, stop, billable: bill, ..Default::default() },
        );
    };
    for back in (0..10).rev() {
        let d = t0 - Duration::days(back);
        if matches!(d.weekday(), Weekday::Sat | Weekday::Sun) && back != 0 {
            continue;
        }
        for (i, (sm, mins, desc, pid, tid, bill)) in day_plan.iter().enumerate() {
            // today: the morning only (the Incomplete state adds unclassified sessions after it)
            if back == 0 && i >= if state == DemoState::Incomplete { 2 } else { 3 } {
                break;
            }
            let wobble = ((back * 7 + i as i64 * 13) % 17) - 8;
            let start = local_time_of(d, sm * 60 + (back % 3) * 60);
            let stop = start + (mins + wobble) * 60 + (back * 11 + i as i64 * 7) % 60;
            if start > now {
                continue;
            }
            add(&mut db, start, Some(stop.min(now)), desc, Some(pid), tid.clone(), *bill);
        }
    }
    if state == DemoState::Incomplete {
        // a late session that crosses midnight into today
        let y = t0 - Duration::days(1);
        let s = local_time_of(y, 23 * 3600 + 30 * 60);
        add(&mut db, s, Some(s + 3600), "Hotfix deploy", Some("p-portal"), k("4812"), true);
        // quick starts left unclassified today
        let s1 = local_time_of(t0, 11 * 3600 + 25 * 60);
        if s1 + 22 * 60 < now - 40 * 60 {
            add(&mut db, s1, Some(s1 + 22 * 60), "", None, None, false);
        }
        let s2 = local_time_of(t0, 11 * 3600 + 55 * 60);
        if s2 + 35 * 60 < now - 40 * 60 {
            add(&mut db, s2, Some(s2 + 35 * 60), "Call with Northwind", None, None, false);
        }
    }
    if running {
        let start = now - (38 * 60 + 12);
        add(&mut db, start, None, "Fix session refresh", Some("p-portal"), k("4812"), true);
    }
    let mut a = TimesheetProfile::new("Company timesheet", "Zucchetti (example setup)");
    a.id = "profile-a".into();
    a.format = ReportFormat::DecimalHours;
    a.decimal_comma = true;
    a.round_scope = RoundScope::PerGroup;
    a.round_step_min = 15;
    a.round_dir = RoundDir::Up;
    a.default_code = String::new();
    a.mappings = vec![
        CodeMapping { project_id: Some("p-portal".into()), task_id: None, code: "NW-2024-017".into(), activity: "Development".into() },
        CodeMapping { project_id: Some("p-portal".into()), task_id: k("4820"), code: "NW-2024-019".into(), activity: "Billing change".into() },
        CodeMapping { project_id: Some("p-migr".into()), task_id: None, code: "NW-2024-022".into(), activity: "Data services".into() },
        CodeMapping { project_id: Some("p-admin".into()), task_id: None, code: "INT-ADM".into(), activity: "Internal".into() },
        CodeMapping { project_id: Some("p-tools".into()), task_id: None, code: "INT-DEV".into(), activity: "Tooling".into() },
    ];
    db.profiles.insert(a.id.clone(), a);
    let mut b = TimesheetProfile::new("Client hours log", "Customer portal (example)");
    b.id = "profile-b".into();
    b.format = ReportFormat::HoursMinutes;
    b.grouping = Grouping::DayProject;
    b.round_scope = RoundScope::None;
    b.mappings = vec![
        CodeMapping { project_id: Some("p-portal".into()), task_id: None, code: "NW-PORTAL".into(), activity: String::new() },
        CodeMapping { project_id: Some("p-migr".into()), task_id: None, code: "NW-DATA".into(), activity: String::new() },
    ];
    b.default_code = "NW-OTHER".into();
    db.profiles.insert(b.id.clone(), b);
    if state == DemoState::Ready {
        // two of the last working day's lines already confirmed in profile A
        let y = last_working_day();
        let p = crate::report::Period::containing(crate::report::PeriodKind::Day, y, true);
        let prof = db.profiles["profile-a"].clone();
        let lines = crate::report::timesheet(&db, &prof, &p, now);
        for l in lines.iter().take(2) {
            if let Some(m) = crate::report::confirm(l, &prof, now) {
                db.marks.insert(m.id.clone(), m);
            }
        }
    }
    db
}

pub fn last_working_day() -> chrono::NaiveDate {
    let mut y = today() - Duration::days(1);
    while matches!(y.weekday(), Weekday::Sat | Weekday::Sun) {
        y -= Duration::days(1);
    }
    y
}

/// The mixed data set behind `--state varied`. Names are invented.
fn build_varied(now: i64) -> Db {
    let mut db = Db::default();
    db.schema = 2;
    for (id, name) in [("c-aurora", "Banca Aurora"), ("c-comune", "Comune di Vallefiorita"), ("c-ferri", "Studio Legale Ferri"), ("c-moda", "Moda Lineare")] {
        db.clients.insert(id.into(), Client { id: id.into(), name: name.into(), ..Default::default() });
    }
    let projects = [
        ("p-hb-app", "Home banking · app mobile", "#0B83D9", "c-aurora", false),
        ("p-hb-web", "Home banking · portale web", "#2D7FF9", "c-aurora", false),
        ("p-core", "Migrazione core", "#2DA608", "c-aurora", false),
        ("p-cittadino", "Portale servizi al cittadino", "#E36A00", "c-comune", false),
        ("p-sito", "Manutenzione sito", "#C7741A", "c-comune", false),
        ("p-ferri", "Gestionale pratiche", "#9E5BD9", "c-ferri", false),
        ("p-ecom", "E-commerce", "#D94182", "c-moda", false),
        ("p-campagna", "Campagna autunno", "#E0478A", "c-moda", false),
        ("p-vecchio", "Vecchio sito", "#525266", "c-moda", true),
        ("p-riunioni", "Riunioni interne", "#6B7280", "", false),
        ("p-form", "Formazione", "#0E9F9F", "", false),
        ("p-prev", "Prevendita", "#B45309", "", false),
    ];
    for (id, name, color, client, archived) in projects {
        let client_id = if client.is_empty() { None } else { Some(client.to_string()) };
        let client_name = client_id.as_ref().map(|c| db.clients[c].name.clone()).unwrap_or_default();
        db.projects.insert(id.into(), Project { id: id.into(), name: name.into(), color: color.into(), client_id, client: client_name, archived, billable_default: !client.is_empty(), ..Default::default() });
    }
    let ado = |org: &str, n: &str, title: &str, state: &str, kind: &str, proj: &str, local: &str| Task {
        id: Task::external_key(SOURCE_AZURE, org, n),
        project_id: Some(local.into()),
        title: title.into(),
        status: state.into(),
        source: SOURCE_AZURE.into(),
        external_id: Some(n.into()),
        external_project: Some(proj.into()),
        external_type: Some(kind.into()),
        url: Some(format!("https://dev.azure.com/{}/{}/_workitems/edit/{}", org, proj, n)),
        ..Default::default()
    };
    let local = |id: &str, title: &str, project: &str| Task { id: id.into(), project_id: Some(project.into()), title: title.into(), status: "Open".into(), source: SOURCE_MANUAL.into(), ..Default::default() };
    // #7301 exists in both organizations: the same number, two different work items
    let tasks = vec![
        ado("aurora", "7301", "Login biometrico", "Active", "User Story", "HomeBanking", "p-hb-app"),
        ado("aurora", "7315", "Notifiche push per i bonifici", "New", "User Story", "HomeBanking", "p-hb-app"),
        ado("aurora", "7340", "Estratto conto in PDF", "Active", "Task", "HomeBanking", "p-hb-web"),
        ado("aurora", "7352", "Errore con sessione scaduta", "Active", "Bug", "HomeBanking", "p-hb-web"),
        ado("aurora", "7401", "Migrazione delle anagrafiche", "Active", "Task", "Core", "p-core"),
        ado("aurora", "7402", "Verifica dei saldi migrati", "New", "Task", "Core", "p-core"),
        ado("vallefiorita", "7301", "Prenotazione degli appuntamenti", "Active", "User Story", "Servizi", "p-cittadino"),
        ado("vallefiorita", "812", "Pagamento dei tributi online", "Active", "User Story", "Servizi", "p-cittadino"),
        local("t-plugin", "Aggiornamento dei plugin", "p-sito"),
        local("t-import", "Importazione delle pratiche", "p-ferri"),
        local("t-checkout", "Checkout", "p-ecom"),
        local("t-scheda", "Scheda prodotto", "p-ecom"),
        local("t-news", "Newsletter", "p-campagna"),
        local("t-weekly", "Riunione giornaliera", "p-riunioni"),
        local("t-rust", "Corso Rust", "p-form"),
    ];
    for t in tasks {
        db.tasks.insert(t.id.clone(), t);
    }
    let k = |org: &str, n: &str| Some(Task::external_key(SOURCE_AZURE, org, n));
    let l = |id: &str| Some(id.to_string());
    // (start minute of day, minutes, description, project, task)
    type Plan = Vec<(i64, i64, &'static str, &'static str, Option<String>)>;
    let day_a: Plan = vec![
        (9 * 60, 15, "Riunione giornaliera", "p-riunioni", l("t-weekly")),
        (9 * 60 + 20, 85, "Login biometrico: flusso iOS", "p-hb-app", k("aurora", "7301")),
        (10 * 60 + 50, 40, "Errore con sessione scaduta", "p-hb-web", k("aurora", "7352")),
        (11 * 60 + 35, 50, "Estratto conto: impaginazione", "p-hb-web", k("aurora", "7340")),
        // the same work item again after lunch and at the end of the day
        (12 * 60 + 30, 25, "Login biometrico: test su Android", "p-hb-app", k("aurora", "7301")),
        (14 * 60, 70, "Appuntamenti: calendario", "p-cittadino", k("vallefiorita", "7301")),
        (15 * 60 + 15, 35, "Aggiornamento dei plugin", "p-sito", l("t-plugin")),
        (15 * 60 + 55, 55, "Checkout: metodi di pagamento", "p-ecom", l("t-checkout")),
        (17 * 60, 30, "Corso Rust, capitolo 8", "p-form", l("t-rust")),
        (17 * 60 + 40, 40, "Login biometrico: correzioni dalla revisione", "p-hb-app", k("aurora", "7301")),
        (18 * 60 + 30, 20, "Checkout: verifica dei pagamenti", "p-ecom", l("t-checkout")),
    ];
    let day_b: Plan = vec![
        (9 * 60, 15, "Riunione giornaliera", "p-riunioni", l("t-weekly")),
        (9 * 60 + 20, 110, "Migrazione delle anagrafiche", "p-core", k("aurora", "7401")),
        (11 * 60 + 15, 45, "Verifica dei saldi migrati", "p-core", k("aurora", "7402")),
        (12 * 60 + 5, 25, "Notifiche push: prototipo", "p-hb-app", k("aurora", "7315")),
        (13 * 60 + 30, 20, "Migrazione delle anagrafiche: controllo dei duplicati", "p-core", k("aurora", "7401")),
        (14 * 60, 60, "Importazione delle pratiche", "p-ferri", l("t-import")),
        (15 * 60 + 5, 40, "Tributi: pagina di riepilogo", "p-cittadino", k("vallefiorita", "812")),
        (15 * 60 + 50, 45, "Newsletter d'autunno", "p-campagna", l("t-news")),
        (16 * 60 + 40, 35, "Demo per un nuovo cliente", "p-prev", None),
        (17 * 60 + 20, 20, "Scheda prodotto: varianti", "p-ecom", l("t-scheda")),
        (17 * 60 + 50, 35, "Migrazione delle anagrafiche: seconda tranche", "p-core", k("aurora", "7401")),
        (18 * 60 + 35, 15, "Notifiche push: revisione con il team", "p-hb-app", k("aurora", "7315")),
    ];
    let t0 = today();
    let mut n = 0;
    for back in (0..15).rev() {
        let d = t0 - Duration::days(back);
        if matches!(d.weekday(), Weekday::Sat | Weekday::Sun) && back != 0 {
            continue;
        }
        let plan = if d.ordinal() % 2 == 0 { &day_a } else { &day_b };
        for (i, (sm, mins, desc, pid, tid)) in plan.iter().enumerate() {
            let wobble = ((back * 7 + i as i64 * 13) % 17) - 8;
            let start = local_time_of(d, sm * 60);
            let stop = start + (mins + wobble).max(5) * 60 + (back * 11 + i as i64 * 7) % 60;
            if stop > now {
                continue;
            }
            n += 1;
            let id = format!("seed-{}", n);
            let billable = db.projects[*pid].billable_default;
            let tags = varied_tags(desc, back, i);
            db.entries.insert(id.clone(), TimeEntry { id, description: desc.to_string(), project_id: Some(pid.to_string()), task_id: tid.clone(), start, stop: Some(stop), billable, tags, ..Default::default() });
        }
    }
    // two tags in the list that no session uses yet
    for name in ["Da rivedere", "Trasferta"] {
        let id = tag_id_for(name);
        db.tags.insert(id.clone(), Tag { id, name: name.into(), ..Default::default() });
    }
    // one unclassified call on the last working day
    let y = last_working_day();
    let s = local_time_of(y, 13 * 3600 + 10 * 60);
    db.entries.insert("seed-call".into(), TimeEntry { id: "seed-call".into(), description: "Telefonata".into(), start: s, stop: Some(s + 12 * 60), ..Default::default() });

    let map = |project: &str, code: &str, activity: &str| CodeMapping { project_id: Some(project.into()), task_id: None, code: code.into(), activity: activity.into() };
    // The same job codes in the first two destinations: several projects share a job.
    // "Prevendita" has no code and there is no fallback, so its line asks for one.
    let codes = vec![
        map("p-hb-app", "COM-2026-014", ""),
        map("p-hb-web", "COM-2026-014", ""),
        map("p-core", "COM-2026-021", ""),
        map("p-cittadino", "COM-2026-030", ""),
        map("p-sito", "COM-2026-030", ""),
        map("p-ferri", "COM-2026-044", ""),
        map("p-ecom", "COM-2026-051", ""),
        map("p-campagna", "COM-2026-051", ""),
        map("p-riunioni", "INT-000", ""),
        map("p-form", "INT-000", ""),
    ];
    // Two conversions, as in Settings › Conversions: both round to the closest 15 minutes.
    // In Report Time the first gathers by job code, the second keeps one line per project.
    let mut a = TimesheetProfile::conversion("DevOps");
    a.id = "seed-a".into();
    a.grouping = Grouping::DayCode;
    a.mappings = codes.clone();
    db.profiles.insert(a.id.clone(), a);
    let mut b = TimesheetProfile::conversion("Zucchetti");
    b.id = "seed-b".into();
    b.format = ReportFormat::HoursMinutes;
    b.grouping = Grouping::DayProject;
    b.mappings = codes;
    db.profiles.insert(b.id.clone(), b);
    db
}

/// Tags for a sample session. Some recur across clients and projects ("sviluppo", "bug",
/// "riunione", "analisi", "code review", "supporto"); others belong to one project only ("iOS",
/// "PDF", "SEO"). A few are written with another capital, as typed by hand on other devices: the
/// list keeps one spelling of each.
fn varied_tags(desc: &str, back: i64, i: usize) -> Vec<String> {
    let odd = (back + i as i64) % 2 == 1;
    let mut t: Vec<&str> = match desc {
        "Riunione giornaliera" => vec!["riunione"],
        "Login biometrico: flusso iOS" => vec!["sviluppo", "iOS", "biometria"],
        "Login biometrico: test su Android" => vec!["test", "biometria"],
        "Login biometrico: correzioni dalla revisione" => vec!["sviluppo", "iOS", "biometria"],
        "Checkout: verifica dei pagamenti" => vec!["test", "pagamenti"],
        "Migrazione delle anagrafiche: controllo dei duplicati" => vec!["migrazione dati", "analisi"],
        "Migrazione delle anagrafiche: seconda tranche" => vec!["migrazione dati"],
        "Notifiche push: revisione con il team" => vec!["riunione", "iOS", "notifiche"],
        "Errore con sessione scaduta" => vec![if odd { "Bug" } else { "bug" }, "sicurezza"],
        "Estratto conto: impaginazione" => vec!["sviluppo", "PDF"],
        "Appuntamenti: calendario" => vec!["sviluppo", "analisi"],
        "Aggiornamento dei plugin" => vec!["supporto", "WordPress"],
        "Checkout: metodi di pagamento" => vec!["sviluppo", "pagamenti"],
        "Corso Rust, capitolo 8" => vec!["formazione", "Rust"],
        "Migrazione delle anagrafiche" => vec!["migrazione dati"],
        "Verifica dei saldi migrati" => vec!["migrazione dati", "test"],
        "Notifiche push: prototipo" => vec!["sviluppo", "iOS", "notifiche"],
        "Importazione delle pratiche" => vec!["supporto", "migrazione dati"],
        "Tributi: pagina di riepilogo" => vec!["sviluppo", "pagamenti", "accessibilità"],
        "Newsletter d'autunno" => vec!["contenuti", "newsletter"],
        "Demo per un nuovo cliente" => vec!["prevendita", "riunione"],
        "Scheda prodotto: varianti" => vec!["sviluppo", "SEO"],
        _ => vec![],
    };
    // every few days some work is reviewed, and now and then something is urgent
    if (back + i as i64) % 3 == 0 && t.contains(&"sviluppo") {
        t.push(if back % 2 == 0 { "code review" } else { "Code Review" });
    }
    if (back * 5 + i as i64) % 11 == 0 && !t.is_empty() {
        t.push("urgente");
    }
    t.into_iter().map(String::from).collect()
}
