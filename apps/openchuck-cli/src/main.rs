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
use openchuck_app::model::{
    Axis, AxisDirection, Degrees, LengthUnit, ModelInspection, TransformStep, orient_to_rotary_axis,
};
use openchuck_app::project::ModelCheck;
use openchuck_app::{
    AppError, Application, Command, Query, QueryResult, Threads, VERIFY_GCODE_NOTICE, inspect_file,
};

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
    /// Print the summary and check report of a model file, or of every
    /// model in a project folder (IMP-01, IMP-03, IMP-04).
    Inspect {
        /// A model file, or a project folder.
        path: PathBuf,
        /// The unit a model file is in (mm, cm, m or in), overriding the
        /// file's own. Not used for a project, whose models keep their units.
        #[arg(long)]
        units: Option<String>,
    },
    /// Add a model file to a project (IMP-01).
    Import {
        /// The project folder.
        project: PathBuf,
        /// The model file. It is referenced by its path relative to the
        /// project folder, so keep it inside or beside the project.
        file: PathBuf,
        /// The unit the file is in (mm, cm, m or in), overriding the file's own.
        #[arg(long)]
        units: Option<String>,
    },
    /// Set the unit a project's model is in (IMP-03).
    Units {
        /// The project folder.
        project: PathBuf,
        /// The model, by ID or by its path in the project.
        model: String,
        /// mm, cm, m or in; or `file` to use the file's own unit.
        unit: String,
    },
    /// Add placement steps to a project's model, applied in the order given
    /// after its existing steps (IMP-05). Steps: `translate=X,Y,Z`,
    /// `rotate=AXIS:DEGREES`, `scale=FACTOR`, `mirror=AXIS`, `centre` (onto
    /// the rotary axis) and `orient=DIRECTION`, which turns the part's
    /// DIRECTION (such as +z) onto the rotary axis and centres it.
    Transform {
        /// The project folder.
        project: PathBuf,
        /// The model, by ID or by its path in the project.
        model: String,
        /// The steps to add.
        steps: Vec<String>,
        /// Remove the model's existing steps first.
        #[arg(long)]
        clear: bool,
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
                    ModelCheck::Unchanged => match app.inspect_model(id) {
                        // Mesh problems are the user's to judge: warn only.
                        Ok(inspection) => {
                            for problem in inspection.report.problems() {
                                eprintln!("warning: model '{path}': {problem}");
                            }
                        }
                        Err(e) => {
                            problems += 1;
                            eprintln!("model '{path}' cannot be read: {e}");
                        }
                    },
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
        Action::Inspect { path, units } => {
            if path.is_dir() {
                let app = open_project(&path, threads, create)?;
                let ids: Vec<_> = app.project().models().iter().map(|m| m.id).collect();
                if ids.is_empty() {
                    println!("{}: no models", path.display());
                }
                for (i, id) in ids.into_iter().enumerate() {
                    if i > 0 {
                        println!();
                    }
                    println!("id:        {id}");
                    print_inspection(&app.inspect_model(id)?);
                }
            } else {
                let units = units.as_deref().map(parse_unit).transpose()?;
                print_inspection(&inspect_file(&path, units, threads)?);
            }
        }
        Action::Import {
            project,
            file,
            units,
        } => {
            let units = units.as_deref().map(parse_unit).transpose()?;
            let mut app = open_project(&project, threads, create)?;
            let (id, inspection) = app.add_model(&file, units)?;
            app.save()?;
            println!("{}: added model {id}", project.display());
            print_inspection(&inspection);
        }
        Action::Units {
            project,
            model,
            unit,
        } => {
            let units = match unit.as_str() {
                "file" | "auto" => None,
                other => Some(parse_unit(other)?),
            };
            let mut app = open_project(&project, threads, create)?;
            let id = app.find_model(&model)?.id;
            app.execute(Command::SetModelUnits { model: id, units })?;
            app.save()?;
            print_inspection(&app.inspect_model(id)?);
        }
        Action::Transform {
            project,
            model,
            steps,
            clear,
        } => {
            let mut app = open_project(&project, threads, create)?;
            let reference = app.find_model(&model)?;
            let id = reference.id;
            let mut transform = if clear {
                Vec::new()
            } else {
                reference.transform.clone()
            };
            for step in &steps {
                transform.extend(parse_step(step)?);
            }
            app.execute(Command::SetModelTransform {
                model: id,
                transform,
            })?;
            app.save()?;
            print_inspection(&app.inspect_model(id)?);
        }
    }
    Ok(())
}

fn parse_unit(text: &str) -> Result<LengthUnit> {
    Ok(text.parse::<LengthUnit>()?)
}

