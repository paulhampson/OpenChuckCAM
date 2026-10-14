// SPDX-License-Identifier: AGPL-3.0-or-later
//! Dialogs in their own operating-system windows, so they can be moved
//! outside the main window, for example to a second screen (OI-130; decision
//! record 0029 item 17).
//!
//! A floating dialog is an egui *immediate* viewport: it is drawn in the same
//! frame as the main window and may borrow the shell's state. Where the
//! platform cannot open another window, egui draws it as an ordinary window
//! inside the main one instead, so the dialog always works. A dialog draws
//! no 3D: a preview stays in the main window's viewport, so the renderer
//! needs no second paint path.

use eframe::egui::{self, ViewportBuilder, ViewportClass, ViewportId};

/// What the user did to the dialog's own window this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowEvent {
    /// Nothing.
    None,
    /// The user closed the dialog's window (its title-bar close button).
    CloseRequested,
}

/// Shows a dialog titled `title`, `size` points large at first, in its own
/// window. `contents` draws it and its result is returned with what happened
/// to the window. `key` identifies the dialog across frames.
pub fn floating<T>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    mut contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    let builder = ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(size)
        .with_min_inner_size([240.0, 160.0]);
    ctx.show_viewport_immediate(ViewportId::from_hash_of(key), builder, |ui, class| {
        let value = match class {
            // Already inside an egui window that egui made for us.
            ViewportClass::EmbeddedWindow => contents(ui),
            _ => {
                egui::CentralPanel::default()
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, &mut contents).inner
                    })
                    .inner
            }
        };
        let event = if class != ViewportClass::EmbeddedWindow
            && ui.ctx().input(|i| i.viewport().close_requested())
        {
            WindowEvent::CloseRequested
        } else {
            WindowEvent::None
        };
        (value, event)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_01_floating_dialog_draws_its_contents_where_windows_are_embedded() {
        // A bare egui context embeds viewports, as a platform without
        // multiple windows does: the dialog is still drawn, inside the main
        // window, and closing it is left to its own buttons.
        let ctx = egui::Context::default();
        assert!(ctx.embed_viewports());
        let mut drawn = 0;
        let mut event = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                let (value, e) = floating(ui.ctx(), "test dialog", "Test", [300.0, 200.0], |ui| {
                    ui.label("inside");
                    7
                });
                drawn += value;
                event = Some(e);
            });
            // No renderer here takes the font texture; egui asks for it to
            // be dropped on purpose.
            output.textures_delta.clear();
        }
        assert_eq!(drawn, 14);
        assert_eq!(event, Some(WindowEvent::None));
    }
}
