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
#[cfg(test)]
pub fn floating<T>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    show(ctx, key, title, size, true, contents)
}

/// Like [`floating`], but the contents are not put in a scroll area, so a
/// dialog can pin its own footer with a bottom panel and scroll the rest.
pub fn floating_unscrolled<T>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    show(ctx, key, title, size, false, contents)
}

/// Like [`floating_unscrolled`], but the window has a fixed size: the user
/// cannot resize or maximise it (UI-16). The contents scroll inside it, with
/// a footer pinned if the dialog wants one.
pub fn floating_fixed<T>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    let builder = placed(ctx, title, size, false);
    run(ctx, key, builder, false, contents)
}

/// The viewport a dialog with this `key` is shown in.
pub fn viewport_id(key: &str) -> ViewportId {
    ViewportId::from_hash_of(key)
}

/// Tells when the application comes back to the front after it was in the
/// background, so a dialog hidden behind the main window can be raised again
/// (OI-430). The application is active while any of its windows has the
/// focus; moving between its own windows is not a return. A window whose
/// focus the platform does not report counts as active, so nothing is raised
/// where nothing can be known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusReturn {
    was_active: bool,
}

impl FocusReturn {
    /// Starts out active, so opening a dialog does not raise it again.
    pub fn new() -> Self {
        Self { was_active: true }
    }

    /// Takes the focus of the main window and of the dialog this frame;
    /// returns true on the frame the application becomes active again.
    pub fn update(&mut self, main: Option<bool>, dialog: Option<bool>) -> bool {
        let active = main.unwrap_or(true) || dialog.unwrap_or(true);
        let returned = active && !self.was_active;
        self.was_active = active;
        returned
    }
}

impl Default for FocusReturn {
    fn default() -> Self {
        Self::new()
    }
}

/// Raises the dialog with this `key` above the main window. Does nothing
/// where dialogs are drawn inside the main window.
pub fn raise(ctx: &egui::Context, key: &str) {
    if !ctx.embed_viewports() {
        ctx.send_viewport_cmd_to(viewport_id(key), egui::ViewportCommand::Focus);
    }
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

/// `size` fitted to the main window `ctx` is drawing, and the position that centres it.
fn fit_to_window(ctx: &egui::Context, size: [f32; 2]) -> ([f32; 2], Option<egui::Pos2>) {
    let (inner, outer) = ctx.input(|i| {
        let v = i.viewport();
        (v.inner_rect, v.outer_rect)
    });
    fitted(size, inner, outer)
}

/// A dialog's viewport, `size` fitted and centred on the window `ctx` draws (UI-16).
fn placed(ctx: &egui::Context, title: &str, size: [f32; 2], resizable: bool) -> ViewportBuilder {
    let (size, position) = fit_to_window(ctx, size);
    let builder = builder(title, size, resizable);
    match position {
        Some(p) => builder.with_position(p),
        None => builder,
    }
}

fn builder(title: &str, size: [f32; 2], resizable: bool) -> ViewportBuilder {
    let builder = ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(size);
    if resizable {
        builder.with_min_inner_size([size[0].min(240.0), size[1].min(160.0)])
    } else {
        builder
            .with_resizable(false)
            .with_maximize_button(false)
            .with_min_inner_size(size)
            .with_max_inner_size(size)
    }
}

fn show<T>(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    size: [f32; 2],
    scroll: bool,
    contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    run(ctx, key, placed(ctx, title, size, true), scroll, contents)
}

fn run<T>(
    ctx: &egui::Context,
    key: &str,
    builder: ViewportBuilder,
    scroll: bool,
    mut contents: impl FnMut(&mut egui::Ui) -> T,
) -> (T, WindowEvent) {
    ctx.show_viewport_immediate(viewport_id(key), builder, |ui, class| {
        let value = match class {
            // Already inside an egui window that egui made for us.
            ViewportClass::EmbeddedWindow => contents(ui),
            _ => {
                egui::CentralPanel::default()
                    .show(ui, |ui| {
                        if scroll {
                            egui::ScrollArea::vertical().show(ui, &mut contents).inner
                        } else {
                            contents(ui)
                        }
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
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;

    /// A fixed dialog cannot be resized or maximised and is exactly the
    /// size asked for; the others stay resizable (UI-16).
    #[test]
    fn req_ui_16_a_fixed_dialog_has_one_size() {
        let fixed = builder("Fixed", [440.0, 540.0], false);
        assert_eq!(fixed.resizable, Some(false));
        assert_eq!(fixed.maximize_button, Some(false));
        assert_eq!(fixed.min_inner_size, fixed.inner_size);
        assert_eq!(fixed.max_inner_size, fixed.inner_size);
        let loose = builder("Loose", [440.0, 540.0], true);
        assert_ne!(loose.resizable, Some(false));
    }

    /// The dialog is raised once, when the application returns from the
    /// background (OI-430), not when focus moves between its own windows.
    #[test]
    fn req_ui_14_the_dialog_is_raised_when_the_application_returns() {
        let mut tracker = FocusReturn::new();
        assert!(!tracker.update(Some(true), Some(false)), "main in front");
        assert!(!tracker.update(Some(false), Some(true)), "to the dialog");
        assert!(!tracker.update(Some(true), Some(false)), "back to main");
        assert!(
            !tracker.update(Some(false), Some(false)),
            "to the background"
        );
        assert!(!tracker.update(Some(false), Some(false)), "still away");
        assert!(tracker.update(Some(true), Some(false)), "returned to main");
        assert!(!tracker.update(Some(true), Some(false)), "only once");
        // Unknown focus never counts as a return.
        let mut unknown = FocusReturn::new();
        assert!(!unknown.update(None, None));
        assert!(!unknown.update(None, None));
    }

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
