// SPDX-License-Identifier: AGPL-3.0-or-later
//! IR to neutral machine program (work breakdown slice 6 item 1).

use std::collections::BTreeMap;

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, Toolpath};
use openchuck_types::kinematics::{AsRotaryFrame, axis_direction, machine_xyz};
use openchuck_types::machine::ARange;
use openchuck_types::units::Degrees;

use crate::error::{PostError, Result};
use crate::feed::{FeedRule, FeedWord, MoveMetrics, NO_MOTION};
use crate::program::{MachineProgram, Step, Target};

/// What the program needs to know about a tool beyond the IR (GC-14).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSetting {
    /// The tool number the program uses.
    pub number: u32,
    /// The tool's name, for comments.
    pub name: String,
    /// Spindle speed, revolutions per minute. Greater than zero.
    pub spindle_rpm: f64,
}

/// Settings of the conversion.
#[derive(Debug, Clone)]
pub struct ConvertOptions<F> {
    /// Controller throughput settings (GC-17).
    pub short_blocks: crate::ShortBlockSettings,
    /// Feed decimal places used by the renderer, or None for full precision.
    pub feed_decimals: Option<u32>,
    /// Allowed rotary travel. Limited axes never receive counter resets.
    pub a_range: ARange,
    /// Required safe Z in the output work frame. A group may offer a
    /// non-moving counter reset only after reaching at least this height.
    pub safe_z: f64,
    /// The work frame the program is written in (`RotaryFrame::work`).
    pub frame: F,
    /// The per-controller feed rule (decision 0019 item 5).
    pub feed_rule: FeedRule,
    /// The most F may be in units per minute; a higher F is lowered and
    /// counted in [`MachineProgram::clamped_feeds`] (GC-04).
    pub max_feed: Option<f64>,
    /// Tool settings keyed by the IR's tool tag.
    pub tools: BTreeMap<String, ToolSetting>,
    /// Decimal places of the written positions (X, Y, Z) and of A. The
    /// controller times a move from the positions as written, so lengths and
    /// feeds are worked out from positions rounded this way, and a move that
    /// rounds to no motion is dropped. `None` keeps full precision.
    pub decimals: Option<(u32, u32)>,
    /// Display names of operations by the IR's operation tag, written in the
    /// comments that name each operation (GC-06). A tag with no name is
    /// written as it is.
    pub operation_names: BTreeMap<String, String>,
    /// Spindle speeds by the IR's operation tag, in revolutions per minute.
    /// An operation with an entry runs at it; any other runs at its tool's
    /// [`ToolSetting::spindle_rpm`] (decision 0196).
    pub operation_spindle: BTreeMap<String, f64>,
}

/// `value` rounded to `decimals` places as the post writes it (ties to even
/// on the exact binary value, never negative zero).
pub fn round_written(value: f64, decimals: u32) -> f64 {
    let places = usize::try_from(decimals).unwrap_or(9).min(9);
    crate::engine::fixed_str(value, places)
        .parse()
        .unwrap_or(value)
}

