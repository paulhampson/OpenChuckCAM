// SPDX-License-Identifier: AGPL-3.0-or-later
//! A setup's workholding and retaining tabs: the fifth section of the setup
//! inspector (SET-09, OP-21; UI-03, UI-07, UI-25; work breakdown slices 21c,
//! 21d and 22; decision records 0045, 0104 and 0114, OI-334; interface
//! specification section 11.7).
//!
//! The section edits a draft of the setup's held regions and tabs
//! ([`Draft`]) in the shared field rows of section 8: a value goes into the
//! draft on Enter or when its field loses focus, and Esc puts back its last
//! value. The draft is checked as it changes, with the reason under the
//! field it marks, and applied with one undoable edit of the setup (UI-08).
//! What needs the part comes from jobs: the part's extent and the default tab
//! of each zone ([`Application::start_part_extent`]), and the checks against
//! the part and the tools, whose findings go on their tab's row.

use eframe::egui::{self, Color32, RichText, Stroke};
use openchuck_app::project::id::SetupId;
use openchuck_app::project::{Item, Setup};
use openchuck_app::setup::{
    HeldRegion, Millimetres, PartExtent, Tab, TabLayout, TabShape, Workholding,
};
use openchuck_app::workholding::{
    PartFacts, SetupTool, WorkholdingFinding, ZoneDefault, setup_tools,
};
use openchuck_app::workholding_view::{
    DefaultTabsState, Draft, End, Field, FieldProblem, NO_TABS_WARNING, Zone, check_draft,
    default_chips, default_tabs_state, held_span, narrowest_line, new_held_region, new_tab,
    no_default_line, part_facing, set_layout, set_shape, tab_zone, zone_lines,
};
use openchuck_app::{Application, Command, JobHandle};

use crate::field_row::{self, FieldState, Outcome, TextRow};
use crate::theme::DARK;

const ERROR: Color32 = DARK.error;
const NOTE: Color32 = DARK.warning;

/// A held region or a tab of the section: a row of its own, a solid in the
/// view (7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeldItem {
    /// The held region at the low-X end of the stock.
    Start,
    /// The held region at the high-X end of the stock.
    End,
    /// The tab at this index, from zero.
    Tab(usize),
}

/// What the view draws for the section (7.5): the draft while it has
/// unapplied changes, the held regions and tabs of the draft that fail a
/// check, the default tabs offered and not added, and the row with keyboard
/// focus.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftView {
    /// The draft, when it differs from what the setup stores.
    pub draft: Option<Workholding>,
    /// The held regions and tabs of the draft that fail a check.
    pub failing: Vec<HeldItem>,
    /// The default tabs offered while the setup has none.
    pub offered: Vec<Tab>,
    /// The row with keyboard focus, whose solid the view outlines.
    pub focused: Option<HeldItem>,
    /// The placed part's extent, which places the solids.
    pub part: Option<PartExtent>,
}

/// The rest of the project the section shows (11.7): the stored
/// workholding's error when it cannot be built, and the operations whose
/// selections reach a held region or a tab, as ("2 Helical finish", the
/// warning).
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Why the stored workholding cannot be built.
    pub stored_error: Option<String>,
    /// The selection warnings of the setup's operations.
    pub reach: Vec<(String, String)>,
}

/// The section's state: the draft being edited.
pub struct WorkholdingPanel {
    setup: SetupId,
    draft: Draft,
    /// Typed text and refused text of the rows.
    fields: FieldState,
    /// The part's extent and the default tabs: being worked out, or what the
    /// job found.
    facts: Facts,
    /// A check against the part and tools that is running.
    checking: Option<Checking>,
    /// What the last check found (amber), or why the last apply refused
    /// (red), with the tab each concerns.
    findings: Vec<WorkholdingFinding>,
    refused: bool,
    message: Option<String>,
    /// Set for one frame to open the tab just added and focus its Start X.
    focus_new_tab: bool,
    /// The row with keyboard focus this frame.
    focused: Option<HeldItem>,
}

/// The part's extent and default tabs (11.7): worked out as a job when the
/// section opens and after each apply.
enum Facts {
    None,
    Running {
        job: JobHandle<PartFacts>,
        key: String,
    },
    Found {
        facts: PartFacts,
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
    job: JobHandle<Vec<WorkholdingFinding>>,
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

/// What the part's facts depend on: the part as placed, the stock and the
/// stored held regions.
fn facts_key(app: &Application, setup: &Setup) -> String {
    let model = setup
        .part
        .and_then(|id| app.project().models().iter().find(|m| m.id == id));
    format!(
        "{model:?} {:?} {:?} {:?}",
        setup.stock, setup.workholding.start, setup.workholding.end
    )
}

impl std::fmt::Debug for WorkholdingPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkholdingPanel")
            .field("setup", &self.setup)
            .finish_non_exhaustive()
    }
}

fn mm(v: f64) -> String {
    field_row::number(v, Some(3))
}

/// The tab or held region a draft field belongs to.
fn field_item(field: Field) -> HeldItem {
    match field {
        Field::Grip(End::Start) | Field::HoldingDiameter(End::Start) => HeldItem::Start,
        Field::Grip(End::End) | Field::HoldingDiameter(End::End) => HeldItem::End,
        Field::TabStartX(i)
        | Field::TabLength(i)
        | Field::TabDiameter(i)
        | Field::TabStartDiameter(i)
        | Field::TabEndDiameter(i)
        | Field::TabCount(i)
        | Field::TabStartAngle(i)
        | Field::TabOffset(i) => HeldItem::Tab(i),
    }
}

