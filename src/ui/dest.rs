//! Destination setup and editing (Task E).
//!
//! New destination: Format → Project codes → Create (or "Set up codes later").
//! Existing destination: the same sections as tabs, plus Destination settings for rarely used
//! options (grouping, fallback code, export date format, copy line). From a "Needs code" row,
//! a contextual single-project code editor opens instead of the whole flow.
//! One dialog: steps replace the body; nothing opens a second modal. Header and footer stay
//! visible; only the body scrolls. Width ≤ 680 and ≤ viewport − 48.

use super::today::modal_frame;
use super::i18n::trf_m;
use super::i18n::tr;
use super::i18n::plural;
use super::widgets::*;
use super::App;
use devtrack_core::model::*;
use devtrack_core::report::{entries_in, render_value};
use devtrack_core::store::now_secs;
use devtrack_core::timefmt::round_secs;
use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Format,
    Codes,
    Settings,
    /// One project's codes, opened from a "Needs code" row.
    FixCode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Exception {
    pub task: Option<Id>,
    pub code: String,
    pub activity: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectCodes {
    pub project: Id,
    pub code: String,
    pub activity: String,
    pub exceptions: Vec<Exception>,
}

pub struct DestDraft {
    pub profile: TimesheetProfile,
    pub original: Option<TimesheetProfile>,
    pub step: Step,
    pub codes: Vec<ProjectCodes>,
    /// Mappings that don't belong to a project (kept untouched).
    pub other_mappings: Vec<CodeMapping>,
    pub fix_project: Option<Id>,
    pub attempted: bool,
    pub confirm_discard: bool,
    pub focused: bool,
    pub add_project: Option<Id>,
    pub errors: BTreeMap<String, String>,
    initial_codes: Vec<ProjectCodes>,
}

/// Projects used in the reporting period being viewed.
fn projects_in_period(app: &App) -> Vec<Id> {
    let mut v: Vec<Id> = vec![];
    for e in entries_in(&app.store.db, &app.rep.period, now_secs()) {
        if let Some(p) = &e.project_id {
            if app.store.db.project(&e.project_id).is_some() && !v.contains(p) {
                v.push(p.clone());
            }
        }
    }
    v
}

fn split_mappings(app: &App, pr: &TimesheetProfile, include: &[Id]) -> (Vec<ProjectCodes>, Vec<CodeMapping>) {
    let mut codes: Vec<ProjectCodes> = vec![];
    let mut other = vec![];
    let get = |codes: &mut Vec<ProjectCodes>, pid: &Id| -> usize {
        if let Some(i) = codes.iter().position(|c| &c.project == pid) {
            i
        } else {
            codes.push(ProjectCodes { project: pid.clone(), code: String::new(), activity: String::new(), exceptions: vec![] });
            codes.len() - 1
        }
    };
    for pid in include {
        get(&mut codes, pid);
    }
    for m in &pr.mappings {
        let task_project = app.store.db.task(&m.task_id).and_then(|t| t.project_id.clone());
        match (&m.task_id, m.project_id.clone().or(task_project)) {
            (None, Some(pid)) => {
                let i = get(&mut codes, &pid);
                codes[i].code = m.code.clone();
                codes[i].activity = m.activity.clone();
            }
            (Some(t), Some(pid)) => {
                let i = get(&mut codes, &pid);
                codes[i].exceptions.push(Exception { task: Some(t.clone()), code: m.code.clone(), activity: m.activity.clone() });
            }
            _ => other.push(m.clone()),
        }
    }
    (codes, other)
}

impl DestDraft {
    pub fn new_destination(app: &App) -> Self {
        let pr = TimesheetProfile::new("", "");
        let (codes, other) = split_mappings(app, &pr, &projects_in_period(app));
        DestDraft { profile: pr, original: None, step: Step::Format, initial_codes: codes.clone(), codes, other_mappings: other, fix_project: None, attempted: false, confirm_discard: false, focused: false, add_project: None, errors: BTreeMap::new() }
    }

    pub fn edit(app: &App, pr: &TimesheetProfile, step: Step) -> Self {
        let (codes, other) = split_mappings(app, pr, &projects_in_period(app));
        DestDraft { profile: pr.clone(), original: Some(pr.clone()), step, initial_codes: codes.clone(), codes, other_mappings: other, fix_project: None, attempted: false, confirm_discard: false, focused: false, add_project: None, errors: BTreeMap::new() }
    }

    pub fn fix_code(app: &App, pr: &TimesheetProfile, project: Option<Id>) -> Self {
        let include: Vec<Id> = project.iter().cloned().collect();
        let (codes, other) = split_mappings(app, pr, &include);
        DestDraft { profile: pr.clone(), original: Some(pr.clone()), step: Step::FixCode, initial_codes: codes.clone(), codes, other_mappings: other, fix_project: project, attempted: false, confirm_discard: false, focused: false, add_project: None, errors: BTreeMap::new() }
    }

    fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub fn dirty(&self) -> bool {
        match &self.original {
            Some(o) => *o != self.profile || self.codes != self.initial_codes,
            None => !self.profile.name.trim().is_empty() || self.codes != self.initial_codes,
        }
    }

    /// Mappings rebuilt from the per-project view; everything else in the profile is kept.
    fn build(&self) -> TimesheetProfile {
        let mut pr = self.profile.clone();
        let mut maps = self.other_mappings.clone();
        for pc in &self.codes {
            if !pc.code.trim().is_empty() || !pc.activity.trim().is_empty() {
                maps.push(CodeMapping { project_id: Some(pc.project.clone()), task_id: None, code: pc.code.trim().into(), activity: pc.activity.trim().into() });
            }
            for ex in &pc.exceptions {
                if let Some(t) = &ex.task {
                    maps.push(CodeMapping { project_id: Some(pc.project.clone()), task_id: Some(t.clone()), code: ex.code.trim().into(), activity: ex.activity.trim().into() });
                }
            }
        }
        pr.mappings = maps;
        pr
    }

    /// Validate on submission. `codes_required`: every listed project needs a code (or a
    /// default code exists). Returns true when valid.
    fn validate(&mut self, codes_required: bool, only: Option<&Id>) -> bool {
        self.errors.clear();
        if self.profile.name.trim().is_empty() {
            self.errors.insert("name".into(), "Give this destination a name".into());
        }
        if self.profile.round_scope != RoundScope::None && self.profile.round_step_min == 0 {
            self.errors.insert("rounding".into(), "Choose a rounding step".into());
        }
        for pc in &self.codes {
            if only.map(|o| o != &pc.project).unwrap_or(false) {
                continue;
            }
            if codes_required && pc.code.trim().is_empty() && self.profile.default_code.trim().is_empty() {
                self.errors.insert(format!("code:{}", pc.project), "Add a code, or choose “Set up codes later”".into());
            }
            for (i, ex) in pc.exceptions.iter().enumerate() {
                if ex.task.is_none() {
                    self.errors.insert(format!("ex-task:{}:{}", pc.project, i), "Choose a task, or remove this exception".into());
                }
                if ex.code.trim().is_empty() {
                    self.errors.insert(format!("ex-code:{}:{}", pc.project, i), "Add a code for this task".into());
                }
            }
        }
        self.errors.is_empty()
    }
}

fn example_value(pr: &TimesheetProfile, secs: i64) -> String {
    let r = if pr.rounds() { round_secs(secs, pr.round_step_min, pr.round_dir) } else { secs };
    render_value(pr, r)
}

pub fn grouping_text(g: Grouping) -> &'static str {
    match g {
        Grouping::DayCode => "one line per day and code",
        Grouping::DayProject => "one line per day, project and code",
        Grouping::DayTask => "one line per day, task and code",
        Grouping::WeekCode => "one line per week and code",
    }
}

