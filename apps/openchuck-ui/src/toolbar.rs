// SPDX-License-Identifier: AGPL-3.0-or-later
//! The legend strip under the view on Model and Setup (specification
//! sections 3 and 7.5). The toolbar strip over the view is the viewport's
//! own (`viewport_toolbar`; decision record 0119).

use eframe::egui::{self, vec2};
use openchuck_app::overview::Step;
use openchuck_app::scene::SceneView;

use crate::viewport_overlays::{swatch_for, workholding_entries};

/// Whether `step` has a legend strip under the view: Model and Setup, where
/// there are no paths, when the setup has a held region or a tab (7.5).
pub fn has_legend(step: Step, view: &SceneView) -> bool {
    matches!(step, Step::Model | Step::Setup) && !workholding_entries(view).is_empty()
}

/// The 36-pixel legend strip: each role present, with a swatch drawn the
/// way the role is drawn (7.5).
pub fn legend(ui: &mut egui::Ui, view: &SceneView) {
    ui.horizontal_centered(|ui| {
        for entry in workholding_entries(view) {
            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
            swatch_for(ui.painter(), r, entry, false);
            ui.label(entry.name());
            ui.add_space(12.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::tests::held_and_tab;

    #[test]
    fn req_ui_25_the_legend_strip_lists_held_regions_and_tabs_on_model_and_setup() {
        let view = held_and_tab();
        // Model and Setup have the strip when the setup has a held region or
        // a tab; Operations and Simulate list them in the view's legends
        // (7.5).
        assert!(has_legend(Step::Model, &view));
        assert!(has_legend(Step::Setup, &view));
        assert!(!has_legend(Step::Operations, &view));
        assert!(!has_legend(Step::Simulate, &view));
        assert!(!has_legend(Step::Export, &view));
        assert!(!has_legend(Step::Setup, &SceneView::default()));
        // It names each role, after its swatch.
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| legend(ui, &view));
        // No renderer here takes the font texture.
        output.textures_delta.clear();
        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["Held region", "Tab"]);
    }
}
