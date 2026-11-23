//! One period toolbar for Today, Report Time and Statistics, with fixed geometry so nothing
//! moves when the period kind, the date, the totals or the status change.
//!
//! Moving in time is one group: Previous 36 · 8 · date 236 · 8 · Next 36 · 8 · Today 100. Then
//! 16 and the Day/Week/Month tabs at the width of their words. The status (Today's views,
//! Report Time's status) goes right on the same row when it fits (`status_fits`); otherwise
//! Today shows it on its summary row and the other pages on a row of their own.
//! Narrower than one row: the date and the tabs shrink a little first; then two rows (moving
//! in time; the tabs), Today joining the tabs when it does not fit beside Next.

use super::i18n::{lang, month, short_date, tr, weekday, Lang};
use super::widgets::*;
use chrono::{Datelike, Duration};
use devtrack_core::report::{today, Period, PeriodKind};
use eframe::egui::{self, Rect, Sense, Stroke, Ui};

pub const BTN: f32 = 36.0;
pub const GAP: f32 = 8.0;
/// Fits "16/11/2026 – 22/11/2026" in Mono with its padding; the word after it drops out first.
pub const LABEL_W: f32 = 236.0;
const LABEL_MIN_W: f32 = 168.0;
pub const TABS_GAP: f32 = 16.0;
pub const SEG_PAD: f32 = 4.0;
pub const TODAY_W: f32 = super::widgets::BTN_MIN_W;
pub const NAV_W: f32 = BTN + GAP + LABEL_W + GAP + BTN; // 324
const KINDS: [&str; 3] = ["Day", "Week", "Month"];

/// The Day/Week/Month segments at the width of their words (Medium, as when selected), 24 wider.
fn tabs_natural(ui: &Ui) -> [f32; 3] {
    KINDS.map(|k| (text_w(ui, &tr(k), &medium(14.0)) + 24.0).ceil())
}

fn tabs_total(w: &[f32]) -> f32 {
    w.iter().sum::<f32>() + SEG_PAD * 2.0
}

/// The whole bar on one row at full size.
pub fn full_w(ui: &Ui) -> f32 {
    NAV_W + GAP + TODAY_W + TABS_GAP + tabs_total(&tabs_natural(ui))
}

/// The narrowest one-row bar: a shorter date slot and tabs at 85%.
fn row_min_w(ui: &Ui) -> f32 {
    BTN * 2.0 + GAP * 3.0 + LABEL_MIN_W + TODAY_W + TABS_GAP + tabs_total(&tabs_natural(ui).map(|w| w * 0.85))
}

/// Whether a status this wide fits right of the bar on its row.
pub fn status_fits(ui: &Ui, status_w: f32) -> bool {
    status_w > 0.0 && ui.available_width() >= full_w(ui) + TABS_GAP + status_w
}

#[derive(Clone, Debug)]
pub struct PeriodBar {
    pub prev: Rect,
    pub label: Rect,
    pub next: Rect,
    pub tabs: Rect,
    pub today: Rect,
    /// "one-row", "one-row+status-row", "two-rows" or "narrow" (breakpoint, not content).
    pub layout: &'static str,
}

/// Full text for the period (accessible name and tooltip) and the parts drawn in the slot.
fn label_parts(p: &Period) -> (String, String, String) {
    let last = p.end - Duration::days(1);
    match p.kind {
        PeriodKind::Day => {
            let iso = short_date(p.start);
            let wd = weekday(p.start);
            (format!("{} {}", iso, wd), iso, wd)
        }
        PeriodKind::Week => {
            let iso = format!("{} – {}", short_date(p.start), short_date(last));
            let wk = match lang() {
                Lang::En => format!("week {}", p.start.iso_week().week()),
                Lang::It => format!("settimana {}", p.start.iso_week().week()),
            };
            (format!("{} ({})", iso, wk), iso, wk)
        }
        PeriodKind::Month => {
            let iso = format!("{} – {}", short_date(p.start), short_date(last));
            let m = month(p.start.month());
            (format!("{} ({} {})", iso, m, p.start.year()), iso, m)
        }
    }
}

