// SPDX-License-Identifier: AGPL-3.0-or-later
//! A setup's workholding and retaining tabs (SET-09, OP-21; UI-03, UI-07,
//! UI-25; work breakdown slices 21c and 21d; decision records 0045 and 0104;
//! interface specification section 11.7).
//!
//! The panel edits a draft of the setup's held regions and tabs
//! ([`Draft`]). The draft is checked as it changes, with the reason under the
//! field it marks (UI-07), and applied with one undoable edit of the setup
//! (UI-08). The checks that need the part and the tools come from a job. The
//! default tabs are computed from the part as a job and shown as defaults
//! until they are added to the draft.

use eframe::egui::{self, Color32, RichText};
use openchuck_app::project::{Item, Setup};
use openchuck_app::setup::{HeldRegion, Millimetres, Stock, Tab, TabLayout, TabShape, Workholding};
use openchuck_app::workholding_view::{
    DefaultTabsState, Draft, End, Field, FieldProblem, NO_TABS_WARNING, check_draft, default_chips,
    default_tabs_state, held_span, narrowest, new_held_region, new_tab, set_layout, set_shape,
};
use openchuck_app::{Application, Command, JobHandle};

const ERROR: Color32 = Color32::from_rgb(255, 123, 114);
const NOTE: Color32 = Color32::from_rgb(230, 180, 60);

/// The workholding panel's state: the draft being edited.
pub struct WorkholdingPanel {
    setup: openchuck_app::project::id::SetupId,
    draft: Draft,
    /// The text of each holding-diameter field (empty for none).
    diameter_text: (String, String),
    /// The default tabs: being computed, or what the job found.
    defaults: DefaultTabs,
    /// A check against the part and tools that is running.
    checking: Option<Checking>,
    /// What the last check of the setup said.
    findings: Vec<String>,
    message: Option<String>,
    /// Set for one frame to focus a tab's first field.
    focus_new_tab: bool,
}

/// The default tabs of the stored held regions (11.7): worked out as a job
/// when the setup has no tabs, kept for the chips after they are added.
enum DefaultTabs {
    None,
    Running {
        job: JobHandle<Vec<Tab>>,
        key: String,
    },
    Found {
        tabs: Vec<Tab>,
        key: String,
    },
    Failed {
        message: String,
        key: String,
    },
}

/// A running check of a draft against the part and the tools. It reads the
/// part, so it is a job, not a call in the frame.
struct Checking {
    job: JobHandle<Vec<String>>,
    /// True to store the draft if the check finds nothing.
    apply: bool,
    /// The draft as it was when the check started.
    draft: Workholding,
    /// What the check reads besides the draft, as it was then.
    inputs: String,
}

/// What a check of `setup`'s workholding depends on besides the draft: its
/// part and stock, and the tools of its operations. A check made on other
/// inputs is stale.
fn check_inputs(app: &Application, setup: &Setup) -> String {
    let project = app.project();
    let tools: Vec<_> = project
        .operations()
        .iter()
        .filter(|o| o.setup == setup.id)
        .map(|o| {
            let tool = o
                .tool
                .and_then(|id| project.tools().iter().find(|t| t.id == id));
            (o.id, tool.map(|t| &t.tool))
        })
        .collect();
    format!("{:?} {:?} {:?}", setup.part, setup.stock, tools)
}

/// What the default tabs depend on: the part, the stock and the stored held
/// regions.
fn defaults_key(setup: &Setup) -> String {
    format!(
        "{:?} {:?} {:?} {:?}",
        setup.part, setup.stock, setup.workholding.start, setup.workholding.end
    )
}

impl std::fmt::Debug for WorkholdingPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkholdingPanel")
            .field("setup", &self.setup)
            .finish_non_exhaustive()
    }
}

/// The text a diameter field shows for `region`.
fn diameter_string(region: &Option<HeldRegion>) -> String {
    region
        .and_then(|r| r.diameter)
        .map_or_else(String::new, |d| format!("{:.3}", d.0))
}

