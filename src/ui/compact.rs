//! Compact window: the same data and the same timer in a small window (430 × 720 at first,
//! 380 × 480 at least). Header with Expand, the current session with its controls, today's
//! total, a needs-details banner and the history grouped by day (today and the previous six
//! days, "Load earlier" for more). Settings opens the full window on Settings.
//! Dialog policy: Edit and Add time open here as normal dialogs; switching window mode is
//! refused while a dialog is open, so unsaved changes are never lost by a resize.

use super::today::{incomplete_in, missing_text, open_edit, Past};
use super::i18n::trf_m;
use super::i18n::tr;
use super::i18n::plural;
use super::widgets::*;
use super::{App, Page};
use chrono::{Duration as CDur, NaiveDate};
use devtrack_core::model::*;
use devtrack_core::report::{day_pieces, needs_completion, today, Period, PeriodKind};
use devtrack_core::store::now_secs;
use devtrack_core::timefmt::clock;
use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use std::collections::BTreeMap;

pub struct CompactState {
    pub days_loaded: i64,
}

impl Default for CompactState {
    fn default() -> Self {
        CompactState { days_loaded: 7 }
    }
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let c = pal(app.dark());
    let h24 = app.store.db.settings.clock_24h;
    // ---- header ----
    egui::TopBottomPanel::top("compact-head").frame(egui::Frame::new().fill(c.nav).inner_margin(egui::Margin::symmetric(16, 10)).stroke(Stroke::new(1.0, c.line))).show(ctx, |ui| {
        ui.horizontal(|ui| {
            // macOS paints the window buttons over the start of this bar
            let bar = ui.max_rect();
            super::window_drag(ui, bar.expand2(egui::vec2(16.0, 10.0)), "compact");
            let expand = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let k = if cfg!(target_os = "macos") { "⌘⇧M" } else { "Ctrl+Shift+M" };
                let r = icon_button(ui, Glyph::Expand, &trf_m!("Expand window ({})", k));
                if r.clicked() {
                    app.exit_compact(ctx, None);
                }
                r.rect
            }).inner;
            // the logo in the middle of the bar, clear of the window buttons on the left and of
            // Expand on the right
            let w = brand_lockup_width(LOGO_H, true);
            let spin = app.logo_spins();
            let at = egui::Rect::from_min_size(egui::pos2(bar.center().x - w / 2.0, expand.center().y - LOGO_H / 2.0), egui::vec2(w, LOGO_H));
            let mut logo = ui.new_child(egui::UiBuilder::new().max_rect(at).layout(egui::Layout::left_to_right(egui::Align::Center)));
            brand_lockup(&mut logo, LOGO_H, true, spin);
        });
    });
    // ---- footer ----
    egui::TopBottomPanel::bottom("compact-foot").frame(egui::Frame::new().fill(c.nav).inner_margin(egui::Margin::symmetric(16, 10)).stroke(Stroke::new(1.0, c.line))).show(ctx, |ui| {
        ui.horizontal(|ui| {
            // the same Settings item as the sidebar's: amber icon and name, no frame; a fill on hover
            let name = tr(Page::Settings.name());
            let text_w = ui.painter().layout_no_wrap(name.clone(), sans(14.0), c.text).size().x;
            // 12 out to the left, like the day bands: the icon sits on the content's edge
            ui.add_space(-12.0);
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(34.0 + text_w + 12.0, 38.0), Sense::click());
            if resp.hovered() || resp.has_focus() {
                ui.painter().rect_filled(rect, RADIUS, c.hover);
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.has_focus() {
                ui.painter().rect_stroke(rect.expand(2.0), RADIUS + 2.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
            }
            draw_icon(ui.painter(), rect.left_center() + egui::vec2(16.0, 0.0), Page::Settings.icon(), c.amber);
            ui.painter().text(rect.left_center() + egui::vec2(34.0, 0.0), egui::Align2::LEFT_CENTER, &name, sans(14.0), c.text);
            let n = name.clone();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, n.clone()));
            if resp.on_hover_text(tr("Opens Settings in the full window")).clicked() {
                app.exit_compact(ctx, Some(Page::Settings));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(problem) = app.settings_problem() {
                    ui.label(RichText::new(tr("Settings need attention")).size(12.0).color(c.amber)).on_hover_text(problem);
                }
            });
        });
    });
    // ---- session, totals, banner: fixed when there is room, scrolling with the history when short ----
    let tall = ctx.screen_rect().height() >= 620.0;
    if tall {
        egui::TopBottomPanel::top("compact-session").frame(egui::Frame::new().fill(c.canvas).inner_margin(egui::Margin { left: 16, right: 16, top: 16, bottom: 8 })).show_separator_line(false).show(ctx, |ui| top(app, ui));
    }
    // ---- history ----
    // no margin below: the rows scroll right up to the footer's line, not to a gap above it
    egui::CentralPanel::default().frame(egui::Frame::new().fill(c.canvas).inner_margin(egui::Margin { left: 16, right: 4, top: 0, bottom: 0 })).show(ctx, |ui| {
        let mut scroll = egui::ScrollArea::vertical().id_salt("compact-history").auto_shrink([false, false]);
        if SHOT_SCROLLED.load(std::sync::atomic::Ordering::Relaxed) {
            // review pictures: a day's band pinned at the top, the next one about to push it
            scroll = scroll.vertical_scroll_offset(SHOT_SCROLL_Y);
        }
        scroll.show(ui, |ui| {
            // the scroll bar, which egui draws over the content, gets the 12 px gutter: the rows
            // end on the same 16 px margin as the controls above
            ui.set_max_width(ui.available_width() - 12.0);
            if !tall {
                ui.add_space(16.0);
                top(app, ui);
            }
            history(app, ui, h24);
        });
    });
}

