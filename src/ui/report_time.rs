//! Report Time: one source of actual time, reconciled against each destination's values.
//! Each destination keeps its own value, code, rule, copy action and reporting status.
//! Values in different destinations are alternatives for the same work, never added together.
//! Copying never marks a line as entered. Destinations are created and edited in the
//! destination dialog (`dest.rs`); a missing code opens a single-project code editor.

use super::dest::{DestDraft, Step};
use super::i18n::{plural, tr, trf};
use super::i18n::trf_m;
use super::widgets::*;
use super::{page_header, App, Page};
use chrono::NaiveDate;
use devtrack_core::model::*;
use devtrack_core::report::{explain_value, local_date, ValueExplained, confirm, entries_in, mapping_issues, needs_completion, orphan_marks, render_value, timesheet, LineStatus, Period, PeriodKind, SheetLine};
use devtrack_core::store::now_secs;
use devtrack_core::timefmt::clock;
use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use std::collections::BTreeSet;

pub struct RepState {
    pub period: Period,
    /// Destinations hidden from the comparison.
    pub hidden: BTreeSet<Id>,
    pub confirm_delete: bool,
    pub selected_row: Option<String>,
    pub show_sessions: bool,
    /// The destination whose values are listed; the first one when unset or gone.
    pub dest: Option<Id>,
}

impl RepState {
    pub fn new(period: Period) -> Self {
        RepState { period, hidden: BTreeSet::new(), confirm_delete: false, selected_row: None, show_sessions: false, dest: None }
    }
}

/// Signed exact difference: "+16 s", "-6 s", "+12 min 16 s".
fn signed_exact(d: i64) -> String {
    let a = d.abs();
    let body = if a < 60 {
        format!("{} s", a)
    } else if a % 60 == 0 {
        words(a)
    } else if a < 3600 {
        format!("{} min {:02} s", a / 60, a % 60)
    } else {
        format!("{} h {:02} min {:02} s", a / 3600, (a % 3600) / 60, a % 60)
    };
    format!("{}{}", if d < 0 { "-" } else { "+" }, body)
}

/// Where a difference between actual time and a written value comes from.
fn reason(e: &ValueExplained) -> String {
    let mut parts = vec![];
    if e.rule_diff != 0 {
        parts.push(trf("{} company rounding", &[&signed_exact(e.rule_diff)]));
    }
    if e.precision_diff != 0 {
        parts.push(trf("Format difference: {}", &[&signed_exact(e.precision_diff)]));
    }
    if parts.is_empty() {
        tr("exact")
    } else {
        parts.join(" · ")
    }
}

fn format_name(p: &TimesheetProfile) -> String {
    let f = match p.format {
        ReportFormat::HoursMinutes => tr("Hours:minutes"),
        ReportFormat::DecimalHours => tr("Decimal hours"),
        ReportFormat::Minutes => tr("Minutes"),
    };
    let r = if p.rounds() { trf_m!("round {} to {} min", tr(p.round_dir.label()).to_lowercase(), p.round_step_min) } else { tr("no company rounding") };
    format!("{} · {}", f, r)
}

/// One baseline row: a project on a day.
/// Projects behind each line of a destination.
fn line_projects(app: &App, l: &SheetLine) -> BTreeSet<Option<Id>> {
    l.sessions.iter().map(|s| app.store.db.entries.get(&s.entry_id).and_then(|e| e.project_id.clone())).collect()
}

