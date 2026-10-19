// SPDX-License-Identifier: AGPL-3.0-or-later
//! The colour roles and line widths of the view (interface specification
//! sections 7.4, 7.5 and 11.5; NFR-15). The same role has the same colour
//! everywhere, in both themes, because the viewport stays dark (14.2). The
//! colours of the dark overlays (7.2) are here too, from the dark tokens of
//! [`crate::theme`] (14.1).

use eframe::egui::Color32;
use openchuck_app::pathview::DeviationBand;
use openchuck_viewport::Colour;

use crate::theme;

/// A colour from its hex value, opaque.
const fn hex(rgb: u32) -> Colour {
    with_alpha(rgb, 1.0)
}

/// A colour from a token.
const fn token(c: Color32) -> Colour {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        1.0,
    ]
}

/// A colour from its hex value with an alpha.
#[allow(clippy::cast_precision_loss)]
const fn with_alpha(rgb: u32, alpha: f32) -> Colour {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        alpha,
    ]
}

/// The part, grey `#b7bfca`, shaded.
pub const PART: Colour = hex(0xb7bfca);
/// Feature edges `#d3d9e0`, thin.
pub const FEATURE_EDGES: Colour = hex(0xd3d9e0);
/// The stock outline, blue `#7fb4ea`, dashed.
pub const STOCK: Colour = hex(0x7fb4ea);
/// The rotary axis and the A+ arrow, cyan `#35d0e0` (D1).
pub const ROTARY_AXIS: Colour = hex(0x35d0e0);
/// The work X axis, red `#ff7b72`.
pub const WORK_X: Colour = hex(0xff7b72);
/// The work Y axis, green `#6fd3a8`.
pub const WORK_Y: Colour = hex(0x6fd3a8);
/// The work Z axis, blue `#6fb2ff`.
pub const WORK_Z: Colour = hex(0x6fb2ff);
/// Cut moves, amber `#ffb454`, solid.
pub const CUT: Colour = hex(0xffb454);
/// Link moves, green `#6fd3a8`, solid and thinner.
pub const LINK: Colour = hex(0x6fd3a8);
/// Rapid moves, grey `#9aa6b5`, dotted.
pub const RAPID: Colour = hex(0x9aa6b5);
/// Entry and exit moves, lilac `#e8b6ff`.
pub const ENTRY: Colour = hex(0xe8b6ff);
/// The tool's body: grey.
pub const TOOL_BODY: Colour = hex(0x9aa6b5);
/// The tool's tip (its cutter): lilac.
pub const TOOL_TIP: Colour = hex(0xe8b6ff);
/// The working selection, blue `#3d8bff` (its fill is at 40%).
pub const SELECTED: Colour = hex(0x3d8bff);
/// The hover yellow `#ffe066`.
pub const HOVER: Colour = hex(0xffe066);
/// Avoid and check surfaces, the travel limit and mesh findings: red
/// `#ff7b72`.
pub const AVOID: Colour = hex(0xff7b72);
/// Mesh findings: a red outline.
pub const FINDING: Colour = hex(0xff7b72);
/// The ring round a hole in a mesh: amber, the `warning` token `#f2b84b`.
pub const HOLE: Colour = hex(0xf2b84b);
/// A held region: violet `#9d8df1` at 35%, cross-hatched.
pub const HELD: Colour = with_alpha(0x9d8df1, 0.35);
/// A held region's hatch and outline, violet.
pub const HELD_LINE: Colour = hex(0x9d8df1);
/// A retaining tab, sand `#d8b98a`, opaque and shaded.
pub const TAB: Colour = hex(0xd8b98a);
/// A tab's dark outline `#15171a`.
pub const TAB_LINE: Colour = hex(0x15171a);
/// The `accent` token `#5aa9ff`: the outline of a selected held region or
/// tab.
pub const ACCENT: Colour = token(theme::DARK.accent);
/// The `error` token `#ff7b72`: the outline of a held region or tab of the
/// draft that fails a check.
pub const ERROR: Colour = token(theme::DARK.error);

/// Plain lines and feature edges: 1 pixel (decision 0112 item 3; OI-341).
pub const LINE_WIDTH: f32 = 1.0;
/// The outline of a role, such as a held region or a tab: 1.5 pixels.
pub const OUTLINE_WIDTH: f32 = 1.5;
/// A hovered mark: 2 pixels.
pub const HOVERED_WIDTH: f32 = 2.0;
/// A selected mark: 3 pixels.
pub const SELECTED_WIDTH: f32 = 3.0;
/// Cut and entry moves, wider than links ("Link moves: solid, thinner").
pub const PATH_WIDTH: f32 = 2.0;
/// The work axes, drawn as arrows.
pub const AXIS_WIDTH: f32 = 2.0;