/// Review pictures only: the history scrolled part way (`--page compact-scrolled`).
pub(super) static SHOT_SCROLLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const SHOT_SCROLL_Y: f32 = 420.0;

/// Height of the logo's mark in the compact window's bar.
const LOGO_H: f32 = 24.0;

fn top(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
        session(app, ui);
        ui.add_space(12.0);
        // today's total is shown once, in the "Today" header of the history below
        let week = Period { kind: PeriodKind::Week, start: today() - CDur::days(app.compact_ui.days_loaded - 1), end: today() + CDur::days(1) };
        let inc = incomplete_in(app, &week);
        if !inc.is_empty() {
            ui.add_space(8.0);
            // the same status box as Today and Report Time, on the right: one click opens the
            // first session to complete
            let text = format!("! {}", plural(inc.len() as i64, "{} session needs details", "{} sessions need details"));
            let mut review = false;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                review = status_chip(ui, &text, c.amber, Some("Complete the first one")).clicked();
            });
            if review {
                let e = inc[0].clone();
                open_edit(app, &e, Some(ui.ctx()), true);
            }
        }
        ui.add_space(12.0);
        row(ui, CTRL_H, |ui| {
            ui.label(RichText::new(tr("Recent sessions")).font(medium(16.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if secondary(ui, "+ Add past time").on_hover_text(tr("Add a session that already happened")).clicked() {
                    app.remember_focus(ui.ctx());
                    app.today.past = Past::default();
                    app.today.past_open = true;
                }
                // one line per task and day, as Today's "By task"
                let on = app.store.local.list_by_task;
                if icon_toggle(ui, Glyph::Group, "Group by task", on).clicked() {
                    app.store.local.list_by_task = !on;
                    app.store.save_local();
                }
            });
        });
    }

