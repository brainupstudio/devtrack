//! Settings in a bounded column, in six tabs by what the person wants to do: General (startup,
//! appearance, keyboard), Time & days, Platforms, Connections (short setup, state and recovery;
//! HTTP details only under Diagnostics), Sync and Data. Settings are rows in titled panels:
//! label and help on the left, the control on the right (`settings_group`, `SettingRows`).

use super::jobs::JobOut;
use super::i18n::{plural, tr, trf};
use super::i18n::trf_m;
use super::widgets::*;
use super::{page_header, App, Page};
use devtrack_core::integrations::{self, azure::normalize_org, org_of, request_log};
use devtrack_core::model::*;
use devtrack_core::secrets;
use devtrack_core::store::{OpenStatus, Store};
use devtrack_core::timefmt::format_duration;
use eframe::egui::{self, RichText};
use std::collections::BTreeMap;
use std::path::PathBuf;

const TOKEN_HELP: &str = "https://learn.microsoft.com/azure/devops/organizations/accounts/use-personal-access-tokens-to-authenticate";
/// The settings column: the whole width the page gives it. Tabs, titles and panels share it, so
/// every edge lines up.
const COL_MIN_W: f32 = 320.0;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tab {
    /// Startup, theme, language, keyboard.
    #[default]
    Display,
    /// How time is read, the working week, idle time and reminders.
    Time,
    Conversions,
    Sync,
    Connections,
    Data,
}

#[derive(Default)]
pub struct SettingsState {
    /// Which period "Export to a CSV file" writes: 0 this week, 1 last week, 2 this month, 3 last month.
    pub export_period: u8,
    /// The lock date as typed, before it is a valid day.
    pub lock_text: String,
    /// The conversion whose removal is being confirmed.
    pub confirm_conversion: Option<String>,
    pub tab: Tab,
    /// A folder picked but not saved yet (reviewed before Save).
    pub pending_folder: Option<PathBuf>,
    pub device: String,
    pub new_org: String,
    pub new_token: String,
    pub token_edit: BTreeMap<Id, String>,
    pub confirm_disconnect: Option<Id>,
    pub org_error: Option<String>,
    pub token_error: Option<String>,
    pub msg: Option<(bool, String)>,
    pub adding: bool,
    pub attempted: bool,
    /// Import from another time-tracking service: its address and the token as typed (never
    /// saved), the days to read, the request in
    /// flight and the entries read, waiting for the person to confirm.
    pub import_address: String,
    pub import_token: String,
    pub import_days: u32,
    pub import_job: Option<std::sync::Arc<std::sync::Mutex<Option<Result<Vec<devtrack_core::integrations::entries_import::ImportedEntry>, String>>>>>,
    pub import_read: Option<Vec<devtrack_core::integrations::entries_import::ImportedEntry>>,
    pub import_error: Option<String>,
}

impl SettingsState {
    pub fn from(store: &Store) -> Self {
        SettingsState { device: store.local.device_name.clone(), ..Default::default() }
    }
    /// Typed or picked but not saved yet.
    pub fn has_input(&self) -> bool {
        self.pending_folder.is_some() || !self.import_token.trim().is_empty() || !self.import_address.trim().is_empty() || !self.new_org.trim().is_empty() || !self.new_token.trim().is_empty() || self.token_edit.values().any(|v| !v.trim().is_empty())
    }
}

/// Cloud-drive folders found on this computer: real directories only, no hidden entries.
fn suggestions() -> Vec<(String, PathBuf)> {
    let mut v = vec![];
    if let Some(home) = dirs::home_dir() {
        let icloud = home.join("Library/Mobile Documents/com~apple~CloudDocs");
        if icloud.is_dir() {
            v.push(("iCloud Drive".into(), icloud));
        }
        for name in ["OneDrive", "Dropbox", "Google Drive"] {
            let p = home.join(name);
            if p.is_dir() {
                v.push((name.into(), p));
            }
        }
        if let Ok(rd) = std::fs::read_dir(home.join("Library/CloudStorage")) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with('.') || !e.path().is_dir() {
                    continue;
                }
                // "GoogleDrive-name@example.com" → "Google Drive": an e-mail stays off screen, a
                // plain account name ("SynologyDrive-nas") tells two folders of one service apart
                let (service, account) = n.split_once('-').unwrap_or((&n, ""));
                let service = match service {
                    "GoogleDrive" => "Google Drive",
                    "SynologyDrive" => "Synology Drive",
                    "OneDrive" => "OneDrive",
                    "Dropbox" => "Dropbox",
                    "Box" => "Box",
                    other => other,
                };
                let label = if !account.is_empty() && !account.contains('@') { format!("{} · {}", service, account) } else { service.to_string() };
                v.push((label, e.path()));
            }
        }
    }
    // the same folder reached two ways, or two accounts of one service under the same name
    let mut seen_paths = std::collections::BTreeSet::new();
    v.retain(|(_, p)| seen_paths.insert(std::fs::canonicalize(p).unwrap_or_else(|_| p.clone())));
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    for (n, _) in v.iter_mut() {
        let k = count.entry(n.clone()).or_insert(0);
        *k += 1;
        if *k > 1 {
            *n = format!("{} ({})", n, k);
        }
    }
    v
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    page_header(ui, "Settings", "", |_| {});
    // by what the person wants to do: the app itself, time, where hours go, where work comes
    // from, other devices, the data
    let tabs = [Tab::Display, Tab::Time, Tab::Conversions, Tab::Connections, Tab::Sync, Tab::Data];
    let names = ["General", "Time & days", "Platforms", "Connections", "Sync", "Data"];
    let cur = tabs.iter().position(|t| *t == app.settings.tab).unwrap_or(0);
    let full_w = ui.available_width();
    let narrow = full_w < 720.0;
    let picked = if narrow {
        // six names don't fit a narrow window as segments
        let opts: Vec<(usize, String)> = names.iter().enumerate().map(|(i, n)| (i, tr(n))).collect();
        let mut v = cur;
        if select(ui, "settings-tab", &mut v, &opts, ui.available_width().min(320.0), false) { Some(v) } else { None }
    } else {
        // the tab bar spans the same width as the panels below it, in equal segments
        let seg_w = (full_w - super::period::SEG_PAD * 2.0) / names.len() as f32;
        super::period::segmented_equal(ui, &names, cur, seg_w, &[true; 6]).0
    };
    if let Some(i) = picked {
        app.settings.tab = tabs[i];
        app.fade_content();
    }
    ui.add_space(24.0 - ui.spacing().item_spacing.y);
    let list = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let w = ui.available_width().max(COL_MIN_W);
        col(ui, w, 0.0, |ui| {
            ui.set_width(w);
            match app.settings.tab {
                Tab::Display => general(app, ui),
                Tab::Time => time_and_days(app, ui),
                Tab::Conversions => conversions(app, ui),
                Tab::Sync => sync(app, ui),
                Tab::Connections => connections(app, ui),
                Tab::Data => data(app, ui),
            }
        });
    });
    scroll_fades(ui, &list, p(ui).canvas);
}

