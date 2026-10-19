// SPDX-License-Identifier: AGPL-3.0-or-later
//! The viewport's toolbar strip and its legend strip (specification sections
//! 3 and 7.1). The shell gives each its region; what they hold is lane X3's
//! (decision record 0114 C5 and C6, decision record 0117), which replaces
//! these bodies. Until then the strip holds the view controls that were in
//! the View menu, which keeps them too (UI-14).

use eframe::egui;
use openchuck_app::overview::Step;
use openchuck_viewport::{DisplayMode, Projection, StandardView};

use crate::viewport::Viewport;

/// The 44-pixel strip over the view: display mode, projection, the standard
/// views and Fit all.
pub fn strip(ui: &mut egui::Ui, viewport: &mut Viewport, _step: Step) {
    ui.horizontal_centered(|ui| {
        for mode in DisplayMode::ALL {
            ui.selectable_value(&mut viewport.mode, mode, mode.name());
        }
        ui.separator();
        let mut projection = viewport.camera.projection;
        ui.selectable_value(&mut projection, Projection::Perspective, "Persp")
            .on_hover_text("Perspective");
        ui.selectable_value(&mut projection, Projection::Orthographic, "Ortho")
            .on_hover_text("Orthographic");
        viewport.set_projection(projection);
        ui.separator();
        for view in StandardView::ALL {
            if ui.button(view.name()).clicked() {
                viewport.set_view(view);
            }
        }
        ui.separator();
        if ui.button("Fit all").clicked() {
            viewport.fit();
        }
    });
}

/// Whether `step` has a legend strip under the view (Model and Setup, where
/// there are no paths). None is drawn yet: the legends come with the colour
/// roles of section 7.4.
pub fn has_legend(_step: Step) -> bool {
    false
}

/// The 36-pixel legend strip under the view on Model and Setup.
pub fn legend(_ui: &mut egui::Ui, _step: Step) {}