/// Equal-width segmented control: every segment `seg_w` wide whatever the font or language.
pub fn segmented_equal(ui: &mut Ui, options: &[&str], selected: usize, seg_w: f32, enabled: &[bool]) -> (Option<usize>, Rect) {
    let widths: Vec<f32> = options.iter().map(|_| seg_w).collect();
    segmented_widths(ui, options, selected, &widths, enabled)
}

/// A segmented control whose segments take the given widths.
pub fn segmented_widths(ui: &mut Ui, options: &[&str], selected: usize, widths: &[f32], enabled: &[bool]) -> (Option<usize>, Rect) {
    let c = p(ui);
    let total = widths.iter().sum::<f32>() + SEG_PAD * 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total, CTRL_H), Sense::hover());
    ui.painter().rect(rect, RADIUS, c.field, Stroke::new(1.0, c.border), egui::StrokeKind::Inside);
    let mut out = None;
    for (i, o) in options.iter().enumerate() {
        // the group keeps its width; neighbours give up 2 each, so 4 separates them like the
        // padding around the group
        let (l, t) = (if i > 0 { SEG_PAD / 2.0 } else { 0.0 }, if i + 1 < options.len() { SEG_PAD / 2.0 } else { 0.0 });
        let x = rect.left() + SEG_PAD + widths[..i].iter().sum::<f32>();
        let r = Rect::from_min_size(egui::pos2(x + l, rect.top() + 4.0), egui::vec2(widths[i] - l - t, CTRL_H - 8.0));
        let on = enabled.get(i).copied().unwrap_or(true);
        let resp = ui.interact(r, ui.id().with(("seg-eq", i, *o)), if on { Sense::click() } else { Sense::hover() });
        let sel = i == selected;
        if sel {
            ui.painter().rect_filled(r, 4.0, c.selected);
        } else {
            ui.painter().rect_filled(r, 4.0, hover_mix(ui, resp.id, resp.hovered() && on, egui::Color32::TRANSPARENT, c.hover));
        }
        let label_txt = tr(o);
        // the selected period in amber, like every selected tab
        let col = if sel { c.amber } else if on { c.text2 } else { c.text2.linear_multiply(0.5) };
        let g = ui.painter().layout_no_wrap(label_txt.clone(), if sel { medium(14.0) } else { sans(14.0) }, col);
        ui.painter().galley(egui::pos2(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, col);
        if resp.has_focus() {
            ui.painter().rect_stroke(r.expand(1.0), 5.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
        }
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, on, sel, label_txt.clone()));
        if on && resp.clicked() && !sel {
            out = Some(i);
        }
    }
    (out, rect)
}