fn session(app: &mut App, ui: &mut egui::Ui) {
    let c = p(ui);
    let now = now_secs();
    let w = ui.available_width();
    let half = (w - ROW_GAP) / 2.0;
    if let Some(e) = app.store.db.running().cloned() {
        // what is being tracked can change while it runs, as on Today: the task is a field with the
        // same suggestions, the project a link to its list; the timer keeps counting
        super::today::description(app, ui, Some(&e), w);
        let e = app.store.db.running().cloned().unwrap_or(e);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            // the links sit as on Today: no gap but the separator's own
            ui.spacing_mut().item_spacing.x = 0.0;
            let rp = recent_projects(&app.store.db);
            let mut proj = e.project_id.clone();
            let pick = project_picker_in(ui, "compact-run-project", &app.store.db, &rp, &mut proj, Trigger::Link, None);
            let proj = match pick {
                Pick::Create(n) => Some(app.create_project(&n)),
                Pick::Changed => proj,
                _ => e.project_id.clone(),
            };
            if proj != e.project_id {
                // a task of another project no longer fits the session
                let keep_task = app.store.db.task(&e.task_id).is_some_and(|t| t.project_id.is_none() || t.project_id == proj);
                let billable = app.store.db.project(&proj).map(|p| p.billable_default).unwrap_or(e.billable);
                app.store.update_running(|x| {
                    x.project_id = proj.clone();
                    if !keep_task { x.task_id = None; }
                    x.billable = billable;
                });
                app.notify(tr("Project changed: the session keeps running"));
            }
            // the client follows the project, as on Today; chosen here it narrows the project to
            // its own (the only one, when it has one)
            ui.label(RichText::new("  ·  ").size(13.0).color(c.text2));
            let mut chosen = app.store.db.project(&e.project_id).and_then(|p| p.client_id.clone()).filter(|id| app.store.db.client(&Some(id.clone())).is_some());
            let before = chosen.clone();
            let cp = client_select_with(ui, "compact-run-client", &app.store.db, &mut chosen, Trigger::Link);
            if let Pick::Create(name) = &cp {
                let cl = Client { id: new_id(), name: name.trim().to_string(), ..Default::default() };
                chosen = Some(cl.id.clone());
                app.store.put_client(cl);
            }
            if chosen != before {
                let own: Vec<Id> = chosen.as_ref().map(|cid| app.store.db.projects_of_client(cid).into_iter().filter(|p| !p.archived).map(|p| p.id.clone()).collect()).unwrap_or_default();
                let proj = if own.len() == 1 { Some(own[0].clone()) } else { None };
                let keep_task = app.store.db.task(&e.task_id).is_some_and(|t| t.project_id.is_none() || t.project_id == proj);
                app.store.update_running(|x| {
                    x.project_id = proj.clone();
                    if !keep_task { x.task_id = None; }
                });
                app.notify(tr("Project changed: the session keeps running"));
            }
            if app.store.db.project(&e.project_id).is_none() {
                ui.add_space(8.0);
                ui.label(RichText::new(tr("! Choose a project")).size(12.0).color(c.amber));
            }
        });
        // running: the green dot beside the time, as in the full window, instead of a line of its own
        let t = ui.allocate_ui_with_layout(egui::vec2(w, 48.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            dot(ui, c.green, 4.0);
            super::today::running_timer(ui, &e, now, 34.0)
        }).inner;
        if t.clicked() || std::mem::take(&mut app.today.run_time_request) {
            super::today::open_run_time(app, ui, &e, t.rect);
        }
        super::today::run_time_popover(app, ui, &e, &t);
        ui.horizontal(|ui| {
            if button_w(ui, Kind::Secondary, "Pause", half, true).on_hover_text(tr("⌘/Ctrl+P")).clicked() {
                app.store.pause();
            }
            if button_w(ui, Kind::Primary, "Stop", half, true).on_hover_text(tr("⌘/Ctrl+Enter")).clicked() {
                app.store.stop();
                app.notify(tr("Session saved"));
            }
        });
    } else if let Some(pe) = app.store.paused().cloned() {
        ui.horizontal(|ui| {
            dot(ui, c.amber, 3.5);
            ui.label(RichText::new(tr("Paused")).size(12.0).color(c.amber));
        });
        let what = super::session_title(&app.store.db, &pe);
        ui.add(egui::Label::new(RichText::new(&what).font(medium(16.0))).truncate());
        ui.label(mono_m(clock(pe.duration(now)), 34.0).color(c.text2));
        ui.horizontal(|ui| {
            if button_w(ui, Kind::Primary, "Resume", w, true).on_hover_text(tr("Continue it as a new piece (⌘/Ctrl+P)")).clicked() {
                app.store.resume();
            }
        });
    } else {
        let rp = recent_projects(&app.store.db);
        let mut enter = false;
        field(ui, "Description", None, None, |ui| {
            let mut text = app.today.draft.desc.clone();
            let r = input(ui, &mut text, Field::new("Description", w).hint("What are you working on?"));
            app.today.draft.desc = text.clone();
            // same suggestions as Today: earlier sessions and projects while typing
            enter = super::today::suggest_under(app, ui, &r, last_input_rect(ui), &text, None, w);
        });
        ui.add_space(ROW_GAP - LABEL_GAP);
        ui.horizontal_top(|ui| {
            let mut proj = app.today.draft.project.clone();
            field(ui, "Project", None, None, |ui| {
                if let Pick::Create(n) = project_picker(ui, "compact-project", &app.store.db, &rp, &mut proj, w - BTN_MIN_W - ROW_GAP) {
                    proj = Some(app.create_project(&n));
                }
            });
            if proj != app.today.draft.project {
                app.today.draft.billable = app.store.db.project(&proj).map(|p| p.billable_default).unwrap_or(false);
                app.today.draft.task = None;
            }
            app.today.draft.project = proj;
            field(ui, " ", None, None, |ui| {
                if button_w(ui, Kind::Primary, "Start", BTN_MIN_W, true).on_hover_text(tr("⌘/Ctrl+Enter")).clicked() || enter {
                    app.start_draft();
                }
            });
        });
    }
}

