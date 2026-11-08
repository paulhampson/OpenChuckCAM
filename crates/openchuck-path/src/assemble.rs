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
//!    end of the group A retains its accumulated position after safe retract.
//!    [`crate::program`] connects operations without a canonical return
//!    (decision 0241); controller counter resets are not physical moves.
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
//!    programmed and an information [`Notice`] says so. Moves without a contact
//!    (rapids, plunges, retracts) use the tool tip for the length.

mod entry;
mod pass_shape;
mod path_ramp;
mod ring_ramp;
mod walls;

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, Segment, ToolPose, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

use crate::error::{PathError, Result};
use crate::notice::{Notice, topic};
use crate::params::{CommonParameters, EntryParameters, Retract};
use crate::rotary::{AxisPosition, RotaryMapping, fit_range, unwrap_continuous};
use crate::stock::{CollisionOracle, StockModel};
use crate::strategy::{CutPoint, Pass, PassStock, StrategyInput};
use openchuck_types::machine::{MaxFeeds, RotaryInFeed};

/// An area of the part a strategy could not reach with the tool (OP-20;
/// decision record 0368): the path leaves it uncut, and a warning says so.
#[derive(Debug, Clone, PartialEq)]
pub struct Unreached {
    /// The X range it spans, `(low, high)`, in millimetres.
    pub x: (f64, f64),
    /// The A range at which its surface faces the tool, `(low, high)`, in
    /// degrees; a range of 360 or more goes all round.
    pub a: (f64, f64),
    /// Its area, in square millimetres.
    pub area: f64,
    /// Why the tool cannot reach it.
    pub reason: UnreachedReason,
    /// The points of the part's surface in it that the tool cannot reach,
    /// as the strategy sampled them, in its order: so a viewer marks those
    /// surfaces and not the reached ones in the same X and A range (issue
    /// #416, decision record 0515). At most [`MAX_UNREACHED_POINTS`], taken
    /// evenly through the area's samples when it has more.
    pub points: Vec<Point3>,
}

/// The most surface points an [`Unreached`] area carries (decision record
/// 0515): enough to find every triangle of the area on a fine mesh, few
/// enough that a large area costs little memory.
pub const MAX_UNREACHED_POINTS: usize = 4096;

/// Why a tool cannot reach an area of the part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnreachedReason {
    /// The part is in the way in every direction the tool's axis can take
    /// across the rotary axis: the tool would have to lean along it.
    Blocked,
    /// A direction is clear, but the tool is too wide for the gap, or its
    /// flutes too short or its shank too wide for the depth.
    TooNarrow,
    /// The tool reaches it only leaning further from the surface normal
    /// than the operation allows.
    NeedsLean {
        /// The least lean that reaches it, in degrees.
        degrees: f64,
    },
    /// The tool could reach it only by coming over the gripped length of
    /// the stock, which every tool keeps clear of by its radius (SET-09;
    /// decision records 0161 and 0505).
    NearHeldRegion,
}

/// A place where a strategy broke a pass because the tool's shank, wider
/// than its cutting part, would meet the part there (OP-12, OP-20; decision
/// record 0368): the pass is linked round it, and a warning says so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShankBreak {
    /// Its X, in millimetres.
    pub x: f64,
    /// A there, in degrees.
    pub a: f64,
}

/// A toolpath group with notes about it.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedPath {
    /// The moves.
    pub group: ToolpathGroup,
    /// What the user should know about the path: its warnings first, then
    /// its information, each with its own level (decision record 0536 item
    /// 6.1). A warning is said once; a host that shows every notice shows
    /// it among them.
    pub notices: Vec<Notice>,
    /// The areas the strategy could not reach, as it gave them (OP-20;
    /// decision record 0368).
    pub unreached: Vec<Unreached>,
    /// The places the strategy broke a pass for the tool's shank, as it gave
    /// them (decision record 0368).
    pub shank_breaks: Vec<ShankBreak>,
}

const EPS: f64 = 1e-9;

impl GeneratedPath {
    /// Its warnings, in order: the notices a host lists in Messages
    /// (decision record 0536 section 5).
    pub fn warnings(&self) -> Vec<&Notice> {
        self.notices.iter().filter(|n| n.is_warning()).collect()
    }
}

/// How much longer, in millimetres, positions written to three decimals can
/// make a move: half a unit in each of two end points on three axes.
const WRITTEN_ALLOWANCE: f64 = 0.002;

/// The last written decimal of a position, in millimetres: two positions
/// closer than this may be written the same.
const WRITTEN_UNIT: f64 = 0.001;

