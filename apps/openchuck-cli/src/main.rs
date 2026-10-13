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
    Axis, AxisDirection, Degrees, LengthUnit, ModelInspection, TransformStep, UnitConcern,
    UnitQuestion, orient_to_rotary_axis,
};
use openchuck_app::project::id::SetupId;
use openchuck_app::project::{Item, ModelCheck, Placement, Setup};
use openchuck_app::setup::{
    ARange, Aabb, Clearance, Confirmation, MachineProfile, Millimetres, Point3, SetupReport, Stock,
    WorkOffsetRegister, a_direction, builtin, builtin_keys, list_library, load_profile,
};
use openchuck_app::tools::{
    ControllerMapping, LibraryTool, NumberSource, SnapshotStatus, ToolLibrary, ToolNumbering,
    ToolReport, ToolSet, check_tools, default_controller, describe, describe_controller, find_tool,
    load_profile as load_tool_profile, tool_number_plan, tool_numbering_command,
    with_custom_profile,
};
use openchuck_app::{
    AppError, Application, Command, PostSelection, Query, QueryResult, Threads,
    VERIFY_GCODE_NOTICE, inspect_file,
};

/// What to do with the tool numbers when G-code is written.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum NumberChoice {
    /// Confirm the numbers as they are.
    Keep,
    /// Renumber in order of first use.
    Auto,
}

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
        /// The post: a built-in id (`makera-z1`, `linuxcnc`) or a post folder.
        /// Without one only toolpaths are generated.
        #[arg(long)]
        post: Option<String>,
        /// Tool numbers for the G-code (GC-14): `keep` confirms the numbers
        /// shown, `auto` renumbers in order of first use. On a terminal the
        /// numbers are shown and you are asked; otherwise this is required.
        #[arg(long, value_enum)]
        tool_numbers: Option<NumberChoice>,
    },
    /// Compare a LinuxCNC simulator report with the replay of the G-code file
    /// it ran (gate 3): every move must reach its position, LinuxCNC must
    /// report no fault, and the program must end where the replay says.
    GcodeSim {
        /// The G-code file the simulator ran (LinuxCNC dialect).
        program: PathBuf,
        /// The simulator driver's JSON report.
        report: PathBuf,
        /// Largest allowed difference per line in mm or degrees. The driver
        /// samples about once a millisecond and LinuxCNC's line number lags
        /// the motion, so a line can seem to end 0.05 degrees short at
        /// 45 degrees a second; the final position is held to 0.001.
        #[arg(long, default_value_t = 0.1)]
        tolerance: f64,
    },
    /// Show the tool numbers the program will use, or change them (GC-14).
    /// The choice is saved in the project.
    ToolNumbers {
        /// The project folder.
        project: PathBuf,
        /// Renumber in order of first use, from 1.
        #[arg(long)]
        auto: bool,
        /// Give a tool a number by hand: `TOOL=NUMBER` (the tool by name, ID
        /// or `T<n>`). Repeat for more tools.
        #[arg(long = "set", value_name = "TOOL=NUMBER")]
        set: Vec<String>,
        /// The machine's largest tool number, if it has a limit.
        #[arg(long)]
        max: Option<u32>,
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
        /// The unit a model file is in (um, mm, cm, m or in); millimetres if
        /// not given. Not used for a project, whose models keep their units.
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
        /// The unit the file is in (um, mm, cm, m or in). Without it the file
        /// is read in mm, and a terminal is asked if the file names another
        /// unit or the part's size is implausible in mm.
        #[arg(long)]
        units: Option<String>,
    },
    /// List the built-in machine profiles and those in a library folder
    /// (SET-01).
    Machines {
        /// A folder of machine profile files.
        #[arg(long)]
        library: Option<PathBuf>,
    },
    /// Show a project's machine profile, or set it from a built-in profile
    /// (by key or name) or a profile file. The project keeps its own copy
    /// (SET-01).
    Machine {
        /// The project folder.
        project: PathBuf,
        /// A built-in profile's key or name (see `machines`), or a profile
        /// file; omit to show the current one.
        profile: Option<String>,
        /// A folder of machine profile files to look in by file stem.
        #[arg(long)]
        library: Option<PathBuf>,
        /// Remove the project's machine profile.
        #[arg(long, conflicts_with = "profile")]
        clear: bool,
    },
    /// Add or edit a setup: part, stock, material, part and work offsets and
    /// clearance (SET-03, SET-05, SET-06), then print its checks. Without
    /// options it only prints them.
    Setup {
        /// The project folder.
        project: PathBuf,
        /// The setup, by ID or name. Omit with --new.
        setup: Option<String>,
        /// Add a new setup with this name.
        #[arg(long, conflicts_with = "setup")]
        new: Option<String>,
        /// Rename the setup.
        #[arg(long)]
        name: Option<String>,
        /// The model machined in this setup, by ID or path, or `none`.
        #[arg(long)]
        part: Option<String>,
        /// `cylinder=DIAMETER,LENGTH,X_START`, `box=X0,Y0,Z0,X1,Y1,Z1`,
        /// `bbox=OFFSET` (the part's bounding box grown by OFFSET), or `none`.
        /// Millimetres, in the model frame.
        #[arg(long)]
        stock: Option<String>,
        /// The stock material's name, or `none`.
        #[arg(long)]
        material: Option<String>,
        /// Distance along the rotary axis from the machine's rotary reference
        /// point to the model origin, in mm.
        #[arg(long, allow_hyphen_values = true)]
        part_offset: Option<f64>,
        /// The work offset register (G54 to G59).
        #[arg(long)]
        work_offset: Option<String>,
        /// Model X of the work zero, which is on the rotary axis, in mm.
        #[arg(long, allow_hyphen_values = true)]
        work_x: Option<f64>,
        /// `RADIUS,SAFE_Z` in mm from the rotary axis, or `machine` to use
        /// the machine profile's.
        #[arg(long)]
        clearance: Option<String>,
    },
    /// List a project's tool snapshots with their checks, and compare them
    /// with a tool library folder (TL-07). The snapshots always win;
    /// differences are only reported.
    Tools {
        /// The project folder.
        project: PathBuf,
        /// A tool library folder to compare with.
        #[arg(long)]
        library: Option<PathBuf>,
    },
    /// Show a project's tool; add one from a tool library; update a snapshot
    /// from the library; set its tool number and offsets; or remove it
    /// (TL-06, TL-07, TL-11). The project keeps its own copy.
    Tool {
        /// The project folder.
        project: PathBuf,
        /// The tool: by ID, name or `T<number>` in the project, or with
        /// --add by ID or name in the library (or `T<number>` in the project
        /// machine's tool set).
        tool: String,
        /// Copy the tool from the library into the project. Its number comes
        /// from the machine's tool set, or is the next free one, unless
        /// --number is given.
        #[arg(long)]
        add: bool,
        /// Replace the project's snapshot with the library's version, keeping
        /// the project's tool number.
        #[arg(long, conflicts_with = "add")]
        update: bool,
        /// The tool library folder for --add and --update.
        #[arg(long)]
        library: Option<PathBuf>,
        /// Set the tool number (T).
        #[arg(long)]
        number: Option<u32>,
        /// Set the length offset register (H), or 0 to follow the tool number.
        #[arg(long)]
        length_offset: Option<u32>,
        /// Set the diameter offset register (D), or 0 to follow the tool number.
        #[arg(long)]
        diameter_offset: Option<u32>,
        /// Remove the tool from the project.
        #[arg(long, conflicts_with_all = ["add", "update"])]
        remove: bool,
    },
    /// Work with a tool library: a folder with one file per tool and machine
    /// tool sets in `tool-sets/`, stored separately from projects (TL-01,
    /// TL-11).
    Library {
        /// The tool library folder (created when first written).
        library: PathBuf,
        #[command(subcommand)]
        action: LibraryAction,
    },
    /// Set the unit a project's model is in (IMP-03).
    Units {
        /// The project folder.
        project: PathBuf,
        /// The model, by ID or by its path in the project.
        model: String,
        /// um, mm, cm, m or in; or `default` to unset it (read in mm, and
        /// asked again if the file suggests another unit).
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

#[derive(Debug, Subcommand)]
enum LibraryAction {
    /// List the library's tools, tool sets and unreadable files.
    List,
    /// Copy tools from another library folder or a single tool file: new
    /// tools get new files, a tool with the same ID replaces the library's.
    Import {
        /// The library folder or tool file to read.
        from: PathBuf,
    },
    /// Write some or all of the library's tools into another folder.
    Export {
        /// The folder to write into.
        to: PathBuf,
        /// The tools, by ID or name (default: all).
        tools: Vec<String>,
    },
    /// Give a library tool the custom profile in a profile text file
    /// (TL-03); its diameter becomes the profile's end radius doubled.
    Profile {
        /// The tool, by ID or name.
        tool: String,
        /// The custom profile file.
        file: PathBuf,
    },
    /// Copy a project's tool snapshot into the library, replacing the
    /// library tool with the same ID. The project's tool number stays in the
    /// project.
    Store {
        /// The project folder.
        project: PathBuf,
        /// The tool, by ID, name or `T<number>` in the project.
        tool: String,
    },
    /// Show or edit a machine's default tool set (TL-11): the tool numbers a
    /// project for that machine gives library tools.
    ToolSet {
        /// The machine profile's name, for example "Makera Z1".
        machine: String,
        /// `NUMBER=TOOL` (tool by ID or name): give the tool this number,
        /// replacing whatever had the number or the tool before.
        #[arg(long = "set")]
        set: Vec<String>,
        /// Remove the tool with this number from the set.
        #[arg(long = "unset")]
        unset: Vec<u32>,
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
                            if let Some(question) = &inspection.unit_question {
                                eprintln!("warning: model '{path}': {}", question.message());
                            }
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
            // Setup problems are warnings: the user may have reasons (SET-06).
            if app.project().machine().is_none() && !app.project().setups().is_empty() {
                eprintln!(
                    "warning: the project has no machine profile; choose one with `openchuck-cli machine`"
                );
            }
            for report in app.check_setups()? {
                for warning in &report.warnings {
                    eprintln!("warning: setup '{}': {warning}", report.name);
                }
                for note in report
                    .notes
                    .iter()
                    .filter(|n| !n.contains("no machine profile"))
                {
                    eprintln!("warning: setup '{}': {note}", report.name);
                }
            }
            for report in check_tools(app.project(), None) {
                for warning in &report.warnings {
                    eprintln!("warning: tool '{}': {warning}", report.name);
                }
            }
            for note in app.regenerate()?.notes {
                eprintln!("warning: {note}");
            }
            println!("{}: valid", project.display());
        }
        Action::Resave { project } => {
            let app = open_project(&project, threads, create)?;
            app.save()?;
            println!("{}: saved", project.display());
        }
        Action::Regenerate {
            project,
            out,
            post,
            tool_numbers,
        } => {
            let mut app = open_project(&project, threads, create)?;
            if out.is_some() && post.is_none() {
                anyhow::bail!("--out needs --post: say which post to write the G-code with");
            }
            if post.is_some() {
                confirm_tool_numbers(&mut app, tool_numbers)?;
            }
            app.set_post(post.as_deref().map(PostSelection::parse));
            let result = app.regenerate()?;
            for note in &result.notes {
                eprintln!("warning: {note}");
            }
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
            let mut unclean = 0;
            for report in &result.reports {
                match &report.comparison {
                    Ok(c) => println!(
                        "  {}: {} moves; replay matches the toolpath (tip {:.4} mm, A {:.4} deg; cut {:.1} s, toolpath {:.1} s)",
                        report.file,
                        report.moves,
                        c.max_position_error,
                        c.max_a_error,
                        c.gcode_cut_seconds,
                        c.ir_cut_seconds
                    ),
                    Err(why) => println!(
                        "  {}: replay does not match the toolpath: {why}",
                        report.file
                    ),
                }
                for finding in &report.findings {
                    println!("  {}: check: {finding}", report.file);
                }
                if !report.is_clean() {
                    unclean += 1;
                }
            }
            // NFR-12: always printed when G-code is generated.
            println!("{VERIFY_GCODE_NOTICE}");
            if unclean > 0 {
                anyhow::bail!("{unclean} G-code file(s) failed the checks above");
            }
        }
        Action::GcodeSim {
            program,
            report,
            tolerance,
        } => {
            use openchuck_app::gcode::{
                Dialect, compare_simulator, parse_sim_report, read, replay,
            };
            let text = std::fs::read_to_string(&program)
                .with_context(|| format!("cannot read '{}'", program.display()))?;
            let read = read(&text).map_err(|errors| {
                anyhow::anyhow!(
                    "{}: {}",
                    program.display(),
                    errors.first().map_or_else(String::new, ToString::to_string)
                )
            })?;
            let replayed = replay(&read, Dialect::Linuxcnc)
                .map_err(|e| anyhow::anyhow!("{}: {e}", program.display()))?;
            let text = std::fs::read_to_string(&report)
                .with_context(|| format!("cannot read '{}'", report.display()))?;
            let sim = parse_sim_report(&text)?;
            let found = compare_simulator(&replayed, &sim, tolerance)?;
            println!(
                "{}: the simulator reached all {} moves (largest difference {:.4}, {:.1} s of motion) and reported no fault",
                program.display(),
                found.moves,
                found.max_error,
                found.seconds
            );
        }
        Action::ToolNumbers {
            project,
            auto,
            set,
            max,
        } => {
            let mut app = open_project(&project, threads, create)?;
            print_tool_numbers(&app);
            if auto || !set.is_empty() {
                let numbering = if auto {
                    if !set.is_empty() {
                        anyhow::bail!("use --auto or --set, not both");
                    }
                    ToolNumbering::InOrderOfUse { first: 1 }
                } else {
                    let mut pairs = Vec::new();
                    for item in &set {
                        let (tool, number) = item
                            .split_once('=')
                            .with_context(|| format!("--set '{item}': write TOOL=NUMBER"))?;
                        let number: u32 = number.trim().parse().with_context(|| {
                            format!("--set '{item}': '{number}' is not a tool number")
                        })?;
                        let found = find_tool(app.project(), tool).with_context(|| {
                            format!("--set '{item}': no single tool matches '{tool}'")
                        })?;
                        pairs.push((found.id, number));
                    }
                    ToolNumbering::Manual(pairs)
                };
                match tool_numbering_command(app.project(), &numbering, max)? {
                    Some(command) => {
                        app.execute(command)?;
                        app.save()?;
                        println!("tool numbers changed and saved:");
                        print_tool_numbers(&app);
                    }
                    None => println!("tool numbers unchanged"),
                }
            }
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
            match app.project().machine() {
                Some(profile) => {
                    println!();
                    print_machine(profile);
                }
                None => println!("machine:    none"),
            }
            for report in app.check_setups()? {
                println!();
                print_setup(&app, &report);
            }
            for report in check_tools(app.project(), None) {
                println!();
                print_tool(&app, &report);
            }
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
            let (id, mut inspection) = app.add_model(&file, units)?;
            println!("{}: added model {id}", project.display());
            // The file names another unit, or its size is implausible in mm:
            // ask on a terminal; otherwise leave it unanswered and say how.
            if let Some(question) = &inspection.unit_question {
                if std::io::stdin().is_terminal() {
                    let answer = ask_unit(question);
                    app.execute(Command::SetModelUnits {
                        model: id,
                        units: Some(answer),
                    })?;
                    inspection = app.inspect_model(id)?;
                } else {
                    eprintln!("warning: {}", question.message());
                    eprintln!(
                        "  set it with `openchuck-cli units {} {id} <unit>`, or `mm` to keep millimetres",
                        project.display()
                    );
                }
            }
            app.save()?;
            print_inspection(&inspection);
        }
        Action::Machines { library } => {
            for key in builtin_keys() {
                let profile = builtin(key)
                    .transpose()?
                    .with_context(|| format!("no built-in {key}"))?;
                println!(
                    "{key:<16} {} (built in; {} of {} values unconfirmed)",
                    profile.name,
                    profile.unconfirmed_fields().len(),
                    openchuck_app::setup::ProfileField::ALL.len()
                );
            }
            if let Some(dir) = library {
                for (path, profile) in list_library(&dir)? {
                    let stem = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    match profile {
                        Ok(p) => println!("{stem:<16} {} ({})", p.name, path.display()),
                        Err(e) => eprintln!("warning: {e}"),
                    }
                }
            }
        }
        Action::Machine {
            project,
            profile,
            library,
            clear,
        } => {
            let mut app = open_project(&project, threads, create)?;
            if clear {
                app.execute(Command::SetMachine(None))?;
                app.save()?;
                println!("{}: machine profile removed", project.display());
            } else if let Some(text) = profile {
                let profile = find_profile(&text, library.as_deref())?;
                app.execute(Command::SetMachine(Some(Box::new(profile))))?;
                app.save()?;
            }
            match app.project().machine() {
                Some(profile) => print_machine(profile),
                None => println!("{}: no machine profile", project.display()),
            }
        }
        Action::Setup {
            project,
            setup,
            new,
            name,
            part,
            stock,
            material,
            part_offset,
            work_offset,
            work_x,
            clearance,
        } => {
            let mut app = open_project(&project, threads, create)?;
            let mut edited = match (&new, &setup) {
                (Some(name), _) => Setup::new(SetupId::new(), name.clone()),
                (None, Some(text)) => app.find_setup(text)?.clone(),
                (None, None) => anyhow::bail!("name a setup, or add one with --new NAME"),
            };
            if let Some(name) = name {
                edited.name = name;
            }
            if let Some(part) = part {
                edited.part = match part.as_str() {
                    "none" => None,
                    text => Some(app.find_model(text)?.id),
                };
            }
            if let Some(stock) = stock {
                edited.stock = parse_stock(&stock)?;
            }
            if let Some(material) = material {
                edited.material = (material != "none").then_some(material);
            }
            if let Some(offset) = part_offset {
                edited.frame.part_offset = Millimetres(offset);
            }
            if let Some(register) = work_offset {
                edited.frame.work_offset.register = WorkOffsetRegister::parse(&register)
                    .with_context(|| {
                        format!("'{register}' is not a work offset; use G54 to G59")
                    })?;
            }
            if let Some(x) = work_x {
                edited.frame.work_offset.x = Millimetres(x);
            }
            if let Some(clearance) = clearance {
                edited.clearance = parse_clearance(&clearance)?;
            }
            let id = edited.id;
            let command = if new.is_some() {
                Command::Add {
                    item: Item::Setup(edited),
                    placement: Placement::End,
                }
            } else {
                Command::Replace(Item::Setup(edited))
            };
            app.execute(command)?;
            app.save()?;
            let report = app
                .check_setups()?
                .into_iter()
                .find(|r| r.setup == id)
                .context("the setup is missing after the edit")?;
            print_setup(&app, &report);
        }
        Action::Tools { project, library } => {
            let app = open_project(&project, threads, create)?;
            let library = open_library(library.as_deref())?;
            if app.project().tools().is_empty() {
                println!("{}: no tools", project.display());
            }
            for (i, report) in check_tools(app.project(), library.as_ref())
                .iter()
                .enumerate()
            {
                if i > 0 {
                    println!();
                }
                print_tool(&app, report);
            }
        }
        Action::Tool {
            project,
            tool,
            add,
            update,
            library,
            number,
            length_offset,
            diameter_offset,
            remove,
        } => {
            let mut app = open_project(&project, threads, create)?;
            let library = open_library(library.as_deref())?;
            let need_library = || {
                library
                    .as_ref()
                    .context("--add and --update need --library FOLDER")
            };
            let id = if add {
                let entry = find_library_tool(need_library()?, app.project(), &tool)?;
                if app.project().tools().iter().any(|t| t.id == entry.id) {
                    anyhow::bail!(
                        "the project already has \"{}\"; use --update to refresh it from the library",
                        entry.name
                    );
                }
                let (controller, source) =
                    default_controller(app.project(), library.as_ref(), entry.id);
                app.execute(Command::Add {
                    item: Item::Tool(entry.snapshot(controller)),
                    placement: Placement::End,
                })?;
                let how = match source {
                    NumberSource::ToolSet => "from the machine's tool set".to_owned(),
                    NumberSource::ToolSetTaken { wanted } => format!(
                        "the next free number; the machine's tool set gives T{wanted}, which another tool here uses"
                    ),
                    NumberSource::NextFree => "the next free number".to_owned(),
                };
                println!(
                    "{}: added tool \"{}\" as T{} ({how})",
                    project.display(),
                    entry.name,
                    controller.number
                );
                entry.id
            } else {
                find_tool(app.project(), &tool)
                    .with_context(|| format!("the project has no single tool named \"{tool}\""))?
                    .id
            };
            let snapshot = app
                .project()
                .tools()
                .iter()
                .find(|t| t.id == id)
                .context("the tool is missing from the project")?
                .clone();
            if remove {
                app.execute(Command::Remove(openchuck_app::project::ItemId::Tool(id)))?;
                app.save()?;
                println!("{}: removed tool \"{}\"", project.display(), snapshot.name);
                return Ok(());
            }
            let mut edited = snapshot.clone();
            if update {
                let entry = need_library()?.get(id).with_context(|| {
                    format!(
                        "the library has no tool with the ID of \"{}\"",
                        snapshot.name
                    )
                })?;
                edited = entry.snapshot(snapshot.controller);
                edited.order = snapshot.order.clone();
            }
            if let Some(n) = number {
                edited.controller.number = n;
            }
            if let Some(h) = length_offset {
                edited.controller.length_offset = (h != 0).then_some(h);
            }
            if let Some(d) = diameter_offset {
                edited.controller.diameter_offset = (d != 0).then_some(d);
            }
            if edited != snapshot {
                app.execute(Command::Replace(Item::Tool(edited)))?;
                if update {
                    println!(
                        "{}: updated tool \"{}\" from the library",
                        project.display(),
                        snapshot.name
                    );
                }
            }
            app.save()?;
            let report = check_tools(app.project(), library.as_ref())
                .into_iter()
                .find(|r| r.tool == id)
                .context("the tool is missing after the edit")?;
            print_tool(&app, &report);
        }
        Action::Library { library, action } => run_library(&library, action, threads, create)?,
        Action::Units {
            project,
            model,
            unit,
        } => {
            let units = match unit.as_str() {
                "default" => None,
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

/// Opens the tool library folder `path`, if given, and warns about files it
/// skipped.
fn open_library(path: Option<&Path>) -> Result<Option<ToolLibrary>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let library = ToolLibrary::open(path)?;
    for problem in library.problems() {
        eprintln!("warning: {problem}");
    }
    Ok(Some(library))
}

/// A library tool by ID or name, or `T<number>` in the tool set of the
/// project's machine.
fn find_library_tool<'a>(
    library: &'a ToolLibrary,
    project: &openchuck_app::project::Project,
    text: &str,
) -> Result<&'a LibraryTool> {
    let number = text
        .trim()
        .strip_prefix(['T', 't'])
        .and_then(|n| n.parse::<u32>().ok());
    if let Some(n) = number {
        let machine = project
            .machine()
            .map(|m| m.name.as_str())
            .unwrap_or_default();
        let set = library.tool_set(machine).with_context(|| {
            format!("T{n} needs a tool set for the project's machine; name the tool instead")
        })?;
        let id = set
            .tool_for(n)
            .with_context(|| format!("the {} tool set has no T{n}", set.machine))?;
        return library
            .get(id)
            .with_context(|| format!("the {} tool set's T{n} is not in the library", set.machine));
    }
    library.find(text).map_err(anyhow::Error::msg)
}

/// Runs a `library` subcommand on the tool library folder at `path` (TL-01,
/// TL-11).
fn run_library(path: &Path, action: LibraryAction, threads: Threads, create: bool) -> Result<()> {
    let mut library = open_library(Some(path))?.context("no library")?;
    match action {
        LibraryAction::List => {
            if library.tools().next().is_none() {
                println!("{}: no tools", path.display());
            }
            for t in library.tools() {
                let file = library
                    .path_of(t.id)
                    .and_then(Path::file_name)
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                println!("{} ({}, {file})", t.name, t.id);
                println!("    {}", describe(&t.tool));
                for material in t.cutting_data.keys() {
                    println!("    cutting data: {material}");
                }
            }
            for set in library.tool_sets() {
                println!();
                print_tool_set(&library, set);
            }
        }
        LibraryAction::Import { from } => {
            let report = library.import(&from)?;
            println!(
                "{}: {} added, {} replaced, {} unchanged",
                path.display(),
                report.added.len(),
                report.replaced.len(),
                report.unchanged.len()
            );
        }
        LibraryAction::Export { to, tools } => {
            let ids = tools
                .iter()
                .map(|text| library.find(text).map(|t| t.id))
                .collect::<std::result::Result<Vec<_>, String>>()
                .map_err(anyhow::Error::msg)?;
            let written = library.export(&ids, &to)?;
            println!("{}: {} tools written", to.display(), written.len());
        }
        LibraryAction::Profile { tool, file } => {
            let profile = load_tool_profile(&file)?;
            let mut entry = library.find(&tool).map_err(anyhow::Error::msg)?.clone();
            entry.tool = with_custom_profile(&entry.tool, profile);
            let written = library.save_tool(entry.clone())?;
            println!(
                "{}: \"{}\" now has the profile from {}",
                written.display(),
                entry.name,
                file.display()
            );
            println!("    {}", describe(&entry.tool));
        }
        LibraryAction::Store { project, tool } => {
            let app = open_project(&project, threads, create)?;
            let snapshot = find_tool(app.project(), &tool)
                .with_context(|| format!("the project has no single tool named \"{tool}\""))?;
            let written = library.save_tool(LibraryTool::from_snapshot(snapshot))?;
            println!("{}: stored \"{}\"", written.display(), snapshot.name);
        }
        LibraryAction::ToolSet {
            machine,
            set: assignments,
            unset,
        } => {
            let mut set = library.tool_set(&machine).cloned().unwrap_or(ToolSet {
                machine: machine.trim().to_owned(),
                entries: Vec::new(),
            });
            for n in unset {
                if !set.unassign(n) {
                    anyhow::bail!("the {} tool set has no T{n}", set.machine);
                }
            }
            for text in &assignments {
                let (number, tool) = text
                    .split_once('=')
                    .with_context(|| format!("'{text}': give --set NUMBER=TOOL"))?;
                let number: u32 = number
                    .trim()
                    .trim_start_matches(['T', 't'])
                    .parse()
                    .with_context(|| format!("'{number}' is not a tool number"))?;
                let id = library.find(tool).map_err(anyhow::Error::msg)?.id;
                set.assign(id, ControllerMapping::numbered(number));
            }
            let existing = library.tool_set(&machine);
            // Showing a set that does not exist yet writes nothing.
            if existing != Some(&set) && (existing.is_some() || !set.entries.is_empty()) {
                library.save_tool_set(set.clone())?;
            }
            print_tool_set(&library, &set);
        }
    }
    Ok(())
}

/// Prints a machine's tool set (TL-11).
fn print_tool_set(library: &ToolLibrary, set: &ToolSet) {
    println!("tool set:   {}", set.machine);
    if set.entries.is_empty() {
        println!("            empty");
    }
    for e in &set.entries {
        let name = library
            .get(e.tool)
            .map_or_else(|| format!("missing tool {}", e.tool), |t| t.name.clone());
        println!("  {:<8} {name}", describe_controller(&e.controller));
    }
}

/// Prints a project tool, its controller values, cutting data and checks
/// (TL-04 to TL-07).
fn print_tool(app: &Application, report: &ToolReport) {
    let Some(t) = app.project().tools().iter().find(|t| t.id == report.tool) else {
        return;
    };
    println!("tool:       {} ({})", t.name, t.id);
    println!("shape:      {}", describe(&t.tool));
    println!("controller: {}", describe_controller(&t.controller));
    if t.cutting_data.is_empty() {
        println!("cutting:    none");
    }
    for (material, d) in &t.cutting_data {
        println!(
            "cutting:    {material}: {} rpm, {} mm per tooth ({} mm/min), plunge {} mm/min, stepover {} mm, stepdown {} mm",
            d.spindle_speed.0,
            d.feed_per_tooth.0,
            d.cutting_feed(&t.tool).0,
            d.plunge_feed.0,
            d.stepover.0,
            d.stepdown.0
        );
    }
    match &report.library {
        None => {}
        Some(SnapshotStatus::Same) => println!("library:    same as the library"),
        Some(SnapshotStatus::NotInLibrary) => println!("library:    not in the library"),
        Some(SnapshotStatus::Differs(fields)) => println!(
            "library:    differs in {}; the project keeps its own copy (update with `tool --update`)",
            fields.join(", ")
        ),
    }
    for warning in &report.warnings {
        println!("warning:    {warning}");
    }
}

/// A machine profile by built-in key or name, by file stem in `library`, or
/// from a profile file.
fn find_profile(text: &str, library: Option<&Path>) -> Result<MachineProfile> {
    if let Some(profile) = builtin(text) {
        return Ok(profile?);
    }
    if let Some(dir) = library {
        let path = dir.join(format!("{text}.toml"));
        if path.is_file() {
            return Ok(load_profile(&path)?);
        }
    }
    let path = Path::new(text);
    if path.is_file() {
        return Ok(load_profile(path)?);
    }
    anyhow::bail!(
        "'{text}' is not a built-in machine ({}), a profile in the library or a profile file",
        builtin_keys().join(", ")
    )
}

/// Reads the `--stock` option.
fn parse_stock(text: &str) -> Result<Option<Stock>> {
    if text == "none" {
        return Ok(None);
    }
    let (kind, values) = text.split_once('=').unwrap_or((text, ""));
    let numbers = values
        .split(',')
        .map(|v| {
            v.trim()
                .parse::<f64>()
                .with_context(|| format!("'{v}' in '{text}' is not a number"))
        })
        .collect::<Result<Vec<f64>>>()?;
    let stock = match (kind, numbers.as_slice()) {
        ("cylinder", [d, l, x]) => Stock::Cylinder {
            diameter: Millimetres(*d),
            length: Millimetres(*l),
            x_start: Millimetres(*x),
        },
        ("box", [x0, y0, z0, x1, y1, z1]) => Stock::Box {
            bounds: Aabb {
                min: Point3::new(*x0, *y0, *z0),
                max: Point3::new(*x1, *y1, *z1),
            },
        },
        ("bbox", [offset]) => Stock::OffsetBoundingBox {
            offset: Millimetres(*offset),
        },
        _ => anyhow::bail!(
            "'{text}' is not a stock: use cylinder=DIAMETER,LENGTH,X_START, box=X0,Y0,Z0,X1,Y1,Z1, \
             bbox=OFFSET or none"
        ),
    };
    Ok(Some(stock))
}

/// Reads the `--clearance` option.
fn parse_clearance(text: &str) -> Result<Option<Clearance>> {
    if text == "machine" {
        return Ok(None);
    }
    let parts: Vec<&str> = text.split(',').collect();
    let [radius, safe_z] = parts.as_slice() else {
        anyhow::bail!("'{text}': give --clearance RADIUS,SAFE_Z in mm, or `machine`");
    };
    let number = |v: &str| {
        v.trim()
            .parse::<f64>()
            .with_context(|| format!("'{v}' in '{text}' is not a number"))
    };
    Ok(Some(Clearance {
        radius: Millimetres(number(radius)?),
        safe_z: Millimetres(number(safe_z)?),
    }))
}

/// Prints a machine profile with where each value came from (SET-01).
fn print_machine(profile: &MachineProfile) {
    let p = profile;
    println!("machine:    {}", p.name);
    println!(
        "rotary:     about machine {}; {}",
        p.rotary_axis.name(),
        a_direction(p)
    );
    let o = p.rotary_origin;
    println!(
        "reference:  rotary reference point at machine X {} Y {} Z {}",
        o.x, o.y, o.z
    );
    match p.a_range {
        ARange::Continuous => println!("A range:    continuous"),
        ARange::Limited { min, max } => println!("A range:    {} to {} deg", min.0, max.0),
    }
    let (lo, hi) = (p.travel.min, p.travel.max);
    println!(
        "travel:     X {} to {}, Y {} to {}, Z {} to {} mm",
        lo.x, hi.x, lo.y, hi.y, lo.z, hi.z
    );
    let f = p.max_feeds;
    println!(
        "feeds:      X and Y {} mm/min, Z {} mm/min, A {} deg/min; rapid X and Y {}, Z {} mm/min, A {} deg/min",
        f.linear.0, f.z.0, f.rotary.0, f.rapid_linear.0, f.rapid_z.0, f.rapid_rotary.0
    );
    println!(
        "clearance:  safe cylinder radius {} mm, safe Z {} mm above the axis (default)",
        p.clearance.radius.0, p.clearance.safe_z.0
    );
    for field in openchuck_app::setup::ProfileField::ALL {
        match p.provenance.get(&field) {
            Some(source) => {
                let state = match source.state {
                    Confirmation::Confirmed => "confirmed",
                    Confirmation::Unconfirmed => "UNCONFIRMED",
                };
                println!("  {field:<14} {state}: {}", source.source);
            }
            None => println!("  {field:<14} UNCONFIRMED: no source recorded"),
        }
    }
}

/// Prints a setup and its checks (SET-03, SET-05, SET-06).
fn print_setup(app: &Application, report: &SetupReport) {
    println!("setup:      {} ({})", report.name, report.setup);
    let Some(setup) = app.project().setup(report.setup) else {
        return;
    };
    let part = setup
        .part
        .and_then(|id| app.project().models().iter().find(|m| m.id == id))
        .map_or("none".to_owned(), |m| m.path.as_str().to_owned());
    println!("part:       {part}");
    let stock = match &setup.stock {
        None => "none".to_owned(),
        Some(Stock::Cylinder {
            diameter,
            length,
            x_start,
        }) => format!(
            "cylinder, diameter {} mm, length {} mm, from X {}",
            diameter.0, length.0, x_start.0
        ),
        Some(Stock::Box { bounds: b }) => format!(
            "box, X {} to {}, Y {} to {}, Z {} to {} mm",
            b.min.x, b.max.x, b.min.y, b.max.y, b.min.z, b.max.z
        ),
        Some(Stock::OffsetBoundingBox { offset }) => {
            format!("the part's bounding box plus {} mm", offset.0)
        }
        Some(_) => "imported (not available yet)".to_owned(),
    };
    println!("stock:      {stock}");
    println!(
        "material:   {}",
        setup.material.as_deref().unwrap_or("not set")
    );
    println!(
        "position:   part origin {} mm along the rotary axis from the machine's reference point",
        setup.frame.part_offset.0
    );
    let w = setup.frame.work_offset;
    println!(
        "work zero:  {} on the rotary axis at model X {} (Y0 Z0 is the axis)",
        w.register, w.x.0
    );
    if let Some(p) = report.expected_work_offset {
        println!(
            "            set {} to machine X {:.3} Y {:.3} Z {:.3} at A 0",
            w.register, p.x, p.y, p.z
        );
    }
    match (setup.clearance, app.project().machine()) {
        (Some(c), _) => println!(
            "clearance:  safe cylinder radius {} mm, safe Z {} mm above the axis",
            c.radius.0, c.safe_z.0
        ),
        (None, Some(m)) => println!(
            "clearance:  the machine's: safe cylinder radius {} mm, safe Z {} mm above the axis",
            m.clearance.radius.0, m.clearance.safe_z.0
        ),
        (None, None) => println!("clearance:  the machine's (no machine chosen)"),
    }
    if let Some(e) = report.part {
        println!(
            "part size:  radius {:.3} mm about the rotary axis, X {} to {}",
            e.radius.0, e.bounds.min.x, e.bounds.max.x
        );
    }
    if report.warnings.is_empty() && report.notes.is_empty() {
        println!("checks:     no problems found");
    }
    for warning in &report.warnings {
        println!("  - {warning}");
    }
    for note in &report.notes {
        println!("note:       {note}");
    }
}

/// Asks on the terminal which unit a model is in. Enter converts from a
/// unit the file names, and keeps millimetres for a size question; a unit
/// may also be typed; anything else asks again.
fn ask_unit(question: &UnitQuestion) -> LengthUnit {
    let named = match question.concern {
        UnitConcern::FileNames(unit) => Some(unit),
        _ => None,
    };
    loop {
        match named {
            Some(unit) => eprint!("{}\nConvert from {unit} to mm? [Y/n] ", question.message()),
            None => {
                let choices: Vec<&str> = question
                    .suggestions
                    .iter()
                    .map(|u| u.short_name())
                    .collect();
                eprint!(
                    "{}\nUnit [{}/mm, Enter keeps mm]: ",
                    question.message(),
                    choices.join("/")
                );
            }
        }
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return LengthUnit::Millimetre;
        }
        if let Some(unit) = read_answer(named, &answer) {
            return unit;
        }
    }
}