impl WorkholdingPanel {
    /// A panel for `setup`, starting from its stored workholding.
    pub fn new(setup: &Setup) -> Self {
        let draft = Draft::new(&setup.workholding);
        Self {
            setup: setup.id,
            diameter_text: (
                diameter_string(&setup.workholding.start),
                diameter_string(&setup.workholding.end),
            ),
            draft,
            defaults: DefaultTabs::None,
            checking: None,
            findings: Vec::new(),
            message: None,
            focus_new_tab: false,
        }
    }

    /// The setup the panel is for.
    pub fn setup(&self) -> openchuck_app::project::id::SetupId {
        self.setup
    }

    /// True while the draft has changes that are not applied (the footer says
    /// so, 11.7).
    pub fn has_unapplied_changes(&self) -> bool {
        self.draft.is_changed()
    }

    /// Draws the panel. Returns the command to execute when the user applies
    /// the draft.
    pub fn show(&mut self, ui: &mut egui::Ui, app: &Application, setup: &Setup) -> Option<Command> {
        // Another edit (an undo, the command line) changed the project: start
        // again from what it holds, saying so if that threw changes away.
        if *self.draft.stored() != setup.workholding {
            self.draft.sync(&setup.workholding);
            self.findings.clear();
            self.message = None;
        }
        self.poll_defaults(ui.ctx(), app, setup);

        let held = usize::from(setup.workholding.start.is_some())
            + usize::from(setup.workholding.end.is_some());
        let tabs = setup.workholding.tabs.len();
        ui.horizontal(|ui| {
            ui.heading("Workholding and tabs");
            let count = if held + tabs == 0 {
                "None".to_owned()
            } else {
                format!("{held} held \u{b7} {tabs} tabs")
            };
            ui.label(RichText::new(count).weak());
            if self.draft.is_changed() {
                ui.label(RichText::new("Not applied").color(NOTE));
            }
        });
        if let Some(note) = self.draft.note() {
            ui.label(RichText::new(note).color(NOTE));
        }
        ui.weak(
            "Held regions are never machined and links keep clear of them. Tabs keep the part \
             joined to the stock. Both count as part of the finished surface.",
        );
        if setup.stock.is_none() {
            ui.label(
                RichText::new("Set the stock first: held regions and tabs are placed on it.")
                    .color(NOTE),
            );
        }
        ui.separator();

        let problem = check_draft(self.draft.workholding(), setup.stock.as_ref());
        self.held_block(ui, setup, End::Start, problem.as_ref());
        self.held_block(ui, setup, End::End, problem.as_ref());
        ui.separator();
        self.tab_list(ui, setup, problem.as_ref());
        ui.separator();

        if let Some(m) = &self.message {
            ui.label(RichText::new(m).color(ERROR));
        }
        let command = self.buttons(ui, app, setup, problem.as_ref());
        for f in &self.findings {
            ui.label(RichText::new(f).color(NOTE));
        }
        // After the controls, so a draft edited or reverted this frame is
        // the draft the finished check is compared with.
        command.or_else(|| self.poll_check(ui.ctx(), app, setup))
    }

