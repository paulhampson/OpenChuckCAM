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

/// A Simulate tick box's field.
type Tick = fn(&mut SimulateShow) -> &mut bool;

/// Simulate's "Show" tick boxes (7.1, as decision record 0168 proposes):
/// each name, its tooltip and the field it sets.
pub const SIMULATE_SHOW: [(&str, &str, Tick); 3] = [
    ("Part", "The part's model", |s| &mut s.model),
    (
        "Cut part",
        "The stock as the simulation cut it, at the play head; with the colours by \
         deviation on, the part coloured by what was left or overcut",
        |s| &mut s.cut_part,
    ),
    ("Toolpaths", "The toolpaths", |s| &mut s.toolpath),
];

/// Why the one ticked box cannot be unticked (issue 204).
pub const LAST_TICK: &str = "At least one must stay ticked, so the view is never empty";

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
                    >= text_width(ui, "Show \u{23f7}") + button_extra(ui) + simulate_show_width(ui);
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
            if inline_show {
                ui.separator();
                ui.label("Show");
                simulate_ticks(ui, viewport);
            }
        },
    );
}

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
        .map(|(name, ..)| {
            text_width(ui, name)
                + ui.spacing().icon_width
                + ui.spacing().icon_spacing
                + ui.spacing().item_spacing.x
        })
        .sum();
    ui.spacing().item_spacing.x * 3.0 + 1.0 + text_width(ui, "Show") + choices + 8.0
}

