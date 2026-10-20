// SPDX-License-Identifier: AGPL-3.0-or-later
//! `openchuck-ui`: the egui front end (architecture section 5.8).
//!
//! A window with menus, the project tree and the 3D viewport (work breakdown
//! slice 8; decision record 0029). It talks only to `openchuck-app` and
//! `openchuck-viewport`. The stack is egui, `egui_wgpu` and wgpu (decision
//! record 0002, accepted).
//!
//! - `openchuck-ui [PROJECT_FOLDER]` opens the application.
//! - `openchuck-ui --bench MODEL [--seconds N] [--mode shaded|edges|wireframe]
//!   [--out FILE]` runs the NFR-03 frame-time benchmark (verification plan
//!   4.2) and exits non-zero if it fails.
//!
//! A Windows release build is a GUI program: no console window opens from the
//! desktop. Started from a terminal it attaches to that terminal, so `--bench`
//! and error text print there as before. Everything is also written to a
//! local log file (`logging`; decision record 0155).
//!
//! On Windows the window uses DX12 when it offers a hardware adapter
//! (decision record 0059, accepted). `WGPU_BACKEND` (`dx12`, `vulkan`, `gl`)
//! overrides the choice, and `RUST_LOG` sets what is logged on standard error;
//! the adapter chosen is logged at `info`, which is on by default.

#![allow(clippy::print_stderr)]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;
use std::sync::{Arc, OnceLock};

mod about;
mod bench;
mod export_panel;
mod field_row;
mod floating;
mod form_panel;
mod graphics;
mod import_panel;
mod label_layout;
mod logging;
mod machine_panel;
mod model_panel;
mod playback_panel;
mod results_panel;
mod roles;
mod run_panel;
mod selection_panel;
mod setup_panes;
mod shell;
mod testing;
mod theme;
mod tool_panel;
mod toolbar;
mod viewport;
mod viewport_overlays;
mod viewport_toolbar;
mod workholding_panel;

fn main() -> ExitCode {
    logging::attach_parent_console();
    graphics::init_logging();
    logging::install_panic_hook();
    let args: Vec<String> = std::env::args().collect();
    match bench::Options::parse(&args) {
        Some(Ok(options)) => run_bench(options),
        Some(Err(message)) => {
            eprintln!("error: {message}");
            log::error!("{message}");
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
    options.wgpu_options.wgpu_setup = graphics::setup();
    if !vsync {
        options.wgpu_options.surface.present_mode =
            openchuck_viewport::wgpu::PresentMode::AutoNoVsync;
    }
    options
}

fn run_app(folder: Option<std::path::PathBuf>) -> ExitCode {
    // TL-12: a new user's tool library starts with the default tools. A
    // failure is reported in a dialog, never fatal (NFR-07); a missing
    // configuration folder (`None`) is a supported absence and stays silent.
    if let Some(Err(e)) = openchuck_app::library::install_default_tools_in_default_folder() {
        eprintln!("warning: could not install the default tools: {e}");
        log::warn!("could not install the default tools: {e}");
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("OpenChuckCAM")
            .set_description(format!("Could not install the default tools: {e}"))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
    let mut options = native_options(true);
    // The window opens at the size it had (UI-15). The first time it fills
    // the screen: at the default 120% scale a smaller window would start
    // below the 1366 × 768 logical pixels of UI-16.
    let size = openchuck_app::settings::SettingsFile::default_location()
        .and_then(|f| f.load().settings.window);
    options.viewport = match size {
        Some(size) => options.viewport.with_inner_size(size),
        None => options.viewport.with_maximized(true),
    };
    graphics::watch_for_stall();
    let result = eframe::run_native(
        "OpenChuckCAM",
        options,
        Box::new(move |cc| {
            graphics::reached(graphics::Stage::DeviceReady);
            graphics::log_adapter(cc.wgpu_render_state.as_ref());
            let available = viewport::install(cc);
            theme::install_fonts(&cc.egui_ctx);
            theme::install_style(&cc.egui_ctx, theme::Theme::Dark);
            // The scale the user chose, kept on this computer (UI-15).
            let shell = shell::Shell::new(available, folder);
            cc.egui_ctx.set_zoom_factor(shell.scale());
            Ok(Box::new(shell))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let message = graphics::failure_message(&error.to_string());
            eprintln!("error: {message}");
            log::error!("{message}");
            // Started from the desktop, standard error may not be seen.
            let _ = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("OpenChuckCAM")
                .set_description(logging::with_log_hint(&message))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
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
            graphics::log_adapter(cc.wgpu_render_state.as_ref());
            if !viewport::install(cc) {
                return Err("the benchmark needs the wgpu renderer".into());
            }
            let adapter = cc
                .wgpu_render_state
                .as_ref()
                .map_or_else(String::new, |s| graphics::describe(&s.adapter.get_info()));
            let run = bench::Run::new(options, adapter, shared);
            Ok(Box::new(shell::Shell::benchmark(app, run)))
        }),
    );
    let _ = std::fs::remove_dir_all(bench::folder());
    if let Err(error) = result {
        eprintln!("error: {}", graphics::failure_message(&error.to_string()));
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
