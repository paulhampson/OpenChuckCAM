// SPDX-License-Identifier: AGPL-3.0-or-later
//! The few colours the shell frame needs, from specification section 14.1
//! (dark theme), named as lane X7a's `theme::Tokens` names them.
//!
//! Temporary: X7a's theme module was not on `main` when lane X2 built the
//! frame, so these stand in for it and are replaced by `theme::Tokens` once
//! it is (decision record 0117). Nothing else in the interface uses them.

use eframe::egui::Color32;

const fn rgb(hex: u32) -> Color32 {
    // The masks keep each value in 0..=255, so the casts cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    Color32::from_rgb(
        (hex >> 16) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Title bar and footer.
pub const BAR: Color32 = rgb(0x15_17_1a);
/// Borders.
pub const LINE: Color32 = rgb(0x38_3d_45);
/// Labels and secondary text.
pub const MUTED: Color32 = rgb(0xa3_ab_b5);
/// Section headings and group labels.
pub const HEADING: Color32 = rgb(0xc3_c9_d1);
/// Focus, the active step, the selection bar, links.
pub const ACCENT: Color32 = rgb(0x5a_a9_ff);
/// The selected row.
pub const SELECTED: Color32 = rgb(0x26_44_66);
/// The primary button.
pub const PRIMARY: Color32 = rgb(0x2f_6f_bd);
/// The primary button's border.
pub const PRIMARY_BORDER: Color32 = rgb(0x4a_8f_e0);
/// Up to date, passed checks.
pub const OK: Color32 = rgb(0x4c_c3_8a);
/// Stale, warnings.
pub const WARNING: Color32 = rgb(0xf2_b8_4b);
/// Errors.
pub const ERROR: Color32 = rgb(0xff_7b_72);