fn history(app: &mut App, ui: &mut egui::Ui, h24: bool) {
    let c = p(ui);
    let now = now_secs();
    let first = today() - CDur::days(app.compact_ui.days_loaded - 1);
    let mut by_day: BTreeMap<NaiveDate, Vec<(TimeEntry, i64, i64)>> = BTreeMap::new();
    for e in app.store.db.live_entries().filter(|e| !e.is_running()) {
        for (d, a, b) in day_pieces(e, now) {
            if d >= first && d <= today() {
                by_day.entry(d).or_default().push((e.clone(), a, b));
            }
        }
    }
    // day totals include the running session, so "Today" here matches the Today page
    let mut running_by_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    if let Some(e) = app.store.db.running() {
        for (d, a, b) in day_pieces(e, now) {
            *running_by_day.entry(d).or_default() += b - a;
        }
    }
    let mut action: Option<(u8, TimeEntry)> = None;
    // what the session popover points at: the row, or the ··· button its menu came from
    let mut anchor: Option<egui::Rect> = None;
    if by_day.is_empty() {
        ui.add_space(12.0);
        meta(ui, trf_m!("No sessions in the last {} days.", app.compact_ui.days_loaded));
    }
    let w = ui.available_width();
    // where each day's band sits in the list, for the one that stays at the top while scrolling
    let mut bands: Vec<(egui::Rect, NaiveDate, i64)> = Vec::new();
    let by_task = app.store.local.list_by_task;
    let profiles: Vec<devtrack_core::model::TimesheetProfile> = app.store.db.live_profiles().into_iter().cloned().collect();
    let mut day_copy: Option<String> = None;
    // one session's row (also a task group's sessions when it is open)
    let draw_row = |app: &mut App, ui: &mut egui::Ui, e: &TimeEntry, a: &i64, b: &i64, action: &mut Option<(u8, TimeEntry)>, anchor: &mut Option<egui::Rect>| {
        let mut actions_left = f32::INFINITY;
        // line 1: title · duration · ▶ · "···"; line 2 (full row width): time and project
        let row = ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(6.0);
            ui.horizontal(|ui| {
            col(ui, (w - 76.0 - CTRL_H * 2.0 - 4.0 - ROW_GAP * 2.0).max(120.0), CTRL_H, |ui| {
                ui.add_space(8.0);
                let title = super::session_title(&app.store.db, e);
                ui.add(egui::Label::new(RichText::new(title).font(medium(14.0))).truncate());
            });
            col_right(ui, 76.0, CTRL_H, |ui| {
                ui.add_space(8.0);
                ui.label(mono(super::shown_duration(&app.store.db, b - a), 14.0));
            });
            ui.spacing_mut().item_spacing.x = 4.0;
            let resume = icon_ghost(ui, Glyph::Play, "Continue now");
            actions_left = resume.rect.left();
            if resume.clicked() {
                *action = Some((0, e.clone()));
            }
            let more = icon_button(ui, Glyph::More, "Session actions");
            let pid = ui.make_persistent_id(("compact-more", &e.id, *a));
            if more.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pid));
            }
            egui::popup_below_widget(ui, pid, &more, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(180.0);
                if menu_row(ui, "Continue now", false, false).clicked() {
                    *action = Some((0, e.clone()));
                }
                if menu_row(ui, "Edit…", false, false).clicked() {
                    *action = Some((1, e.clone()));
                    *anchor = Some(more.rect);
                }
                if menu_row(ui, "Delete", false, false).clicked() {
                    *action = Some((2, e.clone()));
                }
            });
            });
            ui.horizontal(|ui| {
                ui.set_max_width(w);
                ui.label(range_job(&clock_text(*a, h24), &clock_text(*b, h24), 12.0, c.text2));
                let miss = missing_text(e);
                if !miss.is_empty() {
                    // secondary text shrinks with the row, never pushes it wider
                    ui.add(egui::Label::new(RichText::new(tr(miss)).size(12.0).color(c.amber)).truncate());
                } else if let Some(pr) = app.store.db.project(&e.project_id) {
                    dot(ui, hex(&pr.color), 3.0);
                    ui.add(egui::Label::new(RichText::new(&pr.name).size(12.0).color(c.text2)).truncate());
                }
            });
            ui.add_space(10.0);
        });
        // the row opens the editor; its action buttons keep their own clicks
        let r = row.response.rect;
        let click = egui::Rect::from_min_max(r.min, egui::pos2(actions_left.min(r.right()) - ROW_GAP, r.bottom()));
        if ui.interact(click, egui::Id::new(("compact-row", &e.id, *a)), Sense::click()).clicked() {
            *action = Some((1, e.clone()));
            *anchor = Some(r);
        }
    };
    for (date, mut list) in by_day.into_iter().rev() {
        list.sort_by_key(|x| std::cmp::Reverse(x.1));
        // a day is a band, as in Today's list; it opens the group, so no line under it
        ui.add_space(16.0);
        let (slot, _) = ui.allocate_exact_size(egui::vec2(w, 32.0), Sense::hover());
        let total = list.iter().map(|x| x.2 - x.1).sum::<i64>() + running_by_day.get(&date).copied().unwrap_or(0);
        day_band(ui, slot, date, total, by_task);
        if by_task {
            // the day's totals per task, ready for a spreadsheet: the button sits where the band ends
            let br = egui::Rect::from_min_size(egui::pos2(slot.right() + 12.0 - 2.0 - BAND_COPY, slot.center().y - BAND_COPY / 2.0), egui::vec2(BAND_COPY, BAND_COPY));
            let resp = ui.interact(br, egui::Id::new(("compact-day-copy", date)), Sense::click());
            let col = if resp.hovered() { c.text } else { c.text2 };
            if resp.hovered() {
                ui.painter().rect_filled(br, RADIUS, c.hover);
            }
            draw_glyph(ui.painter(), br.center(), Glyph::Copy, col);
            let name = tr("Copy the day's totals per task");
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone()));
            if resp.on_hover_text(tr("Copy the day's totals per task")).clicked() {
                let groups = super::by_task::group(&list);
                day_copy = Some(super::by_task::day_text(&app.store.db, &groups, &profiles, |e| super::session_title(&app.store.db, e)));
            }
        }
        bands.push((slot, date, total));
        ui.add_space(2.0);
        let rows_in_day = list.len();
        if by_task {
            let groups = super::by_task::group(&list);
            let n_groups = groups.len();
            for (gi, g) in groups.into_iter().enumerate() {
                if g.pieces.len() == 1 {
                    let (e, a, b) = &g.pieces[0];
                    draw_row(app, ui, e, a, b, &mut action, &mut anchor);
                } else {
                    let gid = super::by_task::group_id(date, &g);
                    let open = app.today.open_groups.contains(&gid);
                    let (toggle, resume) = group_line(app, ui, &g, open, w);
                    if resume {
                        action = Some((0, g.latest().clone()));
                    }
                    if toggle {
                        if open { app.today.open_groups.remove(&gid); } else { app.today.open_groups.insert(gid.clone()); }
                    }
                    if open {
                        let mut sorted = g.pieces.clone();
                        sorted.sort_by_key(|x| std::cmp::Reverse(x.1));
                        for (e, a, b) in &sorted {
                            divider(ui);
                            let top = ui.cursor().top();
                            draw_row(app, ui, e, a, b, &mut action, &mut anchor);
                            let x = ui.max_rect().left() - 6.0;
                            ui.painter().line_segment([egui::pos2(x, top + 8.0), egui::pos2(x, ui.cursor().top() - 8.0)], Stroke::new(2.0, c.border));
                        }
                    }
                }
                if gi + 1 < n_groups {
                    divider(ui);
                }
            }
            continue;
        }
        for (ri, (e, a, b)) in list.iter().enumerate() {
            draw_row(app, ui, e, a, b, &mut action, &mut anchor);
            // between sessions of a day; the next day's band does the separating after the last
            if ri + 1 < rows_in_day {
                divider(ui);
            }
        }
    }
    sticky_band(ui, &bands, by_task);
    if let Some(t) = day_copy {
        ui.ctx().copy_text(t);
        app.notify(tr("The day's totals per task are copied: paste them into a spreadsheet"));
    }
    ui.add_space(12.0);
    if link_btn(ui, "Load earlier").on_hover_text(tr("Show seven more days")).clicked() {
        app.compact_ui.days_loaded += 7;
    }
    // the room the panel's margin gave, now at the end of the list
    ui.add_space(12.0);
    if let Some((k, e)) = action {
        match k {
            0 => app.store.continue_entry(&e.id),
            1 => {
                let review = needs_completion(&e);
                open_edit(app, &e, Some(ui.ctx()), review);
                app.today.edit_anchor = anchor;
            }
            _ => {
                if devtrack_core::report::locked(&app.store.db, e.start) {
                    let m = super::today::locked_text(app);
                    app.notify(m);
                } else {
                    app.store.delete_entry(&e.id);
                    app.notify_undo(tr("Session deleted"));
                }
            }
        }
    }
}

