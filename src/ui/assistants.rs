//! Settings › AI assistants: what an assistant may read, whether it works, and how to connect one.
//!
//! In reading order: what is shared (before the switch that shares it), the switch, a status
//! line that checks itself, then one assistant at a time (Claude Desktop, Claude Code, others),
//! then things to ask. Claude Desktop is connected with one click: DevTrack adds its own entry
//! to Claude's configuration file, after a confirmation and a copy of the file; the JSON is still
//! there to do it by hand.

use super::i18n::{tr, trf};
use super::widgets::*;
use super::App;
use devtrack_core::facts;
use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// What `devtrack mcp` answered when it was last checked from here.
pub type Check = Arc<Mutex<Option<(bool, String)>>>;

#[derive(Default)]
pub struct AssistantsState {
    /// 0 Claude Desktop, 1 Claude Code, 2 other assistants; None until the page picks one.
    pub client: Option<usize>,
    pub check: Option<Check>,
    /// Asked to change Claude Desktop's file; waiting for the confirmation.
    pub confirm_link: bool,
    pub link_msg: Option<(bool, String)>,
    /// What was copied last and when (seconds of the app's clock), for the tick in place.
    pub copied: Option<(String, f64)>,
}

// ---------- Claude Desktop's configuration ----------

/// `claude_desktop_config.json`, where Claude Desktop keeps it (None where there is no Claude Desktop).
pub fn desktop_config_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join("Library/Application Support/Claude/claude_desktop_config.json"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Claude").join("claude_desktop_config.json"))
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Desktop {
    /// No Claude Desktop folder: it isn't installed (or never opened).
    NotInstalled,
    NotLinked,
    Linked,
    /// Linked to a DevTrack that is no longer there (the app was moved or replaced).
    OldPath(String),
    /// The file exists but isn't valid JSON: it is left alone.
    Unreadable(String),
}

pub fn desktop_state(path: &Path, exe: &str, args: &[String]) -> Desktop {
    if !path.parent().is_some_and(|d| d.is_dir()) {
        return Desktop::NotInstalled;
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Desktop::NotLinked,
        Err(e) => return Desktop::Unreadable(e.to_string()),
    };
    if text.trim().is_empty() {
        return Desktop::NotLinked;
    }
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return Desktop::Unreadable(e.to_string()),
    };
    match v.get("mcpServers").and_then(|m| m.get("devtrack")) {
        None => Desktop::NotLinked,
        Some(entry) => {
            let cmd = entry.get("command").and_then(Value::as_str).unwrap_or("");
            let same_args = entry.get("args").and_then(Value::as_array).is_some_and(|a| a.iter().map(|x| x.as_str().unwrap_or("")).eq(args.iter().map(String::as_str)));
            if cmd == exe && same_args {
                Desktop::Linked
            } else {
                Desktop::OldPath(cmd.to_string())
            }
        }
    }
}

