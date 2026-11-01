// SPDX-License-Identifier: AGPL-3.0-or-later
//! Posting the project's toolpath to G-code files (GC-02, GC-05, GC-14;
//! decision record 0020 item 4).
//!
//! The toolpath is split into runs of consecutive groups from the same setup,
//! because the work frame is the setup's; each run is one program file. Tool
//! numbers are the project's (the tool snapshots' controller numbers) and
//! spindle speeds are the operation's own or, without one, come from the
//! cutting data for the setup's stock material (decision 0196).

use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_gcode::{CheckConfig, Dialect, TimingRule, Tolerances};
use openchuck_post::{
    ADirection, FeedMode, FeedRule, PostDefinition, PostJob, PostProcessor, ShortBlockReport,
    ShortBlockSettings, TemplatePost, ToolSetting, UnwindMode, builtin, builtin_ids,
};
use openchuck_project::Project;
use openchuck_types::ir::{Toolpath, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::kinematics::RotaryFrame;

use crate::error::AppError;

/// Which post turns the toolpath into G-code (OI-88).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostSelection {
    /// A post that ships with the application, by id (`makera-z1`, `linuxcnc`).
    Builtin(String),
    /// A post folder on disk, loaded at run time (GC-03).
    Folder(PathBuf),
}

impl PostSelection {
    /// Reads `value` as a built-in id if it is one, else as a folder path.
    pub fn parse(value: &str) -> Self {
        if builtin_ids().any(|id| id == value) {
            Self::Builtin(value.to_string())
        } else {
            Self::Folder(PathBuf::from(value))
        }
    }

    pub(crate) fn load(&self) -> Result<PostDefinition, AppError> {
        match self {
            Self::Builtin(id) => builtin(id).ok_or_else(|| {
                AppError::Post(format!(
                    "no built-in post '{id}' (available: {})",
                    builtin_ids().collect::<Vec<_>>().join(", ")
                ))
            })?,
            Self::Folder(path) => PostDefinition::load_dir(path),
        }
        .map_err(|e| AppError::Post(e.to_string()))
    }
}

/// How the toolpath is split into G-code files (GC-05, decision record 0028
/// item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputGrouping {
    /// One file for the whole project. A program has one work frame and one
    /// work offset, so a project with more than one setup is refused (OI-92).
    Project,
    /// One file for each run of operations in one setup. The default.
    #[default]
    Setup,
    /// One file for each operation.
    Operation,
    /// One file for each run of consecutive operations that use one tool, so
    /// the machining order is kept and each toolpath still meets the stock
    /// it was made on.
    Tool,
}

impl OutputGrouping {
    /// Every grouping, in the order a front end lists them.
    pub const ALL: [Self; 4] = [Self::Project, Self::Setup, Self::Operation, Self::Tool];

    /// The grouping's name as the command line and project files spell it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Setup => "setup",
            Self::Operation => "operation",
            Self::Tool => "tool",
        }
    }
}

impl std::str::FromStr for OutputGrouping {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|g| g.as_str() == value)
            .ok_or_else(|| {
                format!("unknown output grouping '{value}' (use project, setup, operation or tool)")
            })
    }
}

/// A name made safe for a file name: lower-case ASCII letters and digits
/// joined by hyphens, at most 40 characters, `fallback` when nothing is left.
pub(crate) fn slug(name: &str, fallback: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let joined = mapped
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let joined: String = joined.chars().take(40).collect();
    let joined = joined.trim_end_matches('-');
    if joined.is_empty() {
        fallback.to_string()
    } else {
        joined.to_string()
    }
}

/// A file name stem: the running number, so the files sort in machining
/// order and no two share a name, then the parts that say what is inside.
/// The tool and its version for the comment every program carries (NFR-16): the workspace
/// version only, never a commit or a date, so one version always writes the same program
/// (GC-07, NFR-06).
#[must_use]
fn generator_comment() -> String {
    format!("OpenChuckCAM {}", env!("CARGO_PKG_VERSION"))
}