/// Which version this is, whether a newer one is out, and whether to ask at start.
fn updates(app: &mut App, ui: &mut egui::Ui) {
    use devtrack_core::update::DOWNLOAD_PAGE;
    let available = app.update_available();
    let checked = app.store.local.update_checked_ms;
    let ready = available.clone().filter(|v| app.update.ready.as_deref() == Some(v.as_str()));
    let downloading = available.clone().filter(|v| app.update.downloading() == Some(v.as_str()));
    let status = if let Some(v) = &ready {
        trf("DevTrack {} is ready: it installs when you quit DevTrack, or now with Restart and update. Your data stays where it is.", &[v])
    } else if let Some(v) = &downloading {
        trf("Downloading DevTrack {}…", &[v])
    } else if app.update.checking() {
        tr("Checking…")
    } else if let Some(v) = &available {
        match &app.update.install_error {
            Some(e) => format!("{} {}", trf("DevTrack {} is out, but it could not be installed on its own: download it and install it over this one.", &[v]), tr(e)),
            None => trf("DevTrack {} is out: download it and install it over this one. Your data stays where it is.", &[v]),
        }
    } else if let (true, Some(e)) = (app.update.asked, &app.update.error) {
        tr(e)
    } else if checked > 0 {
        let when = chrono::DateTime::from_timestamp_millis(checked).map(|d| d.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()).unwrap_or_default();
        trf("This is the latest version · checked {}", &[&when])
    } else {
        tr("Not checked yet.")
    };
    let mut auto = app.store.local.update_check;
    let mut check_now = false;
    let mut restart = false;
    let checking = app.update.checking();
    let current = devtrack_core::update::current();
    settings_group(ui, "Updates", None, |rows| {
        rows.row(&trf("DevTrack {}", &[&current]), Some(&status), 180.0, |ui| {
            if ready.is_some() {
                restart = primary(ui, "Restart and update").clicked();
            } else if downloading.is_some() {
                ui.add_enabled_ui(false, |ui| secondary(ui, "Downloading…"));
            } else if available.is_some() {
                if primary(ui, "Download").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(DOWNLOAD_PAGE));
                }
            } else {
                ui.add_enabled_ui(!checking, |ui| {
                    if secondary(ui, "Check now").clicked() {
                        check_now = true;
                    }
                });
            }
        });
        rows.row("Check when DevTrack opens", Some("Each time it opens, and every 6 hours while it stays open, it asks GitHub for the number of the latest version; a new one is downloaded and installed when you quit DevTrack. Nothing about you or your work is sent."), 40.0, |ui| {
            switch(ui, &mut auto, "Check when DevTrack opens");
        });
    });
    if check_now {
        app.check_update(true);
    }
    if restart {
        app.restart_to_update();
    }
    if auto != app.store.local.update_check {
        app.store.local.update_check = auto;
        app.store.save_local();
    }
}

