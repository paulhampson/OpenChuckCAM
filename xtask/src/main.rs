// SPDX-License-Identifier: AGPL-3.0-or-later
//! Repository automation (architecture section 12, work breakdown slice 0 item 6).
//!
//! Run as `cargo xtask <check>`:
//!
//! - `deps`: dependency rules from architecture sections 4 and 12 (NFR-01, NFR-09, INT-01).
//! - `licences`: SPDX headers on source files and the licence field of every crate (NFR-10).
//! - `records`: every third-party dependency is pinned, declared once in the
//!   workspace and recorded in `docs/dependencies.md` (NFR-14).
//! - `trace`: the traceability report from requirement-ID test names (NFR-08).
//! - `testdata`: the test-data size policy.
//! - `all`: every check above.
//!
//! - `golden`: the golden file audit, which checks the committed G-code against the exact parts.
//! - `replay`: gate 3, LinuxCNC programs in the headless simulator (needs LinuxCNC; not part of `all`).
//!
//! The release gate is added in slice 20.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod deps;
mod golden;
mod licences;
mod records;
mod replay;
mod testdata;
mod trace;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "OpenChuckCAM repository checks")]
struct Cli {
    #[command(subcommand)]
    check: Check,
}

#[derive(Debug, Subcommand)]
enum Check {
    /// Dependency rules (architecture sections 4 and 12).
    Deps,
    /// SPDX headers and crate licence fields.
    Licences,
    /// Dependency pins, single declaration and NFR-14 records.
    Records,
    /// Traceability report from requirement-ID test names.
    Trace {
        /// Read test names from this file (one per line) instead of running `cargo test -- --list`.
        #[arg(long)]
        tests_from: Option<PathBuf>,
        /// Write the Markdown report here.
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Test-data size policy.
    Testdata,
    /// Audit the golden G-code files against the exact parts, the post conventions and the
    /// controller's feed rule, so they need no reading by hand.
    Golden,
    /// Gate 3: run LinuxCNC programs in the simulator and compare with the replay.
    /// With no program, every LinuxCNC golden program is run. Needs LinuxCNC:
    /// `--docker IMAGE`, `LINUXCNC_RIP`, or `linuxcnc` on the PATH.
    Replay {
        /// Programs to run (LinuxCNC dialect).
        programs: Vec<PathBuf>,
        /// Run LinuxCNC in this Docker image (see docker/linuxcnc).
        #[arg(long)]
        docker: Option<String>,
        /// Keep each simulator report in this folder.
        #[arg(long)]
        keep_reports: Option<PathBuf>,
        /// Largest per-line difference allowed, mm or degrees.
        #[arg(long, default_value_t = 0.1)]
        tolerance: f64,
        /// Give up on one program after this many seconds.
        #[arg(long, default_value_t = 900)]
        timeout: u64,
    },
    /// Every check.
    All,
}

/// Findings from one check: empty means it passed.
pub type Findings = Vec<String>;

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    let checks: Vec<(&str, Findings)> = match cli.check {
        Check::Deps => vec![("deps", deps::check(&root)?)],
        Check::Licences => vec![("licences", licences::check(&root)?)],
        Check::Records => vec![("records", records::check(&root)?)],
        Check::Trace { tests_from, report } => {
            vec![(
                "trace",
                trace::check(&root, tests_from.as_deref(), report.as_deref())?,
            )]
        }
        Check::Testdata => vec![("testdata", testdata::check(&root)?)],
        Check::Golden => vec![("golden", golden::check(&root)?)],
        Check::Replay {
            programs,
            docker,
            keep_reports,
            tolerance,
            timeout,
        } => vec![(
            "replay",
            replay::check(
                &root,
                &replay::Options {
                    programs,
                    docker,
                    keep_reports,
                    tolerance,
                    timeout_seconds: timeout,
                },
            )?,
        )],
        Check::All => vec![
            ("deps", deps::check(&root)?),
            ("licences", licences::check(&root)?),
            ("records", records::check(&root)?),
            ("testdata", testdata::check(&root)?),
            ("trace", trace::check(&root, None, None)?),
        ],
    };
    let mut ok = true;
    for (name, findings) in checks {
        if findings.is_empty() {
            println!("{name}: ok");
        } else {
            ok = false;
            println!("{name}: {} problem(s)", findings.len());
            for finding in findings {
                println!("  - {finding}");
            }
        }
    }
    Ok(ok)
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("xtask must live one level below the workspace root")
}
