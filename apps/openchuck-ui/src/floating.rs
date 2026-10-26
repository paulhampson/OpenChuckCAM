// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool dialogs (import, tool library, export) as windows inside the main
//! window, sized to fit it (UI-16; decision record 0164, which replaces item
//! 17 of decision 0029). A dialog is an egui window drawn in the main window's
//! frame, so it may borrow the shell's state, cannot be left behind the main
//! window, and can be dragged only within it. A dialog draws no 3D: a preview
//! stays in the main window's viewport.

use eframe::egui;

/// What the user did to the dialog's window this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowEvent {
    /// Nothing.
    None,
    /// The user closed the dialog's window (its title-bar close button).
    CloseRequested,
}

/// Shows a dialog titled `title`, `size` points large at first, inside the
/// main window. `contents` draws it and its result is returned with what
/// happened to the window. `key` identifies the dialog across frames.
#[cfg(test)]
pub fn floating<T: Default>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    run(ctx, key, title, size, true, true, contents)
}

/// Like [`floating`], but the contents are not put in a scroll area, so a
/// dialog can pin its own footer with a bottom panel and scroll the rest.
pub fn floating_unscrolled<T: Default>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    run(ctx, key, title, size, true, false, contents)
}

/// Like [`floating_unscrolled`], but the window has a fixed size: the user
/// cannot resize it (UI-16). The contents scroll inside it, with a footer
/// pinned if the dialog wants one.
pub fn floating_fixed<T: Default>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    run(ctx, key, title, size, false, false, contents)
}

/// The room a dialog leaves round it inside the main window, in points.
const MARGIN: f32 = 24.0;

/// The size a dialog asking for `size` gets in a main window whose inner area is `inner`, and
/// where its top left goes so that it is centred on the main window's `outer` area. A dialog
/// never opens larger than the window it belongs to (UI-16), and never below the smallest a
/// resizable dialog may be; with an unknown window it keeps the size asked for and the system
/// places it.
pub fn fitted(
    size: [f32; 2],
    inner: Option<egui::Rect>,
    outer: Option<egui::Rect>,
) -> ([f32; 2], Option<egui::Pos2>) {
    let Some(inner) = inner else {
        return (size, None);
    };
    // Never larger than the window, however small it is (UI-16).
    let fit = [
        size[0].min((inner.width() - 2.0 * MARGIN).max(1.0)),
        size[1].min((inner.height() - 2.0 * MARGIN).max(1.0)),
    ];
    let centre = outer.unwrap_or(inner).center();
    (fit, Some(centre - egui::vec2(fit[0], fit[1]) / 2.0))
}

/// The size a dialog asking for `size` gets in the main window `ctx` draws,
/// and the position that centres it.
fn place(ctx: &egui::Context, size: [f32; 2]) -> ([f32; 2], egui::Pos2) {
    let screen = ctx.content_rect();
    let (fit, position) = fitted(size, Some(screen), Some(screen));
    (fit, position.unwrap_or(screen.center()))
}

fn run<T: Default>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    resizable: bool,
    scroll: bool,
    mut contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    let (fit, position) = place(ctx, size);
    let mut open = true;
    let window = egui::Window::new(title)
        .id(egui::Id::new(("dialog", key)))
        .open(&mut open)
        .collapsible(false)
        .default_pos(position);
    let window = if resizable {
        window
            .default_size(fit)
            .min_size([fit[0].min(240.0), fit[1].min(160.0)])
            .resizable(true)
    } else {
        window.fixed_size(fit).resizable(false)
    };
    let shown = window.show(ctx, |ui| {
        crate::pending_edits::form(ctx, egui::Id::new(("dialog form", key)), || {
            if scroll {
                egui::ScrollArea::vertical().show(ui, &mut contents).inner
            } else {
                contents(ui)
            }
        })
    });
    let value = shown.and_then(|r| r.inner).unwrap_or_default();
    let event = if open {
        WindowEvent::None
    } else {
        WindowEvent::CloseRequested
    };
    (value, event)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;

    /// UI-16: a dialog never opens larger than the window it belongs to, and is centred on it.
    #[test]
    fn req_ui_16_a_dialog_is_fitted_inside_the_window_and_centred_on_it() {
        let window = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(1366.0, 768.0));
        let (size, position) = fitted([1200.0, 760.0], Some(window), Some(window));
        assert_eq!(size, [1200.0, 720.0], "the height leaves the margin");
        let at = position.unwrap();
        let dialog = egui::Rect::from_min_size(at, egui::vec2(size[0], size[1]));
        assert!(window.contains_rect(dialog), "{dialog:?} in {window:?}");
        assert_eq!(dialog.center(), window.center());
        // A small dialog keeps its size and is centred.
        let (size, position) = fitted([460.0, 520.0], Some(window), Some(window));
        assert_eq!(size, [460.0, 520.0]);
        assert_eq!(
            egui::Rect::from_min_size(position.unwrap(), egui::vec2(460.0, 520.0)).center(),
            window.center()
        );
        // A tiny window still holds the dialog whole.
        let tiny = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 100.0));
        let (size, position) = fitted([1200.0, 760.0], Some(tiny), None);
        let dialog = egui::Rect::from_min_size(position.unwrap(), egui::vec2(size[0], size[1]));
        assert!(tiny.contains_rect(dialog), "{dialog:?} in {tiny:?}");
        // An unknown window leaves the size and the place to the system.
        assert_eq!(fitted([300.0, 200.0], None, None), ([300.0, 200.0], None));
    }

    #[test]
    fn req_ui_01_floating_dialog_draws_its_contents_inside_the_main_window() {
        // The dialog is an egui window in the main window's own frame
        // (decision 0164): it is drawn, and closing it is left to its
        // buttons and its close box.
        let ctx = egui::Context::default();
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
