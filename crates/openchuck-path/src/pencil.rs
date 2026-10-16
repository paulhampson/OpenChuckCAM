// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pencil clean-up: one pass along each concave crease of the part, where the
//! tool rests in the corner touching both faces (OP-09; work breakdown slice
//! 15 item 3; decision record 0044 item 5).
//!
//! **Creases** are the edges of the part's mesh between two faces that meet
//! at a concave angle of at least `crease_angle`: the far corner of one
//! triangle lies outside the plane of the other. Edges that join at vertices
//! with no other crease are chained into polylines, open or closed; an edge
//! that is smooth, convex, or shared by other than two triangles is not a
//! crease. A fillet that the mesh has tessellated finely turns less than
//! `crease_angle` at each edge, so it is not pencilled: a crease is a sharp
//! inside corner.
//!
//! **Placement.** Each chain is resampled no further than `max_step` apart.
//! At a sample the tool is dropped on the part with the stock to leave along
//! vertical lines across the crease, and rests at the position where it is
//! lowest and nearest the crease: in a V that is the one place it touches both
//! sides, and against a wall rising from a floor it is the place where it
//! just touches the wall. The tool is only ever placed by a drop, so the pass
//! cannot gouge the part.
//!
//! **Only where material is left.** Unless `min_remaining` is 0, a stretch of
//! the crease is cut only where the remaining-material map of
//! [`crate::rest`] finds material near the tool's contact. A crease that is
//! clean everywhere gives a clear message, not an empty path.

use std::collections::{BTreeMap, BTreeSet};

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::{Degrees, Millimetres};

use crate::cutter::{self, ToolLine};
use crate::error::{PathError, Result};
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::rest::{self, RemainingMap, parameter};
use crate::strategy::{CutPoint, Pass, Strategy, StrategyInput, StrategyOutput};

/// The registry name of [`PencilFinishing`].
pub const NAME: &str = "pencil_finishing";

/// Parameter names specific to pencil clean-up (it shares
/// [`parameter::MIN_REMAINING`] with rest machining).
pub mod pencil_parameter {
    /// The smallest turn between two faces that counts as a crease, degrees.
    pub const CREASE_ANGLE: &str = "crease_angle";
    /// The furthest apart, along the crease, that points are.
    pub const MAX_STEP: &str = "max_step";
}

/// Most points one run may make (NFR-07).
const MAX_POINTS: f64 = 2.0e6;

/// How close, in millimetres, two vertices must be to be the same corner.
const WELD: f64 = 1e-5;

/// How much higher than the lowest the tool may be, in millimetres, and still
/// be said to rest at the crease.
const REST_TOLERANCE: f64 = 1e-5;

/// Pencil clean-up along the concave creases of the whole model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PencilFinishing;

/// A chain of crease edges as a polyline.
#[derive(Debug, Clone, PartialEq)]
pub struct Crease {
    /// The points, in order. A closed crease repeats its first point last.
    pub points: Vec<Point3>,
    /// Whether the crease closes on itself.
    pub closed: bool,
}

