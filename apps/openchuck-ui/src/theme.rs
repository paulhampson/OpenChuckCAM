// SPDX-License-Identifier: AGPL-3.0-or-later
//! The visual design of the interface: the colour tokens, the type scale, the
//! spacing and target sizes of ui-specification section 14 (UI-14, NFR-15),
//! and the IBM Plex fonts bundled with the program (NFR-09, NFR-10; decision
//! records 0038 and 0105; slice 22 lane X7a, decision 0114 section E).
//!
//! This module defines the design as data and as egui styles. Applying it to
//! the existing panels is lane X7b's work, so only [`install_fonts`] is called
//! from `main.rs` today; the rest waits for the shell rewrite of lane X2.
//!
//! The fonts are the unmodified IBM Plex files under the SIL Open Font
//! License 1.1 (`LICENSES/OFL-1.1.txt`, `LICENSES/IBM-Plex-NOTICE.txt`). They
//! are compiled in, never loaded from a network at run time (NFR-09). egui's
//! own fonts stay behind them as the fallback for characters Plex lacks.

// The tokens and styles are consumed by lanes X2, X4 and X7b; until they
// merge, only the fonts are in use.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Theme as EguiTheme, Vec2, Visuals,
};

/// Which colour set is in use. Dark is the default; light is P2 (UI-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// A bordered coloured box: the warning box and the error box of 14.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxColours {
    pub bg: Color32,
    pub border: Color32,
    pub text: Color32,
}

/// The colour tokens of ui-specification 14.1, for one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tokens {
    /// Window background, splitters.
    pub bg: Color32,
    /// Panels, dock, bars.
    pub panel: Color32,
    /// Buttons, add buttons.
    pub raised: Color32,
    /// Inputs, code, readouts.
    pub field: Color32,
    /// Title bar, footer.
    pub bar: Color32,
    /// Borders.
    pub line: Color32,
    /// Row dividers.
    pub line_soft: Color32,
    pub text: Color32,
    /// Labels, secondary text.
    pub muted: Color32,
    /// Section headings.
    pub heading: Color32,
    /// Focus, active step, selection bar, links.
    pub accent: Color32,
    /// Selected row and segment.
    pub selected: Color32,
    /// The primary button: fill, border and text (the same in both themes).
    pub primary: Color32,
    pub primary_border: Color32,
    pub primary_text: Color32,
    /// Up to date, confirmed.
    pub ok: Color32,
    /// Stale, warnings, unconfirmed.
    pub warning: Color32,
    /// Errors, refused values.
    pub error: Color32,
    /// Overcut values in panels.
    pub overcut: Color32,
    /// Unit question, safety notice.
    pub warning_box: BoxColours,
    /// Error banners.
    pub error_box: BoxColours,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const fn boxed(bg: u32, border: u32, text: u32) -> BoxColours {
    BoxColours {
        bg: rgb(bg),
        border: rgb(border),
        text: rgb(text),
    }
}

/// The dark tokens (14.1, "Dark" column).
pub const DARK: Tokens = Tokens {
    bg: rgb(0x1b1d21),
    panel: rgb(0x23262b),
    raised: rgb(0x2b2f35),
    field: rgb(0x16181b),
    bar: rgb(0x15171a),
    line: rgb(0x383d45),
    line_soft: rgb(0x2d3137),
    text: rgb(0xe4e7eb),
    muted: rgb(0xa3abb5),
    heading: rgb(0xc3c9d1),
    accent: rgb(0x5aa9ff),
    selected: rgb(0x264466),
    primary: rgb(0x2f6fbd),
    primary_border: rgb(0x4a8fe0),
    primary_text: Color32::WHITE,
    ok: rgb(0x4cc38a),
    warning: rgb(0xf2b84b),
    error: rgb(0xff7b72),
    overcut: rgb(0xff8a66),
    warning_box: boxed(0x2a2316, 0x6b5420, 0xf2d28a),
    error_box: boxed(0x2e1a1a, 0x7a3330, 0xffb4ae),
};

