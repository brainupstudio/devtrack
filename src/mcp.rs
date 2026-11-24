//! A Model Context Protocol server over stdio: lets an AI assistant (Claude Desktop, Claude
//! Code, Codex CLI…) read the person's time as clean facts (`crate::facts`) and write reports.
//!
//! - Read only: the data file is read, never written; nothing else in the folder is touched.
//!   The app saves by writing a temporary file and renaming it, so a read always sees a whole
//!   file. The file is read again whenever it changes, so answers follow the open app.
//! - JSON-RPC 2.0, one message per line on stdin and stdout; anything else goes to stderr.
//! - Every tool answers with `structuredContent` and, for clients that only read text, the same
//!   JSON as text. Numbers come from `facts`, so they match the app.

use crate::facts::{self, Dim, Filter, PeriodSpec, Scope};
use crate::model::Db;
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Protocol versions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub const GLOSSARY_URI: &str = "devtrack://glossary";

const INSTRUCTIONS: &str = "DevTrack is the person's own time tracker. Start with devtrack_overview to see what there is. \
Use devtrack_time_summary for any total (it groups and filters; never add up sessions yourself). \
Read devtrack://glossary for what each number means. Every answer has a `meta` block: say which period and time zone a figure covers, \
and mention the `quality` notes (sessions missing details, a running session) instead of leaving them out. \
Values to enter (devtrack_report_time) follow each destination's own rounding: report them as they are. \
With Azure DevOps reading on, devtrack_assigned_work_items lists the work items assigned to the person and devtrack_work_item reads one live, \
with Completed and Remaining Work next to the time DevTrack tracked.";

const NOT_ALLOWED_AZURE: &str = "DevTrack doesn't let assistants read Azure DevOps work items. In DevTrack, open Settings › AI assistants and turn on “Also read Azure DevOps work items”, then ask again.";

const NOT_ALLOWED: &str = "DevTrack doesn't let assistants read its data yet. In DevTrack, open Settings › AI assistants and turn on “Let AI assistants read DevTrack”, then ask again.";

/// Where the data comes from: the data folder, or fixed data (tests).
pub enum Source {
    Folder { dir: PathBuf, cache: Option<(SystemTime, Db)> },
    Fixed(Db),
}

impl Source {
    pub fn folder(dir: impl Into<PathBuf>) -> Source {
        Source::Folder { dir: dir.into(), cache: None }
    }

    /// The person allowed assistants to read the data (Settings › Connections), on this device.
    fn allowed(&self) -> Result<(), String> {
        match self {
            Source::Fixed(_) => Ok(()),
            Source::Folder { dir, .. } => {
                let on = std::fs::read(dir.join("local.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<crate::store::LocalConfig>(&b).ok())
                    .is_some_and(|l| l.mcp_enabled);
                if on {
                    Ok(())
                } else {
                    Err(NOT_ALLOWED.into())
                }
            }
        }
    }

    fn local(&self) -> Option<crate::store::LocalConfig> {
        match self {
            Source::Fixed(_) => None,
            Source::Folder { dir, .. } => std::fs::read(dir.join("local.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()),
        }
    }

    /// The person also allowed Azure DevOps work items to be read.
    fn azure_allowed(&self) -> Result<(), String> {
        match self {
            Source::Fixed(_) => Ok(()),
            Source::Folder { .. } => {
                if self.local().is_some_and(|l| l.mcp_azure) {
                    Ok(())
                } else {
                    Err(NOT_ALLOWED_AZURE.into())
                }
            }
        }
    }

    fn dir(&self) -> Option<PathBuf> {
        match self {
            Source::Folder { dir, .. } => Some(dir.clone()),
            Source::Fixed(_) => None,
        }
    }

    /// The current data, read again only when the file changed.
    fn db(&mut self) -> Result<&Db, String> {
        match self {
            Source::Fixed(db) => Ok(db),
            Source::Folder { dir, cache } => {
                let path = dir.join("data.json");
                let modified = match std::fs::metadata(&path) {
                    Ok(m) => m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(format!("There is no DevTrack data in {} yet: open DevTrack and track some time first.", dir.display()))
                    }
                    Err(e) => return Err(format!("DevTrack's data can't be read: {e}")),
                };
                if cache.as_ref().is_none_or(|(at, _)| *at != modified) {
                    let db = read_only(&path)?;
                    *cache = Some((modified, db));
                }
                Ok(&cache.as_ref().expect("just read").1)
            }
        }
    }
}

/// Reads the data file without ever writing to it (no migration, no backup, no lock).
pub fn read_only(path: &Path) -> Result<Db, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("DevTrack's data can't be read: {e}"))?;
    serde_json::from_slice::<Db>(&bytes).map_err(|e| format!("DevTrack's data file can't be decoded ({e}); open DevTrack to check it."))
}

pub struct Server {
    source: Source,
    /// How a work item is read from Azure DevOps (live; a fake one in tests).
    azure: Box<crate::mcp_azure::Reader>,
    now: fn() -> i64,
    today: fn() -> chrono::NaiveDate,
    protocol: &'static str,
}

impl Server {
    pub fn new(source: Source) -> Server {
        Server { source, azure: Box::new(crate::mcp_azure::live), now: crate::store::now_secs, today: crate::report::today, protocol: PROTOCOL_VERSIONS[0] }
    }

