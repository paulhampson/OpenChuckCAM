// SPDX-License-Identifier: AGPL-3.0-or-later
//! The path fitting the wrap-family strategies share (OP-04; decision record
//! 0018 item 12, decision 0034 item 2).
//!
//! A strategy describes a curve in the development as X and A as functions of
//! a parameter `t` from 0 to 1 (a [`Fit`]): a helix turns A as X advances; an
//! axial pass holds A and moves X. [`fit_curve`] drops the tool along it,
//! splits it where a region constraint forbids cutting or the tool is over no
//! material, and bisects each move until the straight machine move is within
//! [`CHORD_FACTOR`] times the toolpath tolerance of the dropped tool.

use openchuck_types::geometry::Point3;
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::{Degrees, Millimetres};

use crate::candidates::{DevIndex, DevLine};
use crate::cutter::{self, Contact, ToolLine};
use crate::error::{PathError, Result};
use crate::parallel;
use crate::region::RegionConstraint;
use crate::rotary::AxisPosition;
use crate::strategy::{CutPoint, LeaveZone, Pass, StrategyInput};
use crate::surface::SurfaceSource;

/// The fraction of the toolpath tolerance a move's path-fitting error may
/// use (decision record 0018 item 12, closing OI-08): 0.4 leaves room for the
/// motion between points, which SP-3 measured at 0.0056 mm for a factor of
/// 0.5 against the 0.005 mm path-fitting budget of verification plan 4.1.
pub const CHORD_FACTOR: f64 = 0.4;

/// How many times a move may be halved (the smallest step is the seed step
/// divided by 2^10).
const MAX_DEPTH: u32 = 10;

/// How many times the limit of the allowed region is bisected (the seed step
/// divided by 2^12).
const EDGE_STEPS: u32 = 12;

/// Fits the curve of `fit` with `seeds` intervals into passes. `label` names
/// the strategy in progress reports.
pub(crate) fn fit_curve(
    fit: &Fit<'_>,
    seeds: usize,
    label: &str,
    context: &JobContext<'_>,
) -> Result<Vec<Pass>> {
    context.report(
        &format!("{label}: dropping the tool"),
        0,
        Some(seeds as u64),
    );
    let ts: Vec<f64> = (0..=seeds).map(|i| i as f64 / seeds as f64).collect();
    let nodes: Vec<Option<Node>> = parallel::map_in_order(&ts, |t| fit.node(*t));
    context.check_cancelled()?;

    // Runs of consecutive nodes where the tool may cut. With a region
    // constraint a run reaches out to where the tool stops being allowed
    // to cut, not just to its last seed.
    let mut pieces: Vec<Vec<Node>> = Vec::new();
    let mut current: Vec<Node> = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        match node {
            Some(n) => {
                if current.is_empty()
                    && fit.constrained
                    && let Some(previous) = i.checked_sub(1).and_then(|p| ts.get(p))
                    && let Some(edge) = fit.edge(n, *previous)
                {
                    current.push(edge);
                }
                current.push(*n);
            }
            None => {
                if let Some(last) = current.last().copied() {
                    if fit.constrained
                        && let Some(next) = ts.get(i)
                        && let Some(edge) = fit.edge(&last, *next)
                    {
                        current.push(edge);
                    }
                    pieces.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        pieces.push(current);
    }

    context.report(
        &format!("{label}: fitting the path"),
        0,
        Some(pieces.len() as u64),
    );
    let mut passes = Vec::new();
    for (k, piece) in pieces.iter().enumerate() {
        context.check_cancelled()?;
        let thinned;
        let piece = if fit.thin {
            thinned = fit.thin(piece);
            &thinned
        } else {
            piece
        };
        let intervals: Vec<(&Node, &Node)> = piece.iter().zip(piece.iter().skip(1)).collect();
        let refined: Vec<Vec<Option<Node>>> = parallel::map_in_order(&intervals, |(a, b)| {
            let mut out = Vec::new();
            fit.refine(a, b, 0, &mut out);
            out
        });
        // A `None` marks a gap where the tool may not cut: the pass ends
        // there and the next one starts after it.
        let mut points = Vec::new();
        if let Some(first) = piece.first() {
            points.push(fit.cut_point(first));
        }
        for step in refined.iter().flatten() {
            match step {
                Some(n) => points.push(fit.cut_point(n)),
                None => {
                    if !points.is_empty() {
                        passes.push(Pass {
                            points: std::mem::take(&mut points),
                            stock: None,
                        });
                    }
                }
            }
        }
        if !points.is_empty() {
            passes.push(Pass {
                points,
                stock: None,
            });
        }
        context.report(
            &format!("{label}: fitting the path"),
            k as u64 + 1,
            Some(pieces.len() as u64),
        );
    }
    Ok(passes)
}

/// A point of the helix with the tool dropped on the surface there.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Node {
    /// Position along the helix, 0 at the start and 1 at the end.
    pub(crate) t: f64,
    pub(crate) a: Degrees,
    /// The drop with the (grown) tool.
    pub(crate) contact: Contact,
    /// The stock to leave the drop was made with.
    pub(crate) leave: f64,
}

/// A part of the surface with its own stock to leave, and the tool grown by
/// it (decision record 0034 item 4).
pub(crate) struct ZoneFit<'a> {
    pub(crate) surface: &'a dyn SurfaceSource,
    pub(crate) leave: f64,
    pub(crate) profile: RevolvedProfile,
}