    /// One held region: a check box, the grip length, an optional holding
    /// diameter (empty for the stock's own) and the span (11.7).
    fn held_block(
        &mut self,
        ui: &mut egui::Ui,
        setup: &Setup,
        end: End,
        problem: Option<&FieldProblem>,
    ) {
        let label = match end {
            End::Start => "Held at the start",
            End::End => "Held at the end",
        };
        let current = match end {
            End::Start => self.draft.workholding().start,
            End::End => self.draft.workholding().end,
        };
        let mut on = current.is_some();
        if ui.checkbox(&mut on, label).changed() {
            let edit = self.draft.edit();
            let region = match end {
                End::Start => &mut edit.start,
                End::End => &mut edit.end,
            };
            *region = on.then(new_held_region);
        }
        let Some(mut region) = current.filter(|_| on) else {
            return;
        };
        ui.indent(label, |ui| {
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.add_sized([120.0, 18.0], egui::Label::new("Grip length"));
                changed |= mm_field(
                    ui,
                    &mut region.length.0,
                    problem_is(problem, Field::Grip(end)),
                );
            });
            show_problem(ui, problem, Field::Grip(end));
            let stock_diameter = stock_diameter_hint(setup.stock.as_ref());
            // Keep the text in step with the draft when it changed elsewhere
            // (Revert, an undo).
            let shown = diameter_string(&current);
            let text = text_of(&mut self.diameter_text, end).trim();
            let in_step = match current.and_then(|r| r.diameter) {
                None => text.is_empty(),
                Some(d) => {
                    d.0.is_nan() || text.parse::<f64>().is_ok_and(|v| (v - d.0).abs() < 1e-9)
                }
            };
            if !in_step {
                *text_of(&mut self.diameter_text, end) = shown;
            }
            ui.horizontal(|ui| {
                ui.add_sized([120.0, 18.0], egui::Label::new("Holding diameter"));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut *text_of(&mut self.diameter_text, end))
                        .desired_width(90.0)
                        .hint_text(stock_diameter),
                );
                ui.label("mm");
                if response.changed() {
                    let text = text_of(&mut self.diameter_text, end).trim();
                    region.diameter = if text.is_empty() {
                        None
                    } else {
                        Some(Millimetres(text.parse::<f64>().unwrap_or(f64::NAN)))
                    };
                    changed = true;
                }
            });
            show_problem(ui, problem, Field::HoldingDiameter(end));
            if let Some(stock) = &setup.stock
                && let Some(span) = held_span(self.draft.workholding(), end, stock)
            {
                ui.label(RichText::new(format!("Held span: {span} mm")).weak());
            }
            if changed {
                let edit = self.draft.edit();
                match end {
                    End::Start => edit.start = Some(region),
                    End::End => edit.end = Some(region),
                }
            }
        });
    }

    /// The tab rows, the default tabs offered as defaults, and the buttons
    /// under them.
    fn tab_list(&mut self, ui: &mut egui::Ui, setup: &Setup, problem: Option<&FieldProblem>) {
        ui.strong(format!(
            "Retaining tabs ({})",
            self.draft.workholding().tabs.len()
        ));
        let offered: Vec<Tab> = match &self.defaults {
            DefaultTabs::Found { tabs, .. } => tabs.clone(),
            _ => Vec::new(),
        };
        let chips = default_chips(&self.draft.workholding().tabs, &offered);
        let mut remove = None;
        let mut duplicate = None;
        let mut edited: Option<(usize, Tab)> = None;
        let focus = std::mem::take(&mut self.focus_new_tab);
        let last = self.draft.workholding().tabs.len().saturating_sub(1);
        for (i, original) in self.draft.workholding().tabs.iter().enumerate() {
            let mut tab = *original;
            let header = format!(
                "Tab {} \u{b7} {} \u{b7} X {:.3} to {:.3}{}",
                i + 1,
                if matches!(tab.shape, TabShape::Cylinder { .. }) {
                    "Cylinder"
                } else {
                    "Frustum"
                },
                tab.x_start.0,
                tab.x_start.0 + tab.length.0,
                if chips.get(i).copied().unwrap_or(false) {
                    " \u{b7} Default"
                } else {
                    ""
                },
            );
            let marked = problem.is_some_and(|p| field_tab(p.field) == Some(i));
            let response = egui::CollapsingHeader::new(RichText::new(header).color(if marked {
                ERROR
            } else {
                ui.visuals().text_color()
            }))
            .id_salt(("tab", i))
            .default_open(focus && i == last || marked)
            .show(ui, |ui| {
                let mut action = None;
                if tab_editor(ui, i, &mut tab, problem, setup.stock.as_ref()) {
                    action = Some(false);
                }
                ui.horizontal(|ui| {
                    if ui.small_button("Duplicate").clicked() {
                        action = Some(true);
                    }
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
                action
            });
            if response.header_response.has_focus()
                && ui.input(|k| k.key_pressed(egui::Key::Delete))
            {
                remove = Some(i);
            }
            if let Some(Some(duplicated)) = response.body_returned
                && duplicated
            {
                duplicate = Some(i);
            }
            if tab != *original {
                edited = Some((i, tab));
            }
        }
        // The default tabs, offered while the setup has none (11.7).
        if setup.workholding.tabs.is_empty() && self.draft.workholding().tabs.is_empty() {
            match &self.defaults {
                DefaultTabs::Running { .. } => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("Working out from the part\u{2026}");
                    });
                }
                DefaultTabs::Found { tabs, .. } if !tabs.is_empty() => {
                    for (i, tab) in tabs.iter().enumerate() {
                        ui.add_enabled_ui(false, |ui| {
                            ui.label(format!(
                                "Default tab, not added \u{b7} {} \u{b7} X {:.3} to {:.3} \u{b7} \u{d8} {:.3}{}",
                                i + 1,
                                tab.x_start.0,
                                tab.x_start.0 + tab.length.0,
                                narrowest(tab),
                                if matches!(tab.shape, TabShape::Frustum { .. }) { " (frustum)" } else { "" },
                            ));
                        });
                    }
                    ui.label(RichText::new(NO_TABS_WARNING).color(NOTE));
                }
                DefaultTabs::Found { .. } => {
                    ui.label(RichText::new(NO_TABS_WARNING).color(NOTE));
                    ui.weak("No room for a default tab between the held regions and the part.");
                }
                DefaultTabs::Failed { message, .. } => {
                    ui.label(RichText::new(NO_TABS_WARNING).color(NOTE));
                    ui.weak(message);
                }
                DefaultTabs::None => {
                    ui.label(RichText::new(NO_TABS_WARNING).color(NOTE));
                }
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Add tab").clicked() {
                let tab = new_tab(self.draft.workholding(), setup.stock.as_ref());
                self.draft.edit().tabs.push(tab);
                self.focus_new_tab = true;
            }
            let state = default_tabs_state(&setup.workholding, self.draft.workholding());
            let can = matches!(state, DefaultTabsState::Available)
                && matches!(&self.defaults, DefaultTabs::Found { tabs, .. } if !tabs.is_empty());
            let response = ui.add_enabled(can, egui::Button::new("Add default tabs"));
            let response = match state {
                DefaultTabsState::Refused(why) => response.on_disabled_hover_text(why),
                DefaultTabsState::Available => response.on_hover_text(
                    "One tab at the head and one at the tail, frustums with the small end on \
                     the part, sized from the part",
                ),
            };
            if response.clicked()
                && let DefaultTabs::Found { tabs, .. } = &self.defaults
            {
                let tabs = tabs.clone();
                self.draft.edit().tabs.extend(tabs);
            }
        });
        if let Some((i, tab)) = edited
            && let Some(slot) = self.draft.edit().tabs.get_mut(i)
        {
            *slot = tab;
        }
        if let Some(i) = duplicate
            && let Some(tab) = self.draft.workholding().tabs.get(i).copied()
        {
            self.draft.edit().tabs.insert(i + 1, tab);
        }
        if let Some(i) = remove {
            self.draft.edit().tabs.remove(i);
        }
    }

    /// Apply, Revert and Check, with the spinner and Cancel while checking.
    fn buttons(
        &mut self,
        ui: &mut egui::Ui,
        app: &Application,
        setup: &Setup,
        problem: Option<&FieldProblem>,
    ) -> Option<Command> {
        let changed = self.draft.is_changed();
        let idle = self.checking.is_none();
        let mut apply = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(idle, egui::Button::new("Check against the part and tools"))
                .on_hover_text("Tabs lie in the head or tail zone and are wider than the tools")
                .clicked()
            {
                self.start_check(app, setup, false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if idle {
                    let reason = if problem.is_some() {
                        "Fix the marked field first"
                    } else if !changed {
                        "Nothing to apply"
                    } else {
                        "Store the held regions and tabs in the setup (it can be undone)"
                    };
                    let response =
                        ui.add_enabled(changed && problem.is_none(), egui::Button::new("Apply"));
                    let response = if response.enabled() {
                        response.on_hover_text(reason)
                    } else {
                        response.on_disabled_hover_text(
                            problem.map_or_else(|| reason.to_owned(), |p| p.message.clone()),
                        )
                    };
                    apply = response.clicked()
                        || (changed
                            && problem.is_none()
                            && ui
                                .input(|k| k.key_pressed(egui::Key::Enter) && k.modifiers.command));
                } else {
                    if ui.button("Cancel").clicked() {
                        self.checking = None;
                    }
                    ui.spinner();
                }
                if ui
                    .add_enabled(changed, egui::Button::new("Revert"))
                    .clicked()
                {
                    self.draft.revert();
                    self.diameter_text = (
                        diameter_string(&self.draft.workholding().start),
                        diameter_string(&self.draft.workholding().end),
                    );
                }
            });
        });
        if apply {
            self.start_check(app, setup, true);
        }
        None
    }

    /// Starts checking the draft against the part and the tools: tabs outside
    /// the head and tail zones, and tabs narrower than a tool the setup uses,
    /// are refused when stored, as they are when generating (OI-159, OI-180).
    fn start_check(&mut self, app: &Application, setup: &Setup, apply: bool) {
        self.message = None;
        self.checking = Some(Checking {
            job: app.start_check_workholding(self.setup, self.draft.workholding()),
            apply,
            draft: self.draft.workholding().clone(),
            inputs: check_inputs(app, setup),
        });
    }

    /// Takes the result of a finished check. Returns the command that stores
    /// the draft the check was made on, if it was asked to and found nothing.
    fn poll_check(
        &mut self,
        ctx: &egui::Context,
        app: &Application,
        setup: &Setup,
    ) -> Option<Command> {
        let checking = self.checking.as_ref()?;
        let Some(result) = checking.job.try_result() else {
            ctx.request_repaint();
            return None;
        };
        let Checking {
            apply,
            draft,
            inputs,
            ..
        } = self.checking.take()?;
        // The part, the stock or a tool changed since the check started (the
        // panel is polled only while it is shown): the result is stale.
        if inputs != check_inputs(app, setup) {
            self.message =
                Some("The part, stock or tools changed while checking; check again.".to_owned());
            return None;
        }
        self.finish_check(result.map_err(|e| e.to_string()), apply, draft, setup)
    }

    /// Records what a check found. With `apply` and no findings, the command
    /// that stores `draft` in the setup; otherwise `None`, with the reason in
    /// the panel's message (applying) or findings (checking).
    fn finish_check(
        &mut self,
        result: Result<Vec<String>, String>,
        apply: bool,
        draft: Workholding,
        setup: &Setup,
    ) -> Option<Command> {
        match result {
            Ok(found) if found.is_empty() && apply => {
                // Edited or reverted while the check ran: what was checked is
                // not what is on screen, so store nothing.
                if draft != *self.draft.workholding() {
                    self.message = Some(
                        "The draft changed while it was being checked; apply it again.".to_owned(),
                    );
                    return None;
                }
                let mut edited = setup.clone();
                edited.workholding = draft;
                self.message = None;
                Some(Command::Replace(Item::Setup(edited)))
            }
            Ok(found) if apply => {
                self.message = Some(found.join("\n"));
                None
            }
            Ok(found) if found.is_empty() => {
                self.findings = vec![
                    "The tabs are in the head and tail zones and wide enough for the tools."
                        .to_owned(),
                ];
                None
            }
            Ok(found) => {
                self.findings = found;
                None
            }
            Err(e) => {
                self.message = Some(e);
                None
            }
        }
    }

    /// Starts the default-tabs job when the setup has no tabs and its inputs
    /// changed, and takes its result when it has finished (11.7).
    fn poll_defaults(&mut self, ctx: &egui::Context, app: &Application, setup: &Setup) {
        let key = defaults_key(setup);
        if let DefaultTabs::Running { job, key: running } = &self.defaults {
            match job.try_result() {
                None => {
                    ctx.request_repaint();
                    return;
                }
                Some(result) => {
                    let key = running.clone();
                    self.defaults = match result {
                        Ok(tabs) => DefaultTabs::Found { tabs, key },
                        Err(e) => DefaultTabs::Failed {
                            message: e.to_string(),
                            key,
                        },
                    };
                }
            }
        }
        let current = match &self.defaults {
            DefaultTabs::Found { key, .. } | DefaultTabs::Failed { key, .. } => Some(key),
            _ => None,
        };
        // With tabs stored the defaults cannot be worked out again; the last
        // ones stay for the chips.
        if setup.workholding.tabs.is_empty()
            && setup.stock.is_some()
            && setup.part.is_some()
            && current != Some(&key)
            && !matches!(&self.defaults, DefaultTabs::Running { key: k, .. } if *k == key)
        {
            self.defaults = DefaultTabs::Running {
                job: app.start_default_tabs(self.setup),
                key,
            };
            ctx.request_repaint();
        }
    }
}