/// Frameless status action, 36 high, coloured words (never colour alone).
fn status_action(ui: &mut egui::Ui, text: &str, color: Color32, tip: &str, width: f32) -> egui::Response {
    let c = p(ui);
    let text = &tr(text);
    let tip = &tr(tip);
    let font = sans(13.0);
    let g = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width.max(g.size().x + 20.0), CTRL_H), Sense::click());
    // framed like a secondary control so it reads as an action, in its status colour
    let hovered = resp.hovered() || resp.is_pointer_button_down_on();
    ui.painter().rect(rect, RADIUS, if hovered { c.hover } else { Color32::TRANSPARENT }, Stroke::new(1.0, if hovered { color } else { c.border }), egui::StrokeKind::Inside);
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g, color);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), RADIUS + 2.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
    }
    let t = text.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t.clone()));
    resp.on_hover_text(tip)
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let now = now_secs();
    let c = p(ui);
    let monday = app.store.db.settings.week_starts_monday;
    let profiles: Vec<TimesheetProfile> = app.store.db.live_profiles().into_iter().cloned().collect();
    let mut add = false;
    page_header(ui, "Report Time", "Compare actual time with the values each destination requires.", |ui| {
        if secondary(ui, "+ Add destination").clicked() {
            add = true;
        }
    });
    if add {
        app.remember_focus(ui.ctx());
        app.dest = Some(DestDraft::new_destination(app));
    }
    let period = app.rep.period;
    let (n_entries, incomplete, running) = {
        let entries = entries_in(&app.store.db, &period, now);
        (entries.len(), entries.iter().filter(|e| needs_completion(e) && !e.is_running()).count(), entries.iter().any(|e| e.is_running()))
    };
    let mut per = period;
    let mut review = false;
    let incomplete_text = format!("! {}", plural(incomplete as i64, "{} session needs details", "{} sessions need details"));
    // the place the status box needs, measured: it joins the toolbar's row whenever it fits
    let status_w = if n_entries == 0 { 0.0 } else if incomplete > 0 { status_chip_w(ui, &incomplete_text, true) } else if running { status_chip_w(ui, "● A session is still running", false) } else { status_chip_w(ui, "✔ All sessions completed", false) };
    let bar = super::period::period_toolbar(ui, &mut per, monday, [true, true, true], status_w, |ui| {
        // one box, as tall as the buttons: the status itself, and the way to fix it when there
        // is something to fix
        if incomplete > 0 {
            if status_chip(ui, &incomplete_text, c.amber, Some("Review them on Today")).clicked() {
                review = true;
            }
        } else if n_entries == 0 {
            // the page below says it, with the way forward: nothing to repeat here
        } else if running {
            status_chip(ui, "● A session is still running", c.text2, None);
        } else {
            status_chip(ui, "✔ All sessions completed", c.green, None);
        }
    });
    super::period::record(app, "report", &bar);
    if review {
        app.today.period = period;
        app.today.only_incomplete = true;
        app.go(Page::Today);
    }
    if per != period {
        app.rep.period = per;
        app.rep.selected_row = None;
    }
    ui.add_space(16.0);
    if profiles.is_empty() {
        no_destinations(app, ui);
        return;
    }
    // One destination at a time: its lines are the list of values to enter there.
    if app.rep.dest.as_ref().map(|id| !profiles.iter().any(|p| &p.id == id)).unwrap_or(true) {
        app.rep.dest = profiles.first().map(|p| p.id.clone());
    }
    let pr = profiles.iter().find(|p| Some(&p.id) == app.rep.dest.as_ref()).cloned().unwrap_or_else(|| profiles[0].clone());
    let lines = timesheet(&app.store.db, &pr, &period, now);
    let h24 = app.store.db.settings.clock_24h;
    let mut out = CellOut::default();
    let mut select: Option<String> = None;
    let mut open_entry: Option<TimeEntry> = None;
    let mut copy_line: Option<String> = None;

    let list = egui::ScrollArea::vertical().id_salt("report").auto_shrink([false, false]).show(ui, |ui| {
        let available = ui.available_width();
        // ---- which destination, and its actions ----
        row(ui, CTRL_H, |ui| {
            if profiles.len() > 1 {
                let names: Vec<String> = profiles.iter().map(|p| truncate(&p.name, 24)).collect();
                let seg_w: f32 = names.iter().map(|n| ui.painter().layout_no_wrap(n.clone(), medium(14.0), c.text).size().x + 28.0).sum();
                if profiles.len() <= 4 && seg_w + CTRL_H + ROW_GAP * 2.0 <= available {
                    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                    let cur = profiles.iter().position(|p| p.id == pr.id).unwrap_or(0);
                    if let Some(i) = segmented(ui, &refs, cur) {
                        app.rep.dest = Some(profiles[i].id.clone());
                        app.rep.selected_row = None;
                    }
                } else {
                    let opts: Vec<(Id, String)> = profiles.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
                    let mut cur = pr.id.clone();
                    if super::widgets::select(ui, "report-dest", &mut cur, &opts, (available - CTRL_H - ROW_GAP).min(320.0), false) {
                        app.rep.dest = Some(cur);
                        app.rep.selected_row = None;
                    }
                }
            } else {
                ui.add(egui::Label::new(RichText::new(&pr.name).font(medium(16.0))).truncate());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| dest_menu(ui, &pr, &lines, &mut out));
        });
        // what this destination asks for, in one line
        let issues = mapping_issues(&pr, &app.store.db);
        row_wrapped(ui, 20.0, |ui| {
            ui.spacing_mut().item_spacing.x = GROUP_GAP;
            meta(ui, format!("{} · {}", format_name(&pr), tr(super::dest::grouping_text(pr.grouping))));
            if !issues.is_empty() {
                mixed(ui, &plural(issues.len() as i64, "⚠ {} mapping issue", "⚠ {} mapping issues"), 12.0, Some(c.amber)).on_hover_text(issues.join("\n"));
            }
        });
        ui.add_space(ROW_GAP);
        if lines.is_empty() {
            let last = app.last_day_with_sessions(period.start);
            let (mut today, mut go_back) = (false, None);
            empty_state(ui, Icon::Report, "No sessions in this period", &trf_m!("The values to enter in {} are calculated from recorded sessions. Track time on Today, or look at a period that has sessions.", pr.name), |ui| {
                today = secondary(ui, "Go to Today").clicked();
                if let Some(d) = last {
                    if link_btn(ui, "Show the last day with sessions").clicked() { go_back = Some(d); }
                }
            });
            if today { app.go(super::Page::Today); }
            if let Some(d) = go_back {
                app.rep.period = devtrack_core::report::Period::containing(devtrack_core::report::PeriodKind::Day, d, monday);
            }
            return;
        }
        // ---- summary: recorded · to enter · progress ----
        let actual: i64 = lines.iter().map(|l| l.actual).sum();
        let reported: i64 = lines.iter().map(|l| l.reported).sum();
        let entered = lines.iter().filter(|l| l.status == LineStatus::Reported).count();
        let missing = lines.iter().filter(|l| l.status == LineStatus::NotReady).count();
        divider(ui);
        let columns = if available >= 440.0 { 3 } else { 1 };
        let cell_w = (available - ROW_GAP * (columns - 1) as f32) / columns as f32;
        egui::Grid::new("report-summary").num_columns(columns).spacing([ROW_GAP, GROUP_GAP]).show(ui, |ui| {
            for i in 0..3 {
                ui.vertical(|ui| {
                    ui.set_width(cell_w);
                    match i {
                        0 => {
                            label(ui, "Actual time");
                            ui.add(egui::Label::new(mono_m(clock(actual), 24.0)).wrap_mode(egui::TextWrapMode::Extend));
                        }
                        1 => {
                            label(ui, "To enter");
                            ui.add(egui::Label::new(mono_m(render_value(&pr, reported), 24.0)).wrap_mode(egui::TextWrapMode::Extend));
                            let ex = explain_value(&pr, actual, reported);
                            ui.add(egui::Label::new(mixed_job(&reason(&ex), 12.0, c.text2, sans(12.0), mono_font(12.0))).wrap());
                        }
                        _ => {
                            label(ui, "Entered");
                            let g = mixed_job(&trf("{} of {}", &[&entered, &lines.len()]), 24.0, if entered == lines.len() { c.green } else { c.text }, medium(24.0), mono_medium(24.0));
                            ui.add(egui::Label::new(g).wrap_mode(egui::TextWrapMode::Extend));
                            if missing > 0 {
                                mixed(ui, &plural(missing as i64, "{} value needs a code", "{} values need a code"), 12.0, Some(c.amber));
                            }
                        }
                    }
                });
                if columns == 1 || i == 2 { ui.end_row(); }
            }
        });
        divider(ui);
        ui.add_space(GROUP_GAP - ui.spacing().item_spacing.y);

        // ---- the values: one row per thing to enter ----
        let width_of = |t: &str, size: f32| ui.painter().layout_no_wrap(tr(t), sans(size), c.text).size().x;
        let status_w = ["! Needs code", "Confirm entry", "✔ Entered", "⚠ Confirm again"].iter().map(|t| width_of(t, 13.0) + 20.0).fold(width_of("Running: provisional", 12.0), f32::max);
        let inner_w = (available - 32.0).max(200.0);
        let wide = inner_w >= 700.0;
        let (lead_w, rec_w, val_w) = (CTRL_H + 6.0, 84.0, 96.0);
        let actions_w = CTRL_H + ROW_GAP + status_w;
        let what_w = if wide { inner_w - lead_w - rec_w - val_w - actions_w - ROW_GAP * 4.0 } else { inner_w - lead_w };
        let multi_day = lines.iter().map(|l| l.date).collect::<BTreeSet<_>>().len() > 1;
        egui::Frame::new().stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).show(ui, |ui| {
            ui.set_width(inner_w + 32.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            if wide {
                // column names, on the same grid as the rows
                egui::Frame::new().fill(c.nav).corner_radius(egui::CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }).inner_margin(egui::Margin::symmetric(16, 0)).show(ui, |ui| {
                    ui.set_min_width(inner_w);
                    row(ui, CTRL_H, |ui| {
                        ui.spacing_mut().item_spacing.x = ROW_GAP;
                        ui.add_space(lead_w);
                        ui.allocate_ui_with_layout(egui::vec2(what_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_width(what_w); label(ui, what_label(&pr)); });
                        ui.allocate_ui_with_layout(egui::vec2(rec_w, 20.0), egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.set_width(rec_w); label(ui, "Tracked"); });
                        ui.allocate_ui_with_layout(egui::vec2(val_w, 20.0), egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.set_width(val_w); label(ui, "To enter"); });
                    });
                });
                app.geometry.rows.push(("report header".into(), ui.min_rect()));
            }
            let mut last_date: Option<NaiveDate> = None;
            for (li, l) in lines.iter().enumerate() {
                if multi_day && last_date != Some(l.date) {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 6)).show(ui, |ui| {
                        ui.set_min_width(inner_w);
                        if pr.grouping == Grouping::WeekCode {
                            ui.label(RichText::new(&l.date_label).font(medium(13.0)).color(c.text2));
                        } else {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.label(RichText::new(super::i18n::weekday_title(l.date)).font(medium(13.0)).color(c.text2));
                                ui.label(mono(super::i18n::short_date(l.date), 12.0).color(c.text2));
                            });
                        }
                    });
                    divider(ui);
                    last_date = Some(l.date);
                }
                let sel = app.rep.selected_row.as_deref() == Some(l.key.as_str());
                let rect_id = egui::Id::new(("report-line-rect", &l.key));
                let hovered = ui.data(|d| d.get_temp::<egui::Rect>(rect_id)).is_some_and(|r| ui.rect_contains_pointer(r));
                let fill = if sel { c.selected } else if hovered { c.hover.linear_multiply(0.6) } else { Color32::TRANSPARENT };
                let what = describe(app, &pr, l);
                // the fill follows the table's corners: bottom ones on the last row (unless its
                // details follow), top ones on the first row when there is no header band
                let last = li + 1 == lines.len();
                let round = egui::CornerRadius { nw: if li == 0 && !wide { 9 } else { 0 }, ne: if li == 0 && !wide { 9 } else { 0 }, sw: if last && !sel { 9 } else { 0 }, se: if last && !sel { 9 } else { 0 } };
                let resp = egui::Frame::new().fill(fill).corner_radius(round).inner_margin(egui::Margin::symmetric(16, 8)).show(ui, |ui| {
                    ui.set_min_width(inner_w);
                    let lead = |ui: &mut egui::Ui, select: &mut Option<String>| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let t = icon_ghost(ui, if sel { Glyph::Down } else { Glyph::Next }, if sel { "Close details" } else { "Details" });
                        if t.clicked() { *select = Some(l.key.clone()); }
                        col(ui, what_w, 40.0, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if let Some(col) = what.dot { dot(ui, col, 3.5); }
                                let text = if what.code { RichText::new(&what.title).font(mono_medium(14.0)) } else { RichText::new(&what.title).font(medium(14.0)) };
                                let r = ui.add(egui::Label::new(if what.missing { text.color(c.amber) } else { text }).truncate().sense(Sense::click()));
                                if r.on_hover_text(&what.title).clicked() { *select = Some(l.key.clone()); }
                            });
                            if !what.sub.is_empty() {
                                // one tip, its own: egui's for a cut line would stack under it
                                cut_line(ui, mixed_job(&what.sub, 12.0, c.text2, sans(12.0), mono_font(12.0)), Some(what.hover.clone().unwrap_or_else(|| what.sub.clone())));
                            }
                        });
                    };
                    let recorded = |ui: &mut egui::Ui| ui.label(mono(clock(l.actual), 13.0).color(c.text2)).on_hover_text(tr("Actual time, to the second"));
                    let value = |ui: &mut egui::Ui| ui.label(mono_m(&l.value, 20.0)).on_hover_text(tr("Value to enter in the destination"));
                    let actions = |ui: &mut egui::Ui, out: &mut CellOut| {
                        ui.spacing_mut().item_spacing.x = ROW_GAP;
                        // a provisional value (running session) can't be copied yet: the reason is the tooltip
                        let copy_r = ui.add_enabled_ui(!l.provisional, |ui| icon_button(ui, Glyph::Copy, "Copy value")).inner;
                        if l.provisional {
                            copy_r.on_disabled_hover_text(tr("A session is still running: the value can change. Stop it to copy."));
                        } else if copy_r.on_hover_text(trf_m!("Copy {} (doesn't mark it as reported)", l.value)).clicked() {
                            out.copy = Some(l.value.clone());
                        }
                        match l.status {
                            LineStatus::NotReady => {
                                if status_action(ui, "! Needs code", c.amber, "Add a code for this project", status_w).clicked() {
                                    out.fix = Some((0, line_projects(app, l).into_iter().flatten().next(), l.key.clone()));
                                }
                            }
                            LineStatus::Provisional => {
                                meta(ui, "Running: provisional");
                            }
                            LineStatus::ToReport => {
                                if status_action(ui, "Confirm entry", c.text, "Confirm only after entering this value in the destination. Copying does not confirm it.", status_w).clicked() {
                                    if let Some(m) = confirm(l, &pr, now) { out.marks.push(m); }
                                }
                            }
                            LineStatus::Reported => {
                                if status_action(ui, "✔ Entered", c.green, "Entered. Click to undo the mark.", status_w).clicked() {
                                    out.unmark.push(l.key.clone());
                                }
                            }
                            LineStatus::ChangedAfterReporting => {
                                let tip = trf_m!("Entered as {} before the sessions, code or rule changed", l.mark.as_ref().map(|m| m.value.clone()).unwrap_or_default());
                                if status_action(ui, "⚠ Confirm again", c.amber, &tip, status_w).clicked() {
                                    if let Some(m) = confirm(l, &pr, now) { out.marks.push(m); }
                                }
                            }
                        }
                    };
                    if wide {
                        row(ui, 44.0, |ui| {
                            lead(ui, &mut select);
                            ui.spacing_mut().item_spacing.x = ROW_GAP;
                            ui.allocate_ui_with_layout(egui::vec2(rec_w, 40.0), egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.set_width(rec_w); recorded(ui); });
                            ui.allocate_ui_with_layout(egui::vec2(val_w, 40.0), egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.set_width(val_w); value(ui); });
                            ui.add_space((ui.available_width() - actions_w).max(0.0));
                            actions(ui, &mut out);
                        });
                    } else {
                        row(ui, 44.0, |ui| lead(ui, &mut select));
                        ui.add_space(4.0);
                        row(ui, CTRL_H, |ui| {
                            ui.add_space(lead_w);
                            ui.spacing_mut().item_spacing.x = 6.0;
                            recorded(ui);
                            ui.label(RichText::new("→").size(13.0).color(c.text2));
                            value(ui);
                        });
                        ui.add_space(4.0);
                        row(ui, CTRL_H, |ui| {
                            ui.add_space(lead_w);
                            actions(ui, &mut out);
                        });
                    }
                });
                app.geometry.rows.push((format!("report line · {}", l.key), resp.response.rect));
                ui.data_mut(|d| d.insert_temp(rect_id, resp.response.rect));
                if sel {
                    let detail = egui::Frame::new().fill(c.nav).corner_radius(egui::CornerRadius { nw: 0, ne: 0, sw: if last { 9 } else { 0 }, se: if last { 9 } else { 0 } }).inner_margin(egui::Margin { left: 16 + lead_w as i8, right: 16, top: 12, bottom: 16 }).show(ui, |ui| {
                        ui.set_min_width(inner_w - lead_w);
                        ui.spacing_mut().item_spacing.y = 6.0;
                        line_detail(app, ui, &pr, l, h24, &mut open_entry, &mut copy_line);
                    });
                    app.geometry.rows.push((format!("report detail · {}", l.key), detail.response.rect));
                }
                // the table's own border closes the last row
                if li + 1 < lines.len() {
                    divider(ui);
                }
            }
        });
        // confirmations whose line changed shape (code, activity, grouping)
        let orphans = orphan_marks(&app.store.db, &pr, &period, &lines);
        if !orphans.is_empty() {
            ui.add_space(12.0);
            panel(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(trf_m!("{}: entered earlier, no longer matching", pr.name)).font(medium(14.0)));
                meta(ui, "The code, activity or grouping changed after you entered these values. Check the destination, then dismiss.");
                for m in &orphans {
                    ui.horizontal(|ui| {
                        pill(ui, "⚠ Changed after reporting", c.amber);
                        mixed(ui, &trf_m!("{} · {} {} · entered {}", m.date_label, m.code, m.activity, if m.value.is_empty() { trf_m!("{} min", m.reported_minutes) } else { m.value.clone() }), 13.0, None);
                        if ghost(ui, "Dismiss").clicked() {
                            let mut x = m.clone();
                            x.deleted = true;
                            app.store.put_mark(x);
                        }
                    });
                }
            });
        }
    });
    scroll_fades(ui, &list, p(ui).canvas);

    let CellOut { marks, unmark, copy, fix, edit_profile } = out;
    if let Some(s) = select {
        app.rep.selected_row = if app.rep.selected_row.as_ref() == Some(&s) { None } else { Some(s) };
    }
    for m in marks {
        app.store.put_mark(m);
    }
    for id in unmark {
        if let Some(mut m) = app.store.db.marks.get(&id).cloned() {
            m.deleted = true;
            app.store.put_mark(m);
        }
    }
    if let Some(text) = copy {
        ui.ctx().copy_text(text);
        app.notify(tr("Value copied. Confirm after entering it in the destination."));
    }
    if let Some(t) = copy_line {
        ui.ctx().copy_text(t);
        app.notify(tr("Line copied (not marked as reported)"));
    }
    if let Some((_, project, key)) = fix {
        // contextual editor for this one project; the line and date stay where they are
        app.remember_focus(ui.ctx());
        app.rep.selected_row = Some(key);
        app.dest = Some(DestDraft::fix_code(app, &pr, project));
    }
    if let Some((ask_delete, step)) = edit_profile {
        app.remember_focus(ui.ctx());
        app.rep.confirm_delete = ask_delete == 1;
        app.dest = Some(DestDraft::edit(app, &pr, step));
    }
    if let Some(e) = open_entry {
        let ctx = ui.ctx().clone();
        super::today::open_edit(app, &e, Some(&ctx), false);
    }
}