/// The text of a field's message when `problem` marks it.
fn problem_of(problem: Option<&FieldProblem>, field: Field) -> Option<&str> {
    problem
        .filter(|p| p.field == field)
        .map(|p| p.message.as_str())
}

/// Whether a selection warning is about `item`.
fn reaches(note: &str, item: HeldItem) -> bool {
    match item {
        HeldItem::Start => note.contains("reaches the held region at the start"),
        HeldItem::End => note.contains("reaches the held region at the end"),
        HeldItem::Tab(i) => note.contains(&format!("reaches tab {},", i + 1)),
    }
}

/// The muted "Reached by the selections of 2 Helical finish." line under a
/// held region or tab (11.7).
fn reached_line(ui: &mut egui::Ui, reach: &[(String, String)], item: HeldItem) {
    let mut names: Vec<&str> = reach
        .iter()
        .filter(|(_, note)| reaches(note, item))
        .map(|(op, _)| op.as_str())
        .collect();
    names.dedup();
    if !names.is_empty() {
        field_row::note_line(
            ui,
            &format!("Reached by the selections of {}.", names.join(", ")),
            DARK.muted,
        );
    }
}

/// The hint of an empty count of degrees: the angles of the tabs around the
/// axis, "0.00°, 180.00°".
fn angles_hint(count: u32, start: f64) -> String {
    (0..count.min(8))
        .map(|k| {
            format!(
                "{:.2}\u{b0}",
                (start + 360.0 * f64::from(k) / f64::from(count.max(1))).rem_euclid(360.0)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl WorkholdingPanel {
    /// A section for `setup`, starting from its stored workholding.
    pub fn new(setup: &Setup) -> Self {
        Self {
            setup: setup.id,
            draft: Draft::new(&setup.workholding),
            fields: FieldState::default(),
            facts: Facts::None,
            checking: None,
            findings: Vec::new(),
            refused: false,
            message: None,
            focus_new_tab: false,
            focused: None,
        }
    }

    /// The setup the section is for.
    pub fn setup(&self) -> SetupId {
        self.setup
    }

    /// True while the draft has changes that are not applied (the footer says
    /// so, 11.7).
    pub fn has_unapplied_changes(&self) -> bool {
        self.draft.is_changed()
    }

    /// The placed part's extent, once the job has worked it out.
    pub fn part_extent(&self) -> Option<PartExtent> {
        match &self.facts {
            Facts::Found { facts, .. } => Some(facts.extent),
            _ => None,
        }
    }

    /// The default tab of each zone, once worked out.
    fn defaults(&self) -> Option<&[ZoneDefault; 2]> {
        match &self.facts {
            Facts::Found { facts, .. } => facts.defaults.as_ref(),
            _ => None,
        }
    }

    /// The default tabs offered, head then tail.
    fn offered(&self) -> Vec<Tab> {
        self.defaults()
            .map(|d| {
                d.iter()
                    .filter_map(|z| match z {
                        ZoneDefault::Tab(t) => Some(*t),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What the view draws for the section (7.5; OI-334 k): the draft while
    /// it has unapplied changes, its failing parts, the defaults offered while
    /// the setup and the draft have no tabs, and the focused row.
    pub fn draft_view(&self, setup: &Setup) -> DraftView {
        let problem = check_draft(self.draft.workholding(), setup.stock.as_ref());
        let mut failing: Vec<HeldItem> = problem.iter().map(|p| field_item(p.field)).collect();
        if self.refused {
            failing.extend(
                self.findings
                    .iter()
                    .filter_map(|f| f.tab)
                    .map(HeldItem::Tab),
            );
        }
        let offered =
            if setup.workholding.tabs.is_empty() && self.draft.workholding().tabs.is_empty() {
                self.offered()
            } else {
                Vec::new()
            };
        DraftView {
            draft: self
                .draft
                .is_changed()
                .then(|| self.draft.workholding().clone()),
            failing,
            offered,
            focused: self.focused,
            part: self.part_extent(),
        }
    }

    /// Draws the section. Returns the command to execute when the user
    /// applies the draft.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        app: &Application,
        setup: &Setup,
        context: &Context,
    ) -> Option<Command> {
        // Another edit (an undo, the command line) changed the project: start
        // again from what it holds, saying so if that threw changes away.
        if *self.draft.stored() != setup.workholding {
            self.draft.sync(&setup.workholding);
            self.fields.clear();
            self.findings.clear();
            self.message = None;
        }
        self.poll_facts(ui.ctx(), app, setup);
        self.focused = None;

        if let Some(reason) = &context.stored_error {
            let boxed = DARK.error_box;
            egui::Frame::new()
                .fill(boxed.bg)
                .stroke(Stroke::new(1.0, boxed.border))
                .corner_radius(crate::theme::RADIUS)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("The stored workholding can't be built")
                            .strong()
                            .color(boxed.text),
                    );
                    ui.label(RichText::new(reason).color(boxed.text));
                });
        }
        if let Some(note) = self.draft.note() {
            ui.label(RichText::new(note).color(NOTE));
        }
        ui.label(
            RichText::new(
                "Held regions are never machined and links keep clear of them. Tabs keep the \
                 part joined to the stock. Both count as part of the finished surface.",
            )
            .color(DARK.muted),
        );
        if setup.stock.is_none() {
            ui.label(
                RichText::new("Set the stock first: held regions and tabs are placed on it.")
                    .color(NOTE),
            );
        }

        let problem = check_draft(self.draft.workholding(), setup.stock.as_ref());
        self.held_block(ui, setup, End::Start, problem.as_ref(), context);
        self.held_block(ui, setup, End::End, problem.as_ref(), context);
        ui.separator();
        self.zones(ui, setup);
        ui.separator();
        let tools = setup_tools(app.project(), setup.id);
        self.tab_list(ui, setup, problem.as_ref(), &tools, context);
        ui.separator();

        if let Some(m) = &self.message {
            ui.label(RichText::new(m).color(ERROR));
        }
        for f in self.findings.iter().filter(|f| f.tab.is_none()) {
            let colour = if self.refused { ERROR } else { NOTE };
            ui.label(RichText::new(&f.text).color(colour));
        }
        let command = self.buttons(ui, app, setup, problem.as_ref());
        // After the controls, so a draft edited or reverted this frame is
        // the draft the finished check is compared with.
        command.or_else(|| self.poll_check(ui.ctx(), app, setup))
    }

    /// Edits the draft with `edit`, and forgets what a check found, which
    /// was about the draft as it was.
    fn edit(&mut self, edit: impl FnOnce(&mut Workholding)) {
        edit(self.draft.edit());
        if !self.refused {
            self.findings.clear();
        }
    }

    /// A number row of the draft. Returns the new value when one was
    /// committed; text that is not a number stays in its field with "Not a
    /// number." (11.7).
    #[allow(clippy::too_many_arguments)]
    fn number_row(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        label: &str,
        unit: &str,
        value: Option<f64>,
        hint: Option<String>,
        problem: Option<&str>,
        focus: bool,
    ) -> Option<Option<f64>> {
        let applied = value
            .map(|v| field_row::number(v, field_row::places(unit)))
            .unwrap_or_default();
        let outcome = field_row::text_row(
            ui,
            &mut self.fields,
            &TextRow {
                key,
                label,
                unit,
                applied,
                hint,
                problem,
                enabled: true,
                focus,
                ..TextRow::default()
            },
        );
        let Outcome::Commit(typed) = outcome else {
            return None;
        };
        let typed = typed.trim().to_owned();
        if typed.is_empty() {
            self.fields.result(key, Ok(()));
            return Some(None);
        }
        match typed.parse::<f64>().ok().filter(|v| v.is_finite()) {
            Some(v) => {
                self.fields.result(key, Ok(()));
                Some(Some(v))
            }
            None => {
                self.fields.refuse(key, &typed, "Not a number.".to_owned());
                None
            }
        }
    }

    /// One held region: a check box, the grip length, an optional holding
    /// diameter (empty for the stock's own) and the span (11.7 item 2).
    fn held_block(
        &mut self,
        ui: &mut egui::Ui,
        setup: &Setup,
        end: End,
        problem: Option<&FieldProblem>,
        context: &Context,
    ) {
        let (label, item, key) = match end {
            End::Start => ("Held at the start", HeldItem::Start, "start"),
            End::End => ("Held at the end", HeldItem::End, "end"),
        };
        let current = match end {
            End::Start => self.draft.workholding().start,
            End::End => self.draft.workholding().end,
        };
        if let Some(on) = field_row::flag_row(
            ui,
            label,
            "The chuck grips the stock here; it is never machined.",
            current.is_some(),
            true,
        ) {
            self.edit(|w| {
                let region = match end {
                    End::Start => &mut w.start,
                    End::End => &mut w.end,
                };
                *region = on.then(new_held_region);
            });
        }
        let Some(region) = current else { return };
        let set = |w: &mut Workholding, region: HeldRegion| match end {
            End::Start => w.start = Some(region),
            End::End => w.end = Some(region),
        };
        if let Some(Some(v)) = self.number_row(
            ui,
            &format!("grip {key}"),
            "Grip length",
            "mm",
            Some(region.length.0),
            None,
            problem_of(problem, Field::Grip(end)),
            false,
        ) {
            self.edit(|w| {
                set(
                    w,
                    HeldRegion {
                        length: Millimetres(v),
                        ..region
                    },
                );
            });
        }
        let hint = match setup.stock.as_ref().and_then(|s| s.radius(None)) {
            Some(r) => format!("Stock diameter ({} mm)", mm(2.0 * r.0)),
            None => "Stock diameter".to_owned(),
        };
        if let Some(v) = self.number_row(
            ui,
            &format!("diameter {key}"),
            "Holding diameter",
            "mm",
            region.diameter.map(|d| d.0),
            Some(hint),
            problem_of(problem, Field::HoldingDiameter(end)),
            false,
        ) {
            self.edit(|w| {
                set(
                    w,
                    HeldRegion {
                        diameter: v.map(Millimetres),
                        ..region
                    },
                );
            });
        }
        if let Some(stock) = &setup.stock
            && let Some(span) = held_span(self.draft.workholding(), end, stock)
        {
            field_row::read_only_row(ui, "Held span", "", format!("{span} mm"));
        }
        reached_line(ui, &context.reach, item);
    }

    /// The read-only head and tail zone lines (11.7 item 3), or what they
    /// wait for.
    fn zones(&self, ui: &mut egui::Ui, setup: &Setup) {
        if setup.part.is_none() {
            ui.label(
                RichText::new("Import a model first: the zones need the part.").color(DARK.muted),
            );
            return;
        }
        let Some(stock) = &setup.stock else {
            ui.label(RichText::new("Set the stock first: the zones need it.").color(DARK.muted));
            return;
        };
        match &self.facts {
            Facts::Found { facts, .. } => {
                match zone_lines(&setup.workholding, stock, &facts.extent) {
                    Some(lines) => {
                        for line in lines {
                            ui.label(line);
                        }
                    }
                    None => {
                        ui.label(
                            RichText::new("The stock's extent is not known.").color(DARK.muted),
                        );
                    }
                }
            }
            Facts::Failed { message, .. } => {
                ui.label(RichText::new(message).color(ERROR));
            }
            Facts::Running { .. } | Facts::None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Working out from the part\u{2026}").color(DARK.muted));
                });
            }
        }
    }

    /// The tab rows, the default tabs offered as defaults, and the buttons
    /// under them (11.7 items 4 and 5).
    fn tab_list(
        &mut self,
        ui: &mut egui::Ui,
        setup: &Setup,
        problem: Option<&FieldProblem>,
        tools: &[SetupTool],
        context: &Context,
    ) {
        ui.label(
            RichText::new(format!(
                "Retaining tabs ({})",
                self.draft.workholding().tabs.len()
            ))
            .strong()
            .color(DARK.heading),
        );
        let offered = self.offered();
        let chips = default_chips(&self.draft.workholding().tabs, &offered);
        let focus = std::mem::take(&mut self.focus_new_tab);
        let last = self.draft.workholding().tabs.len().saturating_sub(1);
        let mut remove = None;
        let mut duplicate = None;
        let tabs = self.draft.workholding().tabs.clone();
        for (i, tab) in tabs.iter().enumerate() {
            let marked = problem.is_some_and(|p| field_item(p.field) == HeldItem::Tab(i));
            let finding: Vec<String> = self
                .findings
                .iter()
                .filter(|f| f.tab == Some(i))
                .map(|f| f.text.clone())
                .collect();
            let zone = match (&setup.stock, self.part_extent()) {
                (Some(stock), Some(part)) => tab_zone(tab, &setup.workholding, stock, &part),
                _ => None,
            };
            let header = format!(
                "Tab {} \u{b7} {} \u{b7} X {} to {}{}",
                i + 1,
                if matches!(tab.shape, TabShape::Cylinder { .. }) {
                    "Cylinder"
                } else {
                    "Frustum"
                },
                mm(tab.x_start.0).replacen('-', "\u{2212}", 1),
                mm(tab.x_start.0 + tab.length.0).replacen('-', "\u{2212}", 1),
                zone.map_or(String::new(), |z| format!(" \u{b7} {}", z.name())),
            );
            let id = ui.make_persistent_id(("tab", self.setup, i));
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                false,
            );
            if (focus && i == last) || marked || !finding.is_empty() && self.refused {
                state.set_open(true);
            }
            let mut menu = None;
            let is_default = chips.get(i).copied().unwrap_or(false);
            let (enter, left, right, del) = ui.input(|k| {
                (
                    k.key_pressed(egui::Key::Enter),
                    k.key_pressed(egui::Key::ArrowLeft),
                    k.key_pressed(egui::Key::ArrowRight),
                    k.key_pressed(egui::Key::Delete),
                )
            });
            let mut title_response = None;
            let mut header_row = state.show_header(ui, |ui| {
                if !finding.is_empty() {
                    let state = if self.refused {
                        openchuck_app::overview::ItemState::Error
                    } else {
                        openchuck_app::overview::ItemState::Warning
                    };
                    crate::shell::mark(ui, state, &finding.join(" "));
                }
                let colour = if marked || (self.refused && !finding.is_empty()) {
                    ERROR
                } else {
                    DARK.text
                };
                let title = ui.add(
                    egui::Label::new(RichText::new(&header).color(colour))
                        .sense(egui::Sense::click()),
                );
                if is_default {
                    let zone_name = zone.map_or("its", |z| match z {
                        Zone::Head => "the head",
                        Zone::Tail => "the tail",
                    });
                    let chip = egui::Frame::new()
                        .stroke(Stroke::new(1.0, DARK.line))
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(5, 0))
                        .show(ui, |ui| ui.label(RichText::new("Default").small()))
                        .response
                        .interact(egui::Sense::focusable_noninteractive());
                    field_row::tip(
                        chip,
                        &format!(
                            "Same as the default tab for {zone_name} zone. Editing a value \
                             overrides it."
                        ),
                    );
                }
                let more = ui.small_button("\u{22ef}").on_hover_text("Tab menu");
                more.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        format!("Tab {} menu", i + 1),
                    )
                });
                egui::Popup::menu(&more).show(|ui| {
                    if ui.button("Duplicate").clicked() {
                        menu = Some(false);
                    }
                    if ui.button("Remove").clicked() {
                        menu = Some(true);
                    }
                });
                title.context_menu(|ui| {
                    if ui.button("Duplicate").clicked() {
                        menu = Some(false);
                    }
                    if ui.button("Remove").clicked() {
                        menu = Some(true);
                    }
                });
                title_response = Some(title);
            });
            let Some(title) = title_response else {
                continue;
            };
            if title.clicked() {
                header_row.toggle();
            }
            if title.has_focus() {
                self.focused = Some(HeldItem::Tab(i));
                if enter {
                    header_row.toggle();
                }
                if left && header_row.is_open() || right && !header_row.is_open() {
                    header_row.toggle();
                }
                if del {
                    remove = Some(i);
                }
            }
            header_row.body(|ui| {
                for text in &finding {
                    ui.label(RichText::new(text).color(if self.refused { ERROR } else { NOTE }));
                }
                self.tab_editor(ui, i, *tab, problem, tools, focus && i == last);
                reached_line(ui, &context.reach, HeldItem::Tab(i));
            });
            match menu {
                Some(true) => remove = Some(i),
                Some(false) => duplicate = Some(i),
                None => {}
            }
        }
        self.defaults_offered(ui, setup);
        ui.horizontal(|ui| {
            if ui.button("Add tab").clicked() {
                let tab = new_tab(self.draft.workholding(), setup.stock.as_ref());
                self.edit(|w| w.tabs.push(tab));
                self.focus_new_tab = true;
            }
            let state = default_tabs_state(&setup.workholding, self.draft.workholding());
            let reason = if setup.part.is_none() {
                Some("Import a model first".to_owned())
            } else if setup.stock.is_none() {
                Some("Set the stock first: held regions and tabs are placed on it.".to_owned())
            } else {
                match state {
                    DefaultTabsState::Refused(why) => Some(why.to_owned()),
                    _ if offered.is_empty() => Some("There is no default tab to add.".to_owned()),
                    _ => None,
                }
            };
            let response = ui.add_enabled(reason.is_none(), egui::Button::new("Add default tabs"));
            let response = match &reason {
                Some(why) => response.on_disabled_hover_text(why),
                None => response.on_hover_text(
                    "One tab at the head and one at the tail, frustums with the small end on \
                     the part, sized from the part",
                ),
            };
            if response.clicked() {
                self.edit(|w| w.tabs.extend(offered.iter().copied()));
            }
        });
        if let Some(i) = duplicate
            && let Some(tab) = self.draft.workholding().tabs.get(i).copied()
        {
            self.edit(|w| w.tabs.insert(i + 1, tab));
            self.fields.clear();
        }
        if let Some(i) = remove {
            self.edit(|w| {
                w.tabs.remove(i);
            });
            self.fields.clear();
        }
    }

    /// The default tabs offered while the setup has none (11.7), as dimmed
    /// read-only rows, with the warning and a line for each zone that has
    /// none.
    fn defaults_offered(&self, ui: &mut egui::Ui, setup: &Setup) {
        if !setup.workholding.tabs.is_empty() || !self.draft.workholding().tabs.is_empty() {
            return;
        }
        ui.label(RichText::new(NO_TABS_WARNING).color(NOTE));
        if setup.part.is_none() || setup.stock.is_none() {
            return;
        }
        match &self.facts {
            Facts::Running { .. } | Facts::None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Working out from the part\u{2026}").color(DARK.muted));
                });
            }
            Facts::Failed { message, .. } => {
                ui.label(RichText::new(message).color(DARK.muted));
            }
            Facts::Found { .. } => {
                let Some(defaults) = self.defaults() else {
                    return;
                };
                for (zone, default) in [Zone::Head, Zone::Tail].into_iter().zip(defaults) {
                    match default {
                        ZoneDefault::Tab(tab) => {
                            let (d0, d1) = match tab.shape {
                                TabShape::Frustum {
                                    start_diameter,
                                    end_diameter,
                                } => (start_diameter.0, end_diameter.0),
                                TabShape::Cylinder { diameter } => (diameter.0, diameter.0),
                                _ => (f64::NAN, f64::NAN),
                            };
                            ui.add_enabled_ui(false, |ui| {
                                ui.label(format!(
                                    "Default tab, not added \u{b7} {} \u{b7} Frustum \u{b7} X {} to {} \u{b7} \u{d8} {} to \u{d8} {} mm",
                                    zone.name(),
                                    mm(tab.x_start.0),
                                    mm(tab.x_start.0 + tab.length.0),
                                    mm(d0),
                                    mm(d1),
                                ));
                            });
                        }
                        other => {
                            if let Some(line) = no_default_line(zone, other, &setup.workholding) {
                                ui.label(RichText::new(line).color(DARK.muted));
                            }
                        }
                    }
                }
            }
        }
    }

    /// One tab's fields (11.7 item 4).
    #[allow(clippy::too_many_arguments)]
    fn tab_editor(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        tab: Tab,
        problem: Option<&FieldProblem>,
        tools: &[SetupTool],
        focus: bool,
    ) {
        let set = |this: &mut Self, f: &dyn Fn(&mut Tab)| {
            this.edit(|w| {
                if let Some(t) = w.tabs.get_mut(i) {
                    f(t);
                }
            });
        };
        let cylinder = matches!(tab.shape, TabShape::Cylinder { .. });
        if let Some(choice) = field_row::choice_row(
            ui,
            &format!("tab {i} shape"),
            "Shape",
            "A frustum keeps the diameter as its diameter at start X and makes the far end 1.5 \
             times it.",
            &["Cylinder", "Frustum"],
            Some(usize::from(!cylinder)),
            true,
        ) {
            set(self, &|t| set_shape(t, choice == 0));
        }
        let key = |name: &str| format!("tab {i} {name}");
        if let Some(Some(v)) = self.number_row(
            ui,
            &key("start"),
            "Start X",
            "mm",
            Some(tab.x_start.0),
            None,
            problem_of(problem, Field::TabStartX(i)),
            focus,
        ) {
            set(self, &|t| t.x_start = Millimetres(v));
        }
        if let Some(Some(v)) = self.number_row(
            ui,
            &key("length"),
            "Length",
            "mm",
            Some(tab.length.0),
            None,
            problem_of(problem, Field::TabLength(i)),
            false,
        ) {
            set(self, &|t| t.length = Millimetres(v));
        }
        match tab.shape {
            TabShape::Cylinder { diameter } => {
                if let Some(Some(v)) = self.number_row(
                    ui,
                    &key("diameter"),
                    "Diameter",
                    "mm",
                    Some(diameter.0),
                    None,
                    problem_of(problem, Field::TabDiameter(i)),
                    false,
                ) {
                    set(self, &|t| {
                        t.shape = TabShape::Cylinder {
                            diameter: Millimetres(v),
                        };
                    });
                }
            }
            TabShape::Frustum {
                start_diameter,
                end_diameter,
            } => {
                if let Some(Some(v)) = self.number_row(
                    ui,
                    &key("start diameter"),
                    "Diameter at start X",
                    "mm",
                    Some(start_diameter.0),
                    None,
                    problem_of(problem, Field::TabStartDiameter(i)),
                    false,
                ) {
                    set(self, &|t| {
                        if let TabShape::Frustum { start_diameter, .. } = &mut t.shape {
                            *start_diameter = Millimetres(v);
                        }
                    });
                }
                if let Some(Some(v)) = self.number_row(
                    ui,
                    &key("end diameter"),
                    "Diameter at end X",
                    "mm",
                    Some(end_diameter.0),
                    None,
                    problem_of(problem, Field::TabEndDiameter(i)),
                    false,
                ) {
                    set(self, &|t| {
                        if let TabShape::Frustum { end_diameter, .. } = &mut t.shape {
                            *end_diameter = Millimetres(v);
                        }
                    });
                }
                if let Some(part) = self.part_extent()
                    && let Some(facing) = part_facing(&tab, (part.bounds.min.x, part.bounds.max.x))
                {
                    let colour = match facing {
                        openchuck_app::workholding_view::PartFacing::Small => DARK.muted,
                        openchuck_app::workholding_view::PartFacing::Large => NOTE,
                    };
                    field_row::note_line(ui, facing.text(), colour);
                }
            }
            _ => {}
        }
        let (line, warning) = narrowest_line(&tab, tools);
        field_row::read_only_row(
            ui,
            "Narrowest",
            "The smallest diameter, compared with the tools of the setup's operations.",
            RichText::new(line).color(if warning { NOTE } else { DARK.text }),
        );
        let around = matches!(tab.layout, TabLayout::AroundAxis { .. });
        if let Some(choice) = field_row::choice_row(
            ui,
            &format!("tab {i} placement"),
            "Placement",
            "On the rotary axis, or several tabs evenly spaced around it.",
            &["On the axis", "Around the axis"],
            Some(usize::from(around)),
            true,
        ) {
            set(self, &|t| set_layout(t, choice == 1));
        }
        if let TabLayout::AroundAxis {
            count,
            start_angle,
            radial_offset,
        } = tab.layout
        {
            let outcome = field_row::text_row(
                ui,
                &mut self.fields,
                &TextRow {
                    key: &key("count"),
                    label: "Count",
                    range: Some("from 1 to 64"),
                    applied: count.to_string(),
                    problem: problem_of(problem, Field::TabCount(i)),
                    enabled: true,
                    ..TextRow::default()
                },
            );
            if let Outcome::Commit(typed) = outcome {
                match typed.trim().parse::<u32>() {
                    Ok(n) => {
                        self.fields.result(&key("count"), Ok(()));
                        set(self, &|t| {
                            if let TabLayout::AroundAxis { count, .. } = &mut t.layout {
                                *count = n;
                            }
                        });
                    }
                    Err(_) => {
                        self.fields
                            .refuse(&key("count"), &typed, "Not a whole number.".to_owned())
                    }
                }
            }
            if let Some(Some(v)) = self.number_row(
                ui,
                &key("angle"),
                "Start angle",
                "\u{b0}",
                Some(start_angle.0),
                None,
                problem_of(problem, Field::TabStartAngle(i)),
                false,
            ) {
                set(self, &|t| {
                    if let TabLayout::AroundAxis { start_angle, .. } = &mut t.layout {
                        start_angle.0 = v;
                    }
                });
            }
            field_row::note_line(ui, &angles_hint(count, start_angle.0), DARK.muted);
            if let Some(Some(v)) = self.number_row(
                ui,
                &key("offset"),
                "Offset from axis",
                "mm",
                Some(radial_offset.0),
                None,
                problem_of(problem, Field::TabOffset(i)),
                false,
            ) {
                set(self, &|t| {
                    if let TabLayout::AroundAxis { radial_offset, .. } = &mut t.layout {
                        *radial_offset = Millimetres(v);
                    }
                });
            }
        }
    }

    /// Check, then Revert and Apply right-aligned with Apply right-most; a
    /// spinner and Cancel in Apply's place while a check runs (11.7 items 6
    /// and 7).
    fn buttons(
        &mut self,
        ui: &mut egui::Ui,
        app: &Application,
        setup: &Setup,
        problem: Option<&FieldProblem>,
    ) -> Option<Command> {
        let changed = self.draft.is_changed();
        let idle = self.checking.is_none();
        let refused_text = self.fields.has_errors();
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
                    let reason = if let Some(p) = problem {
                        p.message.clone()
                    } else if refused_text {
                        "Fix the marked field first.".to_owned()
                    } else if !changed {
                        "Nothing to apply.".to_owned()
                    } else {
                        "Store the held regions and tabs in the setup (it can be undone)".to_owned()
                    };
                    let can = changed && problem.is_none() && !refused_text;
                    let response = ui.add_enabled(can, egui::Button::new("Apply"));
                    let response = if response.enabled() {
                        response.on_hover_text(reason)
                    } else {
                        response.on_disabled_hover_text(reason)
                    };
                    apply = response.clicked()
                        || (can
                            && ui
                                .input(|k| k.key_pressed(egui::Key::Enter) && k.modifiers.command));
                } else {
                    if ui.button("Cancel").clicked()
                        && let Some(checking) = self.checking.take()
                    {
                        // Cancelling the job, not only dropping its result
                        // (OI-334 j).
                        checking.job.cancel();
                    }
                    ui.spinner();
                }
                if ui
                    .add_enabled(changed, egui::Button::new("Revert"))
                    .clicked()
                {
                    self.draft.revert();
                    self.fields.clear();
                    self.findings.clear();
                    self.refused = false;
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
        if let Some(previous) = self.checking.take() {
            previous.job.cancel();
        }
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
        // section is polled only while it is shown): the result is stale.
        if inputs != check_inputs(app, setup) {
            self.message =
                Some("The part, stock or tools changed while checking; check again.".to_owned());
            return None;
        }
        self.finish_check(result.map_err(|e| e.to_string()), apply, draft, setup)
    }

    /// Records what a check found. With `apply` and no findings, the command
    /// that stores `draft` in the setup; otherwise `None`, with the findings
    /// on their tab's rows: in red with the Error mark when applying, in amber
    /// with the Warning mark when checking (11.7).
    fn finish_check(
        &mut self,
        result: Result<Vec<WorkholdingFinding>, String>,
        apply: bool,
        draft: Workholding,
        setup: &Setup,
    ) -> Option<Command> {
        self.refused = apply;
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
                self.findings.clear();
                self.refused = false;
                // The zones and the defaults follow the stored held regions.
                self.facts = Facts::None;
                Some(Command::Replace(Item::Setup(edited)))
            }
            Ok(found) if found.is_empty() => {
                self.findings = vec![WorkholdingFinding {
                    tab: None,
                    text: "The tabs are in the head and tail zones and wide enough for the tools."
                        .to_owned(),
                }];
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

    /// Starts the part-extent job when the section opens and when what it
    /// reads changed, and takes its result when it has finished (11.7 item
    /// 3).
    fn poll_facts(&mut self, ctx: &egui::Context, app: &Application, setup: &Setup) {
        let key = facts_key(app, setup);
        if let Facts::Running { job, key: running } = &self.facts {
            match job.try_result() {
                None => {
                    ctx.request_repaint();
                    if *running == key {
                        return;
                    }
                    job.cancel();
                }
                Some(result) => {
                    let key = running.clone();
                    self.facts = match result {
                        Ok(facts) => Facts::Found { facts, key },
                        Err(e) => Facts::Failed {
                            message: field_row_sentence(&e.to_string()),
                            key,
                        },
                    };
                }
            }
        }
        let current = match &self.facts {
            Facts::Found { key, .. } | Facts::Failed { key, .. } | Facts::Running { key, .. } => {
                Some(key)
            }
            Facts::None => None,
        };
        if setup.part.is_some() && current != Some(&key) {
            self.facts = Facts::Running {
                job: app.start_part_extent(self.setup),
                key,
            };
            ctx.request_repaint();
        }
    }
}

