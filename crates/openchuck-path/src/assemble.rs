// SPDX-License-Identifier: AGPL-3.0-or-later
//! The shared services that turn a strategy's passes into IR (architecture
//! section 5.2; decision record 0018 items 4, 5, 8 and 9).
//!
//! 1. **A-axis management (OP-15).** Within a pass A is made continuous: each
//!    angle is the equivalent nearest the one before, so no step turns more
//!    than half a turn. Each pass is then moved by whole turns so that it
//!    starts as near the end of the previous pass as the machine's A range
//!    allows; a pass that spans more than the range is refused, naming it,
//!    since the axis cannot unwind in the middle of a cut. Passes start at
//!    the nearest equivalent, so links take the shorter way round. At the
//!    end of the group A unwinds to (-180, 180] at the safe Z: a rapid to the
//!    same tip with A changed by whole turns (decision record 0001 item 1).
//! 2. **Linking (OP-14).** In coordinates fixed to the machine with the
//!    rotary axis through the origin, every link moves in the plane at the
//!    retract height above the axis: the safe cylinder's radius, or the safe
//!    Z (SET-06). Every point of that plane is at least that far from the
//!    axis, so a link at any A and any position across the axis stays
//!    outside the safe cylinder, and so outside stock the cylinder encloses;
//!    the group is refused when the stock reaches beyond the retract height.
//!    Retracts and descents run along the tool axis. A retract first leaves
//!    the cut at the plunge feed until the tip is the approach distance above
//!    the stock on its line, then rapids. A descent is a rapid
//!    that stops the approach distance above where the stock begins on its
//!    line (or above the first cut point, whichever is higher), then the
//!    pass's entry: a plunge at the plunge feed, or a lead arc, ramp or
//!    helix (the `entry` module; decision record 0026). Every move is put to
//!    the [`CollisionOracle`].
//! 3. **Feed and duration (OP-13).** For a cutting move the contact feed `f`
//!    (mm/min) applies to the cutter contact point, which moves from `c0` to
//!    `c1`, so the move takes `t = 60 |c1 - c0| / f` seconds and its contact
//!    length is `|c1 - c0|`. The post derives the machine feed from the move
//!    and `t` (a compensated feed per minute, or inverse time). No move may be
//!    shorter than the slowest axis allows at the machine's feed limits; where
//!    that lengthens a cutting move, the contact feed there is lower than
//!    programmed and a [`PathNote`] says so. Moves without a contact
//!    (rapids, plunges, retracts) use the tool tip for the length.

mod entry;

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, Segment, ToolPose, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

use crate::error::{PathError, Result};
use crate::params::{CommonParameters, EntryParameters, Retract};
use crate::rotary::{AxisPosition, RotaryMapping, fit_range, unwind_turns, unwrap_continuous};
use crate::stock::CollisionOracle;
use crate::strategy::{CutPoint, Pass, StrategyInput};
use openchuck_types::machine::MaxFeeds;

/// Something the user should know about a generated toolpath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathNote {
    /// The segment it concerns, by index in the group.
    pub segment: usize,
    /// What happened.
    pub message: String,
}

/// A toolpath group with notes about it.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedPath {
    /// The moves.
    pub group: ToolpathGroup,
    /// Notes, in segment order.
    pub notes: Vec<PathNote>,
}

const EPS: f64 = 1e-9;

/// How much longer, in millimetres, positions written to three decimals can
/// make a move: half a unit in each of two end points on three axes.
const WRITTEN_ALLOWANCE: f64 = 0.002;

struct Builder<'a> {
    rotary: RotaryMapping,
    max_feeds: MaxFeeds,
    collision: &'a dyn CollisionOracle,
    pose: ToolPose,
    contact: Point3,
    segments: Vec<Segment>,
    notes: Vec<PathNote>,
}

