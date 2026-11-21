//! DevTrack design system: tokens, fonts, theme and the shared control family.
//!
//! Every input, selector trigger and button is drawn here at an exact outer height of
//! [`CTRL_H`] (36 logical units, border inside), radius 6, horizontal padding 12, so controls on
//! one row share their top and bottom edges. Focus is a 2-unit ring drawn *outside* the frame,
//! so it never changes layout. Numbers use Geist Mono; prose uses Geist Sans. See DESIGN.md.

use devtrack_core::model::*;
use super::i18n::{tr, trf_m};
use eframe::egui::{self, text::LayoutJob, Color32, FontFamily, FontId, Rect, RichText, Sense, Stroke, Ui};

// ---------- geometry contract ----------

pub const CTRL_H: f32 = 36.0;
/// Narrowest text button: short labels ("Start", "Save") keep a comfortable target and every
/// button in a group reads as the same kind of object.
pub const BTN_MIN_W: f32 = 100.0;
pub const RADIUS: f32 = 6.0;
pub const PAD_X: f32 = 12.0;
pub const LABEL_GAP: f32 = 6.0;
pub const ROW_GAP: f32 = 8.0;
pub const GROUP_GAP: f32 = 16.0;
pub const MENU_ROW_H: f32 = 36.0;

// ---------- tokens ----------

#[derive(Clone, Copy)]
pub struct Pal {
    pub canvas: Color32,
    pub nav: Color32,
    pub raised: Color32,
    pub line: Color32,
    /// Interactive boundaries are stronger than decorative dividers.
    pub border: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub primary: Color32,
    pub on_primary: Color32,
    pub focus: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub field: Color32,
    pub disabled: Color32,
    pub green: Color32,
    pub amber: Color32,
    pub red: Color32,
}

pub fn pal(dark: bool) -> Pal {
    if dark {
        // No pure black: a near-black with a trace of warmth (the family of the other Brainup
        // apps), in steps that keep the layers apart. Content sits on the canvas; the sidebar
        // and panels rise one step each; fields sink a little below the panel they are in.
        Pal {
            canvas: Color32::from_rgb(0x13, 0x13, 0x12),
            nav: Color32::from_rgb(0x18, 0x18, 0x17),
            raised: Color32::from_rgb(0x1C, 0x1C, 0x1B),
            line: Color32::from_rgb(0x2E, 0x2E, 0x2B),
            // the control reads by its shape and fill, the border only outlines it; hover and
            // focus still raise it
            border: Color32::from_rgb(0x46, 0x46, 0x42),
            text: Color32::from_rgb(0xEE, 0xED, 0xE9),
            text2: Color32::from_rgb(0xA6, 0xA5, 0x9E),
            primary: Color32::from_rgb(0xEE, 0xED, 0xE9),
            on_primary: Color32::from_rgb(0x13, 0x13, 0x12),
            focus: Color32::from_rgb(0x52, 0xA8, 0xFF),
            hover: Color32::from_rgb(0x25, 0x25, 0x23),
            selected: Color32::from_rgb(0x2B, 0x2B, 0x29),
            field: Color32::from_rgb(0x16, 0x16, 0x15),
            disabled: Color32::from_rgb(0x20, 0x20, 0x1F),
            green: Color32::from_rgb(0x62, 0xC0, 0x73),
            amber: Color32::from_rgb(0xF5, 0xA5, 0x24),
            red: Color32::from_rgb(0xFF, 0x61, 0x66),
        }
    } else {
        Pal {
            canvas: Color32::WHITE,
            nav: Color32::from_rgb(0xFA, 0xFA, 0xFA),
            raised: Color32::WHITE,
            line: Color32::from_rgb(0xE5, 0xE5, 0xE5),
            border: Color32::from_rgb(0xC9, 0xC9, 0xC9), // was 0x8C
            text: Color32::from_rgb(0x17, 0x17, 0x17),
            text2: Color32::from_rgb(0x66, 0x66, 0x66),
            primary: Color32::from_rgb(0x17, 0x17, 0x17),
            on_primary: Color32::WHITE,
            focus: Color32::from_rgb(0x00, 0x5F, 0xCC),
            hover: Color32::from_rgb(0xF2, 0xF2, 0xF2),
            selected: Color32::from_rgb(0xEB, 0xEB, 0xEB),
            field: Color32::WHITE,
            disabled: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            green: Color32::from_rgb(0x1A, 0x7F, 0x37),
            amber: Color32::from_rgb(0xA3, 0x52, 0x00),
            red: Color32::from_rgb(0xC6, 0x2A, 0x2F),
        }
    }
}

pub fn p(ui: &Ui) -> Pal {
    pal(ui.visuals().dark_mode)
}

// ---------- fonts ----------

const SEMIBOLD: &str = "geist-semibold";
const MEDIUM: &str = "geist-medium";
const MONO_MEDIUM: &str = "geist-mono-medium";
const BRAND: &str = "fira-code-bold";
/// Size of every text against the sizes written in the code (1.0 = as written). The glyphs are
/// scaled where the fonts are installed, so one number changes the whole app; the logo keeps its
/// size.
pub const TEXT_SCALE: f32 = 0.95;

/// Ascent of Fira Code in em (935 of 1000 units).
const FIRA_ASCENT: f32 = 0.935;

/// Geist Sans (Regular, Medium, SemiBold), Geist Mono (Regular, Medium) and Fira Code Bold
/// (the logo's face), bundled under the SIL OFL in assets/fonts, with egui's own fonts kept as fallbacks for symbols.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let data = |b: &'static [u8]| std::sync::Arc::new(egui::FontData::from_static(b));
    fonts.font_data.insert("geist".into(), data(include_bytes!("../../assets/fonts/Geist-Regular.ttf")));
    fonts.font_data.insert("geist-medium".into(), data(include_bytes!("../../assets/fonts/Geist-Medium.ttf")));
    fonts.font_data.insert("geist-semibold".into(), data(include_bytes!("../../assets/fonts/Geist-SemiBold.ttf")));
    fonts.font_data.insert("geist-mono".into(), data(include_bytes!("../../assets/fonts/GeistMono-Regular.ttf")));
    fonts.font_data.insert("geist-mono-medium".into(), data(include_bytes!("../../assets/fonts/GeistMono-Medium.ttf")));
    // the face of the logo, used for the name beside the mark and nowhere else
    fonts.font_data.insert(BRAND.into(), data(include_bytes!("../../assets/fonts/FiraCode-Bold.ttf")));
    let fallback: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mono_fallback: Vec<String> = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let with = |first: &str, rest: &[String]| {
        let mut v = vec![first.to_string()];
        v.extend(rest.iter().cloned());
        v
    };
    fonts.families.insert(FontFamily::Proportional, with("geist", &fallback));
    fonts.families.insert(FontFamily::Monospace, with("geist-mono", &mono_fallback));
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), with("geist-semibold", &fallback));
    fonts.families.insert(FontFamily::Name(MEDIUM.into()), with("geist-medium", &fallback));
    fonts.families.insert(FontFamily::Name(MONO_MEDIUM.into()), with("geist-mono-medium", &mono_fallback));
    fonts.families.insert(FontFamily::Name(BRAND.into()), with(BRAND, &mono_fallback));
    for (name, font) in fonts.font_data.iter_mut() {
        if name != BRAND {
            let mut f = (**font).clone();
            f.tweak.scale *= TEXT_SCALE;
            *font = std::sync::Arc::new(f);
        }
    }
    ctx.set_fonts(fonts);
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}
pub fn brand_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BRAND.into()))
}
pub fn sans(size: f32) -> FontId {
    FontId::proportional(size)
}
pub fn mono_font(size: f32) -> FontId {
    FontId::monospace(size)
}
pub fn mono_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MONO_MEDIUM.into()))
}

pub fn apply_theme(ctx: &egui::Context, dark: bool) {
    let c = pal(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = c.canvas;
    v.window_fill = c.raised;
    v.window_stroke = Stroke::new(1.0, c.line);
    v.window_shadow = egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 40 }) };
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(if dark { 100 } else { 30 }) };
    v.extreme_bg_color = c.field;
    v.faint_bg_color = c.nav;
    v.override_text_color = Some(c.text);
    v.selection.bg_fill = c.focus.linear_multiply(0.25);
    v.selection.stroke = Stroke::new(2.0, c.focus);
    v.hyperlink_color = c.text;
    v.text_cursor.stroke = Stroke::new(1.5, c.text);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = RADIUS.into();
        w.expansion = 0.0;
    }
    v.widgets.inactive.weak_bg_fill = c.raised;
    v.widgets.inactive.bg_fill = c.selected;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, c.border); // checkboxes and radio marks stay visible unchecked
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, c.text);
    v.widgets.hovered.weak_bg_fill = c.hover;
    v.widgets.hovered.bg_fill = c.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.text2);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, c.text);
    v.widgets.active.weak_bg_fill = c.hover;
    v.widgets.active.bg_fill = c.hover;
    v.widgets.active.bg_stroke = Stroke::new(2.0, c.focus);
    v.widgets.active.fg_stroke = Stroke::new(1.5, c.text);
    v.widgets.open.weak_bg_fill = c.hover;
    v.widgets.open.bg_stroke = Stroke::new(1.0, c.text2);
    v.window_corner_radius = 10.0.into();
    v.menu_corner_radius = 8.0.into();
    v.striped = false;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(ROW_GAP, ROW_GAP);
        s.spacing.button_padding = egui::vec2(PAD_X, 8.0);
        s.spacing.interact_size = egui::vec2(36.0, 20.0); // text rows stay tight; shared controls are 36 high on their own
        s.spacing.menu_margin = egui::Margin::same(6);
        s.text_styles.insert(egui::TextStyle::Body, sans(14.0));
        s.text_styles.insert(egui::TextStyle::Button, sans(14.0));
        s.text_styles.insert(egui::TextStyle::Small, sans(12.0));
        s.text_styles.insert(egui::TextStyle::Monospace, mono_font(14.0));
        s.text_styles.insert(egui::TextStyle::Heading, semibold(28.0));
        // page fades are handled by the shell; widget transitions stay short
        s.animation_time = 0.08;
    });
}

// ---------- text ----------

// Interface text passes through `tr` here (exact-match messages only; anything else, such as
// names and codes, shows unchanged). Digits in prose are drawn in Mono.


pub fn section(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(tr(text)).font(medium(17.0)));
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) -> egui::Response {
    let c = p(ui).text2;
    let t = tr(&text.into());
    ui.label(mixed_job(&t, 14.0, c, sans(14.0), mono_font(14.0)))
}

pub fn meta(ui: &mut Ui, text: impl Into<String>) -> egui::Response {
    let c = p(ui).text2;
    let t = tr(&text.into());
    ui.label(mixed_job(&t, 12.0, c, sans(12.0), mono_font(12.0)))
}

/// Numerical text: Geist Mono.
pub fn mono(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).font(mono_font(size))
}

/// Numerical text: Geist Mono Medium.
pub fn mono_m(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).font(mono_medium(size))
}

/// Mixed prose and numbers: runs of digits (with : , . % + - and unit-free separators) are set
/// in Mono, the rest in Sans. Used for copy such as "2 sessions need attention".
pub fn mixed_job(text: &str, size: f32, color: Color32, sans_font: FontId, mono_f: FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut buf = String::new();
    let mut in_num = false;
    let chars: Vec<char> = text.chars().collect();
    let flush = |job: &mut LayoutJob, buf: &mut String, num: bool| {
        if buf.is_empty() {
            return;
        }
        let font = if num { mono_f.clone() } else { sans_font.clone() };
        job.append(buf, 0.0, egui::TextFormat { font_id: font, color, ..Default::default() });
        buf.clear();
    };
    for (i, ch) in chars.iter().enumerate() {
        let next_digit = chars.get(i + 1).map(|c| c.is_ascii_digit()).unwrap_or(false);
        let numeric = ch.is_ascii_digit() || (in_num && matches!(ch, ':' | ',' | '.' | '%') && next_digit) || (matches!(ch, '+' | '#') && next_digit) || (in_num && *ch == '%');
        if numeric != in_num {
            flush(&mut job, &mut buf, in_num);
            in_num = numeric;
        }
        buf.push(*ch);
    }
    flush(&mut job, &mut buf, in_num);
    let _ = size;
    job
}

/// A clock interval, "09:00 → 09:14": the times in Geist Mono, the arrow in Geist.
pub fn range_job(from: &str, to: &str, size: f32, color: Color32) -> LayoutJob {
    mixed_job(&format!("{} → {}", from, to), size, color, sans(size), mono_font(size))
}

pub fn mixed(ui: &mut Ui, text: &str, size: f32, color: Option<Color32>) -> egui::Response {
    let c = color.unwrap_or(p(ui).text);
    let text = &tr(text);
    let job = mixed_job(text, size, c, sans(size), mono_font(size));
    ui.label(job)
}

pub fn mixed_medium(ui: &mut Ui, text: &str, size: f32) -> egui::Response {
    let c = p(ui).text;
    let text = &tr(text);
    let job = mixed_job(text, size, c, medium(size), mono_medium(size));
    ui.label(job)
}