fn file_stem(index: usize, parts: &[String]) -> String {
    let mut stem = format!("{:02}", index + 1);
    for part in parts {
        stem.push('-');
        stem.push_str(part);
    }
    stem
}

/// What a file's name says after its running number: the project name
/// (`Project`) or the setup name, then what the unit holds beyond its setup.
fn contents(
    project: &Project,
    setup: &str,
    label: Option<&(String, String)>,
    grouping: OutputGrouping,
) -> String {
    let mut parts = Vec::new();
    if grouping == OutputGrouping::Project {
        parts.push(slug(project.name(), "project"));
    } else {
        parts.push(slug(setup, "setup"));
    }
    if let Some((_, part)) = label {
        parts.push(part.clone());
    }
    parts.join("-")
}

/// The name of the file at `index`: the running number, then
/// [`contents`].
fn file_name(
    project: &Project,
    setup: &str,
    label: Option<&(String, String)>,
    grouping: OutputGrouping,
    index: usize,
    extension: &str,
) -> String {
    let stem = file_stem(index, &[contents(project, setup, label, grouping)]);
    format!("{stem}.{extension}")
}

/// What each posted file of `run` holds, in write order, with the part of
/// its posted name after the number and the post's extension (issues 308
/// and 309). A run made without a post has no files.
pub(crate) fn program_files(
    run: &crate::Regeneration,
) -> Result<Vec<(crate::output_names::ProgramFile, String)>, AppError> {
    let Some((_, grouping)) = &run.posted_selection else {
        return Ok(Vec::new());
    };
    let project = &run.project;
    let enabled: Vec<String> = project
        .operations()
        .iter()
        .filter(|o| o.enabled)
        .map(|o| o.id.to_string())
        .collect();
    let units = split_units(project, &run.canonical_toolpath, *grouping)?;
    if units.len() != run.gcode.len() {
        return Err(AppError::Post(format!(
            "the run has {} program files but its toolpath splits into {}; generate again",
            run.gcode.len(),
            units.len()
        )));
    }
    units
        .into_iter()
        .zip(&run.gcode)
        .map(|(unit, (posted_name, _))| {
            let (setup_number, setup) = project
                .setups()
                .iter()
                .enumerate()
                .find(|(_, s)| s.id.to_string() == unit.setup)
                .ok_or_else(|| {
                    AppError::Post(format!(
                        "setup '{}' of the toolpath is not in the project",
                        unit.setup
                    ))
                })?;
            let mut steps: Vec<crate::output_names::ProgramStep> = Vec::new();
            let mut current_tool = None;
            for group in &unit.groups {
                let operation = project
                    .operations()
                    .iter()
                    .find(|o| o.id.to_string() == group.tags.operation);
                let number = enabled
                    .iter()
                    .position(|id| *id == group.tags.operation)
                    .map_or(0, |i| i + 1);
                if steps
                    .last()
                    .is_some_and(|s: &crate::output_names::ProgramStep| {
                        s.operation_number == number
                    })
                {
                    continue;
                }
                let tool = project
                    .tools()
                    .iter()
                    .find(|t| t.id.to_string() == group.tags.tool);
                let tool_number = tool.map(|t| t.controller.number);
                let tool_change = tool_number.is_some() && tool_number != current_tool;
                if tool_number.is_some() {
                    current_tool = tool_number;
                }
                steps.push(crate::output_names::ProgramStep {
                    operation_number: number,
                    operation: operation
                        .map_or_else(|| group.tags.operation.clone(), |o| o.name.clone()),
                    tool_number,
                    tool: tool.map(|t| t.name.clone()),
                    tool_change,
                });
            }
            Ok((
                crate::output_names::ProgramFile {
                    posted_name: posted_name.clone(),
                    name: posted_name.clone(),
                    setup_number: setup_number + 1,
                    setup: setup.name.clone(),
                    steps,
                },
                contents(project, &setup.name, unit.label.as_ref(), *grouping),
            ))
        })
        .collect()
}