/// The text of the holding-diameter field of `end`.
fn text_of(texts: &mut (String, String), end: End) -> &mut String {
    match end {
        End::Start => &mut texts.0,
        End::End => &mut texts.1,
    }
}

fn stock_diameter_hint(stock: Option<&Stock>) -> String {
    match stock.and_then(|s| s.radius(None)) {
        Some(r) => format!("Stock diameter ({:.3} mm)", 2.0 * r.0),
        None => "Stock diameter".to_owned(),
    }
}

/// The tab a field belongs to.
fn field_tab(field: Field) -> Option<usize> {
    match field {
        Field::Grip(_) | Field::HoldingDiameter(_) => None,
        Field::TabStartX(i)
        | Field::TabLength(i)
        | Field::TabDiameter(i)
        | Field::TabStartDiameter(i)
        | Field::TabEndDiameter(i)
        | Field::TabCount(i)
        | Field::TabStartAngle(i)
        | Field::TabOffset(i) => Some(i),
    }
}

fn problem_is(problem: Option<&FieldProblem>, field: Field) -> bool {
    problem.is_some_and(|p| p.field == field)
}

/// The reason, in red under the field it marks (UI-07).
fn show_problem(ui: &mut egui::Ui, problem: Option<&FieldProblem>, field: Field) {
    if let Some(p) = problem.filter(|p| p.field == field) {
        ui.label(RichText::new(&p.message).color(ERROR));
    }
}

