// SPDX-License-Identifier: AGPL-3.0-or-later
//! `openchuck-ui`: the egui front end (architecture section 5.8).
//!
//! A window with menus, the project tree and the 3D viewport (work breakdown
//! slice 8; decision record 0026). It talks only to `openchuck-app` and
//! `openchuck-viewport`. Run `openchuck-ui [PROJECT_FOLDER]`.
//!
//! The stack is egui, `egui_wgpu` and wgpu (decision record 0002, accepted).

#![allow(clippy::print_stderr)]

mod shell;
mod viewport;

fn main() -> std::process::ExitCode {
    let folder = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        multisampling: 0,
        ..Default::default()
    };
    let result = eframe::run_native(
        "OpenChuckCAM",
        options,
        Box::new(move |cc| {
            let available = viewport::install(cc);
            Ok(Box::new(shell::Shell::new(available, folder)))
        }),
    );
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
