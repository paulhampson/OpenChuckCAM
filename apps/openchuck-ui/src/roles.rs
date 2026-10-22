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
/// The hover yellow `#ffe066`: the triangle or face under the pointer.
pub const HOVER: Colour = hex(0xffe066);
/// The edge under the pointer, magenta `#ff4fd8`: a colour of its own, so an
/// edge reads apart from a triangle or face (issue 151; specification 7.4).
pub const EDGE_HOVER: Colour = hex(0xff4fd8);
/// An edge in the working selection, orange `#ff6a00`: orange against the
/// blue of selected triangles and faces is a pair the common colour
/// blindnesses keep apart (issue 151; specification 7.4).
pub const EDGE_SELECTED: Colour = hex(0xff6a00);
/// Avoid and check surfaces, the travel limit and mesh findings: red
/// `#ff7b72`.
pub const AVOID: Colour = hex(0xff7b72);
/// The clearance cylinder and the safe Z, green `#4cc38a`, dashed.
pub const CLEARANCE: Colour = hex(0x4cc38a);
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

/// Deviation (11.5; decision 0216 item 1): overcut, vermilion `#df5230`,
/// on the warm side of the scale, opposite the blues of material left for
/// every colour vision type.
pub const OVERCUT: Colour = hex(0xdf5230);
/// Deviation: within tolerance, grey `#9aa0a8`.
pub const WITHIN: Colour = hex(0x9aa0a8);
/// Deviation: material left, up to 0.25 mm, ice `#c6f9ff`.
pub const LEFT_SMALL: Colour = hex(0xc6f9ff);
/// Deviation: material left, 0.25 to 1 mm, sky blue `#4bb7fc`.
pub const LEFT_MEDIUM: Colour = hex(0x4bb7fc);
/// Deviation: material left, more than 1 mm or a bound, strong blue
/// `#4e71ea`, the darkest that keeps 3:1 against the view (section 16).
pub const LEFT_MORE: Colour = hex(0x4e71ea);

/// A deviation band the user hid (SIM-15): solid dark slate `#5d6570`, a
/// colour of its own, so a hidden band never reads as "within tolerance" or
/// as the part's grey; solid, so it reads apart from "not measured", which
/// is hatched (decision 0216 item 8; OI-882).
pub const HIDDEN_BAND: Colour = hex(0x5d6570);