/// Draws the toolbar and changes `period` when asked. `status_w` is the width reserved for the
/// status (0 = none); `status` draws it right-aligned inside that reservation.
pub fn period_toolbar(ui: &mut Ui, period: &mut Period, monday: bool, kinds_enabled: [bool; 3], status_w: f32, status: impl FnOnce(&mut Ui)) -> PeriodBar {
    let c = p(ui);
    let avail = ui.available_width();
    let mut bar = PeriodBar { prev: Rect::NOTHING, label: Rect::NOTHING, next: Rect::NOTHING, tabs: Rect::NOTHING, today: Rect::NOTHING, layout: "" };
    let mut new_p = *period;
    let kinds = [PeriodKind::Day, PeriodKind::Week, PeriodKind::Month];
    let cur = kinds.iter().position(|k| *k == period.kind).unwrap_or(0);
    let natural = tabs_natural(ui);
    let (full_bar, row_min) = (full_w(ui), row_min_w(ui));
    let one_row = avail >= row_min;
    // 0 at the narrowest one-row bar, 1 at full size and beyond
    let grow = if full_bar > row_min { ((avail - row_min) / (full_bar - row_min)).clamp(0.0, 1.0) } else { 1.0 };
    let label_w = if one_row {
        LABEL_MIN_W + (LABEL_W - LABEL_MIN_W) * grow
    } else if avail >= NAV_W {
        LABEL_W
    } else {
        (avail - BTN * 2.0 - GAP * 2.0).max(0.0)
    };
    let tab_scale = if one_row { 0.85 + 0.15 * grow } else { ((avail - SEG_PAD * 2.0) / natural.iter().sum::<f32>()).min(1.0) };
    let tab_widths: Vec<f32> = natural.iter().map(|w| (w * tab_scale).floor()).collect();
    // Today sits beside Next while the row allows it
    let today_beside = one_row || avail >= NAV_W + GAP + TODAY_W;
    let (full, iso, extra) = label_parts(period);

    let nav = |ui: &mut Ui, bar: &mut PeriodBar, new_p: &mut Period| {
        let prev = icon_button(ui, Glyph::Prev, "Previous period");
        ui.add_space(GAP);
        // a field that looks like one acts like one: a click (or Enter / Space) opens the picker
        let (lr, lresp) = ui.allocate_exact_size(egui::vec2(label_w, CTRL_H), Sense::click());
        let picker = ui.id().with("period-picker");
        let open = ui.data(|d| d.get_temp::<bool>(picker)).unwrap_or(false);
        let hot = lresp.hovered() || open;
        ui.painter().rect(lr, RADIUS, c.field, Stroke::new(1.0, if hot { c.text2 } else { c.border }), egui::StrokeKind::Inside);
        if lresp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if lresp.has_focus() {
            focus_ring(ui, lr);
        }
        // the calendar mark at the right, as on the date fields of the dialogs
        let glyph_w = if label_w >= LABEL_MIN_W { 24.0 } else { 0.0 };
        if glyph_w > 0.0 {
            draw_glyph(ui.painter(), egui::pos2(lr.right() - PAD_X - 8.0, lr.center().y), Glyph::Calendar, if hot { c.text } else { c.text2 });
        }
        // Mono numbers, Sans words; the Sans part is dropped (tooltip keeps it) when it can't fit
        let gi = ui.painter().layout_no_wrap(iso.clone(), mono_font(14.0), c.text);
        let ge = ui.painter().layout_no_wrap(extra.clone(), sans(13.0), c.text2);
        let x0 = lr.left() + PAD_X;
        let room = lr.width() - PAD_X * 2.0 - glyph_w;
        if gi.size().x <= room {
            let wi = gi.size().x;
            ui.painter().galley(egui::pos2(x0, lr.center().y - gi.size().y / 2.0), gi, c.text);
            if wi + 10.0 + ge.size().x <= room {
                ui.painter().galley(egui::pos2(x0 + wi + 10.0, lr.center().y - ge.size().y / 2.0), ge, c.text2);
            }
        } else {
            // very narrow: a shorter form, still complete in the tooltip
            let short = match period.kind {
                PeriodKind::Day => period.start.format("%m-%d").to_string(),
                _ => format!("{}–{}", period.start.format("%m-%d"), (period.end - Duration::days(1)).format("%m-%d")),
            };
            let g = ui.painter().layout_no_wrap(short, mono_font(14.0), c.text);
            ui.painter().galley(egui::pos2(x0, lr.center().y - g.size().y / 2.0), g, c.text);
        }
        let full2 = format!("{} · {}", full, tr("Choose a date"));
        lresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, full2.clone()));
        // review pictures open the picker without a click
        let preview = ui.ctx().data_mut(|d| d.remove_temp::<bool>(egui::Id::new("preview-open-period-picker"))).unwrap_or(false);
        let clicked = lresp.clicked() || preview;
        lresp.on_hover_text(format!("{}\n{}", full, tr("Click to type a date or pick it on the calendar")));
        if let Some(day) = period_picker(ui, picker, lr, clicked, period, monday) {
            *new_p = Period::containing(period.kind, day, monday);
        }
        ui.add_space(GAP);
        let next = icon_button(ui, Glyph::Next, "Next period");
        if prev.clicked() {
            *new_p = period.shift(-1, monday);
        }
        if next.clicked() {
            *new_p = period.shift(1, monday);
        }
        bar.prev = prev.rect;
        bar.label = lr;
        bar.next = next.rect;
    };
    let today_btn = |ui: &mut Ui, bar: &mut PeriodBar, new_p: &mut Period| {
        let active = !period.contains(today());
        let t = button_w(ui, Kind::Secondary, "Today", TODAY_W, active).on_hover_text(tr("Go to the period containing today"));
        if active && t.clicked() {
            *new_p = Period::containing(period.kind, today(), monday);
        }
        bar.today = t.rect;
    };
    let tabs = |ui: &mut Ui, bar: &mut PeriodBar, new_p: &mut Period| {
        let (pick, tr_rect) = segmented_widths(ui, &KINDS, cur, &tab_widths, &kinds_enabled);
        if let Some(i) = pick {
            let anchor = if period.contains(today()) { today() } else { period.start };
            *new_p = Period::containing(kinds[i], anchor, monday);
        }
        bar.tabs = tr_rect;
    };

    let mut status = Some(status);
    if one_row {
        let fits = status_w > 0.0 && avail >= full_bar + TABS_GAP + status_w;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            nav(ui, &mut bar, &mut new_p);
            ui.add_space(GAP);
            today_btn(ui, &mut bar, &mut new_p);
            ui.add_space(TABS_GAP);
            tabs(ui, &mut bar, &mut new_p);
            if fits {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if let Some(s) = status.take() {
                        s(ui);
                    }
                });
            }
        });
        bar.layout = if fits || status_w == 0.0 { "one-row" } else { "one-row+status-row" };
    } else {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            nav(ui, &mut bar, &mut new_p);
            if today_beside {
                ui.add_space(GAP);
                today_btn(ui, &mut bar, &mut new_p);
            }
        });
        ui.add_space(GAP);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, GAP);
            tabs(ui, &mut bar, &mut new_p);
            if !today_beside {
                if avail >= tabs_total(&tab_widths) + TABS_GAP + TODAY_W {
                    ui.add_space(TABS_GAP);
                } else {
                    ui.end_row();
                }
                today_btn(ui, &mut bar, &mut new_p);
            }
        });
        bar.layout = if avail >= NAV_W { "two-rows" } else { "narrow" };
    }
    if let Some(s) = status.take() {
        if status_w > 0.0 {
            ui.horizontal(|ui| {
                ui.set_min_height(CTRL_H);
                s(ui);
            });
        }
    }
    *period = new_p;
    bar
}