/// The concave creases of `mesh` that turn at least `min_angle` degrees,
/// chained into polylines in a deterministic order (by the vertex each starts
/// at). Vertices closer than a hundred-thousandth of a millimetre are one.
pub fn concave_creases(mesh: &TriangleMesh, min_angle: f64) -> Vec<Crease> {
    // Weld the vertices, so a mesh with a vertex per triangle still joins.
    let mut ids: BTreeMap<[i64; 3], u32> = BTreeMap::new();
    let mut welded: Vec<Point3> = Vec::new();
    let mut map: Vec<u32> = Vec::with_capacity(mesh.vertices.len());
    for &p in &mesh.vertices {
        let key = [p.x, p.y, p.z].map(|v| (v / WELD).round() as i64);
        let id = *ids.entry(key).or_insert_with(|| {
            welded.push(p);
            (welded.len() - 1) as u32
        });
        map.push(id);
    }
    let corner = |i: u32| map.get(i as usize).copied();

    struct Face {
        normal: UnitVec3,
        a: Point3,
    }
    let mut faces: Vec<Option<(Face, [u32; 3])>> = Vec::with_capacity(mesh.triangles.len());
    let mut edges: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (t, tri) in mesh.triangles.iter().enumerate() {
        let ids = tri.map(corner);
        let [Some(i), Some(j), Some(k)] = ids else {
            faces.push(None);
            continue;
        };
        let points = [i, j, k].map(|v| welded.get(v as usize).copied());
        let [Some(a), Some(b), Some(c)] = points else {
            faces.push(None);
            continue;
        };
        let normal = (i != j && j != k && i != k)
            .then(|| UnitVec3::new((b - a).cross(c - a)).ok())
            .flatten();
        let Some(normal) = normal else {
            faces.push(None);
            continue;
        };
        for (p, q) in [(i, j), (j, k), (k, i)] {
            edges.entry((p.min(q), p.max(q))).or_default().push(t);
        }
        faces.push(Some((Face { normal, a }, [i, j, k])));
    }

    let cos_min = min_angle.to_radians().cos();
    let mut crease_edges: Vec<(u32, u32)> = Vec::new();
    for (&(p, q), owners) in &edges {
        let [ta, tb] = owners.as_slice() else {
            continue;
        };
        let (Some(Some((fa, _))), Some(Some((fb, ids_b)))) = (faces.get(*ta), faces.get(*tb))
        else {
            continue;
        };
        if fa.normal.get().dot(fb.normal.get()) > cos_min {
            continue;
        }
        // Concave: the corner of B that is not on the edge lies outside A's
        // plane.
        let Some(far) = ids_b
            .iter()
            .find(|v| **v != p && **v != q)
            .and_then(|v| welded.get(*v as usize))
        else {
            continue;
        };
        if (*far - fa.a).dot(fa.normal.get()) > 1e-6 {
            crease_edges.push((p, q));
        }
    }

    // Chain the edges at vertices that have exactly two of them.
    let mut next: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &(p, q) in &crease_edges {
        next.entry(p).or_default().push(q);
        next.entry(q).or_default().push(p);
    }
    let mut used: BTreeSet<(u32, u32)> = BTreeSet::new();
    let key = |p: u32, q: u32| (p.min(q), p.max(q));
    let mut creases = Vec::new();
    let walk = |start: u32, first: u32, used: &mut BTreeSet<(u32, u32)>| -> (Vec<u32>, bool) {
        let mut chain = vec![start];
        let (mut previous, mut here) = (start, first);
        used.insert(key(start, first));
        loop {
            chain.push(here);
            if here == start {
                return (chain, true);
            }
            let onward = next.get(&here).and_then(|n| {
                (n.len() == 2)
                    .then(|| n.iter().copied().find(|v| *v != previous))
                    .flatten()
            });
            match onward {
                Some(v) if used.insert(key(here, v)) => {
                    previous = here;
                    here = v;
                }
                _ => return (chain, false),
            }
        }
    };
    // Open chains begin at a vertex that is not an ordinary part of a chain.
    for (&v, neighbours) in &next {
        if neighbours.len() == 2 {
            continue;
        }
        for &w in neighbours {
            if used.contains(&key(v, w)) {
                continue;
            }
            let (chain, closed) = walk(v, w, &mut used);
            creases.push((chain, closed));
        }
    }
    // What is left are closed loops.
    for &(p, q) in &crease_edges {
        if used.contains(&key(p, q)) {
            continue;
        }
        let (chain, closed) = walk(p, q, &mut used);
        creases.push((chain, closed));
    }
    creases
        .into_iter()
        .map(|(chain, closed)| Crease {
            points: chain
                .iter()
                .filter_map(|v| welded.get(*v as usize).copied())
                .collect(),
            closed,
        })
        .collect()
}