/// Field label: Sans 12, line height 16.
pub fn label(ui: &mut Ui, text: &str) -> egui::Response {
    let c = p(ui).text2;
    ui.add(egui::Label::new(RichText::new(tr(text)).size(12.0).color(c)).wrap_mode(egui::TextWrapMode::Truncate))
}

// ---------- controls ----------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

pub fn focus_ring(ui: &Ui, rect: Rect) {
    let c = p(ui);
    ui.painter().rect_stroke(rect.expand(2.0), RADIUS + 2.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
}

/// Text width of a string in a font, for sizing buttons.
pub fn text_w(ui: &Ui, text: &str, font: &FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE).size().x
}

/// A button with an exact outer height of 36. `width` 0 = fit the text.
/// A colour between `from` and `to`, eased over the style's animation time as the pointer
/// enters or leaves: hover changes are a short fade, not a switch. With motion reduced the
/// time is zero and it switches at once.
pub fn hover_mix(ui: &Ui, id: egui::Id, hovered: bool, from: Color32, to: Color32) -> Color32 {
    let t = ui.ctx().animate_bool_with_time(id.with("hover-mix"), hovered, ui.style().animation_time * 1.5);
    let (a, b) = (from.to_array(), to.to_array());
    let m = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(m(0), m(1), m(2), m(3))
}

pub fn button_w(ui: &mut Ui, kind: Kind, text: &str, width: f32, enabled: bool) -> egui::Response {
    let enabled = enabled && ui.is_enabled();
    let c = p(ui);
    let text = &tr(text);
    let font = if kind == Kind::Primary { medium(14.0) } else { sans(14.0) };
    let w = (if width > 0.0 { width } else { text_w(ui, text, &font) + PAD_X * 2.0 }).max(BTN_MIN_W);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, CTRL_H), sense);
    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let pressed = enabled && resp.is_pointer_button_down_on();
        // rest and hover colours; the one painted fades between them
        let over = hovered || pressed;
        let mix = |from: Color32, to: Color32| hover_mix(ui, resp.id, over, from, to);
        let (fill, stroke, fg) = match kind {
            Kind::Primary => {
                let f = if !enabled { c.disabled } else if pressed { c.primary.linear_multiply(0.8) } else { mix(c.primary, c.primary.linear_multiply(0.9)) };
                (f, if enabled { f } else { c.border }, if enabled { c.on_primary } else { c.text2 })
            }
            Kind::Secondary => (if !enabled { c.disabled } else { mix(c.raised, c.hover) }, mix(c.border, c.text2), if enabled { c.text } else { c.text2 }),
            Kind::Ghost => (mix(Color32::TRANSPARENT, c.hover), Color32::TRANSPARENT, if enabled { c.text2 } else { c.text2.linear_multiply(0.6) }),
            // outline in red at rest: a destructive action reads as one before it is hovered
            Kind::Danger => (mix(c.raised, c.red.linear_multiply(0.12)), if enabled { c.red } else { c.border }, if enabled { c.red } else { c.text2 }),
        };
        ui.painter().rect(rect, RADIUS, fill, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
        let g = ui.painter().layout_no_wrap(text.to_string(), font, fg);
        let pos = egui::pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0);
        ui.painter().galley(pos, g, fg);
        if resp.has_focus() {
            focus_ring(ui, rect);
        }
    }
    let label_txt = text.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label_txt.clone()));
    resp
}

pub fn button(ui: &mut Ui, kind: Kind, text: &str) -> egui::Response {
    button_w(ui, kind, text, 0.0, true)
}
pub fn primary(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, Kind::Primary, text)
}
pub fn secondary(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, Kind::Secondary, text)
}
pub fn ghost(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, Kind::Ghost, text)
}
pub fn danger(ui: &mut Ui, text: &str) -> egui::Response {
    button(ui, Kind::Danger, text)
}

/// Inline text action next to the data it acts on: main-text colour with a quiet underline at
/// rest (so it reads as an action, not a caption), a hover fill and the focus ring.
pub fn link_btn(ui: &mut Ui, text: &str) -> egui::Response {
    let c = p(ui);
    let text = &tr(text);
    let font = medium(13.0);
    let w = text_w(ui, text, &font);
    // 20 high, the same as a text line, so it sits on the text's centre in any row; the hover
    // fill reaches 2 px past it.
    // The link is as wide as its text, so it starts on the same edge as the text around it; the
    // hover fill reaches 6 px past it on each side.
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 20.0), Sense::click());
    let hovered = resp.hovered() || resp.is_pointer_button_down_on();
    if hovered {
        ui.painter().rect_filled(rect.expand2(egui::vec2(6.0, 2.0)), 4.0, c.hover);
    }
    let g = ui.painter().layout_no_wrap(text.to_string(), font, c.text);
    let x = rect.left();
    let base = rect.center().y + g.size().y / 2.0 - 1.0;
    ui.painter().galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, c.text);
    ui.painter().line_segment([egui::pos2(x, base), egui::pos2(rect.right(), base)], Stroke::new(1.0, if hovered { c.text } else { c.text2.linear_multiply(0.6) }));
    if resp.has_focus() {
        focus_ring(ui, rect.expand2(egui::vec2(6.0, 2.0)));
    }
    let t = text.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, t.clone()));
    resp
}

#[derive(Clone, Copy)]
pub enum Glyph {
    Compact,
    Expand,
    More,
    Prev,
    Next,
    Close,
    Play,
    Copy,
    Down,
    Calendar,
    Convert,
    Sidebar,
}

pub fn draw_glyph(painter: &egui::Painter, c: egui::Pos2, g: Glyph, col: Color32) {
    let s = Stroke::new(1.5, col);
    match g {
        Glyph::Compact => {
            // a large window with a small one inside: "shrink to compact"
            painter.rect_stroke(egui::Rect::from_center_size(c, egui::vec2(16.0, 13.0)), 2.0, s, egui::StrokeKind::Middle);
            painter.rect_filled(egui::Rect::from_min_size(c + egui::vec2(1.0, -4.0), egui::vec2(5.0, 8.0)), 1.0, col);
        }
        Glyph::Sidebar => {
            // a window with its left column marked off
            painter.rect_stroke(egui::Rect::from_center_size(c, egui::vec2(16.0, 13.0)), 2.5, s, egui::StrokeKind::Middle);
            painter.line_segment([c + egui::vec2(-2.5, -6.5), c + egui::vec2(-2.5, 6.5)], s);
        }
        Glyph::Expand => {
            painter.line_segment([c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)], s);
            painter.line_segment([c + egui::vec2(0.0, -5.0), c + egui::vec2(5.0, -5.0)], s);
            painter.line_segment([c + egui::vec2(5.0, -5.0), c + egui::vec2(5.0, 0.0)], s);
        }
        Glyph::Calendar => {
            painter.rect_stroke(egui::Rect::from_center_size(c + egui::vec2(0.0, 1.0), egui::vec2(14.0, 12.0)), 2.0, s, egui::StrokeKind::Middle);
            painter.line_segment([c + egui::vec2(-7.0, -2.0), c + egui::vec2(7.0, -2.0)], s);
            painter.line_segment([c + egui::vec2(-3.5, -7.0), c + egui::vec2(-3.5, -4.0)], s);
            painter.line_segment([c + egui::vec2(3.5, -7.0), c + egui::vec2(3.5, -4.0)], s);
        }
        Glyph::Convert => {
            // one value turned into another: two arrows running opposite ways
            for (y, d) in [(-3.5_f32, 1.0_f32), (3.5, -1.0)] {
                painter.line_segment([c + egui::vec2(-6.0, y), c + egui::vec2(6.0, y)], s);
                painter.line_segment([c + egui::vec2(6.0 * d, y), c + egui::vec2(2.5 * d, y - 3.0)], s);
                painter.line_segment([c + egui::vec2(6.0 * d, y), c + egui::vec2(2.5 * d, y + 3.0)], s);
            }
        }
        Glyph::Down => {
            painter.line_segment([c + egui::vec2(-5.0, -2.5), c + egui::vec2(0.0, 2.5)], s);
            painter.line_segment([c + egui::vec2(0.0, 2.5), c + egui::vec2(5.0, -2.5)], s);
        }
        Glyph::Copy => {
            painter.rect_stroke(egui::Rect::from_min_size(c + egui::vec2(-2.0, -2.0), egui::vec2(9.0, 10.0)), 2.0, s, egui::StrokeKind::Middle);
            painter.line_segment([c + egui::vec2(-6.0, 4.0), c + egui::vec2(-6.0, -6.0)], s);
            painter.line_segment([c + egui::vec2(-6.0, -6.0), c + egui::vec2(3.0, -6.0)], s);
        }
        Glyph::Play => {
            let pts = vec![c + egui::vec2(-3.5, -5.5), c + egui::vec2(5.5, 0.0), c + egui::vec2(-3.5, 5.5)];
            painter.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, s));
        }
        Glyph::More => {
            for dx in [-5.0, 0.0, 5.0] {
                painter.circle_filled(c + egui::vec2(dx, 0.0), 1.4, col);
            }
        }
        Glyph::Prev | Glyph::Next => {
            let d = if matches!(g, Glyph::Prev) { -1.0 } else { 1.0 };
            painter.line_segment([c + egui::vec2(-2.5 * d, -5.0), c + egui::vec2(2.5 * d, 0.0)], s);
            painter.line_segment([c + egui::vec2(2.5 * d, 0.0), c + egui::vec2(-2.5 * d, 5.0)], s);
        }
        Glyph::Close => {
            painter.line_segment([c + egui::vec2(-4.5, -4.5), c + egui::vec2(4.5, 4.5)], s);
            painter.line_segment([c + egui::vec2(4.5, -4.5), c + egui::vec2(-4.5, 4.5)], s);
        }
    }
}

/// 36×36 icon button with a tooltip and an accessible name.
pub fn icon_button(ui: &mut Ui, glyph: Glyph, name: &str) -> egui::Response {
    let c = p(ui);
    let name = &tr(name);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(CTRL_H, CTRL_H), Sense::click());
    // disabled (inside add_enabled_ui(false)): no fill, faint border and glyph
    let on = ui.is_enabled();
    let over = on && (resp.hovered() || resp.is_pointer_button_down_on());
    let fill = if on { hover_mix(ui, resp.id, over, c.raised, c.hover) } else { Color32::TRANSPARENT };
    let edge = if on { hover_mix(ui, resp.id.with("edge"), over, c.border, c.text2) } else { c.line };
    ui.painter().rect(rect, RADIUS, fill, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    // add_enabled_ui(false) already fades what is painted: no second fade, or the glyph vanishes
    // on a light surface
    draw_glyph(ui.painter(), rect.center(), glyph, if on { c.text } else { c.text2 });
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    let n = name.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, n.clone()));
    resp.on_hover_text(name)
}

/// A quiet notice box in a status colour: the fill is a light tint (≈ 9 % alpha) so text in the
/// same colour keeps its contrast; the border carries the colour. Text inside should start with
/// a word or "!" so colour is never the only signal.
pub fn notice<R>(ui: &mut Ui, color: Color32, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let [r, g, b, _] = color.to_array();
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(r, g, b, 22))
        .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(r, g, b, 110)))
        .corner_radius(RADIUS)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
}

