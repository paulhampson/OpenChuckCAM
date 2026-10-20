// SPDX-License-Identifier: AGPL-3.0-or-later
//! The About window (Help > About OpenChuckCAM; NFR-16, NFR-10, NFR-12; decision record 0151;
//! specification section 12.4). It names the version and the licence, shows which build is
//! running, and holds the licence texts in a scrollable box. Close is the one primary action;
//! Copy copies the build information, and only that, to the clipboard.

use eframe::egui::{self, RichText};

use crate::theme::DARK;

/// The licence texts the repository ships (NFR-10): the AGPL, then the notices for the fonts and
/// the STEP kernel. The crate licences are in `THIRD-PARTY-NOTICES.html` beside the program.
const LICENCE_FILES: [(&str, &str); 4] = [
    (
        "GNU Affero General Public License, version 3 or later",
        include_str!("../../../LICENSES/AGPL-3.0-or-later.txt"),
    ),
    (
        "IBM Plex fonts (notice)",
        include_str!("../../../LICENSES/IBM-Plex-NOTICE.txt"),
    ),
    (
        "SIL Open Font Licence 1.1",
        include_str!("../../../LICENSES/OFL-1.1.txt"),
    ),
    (
        "OpenCASCADE (notice and source offer)",
        include_str!("../../../LICENSES/OpenCASCADE-NOTICE.txt"),
    ),
];

/// Where the licences of the third-party crates are.
const CRATE_NOTICES: &str = "The licences of the third-party Rust crates in this program are in \
THIRD-PARTY-NOTICES.html, in the folder the program came in.";

/// The text of the scrollable licence box, made once.
pub fn licence_text() -> &'static str {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TEXT.get_or_init(licence_box)
}

fn licence_box() -> String {
    let mut out = String::from(CRATE_NOTICES);
    for (title, text) in LICENCE_FILES {
        out.push_str("\n\n");
        out.push_str(&"=".repeat(72));
        out.push('\n');
        out.push_str(title);
        out.push('\n');
        out.push_str(&"=".repeat(72));
        out.push_str("\n\n");
        out.push_str(text.trim_end());
    }
    out.push('\n');
    out
}

/// The Copy button's tooltip.
const COPY_TIP: &str = "Copies the build information above, and nothing else, to the clipboard.";

/// The Open log folder button's tooltip.
const LOG_TIP: &str = "Shows the folder that holds the log file in the file browser.";

/// "Log file": where this run writes its log (NFR-17, decision 0155) and a button that shows the
/// folder. A failure to open it is written under the button.
fn log_section(ui: &mut egui::Ui) {
    let failure = egui::Id::new("about open log folder failure");
    ui.strong("Log file");
    match crate::logging::log_path() {
        Some(path) => ui.label(RichText::new(path.display().to_string()).monospace()),
        None => ui.label("This run is not writing a log file."),
    };
    let button = ui.button("Open log folder").on_hover_text(LOG_TIP);
    // The tooltip opens on keyboard focus as well as on hover (specification 16).
    if button.has_focus() && !button.hovered() {
        button.show_tooltip_text(LOG_TIP);
    }
    if button.clicked() {
        let result = crate::logging::open_log_folder();
        ui.data_mut(|d| d.insert_temp(failure, result.err()));
    }
    if let Some(Some(message)) = ui.data(|d| d.get_temp::<Option<String>>(failure)) {
        ui.label(message);
    }
}

/// What the window asks of the shell.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// Close was pressed.
    pub close: bool,
}