/// Converts a toolpath to the neutral machine program.
pub fn to_machine_program<F: AsRotaryFrame>(
    toolpath: &Toolpath,
    options: &ConvertOptions<F>,
) -> Result<MachineProgram> {
    toolpath.validate()?;
    options.short_blocks.validate()?;
    if options.short_blocks.enabled() && options.feed_rule == FeedRule::Contact {
        return Err(PostError::Setting {
            name: "feed_rule".into(),
            reason: "Contact has no controller timing model for short-block management; choose a controller feed rule or turn the check Off".into(),
        });
    }
    if !options.safe_z.is_finite() {
        return Err(PostError::Setting {
            name: "safe_z".into(),
            reason: "must be finite".into(),
        });
    }
    if let Some(max) = options.max_feed
        && !(max.is_finite() && max > 0.0)
    {
        return Err(PostError::Setting {
            name: "max_feed".into(),
            reason: "must be a number greater than 0".into(),
        });
    }
    let frame = options.frame.rotary_frame();
    let axis = axis_direction(frame.axis);
    let target_of = |tip: Point3, a: Degrees| {
        let p = machine_xyz(&frame, tip, a);
        match options.decimals {
            Some((pos, rot)) => Target {
                x: round_written(p.x, pos),
                y: round_written(p.y, pos),
                z: round_written(p.z, pos),
                a: round_written(a.0, rot),
            },
            None => Target {
                x: p.x,
                y: p.y,
                z: p.z,
                a: a.0,
            },
        }
    };
    let radius_of = |t: &Target| {
        let along = t.x * axis.x + t.y * axis.y + t.z * axis.z;
        let (px, py, pz) = (
            t.x - along * axis.x,
            t.y - along * axis.y,
            t.z - along * axis.z,
        );
        (px * px + py * py + pz * pz).sqrt()
    };

    let mut program = MachineProgram {
        verification_path: toolpath.clone(),
        ..MachineProgram::default()
    };
    let mut current_tool: Option<u32> = None;
    for (g, group) in toolpath.groups.iter().enumerate() {
        let tool = options
            .tools
            .get(&group.tags.tool)
            .ok_or_else(|| PostError::UnknownTool {
                group: g,
                operation: group.tags.operation.clone(),
                tool: group.tags.tool.clone(),
            })?;
        let (rpm, source) = match options.operation_spindle.get(&group.tags.operation) {
            Some(rpm) => (*rpm, format!("operation_spindle[{}]", group.tags.operation)),
            None => (
                tool.spindle_rpm,
                format!("tools[{}].spindle_rpm", group.tags.tool),
            ),
        };
        if !(rpm.is_finite() && rpm > 0.0) {
            return Err(PostError::Setting {
                name: source,
                reason: "must be a number greater than 0".into(),
            });
        }
        let seconds: f64 = group.segments.iter().map(|s| s.duration.0).sum();
        let operation = options
            .operation_names
            .get(&group.tags.operation)
            .unwrap_or(&group.tags.operation);
        program.steps.push(Step::Comment {
            text: format!(
                "operation {operation}, tool T{} {}, about {seconds:.0} s",
                tool.number, tool.name
            ),
        });
        if current_tool != Some(tool.number) {
            program.steps.push(Step::ToolChange {
                number: tool.number,
                name: tool.name.clone(),
            });
            current_tool = Some(tool.number);
        }
        program.steps.push(Step::SpindleOn { rpm });

        let group_comment = program.steps.len() - 3; // overridden below by locating the last operation comment
        let comment_index = program
            .steps
            .iter()
            .rposition(|s| matches!(s, Step::Comment { .. }))
            .unwrap_or(group_comment);
        let mut runs: Vec<Vec<Block>> = vec![Vec::new()];
        let mut here = target_of(group.start.tip, group.start.a);
        program.steps.push(Step::Rapid { target: here });
        for (s, segment) in group.segments.iter().enumerate() {
            let to = target_of(segment.end.tip, segment.end.a);
            let length =
                ((to.x - here.x).powi(2) + (to.y - here.y).powi(2) + (to.z - here.z).powi(2))
                    .sqrt();
            let delta_a = (to.a - here.a).abs();
            let moved = length > NO_MOTION || delta_a > NO_MOTION;
            if segment.kind == MoveKind::Rapid {
                if runs.last().is_some_and(|r| !r.is_empty()) {
                    runs.push(Vec::new());
                }
                if moved {
                    program.steps.push(Step::Rapid { target: to });
                }
            } else if moved {
                let contact_feed = segment.contact_feed.map_or(0.0, |f| f.0);
                let metrics = MoveMetrics {
                    length,
                    delta_a,
                    end_radius: radius_of(&to),
                    contact_feed,
                    duration: segment.duration.0,
                };
                if metrics.duration <= 0.0 && options.feed_rule != FeedRule::Contact {
                    return Err(PostError::Feed {
                        group: g,
                        segment: s,
                        reason: "the move has motion but no duration, so no feed can be worked out"
                            .into(),
                    });
                }
                let mut feed = options
                    .feed_rule
                    .feed(&metrics)
                    .ok_or_else(|| PostError::Feed {
                        group: g,
                        segment: s,
                        reason: "no feed".into(),
                    })?;
                if let (FeedWord::PerMinute(f), Some(max)) = (&mut feed, options.max_feed)
                    && *f > max
                {
                    *f = max;
                    program.clamped_feeds += 1;
                }
                if let FeedWord::PerMinute(f) | FeedWord::InverseTime(f) = feed
                    && !(f.is_finite() && f > 0.0)
                {
                    return Err(PostError::Feed {
                        group: g,
                        segment: s,
                        reason: format!("the feed worked out is {f}, which cannot be written"),
                    });
                }
                if options.short_blocks.enabled() {
                    let actual = written_feed(feed, options.feed_decimals)?;
                    let amount = match options.feed_rule {
                        FeedRule::InverseTime => 60.0,
                        _ => {
                            let mut unit = metrics;
                            unit.duration = 1.0;
                            feed_number(options.feed_rule.feed(&unit).ok_or_else(|| {
                                PostError::Feed {
                                    group: g,
                                    segment: s,
                                    reason: "no motion".into(),
                                }
                            })?)
                        }
                    };
                    let seconds = amount / actual;
                    if !(seconds.is_finite() && seconds > 0.0) {
                        return Err(PostError::Feed {
                            group: g,
                            segment: s,
                            reason: "posted duration is not finite and positive".into(),
                        });
                    }
                    if let Some(run) = runs.last_mut() {
                        run.push(Block {
                            step: program.steps.len(),
                            segment: s,
                            seconds,
                            amount,
                            contact_length: segment.contact_length.0,
                            feed,
                        });
                    }
                }
                program.steps.push(Step::Feed {
                    kind: segment.kind,
                    target: to,
                    feed,
                });
            }
            if !moved
                && options.short_blocks.enabled()
                && options.short_blocks.mode == openchuck_types::machine::ShortBlockMode::SlowDown
                && let Some(seg) = program
                    .verification_path
                    .groups
                    .get_mut(g)
                    .and_then(|group| group.segments.get_mut(s))
            {
                seg.duration.0 = 0.0;
            }
            here = to;
        }
        if options.short_blocks.enabled() {
            manage_group(
                &mut program,
                &runs,
                g,
                &group.tags.operation,
                comment_index,
                options.short_blocks,
                options.feed_decimals,
            )?;
        }
        if options.a_range == ARange::Continuous && here.z >= options.safe_z {
            program.steps.push(Step::RotaryBoundary { at: here });
        }
    }
    Ok(program)
}