/// Date field: Mono text in `YYYY-MM-DD` (typing still works) with a calendar button at the
/// right edge. The button, or a click in an empty field, opens a month view below the field:
/// ‹ month year ›, weekday initials in the user's week order, today outlined, the chosen day
/// filled; Today jumps to the current date. Choosing a day writes the text and closes it; Escape
/// or a click outside closes it too. The returned response reports the change like typing does.
pub fn date_input(ui: &mut Ui, value: &mut String, name: &str, width: f32, invalid: bool, monday: bool) -> egui::Response {
    use chrono::{Datelike, Duration, NaiveDate};
    let c = p(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, CTRL_H), Sense::hover());
    ui.data_mut(|d| d.insert_temp(egui::Id::new("last-input-rect"), rect));
    let bg = ui.painter().add(egui::Shape::Noop);
    let btn_w = 30.0;
    let inner = Rect::from_min_max(rect.min + egui::vec2(PAD_X, 1.0), rect.max - egui::vec2(PAD_X + btn_w, 1.0));
    let te = egui::TextEdit::singleline(value).frame(false).font(mono_font(14.0)).margin(egui::Margin::ZERO).desired_width(inner.width()).vertical_align(egui::Align::Center);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let mut resp = child.add_sized(inner.size(), te);
    let id = ui.id().with(("date-input", name));
    let open_id = id.with("open");
    let month_id = id.with("month");
    let btn = Rect::from_min_max(egui::pos2(rect.right() - btn_w - 4.0, rect.top() + 3.0), egui::pos2(rect.right() - 4.0, rect.bottom() - 3.0));
    let b = ui.interact(btn, id.with("btn"), Sense::click());
    if b.hovered() {
        ui.painter().rect_filled(btn, 4.0, c.hover);
    }
    draw_glyph(ui.painter(), btn.center(), Glyph::Calendar, if b.hovered() { c.text } else { c.text2 });
    let bn = tr("Choose a date");
    b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, bn.clone()));
    let parsed = NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok();
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    if b.on_hover_text(tr("Choose a date")).clicked() || (resp.clicked() && value.trim().is_empty()) {
        open = !open;
        let today = chrono::Local::now().date_naive();
        let first = parsed.unwrap_or(today).with_day(1).unwrap_or(today);
        ui.data_mut(|d| d.insert_temp(month_id, first));
    }
    if open {
        let today = chrono::Local::now().date_naive();
        let mut first: NaiveDate = ui.data(|d| d.get_temp(month_id)).unwrap_or_else(|| today.with_day(1).unwrap_or(today));
        let cell = egui::vec2(36.0, 32.0);
        // below the field, or above it when the month would leave the window
        let popup_h = 10.0 * 2.0 + CTRL_H + 4.0 + 24.0 + (cell.y + 4.0) * 6.0 + 30.0;
        let below = rect.bottom() + 4.0 + popup_h <= ui.ctx().screen_rect().bottom();
        let (anchor, pivot) = if below { (rect.left_bottom() + egui::vec2(0.0, 4.0), egui::Align2::LEFT_TOP) } else { (rect.left_top() - egui::vec2(0.0, 4.0), egui::Align2::LEFT_BOTTOM) };
        let area = egui::Area::new(id.with("area")).order(egui::Order::Tooltip).pivot(pivot).fixed_pos(anchor).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.border)).corner_radius(8.0).inner_margin(egui::Margin::same(10)).shadow(egui::epaint::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(80) }).show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                ui.set_width(cell.x * 7.0);
                ui.horizontal(|ui| {
                    if icon_ghost(ui, Glyph::Prev, "Previous month").clicked() {
                        first = (first - Duration::days(1)).with_day(1).unwrap_or(first);
                    }
                    let title = format!("{} {}", super::i18n::month(first.month()), first.year());
                    let (tr_rect, _) = ui.allocate_exact_size(egui::vec2(cell.x * 5.0, CTRL_H), Sense::hover());
                    ui.painter().text(tr_rect.center(), egui::Align2::CENTER_CENTER, title, medium(14.0), c.text);
                    if icon_ghost(ui, Glyph::Next, "Next month").clicked() {
                        first = (first + Duration::days(32)).with_day(1).unwrap_or(first);
                    }
                });
                let names: [&str; 7] = match super::i18n::lang() {
                    super::i18n::Lang::It => ["Lu", "Ma", "Me", "Gi", "Ve", "Sa", "Do"],
                    super::i18n::Lang::En => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
                };
                let order: Vec<usize> = if monday { (0..7).collect() } else { std::iter::once(6).chain(0..6).collect() };
                ui.horizontal(|ui| {
                    for i in &order {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(cell.x, 20.0), Sense::hover());
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, names[*i], sans(12.0), c.text2);
                    }
                });
                let lead = if monday { first.weekday().num_days_from_monday() } else { first.weekday().num_days_from_sunday() } as i64;
                let start = first - Duration::days(lead);
                let mut picked: Option<NaiveDate> = None;
                for w in 0..6 {
                    ui.horizontal(|ui| {
                        for d in 0..7 {
                            let day = start + Duration::days(w * 7 + d);
                            let (r, rr) = ui.allocate_exact_size(cell, Sense::click());
                            let inside = day.month() == first.month();
                            let sel = Some(day) == parsed;
                            let r2 = r.shrink2(egui::vec2(2.0, 1.0));
                            if sel {
                                ui.painter().rect_filled(r2, 4.0, c.primary);
                            } else if rr.hovered() {
                                ui.painter().rect_filled(r2, 4.0, c.hover);
                            }
                            if day == today && !sel {
                                ui.painter().rect_stroke(r2, 4.0, Stroke::new(1.0, c.text2), egui::StrokeKind::Inside);
                            }
                            let col = if sel { c.on_primary } else if inside { c.text } else { c.text2.linear_multiply(0.55) };
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, day.day().to_string(), mono_font(13.0), col);
                            let label = day.format("%Y-%m-%d").to_string();
                            rr.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, sel, label.clone()));
                            if rr.clicked() {
                                picked = Some(day);
                            }
                        }
                    });
                }
                ui.add_space(2.0);
                if link_btn(ui, "Today").clicked() {
                    picked = Some(today);
                }
                picked
            }).inner
        });
        if let Some(day) = area.inner {
            *value = day.format("%Y-%m-%d").to_string();
            resp.mark_changed();
            open = false;
        }
        let clicked_outside = ui.input(|i| i.pointer.any_click()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !area.response.rect.contains(pos) && !btn.contains(pos) && !rect.contains(pos));
        if clicked_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        ui.data_mut(|d| d.insert_temp(month_id, first));
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
    let border = if invalid { c.red } else if resp.hovered() || open { c.text2 } else { c.border };
    ui.painter().set(bg, egui::epaint::RectShape::new(rect, RADIUS, c.field, Stroke::new(1.0, border), egui::StrokeKind::Inside));
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    let n = tr(name);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, n.clone()));
    resp
}

/// 36×36 icon button without a frame until hovered or focused: for a repeated row action that
/// should stay available without adding a border to every row.
/// The same button without the name tooltip: for a control whose hover shows its own content.
pub fn icon_ghost_quiet(ui: &mut Ui, glyph: Glyph, name: &str) -> egui::Response {
    let c = p(ui);
    let name = &tr(name);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(CTRL_H, CTRL_H), Sense::click());
    let active = resp.hovered() || resp.is_pointer_button_down_on();
    if active {
        ui.painter().rect(rect, RADIUS, c.hover, Stroke::new(1.0, c.border), egui::StrokeKind::Inside);
    }
    draw_glyph(ui.painter(), rect.center(), glyph, if active { c.text } else { c.text2 });
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    let n = name.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, n.clone()));
    resp
}

pub fn icon_ghost(ui: &mut Ui, glyph: Glyph, name: &str) -> egui::Response {
    let r = icon_ghost_quiet(ui, glyph, name);
    r.on_hover_text(tr(name))
}

/// Options for [`input`].
#[derive(Clone, Copy)]
pub struct Field<'a> {
    pub width: f32,
    pub hint: &'a str,
    pub mono: bool,
    pub invalid: bool,
    pub enabled: bool,
    pub password: bool,
    /// Label to announce for screen readers.
    pub name: &'a str,
}

impl<'a> Field<'a> {
    pub fn new(name: &'a str, width: f32) -> Self {
        Field { width, hint: "", mono: false, invalid: false, enabled: true, password: false, name }
    }
    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = h;
        self
    }
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
    pub fn invalid(mut self, v: bool) -> Self {
        self.invalid = v;
        self
    }
    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }
    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }
}

/// Single-line text field with an exact outer height of 36: frame painted here, a frameless
/// text edit placed inside with 12 units of horizontal padding and vertical centring.
pub fn input(ui: &mut Ui, value: &mut String, f: Field) -> egui::Response {
    let c = p(ui);
    let w = if f.width <= 0.0 { ui.available_width() } else { f.width };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, CTRL_H), Sense::hover());
    ui.data_mut(|d| d.insert_temp(egui::Id::new("last-input-rect"), rect));
    // reserve the frame's place in the shape list so it is drawn behind the text
    let bg = ui.painter().add(egui::Shape::Noop);
    let inner = Rect::from_min_max(rect.min + egui::vec2(PAD_X, 1.0), rect.max - egui::vec2(PAD_X, 1.0));
    let font = if f.mono { mono_font(14.0) } else { sans(14.0) };
    let mut te = egui::TextEdit::singleline(value)
        .frame(false)
        .font(font)
        .margin(egui::Margin::ZERO)
        .desired_width(inner.width())
        .vertical_align(egui::Align::Center)
        .password(f.password);
    if !f.hint.is_empty() {
        // hints are guidance, not data: always Sans, even in a Mono field
        te = te.hint_text(RichText::new(tr(f.hint)).font(sans(14.0)).color(c.text2));
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    if !f.enabled {
        child.disable();
    }
    let resp = child.add_sized(inner.size(), te);
    let hovered = f.enabled && resp.hovered();
    let border = if f.invalid { c.red } else if hovered { c.text2 } else { c.border };
    let fill = if f.enabled { c.field } else { c.disabled };
    ui.painter().set(bg, egui::epaint::RectShape::new(rect, RADIUS, fill, Stroke::new(1.0, border), egui::StrokeKind::Inside));
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    let name = tr(f.name);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, f.enabled, name.clone()));
    resp
}

/// The id of a time field's text, so its focus and caret can be read before it is drawn.
pub fn time_input_id(ui: &Ui, name: &str) -> egui::Id {
    ui.id().with(("time-input", name))
}

/// Time field (HH:MM, Mono). With focus, ↑/↓ change the part the caret is in: hours by one,
/// minutes by one (Shift: ten). Minutes carry into the hour, so 13:59 ↑ is 14:00. The part that
/// changed stays selected, so the next key keeps acting on it. The chevrons step the minutes.
/// Same 36-unit frame as `input`; the steppers sit in reserved space.
pub fn time_input(ui: &mut Ui, value: &mut String, name: &str, width: f32, invalid: bool) -> egui::Response {
    let c = p(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, CTRL_H), Sense::hover());
    ui.data_mut(|d| d.insert_temp(egui::Id::new("last-input-rect"), rect));
    let bg = ui.painter().add(egui::Shape::Noop);
    let step_w = 22.0;
    let inner = Rect::from_min_max(rect.min + egui::vec2(PAD_X, 1.0), rect.max - egui::vec2(PAD_X + step_w, 1.0));
    let id = time_input_id(ui, name);
    // Keys are read before the text field is drawn: it would otherwise use ↑/↓ to move the caret.
    let mut step = 0i64;
    let mut select_hours: Option<bool> = None;
    if ui.memory(|m| m.has_focus(id)) {
        let (up, down, shift) = ui.input_mut(|i| {
            let shift = i.modifiers.shift;
            let up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) | i.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowUp);
            let down = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) | i.consume_key(egui::Modifiers::SHIFT, egui::Key::ArrowDown);
            (up, down, shift)
        });
        if up || down {
            let colon = value.chars().position(|ch| ch == ':');
            let caret = egui::TextEdit::load_state(ui.ctx(), id).and_then(|st| st.cursor.char_range()).map(|r| r.primary.index);
            // the caret at or before the colon is in the hours
            let hours = matches!((colon, caret), (Some(co), Some(ca)) if ca <= co);
            let n = if hours { 60 } else if shift { 10 } else { 1 };
            step = if up { n } else { -n };
            select_hours = Some(hours);
        }
    }
    let mut keyed = false;
    if step != 0 {
        if let Some(v) = devtrack_core::edit::step_clock(value, step) {
            *value = v;
            keyed = true;
            // keep the changed part selected
            if let (Some(hours), Some(colon)) = (select_hours, value.chars().position(|ch| ch == ':')) {
                let (a, b) = if hours { (0, colon) } else { (colon + 1, value.chars().count()) };
                let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b))));
                st.store(ui.ctx(), id);
            }
        }
        step = 0;
    }
    let te = egui::TextEdit::singleline(value).id(id).frame(false).font(mono_font(14.0)).margin(egui::Margin::ZERO).desired_width(inner.width()).vertical_align(egui::Align::Center);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let mut resp = child.add_sized(inner.size(), te);
    if keyed {
        resp.mark_changed();
    }
    // stepper column
    let col = Rect::from_min_max(egui::pos2(rect.right() - step_w - 6.0, rect.top() + 4.0), egui::pos2(rect.right() - 6.0, rect.bottom() - 4.0));
    let up_r = Rect::from_min_max(col.min, egui::pos2(col.right(), col.center().y));
    let dn_r = Rect::from_min_max(egui::pos2(col.left(), col.center().y), col.max);
    for (r, dir, label_txt) in [(up_r, 1i64, "Later"), (dn_r, -1i64, "Earlier")] {
        let rr = ui.interact(r, ui.id().with((name, label_txt)), Sense::click());
        if rr.hovered() {
            ui.painter().rect_filled(r, 3.0, c.hover);
        }
        let cc = r.center();
        let s = Stroke::new(1.3, if rr.hovered() { c.text } else { c.text2 });
        let d = dir as f32;
        ui.painter().line_segment([cc + egui::vec2(-3.5, 1.8 * d), cc + egui::vec2(0.0, -1.8 * d)], s);
        ui.painter().line_segment([cc + egui::vec2(0.0, -1.8 * d), cc + egui::vec2(3.5, 1.8 * d)], s);
        let t = tr(label_txt);
        rr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t.clone()));
        if rr.on_hover_text(tr("1 minute; Shift: 10 minutes")).clicked() {
            let shift = ui.input(|i| i.modifiers.shift);
            step = dir * if shift { 10 } else { 1 };
        }
    }
    if step != 0 {
        if let Some(v) = devtrack_core::edit::step_clock(value, step) {
            *value = v;
            resp.mark_changed();
        }
    }
    let hovered = resp.hovered();
    let border = if invalid { c.red } else if hovered { c.text2 } else { c.border };
    ui.painter().set(bg, egui::epaint::RectShape::new(rect, RADIUS, c.field, Stroke::new(1.0, border), egui::StrokeKind::Inside));
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    let n = tr(name);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, n.clone()));
    resp
}