/// Simulate's tick boxes, in any combination: the last ticked one is
/// disabled with the reason, so the view is never empty (issue 204).
fn simulate_ticks(ui: &mut egui::Ui, viewport: &mut Viewport) {
    let mut show = viewport.simulate_show();
    let last = show.ticked() <= 1;
    for (name, tip, field) in SIMULATE_SHOW {
        let ticked = *field(&mut show);
        let locked = ticked && last;
        ui.add_enabled(!locked, egui::Checkbox::new(field(&mut show), name))
            .on_hover_text(tip)
            .on_disabled_hover_text(format!("{tip}. {LAST_TICK}"));
    }
    viewport.set_simulate_show(show);
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
fn show_or_hide(ui: &mut egui::Ui, viewport: &mut Viewport) {
    let button = ui.button("Show \u{23f7}").on_hover_text(
        "Show or hide: part, stock, axes, held regions and tabs, selections, toolpaths (V)",
    );
    if button.clicked() {
        viewport.show_menu_open = !viewport.show_menu_open;
    }
    let mut show = viewport.visibility();
    let simulate = viewport.step() == ViewStep::Simulate;
    let mut open = viewport.show_menu_open;
    egui::Popup::from_response(&button)
        .open_bool(&mut open)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            for (name, field) in SHOW_OR_HIDE {
                let main = matches!(name, "Part" | "Stock" | "Toolpaths");
                let last =
                    usize::from(show.part) + usize::from(show.stock) + usize::from(show.toolpaths)
                        == 1;
                let locked = simulate && main && last && *field(&mut show);
                let name = if simulate && name == "Stock" {
                    "Cut part"
                } else {
                    name
                };
                ui.add_enabled(!locked, egui::Checkbox::new(field(&mut show), name))
                    .on_disabled_hover_text(LAST_TICK);
            }
            if simulate {
                ui.separator();
                ui.label("Cut part display");
                let mut solid = viewport.cut_part_solid();
                ui.horizontal(|ui| {
                    ui.radio_value(&mut solid, false, "Translucent");
                    ui.radio_value(&mut solid, true, "Solid");
                });
                viewport.set_cut_part_solid(solid);
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
    fn req_ui_03_req_sim_01_menu_and_quick_controls_toggle_every_combination_and_protect_the_last()
    {
        fn frame(
            ctx: &egui::Context,
            viewport: &mut Viewport,
            events: Vec<egui::Event>,
        ) -> Vec<(String, egui::Rect)> {
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 500.0));
            let input = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| show(ui, viewport, screen.with_max_y(HEIGHT)));
            output.textures_delta.clear();
            fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
                match shape {
                    egui::Shape::Text(t) => out.push((
                        t.galley.text().to_owned(),
                        egui::Rect::from_min_size(t.pos, t.galley.size()),
                    )),
                    egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, out)),
                    _ => {}
                }
            }
            let mut out = Vec::new();
            for shape in output.shapes {
                texts(&shape.shape, &mut out);
            }
            out
        }
        for bits in 1u8..8 {
            for choice in 0..3 {
                for menu in [false, true] {
                    let ctx = egui::Context::default();
                    let mut viewport = Viewport::new(true);
                    viewport.set_step(Some(ViewStep::Simulate));
                    let initial = SimulateShow {
                        model: bits & 1 != 0,
                        cut_part: bits & 2 != 0,
                        toolpath: bits & 4 != 0,
                    };
                    viewport.set_simulate_show(initial);
                    let mut flags = viewport.visibility();
                    flags.axes = false;
                    flags.workholding = false;
                    flags.selections = false;
                    viewport.set_visibility(flags);
                    viewport.show_menu_open = menu;
                    let mut painted = Vec::new();
                    for _ in 0..3 {
                        painted = frame(&ctx, &mut viewport, Vec::new());
                    }
                    let label = ["Part", "Cut part", "Toolpaths"][choice];
                    let choices = painted.iter().filter(|(t, _)| t == label);
                    let at = if menu {
                        choices.max_by(|a, b| a.1.top().total_cmp(&b.1.top()))
                    } else {
                        choices.min_by(|a, b| a.1.top().total_cmp(&b.1.top()))
                    }
                    .unwrap_or_else(|| panic!("missing {label}, menu={menu}: {painted:?}"))
                    .1
                    .center();
                    frame(
                        &ctx,
                        &mut viewport,
                        vec![
                            egui::Event::PointerMoved(at),
                            egui::Event::PointerButton {
                                pos: at,
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                    frame(
                        &ctx,
                        &mut viewport,
                        vec![egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers: egui::Modifiers::NONE,
                        }],
                    );
                    let toggled = bits ^ (1 << choice);
                    let expected = if toggled == 0 {
                        initial
                    } else {
                        SimulateShow {
                            model: toggled & 1 != 0,
                            cut_part: toggled & 2 != 0,
                            toolpath: toggled & 4 != 0,
                        }
                    };
                    assert_eq!(
                        viewport.simulate_show(),
                        expected,
                        "bits={bits}, choice={label}, menu={menu}"
                    );
                    let effective = viewport.visibility();
                    assert_eq!(
                        (effective.part, effective.stock, effective.toolpaths),
                        (expected.model, expected.cut_part, expected.toolpath)
                    );
                    assert!(
                        !effective.axes && !effective.workholding && !effective.selections,
                        "main choices preserve menu-only flags"
                    );
                    // The opposite control renders and acts from exactly the same state.
                    viewport.show_menu_open = !menu;
                    for _ in 0..3 {
                        frame(&ctx, &mut viewport, Vec::new());
                    }
                    assert_eq!(viewport.simulate_show(), expected);
                }
            }
        }
    }

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
        for name in ["Part", "Cut part", "Toolpaths"] {
            let (_, rect) = wide
                .iter()
                .find(|(t, _)| t == name)
                .unwrap_or_else(|| panic!("{name} in the wide strip"));
            assert!(rect.max.x <= 1100.0, "{name} at {rect:?}");
        }
        // Narrow (583 pixels, the strip at 1366 by 768 with the inspector open): none is
        // painted in the strip, and none is cut off.
        let narrow = strip_texts(583.0, false);
        assert!(!narrow.iter().any(|(t, _)| t == "Cut part"));
        assert!(
            narrow.iter().all(|(_, r)| r.max.x <= 583.0 + 1.0),
            "{narrow:?}"
        );
        // They are in the Show menu instead.
        let open = strip_texts(583.0, true);
        assert!(open.iter().any(|(t, _)| t == "Cut part"), "{open:?}");
        assert!(!open.iter().any(|(t, _)| t == "Simulate shows"));
        assert!(!open.iter().any(|(t, _)| t == "Stock"));
        for name in ["Part", "Cut part", "Toolpaths"] {
            assert_eq!(
                open.iter().filter(|(t, _)| t == name).count(),
                1,
                "{name} appears once"
            );
        }
    }

    /// Draws the wide strip on Simulate with `events`, and returns where
    /// each text was painted.
    fn strip_frame(
        ctx: &egui::Context,
        viewport: &mut Viewport,
        events: Vec<egui::Event>,
    ) -> Vec<(String, egui::Rect)> {
        let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1100.0, 400.0));
        let strip = egui::Rect::from_min_size(screen.min, egui::vec2(1100.0, HEIGHT));
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| show(ui, viewport, strip));
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_owned(),
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                )),
                _ => None,
            })
            .collect()
    }

    /// Clicks the tick box labelled `name` in the strip.
    fn click_box(ctx: &egui::Context, viewport: &mut Viewport, name: &str) {
        let texts = strip_frame(ctx, viewport, Vec::new());
        let at = texts
            .iter()
            .find(|(t, _)| t == name)
            .unwrap_or_else(|| panic!("{name} is drawn"))
            .1
            .center();
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        strip_frame(ctx, viewport, vec![egui::Event::PointerMoved(at)]);
        strip_frame(ctx, viewport, vec![button(true)]);
        strip_frame(ctx, viewport, vec![button(false)]);
    }

    #[test]
    fn req_sim_03_simulates_tick_boxes_combine_and_the_last_one_stays_ticked() {
        // Issue 204 (decision record 0168): "Cut part" only at first; any
        // combination; the one ticked box is disabled, so the view is never
        // empty.
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_project_has_models(true);
        viewport.set_step(Some(ViewStep::Simulate));
        assert_eq!(viewport.simulate_show(), SimulateShow::default());
        click_box(&ctx, &mut viewport, "Cut part");
        assert!(
            viewport.simulate_show().cut_part,
            "the last box stays ticked"
        );
        click_box(&ctx, &mut viewport, "Toolpaths");
        click_box(&ctx, &mut viewport, "Part");
        assert_eq!(
            viewport.simulate_show(),
            SimulateShow {
                model: true,
                cut_part: true,
                toolpath: true
            }
        );
        click_box(&ctx, &mut viewport, "Cut part");
        click_box(&ctx, &mut viewport, "Part");
        assert_eq!(
            viewport.simulate_show(),
            SimulateShow {
                model: false,
                cut_part: false,
                toolpath: true
            }
        );
        click_box(&ctx, &mut viewport, "Toolpaths");
        assert!(
            viewport.simulate_show().toolpath,
            "the last box stays ticked"
        );
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
        let names: Vec<&str> = SIMULATE_SHOW.iter().map(|(n, ..)| *n).collect();
        assert_eq!(names, ["Part", "Cut part", "Toolpaths"]);
        // Each box sets its own field.
        for (name, _, field) in SIMULATE_SHOW {
            let mut s = SimulateShow::default();
            *field(&mut s) = !*field(&mut s);
            assert_ne!(s, SimulateShow::default(), "{name}");
        }
    }
}
