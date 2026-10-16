// SPDX-License-Identifier: AGPL-3.0-or-later
//! A setup's workholding and retaining tabs (SET-09, OP-21; UI-03, UI-07;
//! work breakdown slice 21c; decision record 0045).
//!
//! The panel edits a draft of the setup's held regions and tabs. The draft is
//! checked as it changes, with the reason shown under it (UI-07), and applied
//! with one undoable edit of the setup (UI-08). The checks that need the part
//! and the tools (the head and tail zones, a tab narrower than a tool) come
//! from the setup report. The default tabs are computed from the part as a
//! job and added to the draft, to be applied or changed.

use eframe::egui::{self, Color32, RichText};
use openchuck_app::project::{Item, Setup};
use openchuck_app::setup::{
    HeldRegion, Millimetres, Tab, TabLayout, TabShape, Workholding, validate_workholding,
};
use openchuck_app::{Application, Command, JobHandle};

const ERROR: Color32 = Color32::from_rgb(230, 80, 80);
const NOTE: Color32 = Color32::from_rgb(230, 180, 60);

/// The workholding panel's state: the draft being edited.
pub struct WorkholdingPanel {
    setup: openchuck_app::project::id::SetupId,
    /// The workholding as the project has it.
    base: Workholding,
    /// The workholding being edited.
    draft: Workholding,
    /// The default tabs being computed.
    defaults: Option<JobHandle<Vec<Tab>>>,
    /// A check against the part and tools that is running.
    checking: Option<Checking>,
    /// What the last check of the setup said.
    findings: Vec<String>,
    message: Option<String>,
}

/// A running check of a draft against the part and the tools. It reads the
/// part, so it is a job, not a call in the frame.
struct Checking {
    job: JobHandle<Vec<String>>,
    /// True to store the draft if the check finds nothing.
    apply: bool,
    /// The draft as it was when the check started.
    draft: Workholding,
}

impl std::fmt::Debug for WorkholdingPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkholdingPanel")
            .field("setup", &self.setup)
            .finish_non_exhaustive()
    }
}

impl WorkholdingPanel {
    /// A panel for `setup`, starting from its stored workholding.
    pub fn new(setup: &Setup) -> Self {
        Self {
            setup: setup.id,
            base: setup.workholding.clone(),
            draft: setup.workholding.clone(),
            defaults: None,
            checking: None,
            findings: Vec::new(),
            message: None,
        }
    }

    /// The setup the panel is for.
    pub fn setup(&self) -> openchuck_app::project::id::SetupId {
        self.setup
    }