/// A day's band: a step above the window's fill, the weekday in full colour, the date, and the
/// day's total on the right. It reaches 12 past the rows on each side, so its words start on the
/// rows' edge.
/// The copy button at the end of a day's band, when the list is grouped by task.
const BAND_COPY: f32 = 28.0;

fn day_band(ui: &egui::Ui, slot: egui::Rect, date: NaiveDate, total: i64, copy_room: bool) {
    let c = p(ui);
    let band = slot.expand2(egui::vec2(12.0, 0.0));
    // between the nav fill and the selection: clearly a band in both themes (the rows' hover is lighter)
    let fill = Color32::from_rgb(((c.nav.r() as u16 + c.selected.r() as u16) / 2) as u8, ((c.nav.g() as u16 + c.selected.g() as u16) / 2) as u8, ((c.nav.b() as u16 + c.selected.b() as u16) / 2) as u8);
    ui.painter().rect_filled(band, RADIUS, fill);
    let name = if date == today() { tr("Today") } else if date == today() - CDur::days(1) { tr("Yesterday") } else { super::i18n::weekday_title(date) };
    let gn = ui.painter().layout_no_wrap(name, medium(13.0), c.text);
    let gd = ui.painter().layout_no_wrap(super::i18n::short_date(date), mono_font(12.0), c.text2);
    let y = band.center().y;
    let x0 = slot.left();
    let wn = gn.size().x;
    ui.painter().galley(egui::pos2(x0, y - gn.size().y / 2.0), gn, c.text);
    ui.painter().galley(egui::pos2(x0 + wn + 8.0, y - gd.size().y / 2.0), gd, c.text2);
    let gt = ui.painter().layout_job(mixed_job(&words(total), 13.0, c.text, medium(13.0), mono_medium(13.0)));
    let right = if copy_room { slot.right() + 12.0 - 2.0 - BAND_COPY - 8.0 } else { slot.right() };
    ui.painter().galley(egui::pos2(right - gt.size().x, y - gt.size().y / 2.0), gt, c.text);
}

