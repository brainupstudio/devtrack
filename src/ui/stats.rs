//! Statistics: totals per day, project and task, at a size proportionate to the data.
//! Uses the same per-day allocation as Today and Report Time, so numbers agree.

use super::i18n::{plural, tr};
use super::i18n::trf_m;
use super::widgets::*;
use super::{page_header, App, Page};
use devtrack_core::model::*;
use devtrack_core::report::{needs_completion, summarize, Period, PeriodKind, Summary};
use devtrack_core::store::now_secs;
use eframe::egui::{self, Color32, RichText, Sense, Stroke};

pub struct StatsState {
    pub period: Period,
    pub project: Option<Id>,
}

impl StatsState {
    pub fn new(period: Period) -> Self {
        StatsState { period, project: None }
    }
}

pub fn page(app: &mut App, ui: &mut egui::Ui) {
    let now = now_secs();
    let c = p(ui);
    let monday = app.store.db.settings.week_starts_monday;
    page_header(ui, "Statistics", "Where your tracked time went.", |_| {});
    let mut period = app.stats.period;
    let mut filter = app.stats.project.clone();
    let bar = super::period::period_toolbar(ui, &mut period, monday, [true, true, true], 220.0, |ui| {
        project_filter(ui, "stats-filter", &app.store.db, &mut filter, 220.0);
    });
    app.stats.project = filter;
    super::period::record(app, "stats", &bar);
    app.stats.period = period;
    ui.add_space(16.0);
    let s = summarize(&app.store.db, &period, now, app.stats.project.as_ref());
    if s.total == 0 {
        let last = app.last_day_with_sessions(period.start);
        let (mut today, mut go_back) = (false, None);
        let body = if app.stats.project.is_some() { "Nothing was tracked on this project in this period. Clear the project filter, or look at another period." } else { "Where the time went, by project and by day, appears once sessions are recorded in this period." };
        empty_state(ui, Icon::Stats, "No tracked time in this period", body, |ui| {
            today = secondary(ui, "Go to Today").clicked();
            if let Some(d) = last {
                if link_btn(ui, "Show the last day with sessions").clicked() { go_back = Some(d); }
            }
        });
        if today { app.go(super::Page::Today); }
        if let Some(d) = go_back {
            app.stats.period = devtrack_core::report::Period::containing(devtrack_core::report::PeriodKind::Day, d, monday);
        }
        return;
    }
    let tracked_days = s.by_day.iter().filter(|(_, d)| *d > 0).count();
    let list = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        divider(ui);
        let items = [
            ("Tracked", words(s.total), plural(s.entries as i64, "{} session", "{} sessions")),
            ("Billable", words(s.billable), trf_m!("{} of tracked time", pct(s.billable, s.total))),
            ("Average per tracked day", words(s.total / tracked_days.max(1) as i64), plural(tracked_days as i64, "over {} tracked day", "over {} tracked days")),
        ];
        // three across while each value has room for "000 h 00 min" at 24; stacked below that
        let across = ui.available_width() >= 620.0;
        let (cols, rows) = if across { (3.0, 1.0) } else { (1.0, 3.0) };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 86.0 * rows), Sense::hover());
        let w = rect.width() / cols;
        for (i, (l, v, sub)) in items.iter().enumerate() {
            let (x, top) = if across { (rect.left() + w * i as f32, rect.top()) } else { (rect.left(), rect.top() + 86.0 * i as f32) };
            ui.painter().text(egui::pos2(x, top + 12.0), egui::Align2::LEFT_TOP, tr(l), sans(12.0), c.text2);
            // digits in Mono, "h" and "min" in Sans, like every other duration in words
            let g = ui.painter().layout_job(mixed_job(v, 24.0, c.text, medium(24.0), mono_medium(24.0)));
            ui.painter().galley(egui::pos2(x, top + 30.0), g, c.text);
            let job = mixed_job(&tr(sub), 12.0, c.text2, sans(12.0), mono_font(12.0));
            let g = ui.painter().layout_job(job);
            ui.painter().galley(egui::pos2(x, top + 62.0), g, c.text2);
        }
        divider(ui);
        ui.add_space(24.0 - ui.spacing().item_spacing.y);
        if period.kind != PeriodKind::Day {
            section(ui, "Per day");
            if tracked_days <= 1 {
                meta(ui, "Only one tracked day so far: a day-by-day chart appears once there's more.");
            } else {
                bars(app, ui, &s, &period);
            }
            ui.add_space(24.0);
        }
        let mut review = false;
        let two = ui.available_width() >= 760.0;
        // 32 between the two lists; rows inside keep the normal 8
        let gap = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 32.0;
        ui.columns(if two { 2 } else { 1 }, |col| {
            for c in col.iter_mut() { c.spacing_mut().item_spacing.x = gap; }
            col[0].vertical(|ui| {
                section(ui, "Projects");
                ui.add_space(4.0);
                for (pid, d) in &s.by_project {
                    let (name, color) = app.store.db.project(pid).map(|p| (p.name.clone(), hex(&p.color))).unwrap_or(("No project".into(), c.text2));
                    let row = ui.horizontal(|ui| {
                        ui.set_min_height(CTRL_H);
                        dot(ui, color, 3.5);
                        ui.add(egui::Label::new(truncate(&name, 30)).truncate()).on_hover_text(&name);
                        if pid.is_none() && ghost(ui, "Review").on_hover_text(tr("Assign these sessions to a project")).clicked() {
                            review = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_sized(egui::vec2(52.0, 20.0), egui::Label::new(mono(pct(*d, s.total), 12.0).color(c.text2)));
                            mixed(ui, &words(*d), 14.0, None);
                        });
                    });
                    app.geometry.rows.push((format!("stats project row · {}", pid.as_deref().unwrap_or("none")), row.response.rect));
                    let frac = *d as f32 / s.total.max(1) as f32;
                    let line = breakdown_footer(ui, Some((frac, if pid.is_none() { c.text2 } else { color.linear_multiply(0.8) })));
                    app.geometry.rows.push(("stats project separator".into(), line));
                }
            });
            if col.len() > 1 {
                col[1].vertical(|ui| tasks(app, ui, &s));
            }
        });
        ui.spacing_mut().item_spacing.x = gap;
        if !two {
            ui.add_space(16.0);
            tasks(app, ui, &s);
        }
        // time by tag: a session with two tags counts under both, so these don't add up to the total
        let mut by_tag: std::collections::BTreeMap<String, i64> = Default::default();
        let (pa, pb) = period.bounds();
        for e in devtrack_core::report::entries_in(&app.store.db, &period, now) {
            if app.stats.project.is_some() && e.project_id != app.stats.project { continue; }
            for t in &e.tags {
                *by_tag.entry(t.clone()).or_default() += devtrack_core::report::overlap(e, pa, pb, now);
            }
        }
        if !by_tag.is_empty() {
            ui.add_space(24.0);
            section(ui, "Tags");
            meta(ui, "A session with several tags counts under each of them.");
            ui.add_space(4.0);
            let mut list: Vec<(String, i64)> = by_tag.into_iter().collect();
            list.sort_by_key(|x| std::cmp::Reverse(x.1));
            let w = ui.available_width().min(560.0);
            for (tag, secs) in list {
                ui.allocate_ui_with_layout(egui::vec2(w, CTRL_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_width(w);
                    ui.label(format!("#{}", tag));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        mixed(ui, &words(secs), 14.0, None);
                    });
                });
                breakdown_footer(ui, None);
            }
        }
        if review {
            let first = app.store.db.live_entries().filter(|e| e.project_id.is_none() && !e.is_running() && period.contains(devtrack_core::report::local_date(e.start))).max_by_key(|e| e.start).cloned();
            app.today.period = Period::containing(PeriodKind::Day, first.as_ref().map(|e| devtrack_core::report::local_date(e.start)).unwrap_or(period.start), monday);
            app.today.only_incomplete = app.store.db.live_entries().any(needs_completion);
            app.go(Page::Today);
        }
    });
    scroll_fades(ui, &list, p(ui).canvas);
}

