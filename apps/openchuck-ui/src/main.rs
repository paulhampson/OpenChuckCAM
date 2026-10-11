// SPDX-License-Identifier: AGPL-3.0-or-later
//! `openchuck-ui`: the egui front end (architecture section 5.8).
//!
//! Slice 0 shell: it opens an empty window so the three-platform CI proves
//! that eframe with the wgpu backend builds (NFR-02, decision record 0002).
//! Panels, the project tree and the viewport arrive in slice 8.

#![allow(clippy::print_stderr)]

use openchuck_viewport as _;

struct Shell;

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("OpenChuckCAM");
        ui.label("Development shell: no project features yet.");
        ui.label(openchuck_app::VERIFY_GCODE_NOTICE);
    }
}

fn main() -> std::process::ExitCode {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    match eframe::run_native("OpenChuckCAM", options, Box::new(|_cc| Ok(Box::new(Shell)))) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