/// An application message as a sentence for the section.
fn field_row_sentence(message: &str) -> String {
    let message = message.trim().trim_end_matches('.');
    let mut chars = message.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
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

    /// The check of the section's draft, run as the section runs it (a job).
    fn check(
        app: &Application,
        panel: &WorkholdingPanel,
    ) -> Result<Vec<WorkholdingFinding>, String> {
        app.start_check_workholding(panel.setup, panel.draft.workholding())
            .wait()
            .map_err(|e| e.to_string())
    }

    #[test]
    fn req_set_09_req_op_21_the_section_stores_a_valid_draft_and_refuses_the_rest() {
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

        // A tab across the part is not in the head or the tail zone: the
        // finding is on the tab's row, and its solid fails in the view.
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
        assert_eq!(panel.findings.len(), 1);
        assert_eq!(panel.findings[0].tab, Some(0));
        assert!(panel.findings[0].text.contains("Tab 1"));
        assert!(panel.draft_view(&setup).failing.contains(&HeldItem::Tab(0)));
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
        // A tab narrower than the 4 mm tool is refused, naming the operation
        // by its number.
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
        let message = &panel.findings[0].text;
        assert!(message.contains("narrower than the tool"), "{message}");
        assert!(message.contains("of 1 Finish"), "{message}");
        assert!(message.starts_with("Tab 1 is 3.000 mm across"), "{message}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The text a frame of the section paints.
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
            panel.show(ui, app, setup, &Context::default());
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

    /// Paints frames until the part-extent job has finished.
    fn settle(
        panel: &mut WorkholdingPanel,
        ctx: &egui::Context,
        app: &Application,
        setup: &Setup,
    ) -> String {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            let text = painted(panel, ctx, app, setup);
            if !matches!(panel.facts, Facts::Running { .. } | Facts::None) {
                return painted(panel, ctx, app, setup);
            }
            assert!(std::time::Instant::now() < deadline, "timed out");
            std::thread::sleep(std::time::Duration::from_millis(5));
            drop(text);
        }
    }

    #[test]
    fn req_ui_25_the_section_shows_the_zones_and_offers_the_defaults_by_zone() {
        let (dir, app, _, setup_id) = cube_project("workholding-frames");
        let setup = app.project().setup(setup_id).unwrap().clone();
        let mut panel = WorkholdingPanel::new(&setup);
        let ctx = egui::Context::default();
        // Nothing stored: the zones and the default tabs are worked out as a
        // job, then offered as defaults with the warning.
        let text = settle(&mut panel, &ctx, &app, &setup);
        // The cube's stock ends where the part does at the high-X end, so
        // the tail zone has no room and no default, and says why.
        assert!(text.contains("Head zone X"), "{text}");
        assert!(text.contains("Tail zone: no room for a tab"), "{text}");
        assert!(text.contains("No tabs are stored."), "{text}");
        assert!(
            text.contains("Default tab, not added \u{b7} Head"),
            "{text}"
        );
        assert!(
            text.contains("No room for a default tab at the tail: the stock ends at the part."),
            "{text}"
        );
        assert!(!text.contains("Not applied"), "{text}");
        assert_eq!(panel.draft_view(&setup).offered.len(), 1);
        assert!(panel.part_extent().is_some());

        // A grip longer than the stock: the plain message under the field,
        // no field path, and the draft is unapplied.
        panel.draft.edit().start = Some(HeldRegion {
            length: Millimetres(500.0),
            diameter: None,
        });
        let text = painted(&mut panel, &ctx, &app, &setup);
        assert!(text.contains("Longer than the stock"), "{text}");
        assert!(!text.contains("setup.workholding"), "{text}");
        assert!(panel.has_unapplied_changes());
        assert!(text.contains("Grip length"), "{text}");
        let view = panel.draft_view(&setup);
        assert!(view.draft.is_some());
        assert_eq!(view.failing, [HeldItem::Start]);

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
        assert!(panel.draft_view(&stored).draft.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn req_ui_25_a_tab_row_says_its_zone_its_facing_and_its_narrowest_against_the_tools() {
        let (dir, app, _, setup_id) = cube_project("workholding-rows");
        let setup = app.project().setup(setup_id).unwrap().clone();
        let mut panel = WorkholdingPanel::new(&setup);
        let ctx = egui::Context::default();
        settle(&mut panel, &ctx, &app, &setup);
        // A 3 mm tab in the head zone, narrower than the 4 mm tool, as a
        // frustum with its large end on the part.
        panel.draft.edit().tabs = vec![Tab {
            shape: TabShape::Frustum {
                start_diameter: Millimetres(3.0),
                end_diameter: Millimetres(5.0),
            },
            ..tab(-7.0, 1.5, 3.0)
        }];
        panel.focus_new_tab = true;
        let text = painted(&mut panel, &ctx, &app, &setup);
        assert!(text.contains("Tab 1 \u{b7} Frustum"), "{text}");
        assert!(text.contains("Head"), "{text}");
        assert!(text.contains("Large end towards the part"), "{text}");
        assert!(text.contains("narrower than the tool"), "{text}");
        assert!(text.contains("of 1 Finish"), "{text}");
        // A reach note names the operation under the tab it reaches.
        let context = Context {
            stored_error: None,
            reach: vec![(
                "1 Finish".to_owned(),
                "The selection 'Top' reaches tab 1, which is never machined. This operation \
                 leaves it uncut there."
                    .to_owned(),
            )],
        };
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(900.0, 1400.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            panel.show(ui, &app, &setup, &context);
        });
        output.textures_delta.clear();
        let text: String = output
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::epaint::Shape::Text(t) => Some(format!("{}\n", t.galley.text())),
                _ => None,
            })
            .collect();
        assert!(
            text.contains("Reached by the selections of 1 Finish."),
            "{text}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn req_ui_25_cancel_cancels_the_check_job() {
        let (dir, app, _, setup_id) = cube_project("workholding-cancel");
        let setup = app.project().setup(setup_id).unwrap().clone();
        let mut panel = WorkholdingPanel::new(&setup);
        panel.start_check(&app, &setup, false);
        let job = panel.checking.as_ref().unwrap().job.id();
        // A second check cancels the first rather than leave it running.
        panel.start_check(&app, &setup, false);
        assert_ne!(panel.checking.as_ref().unwrap().job.id(), job);
        let _ = app.drain_events();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn req_ui_25_messages_are_sentences_and_angles_are_named() {
        assert_eq!(
            field_row_sentence("the setup has no part"),
            "The setup has no part."
        );
        assert_eq!(angles_hint(2, 0.0), "0.00\u{b0}, 180.00\u{b0}");
        assert_eq!(
            angles_hint(3, 90.0),
            "90.00\u{b0}, 210.00\u{b0}, 330.00\u{b0}"
        );
        assert!(reaches(
            "The selection 'A' reaches tab 2, which",
            HeldItem::Tab(1)
        ));
        assert!(!reaches(
            "The selection 'A' reaches tab 12, which",
            HeldItem::Tab(0)
        ));
        assert!(reaches(
            "The selection 'A' reaches the held region at the start of the stock, which",
            HeldItem::Start
        ));
    }
}
