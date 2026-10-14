// SPDX-License-Identifier: AGPL-3.0-or-later
//! `openchuck-ui`: the egui front end (architecture section 5.8).
//!
//! A window with menus, the project tree and the 3D viewport (work breakdown
//! slice 8; decision record 0026). It talks only to `openchuck-app` and
//! `openchuck-viewport`. The stack is egui, `egui_wgpu` and wgpu (decision
//! record 0002, accepted).
//!
//! - `openchuck-ui [PROJECT_FOLDER]` opens the application.
//! - `openchuck-ui --bench MODEL [--seconds N] [--mode shaded|edges|wireframe]
//!   [--out FILE]` runs the NFR-03 frame-time benchmark (verification plan
//!   4.2) and exits non-zero if it fails.

#![allow(clippy::print_stderr)]

use std::process::ExitCode;
use std::sync::{Arc, OnceLock};

mod bench;
mod form_panel;
mod import_panel;
mod machine_panel;
mod model_panel;
mod shell;
mod tool_panel;
mod viewport;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match bench::Options::parse(&args) {
        Some(Ok(options)) => run_bench(options),
        Some(Err(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
        None => run_app(std::env::args_os().nth(1).map(std::path::PathBuf::from)),
    }
}

fn native_options(vsync: bool) -> eframe::NativeOptions {
    let mut options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        multisampling: 0,
        ..Default::default()
    };
    if !vsync {
        options.wgpu_options.surface.present_mode =
            openchuck_viewport::wgpu::PresentMode::AutoNoVsync;
    }
    options
}

fn run_app(folder: Option<std::path::PathBuf>) -> ExitCode {
    let result = eframe::run_native(
        "OpenChuckCAM",
        native_options(true),
        Box::new(move |cc| {
            let available = viewport::install(cc);
            Ok(Box::new(shell::Shell::new(available, folder)))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_bench(options: bench::Options) -> ExitCode {
    let app = match bench::project(&options.model) {
        Ok(app) => app,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = Arc::new(OnceLock::new());
    let shared = Arc::clone(&outcome);
    let mut native = native_options(false);
    // 4.2 names a 1080p display.
    native.viewport = native.viewport.with_inner_size([1920.0, 1080.0]);
    let result = eframe::run_native(
        "OpenChuckCAM benchmark",
        native,
        Box::new(move |cc| {
            if !viewport::install(cc) {
                return Err("the benchmark needs the wgpu renderer".into());
            }
            let adapter = cc.wgpu_render_state.as_ref().map_or_else(String::new, |s| {
                let info = s.adapter.get_info();
                format!(
                    "{} ({:?}, {} {})",
                    info.name, info.backend, info.driver, info.driver_info
                )
            });
            let run = bench::Run::new(options, adapter, shared);
            Ok(Box::new(shell::Shell::benchmark(app, run)))
        }),
    );
    let _ = std::fs::remove_dir_all(bench::folder());
    if let Err(error) = result {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    match outcome.get() {
        Some(true) => ExitCode::SUCCESS,
        Some(false) => ExitCode::from(2),
        None => {
            eprintln!("error: the benchmark did not finish");
            ExitCode::FAILURE
        }
    }
}
