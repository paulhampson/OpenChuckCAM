// SPDX-License-Identifier: AGPL-3.0-or-later
//! Posting the project's toolpath to G-code files (GC-02, GC-05, GC-14;
//! decision record 0020 item 4).
//!
//! The toolpath is split into runs of consecutive groups from the same setup,
//! because the work frame is the setup's; each run is one program file. Tool
//! numbers are the project's (the tool snapshots' controller numbers) and
//! spindle speeds come from the cutting data for the setup's stock material.

use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_gcode::{CheckConfig, Dialect, TimingRule, Tolerances};
use openchuck_post::{
    FeedMode, FeedRule, PostDefinition, PostJob, PostProcessor, TemplatePost, ToolSetting,
    UnwindMode, builtin, builtin_ids,
};
use openchuck_project::Project;
use openchuck_types::ir::{Toolpath, ToolpathGroup};
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
fn file_stem(index: usize, parts: &[String]) -> String {
    let mut stem = format!("{:02}", index + 1);
    for part in parts {
        stem.push('-');
        stem.push_str(part);
    }
    stem
}

/// The name of the file at `index`: the running number, the project name
/// (`Project`) or the setup name, then what the unit holds beyond its setup.
fn file_name(
    project: &Project,
    setup: &str,
    label: Option<&(String, String)>,
    grouping: OutputGrouping,
    index: usize,
    extension: &str,
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
    format!("{}.{extension}", file_stem(index, &parts))
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
    /// programmed feeds, in seconds (decision 0028 item 3; not SIM-08).
    pub programmed_seconds: f64,
}

/// The files `grouping` would write for `toolpath`, in output order, with the
/// names [`post_toolpath`] gives them. A project file across setups is refused
/// as it is when posting (OI-92).
pub(crate) fn plan_files(
    project: &Project,
    toolpath: &Toolpath,
    selection: &PostSelection,
    grouping: OutputGrouping,
) -> Result<Vec<PlannedFile>, AppError> {
    let extension = selection.load()?.options.file_extension;
    split_units(project, toolpath, grouping)?
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
            Ok(PlannedFile {
                name: file_name(
                    project,
                    &setup.name,
                    unit.label.as_ref(),
                    grouping,
                    index,
                    &extension,
                ),
                setup: setup.name.clone(),
                label: unit.label.as_ref().map(|(title, _)| title.clone()),
                programmed_seconds: unit
                    .groups
                    .iter()
                    .flat_map(|g| g.segments.iter())
                    .map(|s| s.duration.0)
                    .sum(),
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
    /// Cut time of the IR, seconds.
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

fn check_file(
    text: &str,
    file: &str,
    definition: &PostDefinition,
    machine: &openchuck_types::machine::MachineProfile,
    setup: &openchuck_project::Setup,
    part: &Toolpath,
    clamped: usize,
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
        safe_z: setup.clearance.unwrap_or(machine.clearance).safe_z.0,
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
    if clamped > 0 {
        // Lowered feeds make moves longer than the IR says, on purpose.
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

/// Posts `toolpath` and returns the files as (name, text), their reports and notes.
#[allow(clippy::type_complexity)]
pub(crate) fn post_toolpath(
    project: &Project,
    toolpath: &Toolpath,
    selection: &PostSelection,
    grouping: OutputGrouping,
) -> Result<(Vec<(String, String)>, Vec<GcodeReport>, Vec<String>), AppError> {
    let post = TemplatePost::new(selection.load()?);
    if toolpath.groups.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let extension = post.definition().options.file_extension.clone();
    let machine = project
        .machine()
        .ok_or_else(|| AppError::Post("the project has no machine profile".into()))?;
    let operation_names: BTreeMap<String, String> = project
        .operations()
        .iter()
        .map(|o| (o.id.to_string(), o.name.clone()))
        .collect();
    let mut files = Vec::new();
    let mut reports = Vec::new();
    let mut notes = Vec::new();
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
        let mut tools = BTreeMap::new();
        for group in &groups {
            let tag = &group.tags.tool;
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
            let material = setup.material.as_deref().ok_or_else(|| {
                AppError::Post(format!(
                    "setup '{}' has no stock material, so tool '{}' has no spindle speed",
                    setup.name, snapshot.name
                ))
            })?;
            let data = snapshot.cutting_data.get(material).ok_or_else(|| {
                AppError::Post(format!(
                    "tool '{}' has no cutting data for the material '{material}', so no spindle speed (names must match exactly)",
                    snapshot.name
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
                    spindle_rpm: data.spindle_speed.0,
                },
            );
        }
        let job = PostJob {
            frame: RotaryFrame::work(machine, &setup.frame),
            tools,
            operation_names: operation_names.clone(),
            title: match &label {
                Some((title, _)) => format!("{} - {} - {title}", project.name(), setup.name),
                None => format!("{} - {}", project.name(), setup.name),
            },
            revision: None,
            work_offset: Some(format!("{:?}", setup.frame.work_offset.register)),
        };
        let part = Toolpath { groups };
        let output = post
            .post(&part, &job)
            .map_err(|e| AppError::Post(format!("setup '{}': {e}", setup.name)))?;
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
            &part,
            output.clamped_feeds,
        ));
        files.push((name, output.text));
    }
    Ok((files, reports, notes))
}