/// Reads one placement step as the `transform` command takes it.
fn parse_step(text: &str) -> Result<Vec<TransformStep>> {
    let (name, value) = text.split_once('=').unwrap_or((text, ""));
    let number = |v: &str| -> Result<f64> {
        v.trim()
            .parse::<f64>()
            .with_context(|| format!("'{v}' in '{text}' is not a number"))
    };
    let axis = |v: &str| {
        Axis::parse(v).with_context(|| format!("'{v}' in '{text}' is not an axis (x, y or z)"))
    };
    let step = match name {
        "translate" => {
            let parts: Vec<&str> = value.split(',').collect();
            let [x, y, z] = parts.as_slice() else {
                anyhow::bail!("'{text}': give translate=X,Y,Z in millimetres");
            };
            TransformStep::Translate {
                x: number(x)?,
                y: number(y)?,
                z: number(z)?,
            }
        }
        "rotate" => {
            let (a, angle) = value.split_once(':').with_context(|| {
                format!("'{text}': give rotate=AXIS:DEGREES, such as rotate=z:90")
            })?;
            TransformStep::Rotate {
                axis: axis(a)?,
                angle: Degrees(number(angle)?),
            }
        }
        "scale" => TransformStep::Scale {
            factor: number(value)?,
        },
        "mirror" => TransformStep::Mirror { axis: axis(value)? },
        "centre" | "center" => TransformStep::CentreOnRotaryAxis,
        "orient" => {
            let along = AxisDirection::parse(value)
                .with_context(|| format!("'{text}': give orient=DIRECTION, such as orient=+z"))?;
            return Ok(orient_to_rotary_axis(along));
        }
        _ => anyhow::bail!(
            "'{text}' is not a step: use translate=X,Y,Z, rotate=AXIS:DEGREES, scale=FACTOR, \
             mirror=AXIS, centre or orient=DIRECTION"
        ),
    };
    Ok(vec![step])
}

/// Prints the most a list shows before it is cut short.
const LIST_LIMIT: usize = 10;

fn list<T: std::fmt::Debug>(items: &[T]) -> String {
    let shown: Vec<String> = items
        .iter()
        .take(LIST_LIMIT)
        .map(|i| format!("{i:?}"))
        .collect();
    let more = items.len().saturating_sub(LIST_LIMIT);
    if more > 0 {
        format!("{} and {more} more", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

/// Prints a model's summary and check report (slice 2 item 6).
fn print_inspection(inspection: &ModelInspection) {
    let summary = &inspection.summary;
    let report = &inspection.report;
    println!(
        "model:     {} ({})",
        inspection.path.display(),
        inspection.format.name()
    );
    println!("units:     {}", inspection.units.describe());
    if inspection.transform.is_empty() {
        println!("placement: none");
    } else {
        let steps: Vec<String> = inspection
            .transform
            .iter()
            .map(ToString::to_string)
            .collect();
        println!("placement: {}", steps.join("; "));
    }
    println!(
        "size:      {} triangles, {} vertices, {} edges, {} piece(s)",
        summary.triangles, summary.vertices, report.edges, report.shells
    );
    if let Some(b) = summary.bounds {
        println!(
            "bounds:    X {} to {}, Y {} to {}, Z {} to {} mm (model frame; X is the rotary axis)",
            b.min.x, b.max.x, b.min.y, b.max.y, b.min.z, b.max.z
        );
    }
    match report.volume {
        Some(volume) => println!(
            "area:      {:.3} mm2; closed, volume {volume:.3} mm3",
            summary.area
        ),
        None => println!(
            "area:      {:.3} mm2; not closed, so no volume",
            summary.area
        ),
    }
    let problems = report.problems();
    if problems.is_empty() {
        println!("checks:    no problems found");
    } else {
        println!("checks:    {} problem(s)", problems.len());
        for problem in &problems {
            println!("  - {problem}");
        }
        if !report.holes.is_empty() {
            let edges: Vec<[u32; 2]> = report.holes.iter().flat_map(|h| h.edges.clone()).collect();
            println!("  open edges: {}", list(&edges));
        }
        if !report.non_manifold_edges.is_empty() {
            println!("  non-manifold edges: {}", list(&report.non_manifold_edges));
        }
        if !report.flipped_triangles.is_empty() {
            println!("  flipped triangles: {}", list(&report.flipped_triangles));
        }
        if !report.normal_mismatches.is_empty() {
            println!(
                "  stated normals reversed: {}",
                list(&report.normal_mismatches)
            );
        }
        if !report.degenerate_triangles.is_empty() {
            println!(
                "  degenerate triangles: {}",
                list(&report.degenerate_triangles)
            );
        }
    }
    for note in &inspection.notes {
        println!("note:      {note}");
    }
}