#[derive(Clone, Copy)]
struct Block {
    step: usize,
    segment: usize,
    seconds: f64,
    amount: f64,
    contact_length: f64,
    feed: FeedWord,
}
fn feed_number(feed: FeedWord) -> f64 {
    match feed {
        FeedWord::PerMinute(f) | FeedWord::InverseTime(f) => f,
    }
}
fn written_feed(feed: FeedWord, decimals: Option<u32>) -> Result<f64> {
    let value = feed_number(feed);
    let written = match decimals {
        Some(d) => crate::feed_str(value, d as usize)
            .parse::<f64>()
            .map_err(|_| PostError::Setting {
                name: "feed".into(),
                reason: "cannot read rendered feed".into(),
            })?,
        None => value,
    };
    if written.is_finite() && written > 0.0 {
        Ok(written)
    } else {
        Err(PostError::Setting {
            name: "feed".into(),
            reason: "rendered feed must be finite and positive".into(),
        })
    }
}
/// Rounding an F word upward would undercut the requested duration. Select
/// the nearest representable lower feed and then use its actual duration.
fn lower_feed(value: f64, decimals: Option<u32>) -> Result<f64> {
    let Some(d) = decimals else {
        return Ok(value);
    };
    let text = crate::feed_str(value, d as usize);
    let rounded = text.parse::<f64>().map_err(|_| PostError::Setting {
        name: "feed".into(),
        reason: "cannot read rendered feed".into(),
    })?;
    let result = if rounded > value {
        let places = text.split_once('.').map_or(0, |(_, digits)| digits.len());
        let unit = (0..places).fold(1.0, |v, _| v / 10.0);
        rounded - unit
    } else {
        rounded
    };
    if result.is_finite() && result > 0.0 {
        Ok(result)
    } else {
        Err(PostError::Setting {
            name: "min_block_ms".into(),
            reason: "requires a feed too small to write; reduce the minimum block time".into(),
        })
    }
}