/// One G-code file the grouping would write, known before anything is posted
/// (GC-05; specification section 11.6, the files to write).
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedFile {
    /// The file name.
    pub name: String,
    /// The setup it belongs to.
    pub setup: String,
    /// The operation or tool name for `Operation` and `Tool` files.
    pub label: Option<String>,
    /// The programmed time: the sum of the toolpath's move durations at their
    /// programmed feeds, including selected-post slowdown, in seconds
    /// (decisions 0028 and 0222; not SIM-08).
    pub programmed_seconds: f64,
}

/// The files `grouping` would write for `toolpath`, in output order, with the
/// names [`post_toolpath`] gives them. Timing uses the selected post, computed
/// without writing anything if the run was posted with other choices. A project
/// file across setups is refused as it is when posting (OI-92).
pub(crate) fn plan_files(
    run: &crate::Regeneration,
    selection: &PostSelection,
    grouping: OutputGrouping,
) -> Result<Vec<PlannedFile>, AppError> {
    let project = &run.project;
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    let extension = selection.load()?.options.file_extension;
    // A preview of a different post/grouping must compute that program's
    // timing rather than reusing another post's slowdown cost.
    let seconds = if run
        .posted_selection
        .as_ref()
        .is_some_and(|(post, split)| post == selection && *split == grouping)
    {
        run.posted_file_seconds.clone()
    } else {
        post_toolpath(run, selection, grouping, &context)?.file_seconds
    };
    split_units(project, &run.canonical_toolpath, grouping)?
        .into_iter()
        .enumerate()
        .map(|(index, unit)| {
            let setup = project
                .setups()
                .iter()
                .find(|s| s.id.to_string() == unit.setup)
                .ok_or_else(|| {
                    AppError::Post(format!(
                        "setup '{}' of the toolpath is not in the project",
                        unit.setup
                    ))
                })?;
            let assembled =
                crate::connections::assemble(project, unit.groups, &run.groups, &context)?;
            let name = file_name(
                project,
                &setup.name,
                unit.label.as_ref(),
                grouping,
                index,
                &extension,
            );
            Ok(PlannedFile {
                name: name.clone(),
                setup: setup.name.clone(),
                label: unit.label.as_ref().map(|(title, _)| title.clone()),
                programmed_seconds: seconds
                    .get(&name)
                    .copied()
                    .unwrap_or_else(|| assembled.total_duration().0),
            })
        })
        .collect()
}

/// What the checks found about one G-code file (GC-08, GC-11; gates 1 and 2).
#[derive(Debug, Clone, PartialEq)]
pub struct GcodeReport {
    /// The file name.
    pub file: String,
    /// Moves the file reaches.
    pub moves: usize,
    /// Gate 1 findings, as `line N: message`.
    pub findings: Vec<String>,
    /// Gate 2: the replay against the IR, or why it failed.
    pub comparison: Result<GcodeComparison, String>,
}

/// The numbers of a passing gate 2 comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct GcodeComparison {
    /// Poses compared.
    pub poses: usize,
    /// Largest tip error, mm.
    pub max_position_error: f64,
    /// Largest A error, degrees.
    pub max_a_error: f64,
    /// Cut time of the verification IR after any post slowdown, seconds.
    pub ir_cut_seconds: f64,
    /// Cut time of the G-code under the controller's feed rule, seconds.
    pub gcode_cut_seconds: f64,
}

impl GcodeReport {
    /// True when gate 1 found nothing and gate 2 passed.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty() && self.comparison.is_ok()
    }
}