fn tasks(app: &mut App, ui: &mut egui::Ui, s: &Summary) {
    let c = p(ui);
    section(ui, "Tasks");
    ui.add_space(4.0);
    for (pid, tid, d) in s.by_task.iter().take(12) {
        let row = ui.horizontal(|ui| {
            ui.set_min_height(CTRL_H);
            dot(ui, app.store.db.project(pid).map(|p| hex(&p.color)).unwrap_or(c.text2), 3.5);
            match app.store.db.task(tid) {
                Some(t) => {
                    if let Some(id) = &t.external_id {
                        ui.label(mono(format!("#{}", id), 12.0).color(c.text2));
                    }
                    // the title takes the row up to the duration; it is cut only when it must be
                    ui.scope(|ui| {
                        ui.set_max_width((ui.available_width() - 112.0).max(60.0));
                        ui.add(egui::Label::new(&t.title).truncate()).on_hover_text(&t.title);
                    });
                }
                None => {
                    muted(ui, "No task");
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                mixed(ui, &words(*d), 14.0, None);
            });
        });
        app.geometry.rows.push(("stats task row".into(), row.response.rect));
        let line = breakdown_footer(ui, None);
        app.geometry.rows.push(("stats task separator".into(), line));
    }
}

/// Both lists use the same footer slot and following gap, irrespective of progress.
fn breakdown_footer(ui: &mut egui::Ui, progress: Option<(f32, egui::Color32)>) -> egui::Rect {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.0), Sense::hover());
    if let Some((fraction, color)) = progress {
        ui.painter().rect_filled(r, 1.5, p(ui).line);
        ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(r.width() * fraction.clamp(0.0, 1.0), 3.0)), 1.5, color);
    } else {
        ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, p(ui).line));
    }
    ui.add_space(6.0);
    r
}