/// What a line is, in the words of its destination's grouping.
struct What {
    dot: Option<Color32>,
    title: String,
    /// The title is a job code: Mono.
    code: bool,
    missing: bool,
    sub: String,
    /// The line with every tag in place of "+N", for the pointer.
    hover: Option<String>,
}

/// The name of the first column: what one row stands for in this destination.
fn what_label(pr: &TimesheetProfile) -> &'static str {
    match pr.grouping {
        Grouping::DayCode | Grouping::WeekCode => "Job code",
        Grouping::DayProject => "Project",
        Grouping::DayTask => "Task",
    }
}

fn describe(app: &App, pr: &TimesheetProfile, l: &SheetLine) -> What {
    let db = &app.store.db;
    let projects = line_projects(app, l);
    let names: Vec<String> = projects.iter().map(|p| db.project(p).map(|p| p.name.clone()).unwrap_or(tr("No project"))).collect();
    // each project with its client: "Design · Northwind, Varie · Fabrikam"
    let with_clients: Vec<String> = projects
        .iter()
        .map(|p| match db.project(p) {
            Some(pr) => {
                let cl = db.client_name(pr);
                if cl.is_empty() { pr.name.clone() } else { format!("{} · {}", pr.name, cl) }
            }
            None => tr("No project"),
        })
        .collect();
    let first_client = projects.iter().flatten().next().and_then(|p| db.projects.get(p)).map(|p| db.client_name(p)).filter(|c| !c.is_empty());
    let code = if l.activity.is_empty() { l.code.clone() } else { format!("{} · {}", l.code, l.activity) };
    let first_color = projects.iter().flatten().next().and_then(|p| db.projects.get(p)).map(|p| hex(&p.color));
    let mut what = match pr.grouping {
        // the code is the thing to enter; the projects it gathers explain it
        Grouping::DayCode | Grouping::WeekCode => {
            if l.code.is_empty() {
                What { dot: None, title: tr("No code"), code: false, missing: true, sub: with_clients.join(", "), hover: None }
            } else {
                What { dot: None, title: code, code: true, missing: false, sub: with_clients.join(", "), hover: None }
            }
        }
        // the project is the title; its client comes first under it, then the code
        Grouping::DayProject => {
            let sub = [first_client.clone().unwrap_or_default(), code].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
            What { dot: first_color, title: names.first().cloned().unwrap_or(tr("No project")), code: false, missing: false, sub, hover: None }
        }
        Grouping::DayTask => {
            let title = l.tasks.first().cloned().or_else(|| l.sessions.iter().map(|s| s.description.clone()).find(|d| !d.trim().is_empty())).unwrap_or(tr("Untitled session"));
            let mut sub = with_clients.join(", ");
            if !l.code.is_empty() { sub.push_str(&format!(" · {}", code)); }
            What { dot: first_color, title, code: false, missing: false, sub, hover: None }
        }
    };
    // the tags of the sessions behind the value, once (informative: they change no value)
    let entries: Vec<TimeEntry> = l.sessions.iter().filter_map(|s| db.entries.get(&s.entry_id).cloned()).collect();
    let counts = super::activities::tag_counts(db, &entries);
    if !counts.is_empty() {
        if !what.sub.is_empty() { what.sub.push_str(" · "); }
        what.sub.push_str(&super::activities::tag_summary(&counts));
    }
    // only a real rule is worth a word here; dropped seconds are explained in the details
    let ex = explain_value(pr, l.actual, l.reported);
    if ex.rule_diff != 0 {
        if !what.sub.is_empty() { what.sub.push_str(" · "); }
        what.sub.push_str(&trf("rounded {}", &[&signed_exact(ex.rule_diff)]));
    }
    // the line as it ends, with every tag in place of "+N"
    what.hover = super::activities::all_tags_hover(&what.sub, &counts);
    what
}