pub fn modal(app: &mut App, ctx: &egui::Context) {
    let Some(mut d) = app.dest.take() else { return };
    let dark = app.dark();
    let c = pal(dark);
    let screen = ctx.screen_rect();
    // outer width (content + 24 padding each side) stays within 680 and the viewport minus 48
    let w = (680.0f32 - 48.0).min(screen.width() - 48.0 - 48.0).max(260.0);
    let max_h = screen.height() - 48.0;
    let mut close = false;
    let mut saved: Option<TimesheetProfile> = None;
    let mut x_close = false;
    let modal = egui::Modal::new(egui::Id::new("destination")).frame(modal_frame(&c)).backdrop_color(Color32::from_black_alpha(if dark { 160 } else { 90 })).show(ctx, |ui| {
        ui.set_width(w);
        x_close = modal_close(ui);
        // ---- header (fixed) ----
        let title_txt = match (d.step, d.is_new()) {
            (Step::FixCode, _) => tr("Add a code"),
            (_, true) => tr("New destination"),
            (_, false) => trf_m!("Edit “{}”", truncate(&d.profile.name, 40)),
        };
        ui.label(RichText::new(title_txt).font(semibold(20.0)));
        if d.is_new() && d.step != Step::FixCode {
            let n = if d.step == Step::Format { 1 } else { 2 };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                meta(ui, "Step");
                mixed(ui, &trf_m!("{} of 2", n), 12.0, Some(c.text2));
                meta(ui, if n == 1 { "· Format" } else { "· Project codes" });
            });
        } else if d.step != Step::FixCode {
            ui.add_space(4.0);
            let cur = match d.step {
                Step::Format => 0,
                Step::Codes => 1,
                _ => 2,
            };
            if let Some(i) = segmented(ui, &["Format", "Project codes", "Destination settings"], cur) {
                d.step = [Step::Format, Step::Codes, Step::Settings][i];
            }
        }
        ui.add_space(8.0);
        divider(ui);
        // ---- body (scrolls) ----
        let body_h = (max_h - 24.0 * 2.0 - 70.0 - 76.0).max(160.0);
        egui::ScrollArea::vertical().id_salt(("dest-body", d.step as u8)).max_height(body_h).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(w);
            ui.add_space(GROUP_GAP);
            match d.step {
                Step::Format => format_step(app, ui, &mut d, w),
                Step::Codes => codes_step(app, ui, &mut d, w, None),
                Step::Settings => settings_step(ui, &mut d, w),
                Step::FixCode => {
                    let pid = d.fix_project.clone();
                    // what the code is for, in the destination's own name, before asking for it
                    let dest = truncate(&d.profile.name, 48);
                    ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
                        ui.set_max_width(w);
                        muted(ui, trf_m!("{} records hours by code. Write the code this project has in {}: the job or project number, as it is written there. Every line to enter for this project will carry it.", dest, dest));
                    });
                    ui.add_space(ROW_GAP);
                    match pid {
                        Some(pid) => codes_step(app, ui, &mut d, w, Some(&pid)),
                        None => {
                            // sessions without a project can only use the fallback code
                            muted(ui, "These sessions have no project, so they use this destination's fallback code. You can also give the sessions a project on Today.");
                            ui.add_space(ROW_GAP);
                            let err = if d.attempted { d.errors.get("fallback").cloned() } else { None };
                            field(ui, "Fallback code", None, err.as_deref(), |ui| {
                                input(ui, &mut d.profile.default_code, Field::new("Fallback code", w).mono().hint("e.g. GEN-000").invalid(err.is_some()));
                            });
                        }
                    }
                }
            }
            ui.add_space(8.0);
        });
        divider(ui);
        ui.add_space(12.0);
        // ---- footer (fixed) ----
        if d.confirm_discard {
            row(ui, CTRL_H, |ui| {
                ui.label(tr("Discard your changes?"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if danger(ui, "Discard").clicked() {
                        close = true;
                    }
                    if secondary(ui, "Keep editing").clicked() {
                        d.confirm_discard = false;
                    }
                });
            });
            return;
        }
        ui.horizontal(|ui| {
            match (d.step, d.is_new()) {
                (Step::Format, true) => {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary(ui, "Next: project codes").clicked() {
                            d.attempted = true;
                            d.errors.clear();
                            if d.profile.name.trim().is_empty() {
                                d.errors.insert("name".into(), "Give this destination a name".into());
                            } else {
                                d.step = Step::Codes;
                            }
                        }
                        if secondary(ui, "Cancel").clicked() {
                            if d.dirty() {
                                d.confirm_discard = true;
                            } else {
                                close = true;
                            }
                        }
                    });
                }
                (Step::Codes, true) => {
                    if secondary(ui, "Back").clicked() {
                        d.step = Step::Format;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary(ui, "Create destination").clicked() {
                            d.attempted = true;
                            if d.validate(true, None) {
                                saved = Some(d.build());
                            }
                        }
                        if ghost(ui, "Set up codes later").on_hover_text(tr("Save now; rows without a code show “Needs code”")).clicked() {
                            d.attempted = true;
                            if d.validate(false, None) {
                                saved = Some(d.build());
                            }
                        }
                    });
                }
                (Step::FixCode, _) => {
                    let only = d.fix_project.clone();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary(ui, "Save code").clicked() {
                            d.attempted = true;
                            let ok = match &only {
                                Some(pid) => d.validate(true, Some(pid)),
                                None => {
                                    d.errors.clear();
                                    if d.profile.default_code.trim().is_empty() {
                                        d.errors.insert("fallback".into(), "Add a fallback code".into());
                                    }
                                    d.errors.is_empty()
                                }
                            };
                            if ok {
                                saved = Some(d.build());
                            }
                        }
                        if secondary(ui, "Cancel").clicked() {
                            if d.dirty() {
                                d.confirm_discard = true;
                            } else {
                                close = true;
                            }
                        }
                    });
                }
                _ => {
                    // editing an existing destination
                    let mut delete = false;
                    if app.rep.confirm_delete {
                        if danger(ui, "Delete destination").clicked() {
                            delete = true;
                        }
                        if ghost(ui, "Keep").clicked() {
                            app.rep.confirm_delete = false;
                        }
                    } else if danger(ui, "Delete…").clicked() {
                        app.rep.confirm_delete = true;
                    }
                    if delete {
                        let mut x = d.profile.clone();
                        x.deleted = true;
                        app.store.put_profile(x);
                        app.rep.confirm_delete = false;
                        close = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary(ui, "Save").clicked() {
                            d.attempted = true;
                            if d.validate(false, None) {
                                saved = Some(d.build());
                            }
                        }
                        if secondary(ui, "Cancel").clicked() {
                            if d.dirty() {
                                d.confirm_discard = true;
                            } else {
                                close = true;
                            }
                        }
                    });
                }
            }
        });
        if !d.errors.is_empty() && d.attempted {
            let n = d.errors.len();
            ui.label(RichText::new(plural(n as i64, "{} field needs attention", "{} fields need attention")).size(12.0).color(c.red));
        }
    });
    if (modal.should_close() || x_close) && !close && saved.is_none() {
        if d.dirty() {
            d.confirm_discard = true;
        } else {
            close = true;
        }
    }
    if let Some(pr) = saved {
        app.rep.hidden.remove(&pr.id);
        let fix = d.step == Step::FixCode;
        app.store.put_profile(pr);
        app.notify(if fix { "Code saved" } else { "Destination saved" });
        close = true;
    }
    if close {
        app.restore_focus(ctx);
    } else {
        app.dest = Some(d);
    }
}