fn bars(app: &App, ui: &mut egui::Ui, s: &Summary, period: &Period) {
    let c = p(ui);
    // 20% taller than it was, so a day's projects don't read as thin slices
    let h = 132.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h + 40.0), Sense::hover());
    let pointer = resp.hover_pos();
    let mut hovered: Option<(String, Color32, i64, chrono::NaiveDate)> = None;
    let painter = ui.painter_at(rect);
    // consistent scale: at least an 8-hour day
    let max = s.by_day.iter().map(|(_, d)| *d).max().unwrap_or(0).max(8 * 3600) as f32;
    // days off with nothing recorded are left out of the chart
    let by_day: Vec<(chrono::NaiveDate, i64)> = s.by_day.iter().copied().filter(|(d, t)| *t > 0 || app.store.db.settings.is_work_day(*d)).collect();
    let n = by_day.len().max(1) as f32;
    let slot = (w - 40.0) / n;
    // columns about twice as wide as before: there is room between the days
    let bw = (slot * 0.6).min(56.0);
    let base = rect.top() + 12.0 + h;
    for hh in [0.5f32, 1.0] {
        let y = base - h * hh;
        painter.line_segment([egui::pos2(rect.left() + 34.0, y), egui::pos2(rect.right(), y)], Stroke::new(1.0, c.line));
        painter.text(egui::pos2(rect.left() + 28.0, y), egui::Align2::RIGHT_CENTER, format!("{:.0} h", max * hh / 3600.0), mono_font(12.0), c.text2);
    }
    painter.line_segment([egui::pos2(rect.left() + 34.0, base), egui::pos2(rect.right(), base)], Stroke::new(1.0, c.line));
    for (i, (date, total)) in by_day.iter().enumerate() {
        let cx = rect.left() + 40.0 + slot * (i as f32 + 0.5);
        let mut y = base;
        if let Some(parts) = s.by_day_project.get(date) {
            for (pid, d) in parts {
                let hgt = h * (*d as f32 / max);
                let full = app.store.db.project(pid).map(|p| hex(&p.color)).unwrap_or(c.text2);
                let seg = egui::Rect::from_min_max(egui::pos2(cx - bw / 2.0, y - hgt), egui::pos2(cx + bw / 2.0, y));
                // the slice under the pointer at full colour, the others slightly muted
                let over = pointer.is_some_and(|p| seg.contains(p));
                if over {
                    let name = app.store.db.project(pid).map(|p| p.name.clone()).unwrap_or_else(|| tr("No project"));
                    hovered = Some((name, full, *d, *date));
                }
                painter.rect_filled(seg, 2.0, if over { full } else { full.linear_multiply(0.85) });
                y -= hgt;
            }
        }
        if *total > 0 && slot > 34.0 {
            // in words like the rest of the page where a day has room; h:mm only in a month
            let g = if slot >= 84.0 {
                painter.layout_job(mixed_job(&words(*total), 12.0, c.text, sans(12.0), mono_font(12.0)))
            } else {
                painter.layout_no_wrap(format!("{}:{:02}", total / 3600, (total % 3600) / 60), mono_font(12.0), c.text)
            };
            painter.galley(egui::pos2(cx - g.size().x / 2.0, y - 8.0 - g.size().y / 2.0), g, c.text);
        }
        let label_txt = if period.kind == PeriodKind::Month { date.format("%-d").to_string() } else { format!("{} {}", super::i18n::weekday_short(*date), date.format("%d")) };
        painter.text(egui::pos2(cx, base + 12.0), egui::Align2::CENTER_CENTER, label_txt, mono_font(12.0), c.text2);
    }
    // which project a slice is: name, time and day, at the pointer
    if let Some((name, col, secs, date)) = hovered {
        resp.on_hover_ui_at_pointer(|ui| {
            ui.horizontal(|ui| {
                dot(ui, col, 4.0);
                ui.label(RichText::new(name).font(medium(13.0)));
            });
            ui.label(mixed_job(&format!("{} · {}", words(secs), super::i18n::short_date(date)), 12.0, c.text2, sans(12.0), mono_font(12.0)));
        });
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use devtrack_core::{sample::{build_state, DemoState}, store::Store};

    #[test]
    fn project_and_task_rows_share_text_and_separator_positions() {
        for width in [1024.0, 1440.0] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let mut app = App::new(Store::in_memory(build_state(now_secs(), DemoState::Incomplete, false)), None);
            app.stats.period = Period::containing(PeriodKind::Week, devtrack_core::report::today(), true);
            let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 1800.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| page(&mut app, ui));
            });
            let project: Vec<_> = app.geometry.rows.iter().filter(|(n, _)| n.starts_with("stats project row")).map(|(_, r)| r).collect();
            let task: Vec<_> = app.geometry.rows.iter().filter(|(n, _)| n == "stats task row").map(|(_, r)| r).collect();
            assert!(project.len().min(task.len()) >= 2);
            for (a, b) in project.iter().zip(task.iter()) {
                assert!((a.center().y - b.center().y).abs() < 0.5, "row text: {a:?} vs {b:?}");
            }
            let project: Vec<_> = app.geometry.rows.iter().filter(|(n, _)| n == "stats project separator").map(|(_, r)| r).collect();
            let task: Vec<_> = app.geometry.rows.iter().filter(|(n, _)| n == "stats task separator").map(|(_, r)| r).collect();
            for (a, b) in project.iter().zip(task.iter()) { assert!((a.center().y - b.center().y).abs() < 0.5, "separators: {a:?} vs {b:?}"); }
        }
    }
}
