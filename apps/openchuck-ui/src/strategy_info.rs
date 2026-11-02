// SPDX-License-Identifier: AGPL-3.0-or-later
//! The text-only strategy information shared by creation and the inspector.

use crate::theme::DARK;
use eframe::egui::{self, RichText};

pub fn show(ui: &mut egui::Ui, name: &str, explanation: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(name).strong());
        ui.add(egui::Label::new(RichText::new(explanation).color(DARK.muted)).wrap());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_16_req_nfr_15_long_strategy_information_wraps_on_one_left_edge() {
        let ctx = egui::Context::default();
        crate::theme::install_style(&ctx, crate::theme::Theme::Dark);
        let explanation = "A wall can curve along its length. ".repeat(60);
        for width in [200.0, 280.0, 520.0] {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.set_width(width);
                show(ui, "A strategy", &explanation);
            });
            output.textures_delta.clear();
            let text: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some(t),
                    _ => None,
                })
                .collect();
            let heading = text
                .iter()
                .find(|t| t.galley.text() == "A strategy")
                .expect("heading");
            let body = text
                .iter()
                .find(|t| t.galley.text() == explanation)
                .expect("whole explanation");
            assert!((heading.pos.x - body.pos.x).abs() < 0.5);
            assert!(body.pos.y >= heading.pos.y + heading.galley.size().y);
            assert!(body.galley.size().x <= width + 0.5);
            assert!(body.galley.rows.len() > 10);
        }
    }
}
