// SPDX-License-Identifier: AGPL-3.0-or-later
//! `openchuck-cli`: the headless front end over `openchuck-app` (GC-11, NFR-08).
//!
//! Wiring only: every action goes through the application API.

// A command-line tool reports on standard output and standard error.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use openchuck_app::project::ModelCheck;
use openchuck_app::{AppError, Application, Query, QueryResult, Threads, VERIFY_GCODE_NOTICE};

#[derive(Debug, Parser)]
#[command(name = "openchuck-cli", version, about = "OpenChuckCAM without the UI")]
struct Cli {
    /// Worker threads (default: one per core).
    #[arg(long, global = true)]
    threads: Option<usize>,
    /// Create a project in a folder that has none, instead of asking.
    #[arg(long, global = true)]
    create: bool,
    #[command(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Create a new, empty project folder.
    New {
        /// The project folder (created if needed).
        project: PathBuf,
        /// The project's name (default: the folder name).
        #[arg(long)]
        name: Option<String>,
    },
    /// Check that a project opens, its files are valid and its model files
    /// match their recorded hashes.
    Validate {
        /// The project folder.
        project: PathBuf,
    },
    /// Load and save a project, rewriting only files whose text changes.
    Resave {
        /// The project folder.
        project: PathBuf,
    },
    /// Regenerate every toolpath and write the G-code.
    Regenerate {
        /// The project folder.
        project: PathBuf,
        /// Folder to write G-code files into (default: print a summary only).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print a summary of a project.
    Report {
        /// The project folder.
        project: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Opens the project in `folder`. A folder with no project gives a warning
/// and an offer to create one: `--create` accepts it, a terminal is asked, and
/// anything else stops with an error so a script never carries on by accident
/// (decision record 0008 item 8).
fn open_project(folder: &Path, threads: Threads, create: bool) -> Result<Application> {
    match Application::open(folder, threads) {
        Err(AppError::NoProject(path)) => {
            eprintln!(
                "warning: '{}' has no project.toml, so it is not a project yet",
                path.display()
            );
            let accepted = create || (std::io::stdin().is_terminal() && confirm_create());
            if !accepted {
                anyhow::bail!(
                    "no project in '{}'; run `openchuck-cli new {}` or pass --create",
                    path.display(),
                    path.display()
                );
            }
            let name = folder
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let app = Application::create(folder, &name, threads)?;
            println!("{}: created project '{name}'", folder.display());
            Ok(app)
        }
        other => Ok(other?),
    }
}

/// Asks on the terminal whether to create a project here.
fn confirm_create() -> bool {
    eprint!("Create a project here? [y/N] ");
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok()
        && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn run(cli: Cli) -> Result<()> {
    let threads = cli.threads.map_or(Threads::All, Threads::Exactly);
    let create = cli.create;
    match cli.command {
        Action::New { project, name } => {
            let name = name.unwrap_or_else(|| {
                project
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            Application::create(&project, &name, threads)?;
            println!("{}: created project '{name}'", project.display());
        }
        Action::Validate { project } => {
            let app = open_project(&project, threads, create)?;
            let mut problems = 0;
            for (id, check) in app.check_models()? {
                let model = app.project().models().iter().find(|m| m.id == id);
                let path = model.map(|m| m.path.as_str()).unwrap_or_default();
                match check {
                    ModelCheck::Unchanged => {}
                    ModelCheck::Changed { .. } => {
                        problems += 1;
                        eprintln!("model '{path}' has changed since it was added (PRJ-02)");
                    }
                    ModelCheck::Missing => {
                        problems += 1;
                        eprintln!("model '{path}' is missing");
                    }
                }
            }
            let long = app.project().long_order_keys();
            if !long.is_empty() {
                // A warning only: the project is valid (OI-35).
                eprintln!(
                    "warning: {} items have long order keys; renumbering their list would shorten them",
                    long.len()
                );
            }
            if problems > 0 {
                anyhow::bail!("{}: {problems} model problem(s)", project.display());
            }
            app.regenerate()?;
            println!("{}: valid", project.display());
        }
        Action::Resave { project } => {
            let app = open_project(&project, threads, create)?;
            app.save()?;
            println!("{}: saved", project.display());
        }
        Action::Regenerate { project, out } => {
            let app = open_project(&project, threads, create)?;
            let result = app.regenerate()?;
            if let Some(out) = &out {
                std::fs::create_dir_all(out)
                    .with_context(|| format!("cannot create output folder '{}'", out.display()))?;
                for (name, text) in &result.gcode {
                    let path = out.join(name);
                    std::fs::write(&path, text)
                        .with_context(|| format!("cannot write '{}'", path.display()))?;
                }
            }
            println!(
                "{}: {} toolpath groups, {} G-code files",
                project.display(),
                result.toolpath.groups.len(),
                result.gcode.len()
            );
            // NFR-12: always printed when G-code is generated.
            println!("{VERIFY_GCODE_NOTICE}");
        }
        Action::Report { project } => {
            let app = open_project(&project, threads, create)?;
            let summary = match app.query(Query::Summary) {
                QueryResult::Summary(summary) => summary,
                other => anyhow::bail!("unexpected answer to a summary query: {other:?}"),
            };
            println!("project:    {}", summary.location.display());
            println!("models:     {}", summary.models);
            println!("setups:     {}", summary.setups);
            println!("operations: {}", summary.operations);
        }
    }
    Ok(())
}
