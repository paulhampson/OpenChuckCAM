// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writes the reference jobs as project folders to open in the application
//! (OI-913, OI-920; the Makera Z1 gate 4 instructions, step P2).
//!
//! ```text
//! cargo run -p openchuck-cli --example sample_projects -- <folder> [job ...]
//! ```
//!
//! Each committed folder in `testdata/projects/` is copied to
//! `<folder>/<job>` and its generated part is written beside it, so the
//! project opens and validates as it is. With no job named, every committed
//! job is written. An existing `<folder>/<job>` is never overwritten.

// An example reports its errors on the terminal and exits.
#![allow(clippy::print_stdout, clippy::print_stderr)]

#[path = "../tests/common/sample_projects.rs"]
mod sample_projects;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(out) = args.next().map(PathBuf::from) else {
        eprintln!("usage: sample_projects <folder> [job ...]");
        return ExitCode::FAILURE;
    };
    let named: Vec<String> = args.map(|a| a.to_string_lossy().into_owned()).collect();
    let jobs = if named.is_empty() {
        match sample_projects::committed_jobs() {
            Ok(jobs) => jobs,
            Err(e) => {
                eprintln!("cannot list the committed projects: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        named
    };
    match sample_projects::write_projects(&out, &jobs) {
        Ok(written) => {
            for folder in written {
                println!("{}", folder.display());
            }
            println!("{}", openchuck_app::VERIFY_GCODE_NOTICE);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