/// Grows the tool for each of `zones`.
pub(crate) fn zone_fits<'a>(
    zones: &[LeaveZone<'a>],
    tool: &RevolvedProfile,
) -> Result<Vec<ZoneFit<'a>>> {
    zones
        .iter()
        .map(|z| {
            Ok(ZoneFit {
                surface: z.surface,
                leave: z.leave,
                profile: cutter::grow_profile(tool, z.leave)?,
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(crate) struct Fit<'a> {
    pub(crate) input: &'a StrategyInput<'a>,
    pub(crate) region: Option<&'a RegionConstraint<'a>>,
    pub(crate) drop_profile: &'a RevolvedProfile,
    pub(crate) leave: f64,
    /// Parts of the surface with their own stock to leave. Empty means the
    /// whole surface with `leave`. Otherwise the tool rides the highest of
    /// the zones' drops, so every part is left its own amount.
    pub(crate) zones: &'a [ZoneFit<'a>],
    pub(crate) x0: f64,
    pub(crate) length: f64,
    pub(crate) start: f64,
    pub(crate) sign: f64,
    pub(crate) total_angle: f64,
    pub(crate) chord: f64,
    /// Whether a region constraint limits where the tool may cut (OP-02).
    pub(crate) constrained: bool,
    /// The selected region in the development, when it is known: the tool is
    /// only dropped where it can reach the region ([`crate::candidates`]).
    pub(crate) candidates: Option<&'a DevIndex>,
    /// How far apart, in mm of path, points are inside those intervals.
    pub(crate) spacing: f64,
    /// Keep only the points the chord limit needs from the seeds of a run
    /// (set where the seeds are much finer than the path needs).
    pub(crate) thin: bool,
    /// The most the parameter may advance between two points of a thinned run:
    /// the strategy's own largest step (angle or X), so thinning only removes
    /// the extra points that finer seeding added. Infinite for no limit.
    pub(crate) max_dt: f64,
    /// Where the tool is wanted: when given, a drop whose contact it rejects
    /// is not cut. Rest machining uses it to cut only where material is left
    /// (decision record 0041 item 3). The caller also sets `constrained`, so
    /// the limits of the wanted places are found by bisection.
    pub(crate) wanted: Option<&'a (dyn Fn(Point3) -> bool + Send + Sync)>,
}

impl Fit<'_> {
    /// The path in the development, if the region is known.
    fn dev_line(&self, index: &DevIndex) -> DevLine {
        let r = index.radius();
        DevLine {
            x0: self.x0,
            dx: self.length,
            u0: r * self.start.to_radians(),
            du: r * (self.sign * self.total_angle).to_radians(),
        }
    }

    /// The part of this path from parameter `a` to `b`, as a path of its own.
    fn between(&self, a: f64, b: f64) -> Self {
        Self {
            x0: self.x0 + self.length * a,
            length: self.length * (b - a),
            start: self.start + self.sign * self.total_angle * a,
            total_angle: self.total_angle * (b - a),
            max_dt: self.max_dt / (b - a),
            ..*self
        }
    }

    /// The length of this path in mm at the development's radius.
    fn path_length(&self, index: &DevIndex) -> f64 {
        let r = index.radius();
        self.length
            .hypot(r * (self.sign * self.total_angle).to_radians())
            .abs()
    }

    fn x_at(&self, t: f64) -> f64 {
        self.x0 + self.length * t
    }

    fn a_at(&self, t: f64) -> Degrees {
        Degrees(self.start + self.sign * self.total_angle * t)
    }

    /// The tool dropped at parameter `t`, or `None` where it is over no
    /// material.
    pub(crate) fn node(&self, t: f64) -> Option<Node> {
        let x = self.x_at(t);
        let a = self.a_at(t);
        let line = self
            .input
            .rotary
            .tool_line(Millimetres(x), a, Millimetres(0.0));
        let (contact, leave, profile) = self.drop(&line)?;
        if let Some(region) = self.region
            && !region.allows(contact.contact, contact.tip, profile, &line)
        {
            return None;
        }
        if let Some(wanted) = self.wanted
            && !wanted(contact.contact)
        {
            return None;
        }
        Some(Node {
            t,
            a,
            contact,
            leave,
        })
    }

    /// The drop onto the surface: the whole surface with the operation's
    /// stock to leave, or the highest drop of the zones, each with its own.
    /// "Highest" compares the original tool's tip, the grown tip plus the
    /// stock to leave along the axis.
    fn drop(&self, line: &ToolLine) -> Option<(Contact, f64, &RevolvedProfile)> {
        if self.zones.is_empty() {
            let c = self.input.surface.drop_tool(self.drop_profile, line)?;
            return Some((c, self.leave, self.drop_profile));
        }
        let mut best: Option<(Contact, f64, &RevolvedProfile)> = None;
        for zone in self.zones {
            if let Some(c) = zone.surface.drop_tool(&zone.profile, line)
                && best.is_none_or(|(b, l, _)| c.offset + zone.leave > b.offset + l)
            {
                best = Some((c, zone.leave, &zone.profile));
            }
        }
        best
    }

    /// The original tool's tip for a grown-tool drop: it lies `leave` above
    /// the grown tip along the tool axis.
    fn tip(&self, n: &Node) -> Point3 {
        let axis = self.input.rotary.tool_axis(n.a);
        n.contact.tip + axis.get() * n.leave
    }

    /// Where the pass makes a cut.
    pub(crate) fn cut_point(&self, n: &Node) -> CutPoint {
        let tip = self.tip(n);
        let axis = self.input.rotary.tool_axis(n.a);
        let contact = if n.leave > 0.0 {
            cutter::cutter_contact(self.input.tool, tip, axis, n.contact.contact)
        } else {
            n.contact.contact
        };
        CutPoint {
            tip,
            a: n.a,
            contact,
        }
    }

    /// The error of the straight machine move from `a` to `b` at its middle,
    /// along the surface normal at the dropped contact.
    fn deviation(&self, a: &Node, b: &Node, mid: &Node) -> f64 {
        let rotary = &self.input.rotary;
        let (pa, pb) = (
            rotary.to_axis(self.tip(a), a.a),
            rotary.to_axis(self.tip(b), b.a),
        );
        let lin = AxisPosition {
            along: 0.5 * (pa.along + pb.along),
            across: 0.5 * (pa.across + pb.across),
            height: 0.5 * (pa.height + pb.height),
        };
        let lin_tip = rotary.to_model(lin, mid.a);
        let ideal = self.tip(mid);
        let delta = ideal - lin_tip;
        match self.input.surface.project(mid.contact.contact) {
            Some(sp) => delta.dot(sp.normal.get()).abs(),
            None => delta.length(),
        }
    }

    /// As [`deviation`](Self::deviation) for a node `mid` anywhere between `a`
    /// and `b`, not only at their middle. A is linear in the parameter, so
    /// the machine's straight move reaches the same fraction of every axis.
    fn deviation_between(&self, a: &Node, b: &Node, mid: &Node) -> f64 {
        let rotary = &self.input.rotary;
        let span = b.t - a.t;
        let frac = if span > 0.0 {
            (mid.t - a.t) / span
        } else {
            0.5
        };
        let (pa, pb) = (
            rotary.to_axis(self.tip(a), a.a),
            rotary.to_axis(self.tip(b), b.a),
        );
        let lin = AxisPosition {
            along: pa.along + frac * (pb.along - pa.along),
            across: pa.across + frac * (pb.across - pa.across),
            height: pa.height + frac * (pb.height - pa.height),
        };
        let lin_tip = rotary.to_model(lin, mid.a);
        let delta = self.tip(mid) - lin_tip;
        match self.input.surface.project(mid.contact.contact) {
            Some(sp) => delta.dot(sp.normal.get()).abs(),
            None => delta.length(),
        }
    }

    /// The fewest of `nodes` (a run where the tool may cut, closely seeded)
    /// that keeps every node within the chord limit of the straight machine
    /// moves between those kept, so a finely seeded run does not make a
    /// point per seed. The ends are always kept; the move between two kept
    /// nodes is refined further afterwards like any other.
    pub(crate) fn thin(&self, nodes: &[Node]) -> Vec<Node> {
        let last = nodes.len().saturating_sub(1);
        let Some(first) = nodes.first() else {
            return Vec::new();
        };
        let mut kept = vec![*first];
        let mut start = 0;
        // Whether the move from `start` to `end` keeps every seed between.
        let holds = |start: usize, end: usize| {
            let (Some(a), Some(b)) = (nodes.get(start), nodes.get(end)) else {
                return false;
            };
            if end == start + 1 {
                return true;
            }
            if b.t - a.t > self.max_dt * (1.0 + 1e-9) {
                return false;
            }
            nodes.get(start + 1..end).is_some_and(|between| {
                between
                    .iter()
                    .all(|mid| self.deviation_between(a, b, mid) <= self.chord)
            })
        };
        while start < last {
            // Gallop out while the move holds, then bisect the last step.
            let (mut good, mut bad) = (1_usize, None);
            let mut step = 2_usize;
            loop {
                let end = start + step;
                if end >= last {
                    if holds(start, last) {
                        good = last - start;
                    } else {
                        bad = Some(last - start);
                    }
                    break;
                }
                if holds(start, end) {
                    good = step;
                    step *= 2;
                } else {
                    bad = Some(step);
                    break;
                }
            }
            if let Some(mut hi) = bad {
                let mut lo = good;
                while hi - lo > 1 {
                    let mid = (lo + hi) / 2;
                    if holds(start, start + mid) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                good = lo;
            }
            start += good;
            if let Some(n) = nodes.get(start) {
                kept.push(*n);
            }
        }
        kept
    }

    /// The node nearest to parameter `bad_t` where the tool may still cut,
    /// found by bisection from `from`, which may cut, towards `bad_t`, where
    /// it may not. `None` when the limit is already at `from`.
    pub(crate) fn edge(&self, from: &Node, bad_t: f64) -> Option<Node> {
        let (mut good, mut bad) = (from.t, bad_t);
        let mut best = None;
        for _ in 0..EDGE_STEPS {
            let mid = 0.5 * (good + bad);
            match self.node(mid) {
                Some(n) => {
                    good = mid;
                    best = Some(n);
                }
                None => bad = mid,
            }
        }
        best
    }

    /// Bisects the move `a` to `b` until the straight machine path is within
    /// the chord tolerance at its middle; pushes the points after `a`, ending
    /// with `b`. Where the middle has no material under it (the surface falls
    /// away between two points that touch it) the move is accepted as it is.
    /// Where a region constraint forbids cutting at the middle, the move is
    /// cut short at the limits on each side and a `None` marks the gap.
    pub(crate) fn refine(&self, a: &Node, b: &Node, depth: u32, out: &mut Vec<Option<Node>>) {
        if depth < MAX_DEPTH {
            let t = 0.5 * (a.t + b.t);
            match self.node(t) {
                Some(mid) if self.deviation(a, b, &mid) > self.chord => {
                    self.refine(a, &mid, depth + 1, out);
                    self.refine(&mid, b, depth + 1, out);
                    return;
                }
                None if self.constrained => {
                    if let Some(left) = self.edge(a, t) {
                        self.refine(a, &left, depth + 1, out);
                    }
                    out.push(None);
                    match self.edge(b, t) {
                        Some(right) => {
                            out.push(Some(right));
                            self.refine(&right, b, depth + 1, out);
                        }
                        None => out.push(Some(*b)),
                    }
                    return;
                }
                _ => {}
            }
        }
        out.push(Some(*b));
    }
}

/// As [`fit_curve`] for a path limited to a region: when the region is known
/// in the development, only the intervals of the path that can reach it are
/// fitted, each with points `fit.spacing` apart, and nothing is dropped
/// elsewhere. Otherwise `seeds` evenly spaced points cover the whole path.
pub(crate) fn fit_region(
    fit: &Fit<'_>,
    seeds: usize,
    label: &str,
    context: &JobContext<'_>,
) -> Result<Vec<Pass>> {
    let Some(index) = fit.candidates.filter(|_| fit.constrained) else {
        return fit_curve(fit, seeds, label, context);
    };
    let intervals = index.intervals(&fit.dev_line(index));
    let mut parts = Vec::new();
    let mut total = 0.0;
    for (a, b) in intervals {
        let part = fit.between(a, b);
        let n = (part.path_length(index) / fit.spacing).ceil().max(2.0);
        total += n;
        parts.push((part, n as usize));
    }
    if total > MAX_POINTS {
        return Err(PathError::Strategy(format!(
            "the region needs {total:.0} points at a spacing of {} mm; increase the tolerance",
            fit.spacing
        )));
    }
    let mut out = Vec::new();
    for (part, n) in parts {
        context.check_cancelled()?;
        out.extend(fit_curve(&Fit { thin: true, ..part }, n, label, context)?);
    }
    Ok(out)
}

/// Most points one pass may make inside a region (NFR-07).
const MAX_POINTS: f64 = 5.0e6;

/// The selected region of `region` in the development at `radius`, for a tool
/// that reaches `reach` from its axis with the largest stock to leave, or
/// `None` when there is no selected region or it cannot be developed (the
/// caller then samples the whole path).
pub(crate) fn region_index(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    radius: f64,
    leave: f64,
    tolerance: f64,
) -> Option<DevIndex> {
    let surface = region?.region?;
    let reach = input.tool.cutting_radius() + leave + tolerance + crate::region::ON_SURFACE;
    DevIndex::build(surface.tessellation(), &input.rotary, radius, reach)
}

/// How far apart, in mm of path, points are inside the intervals that can
/// reach a region: the toolpath tolerance, between 0.002 mm and the old
/// whole-path spacing.
pub(crate) fn region_spacing(tolerance: f64, whole_path: f64) -> f64 {
    tolerance.clamp(0.002, whole_path)
}