/// General: how DevTrack starts and stays running, how it looks, and the keyboard.
fn general(app: &mut App, ui: &mut egui::Ui) {
    let mut s = app.store.db.settings.clone();
    #[cfg(target_os = "macos")]
    settings_group(ui, "Startup and menu bar", Some("Closing the window keeps DevTrack running in the menu bar. Left-click its icon to show the window, right-click for the menu; ⌘Q quits."), |rows| {
        let mut shows = app.store.local.tray_shows;
        rows.row("Next to the icon", None, 270.0, |ui| {
            let opts = vec![(0u8, tr("Time of the running session")), (1u8, tr("Today's total")), (2u8, tr("Nothing"))];
            select(ui, "tray-shows", &mut shows, &opts, 270.0, false);
        });
        if shows != app.store.local.tray_shows {
            app.store.local.tray_shows = shows;
            app.store.save_local();
        }
        match crate::tray::opens_at_login() {
            Some(on) => {
                let mut v = on;
                let mut changed = false;
                rows.row("Open when the Mac starts", Some("In the background: only the menu bar icon shows."), 40.0, |ui| {
                    changed = switch(ui, &mut v, "Open when the Mac starts").changed();
                });
                if changed {
                    match crate::tray::set_open_at_login(v) {
                        Ok(()) => app.notify(tr(if v { "DevTrack will open at login" } else { "DevTrack will not open at login" })),
                        Err(e) => app.notify(e),
                    }
                }
            }
            None => {
                rows.row("Open when the Mac starts", Some("Available in the app installed in Applications."), 40.0, |ui| {
                    ui.add_enabled_ui(false, |ui| {
                        let mut v = false;
                        switch(ui, &mut v, "Open when the Mac starts");
                    });
                });
            }
        }
    });
    settings_group(ui, "Appearance", None, |rows| {
        rows.row("Theme", Some(if s.follow_system_theme { "Follows the Mac's appearance." } else { "" }).filter(|t| !t.is_empty()), 260.0, |ui| {
            let cur = if s.follow_system_theme { 0 } else if s.dark { 1 } else { 2 };
            if let Some(i) = segmented(ui, &["System", "Dark", "Light"], cur) {
                s.follow_system_theme = i == 0;
                if i > 0 {
                    s.dark = i == 1;
                }
            }
        });
        // stored on this device only; System follows the computer's language
        let mut pref = super::i18n::LangPref::parse(&app.store.local.language);
        let opts = vec![
            (super::i18n::LangPref::System, tr("System")),
            (super::i18n::LangPref::En, "English".to_string()),
            (super::i18n::LangPref::It, "Italiano".to_string()),
        ];
        let mut lang_changed = false;
        rows.row("Language / Lingua", Some("Names, codes and destination formats never change with the language"), 200.0, |ui| {
            lang_changed = select(ui, "ui-language", &mut pref, &opts, 200.0, false);
        });
        if lang_changed {
            app.store.local.language = pref.code().to_string();
            app.store.save_local();
            super::i18n::set_lang(pref.resolve());
        }
        rows.row("Reduce motion", Some("Screens change without a fade."), 40.0, |ui| {
            switch(ui, &mut s.reduce_motion, "Reduce motion");
        });
    });
    let mut on_top = app.store.local.compact_on_top;
    settings_group(ui, "Compact window", None, |rows| {
        rows.row("Keep it above other windows", Some("The timer stays in sight while you work in other apps."), 40.0, |ui| {
            switch(ui, &mut on_top, "Keep it above other windows");
        });
    });
    if on_top != app.store.local.compact_on_top {
        app.store.local.compact_on_top = on_top;
        app.store.save_local();
    }
    updates(app, ui);
    settings_group(ui, "Keyboard shortcuts", None, |rows| {
        let k = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" };
        let (enter, shift) = (tr("Enter"), tr("Shift"));
        for (keys, what) in [
            (format!("{k}1 – {k}4"), "Today, Work, Report Time, Statistics"),
            (format!("{k}{enter}"), "Start or stop the session"),
            (format!("{k}P"), "Pause or resume"),
            ("S".to_string(), "Start or stop, when you're not typing"),
            ("C".to_string(), "Continue the last session, when you're not typing"),
            (format!("{k}B"), "Show or hide the sidebar"),
            (format!("{k}{shift}+M"), "Compact window"),
            ("Esc".to_string(), "Close a dialog"),
        ] {
            // a reference list: compact lines, the keys at the right edge
            rows.block(|ui| {
                row(ui, 24.0, |ui| {
                    ui.label(RichText::new(tr(what)).size(14.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| keycap(ui, &keys));
                });
            });
        }
    });
    if s != app.store.db.settings {
        app.store.put_settings(s);
    }
}

/// Time & days: how time is shown, the working week, idle time and reminders.
fn time_and_days(app: &mut App, ui: &mut egui::Ui) {
    let mut s = app.store.db.settings.clone();
    settings_group(ui, "How time is shown", Some("Only the display changes, never what is stored. Shared by all your devices."), |rows| {
        let opts: Vec<(DurationFormat, String)> = DurationFormat::ALL.iter().map(|f| (*f, format!("{}  ·  {}", tr(f.label()), format_duration(3930, *f)))).collect();
        rows.row("Duration format", Some("For sessions and timers. Totals read as “1 h 05 min”."), 280.0, |ui| {
            select(ui, "dur-format", &mut s.duration_format, &opts, 280.0, false);
        });
        rows.row("Round tracked time to the minute", Some("2:34:22 is shown as 2:34."), 40.0, |ui| {
            switch(ui, &mut s.round_to_minute, "Round tracked time to the minute");
        });
        rows.row("24-hour clock", None, 40.0, |ui| {
            switch(ui, &mut s.clock_24h, "24-hour clock");
        });
    });
    settings_group(ui, "Working week", None, |rows| {
        rows.row("Week starts on Monday", None, 40.0, |ui| {
            switch(ui, &mut s.week_starts_monday, "Week starts on Monday");
        });
        // working hours: half-hour steps; the end always comes after the start
        let times: Vec<(u16, String)> = (0..48u16).map(|i| (i * 30, format!("{:02}:{:02}", i / 2, (i % 2) * 30))).collect();
        rows.row("Working hours", Some("Reminders only come inside them; an empty calendar shows them."), 230.0, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let mut end = s.work_end_min;
            let ends: Vec<(u16, String)> = times.iter().filter(|(m, _)| *m > s.work_start_min).cloned().chain(std::iter::once((24 * 60 - 1, "24:00".to_string()))).collect();
            select(ui, "work-end", &mut end, &ends, 100.0, true);
            ui.label(RichText::new("→").size(14.0));
            let mut start = s.work_start_min;
            select(ui, "work-start", &mut start, &times, 100.0, true);
            s.work_start_min = start;
            s.work_end_min = if end > start { end } else { (start + 60).min(24 * 60 - 1) };
        });
        rows.stacked("Working days", Some("Other days stay out of the calendar, the weekly grid and the chart unless they hold time. Reminders skip them."), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let c = p(ui);
                let order: Vec<u32> = if s.week_starts_monday { (0..7).collect() } else { std::iter::once(6).chain(0..6).collect() };
                let names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
                for i in order {
                    let on = s.work_days & (1 << i) != 0;
                    let label_txt = tr(names[i as usize]);
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(52.0, CTRL_H), egui::Sense::click());
                    let over = resp.hovered();
                    ui.painter().rect(r, RADIUS, if on { c.selected } else if over { c.hover } else { c.field }, egui::Stroke::new(1.0, if on { c.text2 } else { c.border }), egui::StrokeKind::Inside);
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, &label_txt, if on { medium(14.0) } else { sans(14.0) }, if on { c.amber } else { c.text2 });
                    if resp.has_focus() {
                        focus_ring(ui, r);
                    }
                    let n = label_txt.clone();
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, n.clone()));
                    // at least one working day stays on
                    if resp.clicked() && !(on && s.work_days.count_ones() == 1) {
                        s.work_days ^= 1 << i;
                    }
                }
            });
        });
    });
    // sessions: the project a new one gets, and how the calendar snaps
    let projects: Vec<(Option<Id>, String)> = std::iter::once((None, tr("None")))
        .chain(app.store.db.live_projects().into_iter().filter(|p| !p.archived).map(|p| (Some(p.id.clone()), p.name.clone())))
        .collect();
    settings_group(ui, "Sessions", None, |rows| {
        rows.row("Default project", Some("Given to a session started without one, so it doesn't wait for details."), 240.0, |ui| {
            select(ui, "default-project", &mut s.default_project, &projects, 240.0, false);
        });
        rows.row("Calendar step", Some("Sessions snap to it when you create, move or stretch them in the calendar."), 160.0, |ui| {
            let opts: Vec<(u8, String)> = [1u8, 5, 10, 15, 30].iter().map(|m| (*m, trf_m!("{} min", m))).collect();
            select(ui, "calendar-step", &mut s.calendar_step, &opts, 160.0, false);
        });
    });
    let mut idle = app.store.local.idle_minutes;
    let mut remind = app.store.local.remind_minutes;
    settings_group(ui, "Idle time and reminders", Some("On this device only. Idle detection reads how long the keyboard and mouse were still: nothing else."), |rows| {
        rows.row("Ask about idle time", Some("While a session runs: when you come back, keep or discard the time away."), 220.0, |ui| {
            let opts: Vec<(u32, String)> = [0u32, 5, 10, 15, 30].iter().map(|m| (*m, if *m == 0 { tr("Never") } else { trf("after {} min away", &[m]) })).collect();
            select(ui, "idle-minutes", &mut idle, &opts, 220.0, false);
        });
        rows.row("Remind me to track", Some("On working days and hours, when nothing is running or paused."), 220.0, |ui| {
            let opts: Vec<(u32, String)> = [0u32, 15, 30, 60].iter().map(|m| (*m, if *m == 0 { tr("Never") } else { trf("every {} min", &[m]) })).collect();
            select(ui, "remind-minutes", &mut remind, &opts, 220.0, false);
        });
    });
    if idle != app.store.local.idle_minutes || remind != app.store.local.remind_minutes {
        app.store.local.idle_minutes = idle;
        app.store.local.remind_minutes = remind;
        app.store.save_local();
    }
    if s != app.store.db.settings {
        app.store.put_settings(s);
    }
}