    /// One incoming message: the response to send, or None for a notification.
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str);
        let Some(method) = method else {
            // a response from the client (we send no requests) or nonsense: nothing to answer
            return id.filter(|_| msg.get("result").is_none() && msg.get("error").is_none()).map(|id| error(id, -32600, "Invalid request: no method"));
        };
        let id = id?; // notifications (initialized, cancelled…) need no answer
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => self.call(&params),
            "resources/list" => Ok(json!({ "resources": [{
                "uri": GLOSSARY_URI, "name": "glossary", "title": "How to read DevTrack's numbers",
                "description": "Definitions of tracked time, days, weeks, estimates and values to enter.", "mimeType": "text/markdown"
            }] })),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "resources/read" => match params.get("uri").and_then(Value::as_str) {
                Some(GLOSSARY_URI) => Ok(json!({ "contents": [{ "uri": GLOSSARY_URI, "mimeType": "text/markdown", "text": facts::GLOSSARY }] })),
                Some(other) => Err((-32002, format!("Resource not found: {other}"))),
                None => Err((-32602, "Missing uri".into())),
            },
            "prompts/list" => Ok(json!({ "prompts": prompts() })),
            "prompts/get" => prompt(&params).map_err(|m| (-32602, m)),
            other => Err((-32601, format!("Method not found: {other}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, m)) => error(id, code, &m),
        })
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
        self.protocol = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
        json!({
            "protocolVersion": self.protocol,
            "capabilities": { "tools": { "listChanged": false }, "resources": { "listChanged": false, "subscribe": false }, "prompts": { "listChanged": false } },
            "serverInfo": { "name": "devtrack", "title": "DevTrack", "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        })
    }

    /// A tool call. Problems with the arguments or the data come back as a tool result with
    /// `isError`, so the model can read them and try again; only an unknown tool is a protocol error.
    fn call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params.get("name").and_then(Value::as_str).ok_or((-32602, "Missing tool name".to_string()))?;
        if !tools().iter().any(|t| t["name"] == name) {
            return Err((-32602, format!("Unknown tool: {name}")));
        }
        let empty = Map::new();
        let args = params.get("arguments").and_then(Value::as_object).unwrap_or(&empty);
        let (now, today) = ((self.now)(), (self.today)());
        let anonymize = args.get("anonymize").and_then(Value::as_bool).unwrap_or(false);
        let azure_tool = matches!(name, "devtrack_assigned_work_items" | "devtrack_work_item");
        let dir = self.source.dir();
        let outcome = if azure_tool {
            let reader = &self.azure;
            self.source.allowed().and_then(|_| self.source.azure_allowed()).and_then(|_| self.source.db()).and_then(|db| match name {
                "devtrack_assigned_work_items" => {
                    let dir = dir.ok_or("No data folder")?;
                    crate::mcp_azure::assigned(&dir, db, now, text_arg(args, "query").as_deref(), text_arg(args, "state").as_deref(), bool_arg(args, "only_untracked")?.unwrap_or(false))
                }
                _ => {
                    let id = text_arg(args, "work_item").ok_or("Name the work item (work_item), e.g. 28885")?;
                    let n = args.get("changes").and_then(Value::as_u64).unwrap_or(10).min(30) as usize;
                    crate::mcp_azure::work_item(db, now, &id, n, reader.as_ref())
                }
            })
        } else {
            let azure_ok = self.source.azure_allowed().is_ok();
            let reader = &self.azure;
            self.source.allowed().and_then(|_| self.source.db()).and_then(|db| {
                if name == "devtrack_report_tables" && !anonymize {
                    let mut v = run(name, args, db, now, today)?;
                    add_azure_progress(&mut v, args, db, now, today, azure_ok, reader.as_ref())?;
                    return Ok(v);
                }
            if anonymize {
                // names are matched on the real data, then everything is answered from the copy
                let mut a = args.clone();
                let f = facts::anonymize_filter(db, &filter_arg(args)?);
                for (k, v) in [("client", f.client), ("project", f.project), ("task", f.task)] {
                    if let Some(v) = v {
                        a.insert(k.into(), Value::String(v));
                    }
                }
                run(name, &a, &facts::anonymized(db), now, today)
            } else {
                run(name, args, db, now, today)
            }
        })
        };
        Ok(match outcome {
            Ok(v) => {
                let text = serde_json::to_string(&v).unwrap_or_default();
                if self.protocol == "2024-11-05" {
                    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
                } else {
                    json!({ "content": [{ "type": "text", "text": text }], "structuredContent": v, "isError": false })
                }
            }
            Err(m) => json!({ "content": [{ "type": "text", "text": m }], "isError": true }),
        })
    }
}

/// Adds the "Azure DevOps progress" table to a report when the person allows Azure DevOps
/// reading, or a note saying how to get it.
fn add_azure_progress(v: &mut Value, args: &Map<String, Value>, db: &Db, now: i64, today: chrono::NaiveDate, allowed: bool, read: &crate::mcp_azure::Reader) -> Result<(), String> {
    let p = period_arg(args, db, today, "this_week")?;
    let filter = filter_arg(args)?;
    if crate::mcp_azure::work_items_in(db, &p, now, &filter).is_empty() {
        return Ok(());
    }
    if !allowed {
        if let Some(n) = v["notes"].as_array_mut() {
            n.push(json!("Some tasks are Azure DevOps work items: turn on “Also read Azure DevOps work items” in DevTrack (Settings › AI assistants) to add their progress (estimate, Completed and Remaining Work)."));
        }
        return Ok(());
    }
    let (table, data, notes) = crate::mcp_azure::progress_table(db, &p, now, &filter, read);
    if let Some(md) = v["markdown"].as_str() {
        v["markdown"] = json!(format!("{}\n{}", md, facts::markdown(&table)));
    }
    if let Some(t) = v["tables"].as_array_mut() {
        t.push(to_value(&table)?);
    }
    if let Some(n) = v["notes"].as_array_mut() {
        n.extend(notes.into_iter().map(Value::String));
    }
    v["azure_progress"] = Value::Array(data);
    Ok(())
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

// ---------- arguments ----------

fn text_arg(args: &Map<String, Value>, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

fn bool_arg(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("“{key}” must be true or false")),
    }
}

fn period_arg(args: &Map<String, Value>, db: &Db, today: chrono::NaiveDate, default: &str) -> Result<crate::report::Period, String> {
    let spec = match (text_arg(args, "from"), text_arg(args, "to")) {
        (Some(f), Some(t)) => PeriodSpec::parse(&format!("{f}..{t}"))?,
        (Some(f), None) => PeriodSpec::parse(&f)?,
        (None, Some(t)) => PeriodSpec::parse(&t)?,
        (None, None) => PeriodSpec::parse(&text_arg(args, "period").unwrap_or_else(|| default.into()))?,
    };
    spec.resolve(today, db.settings.week_starts_monday)
}

fn filter_arg(args: &Map<String, Value>) -> Result<Filter, String> {
    Ok(Filter {
        client: text_arg(args, "client"),
        project: text_arg(args, "project"),
        task: text_arg(args, "task"),
        source: text_arg(args, "source"),
        billable: bool_arg(args, "billable")?,
        tag: text_arg(args, "tag"),
        text: text_arg(args, "text"),
    })
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn run(name: &str, args: &Map<String, Value>, db: &Db, now: i64, today: chrono::NaiveDate) -> Result<Value, String> {
    match name {
        "devtrack_overview" => to_value(facts::overview(db, today, now)),
        "devtrack_catalog" => {
            let all = bool_arg(args, "include_archived")?.unwrap_or(false);
            let q = text_arg(args, "query").map(|q| q.to_lowercase());
            let hit = |s: &str| q.as_deref().is_none_or(|q| s.to_lowercase().contains(q));
            let kind = text_arg(args, "kind").unwrap_or_else(|| "projects".into());
            let items = match kind.as_str() {
                "clients" => to_value(facts::clients(db, all).into_iter().filter(|c| hit(&c.name)).collect::<Vec<_>>())?,
                "projects" => to_value(facts::projects(db, all).into_iter().filter(|p| hit(&p.name) || hit(&p.client)).collect::<Vec<_>>())?,
                "tasks" => to_value(
                    facts::tasks(db, all)
                        .into_iter()
                        .filter(|t| hit(&t.title) || hit(&t.project) || t.work_item.as_deref().is_some_and(&hit))
                        .take(facts::MAX_ROWS)
                        .collect::<Vec<_>>(),
                )?,
                "tags" => to_value(facts::tags(db, now).into_iter().filter(|t| hit(&t.name)).collect::<Vec<_>>())?,
            other => return Err(format!("“{other}” is not a kind; use clients, projects, tasks or tags")),
            };
            Ok(json!({ "meta": facts::meta(db, None, now), "kind": kind, "items": items }))
        }
        "devtrack_time_summary" => {
            let p = period_arg(args, db, today, "this_week")?;
            let dims = match args.get("group_by") {
                None | Some(Value::Null) => vec![],
                Some(Value::String(s)) => vec![Dim::parse(s)?],
                Some(Value::Array(a)) => a.iter().map(|d| d.as_str().ok_or("group_by lists names".to_string()).and_then(Dim::parse)).collect::<Result<_, _>>()?,
                Some(_) => return Err("group_by is a list of dimensions".into()),
            };
            to_value(facts::time_summary(db, &p, now, &filter_arg(args)?, &dims)?)
        }
        "devtrack_sessions" => {
            let p = period_arg(args, db, today, "this_week")?;
            let rows = facts::time_rows(db, &p, now, &filter_arg(args)?);
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(50).clamp(1, 200) as usize;
            let from = text_arg(args, "cursor").and_then(|c| c.parse::<usize>().ok()).unwrap_or(0).min(rows.len());
            let page: Vec<_> = rows.iter().skip(from).take(limit).cloned().collect();
            let next = (from + page.len() < rows.len()).then(|| (from + page.len()).to_string());
            Ok(json!({ "meta": facts::meta(db, Some(&p), now), "total_rows": rows.len(), "rows": to_value(page)?, "next_cursor": next }))
        }
        "devtrack_estimates" => {
            let scope = match text_arg(args, "scope").as_deref().unwrap_or("task") {
                "task" => Scope::Task,
                "project" => Scope::Project,
                other => return Err(format!("“{other}” is not a scope; use task or project")),
            };
            let rows = facts::estimates(db, scope, now, &filter_arg(args)?, bool_arg(args, "only_over")?.unwrap_or(false));
            Ok(json!({ "meta": facts::meta(db, None, now), "scope": scope, "rows": to_value(rows)? }))
        }
        "devtrack_report_time" => {
            let p = period_arg(args, db, today, "this_week")?;
            match text_arg(args, "destination") {
                Some(d) => to_value(facts::report_time(db, &d, &p, now)?),
                None => Ok(json!({ "meta": facts::meta(db, Some(&p), now), "destinations": to_value(facts::destinations(db))?,
                    "note": "Name one destination to get its lines." })),
            }
        }
        "devtrack_report_tables" => {
            let p = period_arg(args, db, today, "this_week")?;
            let kind = facts::ReportKind::parse(&text_arg(args, "kind").unwrap_or_else(|| "weekly".into()))?;
            to_value(facts::report_tables(db, kind, &p, now, &filter_arg(args)?)?)
        }
        "devtrack_data_quality" => {
            let p = period_arg(args, db, today, "this_week")?;
            Ok(json!({ "meta": facts::meta(db, Some(&p), now), "quality": to_value(facts::quality(db, &p, now))? }))
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}

// ---------- tool list ----------

fn period_props() -> Value {
    json!({
        "period": { "type": "string", "description": "today, yesterday, this_week, last_week, this_month, last_month, a day (YYYY-MM-DD) or a range (YYYY-MM-DD..YYYY-MM-DD). Default: this_week." },
        "from": { "type": "string", "description": "First day, YYYY-MM-DD (instead of period)." },
        "to": { "type": "string", "description": "Last day, YYYY-MM-DD, included." }
    })
}

fn filter_props() -> Value {
    json!({
        "client": { "type": "string", "description": "Client name (any case) or id." },
        "project": { "type": "string", "description": "Project name (any case) or id." },
        "task": { "type": "string", "description": "Task title, id or work item number (#4812 or 4812)." },
        "source": { "type": "string", "enum": ["manual", "azure_devops"] },
        "billable": { "type": "boolean" },
        "tag": { "type": "string", "description": "A tag (any case), or \"(no tag)\" for the sessions without any." },
        "text": { "type": "string", "description": "Words in the task title or the session's note." }
    })
}

fn merge(parts: &[Value]) -> Value {
    let mut m = Map::new();
    for p in parts {
        if let Some(o) = p.as_object() {
            m.extend(o.clone());
        }
    }
    Value::Object(m)
}

fn tool(name: &str, title: &str, description: &str, properties: Value) -> Value {
    let mut properties = properties;
    if let Some(o) = properties.as_object_mut() {
        o.insert("anonymize".into(), json!({ "type": "boolean",
            "description": "For a report going outside the team: clients become “Client A”, “Client B”… (also inside project and task names) and session notes are left out. Filters still take the real names." }));
    }
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "additionalProperties": false },
        "annotations": { "title": title, "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
    })
}

pub fn tools() -> Vec<Value> {
    vec![
        tool(
            "devtrack_overview",
            "DevTrack overview",
            "Start here: the days with tracked time, clients and projects, open tasks, destinations for reported hours, working days, any running session, and this week's and last week's totals.",
            json!({}),
        ),
        tool(
            "devtrack_catalog",
            "Clients, projects and tasks",
            "Lists clients, projects, tasks (with estimates, Azure DevOps work items and states) or tags, to find exact names and ids.",
            json!({
                "kind": { "type": "string", "enum": ["clients", "projects", "tasks", "tags"], "description": "Default: projects. tags: the tags as one list, with sessions and time on each." },
                "query": { "type": "string", "description": "Words in the name (or the work item number)." },
                "include_archived": { "type": "boolean", "description": "Also archived clients and projects, and done tasks." }
            }),
        ),
        tool(
            "devtrack_time_summary",
            "Time summary",
            "Tracked time in a period, grouped by up to two dimensions and filtered: the source for every total. \
             Rows carry seconds, hours (2 decimals), h:mm, sessions and share of the total; the answer also lists data-quality notes.",
            merge(&[
                period_props(),
                json!({ "group_by": { "type": "array", "maxItems": 2, "items": { "type": "string", "enum": Dim::ALL },
                    "description": "Up to two of: day, week, month, client, project, task, work_item, source, billable, tag. Empty: the total only." } }),
                filter_props(),
            ]),
        ),
        tool(
            "devtrack_sessions",
            "Sessions",
            "The single sessions behind a total, one row per session per day, oldest first, with notes and tags. Paged: pass next_cursor back as cursor.",
            merge(&[
                period_props(),
                filter_props(),
                json!({ "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "Default 50." }, "cursor": { "type": "string" } }),
            ]),
        ),
        tool(
            "devtrack_estimates",
            "Estimates and progress",
            "Tasks or projects with an estimate: all time tracked on them, remaining hours, progress and whether the estimate is exceeded. A work item's estimate is its Azure DevOps Original Estimate.",
            merge(&[
                json!({ "scope": { "type": "string", "enum": ["task", "project"], "description": "Default: task." },
                    "only_over": { "type": "boolean", "description": "Only those over their estimate." } }),
                filter_props(),
            ]),
        ),
        tool(
            "devtrack_report_time",
            "Values to enter",
            "Report Time: for one destination (a company timesheet, a client's hours log), the value to enter per line after its rounding and format, with each line's status. Without a destination, lists them.",
            merge(&[period_props(), json!({ "destination": { "type": "string", "description": "Destination name (any case) or id." } })]),
        ),
        tool(
            "devtrack_report_tables",
            "Report tables",
            "The tables of a report for a project manager, computed by DevTrack: weekly (by client and project, main tasks, by day), project (by task, then by day or by week; needs project) client (by project and task; needs client) or tag (by tag, then by tag and project: the kinds of work); weekly adds a table by tag when the period's sessions have tags. \
             With Azure DevOps reading on, an \"Azure DevOps progress\" table is added for the work items of the period: state, estimate, Completed and Remaining Work read live, DevTrack's tracked time and where each stands. \
             Each comes with the estimates of the tasks worked on, Markdown to paste as it is and CSV. Use these figures as they are and write the commentary around them.",
            merge(&[
                json!({ "kind": { "type": "string", "enum": ["weekly", "project", "client", "tag"], "description": "Default: weekly." } }),
                period_props(),
                filter_props(),
            ]),
        ),
        tool(
            "devtrack_assigned_work_items",
            "Assigned work items",
            "Azure DevOps work items assigned to the person, as DevTrack last read them (no request to Azure): number, title, type, state, Original Estimate, \
             and for each the DevTrack task and the time tracked on it, or nothing when it isn't in DevTrack. Needs “Also read Azure DevOps work items” on.",
            json!({
                "query": { "type": "string", "description": "Words in the title or the Azure DevOps project, or the number." },
                "state": { "type": "string", "description": "Only this state (New, Active, Resolved…)." },
                "only_untracked": { "type": "boolean", "description": "Only those with no time in DevTrack." }
            }),
        ),
        tool(
            "devtrack_work_item",
            "Work item from Azure DevOps",
            "One work item read live from Azure DevOps, read only: state, assignee, Original Estimate, Completed and Remaining Work, its latest changes, \
             and next to them the time DevTrack tracked on it with the difference from Completed Work. Needs “Also read Azure DevOps work items” on.",
            json!({
                "work_item": { "type": "string", "description": "The number: 28885 or #28885." },
                "changes": { "type": "integer", "minimum": 0, "maximum": 30, "description": "How many latest changes (default 10)." }
            }),
        ),
        tool(
            "devtrack_data_quality",
            "Data quality",
            "What would make a report wrong or incomplete in a period: sessions missing a project or task, a running session, overlaps, unusual durations, tasks no longer in Azure DevOps.",
            period_props(),
        ),
    ]
}

// ---------- prompts ----------

fn prompt_arg(name: &str, description: &str, required: bool) -> Value {
    json!({ "name": name, "description": description, "required": required })
}

/// Report templates the person picks in the assistant (Claude Desktop lists them under "+").
pub fn prompts() -> Vec<Value> {
    let period = prompt_arg("period", "this_week, last_week, this_month, last_month or YYYY-MM-DD..YYYY-MM-DD (default: last_week)", false);
    let language = prompt_arg("language", "Language of the report (default: Italian)", false);
    let anon = prompt_arg("anonymize", "yes to replace client names, for readers outside the team", false);
    vec![
        json!({ "name": "pm_weekly_report", "title": "Weekly report for a PM",
            "description": "Hours by client and project, main tasks and work items, progress against estimates, points of attention.",
            "arguments": [period, prompt_arg("client", "Only this client (optional)", false), language, anon] }),
        json!({ "name": "project_status", "title": "Project status",
            "description": "One project: time in the period and in all, tasks, estimates and remaining hours, trend by week.",
            "arguments": [prompt_arg("project", "The project", true), period, language, anon] }),
        json!({ "name": "client_summary", "title": "Client summary",
            "description": "One client in a period, by project and task, to go with an invoice or a statement of work.",
            "arguments": [prompt_arg("client", "The client", true), prompt_arg("period", "Default: last_month", false), language, anon] }),
    ]
}

fn prompt(params: &Value) -> Result<Value, String> {
    let name = params.get("name").and_then(Value::as_str).ok_or("Missing prompt name")?;
    let empty = Map::new();
    let args = params.get("arguments").and_then(Value::as_object).unwrap_or(&empty);
    let get = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    let language = get("language").unwrap_or_else(|| "Italian".into());
    let anonymize = get("anonymize").is_some_and(|a| matches!(a.to_lowercase().as_str(), "yes" | "si" | "sì" | "true" | "1"));
    let anon = if anonymize { ", \"anonymize\": true" } else { "" };
    let rules = format!(
        "Rules: take every figure from DevTrack's tools and copy it as it is; never add up or round hours yourself. \
         Write every duration exactly as the `duration` fields give it (\"4 h 18 min\", translated into the report's language): never as decimal hours (4,31) \
         or as h:mm (4:18), and never both side by side. Shares are whole percentages. \
         Title the report with the project or client and the period from `period_title` (\"week 47 · 2026-11-16 – 2026-11-22\"), dates written in full in the report's language. \
         Lay out time by task, by day and by project as tables with one time column and the share, as in the tool's `markdown`, the total row in bold. \
         When the tool adds the \"Azure DevOps progress\" table, include it as it is (work items as #number), and call out the rows whose status starts with \"!\". \
         If `quality` lists problems (sessions missing details, a running session, overlaps), add a short \"Data notes\" section instead of leaving them out. \
         Keep it short and factual: a PM reads it in two minutes. Write in {language}; keep names, codes and work item numbers as they are."
    );
    let (description, text) = match name {
        "pm_weekly_report" => {
            let period = get("period").unwrap_or_else(|| "last_week".into());
            let client = get("client").map(|c| format!(", \"client\": \"{c}\"")).unwrap_or_default();
            ("Weekly report for a PM", format!(
                "Prepare the weekly report of my work for a project manager.\n\n\
                 1. Call devtrack_report_tables with {{\"kind\": \"weekly\", \"period\": \"{period}\"{client}{anon}}}.\n\
                 2. If you need the work done on a task, call devtrack_sessions with the same period and filters.\n\n\
                 Write: a two-line summary (total time, where most of it went); time by client and project; the main tasks with their work items; \
                 estimates that are exceeded or close to it; data notes.\n\n{rules}"))
        }
        "project_status" => {
            let project = get("project").ok_or("The project is required")?;
            let period = get("period").unwrap_or_else(|| "last_month".into());
            ("Project status", format!(
                "Prepare a status update of the project “{project}” for its project manager.\n\n\
                 1. Call devtrack_report_tables with {{\"kind\": \"project\", \"project\": \"{project}\", \"period\": \"{period}\"{anon}}}.\n\
                 2. Call devtrack_estimates with {{\"scope\": \"task\", \"project\": \"{project}\"{anon}}} for progress against estimates in all.\n\n\
                 Write: where the project stands in two lines; time in the period by week; tasks and their progress; risks (estimates exceeded, tasks with no estimate); data notes.\n\n{rules}"))
        }
        "client_summary" => {
            let client = get("client").ok_or("The client is required")?;
            let period = get("period").unwrap_or_else(|| "last_month".into());
            ("Client summary", format!(
                "Prepare a summary of the work for the client “{client}”.\n\n\
                 1. Call devtrack_report_tables with {{\"kind\": \"client\", \"client\": \"{client}\", \"period\": \"{period}\"{anon}}}.\n\n\
                 Write: the total time (and billable time) of the period; time by project; the main tasks; data notes.\n\n{rules}"))
        }
        other => return Err(format!("Unknown prompt: {other}")),
    };
    Ok(json!({ "description": description, "messages": [{ "role": "user", "content": { "type": "text", "text": text } }] }))
}

// ---------- stdio ----------

/// Serves one client on stdin and stdout until stdin closes.
pub fn serve_stdio(source: Source) -> std::io::Result<()> {
    let mut server = Server::new(source);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let replies: Vec<Value> = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => batch.iter().filter_map(|m| server.handle(m)).collect(),
            Ok(msg) => server.handle(&msg).into_iter().collect(),
            Err(e) => vec![error(Value::Null, -32700, &format!("Parse error: {e}"))],
        };
        for r in replies {
            serde_json::to_writer(&mut out, &r)?;
            out.write_all(b"\n")?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{build_state, DemoState};

    fn server() -> Server {
        Server::new(Source::Fixed(build_state(crate::store::now_secs(), DemoState::Incomplete, true)))
    }

    fn ask(s: &mut Server, id: i64, method: &str, params: Value) -> Value {
        s.handle(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).expect("an answer")
    }

    fn call(s: &mut Server, name: &str, args: Value) -> Value {
        ask(s, 9, "tools/call", json!({ "name": name, "arguments": args }))["result"].clone()
    }

    #[test]
    fn handshake_then_tools() {
        let mut s = server();
        let init = ask(&mut s, 1, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }));
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(init["result"]["serverInfo"]["name"], "devtrack");
        assert!(init["result"]["capabilities"]["tools"].is_object());
        // the initialized notification gets no answer
        assert!(s.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let list = ask(&mut s, 2, "tools/list", json!({}));
        let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), 10);
        for t in list["result"]["tools"].as_array().unwrap() {
            assert_eq!(t["annotations"]["readOnlyHint"], true, "{}", t["name"]);
            assert_eq!(t["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn an_unknown_protocol_version_gets_the_newest() {
        let mut s = server();
        let init = ask(&mut s, 1, "initialize", json!({ "protocolVersion": "1999-01-01" }));
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
        let old = ask(&mut s, 2, "initialize", json!({ "protocolVersion": "2024-11-05" }));
        assert_eq!(old["result"]["protocolVersion"], "2024-11-05");
        // the oldest version has no structured results: the JSON travels as text only
        let r = call(&mut s, "devtrack_overview", json!({}));
        assert!(r.get("structuredContent").is_none());
        assert!(serde_json::from_str::<Value>(r["content"][0]["text"].as_str().unwrap()).is_ok());
    }

    #[test]
    fn time_summary_matches_the_facts() {
        let mut s = server();
        let db = build_state(crate::store::now_secs(), DemoState::Incomplete, true);
        let r = call(&mut s, "devtrack_time_summary", json!({ "period": "this_week", "group_by": ["project"] }));
        assert_eq!(r["isError"], false);
        let sc = &r["structuredContent"];
        let p = PeriodSpec::ThisWeek.resolve(crate::report::today(), db.settings.week_starts_monday).unwrap();
        let summary = crate::report::summarize(&db, &p, crate::store::now_secs(), None);
        // a running session may have grown by a second between the two reads
        let got = sc["total"]["seconds"].as_i64().unwrap();
        assert!((got - summary.total).abs() <= 2, "{got} vs {}", summary.total);
        assert_eq!(sc["meta"]["period"]["from"], p.start.to_string());
        assert_eq!(serde_json::from_str::<Value>(r["content"][0]["text"].as_str().unwrap()).unwrap()["total"]["seconds"].as_i64().unwrap() / 60, got / 60);
    }

    /// Tags for an assistant: the list with its counts, a summary by tag in the list's spelling,
    /// the sessions without tags, and the report by tag.
    #[test]
    fn tags_are_one_list_for_the_assistant_too() {
        let now = crate::store::now_secs();
        // through the store, as the app keeps it: the tags written by hand become one list
        let db = crate::store::Store::in_memory(build_state(now, DemoState::Varied, false)).db;
        let mut s = Server::new(Source::Fixed(db));
        let r = call(&mut s, "devtrack_catalog", json!({ "kind": "tags" }));
        assert_eq!(r["isError"], false);
        let items = r["structuredContent"]["items"].as_array().unwrap().clone();
        assert!(items.iter().any(|t| t["name"] == "sviluppo" && t["sessions"].as_u64().unwrap() > 0));
        assert!(items.iter().any(|t| t["name"] == "Trasferta" && t["sessions"] == 0), "an unused tag is listed too");
        assert_eq!(items.iter().filter(|t| t["name"].as_str().unwrap().eq_ignore_ascii_case("bug")).count(), 1);
        // by tag: "Bug" and "bug" are one row, in the list's spelling
        let r = call(&mut s, "devtrack_time_summary", json!({ "period": "this_month", "group_by": ["tag"] }));
        let rows = r["structuredContent"]["rows"].as_array().unwrap().clone();
        let bug: Vec<&Value> = rows.iter().filter(|x| x["keys"][0]["label"].as_str().is_some_and(|l| l.eq_ignore_ascii_case("bug"))).collect();
        assert_eq!(bug.len(), 1, "{rows:?}");
        assert!(rows.iter().any(|x| x["keys"][0]["label"] == "(no tag)"), "the unclassified call has no tag");
        // only the sessions without tags
        let r = call(&mut s, "devtrack_sessions", json!({ "period": "this_month", "tag": "(no tag)" }));
        let rows = r["structuredContent"]["rows"].as_array().unwrap().clone();
        assert!(!rows.is_empty() && rows.iter().all(|x| x["tags"].as_array().unwrap().is_empty()));
        // the report by tag
        let r = call(&mut s, "devtrack_report_tables", json!({ "kind": "tag", "period": "this_month" }));
        assert_eq!(r["isError"], false, "{r:?}");
        let titles: Vec<String> = r["structuredContent"]["tables"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap().to_string()).collect();
        assert!(titles.starts_with(&["By tag".to_string(), "By tag and project".to_string()]), "{titles:?}");
    }

    #[test]
    fn bad_arguments_come_back_as_tool_errors() {
        let mut s = server();
        let r = call(&mut s, "devtrack_time_summary", json!({ "group_by": ["weather"] }));
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"].as_str().unwrap().contains("weather"));
        let r = call(&mut s, "devtrack_time_summary", json!({ "period": "2020-01-01..2026-01-01" }));
        assert_eq!(r["isError"], true);
        let r = call(&mut s, "devtrack_report_time", json!({ "destination": "nowhere" }));
        assert_eq!(r["isError"], true);
        // an unknown tool is a protocol error
        let e = ask(&mut s, 3, "tools/call", json!({ "name": "rm_rf", "arguments": {} }));
        assert_eq!(e["error"]["code"], -32602);
        let e = ask(&mut s, 4, "sampling/createMessage", json!({}));
        assert_eq!(e["error"]["code"], -32601);
    }

    #[test]
    fn every_tool_answers_on_sample_data() {
        let mut s = server();
        for (name, args) in [
            ("devtrack_overview", json!({})),
            ("devtrack_catalog", json!({ "kind": "tasks" })),
            ("devtrack_catalog", json!({ "kind": "clients" })),
            ("devtrack_time_summary", json!({ "period": "this_month", "group_by": ["client", "day"] })),
            ("devtrack_sessions", json!({ "period": "this_week", "limit": 2 })),
            ("devtrack_estimates", json!({ "scope": "project" })),
            ("devtrack_report_time", json!({})),
            ("devtrack_data_quality", json!({ "period": "this_week" })),
            ("devtrack_report_tables", json!({ "period": "this_month" })),
            ("devtrack_time_summary", json!({ "group_by": ["client"], "anonymize": true })),
        ] {
            let r = call(&mut s, name, args);
            assert_eq!(r["isError"], false, "{name}: {}", r["content"][0]["text"]);
            assert!(r["structuredContent"].is_object(), "{name}");
        }
    }

    #[test]
    fn sessions_are_paged() {
        let mut s = server();
        let first = call(&mut s, "devtrack_sessions", json!({ "period": "this_month", "limit": 2 }))["structuredContent"].clone();
        let total = first["total_rows"].as_u64().unwrap();
        assert!(total > 2);
        assert_eq!(first["rows"].as_array().unwrap().len(), 2);
        let next = first["next_cursor"].as_str().unwrap().to_string();
        let second = call(&mut s, "devtrack_sessions", json!({ "period": "this_month", "limit": 2, "cursor": next }))["structuredContent"].clone();
        assert_ne!(first["rows"][0]["entry_id"], second["rows"][0]["entry_id"]);
    }

    #[test]
    fn the_glossary_is_a_resource() {
        let mut s = server();
        let list = ask(&mut s, 1, "resources/list", json!({}));
        assert_eq!(list["result"]["resources"][0]["uri"], GLOSSARY_URI);
        let read = ask(&mut s, 2, "resources/read", json!({ "uri": GLOSSARY_URI }));
        assert!(read["result"]["contents"][0]["text"].as_str().unwrap().contains("Tracked time"));
        assert_eq!(ask(&mut s, 3, "resources/read", json!({ "uri": "file:///etc/passwd" }))["error"]["code"], -32002);
    }

    #[test]
    fn the_data_folder_is_never_changed() {
        let dir = std::env::temp_dir().join(format!("devtrack-mcp-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = build_state(crate::store::now_secs(), DemoState::Incomplete, false);
        std::fs::write(dir.join("data.json"), serde_json::to_vec_pretty(&db).unwrap()).unwrap();
        // off until the person turns it on
        let mut s = Server::new(Source::folder(&dir));
        let r = call(&mut s, "devtrack_overview", json!({}));
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"].as_str().unwrap().contains("Settings"));
        let local = crate::store::LocalConfig { mcp_enabled: true, ..Default::default() };
        std::fs::write(dir.join("local.json"), serde_json::to_vec(&local).unwrap()).unwrap();
        let local_before = std::fs::read(dir.join("local.json")).unwrap();
        let before = std::fs::read(dir.join("data.json")).unwrap();
        let modified = std::fs::metadata(dir.join("data.json")).unwrap().modified().unwrap();
        let mut s = Server::new(Source::folder(&dir));
        ask(&mut s, 1, "initialize", json!({}));
        for name in tools().iter().map(|t| t["name"].as_str().unwrap().to_string()) {
            let r = call(&mut s, &name, json!({}));
            if name.contains("work_item") {
                // Azure DevOps needs its own switch
                assert_eq!(r["isError"], true, "{name}");
                assert!(r["content"][0]["text"].as_str().unwrap().contains("Azure DevOps"), "{name}");
            } else {
                assert_eq!(r["isError"], false, "{name}: {}", r["content"][0]["text"]);
            }
        }
        assert_eq!(std::fs::read(dir.join("data.json")).unwrap(), before);
        assert_eq!(std::fs::metadata(dir.join("data.json")).unwrap().modified().unwrap(), modified);
        assert_eq!(std::fs::read(dir.join("local.json")).unwrap(), local_before);
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        files.sort();
        assert_eq!(files, vec![std::ffi::OsString::from("data.json"), std::ffi::OsString::from("local.json")], "nothing else is written");
        // a missing file is a readable tool error, not a crash
        std::fs::remove_file(dir.join("data.json")).unwrap();
        let mut s = Server::new(Source::folder(&dir));
        let r = call(&mut s, "devtrack_overview", json!({}));
        assert_eq!(r["isError"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changes_in_the_app_are_seen() {
        let dir = std::env::temp_dir().join(format!("devtrack-mcp-reload-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut db = build_state(crate::store::now_secs(), DemoState::Ready, false);
        std::fs::write(dir.join("data.json"), serde_json::to_vec(&db).unwrap()).unwrap();
        let local = crate::store::LocalConfig { mcp_enabled: true, ..Default::default() };
        std::fs::write(dir.join("local.json"), serde_json::to_vec(&local).unwrap()).unwrap();
        let mut s = Server::new(Source::folder(&dir));
        let count = |s: &mut Server| call(s, "devtrack_overview", json!({}))["structuredContent"]["sessions"].as_u64().unwrap();
        let first = count(&mut s);
        let id = db.entries.keys().next().unwrap().clone();
        db.entries.get_mut(&id).unwrap().deleted = true;
        // a different modification time, as a later save has
        std::thread::sleep(std::time::Duration::from_millis(20));
        let tmp = dir.join("data.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&db).unwrap()).unwrap();
        std::fs::rename(&tmp, dir.join("data.json")).unwrap();
        assert_eq!(count(&mut s), first - 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prompts_are_listed_and_filled() {
        let mut s = server();
        let list = ask(&mut s, 1, "prompts/list", json!({}));
        assert_eq!(list["result"]["prompts"].as_array().unwrap().len(), 3);
        let got = ask(&mut s, 2, "prompts/get", json!({ "name": "project_status", "arguments": { "project": "Client portal", "anonymize": "yes" } }));
        let text = got["result"]["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("\"project\": \"Client portal\""));
        assert!(text.contains("\"anonymize\": true"));
        assert!(text.contains("Italian"));
        let missing = ask(&mut s, 3, "prompts/get", json!({ "name": "project_status", "arguments": {} }));
        assert_eq!(missing["error"]["code"], -32602);
    }

    #[test]
    fn anonymized_answers_still_take_real_names() {
        let mut s = server();
        let db = build_state(crate::store::now_secs(), DemoState::Incomplete, true);
        let client = db.live_clients().into_iter().find(|c| !db.projects_of_client(&c.id).is_empty()).unwrap().name.clone();
        let r = call(&mut s, "devtrack_report_tables", json!({ "kind": "client", "client": client, "period": "this_month", "anonymize": true }));
        assert_eq!(r["isError"], false, "{}", r["content"][0]["text"]);
        let text = r["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains(&client));
        assert!(text.contains("Client "));
    }

    #[test]
    fn a_work_item_is_read_through_the_reader() {
        let mut s = server();
        s.azure = Box::new(|_, id, _| {
            let item = crate::integrations::RemoteItem { id: id.into(), title: "Footer".into(), description: String::new(), status: "Active".into(), kind: "Task".into(), project: "P".into(), url: "u".into(), assigned_to: "Luca".into(), estimate: Some(2.0) };
            Ok((item, crate::integrations::RemoteDetail { remaining: Some(1.0), completed: Some(1.0), estimate: Some(2.0), updates: vec![] }))
        });
        let r = call(&mut s, "devtrack_work_item", json!({ "work_item": "#123" }));
        assert_eq!(r["isError"], false, "{}", r["content"][0]["text"]);
        assert_eq!(r["structuredContent"]["azure_work"]["remaining_work"], "1 h");
        assert_eq!(call(&mut s, "devtrack_work_item", json!({}))["isError"], true);
    }

    #[test]
    fn reports_carry_azure_progress_when_there_are_work_items() {
        let mut s = server();
        s.azure = Box::new(|_, id, _| {
            let item = crate::integrations::RemoteItem { id: id.into(), title: "Work".into(), description: String::new(), status: "Active".into(), kind: "Task".into(), project: "P".into(), url: "u".into(), assigned_to: String::new(), estimate: Some(1.0) };
            Ok((item, crate::integrations::RemoteDetail { remaining: Some(0.0), completed: Some(1.0), estimate: Some(1.0), updates: vec![] }))
        });
        let r = call(&mut s, "devtrack_report_tables", json!({ "period": "this_month" }));
        assert_eq!(r["isError"], false, "{}", r["content"][0]["text"]);
        let sc = &r["structuredContent"];
        assert!(sc["azure_progress"].as_array().is_some_and(|a| !a.is_empty()), "the sample has work items");
        assert!(sc["markdown"].as_str().unwrap().contains("**Azure DevOps progress**"));
        // anonymized reports leave Azure out
        let r = call(&mut s, "devtrack_report_tables", json!({ "period": "this_month", "anonymize": true }));
        assert!(r["structuredContent"].get("azure_progress").is_none());
    }
}