/// One task's line in the compact list: its title and the day's total on it, Continue and the
/// chevron that shows its sessions; under them, how many sessions and the project. A click on the
/// line shows or hides the sessions. Returns (toggle, continue).
fn group_line(app: &mut App, ui: &mut egui::Ui, g: &super::by_task::TaskGroup, open: bool, w: f32) -> (bool, bool) {
    let c = p(ui);
    let e = g.latest().clone();
    let mut toggle = false;
    let mut resume = false;
    let mut actions_left = f32::INFINITY;
    let row = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            col(ui, (w - 76.0 - CTRL_H * 2.0 - 4.0 - ROW_GAP * 2.0).max(120.0), CTRL_H, |ui| {
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new(super::session_title(&app.store.db, &e)).font(medium(14.0))).truncate());
            });
            col_right(ui, 76.0, CTRL_H, |ui| {
                ui.add_space(8.0);
                ui.label(mono(super::shown_duration(&app.store.db, g.total), 14.0));
            });
            ui.spacing_mut().item_spacing.x = 4.0;
            let play = icon_ghost(ui, Glyph::Play, "Continue now");
            actions_left = play.rect.left();
            resume = play.clicked();
            toggle = icon_button(ui, if open { Glyph::Down } else { Glyph::Next }, if open { "Hide the sessions" } else { "Show the sessions" }).clicked();
        });
        ui.horizontal(|ui| {
            ui.set_max_width(w);
            mixed(ui, &plural(g.pieces.len() as i64, "{} session", "{} sessions"), 12.0, Some(c.text2));
            if let Some(pr) = app.store.db.project(&e.project_id) {
                dot(ui, hex(&pr.color), 3.0);
                ui.add(egui::Label::new(RichText::new(&pr.name).size(12.0).color(c.text2)).truncate());
            }
        });
        ui.add_space(10.0);
    });
    let r = row.response.rect;
    let click = egui::Rect::from_min_max(r.min, egui::pos2(actions_left.min(r.right()) - ROW_GAP, r.bottom()));
    let resp = ui.interact(click, egui::Id::new(("compact-group", &g.key, g.first)), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    toggle |= resp.clicked();
    (toggle, resume)
}

