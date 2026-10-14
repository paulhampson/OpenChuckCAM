// SPDX-License-Identifier: AGPL-3.0-or-later
//! IR to neutral machine program (work breakdown slice 6 item 1).

use std::collections::BTreeMap;

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, Toolpath};
use openchuck_types::kinematics::{AsRotaryFrame, axis_direction, machine_xyz};
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

    let mut program = MachineProgram::default();
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
        if !(tool.spindle_rpm.is_finite() && tool.spindle_rpm > 0.0) {
            return Err(PostError::Setting {
                name: format!("tools[{}].spindle_rpm", group.tags.tool),
                reason: "must be a number greater than 0".into(),
            });
        }
        let seconds: f64 = group.segments.iter().map(|s| s.duration.0).sum();
        program.steps.push(Step::Comment {
            text: format!(
                "operation {} tool {} about {:.0} s",
                group.tags.operation, tool.name, seconds
            ),
        });
        if current_tool != Some(tool.number) {
            program.steps.push(Step::ToolChange {
                number: tool.number,
                name: tool.name.clone(),
            });
            current_tool = Some(tool.number);
        }
        program.steps.push(Step::SpindleOn {
            rpm: tool.spindle_rpm,
        });

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
                if length <= NO_MOTION && delta_a > NO_MOTION {
                    let turns = delta_a / 360.0;
                    if (turns - turns.round()).abs() * 360.0 <= 1e-9 {
                        program.steps.push(Step::Unwind {
                            from: here.a,
                            to: to.a,
                            at: to,
                        });
                        here = to;
                        continue;
                    }
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
                program.steps.push(Step::Feed {
                    kind: segment.kind,
                    target: to,
                    feed,
                });
            }
            here = to;
        }
    }
    Ok(program)
}