/// Read-only value in the same 36-unit frame; legible and selectable.
pub fn readonly(ui: &mut Ui, text: &str, width: f32, is_mono: bool) -> egui::Response {
    let mut s: &str = text;
    let c = p(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, CTRL_H), Sense::hover());
    ui.painter().rect(rect, RADIUS, c.nav, Stroke::new(1.0, c.border), egui::StrokeKind::Inside);
    let inner = Rect::from_min_max(rect.min + egui::vec2(PAD_X, 1.0), rect.max - egui::vec2(PAD_X, 1.0));
    let font = if is_mono { mono_font(14.0) } else { sans(14.0) };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.add_sized(inner.size(), egui::TextEdit::singleline(&mut s).frame(false).font(font).margin(egui::Margin::ZERO).vertical_align(egui::Align::Center))
}

/// Selector trigger: 36 high, text plus a chevron in reserved trailing space.
pub fn select_trigger(ui: &mut Ui, text: &str, placeholder: bool, width: f32, enabled: bool, invalid: bool, is_mono: bool) -> egui::Response {
    let c = p(ui);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, CTRL_H), sense);
    let hovered = enabled && resp.hovered();
    let fill = if !enabled { c.disabled } else if hovered { c.hover } else { c.field };
    let border = if invalid { c.red } else if hovered { c.text2 } else { c.border };
    ui.painter().rect(rect, RADIUS, fill, Stroke::new(1.0, border), egui::StrokeKind::Inside);
    let col = if placeholder || !enabled { c.text2 } else { c.text };
    let font = if is_mono { mono_font(14.0) } else { sans(14.0) };
    let max_w = rect.width() - PAD_X * 2.0 - 18.0;
    let mut t = text.to_string();
    let mut g = ui.painter().layout_no_wrap(t.clone(), font.clone(), col);
    while g.size().x > max_w && t.chars().count() > 2 {
        t = format!("{}…", t.chars().take(t.chars().count().saturating_sub(2)).collect::<String>().trim_end());
        g = ui.painter().layout_no_wrap(t.clone(), font.clone(), col);
    }
    ui.painter().galley(egui::pos2(rect.left() + PAD_X, rect.center().y - g.size().y / 2.0), g, col);
    // chevron
    let cc = egui::pos2(rect.right() - PAD_X - 4.0, rect.center().y);
    let s = Stroke::new(1.4, c.text2);
    ui.painter().line_segment([cc + egui::vec2(-4.0, -2.0), cc + egui::vec2(0.0, 2.0)], s);
    ui.painter().line_segment([cc + egui::vec2(0.0, 2.0), cc + egui::vec2(4.0, -2.0)], s);
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    // remembered so callers (and the geometry check) can read the trigger's outer rectangle
    ui.data_mut(|d| d.insert_temp(egui::Id::new("last-trigger-rect"), rect));
    let full = text.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, enabled, full.clone()));
    if t != text {
        resp.on_hover_text(text)
    } else {
        resp
    }
}

/// A simple selector over fixed options using the shared trigger. Returns true when changed.
pub fn select<T: PartialEq + Clone>(ui: &mut Ui, id_salt: impl std::hash::Hash, value: &mut T, options: &[(T, String)], width: f32, is_mono: bool) -> bool {
    let c = p(ui);
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let resp = select_trigger(ui, &current, false, width, true, false, is_mono);
    let pid = ui.make_persistent_id(id_salt);
    let filter_id = pid.with("filter");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pid));
        ui.data_mut(|d| d.insert_temp(filter_id, String::new()));
    }
    let mut changed = false;
    // As wide as its longest option, between the trigger's width and 440; never taller than
    // eight rows or the room under the trigger: longer lists scroll, and from ten options on
    // a search field narrows them.
    let font = if is_mono { mono_font(14.0) } else { sans(14.0) };
    let widest = options.iter().map(|(_, l)| text_w(ui, l, &font)).fold(0.0, f32::max) + PAD_X * 2.0 + 8.0;
    let pop_w = widest.clamp(width, width.max(440.0));
    let (side, room) = menu_side(ui, resp.rect);
    let room = room.max(MENU_ROW_H * 3.0);
    let searchable = options.len() >= 10;
    let list_h = (MENU_ROW_H * 8.0).min(room - if searchable { CTRL_H + 12.0 } else { 0.0 }).max(MENU_ROW_H * 2.0);
    egui::popup_above_or_below_widget(ui, pid, &resp, side, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(pop_w);
        ui.set_max_width(pop_w);
        let mut filter = ui.data(|d| d.get_temp::<String>(filter_id)).unwrap_or_default();
        if searchable {
            let r = input(ui, &mut filter, Field::new("Search", pop_w).hint("Search"));
            if !r.has_focus() && filter.is_empty() && ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
            ui.add_space(4.0);
        }
        let q = filter.trim().to_lowercase();
        let max_chars = ((pop_w - PAD_X * 2.0) / 7.4).max(8.0) as usize;
        let mut shown = 0;
        egui::ScrollArea::vertical().id_salt(pid.with("list")).max_height(list_h).min_scrolled_height(list_h).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(pop_w);
            for (v, l) in options {
                if !q.is_empty() && !l.to_lowercase().contains(&q) {
                    continue;
                }
                shown += 1;
                let sel = v == value;
                let r = menu_row_raw(ui, &truncate(l, max_chars), sel, is_mono);
                if r.on_hover_text(l).clicked() {
                    *value = v.clone();
                    changed = true;
                }
            }
        });
        if shown == 0 {
            ui.label(RichText::new(tr("No match")).size(12.0).color(c.text2));
        }
        if changed || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.memory_mut(|m| m.close_popup());
        }
    });
    changed
}

/// Menu option row, minimum height 36. The text is an interface message (translated).
pub fn menu_row(ui: &mut Ui, text: &str, selected: bool, is_mono: bool) -> egui::Response {
    let t = tr(text);
    menu_row_raw(ui, &t, selected, is_mono)
}

/// Menu row showing text as given (option lists that may hold names).
pub fn menu_row_raw(ui: &mut Ui, text: &str, selected: bool, is_mono: bool) -> egui::Response {
    let c = p(ui);
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, MENU_ROW_H), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 4.0, c.selected);
    } else {
        ui.painter().rect_filled(rect, 4.0, hover_mix(ui, resp.id, resp.hovered(), Color32::TRANSPARENT, c.hover));
    }
    let font = if is_mono { mono_font(14.0) } else if selected { medium(14.0) } else { sans(14.0) };
    let g = ui.painter().layout_no_wrap(text.to_string(), font, c.text);
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g, c.text);
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    resp
}

/// Segmented switch in the 36-unit family: a framed group of options; neutral selected fill.
/// The width `segmented` takes for these options.
pub fn segmented_w(ui: &Ui, options: &[&str]) -> f32 {
    let font = sans(14.0);
    let sum: f32 = options.iter().map(|o| text_w(ui, &tr(o), &font) + 24.0).sum();
    (sum + 8.0 + 4.0 * options.len().saturating_sub(1) as f32).ceil()
}

pub fn segmented(ui: &mut Ui, options: &[&str], selected: usize) -> Option<usize> {
    let c = p(ui);
    let owned: Vec<String> = options.iter().map(|o| tr(o)).collect();
    let options: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    let options = &options[..];
    let font = sans(14.0);
    let widths: Vec<f32> = options.iter().map(|o| text_w(ui, o, &font) + 24.0).collect();
    // 4 around the segments and 4 between them: the gap equals the group's own padding
    let gap = 4.0;
    let total: f32 = widths.iter().sum::<f32>() + 8.0 + gap * options.len().saturating_sub(1) as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total, CTRL_H), Sense::hover());
    ui.painter().rect(rect, RADIUS, c.field, Stroke::new(1.0, c.border), egui::StrokeKind::Inside);
    let mut x = rect.left() + 4.0;
    let mut out = None;
    for (i, o) in options.iter().enumerate() {
        let r = Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::vec2(widths[i], CTRL_H - 8.0));
        let resp = ui.interact(r, ui.id().with(("seg", i, *o)), Sense::click());
        let sel = i == selected;
        if sel {
            ui.painter().rect_filled(r, 4.0, c.selected);
        } else {
            ui.painter().rect_filled(r, 4.0, hover_mix(ui, resp.id, resp.hovered(), Color32::TRANSPARENT, c.hover));
        }
        // the selected segment's label in amber, the colour of "where you are" across the app
        // (sidebar icons); its fill and heavier weight still mark it without the colour
        let col = if sel { c.amber } else { c.text2 };
        let g = ui.painter().layout_no_wrap(o.to_string(), if sel { medium(14.0) } else { font.clone() }, col);
        ui.painter().galley(egui::pos2(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, col);
        if resp.has_focus() {
            ui.painter().rect_stroke(r.expand(1.0), 5.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
        }
        let label_txt = o.to_string();
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, sel, label_txt.clone()));
        if resp.clicked() && !sel {
            out = Some(i);
        }
        x += widths[i] + gap;
    }
    out
}

/// Label above a control (gap 6), helper or error below (gap 6). `add` draws the control.
pub fn field<R>(ui: &mut Ui, label_txt: &str, help: Option<&str>, error: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    let c = p(ui);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = LABEL_GAP;
        label(ui, label_txt);
        let r = add(ui);
        if let Some(e) = error {
            ui.add(egui::Label::new(RichText::new(tr(e)).size(12.0).color(c.red)).wrap());
        } else if let Some(h) = help {
            ui.label(mixed_job(&tr(h), 12.0, c.text2, sans(12.0), mono_font(12.0)));
        }
        r
    })
    .inner
}

// ---------- surfaces ----------

pub fn panel<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let c = p(ui);
    egui::Frame::new()
        .fill(c.raised)
        .stroke(Stroke::new(1.0, c.line))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::same(20))
        .show(ui, add)
        .inner
}

/// The corners of a table's header band: rounded on top, square where the rows begin.
pub const TABLE_TOP: egui::CornerRadius = egui::CornerRadius { nw: 9, ne: 9, sw: 0, se: 0 };
pub const TABLE_BOTTOM: egui::CornerRadius = egui::CornerRadius { nw: 0, ne: 0, sw: 9, se: 9 };

/// Closes a table drawn as a header band and rows with dividers: removes the divider under the
/// last row (the border replaces it) and draws the table's rounded border from `top` to the
/// current position. Every table in the app ends with this, so they all look alike.
pub fn table_border(ui: &mut Ui, top: f32) {
    let c = p(ui);
    let (left, right) = (ui.max_rect().left(), ui.max_rect().right());
    // the cursor sits one item spacing below the last row's divider: the border ends on the
    // divider itself, which it replaces, not under the gap
    let bottom = ui.cursor().top() - ui.spacing().item_spacing.y;
    if bottom <= top + 2.0 {
        return;
    }
    ui.painter().rect_filled(Rect::from_min_max(egui::pos2(left, bottom - 1.0), egui::pos2(right, bottom)), 0.0, c.canvas);
    let table = Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom));
    clip_corners(ui, table, 10.0, c.canvas);
    ui.painter().rect_stroke(table, 10.0, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
}

/// "Overflow hidden" for a rounded box: whatever a row painted into the four corners (a hover
/// fill, a highlight bar, a divider) is covered with the colour behind the box, so nothing
/// shows outside the rounded border drawn afterwards.
pub fn clip_corners(ui: &Ui, rect: Rect, radius: f32, behind: Color32) {
    let mut mesh = egui::Mesh::default();
    // each corner: the square's outer point, then the arc from one side to the other
    for (corner, centre, from) in [
        (rect.left_top(), rect.left_top() + egui::vec2(radius, radius), std::f32::consts::PI),
        (rect.right_top(), rect.right_top() + egui::vec2(-radius, radius), 1.5 * std::f32::consts::PI),
        (rect.right_bottom(), rect.right_bottom() + egui::vec2(-radius, -radius), 0.0),
        (rect.left_bottom(), rect.left_bottom() + egui::vec2(radius, -radius), 0.5 * std::f32::consts::PI),
    ] {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, behind);
        let steps = 8;
        for i in 0..=steps {
            let a = from + std::f32::consts::FRAC_PI_2 * i as f32 / steps as f32;
            mesh.colored_vertex(centre + egui::vec2(a.cos(), a.sin()) * radius, behind);
        }
        for i in 0..steps as u32 {
            mesh.add_triangle(base, base + 1 + i, base + 2 + i);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

pub fn divider(ui: &mut Ui) {
    let c = p(ui);
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, c.line);
}

pub fn hex(c: &str) -> Color32 {
    let s = c.trim_start_matches('#');
    if s.len() == 6 {
        if let Ok(v) = u32::from_str_radix(s, 16) {
            return Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        }
    }
    Color32::GRAY
}

pub fn dot(ui: &mut Ui, color: Color32, r: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(r * 2.0 + 4.0, r * 2.0 + 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), r, color);
}

