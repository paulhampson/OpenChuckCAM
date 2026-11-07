// SPDX-License-Identifier: AGPL-3.0-or-later
//! The application's own icon on the window and the taskbar (decision 0542). The 256-pixel
//! PNG is embedded at compile time; `cargo xtask icon` makes it from
//! `assets/icon/icon.svg`. The Windows program file's icon is linked in by `build.rs`.

use std::sync::Arc;

/// The 256 by 256 icon, embedded in the program.
pub const ICON_256_PNG: &[u8] = include_bytes!("../assets/icon/icon-256.png");

/// The window icon, or `None` if the embedded PNG cannot be decoded; the window then keeps
/// the toolkit's default icon rather than failing to open (NFR-07).
pub fn window_icon() -> Option<Arc<egui::IconData>> {
    eframe::icon_data::from_png_bytes(ICON_256_PNG)
        .ok()
        .map(Arc::new)
}

/// `viewport` with the application's icon, when it decodes.
pub fn with_icon(viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
    match window_icon() {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_window_icon_is_the_embedded_256_pixel_image() {
        let icon = window_icon().expect("the embedded icon decodes");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        // Not the toolkit's default: it is the same image as the committed PNG file.
        let committed = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/icon/icon-256.png"
        ))
        .unwrap();
        assert_eq!(committed, ICON_256_PNG);
    }

    #[test]
    fn icon_window_viewport_carries_the_icon() {
        let viewport = with_icon(egui::ViewportBuilder::default());
        let icon = viewport.icon.expect("the window has an icon");
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}