fn manage_group(
    program: &mut MachineProgram,
    runs: &[Vec<Block>],
    group: usize,
    operation: &str,
    comment_index: usize,
    settings: crate::ShortBlockSettings,
    decimals: Option<u32>,
) -> Result<()> {
    let mut report = crate::ShortBlockReport {
        operation: operation.into(),
        settings,
        sources: Default::default(),
        short_moves: 0,
        longest_run: 0,
        affected_windows: 0,
        worst_window_ms: f64::INFINITY,
        effective_feed: 0.0,
        added_seconds: 0.0,
        slowed_moves: 0,
    };
    let limit = settings.min_block_ms / 1000.0;
    for run in runs.iter().filter(|r| !r.is_empty()) {
        let n = settings.block_window.min(run.len());
        let mut streak = 0;
        for b in run {
            if b.seconds < limit {
                report.short_moves += 1;
                streak += 1;
                report.longest_run = report.longest_run.max(streak);
            } else {
                streak = 0;
            }
        }
        for window in run.windows(n) {
            let seconds = window.iter().map(|b| b.seconds).sum::<f64>();
            let mean = seconds / n as f64;
            if mean < limit {
                report.affected_windows += 1;
            }
            if mean * 1000.0 < report.worst_window_ms {
                report.worst_window_ms = mean * 1000.0;
                report.effective_feed =
                    window.iter().map(|b| b.contact_length).sum::<f64>() * 60.0 / seconds;
            }
        }
        let mut times: Vec<f64> = run.iter().map(|b| b.seconds).collect();
        crate::short_blocks::lengthen(&mut times, settings);
        for (b, time) in run.iter().zip(times) {
            let mut posted_seconds = b.seconds;
            if time > b.seconds {
                let number = lower_feed(b.amount / time, decimals)?;
                posted_seconds = b.amount / number;
                report.added_seconds += posted_seconds - b.seconds;
                report.slowed_moves += 1;
                if settings.mode == openchuck_types::machine::ShortBlockMode::SlowDown {
                    let feed = match b.feed {
                        FeedWord::PerMinute(_) => FeedWord::PerMinute(number),
                        FeedWord::InverseTime(_) => FeedWord::InverseTime(number),
                    };
                    if let Some(Step::Feed { feed: f, .. }) = program.steps.get_mut(b.step) {
                        *f = feed;
                    }
                }
            }
            if settings.mode == openchuck_types::machine::ShortBlockMode::SlowDown
                && let Some(segment) = program
                    .verification_path
                    .groups
                    .get_mut(group)
                    .and_then(|g| g.segments.get_mut(b.segment))
            {
                segment.duration.0 = posted_seconds;
            }
        }
    }
    if !report.worst_window_ms.is_finite() {
        report.worst_window_ms = 0.0;
    }
    if settings.mode == openchuck_types::machine::ShortBlockMode::SlowDown {
        let seconds = program
            .verification_path
            .groups
            .get(group)
            .map_or(0.0, |g| g.segments.iter().map(|s| s.duration.0).sum());
        if let Some(Step::Comment { text }) = program.steps.get_mut(comment_index)
            && let Some((head, _)) = text.rsplit_once(", about ")
        {
            *text = format!("{head}, about {seconds:.0} s");
        }
    }
    program.short_blocks.push(report);
    Ok(())
}