/// A shown selection set's outline (decision 0216 item 4): 2 pixels in the
/// set's colour inside 4 pixels of [`SET_FRAME`], round the edge of the set.
/// The deviation scale never draws outlines, so a set reads apart from it
/// without colour.
pub const SET_OUTLINE_WIDTH: f32 = 2.0;
/// The dark frame of a shown set's outline, `#15171a`.
pub const SET_FRAME: Colour = hex(0x15171a);
/// The width of [`SET_FRAME`].
pub const SET_FRAME_WIDTH: f32 = 4.0;

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
pub(crate) mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn req_nfr_15_role_colours_are_the_specified_hex_values() {
        // Spot checks against the table of 7.4 and 11.5.
        assert_eq!(to_color32(ROTARY_AXIS), Color32::from_rgb(0x35, 0xd0, 0xe0));
        assert_eq!(to_color32(PART), Color32::from_rgb(0xb7, 0xbf, 0xca));
        assert_eq!(to_color32(CUT), Color32::from_rgb(0xff, 0xb4, 0x54));
        assert_eq!(to_color32(OVERCUT), Color32::from_rgb(0xdf, 0x52, 0x30));
        assert_eq!(to_color32(LEFT_SMALL), Color32::from_rgb(0xc6, 0xf9, 0xff));
        assert_eq!(to_color32(LEFT_MEDIUM), Color32::from_rgb(0x4b, 0xb7, 0xfc));
        assert_eq!(to_color32(LEFT_MORE), Color32::from_rgb(0x4e, 0x71, 0xea));
        assert_eq!(HELD[3], 0.35);
        // D1: the rotary axis is cyan, not the amber of cut moves.
        assert_ne!(ROTARY_AXIS, CUT);
        // The accent and error outlines are the dark theme's tokens.
        assert_eq!(to_color32(ACCENT), theme::DARK.accent);
        assert_eq!(to_color32(ERROR), theme::DARK.error);
    }

    #[test]
    fn req_ui_04_req_nfr_15_edges_and_triangles_have_colours_of_their_own() {
        // Issue 151: hover and selected differ between a triangle and an edge.
        let light = |c: Colour| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let marks = [HOVER, SELECTED, EDGE_HOVER, EDGE_SELECTED];
        for (i, a) in marks.iter().enumerate() {
            for b in &marks[i + 1..] {
                assert_ne!(a, b);
            }
        }
        // Each edge colour stands out from the part, and the two hovers differ
        // in lightness as well as hue (yellow is the lighter).
        let ratio = |a: Colour, b: Colour| {
            let (hi, lo) = (light(a).max(light(b)), light(a).min(light(b)));
            (hi + 0.05) / (lo + 0.05)
        };
        assert!(
            ratio(EDGE_HOVER, PART) >= 1.4,
            "{}",
            ratio(EDGE_HOVER, PART)
        );
        assert!(ratio(EDGE_SELECTED, PART) >= 1.4);
        assert!(ratio(EDGE_HOVER, HOVER) >= 1.4);
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

    /// The colours as the test of decision 0216 measures them: CIEDE2000
    /// differences, and the simulation of dichromacy by Machado, Oliveira and
    /// Fernandes (2009, full severity). Test helpers only.
    pub(crate) mod measure {
        use openchuck_viewport::Colour;

        fn linear(c: f64) -> f64 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }

        fn gamma(c: f64) -> f64 {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.003_130_8 {
                12.92 * c
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        }

        /// The colour's linear RGB.
        fn rgb(c: Colour) -> [f64; 3] {
            [0, 1, 2].map(|i| linear(f64::from(c[i])))
        }

        /// CIELAB, D65.
        fn lab(c: [f64; 3]) -> [f64; 3] {
            let m = [
                [0.4124, 0.3576, 0.1805],
                [0.2126, 0.7152, 0.0722],
                [0.0193, 0.1192, 0.9505],
            ];
            let white = [0.95047, 1.0, 1.08883];
            let f = |t: f64| {
                if t > 0.008_856 {
                    t.cbrt()
                } else {
                    7.787 * t + 16.0 / 116.0
                }
            };
            let xyz: [f64; 3] =
                [0, 1, 2].map(|i| (m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2]) / white[i]);
            let [fx, fy, fz] = xyz.map(f);
            [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
        }

        /// The CIE L* of `c`.
        pub fn lightness(c: Colour) -> f64 {
            lab(rgb(c))[0]
        }

        /// The kinds of dichromacy simulated.
        pub const KINDS: [&str; 3] = ["protanopia", "deuteranopia", "tritanopia"];

        /// `c` as seen with `kind`, in linear RGB.
        fn seen(c: Colour, kind: &str) -> [f64; 3] {
            let m = match kind {
                "protanopia" => [
                    [0.152_286, 1.052_583, -0.204_868],
                    [0.114_503, 0.786_281, 0.099_216],
                    [-0.003_882, -0.048_116, 1.051_998],
                ],
                "deuteranopia" => [
                    [0.367_322, 0.860_646, -0.227_968],
                    [0.280_085, 0.672_501, 0.047_413],
                    [-0.011_820, 0.042_940, 0.968_881],
                ],
                _ => [
                    [1.255_528, -0.076_749, -0.178_779],
                    [-0.078_411, 0.930_809, 0.147_602],
                    [0.004_733, 0.691_367, 0.303_900],
                ],
            };
            let l = rgb(c);
            // Back through the display's gamma, as the simulation is shown.
            [0, 1, 2].map(|i| {
                linear(gamma(
                    (m[i][0] * l[0] + m[i][1] * l[1] + m[i][2] * l[2]).clamp(0.0, 1.0),
                ))
            })
        }

        /// CIEDE2000 between two colours in linear RGB.
        fn de2000(a: [f64; 3], b: [f64; 3]) -> f64 {
            let ([l1, a1, b1], [l2, a2, b2]) = (lab(a), lab(b));
            let c1 = a1.hypot(b1);
            let c2 = a2.hypot(b2);
            let cb = (c1 + c2) / 2.0;
            let g = 0.5 * (1.0 - (cb.powi(7) / (cb.powi(7) + 25f64.powi(7))).sqrt());
            let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
            let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
            let hue = |b: f64, a: f64| b.atan2(a).to_degrees().rem_euclid(360.0);
            let (h1, h2) = (hue(b1, a1p), hue(b2, a2p));
            let dl = l2 - l1;
            let dc = c2p - c1p;
            let mut dh = h2 - h1;
            if c1p * c2p == 0.0 {
                dh = 0.0;
            } else if dh > 180.0 {
                dh -= 360.0;
            } else if dh < -180.0 {
                dh += 360.0;
            }
            let dhh = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).to_radians().sin();
            let lb = (l1 + l2) / 2.0;
            let cbp = (c1p + c2p) / 2.0;
            let hb = if c1p * c2p == 0.0 {
                h1 + h2
            } else if (h1 - h2).abs() <= 180.0 {
                (h1 + h2) / 2.0
            } else if h1 + h2 < 360.0 {
                (h1 + h2 + 360.0) / 2.0
            } else {
                (h1 + h2 - 360.0) / 2.0
            };
            let cos = |d: f64| d.to_radians().cos();
            let t = 1.0 - 0.17 * cos(hb - 30.0) + 0.24 * cos(2.0 * hb) + 0.32 * cos(3.0 * hb + 6.0)
                - 0.20 * cos(4.0 * hb - 63.0);
            let dth = 30.0 * (-((hb - 275.0) / 25.0).powi(2)).exp();
            let rc = 2.0 * (cbp.powi(7) / (cbp.powi(7) + 25f64.powi(7))).sqrt();
            let sl = 1.0 + 0.015 * (lb - 50.0).powi(2) / (20.0 + (lb - 50.0).powi(2)).sqrt();
            let sc = 1.0 + 0.045 * cbp;
            let sh = 1.0 + 0.015 * cbp * t;
            let rt = -(2.0 * dth).to_radians().sin() * rc;
            ((dl / sl).powi(2)
                + (dc / sc).powi(2)
                + (dhh / sh).powi(2)
                + rt * (dc / sc) * (dhh / sh))
                .sqrt()
        }

        /// CIEDE2000 between `a` and `b` in normal vision.
        pub fn difference(a: Colour, b: Colour) -> f64 {
            de2000(rgb(a), rgb(b))
        }

        /// The smallest CIEDE2000 between `a` and `b` over the simulated
        /// kinds of dichromacy.
        pub fn worst_difference(a: Colour, b: Colour) -> f64 {
            KINDS
                .iter()
                .map(|k| de2000(seen(a, k), seen(b, k)))
                .fold(f64::INFINITY, f64::min)
        }

        /// The WCAG contrast ratio of `a` and `b`.
        pub fn contrast(a: Colour, b: Colour) -> f64 {
            let y = |c: Colour| {
                let [r, g, b] = rgb(c);
                0.2126 * r + 0.7152 * g + 0.0722 * b
            };
            let (ya, yb) = (y(a), y(b));
            (ya.max(yb) + 0.05) / (ya.min(yb) + 0.05)
        }
    }

    #[test]
    fn req_sim_05_req_nfr_15_neighbouring_deviation_bands_are_clearly_apart() {
        // Decision 0216 item 7 (issues 256 and 258): each neighbouring pair
        // at least 20 apart in normal vision and 16 for each simulated colour
        // vision type; overcut at least 25 from every other band.
        use measure::{difference, worst_difference};
        let scale = [
            DeviationBand::Overcut,
            DeviationBand::OnTarget,
            DeviationBand::Thin,
            DeviationBand::Medium,
            DeviationBand::Thick,
        ]
        .map(band_colour);
        for pair in scale.windows(2) {
            let (n, w) = (
                difference(pair[0], pair[1]),
                worst_difference(pair[0], pair[1]),
            );
            assert!(n >= 20.0 && w >= 16.0, "{pair:?}: {n:.1} / {w:.1}");
        }
        for other in &scale[1..] {
            let w = worst_difference(scale[0], *other);
            assert!(w >= 25.0, "overcut and {other:?}: {w:.1}");
        }
    }

    #[test]
    fn req_sim_05_req_nfr_15_material_left_darkens_in_clear_steps_with_contrast_on_the_view() {
        // Decision 0216 item 7: at least 15 in L* per step, and every band
        // at least 3:1 against the view's background (section 16).
        use measure::{contrast, lightness};
        let left = [
            DeviationBand::Thin,
            DeviationBand::Medium,
            DeviationBand::Thick,
        ]
        .map(|b| lightness(band_colour(b)));
        assert!(
            left[0] - left[1] >= 15.0 && left[1] - left[2] >= 15.0,
            "{left:?}"
        );
        let view = token(VIEW_TOP);
        for band in DeviationBand::ALL {
            let c = contrast(band_colour(band), view);
            assert!(c >= 3.0, "{band:?}: {c:.2}:1");
        }
    }

    #[test]
    fn req_sim_15_req_nfr_15_a_hidden_band_reads_apart_from_every_band_and_the_part() {
        // Decision 0216 item 8 (OI-882): at least 18 (CIEDE2000) from every
        // band for every simulated colour vision type, and at least 20 from
        // "within tolerance" and the part's grey in normal vision.
        use measure::{difference, worst_difference};
        for band in DeviationBand::ALL {
            let w = worst_difference(HIDDEN_BAND, band_colour(band));
            assert!(w >= 18.0, "{band:?}: {w:.1}");
        }
        assert!(difference(HIDDEN_BAND, WITHIN) >= 20.0);
        assert!(difference(HIDDEN_BAND, PART) >= 20.0);
    }
}