/// Reads an answer to [`ask_unit`]; `None` asks again.
fn read_answer(named: Option<LengthUnit>, answer: &str) -> Option<LengthUnit> {
    let answer = answer.trim().to_ascii_lowercase();
    match (named, answer.as_str()) {
        (Some(unit), "" | "y" | "yes") => Some(unit),
        (Some(_), "n" | "no") | (None, "") => Some(LengthUnit::Millimetre),
        // A unit typed in full answers either question.
        (_, text) => LengthUnit::parse(text),
    }
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
    if let Some(question) = &inspection.unit_question {
        println!("unit?      {}", question.message());
    }
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

fn print_tool_numbers(app: &Application) {
    let plan = tool_number_plan(app.project());
    if plan.is_empty() {
        println!("no operation uses a tool, so the program has no tool numbers");
    }
    for row in plan {
        println!("T{:<3} {}", row.number, row.name);
    }
}

/// GC-14: show the numbers the program will use and settle them before the
/// G-code is written. A terminal is asked; otherwise an option is required.
fn confirm_tool_numbers(app: &mut Application, choice: Option<NumberChoice>) -> Result<()> {
    eprintln!("The program will use these tool numbers, in order of first use:");
    for row in tool_number_plan(app.project()) {
        eprintln!("  T{:<3} {}", row.number, row.name);
    }
    let choice = match choice {
        Some(choice) => choice,
        None if std::io::stdin().is_terminal() => {
            eprint!("Keep these numbers (k), or renumber in order of use (a)? [k/a] ");
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if answer.trim().eq_ignore_ascii_case("a") {
                NumberChoice::Auto
            } else {
                NumberChoice::Keep
            }
        }
        None => anyhow::bail!(
            "confirm the tool numbers shown above with --tool-numbers keep, or renumber them with --tool-numbers auto"
        ),
    };
    let numbering = match choice {
        NumberChoice::Keep => ToolNumbering::Keep,
        NumberChoice::Auto => ToolNumbering::InOrderOfUse { first: 1 },
    };
    // `Keep` still checks for duplicates, so a clash is refused here.
    if let Some(command) = tool_numbering_command(app.project(), &numbering, None)? {
        app.execute(command)?;
        app.save()?;
        eprintln!("Tool numbers changed and saved in the project:");
        for row in tool_number_plan(app.project()) {
            eprintln!("  T{:<3} {}", row.number, row.name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_imp_03_terminal_answers_to_the_unit_question() {
        let inch = Some(LengthUnit::Inch);
        assert_eq!(read_answer(inch, "\n"), inch);
        assert_eq!(read_answer(inch, "Y\n"), inch);
        assert_eq!(read_answer(inch, "no\n"), Some(LengthUnit::Millimetre));
        assert_eq!(read_answer(inch, "cm\n"), Some(LengthUnit::Centimetre));
        assert_eq!(read_answer(inch, "maybe\n"), None);
        assert_eq!(read_answer(None, "\n"), Some(LengthUnit::Millimetre));
        assert_eq!(read_answer(None, " M \n"), Some(LengthUnit::Metre));
        assert_eq!(read_answer(None, "y\n"), None);
    }
}