/// The light tokens (14.1, "Light" column). The light theme is P2 (UI-20).
pub const LIGHT: Tokens = Tokens {
    bg: rgb(0xe6e9ed),
    panel: rgb(0xf6f7f9),
    raised: rgb(0xffffff),
    field: rgb(0xffffff),
    bar: rgb(0xdde1e6),
    line: rgb(0xc9ced5),
    line_soft: rgb(0xe2e5e9),
    text: rgb(0x1d2125),
    muted: rgb(0x56606b),
    heading: rgb(0x2f363d),
    accent: rgb(0x1d6ad0),
    selected: rgb(0xd4e5fb),
    primary: rgb(0x2f6fbd),
    primary_border: rgb(0x4a8fe0),
    primary_text: Color32::WHITE,
    ok: rgb(0x1b7f53),
    warning: rgb(0x8a5d00),
    error: rgb(0xc22b2b),
    overcut: rgb(0xa8321a),
    warning_box: boxed(0xfff4d6, 0xe0b44c, 0x5c4300),
    error_box: boxed(0xfde8e7, 0xe59a95, 0x7a1d18),
};

impl Tokens {
    /// The tokens of `theme`.
    pub const fn of(theme: Theme) -> &'static Tokens {
        match theme {
            Theme::Dark => &DARK,
            Theme::Light => &LIGHT,
        }
    }
}

/// The viewport keeps its dark gradient in both themes (14.2): top and bottom.
pub const VIEWPORT_GRADIENT: (Color32, Color32) = (rgb(0x2c3139), rgb(0x1b1e23));

/// Type sizes in points at 100% scale (14.3). Text is never smaller than 12
/// except headings and chips at 11 (NFR-15).
pub mod size {
    /// Text and values.
    pub const TEXT: f32 = 12.0;
    /// Section headings and group labels: upper case, with letter spacing.
    pub const HEADING: f32 = 11.0;
    /// The operation name.
    pub const OPERATION_NAME: f32 = 13.0;
    /// Result figures.
    pub const RESULT: f32 = 14.0;
    /// Empty-state titles.
    pub const EMPTY_TITLE: f32 = 18.0;
}

/// Spacing is on a 4-pixel grid (14.3).
pub const GRID: f32 = 4.0;
/// Corner radius of controls.
pub const RADIUS: u8 = 3;
/// Corner radius of cards and popovers.
pub const RADIUS_CARD: u8 = 6;

/// Target sizes (14.3).
pub mod target {
    /// Buttons are this high, and icon buttons this square.
    pub const BUTTON: f32 = 28.0;
    /// Small icon and "+" buttons.
    pub const SMALL_BUTTON: f32 = 22.0;
    /// A check box is drawn this big...
    pub const CHECK_BOX: f32 = 14.0;
    /// ...inside a hit area this big.
    pub const CHECK_HIT: f32 = 20.0;
    /// Rows are 28 to 34 high.
    pub const ROW_MIN: f32 = 28.0;
    pub const ROW_MAX: f32 = 34.0;
    /// The focus ring is this many pixels of `accent`.
    pub const FOCUS_RING: f32 = 2.0;
}

/// Widths of the panels (section 3): default, minimum and maximum.
pub mod width {
    /// Project panel.
    pub const PROJECT: (f32, f32, f32) = (264.0, 200.0, 400.0);
    /// Inspector.
    pub const INSPECTOR: (f32, f32, f32) = (340.0, 280.0, 520.0);
    /// Splitters.
    pub const SPLITTER: f32 = 5.0;
    /// The G-code panel of Simulate (11.5).
    pub const GCODE_PANEL: f32 = 300.0;
    /// The label column of an inspector field row (8).
    pub const LABEL_COLUMN: f32 = 120.0;
}

/// The family of IBM Plex Sans SemiBold, for the emphasis egui has no weight for.
pub fn sans_semibold() -> FontFamily {
    FontFamily::Name("plex-sans-semibold".into())
}

/// The family of IBM Plex Mono SemiBold.
pub fn mono_semibold() -> FontFamily {
    FontFamily::Name("plex-mono-semibold".into())
}

const SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
const SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf");
const MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");
const MONO_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-SemiBold.ttf");