pub fn record(app: &mut super::App, page: &str, bar: &PeriodBar) {
    for (n, r) in [("prev", bar.prev), ("label", bar.label), ("next", bar.next), ("tabs", bar.tabs), ("today", bar.today)] {
        app.geometry.rows.push((format!("{} period · {} [{}]", page, n, bar.layout), r));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_stay_inside_their_container_without_overlapping() {
        for width in [232.0, 264.0, 344.0, 500.0, 740.0, 1140.0] {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let mut reference: Option<Vec<Rect>> = None;
            for kind in [PeriodKind::Day, PeriodKind::Week, PeriodKind::Month] {
                let mut period = Period::containing(kind, chrono::NaiveDate::from_ymd_opt(2028, 2, 29).unwrap(), true);
                let mut controls = vec![];
                let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 800.0))), ..Default::default() }, |ctx| {
                    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
                        let bar = period_toolbar(ui, &mut period, true, [true; 3], 0.0, |_| {});
                        controls = vec![bar.prev, bar.label, bar.next, bar.tabs, bar.today];
                    });
                });
                for r in &controls {
                    assert!(r.left() >= -0.5 && r.right() <= width + 0.5, "{width}: {r:?}");
                }
                for (i, a) in controls.iter().enumerate() {
                    for b in &controls[i + 1..] { assert!(!a.shrink(0.5).intersects(*b), "{width}: {a:?} overlaps {b:?}"); }
                }
                if let Some(ref previous) = reference {
                    for (a, b) in previous.iter().zip(&controls) { assert!((a.min - b.min).length() <= 1.0 && (a.size() - b.size()).length() <= 1.0); }
                } else { reference = Some(controls); }
            }
        }
    }
}