/// A status as tall as a button: words in the status colour on a pale fill of the same colour,
/// with no lighter border (the edge is the fill itself, so it measures like the buttons beside
/// it). With `action` it is the button that deals with the status: a chevron, a stronger fill on
/// hover, a focus ring and the tooltip that names where it leads.
/// The width `status_chip` takes for these words, to reserve its place before drawing it.
pub fn status_chip_w(ui: &Ui, text: &str, action: bool) -> f32 {
    let g = ui.painter().layout_job(mixed_job(&tr(text), 14.0, Color32::WHITE, sans(14.0), mono_font(14.0)));
    (g.size().x + PAD_X * 2.0 + if action { 20.0 } else { 0.0 }).max(BTN_MIN_W).ceil()
}

pub fn status_chip(ui: &mut Ui, text: &str, color: Color32, action: Option<&str>) -> egui::Response {
    status_chip_glyph(ui, text, color, action.map(|t| (t, Glyph::Next)))
}

/// `status_chip` with the mark that says what the click does: a chevron leads somewhere, a
/// cross undoes a narrowing.
pub fn status_chip_glyph(ui: &mut Ui, text: &str, color: Color32, action: Option<(&str, Glyph)>) -> egui::Response {
    let glyph = action.map(|(_, g)| g);
    let action = action.map(|(t, _)| t);
    let w = status_chip_w(ui, text, action.is_some());
    let text = tr(text);
    let job = mixed_job(&text, 14.0, color, sans(14.0), mono_font(14.0));
    let g = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, CTRL_H), if action.is_some() { Sense::click() } else { Sense::hover() });
    let hot = action.is_some() && (resp.hovered() || resp.is_pointer_button_down_on());
    // gamma, not linear: a linear 7% reads as 30% on black
    let fill = color.gamma_multiply(if hot { 0.22 } else { 0.14 });
    ui.painter().rect_filled(rect, RADIUS, fill);
    let x = rect.left() + PAD_X;
    ui.painter().galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, color);
    if action.is_some() {
        draw_glyph(ui.painter(), egui::pos2(rect.right() - PAD_X - 4.0, rect.center().y), glyph.unwrap_or(Glyph::Next), color);
        if resp.has_focus() { focus_ring(ui, rect); }
    }
    let t = text.clone();
    let kind = if action.is_some() { egui::WidgetType::Button } else { egui::WidgetType::Label };
    resp.widget_info(|| egui::WidgetInfo::labeled(kind, true, t.clone()));
    match action {
        Some(tip) => resp.on_hover_text(tr(tip)),
        None => resp,
    }
}

/// Display-only status badge (22–24 high): words with a quiet tint; numbers in Mono.
pub fn pill(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.linear_multiply(0.10))
        .stroke(Stroke::new(1.0, color.linear_multiply(0.45)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            let job = mixed_job(&tr(text), 12.0, color, sans(12.0), mono_font(12.0));
            ui.label(job);
        });
}

/// Neutral counter chip: Mono number.
pub fn count_chip(ui: &mut Ui, n: usize) {
    let c = p(ui);
    egui::Frame::new().stroke(Stroke::new(1.0, c.line)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(7, 2)).show(ui, |ui| {
        ui.label(mono(format!("{}", n), 12.0).color(c.text2));
    });
}

// ---------- navigation icons (outline, painter-drawn) ----------

#[derive(Clone, Copy)]
pub enum Icon {
    Today,
    Activities,
    Report,
    Stats,
    Settings,
}

pub fn draw_icon(painter: &egui::Painter, center: egui::Pos2, icon: Icon, color: Color32) {
    let s = Stroke::new(1.4, color);
    let c = center;
    match icon {
        Icon::Today => {
            painter.circle_stroke(c, 6.5, s);
            painter.line_segment([c, c + egui::vec2(0.0, -4.0)], s);
            painter.line_segment([c, c + egui::vec2(3.0, 0.0)], s);
        }
        Icon::Activities => {
            for (dx, dy) in [(-3.5, -3.5), (3.5, -3.5), (-3.5, 3.5), (3.5, 3.5)] {
                painter.rect_stroke(egui::Rect::from_center_size(c + egui::vec2(dx, dy), egui::vec2(5.0, 5.0)), 1.0, s, egui::StrokeKind::Middle);
            }
        }
        Icon::Report => {
            for dy in [-4.5, 0.0, 4.5] {
                painter.circle_filled(c + egui::vec2(-5.5, dy), 1.0, color);
                painter.line_segment([c + egui::vec2(-2.5, dy), c + egui::vec2(6.5, dy)], s);
            }
        }
        Icon::Stats => {
            for (dx, h) in [(-4.5, 5.0), (0.0, 9.0), (4.5, 7.0)] {
                painter.line_segment([c + egui::vec2(dx, 6.0), c + egui::vec2(dx, 6.0 - h)], Stroke::new(2.0, color));
            }
        }
        Icon::Settings => {
            painter.circle_stroke(c, 3.0, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = egui::vec2(a.cos(), a.sin());
                painter.line_segment([c + d * 5.0, c + d * 7.0], s);
            }
        }
    }
}

// ---------- formatting ----------

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>())
    } else {
        s.to_string()
    }
}

/// "6 h 42 min", "42 min", "27 s".
pub fn words(secs: i64) -> String {
    let a = secs.abs();
    let sign = if secs < 0 { "-" } else { "" };
    if a < 60 {
        format!("{}{} s", sign, a)
    } else if a < 3600 {
        format!("{}{} min", sign, a / 60)
    } else {
        format!("{}{} h {:02} min", sign, a / 3600, (a % 3600) / 60)
    }
}

/// Percentages that never show a misleading 0% for small non-zero shares.
pub fn pct(a: i64, b: i64) -> String {
    if b <= 0 || a <= 0 {
        "0%".into()
    } else {
        let v = a as f64 * 100.0 / b as f64;
        if v < 1.0 {
            "<1%".into()
        } else if v > 99.0 && a < b {
            ">99%".into()
        } else {
            format!("{:.0}%", v)
        }
    }
}

pub fn clock_text(secs: i64, h24: bool) -> String {
    use chrono::{Local, TimeZone};
    let Some(t) = Local.timestamp_opt(secs, 0).single() else { return String::new() };
    if h24 {
        t.format("%H:%M").to_string()
    } else {
        t.format("%-I:%M %p").to_string()
    }
}

pub fn ago(ms: i64) -> String {
    let s = (devtrack_core::store::now_ms() - ms) / 1000;
    if s < 10 {
        "just now".into()
    } else if s < 60 {
        trf_m!("{} s ago", s)
    } else if s < 3600 {
        trf_m!("{} min ago", s / 60)
    } else if s < 86400 {
        trf_m!("{} h ago", s / 3600)
    } else {
        trf_m!("{} days ago", s / 86400)
    }
}

/// Source reference, e.g. "DevOps #4812".
pub fn source_ref(t: &Task) -> Option<String> {
    if t.source == SOURCE_AZURE {
        Some(format!("DevOps #{}", t.external_id.clone().unwrap_or_default()))
    } else if t.source != SOURCE_MANUAL {
        Some(format!("{} {}", t.source_label(), t.external_id.clone().unwrap_or_default()))
    } else {
        None
    }
}

// ---------- searchable pickers (shared trigger + popup) ----------

#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    None,
    Changed,
    /// Create a new local project / task with this name.
    Create(String),
    /// Go to the work-item finder.
    Import,
}

fn search_box(ui: &mut Ui, id: egui::Id, hint: &str) -> String {
    let mut q: String = ui.data_mut(|d| d.get_temp::<String>(id).unwrap_or_default());
    let w = ui.available_width();
    let r = input(ui, &mut q, Field::new(hint, w).hint(hint));
    if ui.data_mut(|d| d.remove_temp::<bool>(id.with("initial-focus"))).unwrap_or(false) {
        r.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(id, q.clone()));
    q
}

fn close(ui: &mut Ui, id: egui::Id) {
    ui.data_mut(|d| d.remove::<String>(id));
    ui.memory_mut(|m| m.close_popup());
}

fn matches(hay: &str, q: &str) -> bool {
    q.is_empty() || hay.to_lowercase().contains(&q.trim().to_lowercase())
}

/// Picker row (36 high): dot, name, secondary text.
/// Where a menu opens and how tall it may be. A native window has no room outside itself, so
/// a menu opens below its trigger when there is enough space there (or more than above), and
/// above it otherwise. The second value is the height it can use.
pub fn menu_side(ui: &Ui, trigger: egui::Rect) -> (egui::AboveOrBelow, f32) {
    let screen = ui.ctx().screen_rect();
    let below = screen.bottom() - trigger.bottom() - 16.0;
    let above = trigger.top() - screen.top() - 16.0;
    if below >= 420.0 || below >= above { (egui::AboveOrBelow::Below, below) } else { (egui::AboveOrBelow::Above, above) }
}

/// The height left for a menu's scrolling list once its fixed top (search, pinned rows) is
/// drawn: the rest of `room`, never less than three rows.
pub fn menu_list_h(ui: &Ui, room: f32) -> f32 {
    (room - ui.min_rect().height() - 16.0).max(MENU_ROW_H * 3.0)
}

pub fn pick_row(ui: &mut Ui, selected: bool, color: Option<Color32>, name: &str, sub: &str, sub_mono: bool) -> bool {
    let c = p(ui);
    let w = ui.available_width().max(200.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, MENU_ROW_H), Sense::click());
    if resp.hovered() || selected {
        ui.painter().rect_filled(rect, 4.0, if selected { c.selected } else { c.hover });
    }
    let mut x = rect.left() + 10.0;
    if let Some(col) = color {
        ui.painter().circle_filled(egui::pos2(x + 3.5, rect.center().y), 3.5, col);
    }
    x += 14.0;
    let g = ui.painter().layout_no_wrap(truncate(name, 40), if selected { medium(14.0) } else { sans(14.0) }, c.text);
    let nw = g.size().x;
    ui.painter().galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, c.text);
    if !sub.is_empty() {
        let job = if sub_mono { mixed_job(sub, 12.0, c.text2, sans(12.0), mono_font(12.0)) } else { mixed_job(sub, 12.0, c.text2, sans(12.0), sans(12.0)) };
        let g2 = ui.painter().layout_job(job);
        ui.painter().galley(egui::pos2(x + nw + 10.0, rect.center().y - g2.size().y / 2.0), g2, c.text2);
    }
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    resp.clicked()
}

/// Review pictures: open the timer's project menu once (the "timer-project" scene).
pub static SHOT_OPEN_PROJECT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SHOT_CLEAR_AT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// How a picker opens: a full select field of the given width, or an underlined word inside a
/// line of text (the project and client under the timer's task).
#[derive(Clone, Copy, PartialEq)]
pub enum Trigger {
    Field(f32),
    Link,
}

impl Trigger {
    fn width(self) -> f32 {
        match self {
            Trigger::Field(w) => w,
            Trigger::Link => 0.0,
        }
    }
}