impl Builder<'_> {
    fn pose(&self, tip: Point3, a: Degrees) -> ToolPose {
        ToolPose {
            tip,
            axis: self.rotary.tool_axis(a),
            a,
        }
    }

    fn at(&self) -> AxisPosition {
        self.rotary.to_axis(self.pose.tip, self.pose.a)
    }

    /// Adds a move to `end`. `contact` is the cutter contact at the end of a
    /// cutting move; other moves measure the tip.
    fn push(
        &mut self,
        kind: MoveKind,
        end: ToolPose,
        contact: Option<Point3>,
        feed: Option<FeedRate>,
    ) -> Result<()> {
        let from = self.at();
        let to = self.rotary.to_axis(end.tip, end.a);
        let da = (end.a.0 - self.pose.a.0).abs();
        let moves = (to.along - from.along).abs() > EPS
            || (to.across - from.across).abs() > EPS
            || (to.height - from.height).abs() > EPS
            || da > EPS;
        if !moves {
            if let Some(c) = contact {
                self.contact = c;
            }
            return Ok(());
        }
        if !self.collision.allows(&self.pose, &end, kind) {
            return Err(PathError::Strategy(format!(
                "move {} ({kind:?}) collides, and no way round it is known",
                self.segments.len()
            )));
        }
        let limits = &self.max_feeds;
        let (linear, z, rotary) = if kind.is_fed() {
            (limits.linear.0, limits.z.0, limits.rotary.0)
        } else {
            (
                limits.rapid_linear.0,
                limits.rapid_z.0,
                limits.rapid_rotary.0,
            )
        };
        // The F word is the speed along the straight line of the linear axes, so
        // that line, not just each axis, must stay within the limit the
        // checker applies: the linear limit if X or Y moves, otherwise Z's. The
        // post works the feed out from positions rounded to the written
        // decimals, which can lengthen a short move by a few thousandths of a
        // millimetre, so fed moves are timed for that much more (decision
        // record 0034, as built).
        let (dx, dy, dz) = (
            (to.along - from.along).abs(),
            (to.across - from.across).abs(),
            (to.height - from.height).abs(),
        );
        let allowance = if kind.is_fed() {
            WRITTEN_ALLOWANCE
        } else {
            0.0
        };
        let line = (dx * dx + dy * dy + dz * dz).sqrt();
        let line_limit = if dx > EPS || dy > EPS { linear } else { z };
        let axis_time = 60.0
            * [
                dx / linear,
                dy / linear,
                dz / z,
                da / rotary,
                if line > EPS {
                    (line + allowance) / line_limit
                } else {
                    0.0
                },
            ]
            .into_iter()
            .fold(0.0, f64::max);
        let (length, end_reference) = match contact {
            Some(c) => (c.distance(self.contact), c),
            None => (end.tip.distance(self.pose.tip), end.tip),
        };
        let duration = match feed {
            Some(f) => {
                let at_feed = 60.0 * length / f.0;
                if kind == MoveKind::Cutting && axis_time > at_feed * (1.0 + 1e-6) {
                    self.notes.push(PathNote {
                        segment: self.segments.len(),
                        message: format!(
                            "the machine's feed limits slow this move: the contact feed is {:.1} \
                             mm/min, not {:.1}",
                            60.0 * length / axis_time,
                            f.0
                        ),
                    });
                }
                at_feed.max(axis_time)
            }
            None => axis_time,
        };
        let segment = Segment {
            kind,
            end,
            contact_feed: feed,
            duration: Seconds(duration),
            contact_length: Millimetres(length),
        };
        segment.validate(&format!("segments[{}]", self.segments.len()))?;
        self.segments.push(segment);
        self.pose = end;
        self.contact = end_reference;
        Ok(())
    }

    /// Moves along the tool axis to `height` above the rotary axis, if below it.
    fn retract(&mut self, height: f64) -> Result<()> {
        let at = self.at();
        if at.height >= height - EPS {
            return Ok(());
        }
        let tip = self
            .rotary
            .to_model(AxisPosition { height, ..at }, self.pose.a);
        let end = self.pose(tip, self.pose.a);
        self.push(MoveKind::Rapid, end, None, None)
    }
}

fn check_point(p: &CutPoint, pass: usize, point: usize) -> Result<()> {
    let ok = p.tip.is_finite() && p.contact.is_finite() && p.a.0.is_finite();
    if ok {
        Ok(())
    } else {
        Err(PathError::Strategy(format!(
            "pass {} point {} is not a finite position",
            pass + 1,
            point + 1
        )))
    }
}

/// Places A, links the passes and times every move (see the module
/// documentation).
pub fn assemble(
    passes: &[Pass],
    input: &StrategyInput<'_>,
    common: &CommonParameters,
    context: &JobContext<'_>,
) -> Result<GeneratedPath> {
    assemble_with_entry(passes, input, common, &EntryParameters::PLUNGE, context)
}