// The setup's effective safe Z travels beside the setup (decision record 0530).
#[allow(clippy::too_many_arguments)]
fn check_file(
    text: &str,
    file: &str,
    definition: &PostDefinition,
    machine: &openchuck_types::machine::MachineProfile,
    setup: &openchuck_project::Setup,
    safe_z: f64,
    part: &Toolpath,
    allow_unadjusted_clamp_time: bool,
) -> GcodeReport {
    let options = &definition.options;
    let dialect = if options.unwind == UnwindMode::DropTurns {
        Dialect::MakeraZ1
    } else {
        Dialect::Linuxcnc
    };
    let rule = match definition.feed_rule {
        FeedRule::Makera => TimingRule::Makera,
        FeedRule::InverseTime => TimingRule::InverseTime,
        FeedRule::Linuxcnc | FeedRule::Contact => TimingRule::Linuxcnc,
    };
    let fail = |message: String| GcodeReport {
        file: file.to_string(),
        moves: 0,
        findings: vec![message.clone()],
        comparison: Err(message),
    };
    let program = match openchuck_gcode::read(text) {
        Ok(p) => p,
        Err(errors) => {
            return fail(
                errors
                    .first()
                    .map_or_else(|| "unreadable".into(), ToString::to_string),
            );
        }
    };
    let a_letter = options.a_letter.chars().next().unwrap_or('A');
    let mut replay = match openchuck_gcode::replay_axis(&program, dialect, a_letter) {
        Ok(r) => r,
        Err(e) => return fail(e.to_string()),
    };
    // Limits in program coordinates: the machine travel less the work zero.
    let zero = openchuck_types::setup::expected_work_offset(machine, &setup.frame);
    let travel = [
        (machine.travel.min.x - zero.x, machine.travel.max.x - zero.x),
        (machine.travel.min.y - zero.y, machine.travel.max.y - zero.y),
        (machine.travel.min.z - zero.z, machine.travel.max.z - zero.z),
    ];
    let a_range = match machine.a_range {
        openchuck_types::machine::ARange::Continuous => None,
        openchuck_types::machine::ARange::Limited { min, max } => Some((min.0, max.0)),
    };
    let config = CheckConfig {
        travel,
        a_range,
        max_feed: openchuck_gcode::FeedLimits {
            linear: machine.max_feeds.linear.0,
            z: machine.max_feeds.z.0,
            rotary: machine.max_feeds.rotary.0,
        },
        safe_z,
        spindle_off_at_tool_change: options.tool_change.stop_spindle_first,
        require_work_offset: true,
        allow_inverse_time: options.feed_mode == FeedMode::G93,
    };
    let findings = openchuck_gcode::static_check(&replay, &config)
        .into_iter()
        .map(|f| {
            if f.line == 0 {
                f.message
            } else {
                format!("line {}: {}", f.line, f.message)
            }
        })
        .collect();
    // A reversed post writes the opposite sign; the comparison works in the profile's sense, so the
    // replay is turned back after the checks that look at the A values as written.
    if options.a_direction == openchuck_post::ADirection::Reversed {
        for step in &mut replay.steps {
            match step {
                openchuck_gcode::Step::Move(m) => {
                    m.from[3] = -m.from[3];
                    m.to[3] = -m.to[3];
                }
                openchuck_gcode::Step::Event(openchuck_gcode::Event::DropTurns {
                    from,
                    to,
                    ..
                }) => {
                    *from = -*from;
                    *to = -*to;
                }
                openchuck_gcode::Step::Event(_) => {}
            }
        }
    }
    let mut tolerances = Tolerances::for_decimals(options.decimals.pos, options.decimals.a);
    if allow_unadjusted_clamp_time {
        // Off/Report retain the canonical IR durations when a post maximum
        // lowers feed. Slow down supplies adjusted durations and keeps the
        // normal 1% timing check, including moves clamped to that maximum.
        tolerances.time_relative = f64::INFINITY;
    }
    let frame = RotaryFrame::work(machine, &setup.frame);
    let comparison = openchuck_gcode::compare(&replay, part, &frame, rule, tolerances)
        .map(|c| GcodeComparison {
            poses: c.poses,
            max_position_error: c.max_position_error,
            max_a_error: c.max_a_error,
            ir_cut_seconds: c.ir_cut_seconds,
            gcode_cut_seconds: c.gcode_cut_seconds,
        })
        .map_err(|e| e.to_string());
    GcodeReport {
        file: file.to_string(),
        moves: replay.moves().count(),
        findings,
        comparison,
    }
}

/// One program file's worth of groups.
struct Unit {
    setup: String,
    /// What the file holds beyond its setup, for its title and name.
    label: Option<(String, String)>,
    groups: Vec<ToolpathGroup>,
}