/// The destination's actions menu.
fn dest_menu(ui: &mut egui::Ui, pr: &TimesheetProfile, lines: &[SheetLine], out: &mut CellOut) {
    let now = now_secs();
    let more = icon_button(ui, Glyph::More, "Destination actions");
    let pid = ui.make_persistent_id(("dest-more", &pr.id));
    if more.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pid));
    }
    egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(240.0);
        if menu_row(ui, "Edit format…", false, false).clicked() {
            out.edit_profile = Some((0, Step::Format));
        }
        if menu_row(ui, "Project codes…", false, false).clicked() {
            out.edit_profile = Some((0, Step::Codes));
        }
        if menu_row(ui, "Destination settings…", false, false).clicked() {
            out.edit_profile = Some((0, Step::Settings));
        }
        // removing is one step from here too, not only from Settings › Platforms
        if menu_row(ui, "Remove destination…", false, false).clicked() {
            out.edit_profile = Some((1, Step::Settings));
        }
        let ready: Vec<&SheetLine> = lines.iter().filter(|l| l.ready && !l.provisional).collect();
        if menu_row(ui, &plural(ready.len() as i64, "Copy {} ready line", "Copy {} ready lines"), false, false).clicked() {
            out.copy = Some(ready.iter().map(|l| l.line.clone()).collect::<Vec<_>>().join("\n"));
        }
        let todo: Vec<&SheetLine> = lines.iter().filter(|l| matches!(l.status, LineStatus::ToReport | LineStatus::ChangedAfterReporting)).collect();
        if !todo.is_empty() && menu_row(ui, &plural(todo.len() as i64, "Mark {} line as reported", "Mark {} lines as reported"), false, false).clicked() {
            for l in todo {
                if let Some(m) = confirm(l, pr, now) {
                    out.marks.push(m);
                }
            }
        }
    });
}