/// Conversions: how a tracked duration is written for each platform. Each one has a format and
/// its own rounding; the values appear next to the real time wherever time is listed.
fn conversions(app: &mut App, ui: &mut egui::Ui) {
    use devtrack_core::model::TimesheetProfile;
    section(ui, "Platforms");
    let w0 = ui.available_width();
    ui.allocate_ui(egui::vec2(w0, 0.0), |ui| {
        ui.set_max_width(w0);
        meta(ui, "A platform is a system where you report your hours. Its conversion shows the tracked time the way it writes it, next to the real time on Today and in projects; the tracked time never changes. In Report Time the same platform is a destination, with each project's code and the lines to copy.");
    });
    ui.add_space(GROUP_GAP - ROW_GAP);
    let profiles: Vec<TimesheetProfile> = app.store.db.live_profiles().into_iter().cloned().collect();
    let mut open_codes: Option<TimesheetProfile> = None;
    for orig in &profiles {
        let mut pr = orig.clone();
        let mut remove = false;
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            let iw = ui.available_width();
            field(ui, "Name", None, None, |ui| input(ui, &mut pr.name, Field::new("Name", (iw - ROW_GAP) / 2.0).hint("e.g. Company timesheet")));
            ui.add_space(GROUP_GAP - ROW_GAP);
            super::dest::conversion_editor(ui, &mut pr, &format!("conv-{}", orig.id), iw);
            ui.add_space(ROW_GAP);
            row_wrapped(ui, CTRL_H, |ui| {
                // the platform's other half lives in Report Time: codes and lines
                if link_btn(ui, "Project codes…").on_hover_text(tr("The code each project has in this platform, in Report Time")).clicked() {
                    open_codes = Some(orig.clone());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.settings.confirm_conversion.as_deref() == Some(pr.id.as_str()) {
                        if danger(ui, "Remove platform").clicked() { remove = true; }
                        if ghost(ui, "Keep").clicked() { app.settings.confirm_conversion = None; }
                    } else if ghost(ui, "Remove…").clicked() {
                        app.settings.confirm_conversion = Some(pr.id.clone());
                    }
                });
            });
        });
        ui.add_space(ROW_GAP);
        if remove {
            pr.deleted = true;
            app.settings.confirm_conversion = None;
            app.store.put_profile(pr);
            app.notify(tr("Platform removed"));
        } else if pr != *orig {
            app.store.put_profile(pr);
        }
    }
    if profiles.is_empty() {
        muted(ui, "No platforms yet. Add one for each system you report your hours into.");
        ui.add_space(ROW_GAP);
    }
    // one way to add a platform: the same dialog as Report Time's "+ Add destination"
    if secondary(ui, "+ Add platform").clicked() {
        app.remember_focus(ui.ctx());
        app.dest = Some(super::dest::DestDraft::new_destination(app));
    }
    if let Some(pr) = open_codes {
        app.remember_focus(ui.ctx());
        app.dest = Some(super::dest::DestDraft::edit(app, &pr, super::dest::Step::Codes));
    }
}

fn status_row(ui: &mut egui::Ui, k: &str, v: &str, color: Option<egui::Color32>) {
    row(ui, 20.0, |ui| {
        ui.allocate_ui_with_layout(egui::vec2(190.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_width(190.0);
            meta(ui, k);
        });
        mixed(ui, v, 13.0, color);
    });
}

fn sync(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let mut dev_saved = false;
    let current = app.store.local.sync_folder.clone();
    settings_group(ui, "Sync between your devices", Some("Use a folder your devices already share (iCloud Drive, OneDrive, Dropbox). Each device keeps its own file there; company systems are never contacted."), |rows| {
    rows.row("This device's name", Some("How the other devices see this one."), 240.0, |ui| {
        let dev = input(ui, &mut app.settings.device, Field::new("This device's name", 240.0));
        dev_saved = dev.lost_focus();
    });
    rows.stacked("Shared folder", Some("Choose “Not used” to stop syncing; the folder itself is never changed."), |ui| {
    let w = ui.available_width();
    // one menu: no sync, each cloud folder found on this computer, or any other folder. Preview
    // pictures never show this computer's folders.
    let sug = if app.store.is_persistent() { suggestions() } else { vec![] };
    let mut opts: Vec<(usize, String)> = vec![(0, tr("Not used: this device only"))];
    for (i, (name, _)) in sug.iter().enumerate() {
        opts.push((i + 1, name.clone()));
    }
    let custom = current.as_ref().filter(|p| !sug.iter().any(|(_, s)| s == *p)).cloned();
    let custom_i = sug.len() + 1;
    if let Some(p) = &custom {
        opts.push((custom_i, p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string())));
    }
    let other_i = sug.len() + 2;
    opts.push((other_i, tr("Other folder…")));
    let shown = app.settings.pending_folder.clone().or(current.clone());
    let mut pick = match &shown {
        None => 0,
        Some(p) => sug.iter().position(|(_, s)| s == p).map(|i| i + 1).unwrap_or(custom_i),
    };
    let before = pick;
    select(ui, "sync-folder", &mut pick, &opts, w.min(420.0), false);
    // where the chosen folder is, in full: what the menu names, the path proves
    if let Some(p) = &shown {
        ui.label(mono(p.display().to_string(), 12.0).color(c.text2));
    }
    if pick != before {
        if pick == 0 {
            app.settings.pending_folder = None;
            if current.is_some() {
                // keeps everything on this device; the folder isn't changed
                app.store.local.sync_folder = None;
                app.store.save_local();
                app.sync.result = None;
                app.notify(tr("Sync stopped on this device; the folder wasn't changed"));
            }
        } else if pick == other_i {
            if let Some(p) = rfd::FileDialog::new().set_title("Choose a folder your devices share").pick_folder() {
                app.settings.pending_folder = Some(p);
            }
        } else if let Some((_, p)) = sug.get(pick - 1) {
            app.settings.pending_folder = (Some(p) != current.as_ref()).then(|| p.clone());
        }
    }
    // 2. review, 3. save
    if let Some(pf) = app.settings.pending_folder.clone() {
        ui.add_space(ROW_GAP);
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("Review the folder")).font(medium(14.0)));
            ui.label(mono(pf.display().to_string(), 13.0));
            let ok = pf.is_dir();
            if ok {
                meta(ui, "DevTrack will write one file for this device there and read the other devices' files. Nothing else in the folder changes.");
            } else {
                ui.label(RichText::new(tr("That folder doesn't exist or can't be reached right now.")).size(12.0).color(c.red));
            }
            ui.add_space(ROW_GAP);
            ui.horizontal(|ui| {
                if button_w(ui, Kind::Primary, "Save", 0.0, ok).clicked() && ok {
                    app.store.local.sync_folder = Some(pf.clone());
                    app.store.save_local();
                    app.settings.pending_folder = None;
                    app.sync_now();
                    app.notify(tr("Sync folder saved"));
                }
                if secondary(ui, "Cancel").clicked() {
                    app.settings.pending_folder = None;
                }
            });
        });
    }
    });
    });
    if dev_saved && app.settings.device.trim() != app.store.local.device_name && !app.settings.device.trim().is_empty() {
        app.store.local.device_name = app.settings.device.trim().to_string();
        app.store.save_local();
        app.notify(tr("Device name saved"));
    }
    settings_group(ui, "Status", None, |rows| { rows.block(|ui| {
    // saving locally and delivering to other devices are separate facts
    // where the data is, in plain words: this computer, then the others
    let here = if cfg!(target_os = "macos") { "On this Mac" } else { "On this device" };
    match (&app.store.blocked, &app.store.save_error) {
        (Some(b), _) => status_row(ui, here, &trf_m!("Blocked: {}", b), Some(c.red)),
        (None, Some(e)) => status_row(ui, here, &trf_m!("⚠ Last save failed: {}", e), Some(c.amber)),
        (None, None) => status_row(ui, here, "✔ Saved automatically", None),
    }
    match (&app.store.local.sync_folder, &app.sync.result) {
        (None, _) => status_row(ui, "On your other devices", "Not synced: choose a shared folder above", Some(c.text2)),
        (Some(_), None) => status_row(ui, "On your other devices", "Waiting for the first sync with the folder", Some(c.text2)),
        (Some(_), Some(Ok(o))) => status_row(ui, "On your other devices", &trf_m!("✔ Synced {} · {} change(s) received", ago(o.at_ms), o.received), None),
        (Some(_), Some(Err(e))) => status_row(ui, "On your other devices", &trf_m!("⚠ Not synced: {}. Your changes are kept here and sent later.", e), Some(c.amber)),
    }
    if let Some(Ok(o)) = &app.sync.result {
        if o.devices.is_empty() {
            status_row(ui, "Other devices", "No other device has used the folder yet", Some(c.text2));
        }
        for d in &o.devices {
            let (v, col) = if d.has_our_changes { (trf_m!("✔ Has your latest changes (active {})", ago(d.last_seen_ms)), None) } else { (trf_m!("Not yet updated (active {})", ago(d.last_seen_ms)), Some(c.amber)) };
            status_row(ui, &format!("“{}”", truncate(&d.name, 22)), &v, col);
        }
    }
    }); });
    disclosure(ui, "settings-technical", "Technical details", false, |ui| {
        ui.horizontal(|ui| {
            meta(ui, "Device id");
            ui.label(mono(&app.store.local.device_id, 12.0));
        });
        meta(ui, "Conflicts keep the newer edit; a second running timer is stopped where the newer one started.");
        if app.sync.events.is_empty() {
            meta(ui, "No sync events this session.");
        }
        for (at, e) in app.sync.events.iter().rev().take(20) {
            mixed(ui, &format!("{} · {}", ago(*at), e), 12.0, None);
        }
    });
}