/// An underlined word that opens a menu: the text colour when it names something, the secondary
/// colour when it is a placeholder; a dot of the item's colour before it when given.
pub fn link_trigger(ui: &mut Ui, text: &str, placeholder: bool, dot_color: Option<Color32>) -> egui::Response {
    let c = p(ui);
    let font = sans(13.0);
    let col = if placeholder { c.text2 } else { c.text };
    let text = truncate(text, 36);
    let g = ui.painter().layout_no_wrap(text.clone(), font, col);
    let lead = if dot_color.is_some() { 13.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(lead + g.size().x, 20.0), Sense::click());
    let hot = resp.hovered() || resp.is_pointer_button_down_on();
    if hot {
        ui.painter().rect_filled(rect.expand2(egui::vec2(6.0, 2.0)), 4.0, c.hover);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if let Some(dc) = dot_color {
        ui.painter().circle_filled(egui::pos2(rect.left() + 3.5, rect.center().y), 3.5, dc);
    }
    let ty = rect.center().y - g.size().y / 2.0;
    let baseline = ty + g.rows.first().and_then(|r| r.glyphs.first()).map(|gl| gl.pos.y).unwrap_or(g.size().y * 0.8);
    let x0 = rect.left() + lead;
    let x1 = x0 + g.size().x;
    ui.painter().galley(egui::pos2(x0, ty), g, col);
    // one hairline under the word, a little below the baseline, as in running text
    let line_col = if hot { col } else { col.gamma_multiply(0.55) };
    ui.painter().line_segment([egui::pos2(x0, baseline + 3.0), egui::pos2(x1, baseline + 3.0)], Stroke::new(1.0, line_col));
    if resp.has_focus() {
        focus_ring(ui, rect.expand2(egui::vec2(6.0, 2.0)));
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
    resp
}

fn picker_trigger(ui: &mut Ui, trigger: Trigger, text: &str, placeholder: bool, dot_color: Option<Color32>) -> egui::Response {
    match trigger {
        Trigger::Field(w) => select_trigger(ui, text, placeholder, w, true, false, false),
        Trigger::Link => link_trigger(ui, text, placeholder, dot_color),
    }
}

/// Project picker: search, recent, explicit empty result, create.
pub fn project_picker(ui: &mut Ui, salt: impl std::hash::Hash, db: &Db, recent: &[Id], value: &mut Option<Id>, width: f32) -> Pick {
    project_picker_with(ui, salt, db, recent, value, Trigger::Field(width), None, true)
}

/// Client picker: search, No client, explicit empty result, create. `Pick::Create` carries the
/// typed name; the caller creates the client.
pub fn client_select(ui: &mut Ui, salt: impl std::hash::Hash, db: &Db, value: &mut Option<Id>, width: f32) -> Pick {
    client_select_with(ui, salt, db, value, Trigger::Field(width))
}

pub fn client_select_with(ui: &mut Ui, salt: impl std::hash::Hash, db: &Db, value: &mut Option<Id>, trigger: Trigger) -> Pick {
    let mut out = Pick::None;
    let (txt, ph) = match db.client(value) {
        Some(cl) => (if cl.archived { format!("{} ({})", cl.name, tr("archived")) } else { cl.name.clone() }, false),
        None => (tr("No client"), true),
    };
    let pid = ui.make_persistent_id((&salt, "client-popup"));
    let sid = ui.make_persistent_id((&salt, "client-q"));
    let resp = picker_trigger(ui, trigger, &txt, ph, None);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pid));
        ui.data_mut(|d| d.insert_temp(sid.with("initial-focus"), true));
    }
    let was_open = ui.memory(|m| m.is_popup_open(pid));
    let (side, room) = menu_side(ui, resp.rect);
    egui::popup_above_or_below_widget(ui, pid, &resp, side, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(300f32.max(trigger.width()).min(ui.ctx().screen_rect().width() - 32.0));
        let q = search_box(ui, sid, "Search client, or create one");
        let clients: Vec<&Client> = db.live_clients().into_iter().filter(|c| !c.archived || value.as_ref() == Some(&c.id)).collect();
        let list: Vec<&&Client> = clients.iter().filter(|c| matches(&c.name, &q)).collect();
        let exact = clients.iter().any(|c| c.name.eq_ignore_ascii_case(q.trim()));
        let can_create = !q.trim().is_empty() && !exact;
        // only the search stays put: everything else scrolls in the room the window has
        let list_h = menu_list_h(ui, room - if can_create { CTRL_H + 12.0 } else { 0.0 });
        egui::ScrollArea::vertical().id_salt(pid.with("list")).max_height(list_h).min_scrolled_height(list_h).show(ui, |ui| {
            if q.is_empty() && pick_row(ui, value.is_none(), None, &tr("No client"), "", false) {
                *value = None;
                out = Pick::Changed;
                close(ui, sid);
            }
            for cl in &list {
                if pick_row(ui, value.as_ref() == Some(&cl.id), None, &cl.name, "", false) {
                    *value = Some(cl.id.clone());
                    out = Pick::Changed;
                    close(ui, sid);
                }
            }
            if list.is_empty() && !q.trim().is_empty() {
                meta(ui, trf_m!("No client matches “{}”.", q.trim()));
            }
            if clients.is_empty() && q.is_empty() {
                meta(ui, "No clients yet: type a name to create one.");
            }
        });
        if can_create && secondary(ui, &trf_m!("+ Create client “{}”", truncate(q.trim(), 28))).clicked() {
            out = Pick::Create(q.trim().to_string());
            close(ui, sid);
        }
    });
    if out != Pick::None || (was_open && ui.input(|i| i.key_pressed(egui::Key::Escape))) { resp.request_focus(); }
    out
}

/// The same picker, limited to one client's projects when `client` is given. Its trigger shows
/// the project's name alone: the client is in a field of its own next to it.
pub fn project_picker_in(ui: &mut Ui, salt: impl std::hash::Hash, db: &Db, recent: &[Id], value: &mut Option<Id>, trigger: Trigger, client: Option<&Id>) -> Pick {
    project_picker_with(ui, salt, db, recent, value, trigger, client, false)
}

#[allow(clippy::too_many_arguments)]
fn project_picker_with(ui: &mut Ui, salt: impl std::hash::Hash, db: &Db, recent: &[Id], value: &mut Option<Id>, trigger: Trigger, client: Option<&Id>, show_client: bool) -> Pick {
    let mut out = Pick::None;
    // the client follows the project, so two projects with one name are told apart once chosen
    let (txt, ph, dot_color) = match db.project(value) {
        Some(pr) => {
            let cl = db.client_name(pr);
            (if cl.is_empty() || !show_client { pr.name.clone() } else { format!("{}  ·  {}", pr.name, cl) }, false, Some(hex(&pr.color)))
        }
        None => (tr(if trigger == Trigger::Link { "No project" } else { "Choose project" }), true, None),
    };
    let pid = ui.make_persistent_id((&salt, "project-popup"));
    let sid = ui.make_persistent_id((&salt, "project-q"));
    let resp = picker_trigger(ui, trigger, &txt, ph, dot_color);
    // opened once the window has settled, as a click would, with a search that finds nothing;
    // the search is emptied a little later, as a person would (the menu must grow back)
    let shot_open = trigger == Trigger::Link && ui.ctx().cumulative_pass_nr() > 40 && SHOT_OPEN_PROJECT.swap(false, std::sync::atomic::Ordering::Relaxed);
    if resp.clicked() || shot_open {
        ui.memory_mut(|m| m.toggle_popup(pid));
        ui.data_mut(|d| d.insert_temp(sid.with("initial-focus"), true));
    }
    if shot_open {
        ui.data_mut(|d| d.insert_temp(sid, "zzzz".to_string()));
        SHOT_CLEAR_AT.store(ui.ctx().cumulative_pass_nr() as u32 + 30, std::sync::atomic::Ordering::Relaxed);
    }
    let clear_at = SHOT_CLEAR_AT.load(std::sync::atomic::Ordering::Relaxed);
    if trigger == Trigger::Link && clear_at > 0 && ui.ctx().cumulative_pass_nr() as u32 >= clear_at {
        SHOT_CLEAR_AT.store(0, std::sync::atomic::Ordering::Relaxed);
        ui.data_mut(|d| d.insert_temp(sid, String::new()));
    }
    let was_open = ui.memory(|m| m.is_popup_open(pid));
    let (side, room) = menu_side(ui, resp.rect);
    egui::popup_above_or_below_widget(ui, pid, &resp, side, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(340f32.max(trigger.width()).min(ui.ctx().screen_rect().width() - 32.0));
        let q = search_box(ui, sid, "Search project or client, or create one");
        let projects: Vec<&Project> = db.live_projects().into_iter().filter(|p| !p.archived && (client.is_none() || p.client_id.as_ref() == client)).collect();
        let mut list: Vec<&&Project> = projects.iter().filter(|p| matches(&format!("{} {}", p.name, db.client_name(p)), &q)).collect();
        // grouped by client: No client first, then clients A–Z
        list.sort_by_key(|p| (!db.client_name(p).is_empty(), db.client_name(p).to_lowercase(), p.name.to_lowercase()));
        let exact = projects.iter().any(|p| p.name.eq_ignore_ascii_case(q.trim()));
        let can_create = !q.trim().is_empty() && !exact;
        // Only the search stays put: the recent ones, No project and the groups scroll together
        // in the room the window has on that side, so the list never shrinks to a row or two.
        let list_h = menu_list_h(ui, room - if can_create { CTRL_H + 12.0 } else { 0.0 });
        egui::ScrollArea::vertical().id_salt(pid.with("list")).max_height(list_h).min_scrolled_height(list_h).show(ui, |ui| {
            if q.is_empty() && !recent.is_empty() && projects.len() > 6 {
                meta(ui, "Recent");
                for id in recent.iter().take(3) {
                    if let Some(pr) = projects.iter().find(|p| &p.id == id) {
                        if pick_row(ui, value.as_deref() == Some(pr.id.as_str()), Some(hex(&pr.color)), &pr.name, &db.client_name(pr), false) {
                            *value = Some(pr.id.clone());
                            out = Pick::Changed;
                            close(ui, sid);
                        }
                    }
                }
                ui.separator();
            }
            if q.is_empty() && pick_row(ui, value.is_none(), None, &tr("No project"), "", false) {
                *value = None;
                out = Pick::Changed;
                close(ui, sid);
            }
            let mut group: Option<String> = None;
            for pr in &list {
                let g = db.client_name(pr);
                if group.as_ref() != Some(&g) {
                    ui.add_space(4.0);
                    let head = if g.is_empty() { tr("No client") } else { g.clone() };
                    ui.label(RichText::new(head).size(12.0).color(p(ui).text2));
                    group = Some(g);
                }
                if pick_row(ui, value.as_deref() == Some(pr.id.as_str()), Some(hex(&pr.color)), &pr.name, "", false) {
                    *value = Some(pr.id.clone());
                    out = Pick::Changed;
                    close(ui, sid);
                }
            }
            if list.is_empty() && !q.trim().is_empty() {
                meta(ui, trf_m!("No project matches “{}”.", q.trim()));
            }
            if projects.is_empty() && q.is_empty() {
                meta(ui, if client.is_some() { "This client has no projects yet: type a name to create one." } else { "No projects yet: type a name to create one." });
            }
        });
        if can_create && secondary(ui, &trf_m!("+ Create project “{}”", truncate(q.trim(), 28))).clicked() {
            out = Pick::Create(q.trim().to_string());
            close(ui, sid);
        }
    });
    if out != Pick::None || (was_open && ui.input(|i| i.key_pressed(egui::Key::Escape))) { resp.request_focus(); }
    out
}

/// Project filter, distinct from assignment: None means all projects, never "No project".
/// Search and scroll stay available regardless of the number of projects.
pub fn project_filter(ui: &mut Ui, salt: impl std::hash::Hash, db: &Db, value: &mut Option<Id>, width: f32) -> bool {
    let text = db.project(value).map(|p| p.name.clone()).unwrap_or_else(|| tr("All projects"));
    let pid = ui.make_persistent_id((&salt, "filter-popup"));
    let sid = ui.make_persistent_id((&salt, "filter-query"));
    let response = select_trigger(ui, &text, false, width, true, false, false);
    if response.clicked() {
        let opening = !ui.memory(|m| m.is_popup_open(pid));
        ui.memory_mut(|m| m.toggle_popup(pid));
        if opening {
            ui.data_mut(|d| { d.insert_temp(sid, String::new()); d.insert_temp(sid.with("initial-focus"), true); });
        }
    }
    let mut changed = false;
    let (side, room) = menu_side(ui, response.rect);
    egui::popup_above_or_below_widget(ui, pid, &response, side, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(320.0f32.max(width).min(ui.ctx().screen_rect().width() - 32.0));
        let query = search_box(ui, sid, "Search projects or clients");
        if menu_row(ui, "All projects", value.is_none(), false).clicked() {
            *value = None;
            changed = true;
            close(ui, sid);
        }
        ui.separator();
        let projects = filtered_projects(db, &query);
        let list_h = menu_list_h(ui, room);
        egui::ScrollArea::vertical().id_salt(pid.with("results")).max_height(list_h).min_scrolled_height(list_h).show(ui, |ui| {
            for project in &projects {
                let client = db.client_name(project);
                let title = if client.is_empty() { project.name.clone() } else { format!("{} · {}", project.name, client) };
                if menu_row(ui, &title, value.as_ref() == Some(&project.id), false).on_hover_text(&title).clicked() {
                    *value = Some(project.id.clone());
                    changed = true;
                    close(ui, sid);
                }
            }
            if projects.is_empty() { meta(ui, "No matching projects"); }
        });
    });
    if changed { response.request_focus(); }
    changed
}

fn filtered_projects<'a>(db: &'a Db, query: &str) -> Vec<&'a Project> {
    let mut result: Vec<_> = db.live_projects().into_iter().filter(|p| matches(&format!("{} {}", p.name, db.client_name(p)), query)).collect();
    result.sort_by_key(|p| p.name.to_lowercase());
    result
}

