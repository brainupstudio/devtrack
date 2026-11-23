//! DevTrack desktop app (macOS, Windows, Linux).
//!
//! `--screenshot <file.png>`: open with sample data without taking focus, draw ~120 frames,
//! save the window as a PNG and exit. Nothing is read from or saved to the person's data then.
//! `--state first|active|incomplete|ready` picks the sample state for those pictures
//! (default: incomplete with a running session), `--page <name>` the page to show,
//! `--size WxH` the logical window size and `--light` the light theme.
//! `--demo` opens a separate copy filled with sample data, `--test` a separate copy that starts
//! empty; neither touches the real data.
//! `--background` starts with the window hidden (macOS, used when opening at login).
//! `--rail` collapses the sidebar to its icons for those pictures.
//! `--lang en|it` picks the interface language, `--compact` starts in the compact window, `--reduce-motion` turns the fade off,
//! `--transition-log <file.csv>` records one page transition frame by frame and
//! `--geometry-report <file.json>` writes the measured control rectangles.

// Windows: a release build is a windowed app, with no console window behind it
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
mod tray;
mod ui;

use devtrack_core::sample::{build_state, DemoState};
use devtrack_core::store::{now_secs, Store};
use eframe::egui;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let shot_path = arg(&args, "--screenshot").map(std::path::PathBuf::from);
    let shooting = shot_path.is_some();
    let state = arg(&args, "--state").and_then(|s| DemoState::parse(&s));
    // `--demo`: the persistent sample copy (see below). Ignored with `--state` or `--screenshot`.
    let demo = args.iter().any(|a| a == "--demo");
    // `--test`: a third copy that starts empty and is never filled with samples, to try imports
    // and first-run flows without the demo's data or touching the real data
    let test = !demo && args.iter().any(|a| a == "--test");
    #[cfg(target_os = "macos")]
    let background = args.iter().any(|a| a == "--background");

    let store = if shooting || state.is_some() {
        let st = state.unwrap_or(DemoState::Incomplete);
        let running = matches!(st, DemoState::Active | DemoState::Incomplete);
        let mut s = Store::in_memory(build_state(now_secs(), st, running));
        s.db.settings.dark = !args.iter().any(|a| a == "--light");
        s.db.settings.reduce_motion = args.iter().any(|a| a == "--reduce-motion");
        if let Some(l) = arg(&args, "--lang") {
            s.local.language = l;
        }
        s
    } else if demo {
        // A second, separate copy of the app's data that is kept on disk: sample data to try
        // things on, next to the real folder and never mixed with it. Filled once.
        let dir = devtrack_core::store::default_dir().with_file_name("DevTrack Demo");
        let mut s = Store::open(dir);
        if s.db.entries.is_empty() && s.db.projects.is_empty() {
            let settings = s.db.settings.clone();
            s.db = build_state(now_secs(), DemoState::Varied, false);
            s.db.settings = settings;
            s.save();
        }
        s
    } else if test {
        Store::open(devtrack_core::store::default_dir().with_file_name("DevTrack Test"))
    } else {
        Store::open_default()
    };
    // `--size 1440x900`: logical window size for review pictures
    let size = arg(&args, "--size")
        .and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))))
        .unwrap_or((1440.0, 960.0));

    let mut viewport = egui::ViewportBuilder::default()
        // a preview on sample data says so in its title: it is not the person's own data
        .with_title(if state.is_some() && !shooting { "DevTrack · anteprima con dati di esempio" } else if demo && !shooting { "DevTrack · DEMO (dati di esempio, salvati)" } else if test && !shooting { "DevTrack · PROVA (copia separata)" } else { "DevTrack" })
        .with_inner_size([size.0, size.1])
        .with_min_inner_size(if args.iter().any(|a| a == "--compact") { [380.0, 480.0] } else { [360.0, 480.0] })
        // the graphics context is made once, for this window: asking for transparency here is what
        // lets the compact editor's own window be see-through (rounded). This window stays opaque
        // to the eye: its panels cover it edge to edge.
        .with_transparent(true);
    // macOS: no title bar. The page reaches the top edge and the three window buttons sit over
    // the sidebar; the app draws its own drag areas (see `ui::window_drag`).
    #[cfg(target_os = "macos")]
    {
        viewport = viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false);
    }
    // window and Dock icon: the white mark on a black rounded square, drawn on the macOS grid.
    // A packaged macOS app keeps the icon of its bundle: setting one here would replace the
    // system's own drawing of it in the Dock while DevTrack runs.
    if let Some(icon) = (!in_app_bundle()).then(|| image::load_from_memory(include_bytes!("../assets/icons/app-icon.png")).ok()).flatten() {
        let icon = icon.into_rgba8();
        let (width, height) = icon.dimensions();
        viewport = viewport.with_icon(egui::IconData { rgba: icon.into_raw(), width, height });
    }
    if shooting {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let mut options = eframe::NativeOptions { viewport, persist_window: !shooting, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shooting {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    let shot = shot_path.map(|path| ui::Screenshot { path, frames: 0, requested: false });
    let start_page = arg(&args, "--page");
    let ready = matches!(state, Some(DemoState::Ready | DemoState::Varied));
    let compact = args.iter().any(|a| a == "--compact");
    // `--period day|week|month` and `--date YYYY-MM-DD`: the same period on every page
    let period = arg(&args, "--period");
    let date = arg(&args, "--date").and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());
    let transition_log = arg(&args, "--transition-log").map(std::path::PathBuf::from);
    let geometry_report = arg(&args, "--geometry-report").map(std::path::PathBuf::from);
    eframe::run_native(
        "DevTrack",
        options,
        Box::new(move |cc| {
            // fonts must be registered before the first frame uses them
            ui::widgets::install_fonts(&cc.egui_ctx);
            let mut app = ui::App::new(store, shot);
            // once per data set: descriptions become tasks (the description was the task)
            if !shooting && state.is_none() {
                app.convert_descriptions_once();
            }
            // `--rail`: review pictures with the sidebar collapsed (never saved)
            if shooting && args.iter().any(|a| a == "--rail") {
                app.store.local.sidebar_collapsed = true;
            }
            // sample data says so next to the logo: without a title bar the window title is unseen
            if !shooting {
                app.data_note = if state.is_some() { Some("Preview") } else if demo { Some("Demo") } else if test { Some("Test copy") } else { None };
            }
            // the mark in the menu bar; pictures for review are taken without it
            #[cfg(target_os = "macos")]
            if !shooting {
                tray::install(&cc.egui_ctx);
                // with the item in the menu bar, closing the window keeps DevTrack running;
                // `--background` (opening at login) starts with the window hidden
                app.tray = true;
                app.start_hidden = background;
            }
            if ready {
                app.show_last_working_day();
            }
            if let Some(p) = start_page {
                app.open_page(&p);
            }
            if period.is_some() || date.is_some() {
                app.set_period(period.as_deref().unwrap_or("day"), date);
            }
            if compact {
                app.mode = ui::WinMode::Compact;
            }
            app.transition_log = transition_log.map(|path| ui::TransitionLog { path, frames: vec![], started: None, written: false });
            app.geometry_report = geometry_report;
            Ok(Box::new(app))
        }),
    )
}

/// Whether this program runs from inside a macOS app bundle (".app/Contents/MacOS/").
fn in_app_bundle() -> bool {
    cfg!(target_os = "macos") && std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
}