    /// Draws the panel. Returns the command to execute when the user applies
    /// the draft.
    pub fn show(&mut self, ui: &mut egui::Ui, app: &Application, setup: &Setup) -> Option<Command> {
        // Another edit (an undo, the command line) changed the project: start
        // again from what it holds.
        if setup.workholding != self.base {
            self.base = setup.workholding.clone();
            self.draft = setup.workholding.clone();
            self.findings.clear();
        }
        ui.heading(format!("Workholding: {}", setup.name));
        ui.weak(
            "Held regions are never machined and links keep clear of them; tabs keep the \
             part joined to the stock. Both stay in the simulated stock and count as part \
             of the finished surface.",
        );
        ui.separator();
        self.poll_defaults(ui.ctx());
        let command = self.poll_check(ui.ctx(), setup);

        held(ui, "Held at the start of the stock", &mut self.draft.start);
        held(ui, "Held at the end of the stock", &mut self.draft.end);
        ui.separator();
        ui.strong(format!("Retaining tabs ({})", self.draft.tabs.len()));
        let mut remove = None;
        for (i, tab) in self.draft.tabs.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                if tab_editor(ui, i, tab) {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.draft.tabs.remove(i);
        }
        ui.horizontal(|ui| {
            if ui.button("Add tab").clicked() {
                self.draft.tabs.push(new_tab(setup));
            }
            let busy = self.defaults.is_some();
            ui.add_enabled_ui(!busy && self.draft.tabs.is_empty(), |ui| {
                if ui
                    .button("Add default tabs")
                    .on_hover_text(
                        "One tab at the head and one at the tail, frustums with the small end \
                         on the part, sized from the part",
                    )
                    .clicked()
                {
                    // The held regions fix the zones, so compute from the
                    // draft's regions as stored; apply those first if they
                    // changed.
                    self.message = None;
                    self.defaults = Some(app.start_default_tabs(self.setup));
                }
            });
            if busy {
                ui.spinner();
            }
        });
        if self.draft.start != self.base.start || self.draft.end != self.base.end {
            ui.weak("Apply the held regions first: the default tabs fit the stored ones.");
        }
        ui.separator();

        // The reason the draft cannot be stored, under it (UI-07).
        let invalid = validate_workholding(&self.draft, setup.stock.as_ref(), "setup").err();
        if let Some(e) = &invalid {
            ui.label(RichText::new(e.to_string()).color(ERROR));
        }
        if let Some(m) = &self.message {
            ui.label(RichText::new(m).color(ERROR));
        }
        let changed = self.draft != self.base;
        let idle = self.checking.is_none();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    changed && invalid.is_none() && idle,
                    egui::Button::new("Apply"),
                )
                .on_hover_text("Store the held regions and tabs in the setup (it can be undone)")
                .clicked()
            {
                self.start_check(app, true);
            }
            if ui
                .add_enabled(changed, egui::Button::new("Revert"))
                .clicked()
            {
                self.draft = self.base.clone();
            }
            if ui
                .add_enabled(idle, egui::Button::new("Check against the part and tools"))
                .on_hover_text("Tabs lie in the head or tail zone and are wider than the tools")
                .clicked()
            {
                self.start_check(app, false);
            }
            if !idle {
                ui.spinner();
            }
        });
        for f in &self.findings {
            ui.label(RichText::new(f).color(NOTE));
        }
        command
    }

    /// Starts checking the draft against the part and the tools: tabs outside
    /// the head and tail zones, and tabs narrower than a tool the setup uses,
    /// are refused when stored, as they are when generating (OI-159, OI-180).
    fn start_check(&mut self, app: &Application, apply: bool) {
        self.message = None;
        self.checking = Some(Checking {
            job: app.start_check_workholding(self.setup, &self.draft),
            apply,
            draft: self.draft.clone(),
        });
    }

    /// Takes the result of a finished check. Returns the command that stores
    /// the draft the check was made on, if it was asked to and found nothing.
    fn poll_check(&mut self, ctx: &egui::Context, setup: &Setup) -> Option<Command> {
        let checking = self.checking.as_ref()?;
        let Some(result) = checking.job.try_result() else {
            ctx.request_repaint();
            return None;
        };
        let Checking { apply, draft, .. } = self.checking.take()?;
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
                self.findings =
                    vec!["The tabs are in the head and tail zones and wide enough.".to_owned()];
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

    /// Adds the default tabs to the draft when their job has finished.
    fn poll_defaults(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.defaults else { return };
        match job.try_result() {
            None => ctx.request_repaint(),
            Some(result) => {
                self.defaults = None;
                match result {
                    Ok(tabs) if tabs.is_empty() => {
                        self.message = Some(
                            "There is no room for a tab between the held regions and the part."
                                .to_owned(),
                        );
                    }
                    Ok(tabs) => self.draft.tabs.extend(tabs),
                    Err(e) => self.message = Some(e.to_string()),
                }
            }
        }
    }
}

/// A new tab: a short cylinder at the start of the stock, to be moved.
fn new_tab(setup: &Setup) -> Tab {
    let (x_start, diameter) = match &setup.stock {
        Some(openchuck_app::setup::Stock::Cylinder {
            x_start, diameter, ..
        }) => (x_start.0, (diameter.0 / 4.0).max(1.0)),
        _ => (0.0, 4.0),
    };
    Tab {
        x_start: Millimetres(x_start),
        length: Millimetres(5.0),
        shape: TabShape::Cylinder {
            diameter: Millimetres(diameter),
        },
        layout: TabLayout::OnAxis,
    }
}

fn mm(ui: &mut egui::Ui, value: &mut f64, speed: f64) -> egui::Response {
    ui.add(
        egui::DragValue::new(value)
            .speed(speed)
            .suffix(" mm")
            .max_decimals(3),
    )
}

/// A held region: a switch, its length and an optional holding diameter.
fn held(ui: &mut egui::Ui, label: &str, region: &mut Option<HeldRegion>) {
    ui.horizontal(|ui| {
        let mut on = region.is_some();
        if ui.checkbox(&mut on, label).changed() {
            *region = on.then_some(HeldRegion {
                length: Millimetres(10.0),
                diameter: None,
            });
        }
        if let Some(r) = region {
            ui.label("grips");
            mm(ui, &mut r.length.0, 0.1);
            let mut narrower = r.diameter.is_some();
            if ui
                .checkbox(&mut narrower, "holding diameter")
                .on_hover_text("Where the grip is narrower than the stock")
                .changed()
            {
                r.diameter = narrower.then_some(Millimetres(20.0));
            }
            if let Some(d) = &mut r.diameter {
                mm(ui, &mut d.0, 0.1);
            }
        }
    });
}