/// Task picker: search by title, number or source project; recent; create; import.
/// Picking a task sets its project too.
pub fn task_picker(ui: &mut Ui, salt: impl std::hash::Hash, db: &Db, recent: &[Id], project: &mut Option<Id>, task: &mut Option<Id>, width: f32) -> Pick {
    let mut out = Pick::None;
    let (txt, ph) = match db.task(task) {
        Some(t) => (t.display(), false),
        None => (tr("Choose task"), true),
    };
    let pid = ui.make_persistent_id((&salt, "task-popup"));
    let sid = ui.make_persistent_id((&salt, "task-q"));
    let resp = select_trigger(ui, &txt, ph, width, true, false, false);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pid));
        ui.data_mut(|d| d.insert_temp(sid.with("initial-focus"), true));
    }
    let was_open = ui.memory(|m| m.is_popup_open(pid));
    let (side, room) = menu_side(ui, resp.rect);
    egui::popup_above_or_below_widget(ui, pid, &resp, side, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(400f32.max(width).min(ui.ctx().screen_rect().width() - 32.0));
        let q = search_box(ui, sid, "Search by title or #number, or create one");
        let mut all: Vec<&Task> = db.live_tasks().filter(|t| !t.done).collect();
        all.sort_by_key(|t| (t.project_id != *project, t.title.to_lowercase()));
        let hay = |t: &Task| {
            format!(
                "{} #{} {} {}",
                t.title,
                t.external_id.clone().unwrap_or_default(),
                t.external_project.clone().unwrap_or_default(),
                db.project(&t.project_id).map(|p| p.name.clone()).unwrap_or_default()
            )
        };
        let mut choice: Option<(Option<Id>, Option<Id>)> = None;
        let current = task.clone();
        let in_project = project.clone();
        let sub = |t: &Task| {
            let mut v = vec![];
            if let Some(r) = source_ref(t) {
                v.push(r);
            }
            if let Some(pr) = db.project(&t.project_id) {
                v.push(pr.name.clone());
            }
            v.join(" · ")
        };
        let color = |t: &Task| db.project(&t.project_id).map(|p| hex(&p.color));
        let in_scope = all.iter().filter(|t| in_project.is_none() || t.project_id == in_project).count();
        if q.is_empty() && !recent.is_empty() && in_scope > 6 {
            meta(ui, "Recent");
            for id in recent.iter().take(4) {
                if let Some(t) = all.iter().find(|t| &t.id == id) {
                    if pick_row(ui, current.as_deref() == Some(t.id.as_str()), color(t), &t.title, &sub(t), true) {
                        choice = Some((Some(t.id.clone()), t.project_id.clone()));
                    }
                }
            }
            ui.separator();
        }
        if q.is_empty() && pick_row(ui, current.is_none(), None, "No task", "", false) {
            choice = Some((None, None));
        }
        let list: Vec<&&Task> = all.iter().filter(|t| (in_project.is_none() || t.project_id == in_project || !q.is_empty()) && matches(&hay(t), &q)).collect();
        let list_h = menu_list_h(ui, room);
        egui::ScrollArea::vertical().max_height(list_h).min_scrolled_height(list_h).show(ui, |ui| {
            for t in &list {
                if pick_row(ui, current.as_deref() == Some(t.id.as_str()), color(t), &t.title, &sub(t), true) {
                    choice = Some((Some(t.id.clone()), t.project_id.clone()));
                }
            }
        });
        if list.is_empty() {
            meta(ui, if q.trim().is_empty() { "No tasks in this project yet.".to_string() } else { trf_m!("No task matches “{}”.", q.trim()) });
        }
        if let Some((t, pr)) = choice {
            *task = t.clone();
            if t.is_some() && pr.is_some() {
                *project = pr;
            }
            out = Pick::Changed;
            close(ui, sid);
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if !q.trim().is_empty() && secondary(ui, &trf_m!("+ Create task “{}”", truncate(q.trim(), 24))).clicked() {
                out = Pick::Create(q.trim().to_string());
                close(ui, sid);
            }
            if ghost(ui, "Find in Azure DevOps…").clicked() {
                out = Pick::Import;
                close(ui, sid);
            }
        });
    });
    if out != Pick::None || (was_open && ui.input(|i| i.key_pressed(egui::Key::Escape))) { resp.request_focus(); }
    out
}

pub fn recent_projects(db: &Db) -> Vec<Id> {
    let mut list: Vec<&TimeEntry> = db.live_entries().filter(|e| e.project_id.is_some()).collect();
    list.sort_by_key(|e| std::cmp::Reverse(e.start));
    let mut out: Vec<Id> = vec![];
    for e in list {
        let id = e.project_id.clone().unwrap();
        if !out.contains(&id) {
            out.push(id);
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

pub fn recent_tasks(db: &Db) -> Vec<Id> {
    let mut list: Vec<&TimeEntry> = db.live_entries().filter(|e| e.task_id.is_some()).collect();
    list.sort_by_key(|e| std::cmp::Reverse(e.start));
    let mut out: Vec<Id> = vec![];
    for e in list {
        let id = e.task_id.clone().unwrap();
        if !out.contains(&id) {
            out.push(id);
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

/// Outer rectangle of the most recently drawn selector trigger.
pub fn last_trigger_rect(ui: &Ui) -> Rect {
    ui.data(|d| d.get_temp::<Rect>(egui::Id::new("last-trigger-rect"))).unwrap_or(Rect::NOTHING)
}

/// Outer frame rectangle of the most recently drawn input.
pub fn last_input_rect(ui: &Ui) -> Rect {
    ui.data(|d| d.get_temp::<Rect>(egui::Id::new("last-input-rect"))).unwrap_or(Rect::NOTHING)
}

/// One row whose items share a centre line `h` high: text, links and 36-high controls line up
/// whatever order they are added in (egui otherwise centres each item on the row height reached
/// so far, so text added before a taller control sits high).
pub fn row<R>(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.set_min_height(h);
        add(ui)
    })
    .inner
}

/// Wrapping variant of [`row`]: every wrapped line is `h` high and items are centred on it.
pub fn row_wrapped<R>(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.set_row_height(h);
        // labels stay whole items, centred like the rest; in egui's flowing mode they would sit on
        // the bottom of the line, below a centred link or button
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        add(ui)
    })
    .inner
}

/// Checkbox with a square 16 px box (radius 3): the theme's 6 px control radius would draw it as
/// a circle, which reads as a radio button.
pub fn checkbox(ui: &mut Ui, value: &mut bool, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
            w.corner_radius = 3.0.into();
        }
        ui.spacing_mut().icon_width = 16.0;
        ui.spacing_mut().icon_width_inner = 8.0;
        ui.checkbox(value, text)
    })
    .inner
}

/// The one disclosure style: the same drawn chevron as the report rows (› closed, ⌄ open), a
/// Medium 14 title, a 32-high header that is entirely clickable. Returns the header response
/// and, when open, what `add` returned.
pub fn disclosure<R>(ui: &mut Ui, id_salt: impl std::hash::Hash, title: &str, default_open: bool, add: impl FnOnce(&mut Ui) -> R) -> (egui::Response, Option<R>) {
    let c = p(ui);
    let id = ui.make_persistent_id(id_salt);
    let mut open = ui.data_mut(|d| *d.get_persisted_mut_or(id, default_open));
    let title = tr(title);
    let g = ui.painter().layout_no_wrap(title.clone(), medium(14.0), c.text);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(g.size().x + 36.0, 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, RADIUS, c.hover);
    }
    draw_glyph(ui.painter(), egui::pos2(rect.left() + 12.0, rect.center().y), if open { Glyph::Down } else { Glyph::Next }, if resp.hovered() { c.text } else { c.text2 });
    ui.painter().galley(egui::pos2(rect.left() + 28.0, rect.center().y - g.size().y / 2.0), g, c.text);
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    let t = title.clone();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, t.clone()));
    let body = if open {
        Some(ui.indent(id.with("body"), |ui| add(ui)).inner)
    } else {
        None
    };
    (resp, body)
}

/// Fixed-width column cell (content top-down). Keeps table columns aligned.
pub fn col<R>(ui: &mut Ui, w: f32, h: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(egui::vec2(w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_min_width(w);
        ui.set_max_width(w);
        add(ui)
    })
    .inner
}

/// Fixed-width right-aligned cell for numerical comparison columns.
pub fn col_right<R>(ui: &mut Ui, w: f32, h: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(egui::vec2(w, h), egui::Layout::top_down(egui::Align::Max), |ui| {
        ui.set_min_width(w);
        ui.set_max_width(w);
        add(ui)
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentages_never_hide_small_values() {
        assert_eq!(pct(27, 5600), "<1%");
        assert_eq!(pct(0, 5600), "0%");
        assert_eq!(pct(2800, 5600), "50%");
        assert_eq!(pct(5599, 5600), ">99%");
        assert_eq!(words(6 * 3600 + 42 * 60), "6 h 42 min");
        assert_eq!(words(27), "27 s");
    }
    #[test]
    fn empty_picker_search_does_not_reclaim_focus() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let query_id = egui::Id::new("focus-regression");
        let mut target = None;
        // Simulate opening, then moving keyboard focus to another control while query is empty.
        for frame in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if frame == 0 { ui.data_mut(|d| d.insert_temp(query_id.with("initial-focus"), true)); }
                    if frame == 2 { ui.memory_mut(|m| m.request_focus(target.unwrap())); }
                    search_box(ui, query_id, "Search projects");
                    let button = secondary(ui, "No project");
                    target = Some(button.id);
                });
            });
        }
        assert_eq!(ctx.memory(|m| m.focused()), target);
    }

}

#[cfg(test)]
mod project_filter_tests {
    use super::*;
    use devtrack_core::{sample::{build_state, DemoState}, store::{now_secs, Store}};

    #[test]
    fn project_filter_searches_a_large_catalog_by_name_and_client() {
        let mut db = build_state(now_secs(), DemoState::Incomplete, false);
        for n in 0..300 {
            let id = format!("catalog-{n:03}");
            db.projects.insert(id.clone(), Project { id, name: format!("Catalog project {n:03}"), client_id: Some("c-northwind".into()), ..Default::default() });
        }
        assert_eq!(filtered_projects(&db, "PROJECT 299").len(), 1);
        assert!(filtered_projects(&db, "northwind").len() >= 300);
        assert!(filtered_projects(&db, "not-present-xyz").is_empty());
        let expected = "catalog-299".to_string();
        let mut selected = Some(expected.clone());
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut trigger = egui::Rect::NOTHING;
        let store = Store::in_memory(db);
        // Draw the actual filter, open its popup and type into its focused search field.
        let mut draw = |events| ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 700.0))), events, ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                project_filter(ui, "test-filter", &store.db, &mut selected, 200.0);
                trigger = last_trigger_rect(ui);
            });
        });
        let _ = draw(vec![]);
        let point = trigger.center();
        // The trigger rect is measured by the shared component, never assumed from screen pixels.
        let mut draw = |events| ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 700.0))), events, ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| { project_filter(ui, "test-filter", &store.db, &mut selected, 200.0); });
        });
        let _ = draw(vec![egui::Event::PointerMoved(point), egui::Event::PointerButton { pos: point, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        let _ = draw(vec![egui::Event::PointerButton { pos: point, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
        let _ = draw(vec![]);
        let out = draw(vec![egui::Event::Text("PROJECT 299".into())]);
        fn text(shapes: &[egui::epaint::ClippedShape], result: &mut Vec<String>) {
            fn visit(s: &egui::Shape, out: &mut Vec<String>) {
                match s { egui::Shape::Text(t) => out.push(t.galley.job.text.clone()), egui::Shape::Vec(v) => for s in v { visit(s, out); }, _ => {} }
            }
            for s in shapes { visit(&s.shape, result); }
        }
        let mut strings = vec![];
        text(&out.shapes, &mut strings);
        assert!(strings.iter().any(|s| s.contains("Catalog project 299")), "search result missing: {strings:?}");
        assert!(!strings.iter().any(|s| s.contains("Catalog project 298")), "unfiltered popup");
        assert_eq!(selected, Some(expected), "typing must not change the selected project");
    }
}

#[cfg(test)]
mod date_input_tests {
    use super::*;

    fn frame(ctx: &egui::Context, value: &mut String, events: Vec<egui::Event>) -> (Rect, bool) {
        let mut out = (Rect::NOTHING, false);
        let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 600.0))), events, ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let r = date_input(ui, value, "Date", 200.0, false, true);
                out = (ui.data(|d| d.get_temp::<Rect>(egui::Id::new("last-input-rect"))).unwrap_or(Rect::NOTHING), r.changed());
            });
        });
        out
    }

    fn click(ctx: &egui::Context, value: &mut String, pos: egui::Pos2) -> bool {
        frame(ctx, value, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        let (_, changed) = frame(ctx, value, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
        frame(ctx, value, vec![]).1 || changed
    }

    #[test]
    fn calendar_opens_from_the_button_and_writes_the_chosen_day() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut value = "2026-11-17".to_string();
        let (rect, _) = frame(&ctx, &mut value, vec![]);
        // the calendar button sits at the right edge of the field
        click(&ctx, &mut value, egui::pos2(rect.right() - 18.0, rect.center().y));
        // grid: 10 margin, header 36 + 4, weekday row 20 + 4, then 32-high rows of 36-wide cells.
        // November 2026 starts on a Sunday (Monday-first: column 6), so row 1 col 0 is the 2nd.
        let grid_top = rect.bottom() + 4.0 + 10.0 + 36.0 + 4.0 + 20.0 + 4.0;
        let second = egui::pos2(rect.left() + 10.0 + 18.0, grid_top + 32.0 + 4.0 + 16.0);
        click(&ctx, &mut value, second);
        assert_eq!(value, "2026-11-02");
    }
}

#[cfg(test)]
mod time_input_tests {
    use super::*;