/// [`assemble`] with the entry and exit moves of `entry` (decision record
/// 0026) in place of a plain plunge.
pub fn assemble_with_entry(
    passes: &[Pass],
    input: &StrategyInput<'_>,
    common: &CommonParameters,
    entry: &EntryParameters,
    context: &JobContext<'_>,
) -> Result<GeneratedPath> {
    let rotary = input.rotary;
    let link_height = match common.retract {
        Retract::Clearance => input.clearance.radius.0,
        Retract::SafeZ => input.clearance.safe_z.0,
    };
    let safe_height = input.clearance.safe_z.0.max(link_height);
    if let Some(r) = input.stock.max_radius()
        && r > link_height + EPS
    {
        return Err(PathError::Strategy(format!(
            "the stock reaches {r:.3} mm from the rotary axis, beyond the retract height of \
             {link_height:.3} mm, so links could cross it (OP-14); enlarge the setup's clearance"
        )));
    }

    // A-axis management: continuous within a pass, fitted by whole turns.
    let mut placed: Vec<Vec<CutPoint>> = Vec::new();
    let mut previous = Degrees(0.0);
    for (i, pass) in passes.iter().enumerate() {
        if pass.points.is_empty() {
            continue;
        }
        for (j, p) in pass.points.iter().enumerate() {
            check_point(p, i, j)?;
        }
        let angles: Vec<Degrees> = pass.points.iter().map(|p| p.a).collect();
        let unwrapped = unwrap_continuous(&angles, previous);
        let shift = fit_range(
            &unwrapped,
            rotary.range,
            previous,
            &format!("pass {}", i + 1),
        )?;
        let points: Vec<CutPoint> = pass
            .points
            .iter()
            .zip(&unwrapped)
            .map(|(p, a)| CutPoint {
                a: Degrees(a.0 + shift),
                ..*p
            })
            .collect();
        if let Some(last) = points.last() {
            previous = last.a;
        }
        placed.push(points);
    }
    let Some(first) = placed.first().and_then(|p| p.first()).copied() else {
        return Err(PathError::Strategy(
            "the strategy produced no cut for this selection".to_owned(),
        ));
    };

    let start_at = rotary.to_axis(first.tip, first.a);
    let start_tip = rotary.to_model(
        AxisPosition {
            height: safe_height,
            ..start_at
        },
        first.a,
    );
    let start = ToolPose {
        tip: start_tip,
        axis: rotary.tool_axis(first.a),
        a: first.a,
    };
    let mut b = Builder {
        rotary,
        max_feeds: input.max_feeds,
        collision: input.collision,
        pose: start,
        contact: start_tip,
        segments: Vec::new(),
        notes: Vec::new(),
    };

    let entry = entry::Entry::new(input, common, entry)?;
    let total = placed.len() as u64;
    for (i, points) in placed.iter().enumerate() {
        context.check_cancelled()?;
        context.report("assemble passes", i as u64, Some(total));
        let Some(head) = points.first() else {
            continue;
        };
        if i > 0 {
            leave(&mut b, input, common, link_height)?;
            b.retract(link_height)?;
            let target = rotary.to_axis(head.tip, head.a);
            let height = b.at().height.max(link_height);
            let tip = rotary.to_model(AxisPosition { height, ..target }, head.a);
            let end = b.pose(tip, head.a);
            b.push(MoveKind::Rapid, end, None, None)?;
        }
        entry.enter(&mut b, input, common, i, points)?;
        let contacts = smoothed_contacts(points);
        b.contact = contacts.first().copied().unwrap_or(head.contact);
        for (p, c) in points.iter().zip(&contacts).skip(1) {
            let end = b.pose(p.tip, p.a);
            b.push(MoveKind::Cutting, end, Some(*c), Some(common.feed))?;
        }
        entry.exit(&mut b, common, input, i, points)?;
    }

    leave(&mut b, input, common, safe_height)?;
    b.retract(safe_height)?;
    let turns = unwind_turns(b.pose.a);
    if turns.abs() > EPS {
        let end = b.pose(b.pose.tip, Degrees(b.pose.a.0 + turns));
        b.push(MoveKind::Rapid, end, None, None)?;
    }
    context.report("assemble passes", total, Some(total));
    Ok(GeneratedPath {
        group: ToolpathGroup {
            tags: input.tags.clone(),
            start,
            segments: b.segments,
        },
        notes: b.notes,
    })
}

/// Points either side of a contact averaged with it to measure the contact
/// path (see [`smoothed_contacts`]).
pub const CONTACT_SMOOTHING: usize = 2;

/// The contact points of a pass for measuring its contact path: each the
/// mean of itself and up to [`CONTACT_SMOOTHING`] neighbours either side
/// (fewer at the ends, keeping the window centred). On a mesh the contact
/// jumps from facet to facet by about the tool radius times the facet angle
/// (decision record 0015); averaging removes that jitter from the contact
/// length while a smooth path is left almost unchanged.
pub fn smoothed_contacts(points: &[CutPoint]) -> Vec<Point3> {
    let n = points.len();
    (0..n)
        .map(|i| {
            let half = CONTACT_SMOOTHING.min(i).min(n - 1 - i);
            let window = points.get(i - half..=i + half).unwrap_or_default();
            let count = window.len().max(1) as f64;
            let sum = window
                .iter()
                .fold(Point3::ORIGIN, |acc, p| acc + p.contact.to_vec());
            Point3::new(sum.x / count, sum.y / count, sum.z / count)
        })
        .collect()
}

/// Leaves the cut along the tool axis at the plunge feed until the tool tip
/// is the approach distance above the stock on that line (the stock model
/// may not know what this operation has cut), so the rapid retract that
/// follows starts in the clear.
fn leave(
    b: &mut Builder<'_>,
    input: &StrategyInput<'_>,
    common: &CommonParameters,
    height: f64,
) -> Result<()> {
    let here = b.at();
    if here.height >= height - EPS {
        return Ok(());
    }
    let axis = b.rotary.tool_axis(b.pose.a);
    let down = openchuck_types::geometry::UnitVec3::new(-axis.get()).map_err(PathError::from)?;
    let top = b.rotary.to_model(AxisPosition { height, ..here }, b.pose.a);
    let Some(d) = input.stock.ray_entry(top, down, height - here.height) else {
        return Ok(());
    };
    let clear = (height - d + common.approach_distance.0).min(height);
    if clear <= here.height + EPS {
        return Ok(());
    }
    let tip = b.rotary.to_model(
        AxisPosition {
            height: clear,
            ..here
        },
        b.pose.a,
    );
    let end = b.pose(tip, b.pose.a);
    b.push(MoveKind::Linking, end, None, Some(common.plunge_feed))
}