/// Adds (or replaces) only the "devtrack" entry under `mcpServers`, keeping everything else as
/// it was, after copying the file aside. Returns where the copy is.
pub fn link_desktop(path: &Path, exe: &str, args: &[String]) -> Result<Option<PathBuf>, String> {
    let mut root = match std::fs::read_to_string(path) {
        Ok(t) if !t.trim().is_empty() => serde_json::from_str::<Value>(&t).map_err(|e| trf("The file isn't valid JSON ({}): it was left as it is.", &[&e]))?,
        Ok(_) => json!({}),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.to_string()),
    };
    let obj = root.as_object_mut().ok_or_else(|| tr("The file doesn't hold a JSON object: it was left as it is."))?;
    let servers = obj.entry("mcpServers").or_insert_with(|| Value::Object(Map::new()));
    let servers = servers.as_object_mut().ok_or_else(|| tr("“mcpServers” in the file isn't an object: it was left as it is."))?;
    servers.insert("devtrack".into(), json!({ "command": exe, "args": args }));
    let backup = if path.exists() {
        let b = path.with_file_name(format!("claude_desktop_config.before-devtrack-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        std::fs::copy(path, &b).map_err(|e| e.to_string())?;
        Some(b)
    } else {
        None
    };
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    devtrack_core::store::write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(backup)
}

// ---------- the check ----------

/// Starts `devtrack mcp` as an assistant would, in the background: the handshake, the tools,
/// one read of the data. The answer lands in the returned slot; the window never waits for it.
fn start_check(exe: String, args: Vec<String>) -> Check {
    let slot: Check = Arc::new(Mutex::new(None));
    let out = slot.clone();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader, Write};
        use std::process::{Command, Stdio};
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let run = || -> Result<(usize, Result<(), String>), String> {
                let mut child = Command::new(&exe).args(&args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
                let mut stdin = child.stdin.take().ok_or("no stdin")?;
                for m in [
                    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"devtrack-settings","version":"1"}}}"#,
                    r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                    r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
                    r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"devtrack_overview","arguments":{}}}"#,
                ] {
                    writeln!(stdin, "{m}").map_err(|e| e.to_string())?;
                }
                drop(stdin);
                let lines: Vec<Value> = BufReader::new(child.stdout.take().ok_or("no stdout")?).lines().map_while(Result::ok).filter_map(|l| serde_json::from_str(&l).ok()).collect();
                let _ = child.wait();
                let tools = lines.iter().find(|v| v["id"] == 2).and_then(|v| v["result"]["tools"].as_array()).map(|a| a.len()).ok_or("no answer")?;
                let read = match lines.iter().find(|v| v["id"] == 3) {
                    Some(v) if v["result"]["isError"] == false => Ok(()),
                    Some(v) => Err(v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()),
                    None => Err(String::new()),
                };
                Ok((tools, read))
            };
            let _ = tx.send(run());
        });
        let result = match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok((n, Ok(())))) => (true, trf("Ready: {} tools, and your data can be read.", &[&n])),
            Ok(Ok((n, Err(e)))) => (false, if e.is_empty() { trf("The server answers with {} tools, but can't read the data.", &[&n]) } else { e }),
            Ok(Err(e)) => (false, trf("DevTrack's server didn't start: {}", &[&e])),
            Err(_) => (false, tr("DevTrack's server didn't answer within 5 seconds.")),
        };
        if let Ok(mut s) = out.lock() {
            *s = Some(result);
        }
    });
    slot
}

// ---------- the page ----------

/// A block of code to copy, with the copy button in its top-right corner; the button turns into
/// a tick for a moment once copied.
fn code_box(app: &mut App, ui: &mut egui::Ui, key: &str, text: &str) {
    let c = p(ui);
    let now = ui.input(|i| i.time);
    let just = app.assistants.copied.as_ref().is_some_and(|(k, t)| k == key && now - t < 1.6);
    let frame = egui::Frame::new().fill(c.field).stroke(Stroke::new(1.0, c.line)).corner_radius(RADIUS).inner_margin(egui::Margin { left: 12, right: 52, top: 11, bottom: 11 }).show(ui, |ui| {
        let w = ui.available_width();
        ui.set_width(w);
        ui.set_min_height(22.0);
        let mut job = egui::text::LayoutJob::simple(text.to_string(), mono_font(13.0), c.text, w);
        job.wrap.break_anywhere = true;
        ui.label(job);
    });
    let r = frame.response.rect;
    let btn = egui::Rect::from_min_size(egui::pos2(r.right() - 4.0 - CTRL_H, r.top() + 4.0), egui::vec2(CTRL_H, CTRL_H));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(btn));
    let resp = icon_button(&mut child, if just { Glyph::Check } else { Glyph::Copy }, if just { "Copied" } else { "Copy" });
    if just {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
    if resp.clicked() {
        ui.ctx().copy_text(text.to_string());
        app.assistants.copied = Some((key.to_string(), now));
    }
}