    /// Draws one time field, with the caret at `caret` and the given key pressed.
    fn press(value: &str, caret: usize, key: egui::Key, shift: bool) -> String {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut v = value.to_string();
        // returns the field's id, read inside the frame
        let frame = |ctx: &egui::Context, v: &mut String, events: Vec<egui::Event>, modifiers: egui::Modifiers| -> egui::Id {
            let mut id = egui::Id::NULL;
            let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))), events, modifiers, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    id = time_input_id(ui, "Start time");
                    time_input(ui, v, "Start time", 160.0, false);
                });
            });
            id
        };
        let id = frame(&ctx, &mut v, vec![], egui::Modifiers::NONE);
        ctx.memory_mut(|m| m.request_focus(id));
        let mut st = egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
        st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(caret))));
        st.store(&ctx, id);
        frame(&ctx, &mut v, vec![], egui::Modifiers::NONE);
        let modifiers = if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
        frame(&ctx, &mut v, vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }], modifiers);
        v
    }

    #[test]
    fn arrows_change_the_part_under_the_caret() {
        // caret in the minutes: one minute, ten with Shift, carrying into the hour
        assert_eq!(press("13:20", 4, egui::Key::ArrowUp, false), "13:21");
        assert_eq!(press("13:20", 4, egui::Key::ArrowDown, true), "13:10");
        assert_eq!(press("13:59", 5, egui::Key::ArrowUp, false), "14:00");
        assert_eq!(press("13:55", 5, egui::Key::ArrowUp, true), "14:05");
        assert_eq!(press("14:00", 3, egui::Key::ArrowDown, false), "13:59");
        // caret in the hours: one hour, Shift changes nothing
        assert_eq!(press("13:20", 1, egui::Key::ArrowUp, false), "14:20");
        assert_eq!(press("13:20", 0, egui::Key::ArrowUp, true), "14:20");
        assert_eq!(press("13:20", 2, egui::Key::ArrowDown, false), "12:20");
    }
}

/// Width over height of the trimmed mark in `assets/icons/mark.png`.
const MARK_ASPECT: f32 = 183.0 / 192.0;

/// The width `brand_lockup` takes, for placing it before it is drawn (centred in a bar).
pub fn brand_lockup_width(ui: &Ui, mark_h: f32, name_size: f32) -> f32 {
    let name = ui.painter().layout_no_wrap(tr("DevTrack"), brand_font(name_size), p(ui).text);
    (mark_h * MARK_ASPECT).round() + (mark_h * 0.42).round() + name.size().x.ceil()
}

/// The DevTrack mark and, beside it, the name: the logo of the app.
///
/// The mark is the white drawing on dark surfaces and the black one on light surfaces: one
/// white image with transparency, tinted with the text colour. The name is set in Fira Code
/// Bold, the face the mark was drawn from, at half the mark's scale, and sits on the mark's
/// baseline, so the dot of the mark closes the "d" and opens the name on one line.
/// `name_size` None draws the mark alone (narrow sidebar). Returns the rectangle used.
pub fn brand_lockup(ui: &mut egui::Ui, mark_h: f32, name_size: Option<f32>) -> egui::Rect {
    let c = p(ui);
    let id = egui::Id::new("brand-mark-texture");
    let texture = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)).or_else(|| {
        let img = image::load_from_memory(include_bytes!("../../assets/icons/mark.png")).ok()?.into_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let t = ui.ctx().load_texture("brand-mark", egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(id, t.clone()));
        Some(t)
    });
    let mark_w = (mark_h * MARK_ASPECT).round();
    let name = name_size.map(|size| ui.painter().layout_no_wrap(tr("DevTrack"), brand_font(size), c.text));
    let gap = (mark_h * 0.42).round();
    let w = mark_w + name.as_ref().map_or(0.0, |g| gap + g.size().x.ceil());
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, mark_h), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "DevTrack"));
    if let Some(t) = texture {
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(t.id(), egui::Rect::from_min_size(rect.min, egui::vec2(mark_w, mark_h)), uv, c.text);
    }
    if let (Some(galley), Some(size)) = (name, name_size) {
        // Fira Code's ascent is 0.935 em: the row's top is that far above the shared baseline
        let top = rect.bottom() - (size * FIRA_ASCENT).round();
        ui.painter().galley(egui::pos2(rect.left() + mark_w + gap, top), galley, c.text);
    }
    rect
}

/// What a page shows when it has nothing to show: the page's icon on a small tile, what is
/// missing in one line, why it matters and how to fill it in a second, then the way forward.
/// Left-aligned at the top of the content, where the data would start, with no card around it:
/// it is the page's own content, not a notice laid over it.
pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, body: &str, actions: impl FnOnce(&mut Ui)) {
    let c = p(ui);
    ui.add_space(24.0);
    let (tile, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), Sense::hover());
    ui.painter().rect(tile, RADIUS + 2.0, c.raised, Stroke::new(1.0, c.line), egui::StrokeKind::Inside);
    draw_icon(ui.painter(), tile.center(), icon, c.amber);
    ui.add_space(16.0);
    ui.label(RichText::new(tr(title)).font(medium(17.0)).color(c.text));
    ui.add_space(4.0);
    let w = 480.0f32.min(ui.available_width());
    ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
        ui.set_max_width(w);
        ui.label(mixed_job(&tr(body), 14.0, c.text2, sans(14.0), mono_font(14.0)));
    });
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = ROW_GAP;
        actions(ui);
    });
}

/// The close control of a dialog: an X at the top right, on the title's line. It does what Esc
/// does in that dialog, so one with unsaved changes still asks before discarding them.
/// Call it right after the dialog sets its width; it takes no room in the layout.
pub fn modal_close(ui: &mut Ui) -> bool {
    let c = p(ui);
    let r = ui.max_rect();
    let rect = egui::Rect::from_min_size(egui::pos2(r.right() - 28.0, r.top() - 1.0), egui::vec2(28.0, 28.0));
    let resp = ui.interact(rect, egui::Id::new("modal-close"), Sense::click());
    let over = resp.hovered() || resp.is_pointer_button_down_on();
    ui.painter().rect_filled(rect, RADIUS, hover_mix(ui, resp.id, over, Color32::TRANSPARENT, c.hover));
    draw_glyph(ui.painter(), rect.center(), Glyph::Close, if over { c.text } else { c.text2 });
    if resp.has_focus() {
        focus_ring(ui, rect);
    }
    let name = tr("Close (Esc)");
    let n = name.clone();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, n.clone()));
    resp.on_hover_text(name).clicked()
}

// ---------- settings rows ----------

/// An on/off switch: amber track when on, as every "selected" in the app. Returns the response;
/// `value` flips on click.
pub fn switch(ui: &mut Ui, value: &mut bool, name: &str) -> egui::Response {
    let c = p(ui);
    let on_enabled = ui.is_enabled();
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), if on_enabled { Sense::click() } else { Sense::hover() });
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *value, 0.12);
    let off_track = if resp.hovered() { c.text2.linear_multiply(0.5) } else { c.border };
    let track = lerp_color(off_track, c.amber, t);
    ui.painter().rect_filled(rect, 11.0, if on_enabled { track } else { track.linear_multiply(0.5) });
    let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, t);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 8.0, if *value { c.canvas } else { c.text });
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(3.0), 14.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
    }
    let (n, v) = (tr(name), *value);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, on_enabled, v, n.clone()));
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

/// A key as it is printed on the keyboard, for shortcut lists.
pub fn keycap(ui: &mut Ui, keys: &str) {
    let c = p(ui);
    let g = ui.painter().layout_no_wrap(keys.to_string(), mono_medium(12.0), c.text);
    let size = egui::vec2(g.size().x + 14.0, 24.0);
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(r, 5.0, c.field, Stroke::new(1.0, c.border), egui::StrokeKind::Inside);
    ui.painter().galley(r.center() - g.size() / 2.0, g, c.text);
}

/// A titled group of settings: the title, an optional line on what the group affects, then the
/// rows in one bordered panel, separated by hairlines. `add` draws the rows with `SettingRows`.
pub fn settings_group(ui: &mut Ui, title: &str, about: Option<&str>, add: impl FnOnce(&mut SettingRows)) {
    let c = p(ui);
    ui.label(RichText::new(tr(title)).font(medium(15.0)).color(c.text));
    if let Some(a) = about {
        ui.add_space(2.0);
        ui.label(mixed_job(&tr(a), 12.0, c.text2, sans(12.0), mono_font(12.0)));
    }
    ui.add_space(8.0);
    egui::Frame::new().fill(c.raised).stroke(Stroke::new(1.0, c.line)).corner_radius(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut rows = SettingRows { ui, n: 0 };
        add(&mut rows);
    });
    ui.add_space(28.0);
}

pub struct SettingRows<'a> {
    pub ui: &'a mut Ui,
    n: usize,
}

impl SettingRows<'_> {
    fn separator(&mut self) {
        if self.n > 0 {
            let c = p(self.ui);
            let (r, _) = self.ui.allocate_exact_size(egui::vec2(self.ui.available_width(), 1.0), Sense::hover());
            self.ui.painter().line_segment([egui::pos2(r.left() + 16.0, r.center().y), egui::pos2(r.right(), r.center().y)], Stroke::new(1.0, c.line));
        }
        self.n += 1;
    }

    /// Label and help on the left, the control on the right, both centred on the row.
    /// `ctrl_w` is the room the control needs.
    pub fn row(&mut self, label: &str, help: Option<&str>, ctrl_w: f32, control: impl FnOnce(&mut Ui)) {
        self.separator();
        let ui = &mut *self.ui;
        let c = p(ui);
        let (pad_x, pad_y) = (16.0, 12.0);
        let full = ui.available_width();
        let left = ui.cursor().left();
        let top = ui.cursor().top();
        let text_w = (full - ctrl_w - pad_x * 3.0).max(140.0);
        // the text is measured first, so a short label sits on the control's middle line
        let label_g = ui.painter().layout(tr(label), sans(14.0), c.text, text_w);
        let help_g = help.map(|h| {
            let mut job = mixed_job(&tr(h), 12.0, c.text2, sans(12.0), mono_font(12.0));
            job.wrap.max_width = text_w;
            ui.painter().layout_job(job)
        });
        let text_h = label_g.size().y + help_g.as_ref().map_or(0.0, |g| 2.0 + g.size().y);
        let h = (text_h.max(CTRL_H) + pad_y * 2.0).ceil();
        let row = egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(full, h));
        let ty = row.center().y - text_h / 2.0;
        let lh = label_g.size().y;
        ui.painter().galley(egui::pos2(left + pad_x, ty), label_g, c.text);
        if let Some(g) = help_g {
            ui.painter().galley(egui::pos2(left + pad_x, ty + lh + 2.0), g, c.text2);
        }
        let ctrl_rect = egui::Rect::from_min_size(egui::pos2(row.right() - pad_x - ctrl_w, row.center().y - CTRL_H / 2.0), egui::vec2(ctrl_w, CTRL_H));
        let mut ctrl_ui = ui.new_child(egui::UiBuilder::new().max_rect(ctrl_rect).layout(egui::Layout::right_to_left(egui::Align::Center)));
        control(&mut ctrl_ui);
        ui.allocate_rect(row, Sense::hover());
    }

    /// Label and help above, a control too wide for the right column below them.
    pub fn stacked(&mut self, label: &str, help: Option<&str>, control: impl FnOnce(&mut Ui)) {
        self.separator();
        let ui = &mut *self.ui;
        let c = p(ui);
        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add(egui::Label::new(RichText::new(tr(label)).font(sans(14.0)).color(c.text)).wrap());
            if let Some(h) = help {
                ui.add(egui::Label::new(mixed_job(&tr(h), 12.0, c.text2, sans(12.0), mono_font(12.0))).wrap());
            }
            ui.add_space(10.0);
            ui.spacing_mut().item_spacing.y = ROW_GAP;
            control(ui);
        });
    }

    /// Free content inside the panel, with the rows' padding (lists, status lines).
    pub fn block(&mut self, add: impl FnOnce(&mut Ui)) {
        self.separator();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12)).show(self.ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    }
}

/// Soft edges for a scrolling list: where content continues above or below the visible part,
/// a band fades from `bg` (the colour behind the list) to transparent, so rows dissolve into
/// the page instead of being cut by a hard line.
pub fn scroll_fades<R>(ui: &Ui, out: &egui::scroll_area::ScrollAreaOutput<R>, bg: Color32) {
    let rect = out.inner_rect;
    let offset = out.state.offset.y;
    let hidden_below = out.content_size.y - offset - rect.height();
    let h = 28.0f32.min(rect.height() / 4.0);
    if h < 4.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(rect);
    let band = |top: f32, from_top: bool, strength: f32| {
        let mut mesh = egui::Mesh::default();
        let solid = bg.gamma_multiply(strength);
        let clear = Color32::TRANSPARENT;
        let (c_top, c_bottom) = if from_top { (solid, clear) } else { (clear, solid) };
        let r = Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(rect.width(), h));
        mesh.colored_vertex(r.left_top(), c_top);
        mesh.colored_vertex(r.right_top(), c_top);
        mesh.colored_vertex(r.left_bottom(), c_bottom);
        mesh.colored_vertex(r.right_bottom(), c_bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);
        painter.add(egui::Shape::mesh(mesh));
    };
    // the band grows in with the first pixels scrolled, so it never pops in
    if offset > 0.5 {
        band(rect.top(), true, (offset / h).min(1.0));
    }
    if hidden_below > 0.5 {
        band(rect.bottom() - h, false, (hidden_below / h).min(1.0));
    }
}