/// What is behind one value: its sessions, how the value was produced, and the full line.
#[allow(clippy::too_many_arguments)]
fn line_detail(app: &mut App, ui: &mut egui::Ui, pr: &TimesheetProfile, l: &SheetLine, h24: bool, open_entry: &mut Option<TimeEntry>, copy_line: &mut Option<String>) {
    let c = p(ui);
    let w = ui.available_width();
    ui.label(RichText::new(plural(l.sessions.len() as i64, "{} session in this value", "{} sessions in this value")).font(medium(14.0)));
    let several = line_projects(app, l).len() > 1;
    let entries: Vec<TimeEntry> = l.sessions.iter().filter_map(|s| app.store.db.entries.get(&s.entry_id).cloned()).collect();
    let counts = super::activities::tag_counts(&app.store.db, &entries);
    if !counts.is_empty() {
        super::activities::tag_count_row(ui, &counts, entries.len());
    }
    for s in &l.sessions {
        let entry = app.store.db.entries.get(&s.entry_id).cloned();
        let interval = format!("{} → {}", clock_text(s.start, h24), if s.running { tr("now") } else { clock_text(s.end, h24) });
        let mut title = match &entry {
            Some(e) => super::session_title(&app.store.db, e),
            None if !s.description.trim().is_empty() => s.description.clone(),
            None => tr("Untitled session"),
        };
        if several {
            if let Some(pr) = entry.as_ref().and_then(|e| app.store.db.project(&e.project_id)) {
                let cl = app.store.db.client_name(pr);
                title = if cl.is_empty() { format!("{} · {}", title, pr.name) } else { format!("{} · {} · {}", title, pr.name, cl) };
            }
        }
        // one line while there is room; in a narrow window the title gets its own line
        let r = if w >= 420.0 {
            row(ui, 24.0, |ui| {
                ui.spacing_mut().item_spacing.x = ROW_GAP;
                ui.label(mixed_job(&interval, 12.0, c.text2, sans(12.0), mono_font(12.0)));
                ui.scope(|ui| {
                    ui.set_max_width((w - 300.0).max(120.0));
                    // egui shows the whole title on hover only when it is cut; the tags that set
                    // this session apart follow it
                    let job = match &entry {
                        Some(e) => super::activities::note_with_own_tags(ui, &app.store.db, &title, e, &counts, entries.len()),
                        None => RichText::new(&title).size(13.0).into(),
                    };
                    ui.add(egui::Label::new(job).truncate());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let edit = link_btn(ui, "Edit");
                    ui.label(mono(clock(s.secs), 13.0));
                    edit
                }).inner
            })
        } else {
            ui.scope(|ui| {
                ui.set_max_width(w);
                // egui shows the whole title on hover only when it is cut
                ui.add(egui::Label::new(RichText::new(&title).size(13.0)).truncate());
            });
            row(ui, 24.0, |ui| {
                ui.spacing_mut().item_spacing.x = ROW_GAP;
                ui.label(mixed_job(&interval, 12.0, c.text2, sans(12.0), mono_font(12.0)));
                ui.label(mono(clock(s.secs), 13.0));
                link_btn(ui, "Edit")
            })
        };
        app.geometry.rows.push((format!("report source session · {}", s.entry_id), r.rect));
        if r.clicked() {
            *open_entry = entry;
        }
    }
    // Three blocks, each with its own heading, the same gap between them and the same left edge:
    // the sessions above, how the value comes out of them, the line for a spreadsheet.
    ui.add_space(20.0);
    ui.label(RichText::new(tr("Calculation")).font(medium(14.0)));
    ui.add_space(4.0);
    // the result first, then only the differences that exist, each in words
    mixed(ui, &trf("{} recorded → {} to enter", &[&clock(l.actual), &l.value]), 13.0, None);
    let ex = explain_value(pr, l.actual, l.reported);
    if ex.rule_diff != 0 {
        let dir = tr(pr.round_dir.label()).to_lowercase();
        mixed(ui, &trf("Company rounding {} to {} min: {}", &[&dir, &pr.round_step_min, &signed_exact(ex.rule_diff)]), 13.0, Some(c.text2));
    }
    if ex.precision_diff != 0 {
        let why = match pr.format {
            ReportFormat::HoursMinutes => tr("Hours:minutes cannot show seconds"),
            ReportFormat::Minutes => tr("Minutes cannot show seconds"),
            ReportFormat::DecimalHours => trf("Decimal hours keep {} decimal places", &[&pr.decimal_places]),
        };
        mixed(ui, &format!("{} ({})", why, signed_exact(ex.precision_diff)), 13.0, Some(c.text2));
    }
    if ex.rule_diff == 0 && ex.precision_diff == 0 {
        meta(ui, "No difference: the value is the recorded time.");
    }

    ui.add_space(20.0);
    ui.label(RichText::new(tr("Line for a spreadsheet")).font(medium(14.0)));
    ui.add_space(4.0);
    ui.add(egui::Label::new(RichText::new(tr("Paste it into Excel, Numbers or Google Sheets: each part lands in its own column. Its layout is set in the destination's settings.")).size(13.0).color(c.text2)).wrap());
    ui.add_space(8.0);
    // the preview and its button on one row, the button where the eye ends; stacked when narrow
    let btn_w = 132.0;
    let side = w >= 420.0;
    let box_w = if side { w - btn_w - ROW_GAP } else { w };
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(box_w, CTRL_H), Sense::hover());
        ui.painter().rect(rect, RADIUS, c.field, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
        // tabs shown as wide gaps, the way the columns will split; an empty part shows as a faint
        // dash, so the column that will stay empty can be seen (the copied text is unchanged)
        let mut job = egui::text::LayoutJob::default();
        for (i, part) in l.line.split('\t').enumerate() {
            if i > 0 {
                job.append("    ", 0.0, egui::TextFormat::simple(mono_font(13.0), c.text));
            }
            if part.trim().is_empty() {
                job.append("—", 0.0, egui::TextFormat::simple(mono_font(13.0), c.text2.gamma_multiply(0.6)));
            } else {
                job.append(part, 0.0, egui::TextFormat::simple(mono_font(13.0), c.text));
            }
        }
        let g = ui.painter().layout_job(job);
        let clip = rect.shrink2(egui::vec2(12.0, 0.0));
        ui.painter().with_clip_rect(clip).galley(egui::pos2(clip.left(), rect.center().y - g.size().y / 2.0), g, c.text);
        resp.on_hover_text(l.line.replace('\t', "  →  "));
        if side {
            action = Some(ui.add_enabled_ui(!l.provisional, |ui| button_w(ui, Kind::Secondary, "Copy line", btn_w, true)).inner);
        }
    });
    if !side {
        ui.add_space(ROW_GAP);
        action = Some(ui.add_enabled_ui(!l.provisional, |ui| secondary(ui, "Copy line")).inner);
    }
    if let Some(action) = action {
        app.geometry.rows.push((format!("report copy line · {}", l.key), action.rect));
        if l.provisional {
            action.on_disabled_hover_text(tr("A session is still running: the value can change. Stop it to copy."));
        } else if action.on_hover_text(tr("Copying does not confirm entry in the destination.")).clicked() {
            *copy_line = Some(l.line.clone());
        }
    }
    if l.code.is_empty() {
        // the row already says it and offers Add code: here only what it means for the line
        ui.add_space(6.0);
        ui.label(RichText::new(tr("No code: that column will stay empty.")).size(13.0).color(c.amber));
    }
}