fn format_step(_app: &mut App, ui: &mut egui::Ui, d: &mut DestDraft, w: f32) {
    let c = p(ui);
    if d.is_new() {
        // what a destination is, before asking for one
        ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
            ui.set_max_width(w);
            muted(ui, "A destination is a platform where you report your hours, such as the company timesheet. Here you set how it writes time; next, the code each project has in it. The same platform appears in Settings › Platforms, and its conversion sits next to the time on Today and in projects.");
        });
        ui.add_space(GROUP_GAP);
    }
    let name_err = if d.attempted { d.errors.get("name").cloned() } else { None };
    let r = field(ui, "Destination name", None, name_err.as_deref(), |ui| {
        input(ui, &mut d.profile.name, Field::new("Destination name", w).hint("e.g. Company timesheet").invalid(name_err.is_some()))
    });
    if !d.focused {
        r.request_focus();
        d.focused = true;
    }
    ui.add_space(GROUP_GAP - ROW_GAP);
    conversion_editor(ui, &mut d.profile, "dest", w);
    if let Some(e) = d.errors.get("rounding") {
        ui.label(RichText::new(e).size(12.0).color(c.red));
    }
}

/// How a platform writes time: its format, decimals and rounding, with a live example. The one
/// editor for both places a platform is set up, the new-destination dialog in Report Time and
/// Settings › Platforms, so the two never drift apart.
pub fn conversion_editor(ui: &mut egui::Ui, pr: &mut TimesheetProfile, salt: &str, w: f32) {
    let c = p(ui);
    label(ui, "How time is written");
    ui.add_space(LABEL_GAP - ROW_GAP);
    let card_w = (w - ROW_GAP * 2.0) / 3.0;
    ui.horizontal(|ui| {
        for f in ReportFormat::ALL {
            let mut tmp = pr.clone();
            tmp.format = f;
            tmp.round_scope = RoundScope::None;
            let ex = render_value(&tmp, 97 * 60);
            let (name, sub) = match f {
                ReportFormat::HoursMinutes => ("Hours:minutes", "h:mm"),
                ReportFormat::DecimalHours => ("Decimal hours", "hours with decimals"),
                ReportFormat::Minutes => ("Minutes", "whole minutes"),
            };
            let sel = pr.format == f;
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, 76.0), Sense::click());
            ui.painter().rect(rect, 8.0, if sel { c.selected } else if resp.hovered() { c.hover } else { c.field }, Stroke::new(if sel { 1.5 } else { 1.0 }, if sel { c.text } else { c.line }), egui::StrokeKind::Inside);
            ui.painter().text(rect.left_top() + egui::vec2(12.0, 10.0), egui::Align2::LEFT_TOP, tr(name), medium(14.0), c.text);
            ui.painter().text(rect.left_top() + egui::vec2(12.0, 32.0), egui::Align2::LEFT_TOP, ex, mono_medium(20.0), c.text);
            ui.painter().text(rect.left_top() + egui::vec2(12.0, 56.0), egui::Align2::LEFT_TOP, tr(sub), sans(12.0), c.text2);
            if resp.has_focus() {
                ui.painter().rect_stroke(rect.expand(2.0), 10.0, Stroke::new(2.0, c.focus), egui::StrokeKind::Inside);
            }
            let n = name.to_string();
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, sel, n.clone()));
            if resp.clicked() {
                pr.format = f;
            }
        }
    });
    if pr.format == ReportFormat::DecimalHours {
        ui.add_space(GROUP_GAP - ROW_GAP);
        ui.horizontal_top(|ui| {
            field(ui, "Decimals", None, None, |ui| {
                if let Some(i) = segmented(ui, &["1", "2"], if pr.decimal_places == 1 { 0 } else { 1 }) {
                    pr.decimal_places = if i == 0 { 1 } else { 2 };
                }
            });
            ui.add_space(16.0);
            field(ui, "Decimal separator", None, None, |ui| {
                if let Some(i) = segmented(ui, &["1,50", "1.50"], if pr.decimal_comma { 0 } else { 1 }) {
                    pr.decimal_comma = i == 0;
                }
            });
        });
    }
    // rounding: one checkbox, and its step, direction and scope beside it once it is on
    ui.add_space(GROUP_GAP - ROW_GAP);
    row_wrapped(ui, CTRL_H, |ui| {
        ui.spacing_mut().item_spacing.x = GROUP_GAP;
        let mut on = pr.round_scope != RoundScope::None;
        if checkbox(ui, &mut on, tr("Round")).changed() {
            pr.round_scope = if on { RoundScope::PerGroup } else { RoundScope::None };
            if pr.round_step_min == 0 { pr.round_step_min = 15; }
        }
        if on {
            let steps: Vec<(u32, String)> = [1u32, 5, 6, 10, 15, 30, 60].iter().map(|m| (*m, trf_m!("to {} min", m))).collect();
            select(ui, (salt, "step"), &mut pr.round_step_min, &steps, 100.0, false);
            let dirs: Vec<(RoundDir, String)> = RoundDir::ALL.iter().map(|d| (*d, tr(d.label()))).collect();
            select(ui, (salt, "dir"), &mut pr.round_dir, &dirs, 150.0, false);
            let scopes: Vec<(RoundScope, String)> = [RoundScope::PerGroup, RoundScope::PerEntry].iter().map(|s| (*s, tr(s.label()))).collect();
            select(ui, (salt, "scope"), &mut pr.round_scope, &scopes, 200.0, false);
        }
    });
    meta(ui, "Rounding changes the reported time. Decimals only change how a value is written.");
    // single live example
    ui.add_space(GROUP_GAP - ROW_GAP);
    ui.horizontal(|ui| {
        meta(ui, "Example:");
        mixed(ui, "1 h 37 min", 14.0, None);
        meta(ui, "becomes");
        ui.label(mono_m(example_value(pr, 97 * 60), 16.0));
        let r = if pr.rounds() { round_secs(97 * 60, pr.round_step_min, pr.round_dir) - 97 * 60 } else { 0 };
        if r != 0 {
            mixed(ui, &format!("{}{}", if r > 0 { "+" } else { "-" }, words(r.abs())), 12.0, Some(c.text2));
        }
    });
}

