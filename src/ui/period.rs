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
        let (lr, lresp) = ui.allocate_exact_size(egui::vec2(label_w, CTRL_H), Sense::hover());
        ui.painter().rect(lr, RADIUS, c.field, Stroke::new(1.0, c.border), egui::StrokeKind::Inside);
        // Mono numbers, Sans words; the Sans part is dropped (tooltip keeps it) when it can't fit
        let gi = ui.painter().layout_no_wrap(iso.clone(), mono_font(14.0), c.text);
        let ge = ui.painter().layout_no_wrap(extra.clone(), sans(13.0), c.text2);
        let x0 = lr.left() + PAD_X;
        let room = lr.width() - PAD_X * 2.0;
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
        let full2 = full.clone();
        lresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, full2.clone()));
        lresp.on_hover_text(full.clone());
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