fn no_destinations(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    // the full width of the page, like Settings: one panel, the explanation on top and the
    // example as its own band of three equal tiles
    egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).inner_margin(egui::Margin::same(24)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let w = ui.available_width();
        ui.label(RichText::new(tr("Add your first destination")).font(medium(18.0)));
        ui.add_space(6.0);
        ui.add(egui::Label::new(RichText::new(tr("A destination is a system you enter hours into. It decides the format, the codes and any rounding your company requires.")).font(sans(14.0)).color(c.text2)).wrap());
        ui.add_space(24.0);
        meta(ui, "EXAMPLE · not your data");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            mixed_medium(ui, "1 h 37 min", 20.0);
            ui.add_space(4.0);
            ui.label(RichText::new(tr("actual time can be written as")).font(sans(14.0)).color(c.text2));
        });
        ui.add_space(12.0);
        let gap = 12.0;
        let tile_w = if w >= 480.0 { ((w - gap * 2.0) / 3.0).floor() - 1.0 } else { w };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
            for (v, l) in [("1:37", "hours:minutes"), ("1,62", "decimal hours"), ("97", "minutes")] {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(tile_w, 96.0), Sense::hover());
                ui.painter().rect(rect, 8.0, c.canvas, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
                ui.painter().text(egui::pos2(rect.left() + 20.0, rect.top() + 18.0), egui::Align2::LEFT_TOP, v, mono_medium(32.0), c.text);
                ui.painter().text(egui::pos2(rect.left() + 20.0, rect.bottom() - 18.0), egui::Align2::LEFT_BOTTOM, tr(l), sans(13.0), c.text2);
            }
        });
        ui.add_space(10.0);
        meta(ui, "These are alternatives for the same time, never added together.");
        ui.add_space(24.0);
        if primary(ui, "Create destination").clicked() {
            app.remember_focus(ui.ctx());
            app.dest = Some(DestDraft::new_destination(app));
        }
    });
}