fn codes_step(app: &mut App, ui: &mut egui::Ui, d: &mut DestDraft, w: f32, only: Option<&Id>) {
    let c = p(ui);
    let rp = recent_projects(&app.store.db);
    if only.is_none() {
        muted(ui, "Each project needs the code it has in this destination (the job or project number, as it is written there), so its hours land in the right place. These are the projects you worked on in this period.");
        ui.add_space(8.0);
    }
    if !d.profile.default_code.trim().is_empty() && only.is_none() {
        ui.horizontal(|ui| {
            meta(ui, "Projects without a code use the fallback");
            ui.label(mono(&d.profile.default_code, 12.0).color(c.text2));
        });
    }
    let shown: Vec<usize> = d.codes.iter().enumerate().filter(|(_, pc)| only.map(|o| o == &pc.project).unwrap_or(true)).map(|(i, _)| i).collect();
    if shown.is_empty() {
        panel(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tr("No projects in this period")).font(medium(14.0)));
            meta(ui, "Add a project below, or set up codes later from Report Time.");
        });
    }
    let code_w = ((w - ROW_GAP) * 0.45).max(120.0);
    let act_w = (w - code_w - ROW_GAP).max(120.0);
    for i in shown {
        let pid = d.codes[i].project.clone();
        let pname = app.store.db.projects.get(&pid).map(|p| p.name.clone()).unwrap_or("Removed project".into());
        let pcol = app.store.db.projects.get(&pid).map(|p| hex(&p.color)).unwrap_or(c.text2);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            dot(ui, pcol, 4.0);
            ui.add(egui::Label::new(RichText::new(&pname).font(medium(16.0))).wrap()).on_hover_text(&pname);
        });
        let ce = if d.attempted { d.errors.get(&format!("code:{}", pid)).cloned() } else { None };
        ui.horizontal_top(|ui| {
            field(ui, "Code", None, ce.as_deref(), |ui| {
                input(ui, &mut d.codes[i].code, Field::new("Code", code_w).mono().hint("e.g. P-104").invalid(ce.is_some()));
            });
            field(ui, "Activity (optional)", None, None, |ui| {
                input(ui, &mut d.codes[i].activity, Field::new("Activity", act_w).hint("e.g. Development"));
            });
        });
        // task exceptions
        let tasks: Vec<(Option<Id>, String)> = std::iter::once((None, tr("Choose task")))
            .chain(app.store.db.live_tasks().filter(|t| t.project_id.as_ref() == Some(&pid)).map(|t| (Some(t.id.clone()), t.display())))
            .collect();
        let mut remove = None;
        // exception rows span the same width as the code row above, from the same left edge
        let ex_room = w - ROW_GAP * 3.0 - CTRL_H;
        let (ex_task_w, ex_code_w, ex_act_w) = (ex_room * 0.45, ex_room * 0.25, ex_room * 0.30);
        if !d.codes[i].exceptions.is_empty() {
            // said once, between the project's code and its first exception
            ui.add_space(ROW_GAP);
            ui.allocate_ui(egui::vec2(w, 0.0), |ui| {
                ui.set_max_width(w);
                meta(ui, trf_m!("Exceptions: a task of {} that goes on a different code or activity. Its hours use the exception; the rest of the project uses the code above.", pname));
            });
        }
        for j in 0..d.codes[i].exceptions.len() {
            let te = if d.attempted { d.errors.get(&format!("ex-task:{}:{}", pid, j)).cloned() } else { None };
            let ke = if d.attempted { d.errors.get(&format!("ex-code:{}:{}", pid, j)).cloned() } else { None };
            ui.horizontal_top(|ui| {
                field(ui, "Task exception", None, te.as_deref(), |ui| {
                    let mut v = d.codes[i].exceptions[j].task.clone();
                    if select(ui, ("ex-task", &pid, j), &mut v, &tasks, ex_task_w, false) {
                        d.codes[i].exceptions[j].task = v;
                    }
                });
                field(ui, "Code", None, ke.as_deref(), |ui| {
                    input(ui, &mut d.codes[i].exceptions[j].code, Field::new("Exception code", ex_code_w).mono().invalid(ke.is_some()));
                });
                field(ui, "Activity", None, None, |ui| {
                    input(ui, &mut d.codes[i].exceptions[j].activity, Field::new("Exception activity", ex_act_w));
                });
                field(ui, " ", None, None, |ui| {
                    if icon_button(ui, Glyph::Close, "Remove exception").clicked() {
                        remove = Some(j);
                    }
                });
            });
        }
        if let Some(j) = remove {
            d.codes[i].exceptions.remove(j);
        }
        ui.add_space(4.0);
        if tasks.len() > 1 {
            ui.horizontal(|ui| {
                if link_btn(ui, "+ Add task exception").clicked() {
                    d.codes[i].exceptions.push(Exception { task: None, code: String::new(), activity: String::new() });
                }
            });
        } else {
            // nothing to choose: say why the option is missing instead of offering an empty list
            meta(ui, "This project has no tasks, so all its hours use the code above.");
        }
        ui.add_space(4.0);
        // a single project (the "Add a code" dialog) ends on the footer's own line
        if only.is_none() {
            divider(ui);
        }
    }
    if only.is_none() {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let mut add = d.add_project.clone();
            let pick = field(ui, "Add another project", None, None, |ui| project_picker(ui, "dest-add-project", &app.store.db, &rp, &mut add, 260.0));
            if pick == Pick::Changed {
                if let Some(pid) = add {
                    if !d.codes.iter().any(|pc| pc.project == pid) {
                        d.codes.push(ProjectCodes { project: pid, code: String::new(), activity: String::new(), exceptions: vec![] });
                    }
                }
            }
            d.add_project = None;
        });
    }
}