fn test_job(app: &mut App, cfg: &IntegrationConfig) {
    let id = cfg.id.clone();
    app.jobs.spawn(cfg, "Testing…", move |x| JobOut::Test { cfg: id, res: x.and_then(|x| x.test()) });
}

fn connections(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let w = ui.available_width();
    let mut conns: Vec<IntegrationConfig> = app.store.db.integrations.values().filter(|i| !i.deleted).cloned().collect();
    conns.sort_by_key(|x| x.name.to_lowercase());
    section(ui, "Azure DevOps");
    meta(ui, "DevTrack only reads work items. It never writes hours or changes anything there.");
    ui.add_space(GROUP_GAP);
    // came here from Activities: offer the way back once a connection works
    if let Some(rt) = app.return_to.clone() {
        let ok = conns.iter().find(|cfg| matches!(app.conn_msgs.get(&cfg.id), Some((true, _))));
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            match ok {
                Some(cfg) => {
                    ui.label(RichText::new(tr("Connected")).font(medium(14.0)).color(c.green));
                    ui.horizontal(|ui| {
                        if primary(ui, "Return to assigned tasks").clicked() {
                            app.act.finder = super::activities::Finder { cfg: Some(cfg.id.clone()), ..Default::default() };
                            app.act.open_add(true);
                            if let Some(pid) = rt.project.clone() { app.act.show_project(pid); }
                            app.store.local.last_connection = Some(cfg.id.clone());
                            app.store.save_local();
                            app.return_to = None;
                            app.go(Page::Activities);
                        }
                        if ghost(ui, "Stay in Settings").clicked() {
                            app.return_to = None;
                        }
                    });
                }
                None => {
                    meta(ui, "After connecting you'll be offered a way back to the assigned tasks.");
                    if link_btn(ui, "Back to Activities").clicked() {
                        app.return_to = None;
                        app.go(Page::Activities);
                    }
                }
            }
        });
        ui.add_space(GROUP_GAP);
    }
    for cfg in &conns {
        let has_token = !app.store.is_persistent() || secrets::has(&cfg.id);
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(org_of(cfg)).font(medium(16.0)));
                match (has_token, app.conn_msgs.get(&cfg.id)) {
                    (false, _) => pill(ui, "Token missing on this device", c.amber),
                    (true, Some((false, _))) => pill(ui, "Needs attention", c.red),
                    (true, Some((true, _))) => pill(ui, "✔ Connected", c.green),
                    (true, None) => pill(ui, "Not tested this session", c.text2),
                }
            });
            if let Some(t) = cfg.last_refresh {
                mixed(ui, &trf_m!("Imported tasks last refreshed {}", ago(t * 1000)), 12.0, Some(c.text2));
            }
            if let Some((ok, m)) = app.conn_msgs.get(&cfg.id) {
                mixed(ui, m, 13.0, Some(if *ok { c.text2 } else { c.red }));
            }
            if let Some(l) = &app.jobs.label {
                ui.horizontal(|ui| {
                    ui.spinner();
                    meta(ui, l.clone());
                });
            }
            ui.add_space(ROW_GAP);
            let iw = (ui.available_width() - 130.0 - ROW_GAP).max(180.0);
            let mut save_tok = false;
            field(ui, if has_token { "Replace token" } else { "Personal access token" }, Some("Stored in this device's keychain"), None, |ui| {
                ui.horizontal(|ui| {
                    let t = app.settings.token_edit.entry(cfg.id.clone()).or_default();
                    input(ui, t, Field::new("Personal access token", iw).password());
                    let tok = t.trim().to_string();
                    if button_w(ui, Kind::Secondary, "Save and test", 130.0, !tok.is_empty()).clicked() && !tok.is_empty() {
                        save_tok = true;
                    }
                });
            });
            if save_tok {
                let tok = app.settings.token_edit.get(&cfg.id).cloned().unwrap_or_default();
                match secrets::set(&cfg.id, tok.trim()) {
                    Ok(()) => {
                        app.settings.token_edit.remove(&cfg.id);
                        test_job(app, cfg);
                    }
                    Err(e) => {
                        app.conn_msgs.insert(cfg.id.clone(), (false, trf_m!("Keychain: {}", e)));
                    }
                }
            }
            ui.add_space(ROW_GAP);
            ui.horizontal(|ui| {
                if has_token && secondary(ui, "Test again").clicked() {
                    test_job(app, cfg);
                }
                if app.settings.confirm_disconnect.as_ref() == Some(&cfg.id) {
                    ui.label(RichText::new(tr("Disconnect? Imported tasks and their history stay.")).size(13.0));
                    if danger(ui, "Disconnect").clicked() {
                        secrets::remove(&cfg.id);
                        let mut x = cfg.clone();
                        x.deleted = true;
                        app.store.put_integration(x);
                        app.settings.confirm_disconnect = None;
                    }
                    if ghost(ui, "Keep").clicked() {
                        app.settings.confirm_disconnect = None;
                    }
                } else if ghost(ui, "Disconnect…").clicked() {
                    app.settings.confirm_disconnect = Some(cfg.id.clone());
                }
            });
        });
        ui.add_space(GROUP_GAP);
    }
    if conns.is_empty() || app.settings.adding {
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            let iw = ui.available_width();
            ui.label(RichText::new(tr(if conns.is_empty() { "Connect your organization" } else { "Add another organization" })).font(medium(16.0)));
            ui.add_space(4.0);
            meta(ui, "1. Create a personal access token with Work Items (Read) and Project and Team (Read).");
            meta(ui, "2. Enter the organization and paste the token. It stays in this device's keychain.");
            ui.hyperlink_to(RichText::new(tr("How to create a token ↗")).size(12.0), TOKEN_HELP);
            ui.add_space(GROUP_GAP - ROW_GAP);
            let oe = if app.settings.attempted { app.settings.org_error.clone() } else { None };
            let o = field(ui, "Organization", Some("Name or address, e.g. contoso or https://dev.azure.com/contoso"), oe.as_deref(), |ui| {
                input(ui, &mut app.settings.new_org, Field::new("Organization", iw).hint("contoso").invalid(oe.is_some()))
            });
            ui.add_space(GROUP_GAP - ROW_GAP);
            let te = if app.settings.attempted { app.settings.token_error.clone() } else { None };
            let t = field(ui, "Personal access token", None, te.as_deref(), |ui| input(ui, &mut app.settings.new_token, Field::new("Personal access token", iw).hint("Paste the token").password().invalid(te.is_some())));
            ui.add_space(GROUP_GAP);
            let mut submit = false;
            ui.horizontal(|ui| {
                if primary(ui, "Connect and test").clicked() {
                    submit = true;
                }
                if !conns.is_empty() && secondary(ui, "Cancel").clicked() {
                    app.settings.adding = false;
                }
            });
            if submit {
                app.settings.attempted = true;
                app.settings.org_error = if normalize_org(&app.settings.new_org).is_none() { Some("Enter the organization name".into()) } else { None };
                app.settings.token_error = if app.settings.new_token.trim().is_empty() { Some("Paste a personal access token".into()) } else { None };
                // focus the first invalid field
                if app.settings.org_error.is_some() {
                    o.request_focus();
                } else if app.settings.token_error.is_some() {
                    t.request_focus();
                }
                if let (None, None, Some(org)) = (&app.settings.org_error, &app.settings.token_error, normalize_org(&app.settings.new_org)) {
                    let mut cfg = IntegrationConfig { id: new_id(), kind: SOURCE_AZURE.into(), name: trf_m!("Azure DevOps · {}", org), enabled: true, ..Default::default() };
                    cfg.settings.insert("organization".into(), org);
                    match secrets::set(&cfg.id, app.settings.new_token.trim()) {
                        Ok(()) => {
                            app.settings.new_token.clear();
                            app.settings.new_org.clear();
                            app.settings.adding = false;
                            app.settings.attempted = false;
                            app.store.put_integration(cfg.clone());
                            app.store.local.last_connection = Some(cfg.id.clone());
                            app.store.save_local();
                            test_job(app, &cfg);
                            app.settings.msg = None;
                        }
                        Err(e) => app.settings.msg = Some((false, trf_m!("Couldn't save the token in the system keychain: {}", e))),
                    }
                }
            }
            if let Some((ok, m)) = &app.settings.msg {
                ui.label(RichText::new(m).size(12.0).color(if *ok { c.text2 } else { c.red }));
            }
        });
    } else if link_btn(ui, "+ Add another organization").clicked() {
        app.settings.adding = true;
    }
    ui.add_space(GROUP_GAP);
    let _ = w;
    disclosure(ui, "settings-diagnostics", "Diagnostics", false, |ui| {
        meta(ui, "Only GET requests and the read-only work-item query (POST …/_apis/wit/wiql) are allowed; anything else is refused before it leaves this computer.");
        let log = request_log();
        if log.is_empty() {
            meta(ui, "No requests sent this session.");
        }
        for l in log.iter().rev().take(25) {
            ui.label(mono(l, 12.0));
        }
        let later: Vec<&str> = integrations::KINDS.iter().filter(|k| !k.available).map(|k| k.name).collect();
        if !later.is_empty() {
            meta(ui, trf_m!("Planned later: {}.", later.join(", ")));
        }
    });
}

