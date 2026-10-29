// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connections between context-independent operation groups (decision 0241).

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, Segment, ToolPose, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::kinematics::{RotaryFrame, machine_xyz, tool_axis_in_model};
use openchuck_types::machine::{ARange, Clearance, MachineProfile};
use openchuck_types::units::{Degrees, Millimetres, Seconds};

use crate::error::{PathError, Result};

const EPS: f64 = 1e-9;
// More than the maximum 3-decimal XYZ rounding error. The cylinder includes
// all obstacles; the tool extends upward from its tip, above this plane.
const CLEARANCE_ALLOWANCE: f64 = 0.002;

/// Geometry and limits for one connection in a setup/program.
#[derive(Debug, Clone, Copy)]
pub struct ConnectionOptions<'a> {
    /// Machine limits and rotary conventions.
    pub machine: &'a MachineProfile,
    /// Required safe height, measured above the rotary axis.
    pub clearance: Clearance,
    /// Radius enclosing all stock, target, held regions, tabs and configured
    /// fixtures, not just the selected cutting region. Must be conservative.
    pub enclosing_radius: f64,
    /// An explicitly required, unwrapped destination A. `None` permits the
    /// nearest equivalent orientation, with positive travel winning a tie.
    pub required_start_a: Option<Degrees>,
}

/// A placed copy of an operation, with its incoming physical connection.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectedGroup {
    /// The complete group, including the explicit timed incoming moves.
    pub group: ToolpathGroup,
    /// Number of incoming segments prepended to the operation's moves.
    pub link_segments: usize,
    /// Whole-turn offset applied uniformly to the original operation.
    pub a_offset: Degrees,
}

/// Connect a canonical operation to its predecessor in one setup/program.
///
/// The original groups are never mutated. Call independently for each output
/// file, in machining order; never feed an already connected group as `next`.
/// Both tools must have supported non-undercutting profiles, extending from
/// the tip toward machine +Z. The enclosing cylinder proves whole-tool
/// clearance along the entire connection, including at a tool change; this
/// does not use sampled collision checks as a clearance certificate.
pub fn connect_group(
    previous: Option<&ToolpathGroup>,
    next: &ToolpathGroup,
    options: &ConnectionOptions<'_>,
    context: &JobContext<'_>,
) -> Result<ConnectedGroup> {
    context.check_cancelled()?;
    options.machine.validate()?;
    options.clearance.validate("connection.clearance")?;
    if !(options.enclosing_radius.is_finite() && options.enclosing_radius >= 0.0) {
        return Err(refuse(
            next,
            "the enclosing radius is not a finite non-negative bound",
        ));
    }
    if previous.is_some_and(|p| p.tags.setup != next.tags.setup) {
        return Err(refuse(
            next,
            "a connection cannot cross a setup/program boundary",
        ));
    }
    let from = previous.map(last_pose);
    let offset = placement(next, from.map(|p| p.a), options, context)?;
    let mut group = next.clone();
    group.start.a.0 += offset;
    let frame = RotaryFrame::of_profile(options.machine);
    check_pose(group.start, next, options, &frame)?;
    for (i, s) in group.segments.iter_mut().enumerate() {
        context.check_cancelled()?;
        s.validate(&format!("operation {} segment {i}", next.tags.operation))?;
        s.end.a.0 += offset;
        check_pose(s.end, next, options, &frame)?;
    }
    check_boundary(group.start, next, options, &frame)?;
    check_boundary(last_pose(&group), next, options, &frame)?;
    let Some(from) = from else {
        return Ok(ConnectedGroup {
            group,
            link_segments: 0,
            a_offset: Degrees(offset),
        });
    };
    // Both complete tool envelopes point up and remain strictly above the
    // enclosing cylinder. Linear XYZ interpolation stays in that half-space
    // at every A, even if both ends lie on opposite sides of an obstacle.
    // This conservative proof also covers both tools at a tool change.
    check_boundary(from, next, options, &frame)?;
    let to = group.start;
    let a = machine_xyz(&frame, from.tip, from.a);
    let b = machine_xyz(&frame, to.tip, to.a);
    let linear = a.distance(b);
    let da = (to.a.0 - from.a.0).abs();
    let mut link_segments = 0;
    if linear > EPS || da > EPS {
        let feeds = options.machine.max_feeds;
        let seconds = 60.0
            * (linear / feeds.rapid_linear.0)
                .max((b.z - a.z).abs() / feeds.rapid_z.0)
                .max(da / feeds.rapid_rotary.0);
        let link = Segment {
            kind: MoveKind::Rapid,
            end: to,
            contact_feed: None,
            duration: Seconds(seconds),
            contact_length: Millimetres(linear),
        };
        link.validate("operation connection")?;
        group.segments.insert(0, link);
        for index in &mut group.rotary_reset_after {
            *index = index
                .checked_add(1)
                .ok_or_else(|| refuse(next, "invalid pass boundary index"))?;
        }
        link_segments = 1;
    }
    group.start = from;
    Ok(ConnectedGroup {
        group,
        link_segments,
        a_offset: Degrees(offset),
    })
}