fn settings_step(ui: &mut egui::Ui, d: &mut DestDraft, w: f32) {
    let c = p(ui);
    let half = (w - ROW_GAP) / 2.0;
    ui.horizontal_top(|ui| {
        field(ui, "Lines", None, None, |ui| {
            let opts: Vec<(Grouping, String)> = Grouping::ALL.iter().map(|g| (*g, tr(g.label()))).collect();
            select(ui, "dest-group", &mut d.profile.grouping, &opts, half, false);
        });
        field(ui, "Fallback code", Some("Used when a project has no code. Empty: every project needs its own code."), None, |ui| {
            input(ui, &mut d.profile.default_code, Field::new("Fallback code", half).mono().hint("None"));
        });
    });
    ui.add_space(GROUP_GAP - ROW_GAP);
    ui.horizontal_top(|ui| {
        field(ui, "Date format in copied lines", None, None, |ui| {
            let opts: Vec<(String, String)> = [("%d/%m/%Y", "31/12/2026"), ("%Y-%m-%d", "2026-12-31"), ("%d.%m.%Y", "31.12.2026"), ("%m/%d/%Y", "12/31/2026")].iter().map(|(f, l)| (f.to_string(), l.to_string())).collect();
            select(ui, "dest-date", &mut d.profile.date_format, &opts, half, true);
        });
    });
    ui.add_space(GROUP_GAP - ROW_GAP);
    field(ui, "Line for a spreadsheet", Some("For pasting into Excel or another spreadsheet: {date} {code} {activity} {project} {task} {value} {actual}; each tab starts a new column"), None, |ui| {
        input(ui, &mut d.profile.line_template, Field::new("Line for a spreadsheet", w).mono());
    });
    ui.add_space(GROUP_GAP);
    // plain-language summary and a sample copied line
    let rounding = if d.profile.rounds() { trf_m!("rounded {} to {} min", tr(d.profile.round_dir.label()).to_lowercase(), d.profile.round_step_min) } else { tr("not rounded") };
    panel(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, "In short");
        ui.label(trf_m!("{}; values {}, {}.", tr(grouping_text(d.profile.grouping)), tr(&d.profile.precision_text()), rounding));
        ui.add_space(6.0);
        label(ui, "Sample copied line");
        let sample = d
            .profile
            .line_template
            .replace("{date}", &chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap().format(&d.profile.date_format).to_string())
            .replace("{code}", "P-104")
            .replace("{activity}", "Development")
            .replace("{project}", "Platform API")
            .replace("{task}", "#4821 Implement token refresh")
            .replace("{value}", &example_value(&d.profile, 3 * 3600 + 37 * 60))
            .replace("{actual}", &render_value(&d.profile, 3 * 3600 + 37 * 60))
            .replace('\t', "  →  ");
        ui.label(mono(sample, 13.0).color(c.text));
    });
}