/// The band of the day being read stays at the top of the list until the next day's band
/// reaches it and pushes it up and out.
fn sticky_band(ui: &mut egui::Ui, bands: &[(egui::Rect, NaiveDate, i64)], copy_room: bool) {
    let top = ui.clip_rect().top();
    let Some((i, &(slot, date, total))) = bands.iter().enumerate().filter(|(_, b)| b.0.top() < top).last() else { return };
    let next = bands.get(i + 1).map(|b| b.0.top());
    let at = sticky_top(top, slot.height(), next);
    let pinned = egui::Rect::from_min_size(egui::pos2(slot.left(), at), slot.size());
    // the window's fill behind it, so the rows scrolling under don't show at its rounded corners
    let c = p(ui);
    ui.painter().rect_filled(pinned.expand2(egui::vec2(12.0, 0.0)), 0.0, c.canvas);
    day_band(ui, pinned, date, total, copy_room);
    // it covers the rows under it, clicks included
    ui.interact(pinned, egui::Id::new("compact-sticky-band"), Sense::click());
}

/// Where the pinned band's top goes: at the list's top edge, or higher when the next band is
/// already climbing into its place.
fn sticky_top(top: f32, height: f32, next_top: Option<f32>) -> f32 {
    match next_top {
        Some(n) => top.min(n - height),
        None => top,
    }
}

#[cfg(test)]
mod tests {
    use super::sticky_top;

    #[test]
    fn the_pinned_band_waits_at_the_top_then_is_pushed_up_by_the_next() {
        // next day still far below: stays on the edge
        assert_eq!(sticky_top(100.0, 32.0, Some(300.0)), 100.0);
        // next day's band 20 px under the edge: the pinned one sits right above it
        assert_eq!(sticky_top(100.0, 32.0, Some(120.0)), 88.0);
        // last day: nothing to push it
        assert_eq!(sticky_top(100.0, 32.0, None), 100.0);
    }
}