/// A line of state: a dot of the state's colour, then the words (the colour is never the only cue).
fn state_line(ui: &mut egui::Ui, color: Color32, text: &str) {
    let c = p(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        dot(ui, color, 4.0);
        ui.add(egui::Label::new(mixed_job(text, 14.0, if color == c.text2 { c.text2 } else { c.text }, sans(14.0), mono_font(14.0))).wrap());
    });
}

/// What an assistant reads, what it never sees and where it goes, said before the switch.
fn privacy(ui: &mut egui::Ui) {
    let c = p(ui);
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
        draw_glyph(ui.painter(), r.center(), Glyph::Lock, c.text2);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for (lead, rest) in [
                ("It reads", "sessions, clients, projects, tasks, estimates and the values to enter in Report Time, and Azure DevOps work items only if you allow it below. It can't change anything."),
                ("It never sees", "tokens or the details of your connections."),
                ("It goes to", "the service of the assistant you use: Anthropic for Claude, OpenAI for ChatGPT and Codex."),
            ] {
                let mut job = egui::text::LayoutJob::default();
                job.append(&format!("{} ", tr(lead)), 0.0, egui::TextFormat { font_id: medium(13.0), color: c.text, ..Default::default() });
                job.append(&tr(rest), 0.0, egui::TextFormat { font_id: sans(13.0), color: c.text2, ..Default::default() });
                job.wrap.max_width = ui.available_width();
                ui.label(job);
            }
        });
    });
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let mut on = app.store.local.mcp_enabled;
    let mut azure = app.store.local.mcp_azure;
    let has_azure = app.store.db.integrations.values().any(|c| !c.deleted && c.enabled && c.kind == devtrack_core::model::SOURCE_AZURE);
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "devtrack".into());
    // the data this window has open (demo, test copy), named only when it isn't the usual one
    let dir = app.store.dir().map(|d| d.to_path_buf());
    let mut args = vec!["mcp".to_string()];
    if let Some(d) = dir.as_ref().filter(|d| **d != devtrack_core::store::default_dir()) {
        args.push("--data-dir".into());
        args.push(d.display().to_string());
    }
    let desktop_path = desktop_config_path();
    let desktop = desktop_path.as_ref().map(|p| desktop_state(p, &exe, &args));

    // the check runs by itself the first time the page shows with sharing on
    if on && app.assistants.check.is_none() && dir.is_some() {
        app.assistants.check = Some(start_check(exe.clone(), args.clone()));
    }
    let checked: Option<(bool, String)> = app.assistants.check.as_ref().and_then(|s| s.lock().ok().and_then(|g| g.clone()));
    let checking = on && app.assistants.check.is_some() && checked.is_none();
    if checking {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
    }
    let mut recheck = false;
    let mut relink = false;

    settings_group(ui, "What assistants can read", Some("Ask Claude or another assistant about your hours, and have it write reports for your project managers."), |rows| {
        rows.block(privacy);
        rows.row("Let AI assistants read DevTrack", Some("On this device only. Turn it off at any time: they stop reading at once."), 40.0, |ui| {
            switch(ui, &mut on, "Let AI assistants read DevTrack");
        });
        if !on {
            return;
        }
        // Azure DevOps titles, states and changes are more than DevTrack's own data: their own switch
        let help = if has_azure {
            "The work items assigned to you and, when asked, one work item read live with your token: state, Completed and Remaining Work, latest changes. Their titles and changes go to the assistant's service too."
        } else {
            "No Azure DevOps connection on this device yet (Settings › Connections)."
        };
        rows.row("Also read Azure DevOps work items", Some(help), 40.0, |ui| {
            ui.add_enabled_ui(has_azure, |ui| switch(ui, &mut azure, "Also read Azure DevOps work items"));
        });
        rows.block(|ui| {
            ui.horizontal(|ui| {
                let btn_w = BTN_MIN_W + 20.0;
                ui.allocate_ui(egui::vec2((ui.available_width() - btn_w - 16.0).max(120.0), CTRL_H), |ui| {
                    ui.set_min_height(CTRL_H);
                    ui.vertical(|ui| {
                        ui.add_space(8.0);
                        match (&checked, checking) {
                            (_, true) => state_line(ui, c.text2, &tr("Checking that assistants can reach DevTrack…")),
                            (Some((true, m)), _) => state_line(ui, c.green, m),
                            (Some((false, m)), _) => state_line(ui, c.red, m),
                            (None, _) => state_line(ui, c.text2, &tr("Not checked yet.")),
                        }
                        if let Some(Desktop::OldPath(old)) = &desktop {
                            ui.add_space(6.0);
                            state_line(ui, c.amber, &trf("! Claude Desktop still points to a DevTrack that isn't there any more ({}).", &[old]));
                        }
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if matches!(desktop, Some(Desktop::OldPath(_))) {
                        relink = button_w(ui, Kind::Primary, &tr("Update Claude Desktop"), btn_w + 40.0, true).clicked();
                    } else {
                        let label = if checked.is_some() { "Check again" } else { "Check" };
                        recheck = button_w(ui, Kind::Secondary, &tr(label), btn_w, !checking && dir.is_some()).clicked();
                    }
                });
            });
        });
    });
    if azure != app.store.local.mcp_azure {
        app.store.local.mcp_azure = azure;
        app.store.save_local();
    }
    if on != app.store.local.mcp_enabled {
        app.store.local.mcp_enabled = on;
        app.store.save_local();
        app.assistants.check = None;
    }
    if recheck {
        app.assistants.check = Some(start_check(exe.clone(), args.clone()));
    }
    if relink {
        app.assistants.confirm_link = true;
        app.assistants.client = Some(0);
    }
    if !on {
        return;
    }

    // ---- one assistant at a time ----
    let default_client = if matches!(desktop, Some(Desktop::NotInstalled) | None) { 1 } else { 0 };
    let mut client = app.assistants.client.unwrap_or(default_client);
    let desktop_json = serde_json::to_string_pretty(&json!({ "mcpServers": { "devtrack": { "command": exe, "args": args } } })).unwrap_or_default();
    let quote = |s: &str| if s.contains(' ') { format!("\"{}\"", s) } else { s.to_string() };
    let claude_code = format!("claude mcp add devtrack -- {} {}", quote(&exe), args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" "));
    let mut link_now = false;

    ui.label(RichText::new(tr("Connect your assistant")).font(medium(15.0)).color(c.text));
    ui.add_space(8.0);
    if let Some(i) = segmented(ui, &["Claude Desktop", "Claude Code", "Other assistants"], client) {
        client = i;
        app.assistants.confirm_link = false;
        app.assistants.link_msg = None;
    }
    app.assistants.client = Some(client);
    ui.add_space(12.0);
    egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 10.0;
        match client {
            0 => {
                let state = desktop.clone();
                match &state {
                    None => {
                        meta(ui, "Claude Desktop isn't available for this system: use Claude Code or another assistant.");
                    }
                    Some(Desktop::NotInstalled) => {
                        state_line(ui, c.text2, &tr("Claude Desktop doesn't seem to be installed on this computer."));
                        meta(ui, "Install it and open it once, then come back here.");
                    }
                    Some(Desktop::Linked) => {
                        state_line(ui, c.green, &tr("✔ Connected to Claude Desktop."));
                        meta(ui, "If DevTrack isn't in Claude Desktop's tools yet, quit Claude Desktop and open it again.");
                    }
                    Some(Desktop::Unreadable(e)) => {
                        state_line(ui, c.red, &trf("Claude Desktop's configuration can't be read ({}). DevTrack leaves it alone: fix it, or set it up by hand below.", &[e]));
                    }
                    Some(Desktop::NotLinked) | Some(Desktop::OldPath(_)) => {
                        let path = desktop_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                        if app.assistants.confirm_link {
                            ui.label(RichText::new(tr("Add DevTrack to Claude Desktop?")).font(medium(14.0)).color(c.text));
                            ui.add(egui::Label::new(mixed_job(&trf("DevTrack adds only its own “devtrack” entry to {} and leaves the rest as it is. A copy of the file is kept next to it first.", &[&path]), 13.0, c.text2, sans(13.0), mono_font(13.0))).wrap());
                            ui.horizontal(|ui| {
                                link_now = primary(ui, "Add").clicked();
                                if secondary(ui, "Cancel").clicked() {
                                    app.assistants.confirm_link = false;
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                let w = (ui.available_width() - 220.0).max(160.0);
                                ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(tr("One click, then restart Claude Desktop")).font(sans(14.0)).color(c.text));
                                        ui.add(egui::Label::new(mixed_job(&tr("DevTrack adds itself to Claude Desktop's configuration, after keeping a copy of it."), 12.0, c.text2, sans(12.0), mono_font(12.0))).wrap());
                                    });
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if primary(ui, "Add to Claude Desktop").clicked() {
                                        app.assistants.confirm_link = true;
                                        app.assistants.link_msg = None;
                                    }
                                });
                            });
                        }
                    }
                }
                if let Some((ok, m)) = app.assistants.link_msg.clone() {
                    state_line(ui, if ok { c.green } else { c.red }, &m);
                }
                if state.is_some() {
                    disclosure(ui, "assistants-desktop-manual", "Set it up by hand", false, |ui| {
                        let path = desktop_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                        meta(ui, trf("Add this to {} (merge it with the servers already there), then restart Claude Desktop.", &[&path]));
                        ui.add_space(4.0);
                        code_box(app, ui, "desktop", &desktop_json);
                    });
                }
            }
            1 => {
                ui.label(RichText::new(tr("Run this once in a terminal")).font(sans(14.0)).color(c.text));
                code_box(app, ui, "code", &claude_code);
                meta(ui, "Then, in Claude Code, /mcp shows DevTrack among the connected servers.");
            }
            _ => {
                meta(ui, "For Codex CLI, Cursor and any assistant that starts an MCP server on its own, over stdio.");
                ui.label(RichText::new(tr("Command")).font(sans(13.0)).color(c.text2));
                code_box(app, ui, "command", &exe);
                ui.label(RichText::new(tr("Arguments")).font(sans(13.0)).color(c.text2));
                code_box(app, ui, "args", &args.join(" "));
            }
        }
    });
    if link_now {
        if let Some(path) = &desktop_path {
            app.assistants.confirm_link = false;
            app.assistants.link_msg = Some(match link_desktop(path, &exe, &args) {
                Ok(Some(b)) => (true, trf("Done. Restart Claude Desktop to see DevTrack. The previous file is kept as {}.", &[&b.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()])),
                Ok(None) => (true, tr("Done. Restart Claude Desktop to see DevTrack.")),
                Err(e) => (false, e),
            });
        }
    }

    // ---- things to ask ----
    ui.add_space(28.0);
    let now = devtrack_core::store::now_secs();
    let today = devtrack_core::report::today();
    let busiest = facts::PeriodSpec::Range { from: today - chrono::Duration::days(29), to: today }
        .resolve(today, app.store.db.settings.week_starts_monday)
        .ok()
        .and_then(|p| facts::time_summary(&app.store.db, &p, now, &facts::Filter::default(), &[facts::Dim::Project]).ok())
        .and_then(|s| s.rows.into_iter().map(|r| r.keys[0].label.clone()).find(|l| l != facts::NO_PROJECT));
    let examples = [
        tr("How many hours did I track last week, by client and project?"),
        match &busiest {
            Some(p) => trf("Prepare the weekly report for the PM of {}.", &[p]),
            None => tr("Prepare my weekly report for the PM."),
        },
        tr("Summarize last month by client, anonymized, to send outside the team."),
    ];
    settings_group(ui, "Try asking", Some("In Claude Desktop, the + menu also lists three ready-made reports: weekly, project status and client summary."), |rows| {
        for (i, ex) in examples.iter().enumerate() {
            rows.block(|ui| {
                code_like_example(app, ui, &format!("example-{i}"), ex);
            });
        }
    });
}