/// `crease` resampled so that no two neighbouring points are further than
/// `max_step` apart, each with the direction of the crease there. A closed
/// crease keeps its first point as its last.
fn resample(crease: &Crease, max_step: f64) -> Vec<(Point3, Vec3)> {
    let mut points = Vec::new();
    for (a, b) in crease.points.iter().zip(crease.points.iter().skip(1)) {
        let n = (a.distance(*b) / max_step).ceil().max(1.0) as usize;
        for i in 0..n {
            let f = i as f64 / n as f64;
            points.push(*a + (*b - *a) * f);
        }
    }
    if let Some(last) = crease.points.last() {
        points.push(*last);
    }
    let count = points.len();
    (0..count)
        .filter_map(|i| {
            let here = *points.get(i)?;
            let before = if i > 0 {
                points.get(i - 1)
            } else if crease.closed {
                points.get(count.checked_sub(2)?)
            } else {
                Some(&here)
            };
            let after = if i + 1 < count {
                points.get(i + 1)
            } else if crease.closed {
                points.get(1)
            } else {
                Some(&here)
            };
            Some((here, *after? - *before?))
        })
        .collect()
}

/// The tool dropped on the part along `across` from the rotary axis at `x`
/// and `a`: where it rests.
struct Placement<'a> {
    input: &'a StrategyInput<'a>,
    grown: &'a RevolvedProfile,
    leave: f64,
    reach: f64,
}

impl Placement<'_> {
    /// The vertical line over the point `s` mm from (`along`, `across`) in
    /// direction (`d_along`, `d_across`).
    fn line(&self, site: &Site, s: f64) -> ToolLine {
        self.input.rotary.tool_line(
            Millimetres(site.along + s * site.d_along),
            site.a,
            Millimetres(site.across + s * site.d_across),
        )
    }

    /// The height of the tip along the line `s` mm from the crease, or
    /// infinity when the tool touches nothing there.
    fn height(&self, site: &Site, s: f64) -> f64 {
        self.input
            .surface
            .drop_tool(self.grown, &self.line(site, s))
            .map_or(f64::INFINITY, |c| c.offset)
    }

    /// Where the tool rests in a crease at `crease` running in direction
    /// `tangent`: searched across the crease, within the tool's reach, for the
    /// place where its tip is lowest and nearest the crease.
    fn rest(&self, crease: Point3, tangent: Vec3) -> Option<CutPoint> {
        let rotary = &self.input.rotary;
        let radial = Vec3::new(0.0, crease.y, crease.z);
        if radial.length() < 1e-6 {
            return None;
        }
        let a = rotary.a_facing(radial)?;
        let here = rotary.to_axis(crease, a);
        // The crease's direction in the plane the tool moves in (along the
        // rotary axis and across it); the search is at right angles to it.
        let ahead = rotary.to_axis(crease + tangent, a);
        let (t_along, t_across) = (ahead.along - here.along, ahead.across - here.across);
        let size = t_along.hypot(t_across);
        let (d_along, d_across) = if size > 1e-9 {
            (-t_across / size, t_along / size)
        } else {
            (0.0, 1.0)
        };
        let site = Site {
            a,
            along: here.along,
            across: here.across,
            d_along,
            d_across,
        };
        // A tool rests up to a few of its radii from the crease, further in a
        // narrower corner, so the search reaches 2.5 radii each way.
        let w = self.reach;

        // Coarse scan for the lowest, then golden section about it.
        const SCAN: usize = 16;
        let at = |i: usize| -w + 2.0 * w * i as f64 / SCAN as f64;
        let heights: Vec<f64> = (0..=SCAN).map(|i| self.height(&site, at(i))).collect();
        let (best, h0) =
            heights.iter().enumerate().fold(
                (0, f64::INFINITY),
                |m, (i, h)| if *h < m.1 { (i, *h) } else { m },
            );
        if !h0.is_finite() {
            return None;
        }
        let (mut lo, mut hi) = (at(best.saturating_sub(1)), at((best + 1).min(SCAN)));
        let phi = 0.5 * (5.0_f64.sqrt() - 1.0);
        for _ in 0..40 {
            let (c, d) = (hi - phi * (hi - lo), lo + phi * (hi - lo));
            if self.height(&site, c) <= self.height(&site, d) {
                hi = d;
            } else {
                lo = c;
            }
        }
        let mut u = 0.5 * (lo + hi);
        let low = self.height(&site, u).min(h0);
        let resting = |s: f64| self.height(&site, s) <= low + REST_TOLERANCE;
        if !resting(u) {
            u = at(best);
        }
        // Move towards the crease while the tool stays as low: it rests at
        // the edge of the flat that a wall comes down to.
        let step = w / SCAN as f64;
        let towards = if u > 0.0 { -1.0 } else { 1.0 };
        let mut good = u;
        let mut bad = None;
        while good != 0.0 {
            let probe = if towards > 0.0 {
                (good + step).min(0.0)
            } else {
                (good - step).max(0.0)
            };
            if resting(probe) {
                good = probe;
            } else {
                bad = Some(probe);
                break;
            }
        }
        if let Some(mut bad) = bad {
            for _ in 0..40 {
                let mid = 0.5 * (good + bad);
                if resting(mid) {
                    good = mid;
                } else {
                    bad = mid;
                }
            }
        }
        let line = self.line(&site, good);
        let drop = self.input.surface.drop_tool(self.grown, &line)?;
        let axis = rotary.tool_axis(a);
        let tip = drop.tip + axis.get() * self.leave;
        let contact = if self.leave > 0.0 {
            cutter::cutter_contact(self.input.tool, tip, axis, drop.contact)
        } else {
            drop.contact
        };
        Some(CutPoint { tip, a, contact })
    }
}

