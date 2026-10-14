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

    fn load(&self) -> Result<PostDefinition, AppError> {
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

fn file_stem(index: usize, name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() {
        "setup".to_string()
    } else {
        slug
    };
    format!("{:02}-{slug}", index + 1)
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

/// Posts `toolpath` and returns the files as (name, text), their reports and notes.
#[allow(clippy::type_complexity)]
pub(crate) fn post_toolpath(
    project: &Project,
    toolpath: &Toolpath,
    selection: &PostSelection,
) -> Result<(Vec<(String, String)>, Vec<GcodeReport>, Vec<String>), AppError> {
    let post = TemplatePost::new(selection.load()?);
    if toolpath.groups.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let extension = post.definition().options.file_extension.clone();
    let machine = project
        .machine()
        .ok_or_else(|| AppError::Post("the project has no machine profile".into()))?;
    // Runs of consecutive groups from one setup.
    let mut runs: Vec<(String, Vec<ToolpathGroup>)> = Vec::new();
    for group in &toolpath.groups {
        match runs.last_mut() {
            Some((setup, groups)) if *setup == group.tags.setup => groups.push(group.clone()),
            _ => runs.push((group.tags.setup.clone(), vec![group.clone()])),
        }
    }
    let mut files = Vec::new();
    let mut reports = Vec::new();
    let mut notes = Vec::new();
    for (index, (setup_tag, groups)) in runs.into_iter().enumerate() {
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
            title: format!("{} - {}", project.name(), setup.name),
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
        let name = format!("{}.{extension}", file_stem(index, &setup.name));
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
