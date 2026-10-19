// SPDX-License-Identifier: AGPL-3.0-or-later
//! The viewport toolbar (interface specification 7.1; UI-02, UI-03, SIM-01,
//! SIM-03, SIM-05, IMP-04): a 44-pixel strip above the view, never over it.
//! Its controls change the viewport's own state, so the strip needs nothing
//! from the shell but the viewport. Section, Measure and Turn part are P2
//! (UI-21) and not built.

use eframe::egui;
use openchuck_viewport::{DisplayMode, Projection, StandardView};

use crate::viewport::{SimulateShow, ViewStep, Viewport, Visibility};

/// The strip's height (7.1).
pub const HEIGHT: f32 = 44.0;

/// The display modes with their toolbar names (7.1: Shaded, Wire, Edges).
pub const DISPLAY: [(DisplayMode, &str, &str); 3] = [
    (DisplayMode::Shaded, "Shaded", "Shaded surfaces"),
    (
        DisplayMode::Wireframe,
        "Wire",
        "Every triangle edge, no surfaces",
    ),
    (
        DisplayMode::ShadedWithEdges,
        "Edges",
        "Shaded, with the feature edges at 30°",
    ),
];

/// The projections with their toolbar names.
pub const PROJECTIONS: [(Projection, &str, &str); 2] = [
    (Projection::Perspective, "Persp", "Perspective"),
    (Projection::Orthographic, "Ortho", "Orthographic"),
];

/// The standard views on the toolbar; all seven are in the View menu and on
/// the view cube.
pub const VIEWS: [(StandardView, &str); 3] = [
    (StandardView::Front, "Front"),
    (StandardView::Top, "Top"),
    (StandardView::Isometric, "Iso"),
];

/// The entries of "Show or hide" (7.1, 7.5), with how each reads and
/// writes [`Visibility`].
type Entry = (&'static str, fn(&mut Visibility) -> &mut bool);

/// The entries of "Show or hide", in order.
pub const SHOW_OR_HIDE: [Entry; 6] = [
    ("Part", |v| &mut v.part),
    ("Stock", |v| &mut v.stock),
    ("Rotary axis and work axes", |v| &mut v.axes),
    ("Held regions and tabs", |v| &mut v.workholding),
    ("Selections", |v| &mut v.selections),
    ("Toolpaths", |v| &mut v.toolpaths),
];

/// The line under "Show or hide" (7.5).
pub const SHOW_OR_HIDE_NOTE: &str = "Hiding changes only the view, never what is machined.";

/// Simulate's "Show" choices (7.1).
pub const SIMULATE_SHOW: [(SimulateShow, &str); 3] = [
    (SimulateShow::Stock, "Stock"),
    (SimulateShow::PartAndPath, "Part + path"),
    (SimulateShow::Both, "Both"),
];

/// Draws the strip in `strip` and applies what is clicked. What does not
/// fit is cut off, never pushed over the view or past the window.
pub fn show(ui: &mut egui::Ui, viewport: &mut Viewport, strip: egui::Rect) {
    let step = viewport.step();
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(strip)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.set_clip_rect(strip.intersect(ui.clip_rect()));
            ui.add_space(8.0);
            segments(ui, &DISPLAY, &mut viewport.mode);
            ui.separator();
            let mut projection = viewport.camera.projection;
            segments(ui, &PROJECTIONS, &mut projection);
            viewport.set_projection(projection);
            if matches!(step, ViewStep::Model | ViewStep::Setup) {
                ui.separator();
                for (view, name) in VIEWS {
                    if ui
                        .button(name)
                        .on_hover_text(format!("{} view", view.name()))
                        .clicked()
                    {
                        viewport.set_view(view);
                    }
                }
            }
            ui.separator();
            if ui.button("Fit").on_hover_text("Fit all (F)").clicked() {
                viewport.fit();
            }
            show_or_hide(ui, viewport);
            if step == ViewStep::Model && viewport.has_findings() {
                let mut show = viewport.visibility();
                if ui
                    .toggle_value(&mut show.findings, "Show findings")
                    .on_hover_text("What the mesh check found, outlined on the part")
                    .changed()
                {
                    viewport.set_visibility(show);
                }
            }
            if step == ViewStep::Simulate {
                ui.separator();
                ui.label("Show");
                segments(ui, &SIMULATE_SHOW_TIPS, &mut viewport.simulate_show);
            }
        },
    );
}

/// Simulate's "Show" choices with their tooltips.
const SIMULATE_SHOW_TIPS: [(SimulateShow, &str, &str); 3] = [
    (
        SIMULATE_SHOW[0].0,
        SIMULATE_SHOW[0].1,
        "Only the simulated stock",
    ),
    (
        SIMULATE_SHOW[1].0,
        SIMULATE_SHOW[1].1,
        "The part and the toolpaths, without the stock",
    ),
    (
        SIMULATE_SHOW[2].0,
        SIMULATE_SHOW[2].1,
        "The stock, the part and the toolpaths",
    ),
];

/// A segmented control: one selectable button for each choice.
fn segments<T: Copy + PartialEq>(ui: &mut egui::Ui, choices: &[(T, &str, &str)], value: &mut T) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (choice, name, tip) in choices {
            ui.selectable_value(value, *choice, *name)
                .on_hover_text(*tip);
        }
    });
}

/// The "Show or hide" button and its list (V).
fn show_or_hide(ui: &mut egui::Ui, viewport: &mut Viewport) {
    let button = ui.button("Show ▾").on_hover_text(
        "Show or hide: part, stock, axes, held regions and tabs, selections, toolpaths (V)",
    );
    if button.clicked() {
        viewport.show_menu_open = !viewport.show_menu_open;
    }
    let mut show = viewport.visibility();
    let mut open = viewport.show_menu_open;
    egui::Popup::from_response(&button)
        .open_bool(&mut open)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            for (name, field) in SHOW_OR_HIDE {
                ui.checkbox(field(&mut show), name);
            }
            ui.weak(SHOW_OR_HIDE_NOTE);
        });
    viewport.show_menu_open = open;
    viewport.set_visibility(show);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_02_req_ui_03_the_toolbar_offers_the_specified_controls() {
        // Display and projection by their short names (7.1).
        let names: Vec<&str> = DISPLAY.iter().map(|(_, n, _)| *n).collect();
        assert_eq!(names, ["Shaded", "Wire", "Edges"]);
        let names: Vec<&str> = PROJECTIONS.iter().map(|(_, n, _)| *n).collect();
        assert_eq!(names, ["Persp", "Ortho"]);
        let names: Vec<&str> = VIEWS.iter().map(|(_, n)| *n).collect();
        assert_eq!(names, ["Front", "Top", "Iso"]);
        // Every mode is offered once.
        for mode in DisplayMode::ALL {
            assert_eq!(DISPLAY.iter().filter(|(m, ..)| *m == mode).count(), 1);
        }
        // Show or hide has one entry for held regions and tabs, and each
        // entry turns its own field off.
        for (name, field) in SHOW_OR_HIDE {
            let mut v = Visibility::default();
            *field(&mut v) = false;
            assert_ne!(v, Visibility::default(), "{name}");
        }
        assert_eq!(
            SHOW_OR_HIDE
                .iter()
                .filter(|(n, _)| *n == "Held regions and tabs")
                .count(),
            1
        );
        let names: Vec<&str> = SIMULATE_SHOW.iter().map(|(_, n)| *n).collect();
        assert_eq!(names, ["Stock", "Part + path", "Both"]);
    }
}