fn last_pose(group: &ToolpathGroup) -> ToolPose {
    group.segments.last().map_or(group.start, |s| s.end)
}

fn refuse(group: &ToolpathGroup, reason: &str) -> PathError {
    PathError::Strategy(format!(
        "operation '{}': cannot connect safely: {reason}",
        group.tags.operation
    ))
}

fn placement(
    group: &ToolpathGroup,
    previous: Option<Degrees>,
    options: &ConnectionOptions<'_>,
    context: &JobContext<'_>,
) -> Result<f64> {
    let mut lo = group.start.a.0;
    let mut hi = lo;
    for a in std::iter::once(group.start.a).chain(group.segments.iter().map(|s| s.end.a)) {
        context.check_cancelled()?;
        if !a.0.is_finite() {
            return Err(refuse(group, "an A coordinate is not finite"));
        }
        lo = lo.min(a.0);
        hi = hi.max(a.0);
    }
    let (k_lo, k_hi) = match options.machine.a_range {
        ARange::Continuous => (f64::NEG_INFINITY, f64::INFINITY),
        ARange::Limited { min, max } => (
            ((min.0 - lo) / 360.0).ceil(),
            ((max.0 - hi) / 360.0).floor(),
        ),
    };
    if k_lo > k_hi {
        return Err(refuse(
            group,
            "the complete operation cannot fit within the A limits by whole turns",
        ));
    }
    let k = if let Some(required) = options.required_start_a {
        let turns = (required.0 - group.start.a.0) / 360.0;
        if !turns.is_finite() || (turns - turns.round()).abs() * 360.0 > EPS {
            return Err(refuse(
                group,
                "the required A is not equivalent to the operation's start orientation",
            ));
        }
        turns.round()
    } else if let Some(previous) = previous {
        if !previous.0.is_finite() {
            return Err(refuse(group, "the preceding A coordinate is not finite"));
        }
        // floor(x + 1/2), unlike round(), resolves both signs of the exact
        // half-turn tie toward positive machine A travel.
        (((previous.0 - group.start.a.0) / 360.0 + 0.5).floor()).clamp(k_lo, k_hi)
    } else {
        // The first move starts at an unknown controller position. Preserve
        // the standalone group's reference; do not invent a predecessor.
        0.0
    };
    if !(k.is_finite() && k >= k_lo && k <= k_hi) {
        return Err(refuse(group, "the required placement exceeds the A limits"));
    }
    Ok(k * 360.0)
}

fn check_pose(
    pose: ToolPose,
    group: &ToolpathGroup,
    options: &ConnectionOptions<'_>,
    frame: &RotaryFrame,
) -> Result<Point3> {
    if !pose.tip.is_finite() || !pose.a.0.is_finite() {
        return Err(refuse(group, "a placed coordinate is not finite"));
    }
    if let ARange::Limited { min, max } = options.machine.a_range
        && (pose.a.0 < min.0 - EPS || pose.a.0 > max.0 + EPS)
    {
        return Err(refuse(group, "a placed coordinate exceeds the A limits"));
    }
    let xyz = machine_xyz(frame, pose.tip, pose.a);
    Ok(xyz)
}

fn check_boundary(
    pose: ToolPose,
    group: &ToolpathGroup,
    options: &ConnectionOptions<'_>,
    frame: &RotaryFrame,
) -> Result<()> {
    let xyz = check_pose(pose, group, options, frame)?;
    // This service validates linear travel for the newly generated link.
    // Cutting-segment linear travel remains with generation/post checks;
    // whole-group A limits above are required for equivalent placement.
    if !options.machine.travel.contains(xyz) {
        return Err(refuse(
            group,
            &format!(
                "connection position ({:.6}, {:.6}, {:.6}) exceeds the machine's linear travel",
                xyz.x, xyz.y, xyz.z
            ),
        ));
    }
    let height = xyz.z - frame.origin.z;
    if height < options.clearance.safe_z.0 - EPS
        || height < options.enclosing_radius + CLEARANCE_ALLOWANCE
    {
        return Err(refuse(
            group,
            "the operation boundary is below the required clear plane; enlarge the setup's clearance",
        ));
    }
    if (pose.axis.get() - tool_axis_in_model(frame, pose.a).get()).length() > 1e-8 {
        return Err(refuse(
            group,
            "the boundary tool axis is not the supported vertical orientation",
        ));
    }
    Ok(())
}