/// Data: where it lives and how to take it out, the lock on reported days, and sessions to check.
fn data(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let status = match &app.store.status {
        OpenStatus::InMemory => tr("Preview: nothing is saved"),
        OpenStatus::FirstLaunch | OpenStatus::Opened => tr("Saved on this device; the previous version is kept on every save"),
        OpenStatus::Unreadable(e) => trf_m!("The data file can't be read: {}", e),
        OpenStatus::Damaged { error, .. } => trf_m!("The data file was damaged: {}", error),
    };
    let save_error = app.store.save_error.clone();
    let mut copy = false;
    let mut export_file = false;
    let mut export_period = app.settings.export_period;
    let mut sample = false;
    let can_sample = app.store.is_persistent() && app.store.blocked.is_none();
    let dir = app.store.dir();
    settings_group(ui, "Your data", Some(&status), |rows| {
        if let Some(e) = &save_error {
            rows.block(|ui| {
                ui.label(RichText::new(trf_m!("Couldn't save: {}", e)).color(c.red));
            });
        }
        rows.row("Export all sessions", Some("As CSV, copied to the clipboard."), 160.0, |ui| {
            copy = secondary(ui, "Copy as CSV").clicked();
        });
        // a period as a file: the sessions, with a column for each conversion
        let opts = vec![(0u8, tr("This week")), (1u8, tr("Last week")), (2u8, tr("This month")), (3u8, tr("Last month"))];
        rows.row("Export a period to a CSV file", Some("The sessions of that period, with a column for each conversion."), 160.0 + ROW_GAP + 170.0, |ui| {
            export_file = secondary(ui, "Export…").clicked();
            ui.add_space(ROW_GAP);
            select(ui, "export-period", &mut export_period, &opts, 170.0, false);
        });
        match &dir {
            Some(d) => {
                let path = d.display().to_string();
                rows.row("Data folder", Some(&path), 160.0, |ui| {
                    if secondary(ui, "Show in Finder").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!("file://{}", path)));
                    }
                });
            }
            None => rows.row("Data folder", Some("Preview mode: no folder."), 0.0, |_| {}),
        }
        if can_sample {
            rows.row("Sample data", Some("Adds example projects, tasks, sessions and two destinations"), 160.0, |ui| {
                sample = secondary(ui, "Add sample data").clicked();
            });
        }
    });
    app.settings.export_period = export_period;
    if export_file {
        use devtrack_core::report::{today, Period, PeriodKind};
        let monday = app.store.db.settings.week_starts_monday;
        let now = today();
        let period = match export_period {
            0 => Period::containing(PeriodKind::Week, now, monday),
            1 => Period::containing(PeriodKind::Week, now - chrono::Duration::days(7), monday),
            2 => Period::containing(PeriodKind::Month, now, monday),
            _ => Period::containing(PeriodKind::Month, Period::containing(PeriodKind::Month, now, monday).start - chrono::Duration::days(1), monday),
        };
        super::today::export_csv(app, &period);
    }
    if copy {
        ui.ctx().copy_text(csv(&app.store));
        app.notify(tr("Sessions copied as CSV"));
    }
    if sample {
        app.store.load_sample();
        app.notify(tr("Sample data added"));
    }
    entries_import(app, ui);
    // sessions whose description is really their task (imported or typed before tasks came first)
    let (new_tasks, changed) = devtrack_core::integrations::entries_import::descriptions_to_tasks(&app.store.db);
    if !changed.is_empty() {
        let mut convert = false;
        settings_group(ui, "Descriptions into tasks", Some("The task now says what a session was; the description becomes an optional note. Sessions that only have a description get the task of that name in their project (made when missing), or the Azure DevOps work item their description starts with."), |rows| {
            let help = trf_m!("{} · {}", plural(changed.len() as i64, "{} session", "{} sessions"), plural(new_tasks.len() as i64, "{} new task", "{} new tasks"));
            rows.row("Turn descriptions into tasks", Some(&help), 200.0, |ui| {
                convert = secondary(ui, "Convert").clicked();
            });
        });
        if convert {
            let n = changed.len();
            for t in new_tasks { app.store.put_task(t); }
            for e in changed { app.store.put_entry(e); }
            app.notify(plural(n as i64, "{} session now has its task", "{} sessions now have their task"));
        }
    }
    // the lock on days already reported elsewhere
    let mut s = app.store.db.settings.clone();
    if app.settings.lock_text.is_empty() {
        app.settings.lock_text = s.lock_until.clone().unwrap_or_default();
    }
    settings_group(ui, "Lock", Some("Sessions that start on or before this day can't be changed or deleted, here or by mistake. Useful once the time has been entered elsewhere."), |rows| {
        rows.stacked("Lock sessions up to", Some(if s.lock_until.is_some() { "On." } else { "Off: every session can be changed." }), |ui| {
            row(ui, CTRL_H, |ui| {
                let monday = s.week_starts_monday;
                date_input(ui, &mut app.settings.lock_text, "Lock sessions up to", 200.0, false, monday).on_hover_text("YYYY-MM-DD");
                if secondary(ui, "Up to yesterday").clicked() {
                    app.settings.lock_text = (devtrack_core::report::today() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
                }
                if s.lock_until.is_some() && ghost(ui, "Remove the lock").clicked() {
                    app.settings.lock_text.clear();
                    s.lock_until = None;
                }
            });
        });
    });
    if chrono::NaiveDate::parse_from_str(app.settings.lock_text.trim(), "%Y-%m-%d").is_ok() {
        s.lock_until = Some(app.settings.lock_text.trim().to_string());
    }
    if s != app.store.db.settings {
        app.store.put_settings(s);
    }
    // sessions that may have been stretched by mistake: listed, never changed automatically
    let h24 = app.store.db.settings.clock_24h;
    let mut long: Vec<TimeEntry> = app.store.db.live_entries().filter(|e| e.stop.map(|s| s - e.start > 20 * 3600).unwrap_or(false)).cloned().collect();
    long.sort_by_key(|e| std::cmp::Reverse(e.start));
    let prev = if long.is_empty() { None } else { app.store.read_previous() };
    let mut open: Option<TimeEntry> = None;
    settings_group(ui, "Long sessions", Some("Sessions longer than 20 hours are listed for review. Nothing is changed automatically: some may be real."), |rows| {
        if long.is_empty() {
            rows.block(|ui| { meta(ui, "No sessions longer than 20 hours."); });
        }
        for e in long.iter().take(20) {
            let stop = e.stop.unwrap_or(e.start);
            let title = super::session_title(&app.store.db, e);
            let when = format!("{} {} → {} {} · {}", super::i18n::short_date(devtrack_core::report::local_date(e.start)), clock_text(e.start, h24), super::i18n::short_date(devtrack_core::report::local_date(stop)), clock_text(stop, h24), devtrack_core::timefmt::clock(stop - e.start));
            let note = match prev.as_ref().and_then(|d| d.entries.get(&e.id)) {
                Some(p) if p.stop != e.stop || p.start != e.start => {
                    let ps = p.stop.unwrap_or(p.start);
                    format!("{} · {}", when, trf("previous saved copy: {}", &[&format!("{}–{} ({})", clock_text(p.start, h24), clock_text(ps, h24), devtrack_core::timefmt::clock(ps - p.start))]))
                }
                Some(_) => format!("{} · {}", when, tr("same in the previous saved copy")),
                None => when,
            };
            rows.row(&title, Some(&note), BTN_MIN_W, |ui| {
                if secondary(ui, "Review").clicked() {
                    open = Some(e.clone());
                }
            });
        }
    });
    if let Some(e) = open {
        let ctx = ui.ctx().clone();
        super::today::open_edit(app, &e, Some(&ctx), false);
    }
    if app.store.dir().map(|d| d.join("data.before-clients.json").exists()).unwrap_or(false) {
        meta(ui, "Clients from before this version were created from project texts; a backup of the earlier file is kept.");
    }
}