/// Deviation (11.5, D12): overcut, dark magenta `#a3157f`.
pub const OVERCUT: Colour = hex(0xa3157f);
/// Deviation: within tolerance, grey `#9aa0a8`.
pub const WITHIN: Colour = hex(0x9aa0a8);
/// Deviation: material left, small, light blue `#b9d6ff`.
pub const LEFT_SMALL: Colour = hex(0xb9d6ff);
/// Deviation: material left, between the two blues.
pub const LEFT_MEDIUM: Colour = hex(0x7ca7e8);
/// Deviation: material left, more, blue `#3f78d1`.
pub const LEFT_MORE: Colour = hex(0x3f78d1);

/// The colour a deviation band is drawn in: decision 0040's six bands in
/// the colours of section 11.5 (D12, until OI-194 is decided). Not measured
/// is the grey of "within", hatched (see [`band_hatched`]).
pub fn band_colour(band: DeviationBand) -> Colour {
    match band {
        DeviationBand::Overcut => OVERCUT,
        DeviationBand::OnTarget => WITHIN,
        DeviationBand::Thin => LEFT_SMALL,
        DeviationBand::Medium => LEFT_MEDIUM,
        DeviationBand::Thick => LEFT_MORE,
        _ => WITHIN,
    }
}

/// Whether a band is drawn hatched: the triangles not measured, so they
/// read apart from "within tolerance" without hue (D12).
pub fn band_hatched(band: DeviationBand) -> bool {
    !matches!(
        band,
        DeviationBand::Overcut
            | DeviationBand::OnTarget
            | DeviationBand::Thin
            | DeviationBand::Medium
            | DeviationBand::Thick
    )
}

/// The hatch of a band that is not measured: dark lines on the grey.
pub const UNMEASURED_HATCH: Colour = hex(0x5d6570);

/// The top of the view's gradient, `#2c3139` (14.2).
pub const VIEW_TOP: Color32 = theme::VIEWPORT_GRADIENT.0;
/// The bottom of the view's gradient, `#1b1e23`.
pub const VIEW_BOTTOM: Color32 = theme::VIEWPORT_GRADIENT.1;
/// An overlay box's fill (7.2): dark in both themes.
pub const OVERLAY_FILL: Color32 = Color32::from_rgba_premultiplied(20, 22, 25, 235);
/// An overlay box's border.
pub const OVERLAY_BORDER: Color32 = Color32::from_rgb(0x3a, 0x3f, 0x46);
/// Text in an overlay.
pub const OVERLAY_TEXT: Color32 = theme::DARK.text;
/// Secondary text in an overlay.
pub const OVERLAY_MUTED: Color32 = theme::DARK.muted;

/// A role colour for egui, opaque.
pub fn to_color32(c: Colour) -> Color32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(byte(c[0]), byte(c[1]), byte(c[2]))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn req_nfr_15_role_colours_are_the_specified_hex_values() {
        // Spot checks against the table of 7.4 and 11.5.
        assert_eq!(to_color32(ROTARY_AXIS), Color32::from_rgb(0x35, 0xd0, 0xe0));
        assert_eq!(to_color32(PART), Color32::from_rgb(0xb7, 0xbf, 0xca));
        assert_eq!(to_color32(CUT), Color32::from_rgb(0xff, 0xb4, 0x54));
        assert_eq!(to_color32(OVERCUT), Color32::from_rgb(0xa3, 0x15, 0x7f));
        assert_eq!(HELD[3], 0.35);
        // D1: the rotary axis is cyan, not the amber of cut moves.
        assert_ne!(ROTARY_AXIS, CUT);
        // The accent and error outlines are the dark theme's tokens.
        assert_eq!(to_color32(ACCENT), theme::DARK.accent);
        assert_eq!(to_color32(ERROR), theme::DARK.error);
    }

    #[test]
    fn req_nfr_15_widths_follow_one_scale() {
        // Plain 1, role outline 1.5, hovered 2, selected 3 (7.5; OI-341).
        assert_eq!(
            [LINE_WIDTH, OUTLINE_WIDTH, HOVERED_WIDTH, SELECTED_WIDTH],
            [1.0, 1.5, 2.0, 3.0]
        );
    }

    #[test]
    fn req_sim_05_req_nfr_15_deviation_bands_read_by_lightness_and_not_measured_is_hatched() {
        // The bands of material left get darker as more is left, so the scale
        // reads without hue.
        let light = |c: Colour| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let left = [
            DeviationBand::Thin,
            DeviationBand::Medium,
            DeviationBand::Thick,
        ]
        .map(|b| light(band_colour(b)));
        assert!(left[0] > left[1] && left[1] > left[2], "{left:?}");
        // Overcut is the darkest of all.
        assert!(light(band_colour(DeviationBand::Overcut)) < left[2]);
        // Every band but "not measured" is a plain fill.
        for band in DeviationBand::ALL {
            assert_eq!(band_hatched(band), band == DeviationBand::Unmeasured);
        }
    }
}