/// Splits the toolpath into the files `grouping` asks for.
fn split_units(
    project: &Project,
    toolpath: &Toolpath,
    grouping: OutputGrouping,
) -> Result<Vec<Unit>, AppError> {
    // Runs of consecutive groups from one setup: the work frame is the setup's.
    let mut runs: Vec<(String, Vec<ToolpathGroup>)> = Vec::new();
    for group in &toolpath.groups {
        match runs.last_mut() {
            Some((setup, groups)) if *setup == group.tags.setup => groups.push(group.clone()),
            _ => runs.push((group.tags.setup.clone(), vec![group.clone()])),
        }
    }
    if grouping == OutputGrouping::Project && runs.len() > 1 {
        return Err(AppError::Post(format!(
            "one file for the project cannot hold {} setup runs: each setup has its own work frame and work offset, and the operator refixtures between them; choose one file per setup, operation or tool",
            runs.len()
        )));
    }
    let tool_name = |tag: &str| {
        project
            .tools()
            .iter()
            .find(|t| t.id.to_string() == tag)
            .map(|t| (t.controller.number, t.name.clone()))
    };
    let mut units = Vec::new();
    for (setup, groups) in runs {
        match grouping {
            OutputGrouping::Project | OutputGrouping::Setup => units.push(Unit {
                setup,
                label: None,
                groups,
            }),
            OutputGrouping::Operation => {
                for group in groups {
                    let name = project
                        .operations()
                        .iter()
                        .find(|o| o.id.to_string() == group.tags.operation)
                        .map_or_else(|| group.tags.operation.clone(), |o| o.name.clone());
                    units.push(Unit {
                        setup: setup.clone(),
                        label: Some((name.clone(), slug(&name, "operation"))),
                        groups: vec![group],
                    });
                }
            }
            OutputGrouping::Tool => {
                let mut by_tool: Vec<(String, Vec<ToolpathGroup>)> = Vec::new();
                for group in groups {
                    // Only consecutive groups: each toolpath was made on the stock
                    // the operations before it leave (SIM-06), so the machining
                    // order must not change.
                    match by_tool.last_mut() {
                        Some((t, list)) if *t == group.tags.tool => list.push(group),
                        _ => by_tool.push((group.tags.tool.clone(), vec![group])),
                    }
                }
                for (tag, groups) in by_tool {
                    let (title, part) = match tool_name(&tag) {
                        Some((number, name)) => (
                            format!("T{number} {name}"),
                            format!("t{number}-{}", slug(&name, "tool")),
                        ),
                        None => (tag.clone(), slug(&tag, "tool")),
                    };
                    units.push(Unit {
                        setup: setup.clone(),
                        label: Some((title, part)),
                        groups,
                    });
                }
            }
        }
    }
    Ok(units)
}

/// The posted files and their matching verification/reporting data.
pub(crate) struct PostedFiles {
    pub files: Vec<(String, String)>,
    pub reports: Vec<GcodeReport>,
    pub notes: Vec<String>,
    pub short_blocks: Vec<ShortBlockReport>,
    pub toolpath: Toolpath,
    pub file_seconds: BTreeMap<String, f64>,
    pub settings: Option<(ShortBlockSettings, openchuck_post::ShortBlockSources)>,
}

/// One short-block result in the operator's words, shared by export checks
/// and the run's Messages notes (GC-17).
pub(crate) fn short_block_note(project: &Project, report: &ShortBlockReport) -> String {
    let operation = project
        .operations()
        .iter()
        .find(|o| o.id.to_string() == report.operation)
        .map_or(report.operation.as_str(), |o| o.name.as_str());
    let action = match report.settings.mode {
        openchuck_types::machine::ShortBlockMode::SlowDown => format!(
            "Slow down lengthened {} moves and added {:.3} s",
            report.slowed_moves, report.added_seconds
        ),
        _ => format!(
            "Report leaves the program unchanged; slowdown would add {:.3} s",
            report.added_seconds
        ),
    };
    format!(
        "Operation '{operation}': short blocks: {} windows below {:.3} ms averaged over {} {} (limit from {}, window from {}, mode from {}); {} moves below the limit, longest run {}, worst window before slowdown {:.3} ms, original effective feed {:.3} mm/min. {action}.",
        report.affected_windows,
        report.settings.min_block_ms,
        report.settings.block_window,
        if report.settings.block_window == 1 {
            "block"
        } else {
            "blocks"
        },
        report.sources.min_block_ms.as_str(),
        report.sources.block_window.as_str(),
        report.sources.mode.as_str(),
        report.short_moves,
        report.longest_run,
        report.worst_window_ms,
        report.effective_feed
    )
}