/// One tab's fields. Returns true when the user removed it.
fn tab_editor(ui: &mut egui::Ui, index: usize, tab: &mut Tab) -> bool {
    let mut remove = false;
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.strong(format!("Tab {}", index + 1));
            let mut cylinder = matches!(tab.shape, TabShape::Cylinder { .. });
            egui::ComboBox::from_id_salt("shape")
                .selected_text(if cylinder { "Cylinder" } else { "Frustum" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cylinder, true, "Cylinder");
                    ui.selectable_value(&mut cylinder, false, "Frustum");
                });
            tab.shape = match (cylinder, tab.shape) {
                (true, shape @ TabShape::Cylinder { .. }) => shape,
                (true, TabShape::Frustum { start_diameter, .. }) => TabShape::Cylinder {
                    diameter: start_diameter,
                },
                (false, shape @ TabShape::Frustum { .. }) => shape,
                (false, TabShape::Cylinder { diameter }) => TabShape::Frustum {
                    start_diameter: diameter,
                    end_diameter: Millimetres(diameter.0 * 1.5),
                },
                (_, shape) => shape,
            };
            if ui.small_button("Remove").clicked() {
                remove = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("from X");
            mm(ui, &mut tab.x_start.0, 0.1);
            ui.label("length");
            mm(ui, &mut tab.length.0, 0.1);
        });
        ui.horizontal(|ui| match &mut tab.shape {
            TabShape::Cylinder { diameter } => {
                ui.label("diameter");
                mm(ui, &mut diameter.0, 0.1);
            }
            TabShape::Frustum {
                start_diameter,
                end_diameter,
            } => {
                ui.label("diameter at the start");
                mm(ui, &mut start_diameter.0, 0.1);
                ui.label("at the far end");
                mm(ui, &mut end_diameter.0, 0.1);
            }
            _ => {}
        });
        ui.horizontal(|ui| {
            let mut around = matches!(tab.layout, TabLayout::AroundAxis { .. });
            ui.radio_value(&mut around, false, "On the axis");
            ui.radio_value(&mut around, true, "Around the axis");
            tab.layout = match (around, tab.layout) {
                (false, TabLayout::AroundAxis { .. }) => TabLayout::OnAxis,
                (true, TabLayout::OnAxis) => TabLayout::AroundAxis {
                    count: 2,
                    start_angle: openchuck_app::model::Degrees(0.0),
                    radial_offset: Millimetres(5.0),
                },
                (_, layout) => layout,
            };
            if let TabLayout::AroundAxis {
                count,
                start_angle,
                radial_offset,
            } = &mut tab.layout
            {
                ui.label("count");
                ui.add(egui::DragValue::new(count).range(1..=64));
                ui.label("first at");
                ui.add(
                    egui::DragValue::new(&mut start_angle.0)
                        .speed(1.0)
                        .suffix("°"),
                );
                ui.label("from the axis");
                mm(ui, &mut radial_offset.0, 0.1);
            }
        });
    });
    remove
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
        app.start_check_workholding(panel.setup, &panel.draft)
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
        panel.draft.start = Some(HeldRegion {
            length: Millimetres(1.0),
            diameter: None,
        });
        panel.draft.tabs = vec![tab(-6.0, 1.0, 4.0)];
        let command = panel
            .finish_check(check(&app, &panel), true, panel.draft.clone(), &setup)
            .expect("a valid draft");
        app.execute(command).unwrap();
        assert_eq!(
            app.project().setup(setup_id).unwrap().workholding,
            panel.draft
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
        panel.draft.tabs = vec![tab(-1.0, 2.0, 6.0)];
        assert!(
            panel
                .finish_check(check(&app, &panel), true, panel.draft.clone(), &setup)
                .is_none()
        );
        let message = panel.message.clone().unwrap();
        assert!(message.contains("tab 1"), "{message}");
        // A tab narrower than the 4 mm tool is refused, naming the operation.
        panel.draft.tabs = vec![tab(-6.0, 1.0, 3.0)];
        assert!(
            panel
                .finish_check(check(&app, &panel), true, panel.draft.clone(), &setup)
                .is_none()
        );
        let message = panel.message.clone().unwrap();
        assert!(message.contains("narrower than the tool"), "{message}");
        assert!(message.contains("Finish"), "{message}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