/// An example question: plain text (it is what the person types), with the copy button.
fn code_like_example(app: &mut App, ui: &mut egui::Ui, key: &str, text: &str) {
    let c = p(ui);
    let now = ui.input(|i| i.time);
    let just = app.assistants.copied.as_ref().is_some_and(|(k, t)| k == key && now - t < 1.6);
    ui.horizontal(|ui| {
        let w = (ui.available_width() - CTRL_H - 12.0).max(120.0);
        ui.allocate_ui(egui::vec2(w, CTRL_H), |ui| {
            ui.set_min_height(CTRL_H);
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new(format!("“{}”", text)).font(sans(14.0)).color(c.text)).wrap());
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if icon_button(ui, if just { Glyph::Check } else { Glyph::Copy }, if just { "Copied" } else { "Copy" }).clicked() {
                ui.ctx().copy_text(text.to_string());
                app.assistants.copied = Some((key.to_string(), now));
            }
        });
    });
    if just {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("devtrack-claude-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn linking_keeps_everything_else_and_a_copy() {
        let d = dir("link");
        let path = d.join("claude_desktop_config.json");
        std::fs::write(&path, r#"{ "theme": "dark", "mcpServers": { "files": { "command": "npx", "args": ["fs"] } } }"#).unwrap();
        let args = vec!["mcp".to_string()];
        assert_eq!(desktop_state(&path, "/A/DevTrack", &args), Desktop::NotLinked);
        let backup = link_desktop(&path, "/A/DevTrack", &args).unwrap().expect("a copy");
        assert!(std::fs::read_to_string(&backup).unwrap().contains("\"files\""));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["mcpServers"]["files"]["command"], "npx");
        assert_eq!(v["mcpServers"]["devtrack"]["command"], "/A/DevTrack");
        assert_eq!(desktop_state(&path, "/A/DevTrack", &args), Desktop::Linked);
        // the app moved: the entry points elsewhere
        assert_eq!(desktop_state(&path, "/B/DevTrack", &args), Desktop::OldPath("/A/DevTrack".into()));
        link_desktop(&path, "/B/DevTrack", &args).unwrap();
        assert_eq!(desktop_state(&path, "/B/DevTrack", &args), Desktop::Linked);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_broken_file_is_never_touched() {
        let d = dir("broken");
        let path = d.join("claude_desktop_config.json");
        std::fs::write(&path, "{ not json").unwrap();
        let args = vec!["mcp".to_string()];
        assert!(matches!(desktop_state(&path, "/A", &args), Desktop::Unreadable(_)));
        assert!(link_desktop(&path, "/A", &args).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "no copy, no new file");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn no_folder_means_not_installed_and_a_missing_file_is_created() {
        let d = dir("fresh");
        assert_eq!(desktop_state(&d.join("nope").join("claude_desktop_config.json"), "/A", &[]), Desktop::NotInstalled);
        let path = d.join("claude_desktop_config.json");
        assert_eq!(desktop_state(&path, "/A", &[]), Desktop::NotLinked);
        assert_eq!(link_desktop(&path, "/A", &["mcp".into()]).unwrap(), None);
        assert_eq!(desktop_state(&path, "/A", &["mcp".into()]), Desktop::Linked);
        let _ = std::fs::remove_dir_all(&d);
    }
}