/// Posts `toolpath` and returns the files and matching timing/check data.
pub(crate) fn post_toolpath(
    run: &crate::Regeneration,
    selection: &PostSelection,
    grouping: OutputGrouping,
    context: &JobContext<'_>,
) -> Result<PostedFiles, AppError> {
    let project = &run.project;
    let toolpath = &run.canonical_toolpath;
    let post = TemplatePost::new(selection.load()?);
    if toolpath.groups.is_empty() {
        return Ok(PostedFiles {
            files: Vec::new(),
            reports: Vec::new(),
            notes: Vec::new(),
            short_blocks: Vec::new(),
            toolpath: Toolpath::default(),
            file_seconds: BTreeMap::new(),
            settings: None,
        });
    }
    for group in &toolpath.groups {
        context.check_cancelled()?;
        if let Some(operation) = project
            .operations()
            .iter()
            .find(|o| o.id.to_string() == group.tags.operation)
            && crate::facing::needs_verification(operation)
            && !run
                .groups
                .iter()
                .any(|data| data.operation == operation.id && data.facing.is_some())
        {
            return Err(AppError::Post(format!(
                "operation '{}': whole-setup Facing verification inputs are missing; regenerate before posting",
                operation.name
            )));
        }
    }
    let decimals = &post.definition().options.decimals;
    if decimals.pos < 3 || decimals.a < 3 {
        for group in &toolpath.groups {
            if let Some(operation) = project
                .operations()
                .iter()
                .find(|o| o.id.to_string() == group.tags.operation)
            {
                let reversing = matches!(
                    operation.parameters.get("ramp_back_and_forth"),
                    Some(openchuck_project::ParameterValue::Flag(true))
                ) && matches!(operation.parameters.get("entry"),Some(openchuck_project::ParameterValue::Text(style)) if style=="ramp");
                if reversing {
                    return Err(AppError::Post(format!(
                        "operation '{}': back-and-forth ramp clearance requires at least three decimal places for XYZ and A; increase the post precision or choose another entry",
                        operation.name
                    )));
                }
            }
        }
    }
    let extension = post.definition().options.file_extension.clone();
    let machine = project
        .machine()
        .ok_or_else(|| AppError::Post("the project has no machine profile".into()))?;
    let settings = post
        .definition()
        .options
        .effective_short_blocks(ShortBlockSettings {
            min_block_ms: machine.min_block_ms,
            block_window: machine.block_window,
            mode: machine.short_block_mode,
        });
    let operation_names: BTreeMap<String, String> = project
        .operations()
        .iter()
        .map(|o| (o.id.to_string(), o.name.clone()))
        .collect();
    let mut files = Vec::new();
    let mut reports = Vec::new();
    let mut notes = Vec::new();
    let mut short_blocks = Vec::new();
    let mut posted_toolpath = Toolpath::default();
    let mut file_seconds = BTreeMap::new();
    for (index, unit) in split_units(project, toolpath, grouping)?
        .into_iter()
        .enumerate()
    {
        let Unit {
            setup: setup_tag,
            label,
            groups,
        } = unit;
        let setup = project
            .setups()
            .iter()
            .find(|s| s.id.to_string() == setup_tag)
            .ok_or_else(|| {
                AppError::Post(format!(
                    "setup '{setup_tag}' of the toolpath is not in the project"
                ))
            })?;
        // The setup's effective safe Z (decision record 0530): the same one
        // its toolpaths and links were made with, from the part bounds the
        // run measured.
        let part_bounds = run
            .groups
            .iter()
            .find(|g| g.setup == setup.id)
            .map(|g| g.part_bounds);
        let safe_z = crate::setup::effective_clearance(machine, setup, part_bounds.as_ref())
            .clearance
            .safe_z
            .0;
        let mut tools = BTreeMap::new();
        let mut operation_spindle = BTreeMap::new();
        for group in &groups {
            let tag = &group.tags.tool;
            let operation = project
                .operations()
                .iter()
                .find(|o| o.id.to_string() == group.tags.operation)
                .ok_or_else(|| {
                    AppError::Post(format!(
                        "operation '{}' of the toolpath is not in the project",
                        group.tags.operation
                    ))
                })?;
            let (speed, _) =
                crate::requirements::spindle_speed(project, operation).ok_or_else(|| {
                    AppError::Post(format!(
                        "cannot post the G-code: {}",
                        crate::requirements::missing_spindle_message(project, operation)
                    ))
                })?;
            if let Some(why) = crate::requirements::spindle_out_of_range(project, operation) {
                return Err(AppError::Post(format!("cannot post the G-code: {why}")));
            }
            operation_spindle.insert(group.tags.operation.clone(), speed.0);
            if tools.contains_key(tag) {
                continue;
            }
            let snapshot = project
                .tools()
                .iter()
                .find(|t| t.id.to_string() == *tag)
                .ok_or_else(|| {
                    AppError::Post(format!(
                        "tool '{tag}' of the toolpath is not in the project"
                    ))
                })?;
            let number = snapshot.controller.number;
            if number < 1 || number > machine.max_tool_number {
                return Err(AppError::Post(format!(
                    "tool '{}' has the number T{number}, outside the machine's range 1 to {} (renumber it with tool-numbers)",
                    snapshot.name, machine.max_tool_number
                )));
            }
            tools.insert(
                tag.clone(),
                ToolSetting {
                    number: snapshot.controller.number,
                    name: snapshot.name.clone(),
                    spindle_rpm: speed.0,
                },
            );
        }
        let job = PostJob {
            block_time: openchuck_post::BlockTimeSettings {
                min_block_ms: machine.min_block_ms,
                // Validated machine profiles cap this value at 4096.
                block_window: machine.block_window as u32,
                mode: machine.short_block_mode,
            },
            a_range: machine.a_range,
            safe_z,
            frame: RotaryFrame::work(machine, &setup.frame),
            tools,
            operation_names: operation_names.clone(),
            operation_spindle,
            title: match &label {
                Some((title, _)) => format!("{} - {} - {title}", project.name(), setup.name),
                None => format!("{} - {}", project.name(), setup.name),
            },
            revision: None,
            generator: Some(generator_comment()),
            work_offset: Some(format!("{:?}", setup.frame.work_offset.register)),
        };
        let part = crate::connections::assemble(project, groups, &run.groups, context)?;
        // The certificate's 0.002 mm linear allowance covers decimal
        // rounding plus <=0.0001 mm float ULP on each work-coordinate word.
        // A large work offset can otherwise lose clearance in a float
        // controller even though the physical machine travel is small.
        if post.definition().options.unwind == UnwindMode::DropTurns
            || post.definition().feed_rule == FeedRule::Makera
        {
            let options = &post.definition().options;
            let sign = match options.a_direction {
                ADirection::Same => 1.0,
                ADirection::Reversed => -1.0,
            };
            let mut a_shift = 0.0;
            for group in &part.groups {
                let end = group.segments.last().map_or(group.start, |s| s.end);
                let written_end_a = openchuck_post::round_written(end.a.0, options.decimals.a);
                let end_z = openchuck_types::kinematics::machine_xyz(&job.frame, end.tip, end.a).z;
                let reversing=project.operations().iter().find(|o|o.id.to_string()==group.tags.operation).is_some_and(|o|
                    matches!(o.parameters.get("ramp_back_and_forth"),Some(openchuck_project::ParameterValue::Flag(true)))
                    && matches!(o.parameters.get("entry"),Some(openchuck_project::ParameterValue::Text(style)) if style=="ramp"));
                if reversing {
                    let canonical = toolpath.groups.iter().find(|g| g.tags == group.tags)
                        .ok_or_else(|| AppError::Post("back-and-forth ramp has no canonical numeric precision certificate; regenerate the toolpath".into()))?;
                    let canonical_end =
                        canonical.segments.last().map_or(canonical.start, |s| s.end);
                    let counter_offset = (end.a.0 - canonical_end.a.0) * sign + a_shift;
                    if !counter_offset.is_finite() || counter_offset.abs() > 720.0 + 1e-7 {
                        return Err(AppError::Post("back-and-forth ramp cannot certify controller numeric precision at this rotary counter; use a supported counter-reset post or choose another entry".into()));
                    }
                }
                for pose in std::iter::once(group.start)
                    .chain(group.segments.iter().map(|s| s.end))
                    .filter(|_| reversing)
                {
                    context.check_cancelled()?;
                    let xyz =
                        openchuck_types::kinematics::machine_xyz(&job.frame, pose.tip, pose.a);
                    let machine_xyz = openchuck_types::kinematics::machine_xyz(
                        &RotaryFrame::of_profile(machine),
                        pose.tip,
                        pose.a,
                    );
                    for value in [
                        xyz.x,
                        xyz.y,
                        xyz.z,
                        machine_xyz.x,
                        machine_xyz.y,
                        machine_xyz.z,
                    ] {
                        #[allow(clippy::cast_possible_truncation)]
                        let word = (value.abs() + 1.0) as f32;
                        if !word.is_finite()
                            || f64::from(f32::from_bits(word.to_bits() + 1)) - f64::from(word)
                                > 0.0001
                        {
                            return Err(AppError::Post("back-and-forth ramp cannot certify controller numeric precision at this work offset; choose another entry".into()));
                        }
                    }
                }
                // Mirror the nonmoving boundary emitted by conversion and
                // the firmware's strict >360 reset rule, including preceding
                // operations that do not use reversing entry.
                if machine.a_range == openchuck_types::machine::ARange::Continuous
                    && options.unwind == UnwindMode::DropTurns
                    && openchuck_post::round_written(end_z, options.decimals.pos) >= job.safe_z
                {
                    let held = written_end_a * sign + a_shift;
                    if held.abs() > 360.0 {
                        a_shift = held % 360.0 - written_end_a * sign;
                    }
                }
            }
        }
        let output = post
            .post(&part, &job)
            .map_err(|e| AppError::Post(format!("setup '{}': {e}", setup.name)))?;
        crate::facing::written::verify_output(
            &output.text,
            post.definition(),
            &job,
            &part,
            &run.groups,
            context,
        )?;
        if output.clamped_feeds > 0 {
            notes.push(format!(
                "setup '{}': {} feed(s) were lowered to the post's maximum feed",
                setup.name, output.clamped_feeds
            ));
        }
        let name = file_name(
            project,
            &setup.name,
            label.as_ref(),
            grouping,
            index,
            &extension,
        );
        reports.push(check_file(
            &output.text,
            &name,
            post.definition(),
            machine,
            setup,
            safe_z,
            &output.verification_path,
            output.clamped_feeds > 0
                && !(settings.0.min_block_ms > 0.0
                    && settings.0.mode == openchuck_types::machine::ShortBlockMode::SlowDown),
        ));
        file_seconds.insert(name.clone(), output.estimated_seconds);
        for report in &output.short_blocks {
            if report.affected_windows > 0 {
                notes.push(short_block_note(project, report));
            }
        }
        short_blocks.extend(output.short_blocks);
        posted_toolpath
            .groups
            .extend(output.verification_path.groups);
        files.push((name, output.text));
    }
    Ok(PostedFiles {
        files,
        reports,
        notes,
        short_blocks,
        toolpath: posted_toolpath,
        file_seconds,
        settings: Some(settings),
    })
}