/// The font set: Plex first in the proportional and monospace families, with
/// egui's own fonts kept behind it as the fallback (14.3).
pub fn font_definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let bundled: [(&str, &'static [u8]); 4] = [
        ("plex-sans", SANS_REGULAR),
        ("plex-sans-semibold", SANS_SEMIBOLD),
        ("plex-mono", MONO_REGULAR),
        ("plex-mono-semibold", MONO_SEMIBOLD),
    ];
    for (name, bytes) in bundled {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    let fallback = |family: &FontFamily, fonts: &FontDefinitions| {
        fonts.families.get(family).cloned().unwrap_or_default()
    };
    let proportional = fallback(&FontFamily::Proportional, &fonts);
    let monospace = fallback(&FontFamily::Monospace, &fonts);
    let with_fallback = |first: &str, rest: &[String]| {
        let mut list = vec![first.to_owned()];
        list.extend(rest.iter().cloned());
        list
    };
    fonts.families = BTreeMap::from([
        (
            FontFamily::Proportional,
            with_fallback("plex-sans", &proportional),
        ),
        (
            FontFamily::Monospace,
            with_fallback("plex-mono", &monospace),
        ),
        (
            sans_semibold(),
            with_fallback("plex-sans-semibold", &proportional),
        ),
        (
            mono_semibold(),
            with_fallback("plex-mono-semibold", &monospace),
        ),
    ]);
    fonts
}

/// Installs the Plex fonts on `ctx`.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

/// egui's text styles at the sizes of 14.3: text and values at 12, small at
/// 11, the operation name at 13 (`Heading`), numbers in Plex Mono at 12.
pub fn text_styles() -> BTreeMap<TextStyle, FontId> {
    let sans = |size| FontId::new(size, FontFamily::Proportional);
    BTreeMap::from([
        (TextStyle::Small, sans(size::HEADING)),
        (TextStyle::Body, sans(size::TEXT)),
        (TextStyle::Button, sans(size::TEXT)),
        (TextStyle::Heading, sans(size::OPERATION_NAME)),
        (
            TextStyle::Monospace,
            FontId::new(size::TEXT, FontFamily::Monospace),
        ),
    ])
}

/// The egui visuals for `theme`: every colour from the tokens, corner radii of
/// 3 and 6, a 2-pixel `accent` focus ring.
pub fn visuals(theme: Theme) -> Visuals {
    let t = Tokens::of(theme);
    let mut v = match theme {
        Theme::Dark => Visuals::dark(),
        Theme::Light => Visuals::light(),
    };
    v.panel_fill = t.panel;
    v.window_fill = t.panel;
    v.window_stroke = Stroke::new(1.0, t.line);
    v.extreme_bg_color = t.field;
    v.faint_bg_color = t.line_soft;
    v.code_bg_color = t.field;
    v.hyperlink_color = t.accent;
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.error;
    v.window_corner_radius = CornerRadius::same(RADIUS_CARD);
    v.menu_corner_radius = CornerRadius::same(RADIUS_CARD);
    v.selection.bg_fill = t.selected;
    v.selection.stroke = Stroke::new(target::FOCUS_RING, t.accent);

    let radius = CornerRadius::same(RADIUS);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.panel;
    w.noninteractive.weak_bg_fill = t.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.line_soft);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    w.inactive.bg_fill = t.raised;
    w.inactive.weak_bg_fill = t.raised;
    w.inactive.bg_stroke = Stroke::new(1.0, t.line);
    w.inactive.fg_stroke = Stroke::new(1.0, t.text);
    w.hovered.bg_fill = t.raised;
    w.hovered.weak_bg_fill = t.raised;
    w.hovered.bg_stroke = Stroke::new(1.0, t.accent);
    w.hovered.fg_stroke = Stroke::new(1.0, t.text);
    w.active.bg_fill = t.selected;
    w.active.weak_bg_fill = t.selected;
    w.active.bg_stroke = Stroke::new(target::FOCUS_RING, t.accent);
    w.active.fg_stroke = Stroke::new(1.0, t.text);
    w.open.bg_fill = t.selected;
    w.open.weak_bg_fill = t.selected;
    w.open.bg_stroke = Stroke::new(1.0, t.line);
    w.open.fg_stroke = Stroke::new(1.0, t.text);
    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.corner_radius = radius;
    }
    v
}

/// Applies the type scale, the spacing and the target sizes to egui's style.
fn apply_metrics(style: &mut egui::Style) {
    style.text_styles = text_styles();
    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(2.0 * GRID, GRID);
    s.button_padding = Vec2::new(2.0 * GRID, GRID);
    s.window_margin = egui::Margin::same(2 * GRID as i8);
    s.menu_margin = egui::Margin::same(GRID as i8);
    s.indent = 3.0 * GRID;
    s.interact_size = Vec2::new(target::BUTTON, target::BUTTON);
    s.icon_width = target::CHECK_BOX;
    s.icon_width_inner = target::CHECK_BOX - 2.0 * GRID;
    s.icon_spacing = 2.0 * GRID;
}

/// Gives the scroll areas drawn in `ui` a bar that takes its own strip, so it
/// never lies over a field (UI-16; issue 174, F-19). Used where field rows
/// scroll: the inspector and the tool library window.
pub fn solid_scroll_bar(ui: &mut egui::Ui) {
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
}