/// Where a crease sample is, in the frame fixed to the machine at its angle,
/// and the direction across the crease in which the tool is moved.
struct Site {
    a: Degrees,
    along: f64,
    across: f64,
    d_along: f64,
    d_across: f64,
}

impl Strategy for PencilFinishing {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Pencil clean-up"
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(vec![
            rest::min_remaining_spec(),
            ParameterSpec {
                name: pencil_parameter::CREASE_ANGLE.to_owned(),
                label: "Crease angle".to_owned(),
                description: "The smallest turn between two faces that counts as an inside \
                              corner to pencil. Smoother joins are left alone."
                    .to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: Some(0.0),
                    min_exclusive: true,
                    max: Some(180.0),
                    default: Some(30.0),
                },
            },
            ParameterSpec {
                name: pencil_parameter::MAX_STEP.to_owned(),
                label: "Largest step".to_owned(),
                description: "The furthest apart, along the crease, that points of the path \
                              are."
                    .to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Millimetres,
                    min: Some(0.0),
                    min_exclusive: true,
                    max: None,
                    default: Some(0.5),
                },
            },
        ])
    }

    fn needs_earlier_operations(&self) -> bool {
        true
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Ok(self
            .passes_with_notes(input, None, parameters, context)?
            .passes)
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&crate::region::RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        if region.is_some() {
            return Err(PathError::Strategy(format!(
                "the strategy '{NAME}' cannot yet be limited to a region"
            )));
        }
        let leave = parameters.number(names::STOCK_TO_LEAVE)?;
        let min_remaining = parameters.number(parameter::MIN_REMAINING)?;
        let angle = parameters.number(pencil_parameter::CREASE_ANGLE)?;
        let max_step = parameters.number(pencil_parameter::MAX_STEP)?;

        let creases = concave_creases(input.surface.tessellation(), angle);
        if creases.is_empty() {
            return Err(PathError::Strategy(format!(
                "pencil clean-up has no inside corner to follow: no concave crease turns \
                 {angle} degrees or more; lower {}",
                pencil_parameter::CREASE_ANGLE
            )));
        }
        let radius = input.tool.cutting_radius();
        let samples: Vec<Vec<(Point3, Vec3)>> =
            creases.iter().map(|c| resample(c, max_step)).collect();
        let total: f64 = samples.iter().map(|s| s.len() as f64).sum();
        if total > MAX_POINTS {
            return Err(PathError::parameter(
                pencil_parameter::MAX_STEP,
                format!(
                    "{total:.0} points are needed along the creases; increase the largest step"
                ),
            ));
        }

        // Where there is material to take: nothing is cut elsewhere.
        let map = if min_remaining > 0.0 {
            let spacing = rest::detection_spacing(input.tool, 2.0 * radius);
            let map = RemainingMap::detect(
                input.surface,
                input.stock,
                leave + min_remaining,
                spacing,
                radius + spacing,
                context,
            )?;
            if map.is_empty() {
                return Err(PathError::Strategy(rest::nothing_left(
                    "pencil clean-up",
                    min_remaining,
                    leave,
                    &map,
                )));
            }
            Some(map)
        } else {
            None
        };

        let grown = cutter::grow_profile(input.tool, leave)?;
        let placement = Placement {
            input,
            grown: &grown,
            leave,
            reach: 2.5 * radius.max(0.04),
        };
        context.report(
            "pencil clean-up: resting the tool in the creases",
            0,
            Some(total as u64),
        );
        let mut passes: Vec<Pass> = Vec::new();
        for (crease, points) in creases.iter().zip(&samples) {
            context.check_cancelled()?;
            let placed: Vec<Option<CutPoint>> =
                parallel::map_in_order(points, |(c, t)| placement.rest(*c, *t));
            let last_index = placed.len().saturating_sub(1);
            let mut run: Vec<CutPoint> = Vec::new();
            let mut runs: Vec<Vec<CutPoint>> = Vec::new();
            let (mut from_start, mut to_end) = (false, false);
            for (i, point) in placed.into_iter().enumerate() {
                let wanted = point.filter(|p| map.as_ref().is_none_or(|m| m.near(p.contact)));
                match wanted {
                    Some(p) => {
                        from_start |= i == 0;
                        to_end |= i == last_index;
                        run.push(p);
                    }
                    None => {
                        if !run.is_empty() {
                            runs.push(std::mem::take(&mut run));
                        }
                    }
                }
            }
            if !run.is_empty() {
                runs.push(run);
            }
            // A closed crease's last sample is its first. When both are cut,
            // the run that ends at the end and the one that starts at the
            // start are one stretch across the start; when either is not, the
            // gap between them must stay a gap.
            if crease.closed && runs.len() > 1 && from_start && to_end {
                let first = runs.remove(0);
                if let Some(last) = runs.last_mut() {
                    last.extend(first.into_iter().skip(1));
                }
            }
            passes.extend(runs.into_iter().map(|points| Pass {
                points,
                stock: None,
            }));
        }
        if passes.is_empty() {
            return Err(PathError::Strategy(
                "pencil clean-up found material left but the tool cannot rest in any \
                 corner that holds it"
                    .to_owned(),
            ));
        }
        let passes = order(passes);
        let notes = vec![format!(
            "pencil clean-up: {} concave creases, {} passes",
            creases.len(),
            passes.len()
        )];
        Ok(StrategyOutput { passes, notes })
    }
}

/// The passes in a short order: each next is the one whose nearer end is
/// nearest the end of the one before, starting from the lowest X. A pass is
/// reversed when its far end is the nearer. Ties go to the earlier pass, so
/// the order is deterministic.
fn order(mut passes: Vec<Pass>) -> Vec<Pass> {
    let mut out = Vec::with_capacity(passes.len());
    let start = passes
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let x = |p: &Pass| p.points.first().map_or(f64::INFINITY, |c| c.tip.x);
            x(a).total_cmp(&x(b))
        })
        .map(|(i, _)| i);
    let Some(start) = start else {
        return passes;
    };
    let mut current = passes.remove(start);
    loop {
        let end = current.points.last().map(|c| c.tip);
        out.push(current);
        let Some(end) = end else { break };
        let nearest = passes
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let (f, l) = (p.points.first()?, p.points.last()?);
                let (df, dl) = (f.tip.distance(end), l.tip.distance(end));
                Some((i, df.min(dl), dl < df))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, _, reverse)) = nearest else {
            break;
        };
        current = passes.remove(i);
        if reverse {
            current.points.reverse();
        }
    }
    out
}
