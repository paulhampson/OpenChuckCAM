// SPDX-License-Identifier: AGPL-3.0-or-later
//! The import dialog's step 2 drawn whole at the sizes a user can reach
//! (UI-16, UI-10; issue 222, OI-580): every control is inside the dialog,
//! nothing is drawn over anything else, and the preview's readout does not
//! sit on the dialog.

#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use eframe::egui;

use super::frame_tests::Rig;
use crate::import_panel::ImportSession;

const CASES: [(f32, f32, f32); 4] = [
    (1366.0, 768.0, 1.0),
    (1366.0, 768.0, 1.2),
    (1920.0, 1080.0, 1.0),
    (1920.0, 1080.0, 1.2),
];

/// What step 2 must show, each by a text on its control.
const CONTROLS: [&str; 13] = [
    "FILE",
    "Browse…",
    "Check and preview",
    "UNITS",
    "mm",
    "Largest side after conversion",
    "PLACEMENT",
    "Centre on the rotary axis",
    "MESH CHECK",
    "Copy the file into the project folder",
    "The file is not changed. Its path and hash are stored in the project.",
    "Cancel",
    "Add to project",
];

fn model(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

fn dialog_rect(rig: &Rig) -> egui::Rect {
    rig.ctx
        .memory(|m| m.area_rect(egui::Id::new(("dialog", "import model"))))
        .expect("the dialog is open")
}

/// Opens step 2 on `file`; false when this build cannot read the file's format.
fn step_two(rig: &mut Rig, file: std::path::PathBuf) -> bool {
    let app = rig.shell.app.as_ref().unwrap();
    rig.shell.import = Some(ImportSession::new(app, file));
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.import.as_ref().unwrap().preview.is_none() {
        rig.frame(Vec::new());
        let session = rig.shell.import.as_ref().unwrap();
        if let Some(e) = &session.error
            && e.contains("STEP reader program")
        {
            return false;
        }
        assert!(
            Instant::now() < deadline,
            "timed out at {:?}: {:?}",
            rig.size,
            session.error
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    for _ in 0..8 {
        rig.frame(Vec::new());
    }
    true
}

fn check(rig: &mut Rig, file: &std::path::Path, label: &str) {
    for (w, h, scale) in CASES {
        rig.ctx.set_pixels_per_point(scale);
        rig.size = egui::vec2(w / scale, h / scale);
        if !step_two(rig, file.to_path_buf()) {
            eprintln!("{label}: the STEP reader program is not built here; skipped");
            return;
        }
        let dialog = dialog_rect(rig);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, rig.size);
        let case = format!("{label} {w}x{h} at {scale}: dialog {dialog:?}");
        assert!(
            screen.expand(0.5).contains_rect(dialog),
            "{case}: inside the window"
        );
        let texts = rig.text_rects();
        // The dialog's window is drawn after the main window's panels and the view's
        // overlays, so its texts are those from its first label on.
        let first = texts
            .iter()
            .rposition(|(t, _)| t == "FILE")
            .expect("the dialog's FILE label is drawn");
        // The path field clips its own text, so its text is not a rectangle that is drawn.
        let inside: Vec<_> = texts[first..]
            .iter()
            .filter(|(t, _)| !t.ends_with(".stl") && !t.ends_with(".step"))
            .collect();
        // Every control is whole inside the dialog.
        for want in CONTROLS {
            let found = inside.iter().find(|(t, _)| t.contains(want));
            let (_, r) = found.unwrap_or_else(|| panic!("{case}: {want} is not in the dialog"));
            assert!(dialog.contains_rect(*r), "{case}: {want} {r:?} is cut off");
        }
        // Nothing is drawn over anything else. The body scrolls between the header and the
        // footer (UI-16), so what it clips away under them is not drawn: only the visible part
        // of each text counts.
        let visible = rig.visible_text_rects();
        let first_visible = visible
            .iter()
            .rposition(|(t, _)| t == "FILE")
            .expect("the dialog's FILE label is visible");
        let shown: Vec<_> = visible[first_visible..]
            .iter()
            .filter(|(t, _)| !t.ends_with(".stl") && !t.ends_with(".step"))
            .collect();
        for (i, (a, ra)) in shown.iter().enumerate() {
            for (b, rb) in shown.iter().skip(i + 1) {
                let overlap = ra.intersect(*rb);
                assert!(
                    overlap.width() < 1.0 || overlap.height() < 1.0,
                    "{case}: {a:?} {ra:?} overlaps {b:?} {rb:?}"
                );
            }
        }
        // The readout and the legend belong to the view and are drawn under the dialog, never
        // over it (the owner's screenshot of issue 222).
        for want in ["Size X", "Volume", "Rotary axis"] {
            assert!(
                texts[..first].iter().any(|(t, _)| t == want),
                "{case}: {want} is drawn under the dialog"
            );
            assert!(
                inside.iter().all(|(t, _)| t != want),
                "{case}: {want} is drawn over the dialog"
            );
        }
    }
}

#[test]
fn req_ui_16_req_imp_03_step_two_of_an_stl_import_lays_out_inside_the_dialog() {
    let mut rig = Rig::cube("import-layout-stl", None);
    let dir = std::env::temp_dir().join(format!("import-long-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A long file name, as a user's folders give.
    let long = dir.join(format!("{}.stl", "a_very_long_model_file_name_".repeat(6)));
    std::fs::copy(model("stl/cube_ascii.stl"), &long).unwrap();
    check(&mut rig, &model("stl/cube_ascii.stl"), "stl");
    check(&mut rig, &long, "long name");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn req_ui_16_req_imp_03_step_two_of_a_step_import_lays_out_inside_the_dialog() {
    let mut rig = Rig::cube("import-layout-step", None);
    check(&mut rig, &model("step/valid/box.step"), "step");
}

/// The tool library and export dialogs at the same sizes: each is inside the main window, its
/// footer buttons are whole inside it, and no text is drawn over them (UI-16, UI-10).
fn check_dialog(rig: &mut Rig, key: &str, title: &str, buttons: &[&str], open: fn(&mut Rig)) {
    for (w, h, scale) in CASES {
        rig.ctx.set_pixels_per_point(scale);
        rig.size = egui::vec2(w / scale, h / scale);
        open(rig);
        for _ in 0..10 {
            rig.frame(Vec::new());
        }
        let dialog = rig
            .ctx
            .memory(|m| m.area_rect(egui::Id::new(("dialog", key))))
            .expect("the dialog is open");
        let case = format!("{title} {w}x{h} at {scale}: dialog {dialog:?}");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, rig.size);
        assert!(
            screen.expand(0.5).contains_rect(dialog),
            "{case}: inside the window"
        );
        let texts = rig.text_rects();
        let first = texts
            .iter()
            .rposition(|(t, _)| t == title)
            .expect("the dialog's title is drawn");
        let inside: Vec<_> = texts[first..].iter().collect();
        let visible = rig.visible_text_rects();
        let first_visible = visible
            .iter()
            .rposition(|(t, _)| t == title)
            .expect("the dialog's title is visible");
        let seen: Vec<_> = visible[first_visible..].iter().collect();
        for want in buttons {
            let (_, r) = inside
                .iter()
                .find(|(t, _)| t == want)
                .unwrap_or_else(|| panic!("{case}: {want} is not drawn"));
            assert!(dialog.contains_rect(*r), "{case}: {want} {r:?} is cut off");
        }
        // A wrapped label's own rectangle starts at its line start, so only the buttons are
        // held against every other text.
        for want in buttons {
            let (_, button) = seen.iter().find(|(t, _)| t == want).unwrap();
            for (other, r) in seen.iter().filter(|(t, _)| t != want) {
                let overlap = button.intersect(*r);
                assert!(
                    overlap.width() < 1.0 || overlap.height() < 1.0,
                    "{case}: {want} {button:?} overlaps {other:?} {r:?}"
                );
            }
        }
    }
}

#[test]
fn req_ui_16_the_tool_library_dialog_lays_out_inside_the_window() {
    use crate::tool_library_window::Purpose;
    let mut rig = Rig::cube("dialog-layout-library", None);
    check_dialog(
        &mut rig,
        "tool library",
        "Tool library",
        &["Import library…", "Export library…", "Close"],
        |rig| rig.shell.open_tool_library(Purpose::Browse, None),
    );
}

#[test]
fn req_ui_16_the_export_dialog_lays_out_inside_the_window() {
    let mut rig = Rig::cube("dialog-layout-export", None);
    check_dialog(
        &mut rig,
        "export dialog",
        "Post and export",
        &["Cancel", "Write G-code"],
        |rig| {
            if !rig.shell.export.is_open() {
                let app = rig.shell.app.as_ref().unwrap();
                rig.shell.export.open(app);
            }
        },
    );
}
