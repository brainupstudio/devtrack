//! Component gallery (Settings › Display › Open component gallery): every shared control in
//! each state, in the current theme (switchable here), plus a geometry check that measures the
//! real controls of the Today start row and the Edit entry time row.

use super::i18n::trf_m;
use super::widgets::*;
use super::{page_header, App, Page};
use devtrack_core::model::*;
use eframe::egui::{self, RichText, Stroke};

/// Measured rows kept between frames for the table (label, rect).
#[derive(Clone, Default)]
struct Measured(Vec<(String, egui::Rect)>);

fn demo_text(ui: &mut egui::Ui, key: &str, init: &str) -> String {
    let id = egui::Id::new(("gallery-text", key));
    ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| init.to_string())
}

fn store_text(ui: &mut egui::Ui, key: &str, v: String) {
    let id = egui::Id::new(("gallery-text", key));
    ui.data_mut(|d| d.insert_temp(id, v));
}

fn demo_input(ui: &mut egui::Ui, key: &str, init: &str, f: Field) -> egui::Rect {
    let mut s = demo_text(ui, key, init);
    input(ui, &mut s, f);
    store_text(ui, key, s);
    last_input_rect(ui)
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let mut back = false;
    page_header(ui, "Component gallery", "Shared controls in every state. Outer height 36, radius 6, padding 12, Sans 14 / Mono 14.", |ui| {
        if secondary(ui, "Back to Settings").clicked() {
            back = true;
        }
    });
    if back {
        app.go(Page::Settings);
        return;
    }
    let mut measured: Vec<(String, egui::Rect)> = vec![];
    egui::ScrollArea::vertical().id_salt("gallery").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            label(ui, "Theme");
            let dark = app.store.db.settings.dark;
            if let Some(i) = segmented(ui, &["Dark", "Light"], if dark { 0 } else { 1 }) {
                let mut s = app.store.db.settings.clone();
                s.dark = i == 0;
                app.store.put_settings(s);
            }
        });
        ui.add_space(GROUP_GAP);

        section(ui, "Buttons");
        ui.horizontal(|ui| {
            let r = primary(ui, "Primary");
            measured.push(("button · primary".into(), r.rect));
            let r = secondary(ui, "Secondary");
            measured.push(("button · secondary".into(), r.rect));
            ghost(ui, "Ghost");
            danger(ui, "Danger");
            button_w(ui, Kind::Primary, "Disabled", 0.0, false);
            button_w(ui, Kind::Secondary, "Disabled", 0.0, false);
            let r = icon_button(ui, Glyph::More, "More actions");
            measured.push(("icon button".into(), r.rect));
            icon_button(ui, Glyph::Compact, "Compact window");
            icon_button(ui, Glyph::Expand, "Expand window");
            icon_button(ui, Glyph::Prev, "Previous");
            icon_button(ui, Glyph::Next, "Next");
            icon_button(ui, Glyph::Close, "Close");
        });
        ui.horizontal(|ui| {
            let r = secondary(ui, "Focused");
            ui.painter().rect_stroke(r.rect.expand(2.0), RADIUS + 2.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
            meta(ui, "Focus ring: 2 units outside the control, layout unchanged (static illustration).");
            link_btn(ui, "Inline link");
        });
        ui.add_space(GROUP_GAP + 8.0);

        section(ui, "Text fields");
        ui.horizontal_top(|ui| {
            field(ui, "Default", Some("Helper text, 12 / 6 below"), None, |ui| {
                let r = demo_input(ui, "default", "", Field::new("Default", 220.0).hint("Placeholder is not a label"));
                measured.push(("input · default".into(), r));
            });
            field(ui, "Filled, Mono", None, None, |ui| {
                let r = demo_input(ui, "mono", "1:30", Field::new("Mono", 160.0).mono());
                measured.push(("input · mono".into(), r));
            });
            field(ui, "Invalid", None, Some("Use HH:MM, for example 09:30"), |ui| {
                let r = demo_input(ui, "invalid", "25:80", Field::new("Invalid", 160.0).mono().invalid(true));
                measured.push(("input · invalid".into(), r));
            });
        });
        ui.add_space(GROUP_GAP - ROW_GAP);
        ui.horizontal_top(|ui| {
            field(ui, "Disabled", Some("Not available right now"), None, |ui| {
                let r = demo_input(ui, "disabled", "Locked value", Field::new("Disabled", 220.0).enabled(false));
                measured.push(("input · disabled".into(), r));
            });
            field(ui, "Read-only (selectable)", None, None, |ui| {
                let r = readonly(ui, "#4821", 160.0, true);
                measured.push(("read-only".into(), r.rect.expand2(egui::vec2(PAD_X, 1.0))));
            });
            field(ui, "Password", None, None, |ui| {
                demo_input(ui, "password", "secret-token", Field::new("Password", 160.0).password());
            });
            field(ui, "Focused", None, None, |ui| {
                let r = demo_input(ui, "focused", "Draft", Field::new("Focused", 160.0));
                ui.painter().rect_stroke(r.expand(2.0), RADIUS + 2.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
            });
        });
        ui.add_space(GROUP_GAP + 8.0);

        section(ui, "Selectors and menus");
        ui.horizontal_top(|ui| {
            field(ui, "Placeholder", None, None, |ui| {
                select_trigger(ui, "Choose project", true, 200.0, true, false, false);
                measured.push(("selector · placeholder".into(), last_trigger_rect(ui)));
            });
            field(ui, "Value", None, None, |ui| {
                let mut v = 15u32;
                let opts: Vec<(u32, String)> = [5u32, 15, 30].iter().map(|s| (*s, trf_m!("{} min", s))).collect();
                select(ui, "gallery-select", &mut v, &opts, 140.0, true);
                measured.push(("selector · value".into(), last_trigger_rect(ui)));
            });
            field(ui, "Invalid", None, Some("Choose a task, or remove this exception"), |ui| {
                select_trigger(ui, "Choose task", true, 200.0, true, true, false);
            });
            field(ui, "Disabled", None, None, |ui| {
                select_trigger(ui, "Unavailable", false, 160.0, false, false, false);
            });
        });
        ui.add_space(GROUP_GAP - ROW_GAP);
        ui.horizontal_top(|ui| {
            field(ui, "Project picker", None, None, |ui| {
                let rp = recent_projects(&app.store.db);
                let mut v: Option<Id> = app.store.db.live_projects().first().map(|p| p.id.clone());
                project_picker(ui, "gallery-project", &app.store.db, &rp, &mut v, 220.0);
            });
            field(ui, "Segmented", None, None, |ui| {
                let r = segmented(ui, &["Day", "Week", "Month"], 1);
                let _ = r;
            });
            field(ui, "Menu rows (≥ 36)", None, None, |ui| {
                egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(8.0).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                    ui.set_width(200.0);
                    menu_row(ui, "Continue now", false, false);
                    let r = menu_row(ui, "Selected option", true, false);
                    measured.push(("menu row".into(), r.rect));
                });
            });
        });
        ui.add_space(GROUP_GAP + 8.0);

        section(ui, "Status and numbers");
        ui.horizontal(|ui| {
            pill(ui, "✔ Reported", c.green);
            pill(ui, "! Needs code", c.amber);
            pill(ui, "Needs attention", c.red);
            pill(ui, "Running: provisional", c.text2);
            count_chip(ui, 3);
            mixed(ui, "3 sessions need details · 1 h 37 min · #4821", 14.0, None);
        });
        ui.horizontal(|ui| {
            ui.label(mono_m("01:12:09", 34.0));
            ui.add_space(16.0);
            ui.label(mono_m("6 h 42 min", 24.0));
            ui.add_space(16.0);
            ui.label(mono_m("3,75", 20.0));
            ui.add_space(16.0);
            ui.label(mono("09:00–10:37", 13.0).color(c.text2));
        });
        ui.add_space(GROUP_GAP + 8.0);

        // ---- geometry check with the real row layouts ----
        section(ui, "Geometry check");
        meta(ui, "The same controls as the Today start row and the Edit entry time row, measured on this frame.");
        ui.add_space(ROW_GAP);
        let rp = recent_projects(&app.store.db);
        let rt = recent_tasks(&app.store.db);
        let mut start_rects: Vec<(String, egui::Rect)> = vec![];
        ui.horizontal_top(|ui| {
            field(ui, "Description", None, None, |ui| {
                let r = demo_input(ui, "g-desc", "", Field::new("Description", 280.0).hint("What are you working on?"));
                start_rects.push(("start row · description".into(), r));
            });
            let mut proj: Option<Id> = None;
            let mut task: Option<Id> = None;
            field(ui, "Project", None, None, |ui| {
                project_picker(ui, "g-project", &app.store.db, &rp, &mut proj, 180.0);
                start_rects.push(("start row · project".into(), last_trigger_rect(ui)));
            });
            field(ui, "Task", None, None, |ui| {
                task_picker(ui, "g-task", &app.store.db, &rt, &mut proj, &mut task, 200.0);
                start_rects.push(("start row · task".into(), last_trigger_rect(ui)));
            });
            field(ui, " ", None, None, |ui| {
                let r = button_w(ui, Kind::Primary, "Start", 88.0, true);
                start_rects.push(("start row · Start".into(), r.rect));
            });
        });
        ui.add_space(GROUP_GAP - ROW_GAP);
        ui.horizontal_top(|ui| {
            for (k, name, init, w) in [("g-date", "Date", "2026-11-16", 140.0), ("g-start", "Start", "09:00", 100.0), ("g-end", "End", "10:37", 100.0), ("g-dur", "Duration", "1:37:00", 120.0)] {
                field(ui, name, None, None, |ui| {
                    let r = demo_input(ui, k, init, Field::new(name, w).mono());
                    start_rects.push((format!("time row · {}", name.to_lowercase()), r));
                });
            }
        });
        measured.extend(start_rects);
        ui.add_space(GROUP_GAP);
        // table
        egui::Frame::new().stroke(Stroke::new(1.0, c.line)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
            let prev: Measured = ui.data(|d| d.get_temp(egui::Id::new("gallery-measured"))).unwrap_or_default();
            egui::Grid::new("geometry").num_columns(5).spacing([24.0, 6.0]).show(ui, |ui| {
                for h in ["Control", "Height", "Width", "Top", "Result (36 ± 1)"] {
                    label(ui, h);
                }
                ui.end_row();
                for (n, r) in &prev.0 {
                    ui.label(RichText::new(n).size(13.0));
                    ui.label(mono(format!("{:.1}", r.height()), 13.0));
                    ui.label(mono(format!("{:.1}", r.width()), 13.0).color(c.text2));
                    ui.label(mono(format!("{:.1}", r.top()), 13.0).color(c.text2));
                    let expect = n != "menu row" || r.height() >= 35.0;
                    let ok = if n == "menu row" { r.height() >= 35.0 } else { (r.height() - 36.0).abs() <= 1.0 && expect };
                    ui.label(RichText::new(if ok { "✔ pass" } else { "✖ fail" }).size(13.0).color(if ok { c.green } else { c.red }));
                    ui.end_row();
                }
            });
            // rows sharing a line must share their top edge
            let tops = |prefix: &str| -> Option<(f32, f32)> {
                let v: Vec<f32> = prev.0.iter().filter(|(n, _)| n.starts_with(prefix)).map(|(_, r)| r.top()).collect();
                if v.is_empty() {
                    None
                } else {
                    Some((v.iter().cloned().fold(f32::MAX, f32::min), v.iter().cloned().fold(f32::MIN, f32::max)))
                }
            };
            for (prefix, name) in [("start row", "Start row"), ("time row", "Time row")] {
                if let Some((a, b)) = tops(prefix) {
                    let ok = b - a <= 1.0;
                    mixed(ui, &format!("{}: tops differ by {:.1} units {}", name, b - a, if ok { "· aligned" } else { "· NOT aligned" }), 13.0, Some(if ok { c.text2 } else { c.red }));
                }
            }
        });
        ui.data_mut(|d| d.insert_temp(egui::Id::new("gallery-measured"), Measured(measured.clone())));
    });
    app.geometry.rows.extend(measured);
}