/// The period field's picker, under the field: a date to type (23/11/2026, 23/11, 2026-11-23),
/// already selected so typing replaces it, and the month with the current period marked (a day,
/// its week or its month). Enter or a day jumps there in the same Day/Week/Month; Today and
/// Esc as everywhere. Returns the day chosen.
fn period_picker(ui: &mut Ui, id: egui::Id, field: Rect, toggle: bool, period: &Period, monday: bool) -> Option<chrono::NaiveDate> {
    use chrono::NaiveDate;
    let c = p(ui);
    let open_id = id;
    let text_id = id.with("text");
    let month_id = id.with("month");
    let fresh_id = id.with("fresh");
    let err_id = id.with("error");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let today = today();
    if toggle {
        open = !open;
        if open {
            // the field's own date, ready to be typed over; the month it falls in
            let shown = if period.contains(today) && period.kind != PeriodKind::Day { today } else { period.start };
            ui.data_mut(|d| {
                d.insert_temp(text_id, short_date(shown));
                d.insert_temp(month_id, shown.with_day(1).unwrap_or(shown));
                d.insert_temp(fresh_id, true);
                d.insert_temp(err_id, false);
            });
        }
    }
    if !open {
        ui.data_mut(|d| d.insert_temp(open_id, false));
        return None;
    }
    let mut text: String = ui.data(|d| d.get_temp(text_id)).unwrap_or_default();
    let mut first: NaiveDate = ui.data(|d| d.get_temp(month_id)).unwrap_or(period.start);
    let fresh = ui.data_mut(|d| d.remove_temp::<bool>(fresh_id)).unwrap_or(false);
    let mut error = ui.data(|d| d.get_temp::<bool>(err_id)).unwrap_or(false);
    let last = period.end - Duration::days(1);
    let cell = egui::vec2(36.0, 32.0);
    let width = cell.x * 7.0;
    let popup_h = 10.0 * 2.0 + CTRL_H + 30.0 + CTRL_H + 24.0 + (cell.y + 4.0) * 6.0 + 30.0;
    let below = field.bottom() + 4.0 + popup_h <= ui.ctx().screen_rect().bottom();
    let (anchor, pivot) = if below { (field.left_bottom() + egui::vec2(0.0, 4.0), egui::Align2::LEFT_TOP) } else { (field.left_top() - egui::vec2(0.0, 4.0), egui::Align2::LEFT_BOTTOM) };
    let mut chosen: Option<NaiveDate> = None;
    let mut close = false;
    let area = egui::Area::new(id.with("area")).order(egui::Order::Foreground).pivot(pivot).fixed_pos(anchor).show(ui.ctx(), |ui| {
        egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.border)).corner_radius(8.0).inner_margin(egui::Margin::same(10)).shadow(egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: egui::Color32::from_black_alpha(80) }).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
            ui.set_width(width);
            label(ui, "Go to date");
            let r = input(ui, &mut text, Field::new("Go to date", width).hint(&short_date(today)).mono().invalid(error));
            if fresh {
                // focused with its text selected: typing a date replaces it
                r.request_focus();
                if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), r.id) {
                    let n = text.chars().count();
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                    st.store(ui.ctx(), r.id);
                }
            }
            if r.changed() {
                error = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                match parse_user_date(&text, today.year()) {
                    Some(d) => chosen = Some(d),
                    None => {
                        error = true;
                        r.request_focus();
                    }
                }
            }
            if error {
                ui.label(egui::RichText::new(tr("Not a date: type it as 23/11/2026, 23/11 or 2026-11-23.")).size(12.0).color(c.red));
            }
            ui.add_space(6.0);
            if let Some(d) = month_grid(ui, &mut first, Some((period.start, last)), today, monday, cell) {
                chosen = Some(d);
            }
            ui.add_space(2.0);
            if link_btn(ui, "Today").clicked() {
                chosen = Some(today);
            }
        });
    });
    let outside = ui.input(|i| i.pointer.any_click()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !area.response.rect.contains(pos) && !field.contains(pos));
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if chosen.is_some() {
        close = true;
    }
    ui.data_mut(|d| {
        d.insert_temp(open_id, !close);
        d.insert_temp(text_id, text);
        d.insert_temp(month_id, first);
        d.insert_temp(err_id, error);
    });
    chosen
}