/// Import from another time-tracking service: read once from the address and API token the
/// person types, review what would change, then write it. Neither is saved; both are cleared
/// after the import.
fn entries_import(app: &mut App, ui: &mut egui::Ui) {
    use devtrack_core::integrations::entries_import as source;
    let c = p(ui);
    // the request runs on its own thread; its answer is picked up here
    if let Some(job) = app.settings.import_job.clone() {
        let done = job.lock().ok().and_then(|mut g| g.take());
        match done {
            Some(Ok(entries)) => {
                app.settings.import_read = Some(entries);
                app.settings.import_error = None;
                app.settings.import_job = None;
            }
            Some(Err(e)) => {
                app.settings.import_error = Some(e);
                app.settings.import_job = None;
            }
            None => ui.ctx().request_repaint_after(std::time::Duration::from_millis(200)),
        }
    }
    if app.settings.import_days == 0 {
        app.settings.import_days = 30;
    }
    let loading = app.settings.import_job.is_some();
    let mut read = false;
    let mut import = false;
    let mut cancel = false;
    let plan = app.settings.import_read.as_ref().map(|e| source::plan(&app.store.db, e));
    settings_group(ui, "Import from another time tracker", Some("Your entries from another time-tracking service, read once with its address and your API token. Neither is saved, and nothing is changed on the service."), |rows| {
        rows.row("Service address", Some("The address of the service's API, as its documentation gives it."), 300.0, |ui| {
            input(ui, &mut app.settings.import_address, Field::new("Service address", 300.0).hint("api.example.com").mono());
        });
        rows.row("API token", Some("You find it in your profile on the service."), 300.0, |ui| {
            input(ui, &mut app.settings.import_token, Field::new("API token", 300.0).hint("Paste the token").password());
        });
        rows.row("Period", Some("Sessions that started in these days, today included."), 200.0, |ui| {
            let opts = vec![(7u32, tr("Last 7 days")), (30u32, tr("Last 30 days")), (90u32, tr("Last 90 days"))];
            select(ui, "import-days", &mut app.settings.import_days, &opts, 200.0, false);
        });
        rows.block(|ui| {
            row(ui, CTRL_H, |ui| {
                ui.spacing_mut().item_spacing.x = GROUP_GAP;
                let ok = !app.settings.import_token.trim().is_empty() && !app.settings.import_address.trim().is_empty() && !loading;
                read = button_w(ui, Kind::Secondary, "Read entries", 0.0, ok).clicked() && ok;
                if loading {
                    ui.spinner();
                    meta(ui, "Reading…");
                }
            });
            if let Some(e) = &app.settings.import_error {
                ui.label(RichText::new(tr(e)).size(12.0).color(c.red));
            }
            if let Some(pl) = &plan {
                ui.add_space(ROW_GAP);
                ui.label(RichText::new(plural(pl.new.len() as i64, "{} new session", "{} new sessions")).font(medium(14.0)));
                if pl.already > 0 {
                    meta(ui, plural(pl.already as i64, "{} already in DevTrack, left as it is", "{} already in DevTrack, left as they are"));
                }
                if !pl.new_projects.is_empty() {
                    let names: Vec<String> = pl.new_projects.iter().map(|(n, _)| n.clone()).collect();
                    meta(ui, trf_m!("New projects: {}", names.join(", ")));
                }
                if !pl.new_clients.is_empty() {
                    meta(ui, trf_m!("New clients: {}", pl.new_clients.join(", ")));
                }
                // each description becomes a task here
                if !pl.new_tasks.is_empty() {
                    meta(ui, plural(pl.new_tasks.len() as i64, "{} new task, from the descriptions", "{} new tasks, from the descriptions"));
                }
                if pl.linked > 0 {
                    meta(ui, plural(pl.linked as i64, "{} session linked to an Azure DevOps work item by its number", "{} sessions linked to Azure DevOps work items by their number"));
                }
                meta(ui, "Projects and clients with the same name as one in DevTrack are reused.");
                ui.add_space(ROW_GAP);
                row(ui, CTRL_H, |ui| {
                    let n = pl.new.len();
                    import = button_w(ui, Kind::Primary, &plural(n as i64, "Import {} session", "Import {} sessions"), 0.0, n > 0).clicked() && n > 0;
                    cancel = ghost(ui, "Cancel").clicked();
                });
            }
        });
    });
    if read {
        let token = app.settings.import_token.clone();
        let address = app.settings.import_address.clone();
        let to = devtrack_core::report::today();
        let from = to - chrono::Duration::days(app.settings.import_days as i64 - 1);
        let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        let out = slot.clone();
        let ctx = ui.ctx().clone();
        app.settings.import_read = None;
        app.settings.import_error = None;
        app.settings.import_job = Some(slot);
        std::thread::spawn(move || {
            let r = source::fetch(&address, &token, &from.format("%Y-%m-%d").to_string(), &to.format("%Y-%m-%d").to_string());
            if let Ok(mut g) = out.lock() {
                *g = Some(r);
            }
            ctx.request_repaint();
        });
    }
    if import {
        if let Some(pl) = plan {
            let color = app.store.db.next_color();
            let (clients, projects, tasks, sessions) = source::apply(&app.store.db, &pl, || color.clone());
            let n = sessions.len();
            for x in clients { app.store.put_client(x); }
            for x in projects { app.store.put_project(x); }
            for x in tasks { app.store.put_task(x); }
            for x in sessions { app.store.put_entry(x); }
            app.notify(plural(n as i64, "{} session imported", "{} sessions imported"));
        }
        cancel = true;
    }
    if cancel {
        app.settings.import_read = None;
        app.settings.import_token.clear();
        app.settings.import_address.clear();
        app.settings.import_error = None;
    }
}

fn csv(store: &Store) -> String {
    let mut out = String::from("date,start,stop,duration_seconds,description,client,project,task,source,external_id,billable\n");
    let mut list: Vec<&TimeEntry> = store.db.live_entries().collect();
    list.sort_by_key(|e| e.start);
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    for e in list {
        let t = store.db.task(&e.task_id);
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            devtrack_core::report::local_date(e.start),
            clock_text(e.start, true),
            e.stop.map(|s| clock_text(s, true)).unwrap_or_default(),
            e.duration(devtrack_core::store::now_secs()),
            q(&e.description),
            q(&store.db.project(&e.project_id).map(|p| store.db.client_name(p)).unwrap_or_default()),
            q(&store.db.project(&e.project_id).map(|p| p.name.clone()).unwrap_or_default()),
            q(&t.map(|t| t.title.clone()).unwrap_or_default()),
            t.map(|t| t.source.clone()).unwrap_or_default(),
            t.and_then(|t| t.external_id.clone()).unwrap_or_default(),
            e.billable
        ));
    }
    out
}