/// What a destination cell or header asked for this frame.
#[derive(Default)]
struct CellOut {
    marks: Vec<ReportedMark>,
    unmark: Vec<Id>,
    copy: Option<String>,
    fix: Option<(usize, Option<Id>, String)>,
    edit_profile: Option<(usize, Step)>,
}

/// In-memory demonstration route used by the renderer, never a personal archive.
pub fn prepare_detail(app: &mut App) {
    if let Some(entry) = app.store.db.live_entries().filter(|e| !e.is_running()).max_by_key(|e| e.duration(now_secs())) {
        app.rep.period = Period::containing(PeriodKind::Day, local_date(entry.start), app.store.db.settings.week_starts_monday);
    }
    if let Some(pr) = app.store.db.live_profiles().first().cloned().cloned() {
        app.rep.selected_row = timesheet(&app.store.db, &pr, &app.rep.period, now_secs()).first().map(|l| l.key.clone());
        app.rep.dest = Some(pr.id.clone());
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use devtrack_core::{sample::{build_state, DemoState}, store::Store};

    #[test]
    fn every_destination_lists_its_lines_inside_the_window_with_details() {
        for width in [344.0, 760.0, 1160.0, 1600.0] {
            for dest in ["seed-a", "seed-b"] {
                let ctx = egui::Context::default();
                install_fonts(&ctx);
                let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Varied, false)), None);
                app.show_last_working_day();
                app.rep.dest = Some(dest.into());
                let pr = app.store.db.profiles[dest].clone();
                let lines = timesheet(&app.store.db, &pr, &app.rep.period, now_secs());
                assert!(lines.len() >= 3, "{dest}: the seed has lines to show");
                app.rep.selected_row = Some(lines[0].key.clone());
                for _ in 0..2 {
                    app.geometry.rows.clear();
                    let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 3000.0))), ..Default::default() }, |ctx| {
                        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| page(&mut app, ui));
                    });
                }
                let rows: Vec<_> = app.geometry.rows.iter().filter(|(n, _)| n.starts_with("report line")).collect();
                assert_eq!(rows.len(), lines.len(), "{width} {dest}: one row per value to enter");
                for (name, rect) in app.geometry.rows.iter().filter(|(n, _)| n.starts_with("report")) {
                    assert!(rect.left() >= -0.5 && rect.right() <= width + 1.0, "{width} {dest}: {name} {rect:?}");
                }
                assert!(app.geometry.rows.iter().any(|(n, _)| n.starts_with("report detail")), "{width} {dest}: the selected line shows its sessions");
                assert!(app.geometry.rows.iter().any(|(n, _)| n.starts_with("report source session")));
            }
        }
    }

    #[test]
    fn grouping_decides_how_many_values_there_are() {
        let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Varied, false)), None);
        app.show_last_working_day();
        let count = |app: &App, id: &str| timesheet(&app.store.db, &app.store.db.profiles[id], &app.rep.period, now_secs()).len();
        // by job code gathers projects; by project keeps them apart
        assert!(count(&app, "seed-a") < count(&app, "seed-b"));
    }
}