/// Installs the visuals and metrics for `theme` on `ctx`: the dark style on
/// the dark egui theme and the light one on the light theme.
pub fn install_style(ctx: &egui::Context, theme: Theme) {
    let egui_theme = match theme {
        Theme::Dark => EguiTheme::Dark,
        Theme::Light => EguiTheme::Light,
    };
    ctx.set_visuals_of(egui_theme, visuals(theme));
    ctx.style_mut_of(egui_theme, apply_metrics);
    ctx.set_theme(egui::ThemePreference::from(egui_theme));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an opaque colour.
    fn luminance(c: Color32) -> f64 {
        let lin = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    const THEMES: [Theme; 2] = [Theme::Dark, Theme::Light];

    #[test]
    fn req_ui_14_tokens_match_specification_14_1() {
        // A spot check of both columns against the table in 14.1.
        assert_eq!(DARK.bg, Color32::from_rgb(0x1b, 0x1d, 0x21));
        assert_eq!(DARK.accent, Color32::from_rgb(0x5a, 0xa9, 0xff));
        assert_eq!(DARK.error_box.text, Color32::from_rgb(0xff, 0xb4, 0xae));
        assert_eq!(LIGHT.panel, Color32::from_rgb(0xf6, 0xf7, 0xf9));
        assert_eq!(
            LIGHT.warning_box.border,
            Color32::from_rgb(0xe0, 0xb4, 0x4c)
        );
        // The overcut text follows the vermilion of the scale (decision 0216).
        assert_eq!(DARK.overcut, Color32::from_rgb(0xff, 0x8a, 0x66));
        assert_eq!(LIGHT.overcut, Color32::from_rgb(0xa8, 0x32, 0x1a));
        for theme in THEMES {
            let t = Tokens::of(theme);
            assert_eq!(t.primary, Color32::from_rgb(0x2f, 0x6f, 0xbd));
            assert_eq!(t.primary_border, Color32::from_rgb(0x4a, 0x8f, 0xe0));
            assert_eq!(t.primary_text, Color32::WHITE);
        }
        assert_eq!(Theme::default(), Theme::Dark, "dark is the default (14.2)");
    }

    #[test]
    fn req_nfr_15_text_tokens_reach_4_5_to_1_on_every_surface() {
        for theme in THEMES {
            let t = Tokens::of(theme);
            for surface in [t.bg, t.panel, t.raised, t.field, t.bar] {
                for (name, fg) in [("text", t.text), ("muted", t.muted), ("heading", t.heading)] {
                    let c = contrast(fg, surface);
                    assert!(c >= 4.5, "{theme:?} {name} on {surface:?} is {c:.2}:1");
                }
            }
        }
    }

    #[test]
    fn req_nfr_15_state_and_box_text_reach_4_5_to_1() {
        for theme in THEMES {
            let t = Tokens::of(theme);
            for (name, fg) in [
                ("accent", t.accent),
                ("ok", t.ok),
                ("warning", t.warning),
                ("error", t.error),
                ("overcut", t.overcut),
            ] {
                let c = contrast(fg, t.panel);
                assert!(c >= 4.5, "{theme:?} {name} on panel is {c:.2}:1");
            }
            for (name, b) in [("warning box", t.warning_box), ("error box", t.error_box)] {
                let c = contrast(b.text, b.bg);
                assert!(c >= 4.5, "{theme:?} {name} text is {c:.2}:1");
            }
            let c = contrast(t.primary_text, t.primary);
            assert!(c >= 4.5, "{theme:?} primary button text is {c:.2}:1");
        }
    }

    #[test]
    fn req_nfr_15_req_ui_11_the_bar_state_colours_have_contrast_on_the_bar() {
        // Issue 210: the overall status and each step's mark sit on the bar.
        // The marks are shapes, which need 3:1 (UI-11); on the dark theme,
        // which the bar is drawn in, they reach 4.5:1 as text would.
        for theme in THEMES {
            let t = Tokens::of(theme);
            for (name, fg) in [("ok", t.ok), ("warning", t.warning), ("error", t.error)] {
                let c = contrast(fg, t.bar);
                assert!(c >= 3.0, "{theme:?} {name} on the bar is {c:.2}:1");
                let c = contrast(fg, t.field);
                assert!(c >= 3.0, "{theme:?} {name} on the badge is {c:.2}:1");
                if theme == Theme::Dark {
                    let c = contrast(fg, t.bar);
                    assert!(c >= 4.5, "dark {name} on the bar is {c:.2}:1");
                }
            }
        }
    }

    #[test]
    fn req_nfr_15_graphics_tokens_reach_3_to_1() {
        for theme in THEMES {
            let t = Tokens::of(theme);
            let c = contrast(t.accent, t.panel);
            assert!(c >= 3.0, "{theme:?} focus ring on panel is {c:.2}:1");
            let c = contrast(t.accent, t.selected);
            assert!(
                c >= 3.0,
                "{theme:?} focus ring on a selected row is {c:.2}:1"
            );
        }
    }

    #[test]
    fn req_nfr_15_text_sizes_are_at_least_12_except_headings_at_11() {
        let styles = text_styles();
        for (style, font) in &styles {
            let floor = if *style == TextStyle::Small {
                size::HEADING
            } else {
                size::TEXT
            };
            assert!(font.size >= floor, "{style:?} is {}", font.size);
        }
        assert_eq!(styles[&TextStyle::Body].size.to_bits(), 12.0_f32.to_bits());
        assert_eq!(
            styles[&TextStyle::Heading].size.to_bits(),
            13.0_f32.to_bits()
        );
        assert_eq!(size::RESULT.to_bits(), 14.0_f32.to_bits());
        assert_eq!(size::EMPTY_TITLE.to_bits(), 18.0_f32.to_bits());
        assert_eq!(styles[&TextStyle::Monospace].family, FontFamily::Monospace);
    }

    #[test]
    fn req_ui_14_sizes_are_on_the_four_pixel_grid() {
        let mut style = egui::Style::default();
        apply_metrics(&mut style);
        let s = &style.spacing;
        for v in [
            s.item_spacing.x,
            s.item_spacing.y,
            s.button_padding.x,
            s.button_padding.y,
            s.indent,
            s.icon_spacing,
            f32::from(s.window_margin.left),
            f32::from(s.menu_margin.left),
        ] {
            assert!(
                (v / GRID).fract().abs() < f32::EPSILON,
                "{v} is off the grid"
            );
        }
        assert_eq!(s.interact_size.y.to_bits(), target::BUTTON.to_bits());
        assert_eq!(s.icon_width.to_bits(), target::CHECK_BOX.to_bits());
    }

    #[test]
    fn req_ui_14_visuals_use_the_tokens_and_the_focus_ring() {
        for theme in THEMES {
            let t = Tokens::of(theme);
            let v = visuals(theme);
            assert_eq!(v.dark_mode, theme == Theme::Dark);
            assert_eq!(v.panel_fill, t.panel);
            assert_eq!(v.extreme_bg_color, t.field);
            assert_eq!(v.widgets.inactive.weak_bg_fill, t.raised);
            assert_eq!(v.selection.bg_fill, t.selected);
            assert_eq!(v.selection.stroke.color, t.accent);
            assert_eq!(
                v.selection.stroke.width.to_bits(),
                target::FOCUS_RING.to_bits()
            );
            assert_eq!(v.window_corner_radius, CornerRadius::same(RADIUS_CARD));
            assert_eq!(v.widgets.inactive.corner_radius, CornerRadius::same(RADIUS));
            assert_eq!(v.error_fg_color, t.error);
        }
    }

    #[test]
    fn req_nfr_09_req_nfr_10_plex_is_bundled_and_first_in_every_family() {
        let fonts = font_definitions();
        for (family, first) in [
            (FontFamily::Proportional, "plex-sans"),
            (FontFamily::Monospace, "plex-mono"),
            (sans_semibold(), "plex-sans-semibold"),
            (mono_semibold(), "plex-mono-semibold"),
        ] {
            let list = &fonts.families[&family];
            assert_eq!(list[0], first);
            assert!(
                list.len() > 1,
                "egui's own fonts stay as the fallback for {family:?}"
            );
            assert!(fonts.font_data.contains_key(first));
        }
        // The files are real TrueType data, not the git LFS or a placeholder.
        for bytes in [SANS_REGULAR, SANS_SEMIBOLD, MONO_REGULAR, MONO_SEMIBOLD] {
            assert!(bytes.len() > 100_000);
            assert_eq!(&bytes[..4], &[0, 1, 0, 0], "TrueType signature");
        }
    }

    #[test]
    fn req_nfr_10_plex_files_are_unmodified_and_the_notice_names_them() {
        // The notice records the SHA-256 of each file; a changed or subset
        // font (which the OFL forbids under the name Plex) fails here.
        let notice = include_str!("../../../LICENSES/IBM-Plex-NOTICE.txt");
        for (name, bytes) in [
            ("IBMPlexSans-Regular.ttf", SANS_REGULAR),
            ("IBMPlexSans-SemiBold.ttf", SANS_SEMIBOLD),
            ("IBMPlexMono-Regular.ttf", MONO_REGULAR),
            ("IBMPlexMono-SemiBold.ttf", MONO_SEMIBOLD),
        ] {
            let line = notice
                .lines()
                .find(|l| l.trim_end().ends_with(name) && l.contains("  "))
                .unwrap_or_else(|| panic!("{name} has no hash line in the notice"));
            let want = line.split_whitespace().next().unwrap_or_default();
            assert_eq!(sha256_hex(bytes), want, "{name} differs from the notice");
        }
        let ofl = include_str!("../../../LICENSES/OFL-1.1.txt");
        assert!(ofl.contains("SIL OPEN FONT LICENSE Version 1.1"));
        assert!(ofl.contains("Copyright © 2017 IBM Corp."));
    }

    #[test]
    fn req_nfr_15_installing_the_style_and_fonts_works_on_a_context() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        install_style(&ctx, Theme::Dark);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("Plex 0.123 mm");
            ui.monospace("12.345");
        });
        // No renderer here takes the font texture; egui asks for it to be
        // dropped on purpose.
        output.textures_delta.clear();
        let style = ctx.style_of(EguiTheme::Dark);
        assert_eq!(style.visuals.panel_fill, DARK.panel);
        assert_eq!(
            style.text_styles[&TextStyle::Body].size.to_bits(),
            size::TEXT.to_bits()
        );
    }

    /// A small SHA-256 (FIPS 180-4), so the test needs no dependency.
    fn sha256_hex(data: &[u8]) -> String {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let mut msg = data.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
        for block in msg.chunks(64) {
            let mut w = [0u32; 64];
            for (i, word) in block.chunks(4).enumerate() {
                w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }
            let mut v = h;
            for i in 0..64 {
                let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
                let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
                let t1 = v[7]
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
                let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
                let t2 = s0.wrapping_add(maj);
                v = [
                    t1.wrapping_add(t2),
                    v[0],
                    v[1],
                    v[2],
                    v[3].wrapping_add(t1),
                    v[4],
                    v[5],
                    v[6],
                ];
            }
            for (a, b) in h.iter_mut().zip(v) {
                *a = a.wrapping_add(b);
            }
        }
        h.iter().map(|x| format!("{x:08x}")).collect()
    }

    /// Every non-ASCII character the interface draws is in the bundled Plex
    /// fonts or in egui's own fallback fonts, so none shows as an empty
    /// square on any system (issue 177, F-22: the drop-down arrows and a
    /// button glyph did on Windows). The sweep reads the sources, so a new
    /// character is checked without a list to keep.
    #[test]
    fn req_nfr_09_req_ui_16_every_character_the_interface_draws_has_a_glyph() {
        fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    rust_files(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .contains("_tests")
                {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        rust_files(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut wanted: BTreeMap<char, String> = BTreeMap::new();
        for file in files {
            let source = std::fs::read_to_string(&file).unwrap();
            for line in source.lines() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                // Escapes such as "\u{21ba}" count as the character they name.
                let mut rest = line;
                while let Some(at) = rest.find("\\u{") {
                    let tail = &rest[at + 3..];
                    if let Some(end) = tail.find('}')
                        && let Ok(code) = u32::from_str_radix(&tail[..end], 16)
                        && let Some(c) = char::from_u32(code)
                    {
                        wanted
                            .entry(c)
                            .or_insert_with(|| file.display().to_string());
                    }
                    rest = &rest[at + 3..];
                }
                for c in line.chars().filter(|c| !c.is_ascii()) {
                    wanted
                        .entry(c)
                        .or_insert_with(|| file.display().to_string());
                }
            }
        }
        assert!(wanted.len() > 20, "the sweep found only {}", wanted.len());
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        // The fonts load on the next frame.
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        let missing: Vec<String> = ctx.fonts_mut(|fonts| {
            wanted
                .iter()
                .filter(|(c, _)| {
                    !fonts.has_glyph(&FontId::new(12.0, FontFamily::Proportional), **c)
                        || !fonts.has_glyph(&FontId::new(12.0, FontFamily::Monospace), **c)
                })
                .map(|(c, file)| format!("U+{:04X} {c} in {file}", *c as u32))
                .collect()
        });
        assert!(missing.is_empty(), "no glyph for: {missing:#?}");
    }
}