struct Builder<'a> {
    rotary: RotaryMapping,
    /// The tool's cutting radius, for probing under a link.
    tool_radius: f64,
    /// The tool's profile, for the cutter-surface check of a finishing pass.
    profile: &'a openchuck_types::tool::profile::RevolvedProfile,
    max_feeds: MaxFeeds,
    collision: &'a dyn CollisionOracle,
    pose: ToolPose,
    contact: Point3,
    segments: Vec<Segment>,
    notes: Vec<Notice>,
    /// Entries that rose above the link's end before moving to their start
    /// (issue #642; decision 0544 item 6).
    raised_entries: usize,
    /// Passes whose ramp followed a drop of the pass steeper than the ramp
    /// angle just before its first point (decision 0543 D4).
    steep_ramps: Vec<usize>,
    /// Single-point passes plunged under a ramp (decision 0543 D7).
    single_point_plunges: Vec<usize>,
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
        // A controller that counts A's travel round the part in the feed word
        // holds that length, not just the line, to the line's limit (decision
        // record 0076).
        let counted = if line > EPS && kind.is_fed() {
            (line + allowance).max(rotary_in_feed(
                limits.rotary_in_feed,
                da,
                to.across.hypot(to.height),
            ))
        } else {
            line + allowance
        };
        // A move that changes X or Y by less than a written unit may be
        // written with or without that change, so it is checked against
        // either limit: it is timed for the lower (decision record 0050).
        let line_limit = if dx >= WRITTEN_UNIT || dy >= WRITTEN_UNIT {
            linear
        } else if dx > EPS || dy > EPS {
            linear.min(z)
        } else {
            z
        };
        let axis_time = 60.0
            * [
                dx / linear,
                dy / linear,
                dz / z,
                da / rotary,
                if line > EPS {
                    counted / line_limit
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
                    let contact_feed = 60.0 * length / axis_time;
                    // A turn of A with the contact point at the rotary axis
                    // barely moves it: the feed there rounds to zero and says
                    // nothing, so the note says what the move is (issue
                    // #600). It is not a feed-limit note to summarise.
                    let (topic, text) = if contact_feed < 0.05 {
                        (
                            topic::ROTARY_AXIS,
                            format!(
                                "A turns with the contact point at the rotary axis during this \
                             move, so the contact point barely moves and the rotary axis's speed \
                             limit sets the time taken ({axis_time:.3} s); the feed set is \
                             {:.1} mm/min",
                                f.0
                            ),
                        )
                    } else {
                        (
                            topic::FEED_LIMIT,
                            format!(
                                "the machine's feed limits slow this move: the contact feed is \
                             {contact_feed:.1} mm/min, not {:.1}",
                                f.0
                            ),
                        )
                    };
                    self.notes.push(
                        Notice::information(text)
                            .about(topic)
                            .at_segment(self.segments.len()),
                    );
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

/// How much a change of A can grow when written to three decimals: half a
/// unit at each end.
const WRITTEN_A_ALLOWANCE: f64 = 0.001;

/// How much the end point's distance from the rotary axis can grow when its
/// two coordinates across the axis are written to three decimals.
const WRITTEN_RADIUS_ALLOWANCE: f64 = 0.001;

/// The length, in millimetres, that a controller counts for a change of A of
/// `da` degrees ending `radius` from the rotary axis, with the written
/// rounding allowed for; 0 where A is not counted with the line (decision
/// record 0076). The Makera Z1 counts `da * (2 pi r + 30) / 360`, with `r` at
/// least 1 mm (decision record 0007).
fn rotary_in_feed(rule: RotaryInFeed, da: f64, radius: f64) -> f64 {
    match rule {
        RotaryInFeed::Makera if da > EPS => {
            let r = (radius + WRITTEN_RADIUS_ALLOWANCE).max(1.0);
            (da + WRITTEN_A_ALLOWANCE) * (std::f64::consts::TAU * r + 30.0) / 360.0
        }
        _ => 0.0,
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

    let tolerance = common.tolerance.get().0;
    let chord = crate::wrap_fit::CHORD_FACTOR * tolerance;

    // A-axis management: continuous within a pass, fitted by whole turns.
    let mut placed: Vec<Vec<CutPoint>> = Vec::new();
    let mut stocks: Vec<Option<PassStock>> = Vec::new();
    let mut rapids: Vec<Vec<usize>> = Vec::new();
    let mut helical_starts = Vec::new();
    let mut low_links = Vec::new();
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
        // The micro-moves path fitting leaves at a wall become one move
        // (decision record 0050). A pass with rapid points (a drilled hole)
        // is left as it is: its rapids index its points, and its pecks run
        // along the tool axis on purpose.
        let points = if pass.rapids.is_empty() {
            walls::merge_at_walls(&rotary, &points, tolerance, chord)
        } else {
            points
        };
        if let Some(last) = points.last() {
            previous = last.a;
        }
        placed.push(points);
        stocks.push(pass.stock.clone());
        rapids.push(pass.rapids.clone());
        helical_starts.push(pass.helical_pass_start);
        low_links.push(pass.low_link);
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
        tool_radius: input.tool.cutting_radius(),
        profile: input.tool,
        max_feeds: input.max_feeds,
        collision: input.collision,
        pose: start,
        contact: start_tip,
        segments: Vec::new(),
        notes: Vec::new(),
        raised_entries: 0,
        steep_ramps: Vec::new(),
        single_point_plunges: Vec::new(),
    };

    let entry = entry::Entry::new(input, common, entry)?;
    let total = placed.len() as u64;
    let (mut asked_low, mut kept_high) = (0_usize, 0_usize);
    let mut rotary_reset_after = Vec::new();
    for (i, points) in placed.iter().enumerate() {
        context.check_cancelled()?;
        context.report("assemble passes", i as u64, Some(total));
        let Some(head) = points.first() else {
            continue;
        };
        let stock = view(&stocks, i, input);
        let pass_rapids = rapids.get(i).map_or(&[][..], Vec::as_slice);
        // Where the entry starts, when it is not over the first cut point
        // (a ramp round a ring, decision 0543): the link goes there.
        let entry_start = entry.start(&b, input, stock, common, i, points, pass_rapids)?;
        let head = entry_start.as_ref().unwrap_or(head);
        if i > 0 {
            // A pass that tracks its stock may be linked at a height just
            // above that stock, where the whole link is clear of it, instead
            // of at the clearance cylinder (OP-14, decision record 0033).
            let local = match stocks.get(i).and_then(Option::as_ref) {
                Some(_) => local_link_height(&b, stock.stock, common, head, link_height),
                // A pass without its own stock that asks for low links is
                // linked against the setup's stock, and only where no held
                // region lies in the way (decision 0496).
                None if low_links.get(i).copied().unwrap_or(false) => {
                    asked_low += 1;
                    let low = local_link_height(&b, input.stock, common, head, link_height)
                        .filter(|_| !crosses_keep_out(input.keep_out, &b, head));
                    if low.is_none() {
                        kept_high += 1;
                    }
                    low
                }
                _ => None,
            };
            let height = local.unwrap_or(link_height);
            let link = |b: &mut Builder<'_>, resets: &mut Vec<usize>, height: f64| -> Result<()> {
                leave(b, view(&stocks, i - 1, input), common, height)?;
                b.retract(height)?;
                if helical_starts.get(i).copied().unwrap_or(false)
                    && rotary.range == openchuck_types::machine::ARange::Continuous
                    && let Some((index, segment)) = b.segments.iter().enumerate().next_back()
                {
                    let before = index
                        .checked_sub(1)
                        .and_then(|s| b.segments.get(s))
                        .map_or(start, |s| s.end);
                    // A pass already at clearance may need no retract. Preserve
                    // its valid motion without inventing a reset boundary. A
                    // pass that adds no motion leaves the same segment last, so
                    // it is recorded once and the list stays strictly increasing.
                    if matches!(segment.kind, MoveKind::Rapid | MoveKind::Linking)
                        && segment.end.a == before.a
                        && resets.last() != Some(&index)
                    {
                        resets.push(index);
                    }
                }
                let target = rotary.to_axis(head.tip, head.a);
                let at = b.at().height.max(height);
                let tip = rotary.to_model(
                    AxisPosition {
                        height: at,
                        ..target
                    },
                    head.a,
                );
                let end = b.pose(tip, head.a);
                b.push(MoveKind::Rapid, end, None, None)
            };
            // A low link the collision oracle refuses goes up to the retract
            // height instead, and is counted with those that could not be
            // lowered (COL-08, NFR-12; CodeRabbit on #459).
            let lowered_here = local.is_some() && stocks.get(i).and_then(Option::as_ref).is_none();
            let mark = (
                b.segments.len(),
                b.notes.len(),
                b.pose,
                b.contact,
                rotary_reset_after.len(),
            );
            let mut linked = link(&mut b, &mut rotary_reset_after, height);
            if linked.is_err() && lowered_here {
                b.segments.truncate(mark.0);
                b.notes.truncate(mark.1);
                b.pose = mark.2;
                b.contact = mark.3;
                rotary_reset_after.truncate(mark.4);
                kept_high += 1;
                linked = link(&mut b, &mut rotary_reset_after, link_height);
            }
            linked?;
        }
        entry.enter(&mut b, input, stock, common, i, points, pass_rapids)?;
        let doubling = rapids.get(i).filter(|r| !r.is_empty());
        let contacts = match doubling {
            Some(_) => points.iter().map(|p| p.contact).collect(),
            None => {
                let breaks = walls::wall_breaks(&rotary, points, tolerance);
                smoothed_contacts_between(&rotary, points, &breaks)
            }
        };
        b.contact = contacts.first().copied().unwrap_or(head.contact);
        for (k, (p, c)) in points.iter().zip(&contacts).enumerate().skip(1) {
            let end = b.pose(p.tip, p.a);
            if doubling.is_some_and(|r| r.contains(&k)) {
                b.push(MoveKind::Rapid, end, None, None)?;
            } else {
                b.push(MoveKind::Cutting, end, Some(*c), Some(common.feed))?;
            }
        }
        entry.exit(&mut b, common, input, i, points)?;
    }

    leave(
        &mut b,
        view(&stocks, stocks.len().saturating_sub(1), input),
        common,
        safe_height,
    )?;
    b.retract(safe_height)?;
    let list = |passes: &[usize]| {
        passes
            .iter()
            .map(|p| (p + 1).to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !b.single_point_plunges.is_empty() {
        let text = format!(
            "{} with no path for a ramp to follow, so {} entered by a straight plunge, as \
             Allow single-point plunge asks",
            if b.single_point_plunges.len() == 1 {
                format!("pass {} is a single point", list(&b.single_point_plunges))
            } else {
                format!("passes {} are single points", list(&b.single_point_plunges))
            },
            if b.single_point_plunges.len() == 1 {
                "it is"
            } else {
                "they are"
            }
        );
        b.notes
            .insert(0, Notice::information(text).about(topic::RAMP_ENTRY));
    }
    if !b.steep_ramps.is_empty() {
        let text = format!(
            "passes {} drop more steeply than the ramp angle just before their first point, so \
             their ramps follow that drop there, as the passes themselves do",
            list(&b.steep_ramps)
        );
        b.notes
            .insert(0, Notice::information(text).about(topic::RAMP_ENTRY));
    }
    if b.raised_entries > 0 {
        b.notes.insert(
            0,
            Notice::information(format!(
                "{} entries start higher than the link before them ends, so the move to the \
                 entry's start stays clear of the stock beside the first cut point",
                b.raised_entries
            ))
            .about(topic::ENTRY),
        );
    }
    if kept_high > 0 {
        // Said against the first segment, as a strategy's notes are
        // (NFR-07).
        b.notes.insert(
            0,
            Notice::information(format!(
                "{kept_high} of {asked_low} links between passes could not be lowered \
                 (stock, a held region or the retract height leaves no room) and retract \
                 as usual"
            ))
            .about(topic::LINKS),
        );
    }
    context.report("assemble passes", total, Some(total));
    Ok(GeneratedPath {
        group: ToolpathGroup {
            rotary_reset_after,
            tags: input.tags.clone(),
            start,
            segments: b.segments,
        },
        notices: b.notes,
        unreached: Vec::new(),
        shank_breaks: Vec::new(),
    })
}

/// Points either side of a contact averaged with it to measure the contact
/// path (see [`smoothed_contacts`]).
pub const CONTACT_SMOOTHING: usize = 2;

/// The contact points of a pass for measuring its contact path: each the
/// point's own tip plus the mean, over itself and up to
/// [`CONTACT_SMOOTHING`] neighbours either side (fewer at the ends, keeping
/// the window centred), of the contact's offset from the tip, taken in the
/// machine's fixed frame of `rotary`, where the tool stands still and the
/// part turns, so the offset says where on the tool the contact sits. On a
/// mesh the contact jumps from facet to facet by about the tool radius times
/// the facet angle (decision record 0015); averaging the offset removes that
/// jitter from the contact length while a smooth path is left unchanged.
/// The position follows the point's own tip, not the mean of the window's
/// positions, so points spaced unevenly (one that path fitting adds between
/// two seeds) do not shift their neighbours' contacts along the path, which
/// made the short moves beside such a point a third slower than programmed
/// (decision record 0115 item 5; OP-13).
pub fn smoothed_contacts(rotary: &RotaryMapping, points: &[CutPoint]) -> Vec<Point3> {
    smoothed_contacts_between(rotary, points, &[])
}

/// [`smoothed_contacts`] with the window stopped at each index of `breaks`
/// (in increasing order): the points before a break and those from it on are
/// smoothed separately, so a wall step's contact jump is not shared with the
/// moves beside it (decision record 0050).
fn smoothed_contacts_between(
    rotary: &RotaryMapping,
    points: &[CutPoint],
    breaks: &[usize],
) -> Vec<Point3> {
    let mut out = Vec::with_capacity(points.len());
    let mut start = 0;
    for &end in breaks.iter().chain(std::iter::once(&points.len())) {
        if let Some(piece) = points.get(start..end) {
            out.extend(smoothed_piece(rotary, piece));
        }
        start = end;
    }
    out
}

fn smoothed_piece(rotary: &RotaryMapping, points: &[CutPoint]) -> Vec<Point3> {
    let n = points.len();
    let fixed = |p: &CutPoint| {
        let tip = rotary.to_axis(p.tip, p.a);
        let contact = rotary.to_axis(p.contact, p.a);
        (
            tip,
            [
                contact.along - tip.along,
                contact.across - tip.across,
                contact.height - tip.height,
            ],
        )
    };
    (0..n)
        .map(|i| {
            let half = CONTACT_SMOOTHING.min(i).min(n - 1 - i);
            let window = points.get(i - half..=i + half).unwrap_or_default();
            let count = window.len().max(1) as f64;
            let offset = window.iter().fold([0.0; 3], |acc, p| {
                let (_, o) = fixed(p);
                [acc[0] + o[0], acc[1] + o[1], acc[2] + o[2]]
            });
            let Some(own) = points.get(i) else {
                return Point3::ORIGIN;
            };
            let (tip, _) = fixed(own);
            rotary.to_model(
                AxisPosition {
                    along: tip.along + offset[0] / count,
                    across: tip.across + offset[1] / count,
                    height: tip.height + offset[2] / count,
                },
                own.a,
            )
        })
        .collect()
}

/// How many parallel lines [`swept_entry`] casts round the cutter at each of
/// [`SWEPT_RINGS`] radii.
const SWEPT_RAYS: u32 = 32;

/// How many radii out to the cutting radius [`swept_entry`] samples.
const SWEPT_RINGS: u32 = 4;

/// The distance the tool tip, at `tip`, can move along `direction` (the way
/// the tool moves, a unit vector along its axis) before the cutter first meets
/// material, no further than `max_distance`; `None` when it stays clear.
///
/// A line through the tool tip alone can miss stock that the body of the
/// cutter would hit, such as a stock end beside a lead arc outside the part.
/// So lines are cast from the cutter's own surface: at each of
/// [`SWEPT_RINGS`] radii out to the cutting radius, [`SWEPT_RAYS`] parallel
/// lines start at the point of the profile at that radius (its height above
/// the tip) and the least entry counts. A flat tool's profile is flat, so it
/// behaves as a cylinder; a ball is narrow near its tip. The cutter may reach
/// material by at most `radius * (1 - cos(pi / SWEPT_RAYS))` between two
/// lines, 0.007 mm for a 1.5 mm radius.
///
/// **Resolution.** Material is found only if one of these lines meets it, so a
/// stock whose overlap with the cutter's footprint is thinner than the spacing
/// of the lines (about 0.2 times the cutting radius, 0.3 mm for a 1.5 mm
/// radius) can be missed. The stock models are convex solids that contain the
/// part (a cylinder, or the part's bounding box), so such a sliver is only the
/// rim of the footprint, where the miss is the 0.007 mm above. `None` means no
/// line met material, and the caller then descends to the approach distance
/// above the goal; it is not a measured clearance.
pub(crate) fn swept_entry(
    stock: &dyn crate::stock::StockModel,
    profile: &openchuck_types::tool::profile::RevolvedProfile,
    tip: Point3,
    direction: openchuck_types::geometry::UnitVec3,
    max_distance: f64,
) -> Option<f64> {
    let mut best = stock.ray_entry(tip, direction, max_distance);
    let radius = profile.cutting_radius();
    if !(radius.is_finite() && radius > 0.0) {
        return best;
    }
    // Up the tool axis is against the way it moves.
    let up = -direction.get();
    // Two directions across the tool axis: along the rotary axis (model X)
    // and the one perpendicular to both.
    let along = openchuck_types::geometry::Vec3::new(1.0, 0.0, 0.0);
    let across = up.cross(along);
    let len = across.length();
    if len < 1e-9 {
        return best;
    }
    let across = across * (1.0 / len);
    for ring in 1..=SWEPT_RINGS {
        let d = radius * f64::from(ring) / f64::from(SWEPT_RINGS);
        let rise = profile.height_at(d).unwrap_or(0.0);
        for k in 0..SWEPT_RAYS {
            let angle = std::f64::consts::TAU * f64::from(k) / f64::from(SWEPT_RAYS);
            let origin = tip + up * rise + along * (d * angle.cos()) + across * (d * angle.sin());
            if let Some(e) = stock.ray_entry(origin, direction, max_distance) {
                best = Some(best.map_or(e, |b| b.min(e)));
            }
        }
    }
    best
}

/// Leaves the cut along the tool axis at the plunge feed until the tool tip
/// is the approach distance above the stock on that line (the stock model
/// may not know what this operation has cut), so the rapid retract that
/// follows starts in the clear.
fn leave(
    b: &mut Builder<'_>,
    stock: View<'_>,
    common: &CommonParameters,
    height: f64,
) -> Result<()> {
    let here = b.at();
    if here.height >= height - EPS {
        return Ok(());
    }
    let Some(d) = footprint_entry(b, stock, here, b.pose.a, height, height - here.height) else {
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

/// True when the straight link from where the tool is to `head` lies over
/// any of the intervals `keep_out` (SET-09), the held regions widened by
/// the tool's radius, so it is not kept low there.
fn crosses_keep_out(keep_out: &[(f64, f64)], b: &Builder<'_>, head: &CutPoint) -> bool {
    let from = b.at().along;
    let to = b.rotary.to_axis(head.tip, head.a).along;
    let (lo, hi) = (from.min(to), from.max(to));
    keep_out.iter().any(|&(a, c)| lo < c && hi > a)
}

/// The stock a pass sees: the setup's stock, probed along the tool axis only
/// (as finishing always has), or the pass's own stock, probed across the
/// tool's whole width (decision record 0033 item 6).
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    stock: &'a dyn StockModel,
    wide: bool,
}

/// The distance down from `from_height` at which the tool, held at `at` with
/// its axis at angle `a`, first meets `stock` anywhere under its footprint,
/// no further than `travel`: the tool's whole width is probed, not only its
/// axis, so a rapid never clips stock beside the line it comes down on.
fn footprint_entry(
    b: &Builder<'_>,
    stock: View<'_>,
    at: AxisPosition,
    a: Degrees,
    from_height: f64,
    travel: f64,
) -> Option<f64> {
    let down = openchuck_types::geometry::UnitVec3::new(-b.rotary.tool_axis(a).get()).ok()?;
    if !stock.wide {
        // A finishing pass against the setup's stock: the cutter's own
        // surface is probed, not the tool axis alone, so a rapid beside the
        // stock (a lead arc outside the part, next to the stock end) never
        // puts the cutter into it (decision record 0034, as built).
        let tip = b.rotary.to_model(
            AxisPosition {
                height: from_height,
                ..at
            },
            a,
        );
        return swept_entry(stock.stock, b.profile, tip, down, travel);
    }
    let mut nearest: Option<f64> = None;
    let probes = &FOOTPRINT[..];
    for &(u, v) in probes {
        let origin = b.rotary.to_model(
            AxisPosition {
                along: at.along + u * b.tool_radius,
                across: at.across + v * b.tool_radius,
                height: from_height,
            },
            a,
        );
        if let Some(d) = stock.stock.ray_entry(origin, down, travel) {
            nearest = Some(nearest.map_or(d, |n| n.min(d)));
        }
    }
    nearest
}

/// Rises first, when needed, so that a move at the tool's height from where
/// it is to above `to` (at angle `a`) clears `stock` along its whole line
/// under the tool's footprint, by the approach distance, as a link does
/// (OP-14; issue #642, decision 0544 item 6). `ceiling` is the retract
/// height, which is above all the stock; the tool never rises above it.
fn clear_traverse(
    b: &mut Builder<'_>,
    stock: View<'_>,
    to: AxisPosition,
    a: Degrees,
    approach: f64,
    ceiling: f64,
) -> Result<()> {
    let from = b.at();
    let reach = (to.along - from.along).hypot(to.across - from.across);
    let step = 0.5 * b.tool_radius;
    if reach <= EPS || step <= 0.0 {
        return Ok(());
    }
    let probe = ceiling.max(from.height);
    let steps = ((reach / step).ceil() as usize).clamp(1, MAX_LINK_SAMPLES);
    let mut top = f64::NEG_INFINITY;
    for k in 0..=steps {
        let f = k as f64 / steps as f64;
        let at = AxisPosition {
            along: from.along + f * (to.along - from.along),
            across: from.across + f * (to.across - from.across),
            height: probe,
        };
        if let Some(d) = footprint_entry(b, stock, at, a, probe, 2.0 * probe.abs() + 1.0) {
            top = top.max(probe - d);
        }
    }
    let needed = top + approach;
    if needed <= from.height + EPS {
        return Ok(());
    }
    if top >= probe - EPS {
        return Err(PathError::Strategy(format!(
            "Entry: the move to an entry's start would pass through stock that reaches \
             {top:.3} mm from the rotary axis, at or above the retract height of {probe:.3} mm. \
             Raise the setup's clearance, or set Entry to Plunge."
        )));
    }
    b.retract(needed.min(probe))?;
    b.raised_entries += 1;
    Ok(())
}

/// The most samples one local link is checked at.
const MAX_LINK_SAMPLES: usize = 2_000;

/// Offsets across the tool's footprint, as fractions of its radius, at which
/// the stock is probed under each sample of a link.
const FOOTPRINT: [(f64, f64); 9] = [
    (0.0, 0.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.7, 0.7),
    (0.7, -0.7),
    (-0.7, 0.7),
    (-0.7, -0.7),
];

/// The height for a link from where the tool is to the start of the next
/// pass that is clear of `stock` along the whole straight move, when that is
/// lower than the retract height (OP-14). The tool's tip is held the
/// approach distance above the highest stock under its footprint at any
/// sample. `None` when the link needs the retract height anyway.
fn local_link_height(
    b: &Builder<'_>,
    stock: &dyn StockModel,
    common: &CommonParameters,
    head: &CutPoint,
    link_height: f64,
) -> Option<f64> {
    let from = b.at();
    let to = b.rotary.to_axis(head.tip, head.a);
    let radius = 0.5 * b.tool_radius;
    if radius <= 0.0 {
        return None;
    }
    let reach = (to.along - from.along).hypot(to.across - from.across);
    let turn = (head.a.0 - b.pose.a.0).abs().to_radians() * link_height;
    let steps = (((reach + turn) / radius).ceil() as usize).clamp(1, MAX_LINK_SAMPLES);
    let mut top = f64::NEG_INFINITY;
    for k in 0..=steps {
        let f = k as f64 / steps as f64;
        let a = Degrees(b.pose.a.0 + f * (head.a.0 - b.pose.a.0));
        let at = AxisPosition {
            along: from.along + f * (to.along - from.along),
            across: from.across + f * (to.across - from.across),
            height: link_height,
        };
        let wide = View { stock, wide: true };
        if let Some(d) = footprint_entry(b, wide, at, a, link_height, 2.0 * link_height.abs() + 1.0)
        {
            top = top.max(link_height - d);
        }
    }
    let height = top.max(from.height).max(to.height) + common.approach_distance.0;
    (height < link_height - EPS).then_some(height)
}

/// The stock pass `i` sees.
fn view<'a>(stocks: &'a [Option<PassStock>], i: usize, input: &'a StrategyInput<'a>) -> View<'a> {
    match stocks.get(i).and_then(Option::as_ref) {
        Some(s) => View {
            stock: s.0.as_ref(),
            wide: true,
        },
        None => View {
            stock: input.stock,
            wide: false,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::stock::AcceptAll;
    use openchuck_types::machine::MachineProfile;
    use openchuck_types::tool::{Tool, ToolShape};
    use openchuck_types::units::{FeedRate, RotaryFeedRate};

    fn point(x: f64, height: f64, a: f64, contact: Point3) -> CutPoint {
        let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
        CutPoint {
            tip: rotary.to_model(
                AxisPosition {
                    along: x,
                    across: 0.0,
                    height,
                },
                Degrees(a),
            ),
            a: Degrees(a),
            contact,
        }
    }

    #[test]
    fn req_op_13_contact_smoothing_stops_at_a_wall_step() {
        // Three points at the foot of a wall, three on top, the contact
        // jumping 5 mm between them.
        let low = Point3::new(0.0, 0.0, 5.0);
        let high = Point3::new(1.5, 0.0, 10.0);
        let points: Vec<CutPoint> = (0..6)
            .map(|i| {
                let up = i >= 3;
                let c = if up { high } else { low };
                point(f64::from(i) * 0.1, if up { 8.5 } else { 5.0 }, 0.0, c)
            })
            .collect();
        let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
        let across = smoothed_contacts(&rotary, &points);
        let stopped = smoothed_contacts_between(&rotary, &points, &[3]);
        // Smoothed across the wall, the moves beside it share its jump;
        // stopped at it, they keep their own contact and the wall step has
        // the whole jump.
        assert!(across[2].distance(across[1]) > 0.5);
        for (c, expected) in stopped.iter().zip([low, low, low, high, high, high]) {
            assert!(c.distance(expected) < 1e-9, "{c:?} against {expected:?}");
        }
    }

    #[test]
    fn req_op_13_contact_smoothing_follows_each_tip_whatever_the_spacing() {
        // Points round the axis 5 degrees apart with one more at 12.5
        // degrees, as path fitting adds one between two seeds (decision
        // record 0115), each touching the surface 0.3 mm below its tip.
        // Averaging the contact positions would pull the points either side
        // of the odd one 1.5 degrees towards it, and the short moves beside
        // it would carry a contact path 60 % longer than the tip's (OI-331:
        // a third slower than programmed on the groove reference job).
        let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
        let points: Vec<CutPoint> = [0.0, 5.0, 10.0, 12.5, 15.0, 20.0, 25.0]
            .into_iter()
            .map(|a| {
                let p = point(0.0, 10.0, a, Point3::ORIGIN);
                let contact = p.tip + rotary.tool_axis(Degrees(a)).get() * -0.3;
                CutPoint { contact, ..p }
            })
            .collect();
        let smoothed = smoothed_contacts(&rotary, &points);
        for (p, c) in points.iter().zip(&smoothed) {
            assert!(
                c.distance(p.contact) < 1e-9,
                "A {}: smoothed contact {c:?} against {:?}",
                p.a.0,
                p.contact
            );
        }
    }

    #[test]
    fn req_op_13_a_controller_that_counts_a_in_the_feed_word_keeps_it_within_the_limit() {
        // The ball rolls round a corner 18 mm from the axis: A turns 10
        // degrees at its limit while X moves 0.01 mm. The Makera Z1 counts
        // 10 * (2 pi 18 + 30) / 360 = 3.97 mm for that A, so at A's limit
        // (1/6 s) its F word would be 1430 against 1200 (decision record
        // 0076; lane Z found F1426 on the corpus box, OI-236).
        let machine = MachineProfile::example_x_rotary();
        let rotary = RotaryMapping::of_profile(&machine);
        let profile = Tool {
            shape: ToolShape::BallEnd,
            diameter: openchuck_types::units::Millimetres(3.0),
            flute_length: openchuck_types::units::Millimetres(8.0),
            overall_length: openchuck_types::units::Millimetres(40.0),
            shank_diameter: openchuck_types::units::Millimetres(3.0),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        }
        .profile()
        .unwrap();
        let from = point(5.0, 18.0, 0.0, Point3::ORIGIN);
        let to = point(5.01, 18.0, 10.0, Point3::ORIGIN);
        let mut feeds = machine.max_feeds;
        (feeds.linear, feeds.z, feeds.rotary) =
            (FeedRate(1200.0), FeedRate(600.0), RotaryFeedRate(3600.0));
        let time = |rule: RotaryInFeed, from: &CutPoint, to: &CutPoint| {
            let mut b = Builder {
                rotary,
                tool_radius: 1.5,
                profile: &profile,
                max_feeds: MaxFeeds {
                    rotary_in_feed: rule,
                    ..feeds
                },
                collision: &AcceptAll,
                pose: ToolPose {
                    tip: from.tip,
                    axis: rotary.tool_axis(from.a),
                    a: from.a,
                },
                contact: from.tip,
                segments: Vec::new(),
                notes: Vec::new(),
                raised_entries: 0,
                steep_ramps: Vec::new(),
                single_point_plunges: Vec::new(),
            };
            let end = b.pose(to.tip, to.a);
            // A contact feed far above the limits, so the axes set the time.
            b.push(MoveKind::Cutting, end, Some(to.tip), Some(FeedRate(1.0e6)))
                .unwrap();
            b.segments[0].duration.0
        };
        let makera_f = |seconds: f64| {
            let p = std::f64::consts::TAU * 18.0 + 30.0;
            60.0 * (10.0 * p / 360.0) / seconds
        };
        // Counted separately, A's limit sets the time and the Z1's F is over.
        let separate = time(RotaryInFeed::Separate, &from, &to);
        assert!(
            (separate - 60.0 * 10.0 / 3600.0).abs() < 1e-12,
            "{separate}"
        );
        assert!(makera_f(separate) > 1400.0, "F{:.1}", makera_f(separate));
        // Counted as the Z1 does, the move is slowed until it is within.
        let makera = time(RotaryInFeed::Makera, &from, &to);
        let f = makera_f(makera);
        assert!(f <= 1200.0 && f > 1199.0, "F{f:.3}");
        // An A-only move is held to A's own limit, as before: the Z1 counts
        // at most a full degree's millimetre for it.
        let a_only = point(5.0, 18.0, 10.0, Point3::ORIGIN);
        let seconds = time(RotaryInFeed::Makera, &from, &a_only);
        assert!((seconds - separate).abs() < 1e-12, "{seconds}");
    }

    /// Issue #600: a turn of A with the contact point at the rotary axis
    /// barely moves the contact point, so the feed there is about zero. The
    /// note says it is such a turn and that the rotary axis's limit sets its
    /// time, never "the contact feed is 0.0 mm/min", and it is not a
    /// feed-limit note a host would summarise as one.
    #[test]
    fn req_op_13_req_ui_12_a_turn_at_the_rotary_axis_never_reads_as_a_feed_of_zero() {
        let machine = MachineProfile::example_x_rotary();
        let rotary = RotaryMapping::of_profile(&machine);
        let profile = Tool {
            shape: ToolShape::BallEnd,
            diameter: openchuck_types::units::Millimetres(3.0),
            flute_length: openchuck_types::units::Millimetres(8.0),
            overall_length: openchuck_types::units::Millimetres(40.0),
            shank_diameter: openchuck_types::units::Millimetres(3.0),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        }
        .profile()
        .unwrap();
        // The tip on the rotary axis, turning A by 30 degrees.
        let from = point(4.0, 0.0, 0.0, Point3::ORIGIN);
        let to = point(4.0, 0.0, 30.0, Point3::ORIGIN);
        let mut b = Builder {
            rotary,
            tool_radius: 1.5,
            profile: &profile,
            max_feeds: machine.max_feeds,
            collision: &AcceptAll,
            pose: ToolPose {
                tip: from.tip,
                axis: rotary.tool_axis(from.a),
                a: from.a,
            },
            contact: Point3::ORIGIN,
            segments: Vec::new(),
            notes: Vec::new(),
            raised_entries: 0,
            steep_ramps: Vec::new(),
            single_point_plunges: Vec::new(),
        };
        let end = b.pose(to.tip, to.a);
        b.push(
            MoveKind::Cutting,
            end,
            Some(Point3::ORIGIN),
            Some(FeedRate(500.0)),
        )
        .unwrap();
        assert_eq!(b.notes.len(), 1, "{:?}", b.notes);
        let text = &b.notes[0].text;
        assert!(!text.contains(" 0.0 mm/min"), "{text}");
        assert!(
            !text.starts_with("the machine's feed limits slow"),
            "{text}"
        );
        assert!(
            text.contains("A turns with the contact point at the rotary axis during this move"),
            "{text}"
        );
        assert!(text.contains("500.0 mm/min"), "{text}");
        assert!(!b.notes[0].is_warning());
    }

    #[test]
    fn req_op_13_a_move_that_hardly_moves_x_is_timed_against_z() {
        // A climb of 2.2935 mm with X changing by a thousandth of a micron:
        // written to three decimals, X does not change and Z changes by
        // 2.294 mm, which the checker holds to Z's limit.
        let machine = MachineProfile::example_x_rotary();
        let rotary = RotaryMapping::of_profile(&machine);
        let profile = Tool {
            shape: ToolShape::BallEnd,
            diameter: openchuck_types::units::Millimetres(3.0),
            flute_length: openchuck_types::units::Millimetres(8.0),
            overall_length: openchuck_types::units::Millimetres(40.0),
            shank_diameter: openchuck_types::units::Millimetres(3.0),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        }
        .profile()
        .unwrap();
        let from = point(-1.5, 5.0, 0.0, Point3::ORIGIN);
        let to = point(-1.5 + 1e-6, 7.2935, 0.0, Point3::ORIGIN);
        let mut b = Builder {
            rotary,
            tool_radius: 1.5,
            profile: &profile,
            max_feeds: machine.max_feeds,
            collision: &AcceptAll,
            pose: ToolPose {
                tip: from.tip,
                axis: rotary.tool_axis(from.a),
                a: from.a,
            },
            contact: from.tip,
            segments: Vec::new(),
            notes: Vec::new(),
            raised_entries: 0,
            steep_ramps: Vec::new(),
            single_point_plunges: Vec::new(),
        };
        let end = b.pose(to.tip, to.a);
        // A contact feed far above the limits, so the axes set the time.
        b.push(MoveKind::Cutting, end, Some(to.tip), Some(FeedRate(1.0e6)))
            .unwrap();
        let seconds = b.segments[0].duration.0;
        let written_z = 2.294;
        let f = 60.0 * written_z / seconds;
        assert!(f <= machine.max_feeds.z.0, "F{f:.3}");
        // On a machine whose Z is faster than its linear axes, X may be
        // written as changing by a unit, and the linear limit then applies.
        let mut fast_z = machine.max_feeds;
        fast_z.z = FeedRate(2.0 * fast_z.linear.0);
        b.max_feeds = fast_z;
        b.segments.clear();
        b.pose = ToolPose {
            tip: from.tip,
            axis: rotary.tool_axis(from.a),
            a: from.a,
        };
        b.push(MoveKind::Cutting, end, Some(to.tip), Some(FeedRate(1.0e6)))
            .unwrap();
        let f = 60.0 * (written_z.hypot(0.001)) / b.segments[0].duration.0;
        assert!(f <= fast_z.linear.0, "F{f:.3}");
    }
}
