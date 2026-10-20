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

/// Why the standard views are unavailable in the shell's current view, if
/// they are (decision 0148 item 2): the same rule for the toolbar, the View
/// menu and the view cube. Every 3D view has them; Setup's two fixed panes
/// have no camera to turn, and with no model there is nothing to look at.
pub const fn directions_unavailable(setup_panes: bool, has_model: bool) -> Option<&'static str> {
    if setup_panes {
        Some("Setup shows fixed front and end views")
    } else if !has_model {
        Some("Import a model first")
    } else {
        None
    }
}

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
            // The toolbar belongs to a 3D view, never to the Setup panes; the
            // buttons follow the View menu by the same rule, on whether the
            // project holds a model (an import preview alone does not count).
            if directions_unavailable(false, viewport.project_has_models()).is_none() {
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
            // Simulate's "Show" choices stay in the strip while they fit after the other
            // controls, else they move into the "Show" menu, so none is cut off (OI-529).
            let inline_show = step == ViewStep::Simulate
                && ui.available_width()
                    >= text_width(ui, "Show ▾") + button_extra(ui) + simulate_show_width(ui);
            let in_menu = step == ViewStep::Simulate && !inline_show;
            show_or_hide(ui, viewport, in_menu);
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
            if inline_show {
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

/// The width of `text` in the body font.
fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE)
        .size()
        .x
}

/// What a button adds to its text: its padding on both sides and the gap to the next widget.
fn button_extra(ui: &egui::Ui) -> f32 {
    ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x
}

/// The room Simulate's "Show" label and choices take in the strip, with their separator.
fn simulate_show_width(ui: &egui::Ui) -> f32 {
    let choices: f32 = SIMULATE_SHOW
        .iter()
        .map(|(_, name)| text_width(ui, name) + ui.spacing().button_padding.x * 2.0)
        .sum();
    ui.spacing().item_spacing.x * 3.0 + 1.0 + text_width(ui, "Show") + choices + 8.0
}

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
fn show_or_hide(ui: &mut egui::Ui, viewport: &mut Viewport, simulate_choices: bool) {
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
            if simulate_choices {
                ui.separator();
                ui.label("Simulate shows");
                segments(ui, &SIMULATE_SHOW_TIPS, &mut viewport.simulate_show);
            }
        });
    viewport.show_menu_open = open;
    viewport.set_visibility(show);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_02_the_view_directions_apply_to_every_3d_view_and_not_to_the_setup_panes() {
        assert_eq!(directions_unavailable(false, true), None);
        assert_eq!(
            directions_unavailable(true, true),
            Some("Setup shows fixed front and end views")
        );
        assert_eq!(
            directions_unavailable(false, false),
            Some("Import a model first")
        );
    }

    /// The texts one frame of the strip paints, `width` wide, on the Simulate step.
    fn strip_texts(width: f32, menu_open: bool) -> Vec<(String, egui::Rect)> {
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_project_has_models(true);
        viewport.set_step(Some(ViewStep::Simulate));
        viewport.show_menu_open = menu_open;
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, 400.0));
        let strip = egui::Rect::from_min_size(screen.min, egui::vec2(width, HEIGHT));
        let mut texts = Vec::new();
        // The popup is placed in the frame after the button is known.
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| show(ui, &mut viewport, strip));
            output.textures_delta.clear();
            texts.clear();
            for clipped in &output.shapes {
                if let egui::Shape::Text(t) = &clipped.shape {
                    texts.push((
                        t.galley.text().to_owned(),
                        egui::Rect::from_min_size(t.pos, t.galley.size()),
                    ));
                }
            }
        }
        texts
    }

    #[test]
    fn req_ui_03_simulates_show_choices_are_never_cut_off_they_move_into_the_show_menu() {
        // OI-529. Wide: the choices are in the strip, each inside it.
        let wide = strip_texts(1100.0, false);
        for name in ["Stock", "Part + path", "Both"] {
            let (_, rect) = wide
                .iter()
                .find(|(t, _)| t == name)
                .unwrap_or_else(|| panic!("{name} in the wide strip"));
            assert!(rect.max.x <= 1100.0, "{name} at {rect:?}");
        }
        // Narrow (583 pixels, the strip at 1366 by 768 with the inspector open): none is
        // painted in the strip, and none is cut off.
        let narrow = strip_texts(583.0, false);
        assert!(!narrow.iter().any(|(t, _)| t == "Part + path"));
        assert!(
            narrow.iter().all(|(_, r)| r.max.x <= 583.0 + 1.0),
            "{narrow:?}"
        );
        // They are in the Show menu instead.
        let open = strip_texts(583.0, true);
        assert!(open.iter().any(|(t, _)| t == "Part + path"), "{open:?}");
        assert!(open.iter().any(|(t, _)| t == "Simulate shows"));
    }

    #[test]
    fn req_ui_02_the_toolbar_and_the_menu_share_one_rule_on_the_projects_models() {
        // OI-514: the rule reads whether the project holds a model, from the one value the
        // shell sets, so an import preview alone (no model in the project) leaves the
        // directions off in both, and a project with a model turns them on in both.
        let mut viewport = Viewport::new(true);
        assert!(!viewport.project_has_models());
        assert_eq!(
            directions_unavailable(false, viewport.project_has_models()),
            Some("Import a model first")
        );
        viewport.set_project_has_models(true);
        assert!(viewport.project_has_models());
        assert_eq!(
            directions_unavailable(false, viewport.project_has_models()),
            None
        );
    }

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