/// Draws the window's content. `build` is the build information (what Copy copies) and
/// `licences` the licence box's text.
pub fn show(ui: &mut egui::Ui, build: &str, licences: &str, project: Option<&str>) -> Outcome {
    let mut outcome = Outcome::default();
    ui.heading("OpenChuckCAM");
    ui.label("Free software under the GNU Affero General Public License, version 3 or later.");
    ui.label(openchuck_app::VERIFY_GCODE_NOTICE);
    ui.add_space(6.0);

    if let Some(path) = project {
        ui.strong("Project folder");
        ui.label(path);
        ui.add_space(6.0);
    }

    ui.strong("Build");
    ui.label(RichText::new(build.trim_end()).monospace());
    ui.add_space(6.0);

    ui.strong("Licences");
    // A read-only text box takes the keyboard (Tab) and scrolls with the arrow keys and the
    // wheel, so it is reachable without a pointer (UI-14).
    let mut shown = licences;
    egui::ScrollArea::vertical()
        .id_salt("about licences")
        .max_height(220.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut shown)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(10),
            );
        });
    ui.add_space(6.0);

    log_section(ui);
    ui.add_space(6.0);

    // Copy is secondary, on the left; Close is the primary action, bottom right. Tab visits them
    // in this order, which is the reading order.
    egui::containers::Sides::new().show(
        ui,
        |ui| {
            let copy = ui.button("Copy build information").on_hover_text(COPY_TIP);
            // The tooltip opens on keyboard focus as well as on hover (specification 16).
            if copy.has_focus() && !copy.hovered() {
                copy.show_tooltip_text(COPY_TIP);
            }
            if copy.clicked() {
                ui.ctx().copy_text(build.to_owned());
            }
        },
        |ui| {
            let close = ui.add(
                egui::Button::new(RichText::new("Close").color(DARK.primary_text).strong())
                    .fill(DARK.primary)
                    .stroke(egui::Stroke::new(1.0, DARK.primary_border)),
            );
            if close.clicked() {
                outcome.close = true;
            }
        },
    );
    outcome
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use eframe::egui::{Event, Key, Modifiers, OutputCommand, PointerButton};

    const BUILD: &str = "OpenChuckCAM 1.2.3\nCommit: 0123\nUncommitted changes: no\n";

    /// A window drawn frame by frame without a screen.
    struct Rig {
        ctx: egui::Context,
        outcome: Outcome,
        copied: Vec<String>,
        shapes: Vec<egui::epaint::ClippedShape>,
    }

    impl Rig {
        fn new() -> Self {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let mut rig = Self {
                ctx,
                outcome: Outcome::default(),
                copied: Vec::new(),
                shapes: Vec::new(),
            };
            rig.frame(Vec::new());
            rig
        }

        fn frame(&mut self, events: Vec<Event>) {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(900.0, 700.0),
                )),
                events,
                ..Default::default()
            };
            let mut output = self.ctx.run_ui(input, |ui| {
                self.outcome = show(ui, BUILD, licence_text(), None);
            });
            output.textures_delta.clear();
            for command in output.platform_output.commands {
                if let OutputCommand::CopyText(text) = command {
                    self.copied.push(text);
                }
            }
            self.shapes = output.shapes;
        }

        fn key(&mut self, key: Key) {
            let event = |pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            };
            self.frame(vec![event(true), event(false)]);
        }

        fn centre_of(&self, needle: &str) -> egui::Pos2 {
            fn walk(shape: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
                match shape {
                    egui::Shape::Text(t) if t.galley.text().contains(needle) => {
                        Some(t.pos + t.galley.rect.center().to_vec2())
                    }
                    egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| walk(s, needle)),
                    _ => None,
                }
            }
            self.shapes
                .iter()
                .find_map(|s| walk(&s.shape, needle))
                .unwrap_or_else(|| panic!("{needle} is not on screen"))
        }

        fn click(&mut self, at: egui::Pos2) {
            let button = |pressed| Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            self.frame(vec![Event::PointerMoved(at)]);
            self.frame(vec![button(true)]);
            self.frame(vec![button(false)]);
        }

        fn shows(&self, needle: &str) -> bool {
            fn has(shape: &egui::Shape, needle: &str) -> bool {
                match shape {
                    egui::Shape::Text(t) => t.galley.text().contains(needle),
                    egui::Shape::Vec(shapes) => shapes.iter().any(|s| has(s, needle)),
                    _ => false,
                }
            }
            self.shapes.iter().any(|s| has(&s.shape, needle))
        }
    }

    /// NFR-16, NFR-12: the window shows the licence, the notice and every line of the build text.
    #[test]
    fn req_nfr_16_about_window_renders_the_build_and_the_notices() {
        let rig = Rig::new();
        assert!(rig.shows("OpenChuckCAM"));
        assert!(rig.shows("GNU Affero General Public License"));
        assert!(rig.shows("Generated G-code must be verified"));
        assert!(rig.shows("Commit: 0123"));
        assert!(rig.shows("Uncommitted changes: no"));
        assert!(rig.shows("Copy build information"));
        assert!(rig.shows("Close"));
        assert!(rig.shows("GNU AFFERO GENERAL PUBLIC LICENSE"));
    }

    /// NFR-10, NFR-16: the licence box holds the AGPL and the notices the repository ships,
    /// and says where the crate licences are.
    #[test]
    fn req_nfr_16_req_nfr_10_licence_box_holds_the_shipped_texts() {
        let text = licence_text();
        for needle in [
            "THIRD-PARTY-NOTICES.html",
            "GNU AFFERO GENERAL PUBLIC LICENSE",
            "IBM Plex",
            "SIL OPEN FONT LICENSE",
            "OpenCASCADE",
        ] {
            assert!(text.contains(needle), "{needle}");
        }
    }

    /// NFR-17: the window names the log file, and Open log folder says so when there is none.
    #[test]
    fn req_nfr_17_about_window_offers_the_log_folder() {
        let mut rig = Rig::new();
        assert!(rig.shows("Log file"));
        assert!(rig.shows("This run is not writing a log file."));
        assert!(!rig.shows("There is no log file in this run."));
        let at = rig.centre_of("Open log folder");
        rig.click(at);
        assert!(rig.shows("There is no log file in this run."));
        assert!(!rig.outcome.close);
    }

    /// NFR-16: clicking Copy puts exactly the build text on the clipboard, and does not close.
    #[test]
    fn req_nfr_16_copy_puts_exactly_the_build_text_on_the_clipboard() {
        let mut rig = Rig::new();
        let at = rig.centre_of("Copy build information");
        rig.click(at);
        assert_eq!(rig.copied, vec![BUILD.to_owned()]);
        assert!(!rig.outcome.close);
    }

    /// NFR-16, NFR-17, UI-14: the licence box, Open log folder, Copy and Close are reached in
    /// that order with Tab, and Enter presses the one that has the keyboard.
    #[test]
    fn req_nfr_16_req_ui_14_keyboard_reaches_the_box_log_copy_and_close() {
        let mut rig = Rig::new();
        rig.key(Key::Tab); // the licence box
        rig.key(Key::Tab); // Open log folder
        assert!(!rig.shows("There is no log file in this run."));
        rig.key(Key::Enter);
        assert!(rig.shows("There is no log file in this run."));
        assert!(rig.copied.is_empty());
        rig.key(Key::Tab); // Copy
        rig.key(Key::Enter);
        assert_eq!(rig.copied, vec![BUILD.to_owned()]);
        rig.key(Key::Tab); // Close
        assert!(!rig.outcome.close);
        rig.key(Key::Enter);
        assert!(rig.outcome.close);
    }

    /// NFR-16, specification 16: the Copy tooltip opens when Copy has the keyboard.
    #[test]
    fn req_nfr_16_copy_tooltip_opens_on_focus() {
        let mut rig = Rig::new();
        assert!(!rig.shows("nothing else"));
        rig.key(Key::Tab);
        rig.key(Key::Tab);
        rig.key(Key::Tab);
        rig.frame(Vec::new());
        assert!(rig.shows("nothing else"));
    }

    /// NFR-16: Close is the primary action and the click closes.
    #[test]
    fn req_nfr_16_close_button_closes() {
        let mut rig = Rig::new();
        let at = rig.centre_of("Close");
        rig.click(at);
        assert!(rig.outcome.close);
        assert!(rig.copied.is_empty());
    }
}