/// A millimetre field with a red border while it is marked. Returns true when
/// the value changed.
fn mm_field(ui: &mut egui::Ui, value: &mut f64, marked: bool) -> bool {
    if marked {
        let stroke = &mut ui.style_mut().visuals.widgets.inactive.bg_stroke;
        *stroke = egui::Stroke::new(1.0, ERROR);
    }
    ui.add(egui::DragValue::new(value).speed(0.1).max_decimals(3))
        .changed()
        | {
            ui.label("mm");
            false
        }
}

/// One labelled millimetre row with its message.
fn row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    problem: Option<&FieldProblem>,
    field: Field,
) -> bool {
    let changed = ui
        .horizontal(|ui| {
            ui.add_sized([120.0, 18.0], egui::Label::new(label));
            mm_field(ui, value, problem_is(problem, field))
        })
        .inner;
    show_problem(ui, problem, field);
    changed
}

/// One tab's fields (11.7). Returns true when a value changed.
fn tab_editor(
    ui: &mut egui::Ui,
    index: usize,
    tab: &mut Tab,
    problem: Option<&FieldProblem>,
    _stock: Option<&Stock>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 18.0], egui::Label::new("Shape"));
        let mut cylinder = matches!(tab.shape, TabShape::Cylinder { .. });
        let before = cylinder;
        ui.selectable_value(&mut cylinder, true, "Cylinder");
        ui.selectable_value(&mut cylinder, false, "Frustum");
        if cylinder != before {
            set_shape(tab, cylinder);
            changed = true;
        }
    });
    let i = index;
    changed |= row(
        ui,
        "Start X",
        &mut tab.x_start.0,
        problem,
        Field::TabStartX(i),
    );
    changed |= row(
        ui,
        "Length",
        &mut tab.length.0,
        problem,
        Field::TabLength(i),
    );
    match &mut tab.shape {
        TabShape::Cylinder { diameter } => {
            changed |= row(
                ui,
                "Diameter",
                &mut diameter.0,
                problem,
                Field::TabDiameter(i),
            );
        }
        TabShape::Frustum {
            start_diameter,
            end_diameter,
        } => {
            changed |= row(
                ui,
                "Diameter at start X",
                &mut start_diameter.0,
                problem,
                Field::TabStartDiameter(i),
            );
            changed |= row(
                ui,
                "Diameter at end X",
                &mut end_diameter.0,
                problem,
                Field::TabEndDiameter(i),
            );
        }
        _ => {}
    }
    ui.label(RichText::new(format!("Narrowest: {:.3} mm", narrowest(tab))).weak());
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 18.0], egui::Label::new("Placement"));
        let mut around = matches!(tab.layout, TabLayout::AroundAxis { .. });
        let before = around;
        ui.selectable_value(&mut around, false, "On the axis");
        ui.selectable_value(&mut around, true, "Around the axis");
        if around != before {
            set_layout(tab, around);
            changed = true;
        }
    });
    if let TabLayout::AroundAxis {
        count,
        start_angle,
        radial_offset,
    } = &mut tab.layout
    {
        ui.horizontal(|ui| {
            ui.add_sized([120.0, 18.0], egui::Label::new("Count"));
            changed |= ui.add(egui::DragValue::new(count).range(0..=999)).changed();
        });
        show_problem(ui, problem, Field::TabCount(i));
        ui.horizontal(|ui| {
            ui.add_sized([120.0, 18.0], egui::Label::new("Start angle"));
            changed |= ui
                .add(
                    egui::DragValue::new(&mut start_angle.0)
                        .speed(1.0)
                        .max_decimals(2)
                        .suffix("\u{b0}"),
                )
                .changed();
        });
        show_problem(ui, problem, Field::TabStartAngle(i));
        changed |= row(
            ui,
            "Offset from axis",
            &mut radial_offset.0,
            problem,
            Field::TabOffset(i),
        );
    }
    changed
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::testing::cube_project;

    fn tab(x_start: f64, length: f64, diameter: f64) -> Tab {
        Tab {
            x_start: Millimetres(x_start),
            length: Millimetres(length),
            shape: TabShape::Cylinder {
                diameter: Millimetres(diameter),
            },
            layout: TabLayout::OnAxis,
        }
    }

    /// The check of the panel's draft, run as the panel runs it (a job).
    fn check(app: &Application, panel: &WorkholdingPanel) -> Result<Vec<String>, String> {
        app.start_check_workholding(panel.setup, panel.draft.workholding())
            .wait()
            .map_err(|e| e.to_string())
    }

    #[test]
    fn req_set_09_req_op_21_the_panel_stores_a_valid_draft_and_refuses_the_rest() {
        let (dir, mut app, _, setup_id) = cube_project("workholding-panel");
        let setup = app.project().setup(setup_id).unwrap().clone();
        let mut panel = WorkholdingPanel::new(&setup);
        // The cube spans X about -5 to 5 in stock from -7 to 7: a 1 mm grip
        // and a 4 mm tab in the 1 mm of head zone left fit; the tool is 4 mm.
        panel.draft.edit().start = Some(HeldRegion {
            length: Millimetres(1.0),
            diameter: None,
        });
        panel.draft.edit().tabs = vec![tab(-6.0, 1.0, 4.0)];
        let command = panel
            .finish_check(
                check(&app, &panel),
                true,
                panel.draft.workholding().clone(),
                &setup,
            )
            .expect("a valid draft");
        app.execute(command).unwrap();
        assert_eq!(
            app.project().setup(setup_id).unwrap().workholding,
            *panel.draft.workholding()
        );
        // One undo takes it back (UI-08).
        assert!(app.undo().unwrap());
        assert!(
            app.project()
                .setup(setup_id)
                .unwrap()
                .workholding
                .is_empty()
        );

        // A tab across the part is not in the head or the tail zone.
        panel.draft.edit().tabs = vec![tab(-1.0, 2.0, 6.0)];
        assert!(
            panel
                .finish_check(
                    check(&app, &panel),
                    true,
                    panel.draft.workholding().clone(),
                    &setup
                )
                .is_none()
        );
        let message = panel.message.clone().unwrap();
        assert!(message.contains("Tab 1"), "{message}");
        // A change to the stock is a change to what a check depends on.
        let before = check_inputs(&app, &setup);
        let mut moved = setup.clone();
        moved.stock = Some(openchuck_app::setup::Stock::Cylinder {
            diameter: Millimetres(22.0),
            length: Millimetres(14.0),
            x_start: Millimetres(-7.0),
        });
        app.execute(Command::Replace(Item::Setup(moved))).unwrap();
        let after_setup = app.project().setup(setup_id).unwrap().clone();
        assert_ne!(before, check_inputs(&app, &after_setup));
        app.undo().unwrap();
        assert_eq!(before, check_inputs(&app, &setup));
        // A draft edited or reverted while a clean check ran is not stored.
        let checked = Workholding::default();
        assert!(
            panel
                .finish_check(Ok(Vec::new()), true, checked, &setup)
                .is_none()
        );
        let message = panel.message.clone().unwrap();
        assert!(message.contains("changed while"), "{message}");
        // A tab narrower than the 4 mm tool is refused, naming the operation.
        panel.draft.edit().tabs = vec![tab(-6.0, 1.0, 3.0)];
        assert!(
            panel
                .finish_check(
                    check(&app, &panel),
                    true,
                    panel.draft.workholding().clone(),
                    &setup
                )
                .is_none()
        );
        let message = panel.message.clone().unwrap();
        assert!(message.contains("narrower than the tool"), "{message}");
        assert!(message.contains("Finish"), "{message}");
        assert!(message.starts_with("Tab 1 is 3.000 mm across"), "{message}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The text a frame of the panel paints.
    fn painted(
        panel: &mut WorkholdingPanel,
        ctx: &egui::Context,
        app: &Application,
        setup: &Setup,
    ) -> String {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(900.0, 1400.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            panel.show(ui, app, setup);
        });
        output.textures_delta.clear();
        let mut text = String::new();
        for clipped in &output.shapes {
            if let egui::epaint::Shape::Text(t) = &clipped.shape {
                text.push_str(t.galley.text());
                text.push('\n');
            }
        }
        text
    }

    #[test]
    fn req_ui_25_the_panel_marks_a_refused_draft_in_plain_words_and_offers_the_defaults() {
        let (dir, app, _, setup_id) = cube_project("workholding-frames");
        let setup = app.project().setup(setup_id).unwrap().clone();
        let mut panel = WorkholdingPanel::new(&setup);
        let ctx = egui::Context::default();
        // Nothing stored: the default tabs are worked out as a job, then
        // offered as defaults with the warning.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let text = loop {
            let text = painted(&mut panel, &ctx, &app, &setup);
            if !matches!(panel.defaults, DefaultTabs::Running { .. }) {
                break text;
            }
            assert!(std::time::Instant::now() < deadline, "timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert!(text.contains("Workholding and tabs"), "{text}");
        assert!(text.contains("None"), "{text}");
        assert!(text.contains("No tabs are stored."), "{text}");
        assert!(!text.contains("Not applied"), "{text}");

        // A grip longer than the stock: the plain message under the field,
        // no field path, and the chip and the footer note say it is unapplied.
        panel.draft.edit().start = Some(HeldRegion {
            length: Millimetres(500.0),
            diameter: None,
        });
        let text = painted(&mut panel, &ctx, &app, &setup);
        assert!(text.contains("Longer than the stock"), "{text}");
        assert!(!text.contains("setup.workholding"), "{text}");
        assert!(text.contains("Not applied"), "{text}");
        assert!(panel.has_unapplied_changes());
        assert!(
            text.contains("Held span") || text.contains("Grip length"),
            "{text}"
        );

        // An undo of the stored value elsewhere discards the draft, saying so.
        let mut stored = setup.clone();
        stored.workholding.end = Some(HeldRegion {
            length: Millimetres(1.0),
            diameter: None,
        });
        let text = painted(&mut panel, &ctx, &app, &stored);
        assert!(
            text.contains("your unapplied changes were discarded"),
            "{text}"
        );
        assert!(!panel.has_unapplied_changes());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
