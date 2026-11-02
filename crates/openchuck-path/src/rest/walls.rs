// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rest machining at walls: rings where the tool's side meets the material
//! left, and what the tool cannot reach (OP-09, with OP-20's reporting;
//! SET-09; issue #508; decision record 0644).
//!
//! **Why.** Rest machining cuts rings (or a spiral) at its stepover, and the
//! faces square to the axis get a turn each (decision record 0297). A wall
//! that is steep but not square is left to the rings. A tool across the
//! rotary axis meets such a wall with its side, at one place along X for
//! each point of the wall: between two rings a flat end leaves a step about
//! the spacing times the wall's slope, and the inside corner at the wall's
//! foot is reached only if a ring happens to lie exactly where the side
//! touches it. A ball an earlier operation used leaves its fillet there, so
//! that is where the material is.
//!
//! **Wall rings.** For each sample with material left whose normal leans
//! along X, the tool's own profile gives the point of its edge whose normal
//! matches the sample's ([`RevolvedProfile::point_for_normal`]), and so the
//! X of the tool's axis at which that edge touches the sample: a cutting
//! radius from it for a flat end, less for a ball or a corner radius. A ring
//! anywhere from there to `target / slope` further from the wall leaves no
//! more than about `target` on it (`slope` the share of the normal along
//! X). The fewest rings that put one in each sample's range are found by
//! the greedy stab of intervals sorted by their far end, which gives each
//! foot of a wall a ring exactly at its touching place. The rings are cut,
//! as every rest pass is, only where material is left (decision record 0420
//! item 2).
//!
//! **Reach.** Each sample is then tried at the place that should reach it,
//! the tool dropped there on the whole part. If the dropped tool stays
//! further from the sample than the threshold, no place of this tool reaches
//! it: the part holds it up (the sample is in a groove narrower than the
//! tool), or the sample faces away from every direction the tool can come
//! from, or the place is over a held region's keep-out. Such samples are
//! grouped into areas and reported with where they are, how large and why,
//! instead of being left without a word (NFR-12).
//!
//! Everything is computed in a fixed order from the samples, which the map
//! gives in a fixed order, and the drops run through
//! [`parallel::map_in_order`], so the result is the same at any thread
//! count (GC-07, NFR-06).

use std::collections::BTreeMap;
use std::f64::consts::FRAC_PI_2;

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;

use super::Sample;
use crate::assemble::{Unreached, UnreachedReason};
use crate::cutter::{self, ToolLine};
use crate::error::Result;
use crate::parallel;
use crate::strategy::{HELD_CLEARANCE, StrategyInput, reaches_in};

/// Most rings added at walls in one operation; past it the rest are not
/// added and a warning says so (as adaptive spacing's added passes,
/// decision record 0239 item 8).
pub(crate) const MAX_WALL_RINGS: usize = 10_000;

/// Below this share of its normal along X a sample lies on a surface round
/// the axis, which the rings at the stepover cover: about one degree.
const ROUND: f64 = 0.02;

/// How far, in toolpath tolerances, a ring at a wall stands clear of it, as
/// the turns at faces across the axis do (decision record 0297).
const CLEAR: f64 = 0.1;

/// A remaining sample nearer the tool's surface than this, in a grid step,
/// is seen from outside along its radius.
const VISIBLE_GAP: f64 = 0.1;

/// Samples further apart along X than this many sample spacings, or than
/// [`MIN_SPLIT`], are in separate areas.
const SPLIT: f64 = 4.0;
const MIN_SPLIT: f64 = 0.5;

/// How far, in millimetres, a sample may lie beyond the end of a held
/// region and still count as on it: rounding in the host's widening and
/// in the region's faces.
const GRIP_SLACK: f64 = 1e-6;

/// The size of a cell of the reach check, millimetres along X and round
/// the axis: one drop answers for the samples of a cell.
const REACH_CELL: f64 = 0.25;

/// Most places of the tool one wall sample asks for across its patch.
const PATCH_STEPS: f64 = 64.0;

/// An area all round the axis when no gap between its samples' angles is
/// wider than this, in degrees: the samples are spread thinly where little
/// is left, so a narrower gap is not a place without material.
const ROUND_GAP: f64 = 90.0;

/// What a rest operation needs to place its wall rings and check its reach.
pub(crate) struct Walls<'a> {
    /// The tool, grown by the stock to leave, as the operation drops it.
    pub grown: &'a RevolvedProfile,
    /// The part's extent along X.
    pub span: (f64, f64),
    /// The stepover: the widest gap between rings.
    pub stepover: f64,
    /// The material a ring spacing may leave on a wall, millimetres.
    pub target: f64,
    /// The closest two rings may be, millimetres.
    pub smallest: f64,
    /// The toolpath tolerance, millimetres.
    pub tolerance: f64,
    /// The threshold the samples were found with, above the stock to
    /// leave: a dropped tool this near a sample reaches it.
    pub min_remaining: f64,
}

/// Where the tool's axis must be along X to reach a sample.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Place {
    /// On a surface round the axis: any X within `window` of the sample's
    /// own, the reach of the tool's flat bottom (0 for a round end).
    Round { window: f64 },
    /// On a wall: the axis at `x`, on the `side` of the sample (+1 or -1
    /// along X) where the wall faces, `slope` the share of the normal
    /// along X and `rise` the share of the wall's own direction along X.
    Wall {
        x: f64,
        side: f64,
        slope: f64,
        rise: f64,
    },
    /// The surface faces the rotary axis: no direction across it reaches.
    FacesAxis,
    /// On the rotary axis: the free ends' layers handle it (decision
    /// record 0344).
    OnAxis,
    /// The tool's profile is not convex, so no edge point is known.
    Unmodelled,
}

impl Walls<'_> {
    fn place(&self, s: &Sample) -> Place {
        let p = s.point;
        let rho = p.y.hypot(p.z);
        if rho < 1e-6 {
            return Place::OnAxis;
        }
        let up = Vec3::new(0.0, p.y / rho, p.z / rho);
        let (nx, nr) = (s.normal.x, s.normal.dot(up));
        let m = nx.hypot(nr);
        if m < 1e-9 {
            // A face turned round the axis (the side of a flute): a ring
            // sweeps across it, as across a surface round the axis.
            return Place::Round { window: 0.0 };
        }
        let (sx, sr) = (nx / m, nr / m);
        if sr < -1e-6 {
            return Place::FacesAxis;
        }
        if sx.abs() < ROUND {
            return self
                .grown
                .point_for_normal(-FRAC_PI_2 + 1e-6)
                .map_or(Place::Unmodelled, |q| Place::Round { window: q.radius.0 });
        }
        // The tool's outward normal at the contact is the sample's turned
        // round: along the tool's axis it is -sr, across it |sx|.
        let Some(edge) = self.grown.point_for_normal((-sr).atan2(sx.abs())) else {
            return Place::Unmodelled;
        };
        let side = sx.signum();
        Place::Wall {
            x: p.x + side * (edge.radius.0 + CLEAR * self.tolerance),
            side,
            slope: sx.abs(),
            rise: sr,
        }
    }

    /// Whether the tool's profile lets wall rings be placed at all.
    pub(crate) fn modelled(&self) -> bool {
        self.grown.point_for_normal(-FRAC_PI_2).is_some()
    }

    /// The X of the rings that put one in reach of every wall sample,
    /// sorted, with whether [`MAX_WALL_RINGS`] stopped them. A sample
    /// stands for the wall within `spacing` of it (the samples are about
    /// that far apart), so the lowest sample of a wall also places the ring
    /// at the wall's foot, the inside corner.
    pub(crate) fn rings(
        &self,
        samples: &[Sample],
        spacing: f64,
        keep_out: &[(f64, f64)],
    ) -> (Vec<f64>, bool) {
        let (x0, x1) = self.span;
        let mut ranges: Vec<(f64, f64)> = Vec::new();
        for s in samples {
            let (x, side, slope, rise) = match self.place(s) {
                Place::Wall {
                    x,
                    side,
                    slope,
                    rise,
                } => (x, side, slope, rise),
                Place::Round { window } => {
                    // Beside a held region the rings at the stepover stop
                    // short of the keep-out's edge; a ring on the edge is
                    // the nearest the tool may come, as the foot of a wall.
                    for &(a, b) in keep_out {
                        for edge in [a, b] {
                            if (edge - s.point.x).abs() <= window
                                && edge >= x0
                                && edge <= x1
                                && !reaches_in(keep_out, edge, edge)
                            {
                                ranges.push((edge, edge));
                            }
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            let reach = (self.target / slope).clamp(self.smallest, 0.5 * self.stepover);
            // The patch of wall the sample stands for, as places of the
            // tool's axis: the wall's own direction moves along X by `rise`
            // of the distance along it.
            let half = spacing * rise;
            let steps = (2.0 * half / reach).ceil().clamp(0.0, PATCH_STEPS);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            for k in 0..=steps as usize {
                let c = if steps > 0.0 {
                    x - half + 2.0 * half * k as f64 / steps
                } else {
                    x
                };
                if c < x0 || c > x1 || reaches_in(keep_out, c, c) {
                    continue;
                }
                ranges.push(if side > 0.0 {
                    (c, c + reach)
                } else {
                    (c - reach, c)
                });
            }
        }
        ranges.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
        let mut rings: Vec<f64> = Vec::new();
        for (lo, hi) in ranges {
            if rings.last().is_some_and(|&r| r >= lo - 1e-12) {
                continue;
            }
            if rings.len() == MAX_WALL_RINGS {
                return (rings, true);
            }
            rings.push(hi.clamp(x0, x1));
        }
        rings.dedup_by(|b, a| (*b - *a).abs() < 1e-9);
        (rings, false)
    }

    /// The samples no place of the tool reaches, grouped into areas,
    /// largest first.
    pub(crate) fn unreached(
        &self,
        input: &StrategyInput<'_>,
        samples: &[Sample],
        spacing: f64,
        context: &JobContext<'_>,
    ) -> Result<Vec<Unreached>> {
        context.report("rest machining: checking what the tool can reach", 0, None);
        let outside = {
            let b = input.surface.bounds();
            [b.min.y, b.max.y, b.min.z, b.max.z]
                .iter()
                .fold(0.0_f64, |m, v| m.max(v.abs()))
                * std::f64::consts::SQRT_2
                + 1.0
        };
        // The tool's largest radius and the held clearance, by which the
        // host widened each held region into its keep-out interval: a
        // sample inside the interval shrunk by them is under the grip, which
        // holds it by design and the simulation does not count (decision
        // record 0194).
        // The radius is the tool's own, not grown by the stock to leave, as
        // the host widens by it. The held region's own faces lie on the
        // shrunk interval's ends, and the samples on them are of the
        // workholding, not the part, so the ends count as under the grip.
        let widest = HELD_CLEARANCE
            + input
                .tool
                .pieces()
                .iter()
                .map(|p| p.end().radius.0)
                .fold(input.tool.cutting_radius(), f64::max)
                .max(input.shank.map_or(0.0, |s| s.radius));
        let gripped = |x: f64| {
            input
                .keep_out
                .iter()
                .any(|&(a, b)| x >= a + widest - GRIP_SLACK && x <= b - widest + GRIP_SLACK)
        };
        // One representative per small cell of the surface, of one kind of
        // place: the report is of areas, and a drop per sample would cost
        // as many drops as the map has samples (millions on a finely
        // tessellated part). Every sample of a cell takes its
        // representative's answer; the cells are in a fixed order.
        let mut cells: BTreeMap<(i64, i64, i8), Vec<usize>> = BTreeMap::new();
        for (i, s) in samples.iter().enumerate() {
            if gripped(s.point.x) {
                continue;
            }
            cells.entry(self.cell(s)).or_default().push(i);
        }
        let groups: Vec<Vec<usize>> = cells.into_values().collect();
        let reasons: Vec<Option<UnreachedReason>> = parallel::map_in_order(&groups, |g| {
            g.first()
                .and_then(|&i| samples.get(i))
                .and_then(|s| self.why(input, s, outside))
        });
        context.check_cancelled()?;
        let found: Vec<(Sample, UnreachedReason)> = groups
            .iter()
            .zip(reasons)
            .filter_map(|(g, r)| r.map(|r| (g, r)))
            .flat_map(|(g, r)| {
                g.iter()
                    .filter_map(move |&i| samples.get(i).map(|s| (*s, r)))
            })
            .collect();
        Ok(group(found, spacing))
    }

    /// The cell of the reach check a sample falls in: [`REACH_CELL`] along
    /// X and round the axis at its radius, and the kind of place it is (a
    /// wall facing either way, a surface round the axis, and the rest), so
    /// a cell at a wall's foot does not mix the floor with the wall.
    fn cell(&self, s: &Sample) -> (i64, i64, i8) {
        let p = s.point;
        let around = p.z.atan2(p.y).rem_euclid(std::f64::consts::TAU) * p.y.hypot(p.z);
        let kind = match self.place(s) {
            Place::Round { .. } => 0,
            Place::Wall { side, .. } if side > 0.0 => 1,
            Place::Wall { .. } => 2,
            Place::FacesAxis => 3,
            Place::OnAxis => 4,
            Place::Unmodelled => 5,
        };
        #[allow(clippy::cast_possible_truncation)]
        let key = |v: f64| (v / REACH_CELL).floor() as i64;
        (key(p.x), key(around), kind)
    }

    /// Why no place of the tool reaches the sample; `None` when one does,
    /// or when this check cannot tell.
    fn why(&self, input: &StrategyInput<'_>, s: &Sample, outside: f64) -> Option<UnreachedReason> {
        let p = s.point;
        let (x0, x1) = self.span;
        let x = match self.place(s) {
            Place::OnAxis | Place::Unmodelled => return None,
            Place::FacesAxis => return Some(UnreachedReason::Blocked),
            Place::Wall { x, .. } => {
                // Past an end of the part: the free ends' layers go down
                // there (decision record 0344).
                if x < x0 || x > x1 {
                    return None;
                }
                if reaches_in(input.keep_out, x, x) {
                    return Some(UnreachedReason::NearHeldRegion);
                }
                x
            }
            Place::Round { window } => {
                // Anywhere under the tool's flat bottom reaches it, unless
                // the tool rides up on something beside it there: try the
                // middle, then the ends, then between.
                let w = 0.999 * window;
                let tries: Vec<f64> = ROUND_TRIES
                    .iter()
                    .map(|f| p.x + f * w)
                    .filter(|&c| !reaches_in(input.keep_out, c, c))
                    .collect();
                if tries.is_empty() {
                    return Some(UnreachedReason::NearHeldRegion);
                }
                if tries.iter().any(|&c| self.reaches(input, p, c)) {
                    return None;
                }
                return Some(self.blocked_or_narrow(input, p, outside));
            }
        };
        if self.reaches(input, p, x) {
            return None;
        }
        Some(self.blocked_or_narrow(input, p, outside))
    }

    /// Whether the tool dropped with its axis at `x`, across the axis
    /// through `p`, comes within the threshold of `p`. A drop that meets
    /// nothing says yes: nothing holds the tool up there.
    fn reaches(&self, input: &StrategyInput<'_>, p: Point3, x: f64) -> bool {
        let rho = p.y.hypot(p.z);
        let Ok(up) = UnitVec3::new(Vec3::new(0.0, p.y / rho, p.z / rho)) else {
            return true;
        };
        let line = ToolLine {
            origin: Point3::new(x, 0.0, 0.0),
            axis: up,
        };
        input
            .surface
            .drop_tool(self.grown, &line)
            .is_none_or(|contact| {
                cutter::clearance_at(self.grown, contact.tip, up, p)
                    <= self.min_remaining + self.tolerance
            })
    }

    /// Why a place the tool's direction fits does not reach `p`.
    fn blocked_or_narrow(
        &self,
        input: &StrategyInput<'_>,
        p: Point3,
        outside: f64,
    ) -> UnreachedReason {
        let rho = p.y.hypot(p.z);
        let Ok(up) = UnitVec3::new(Vec3::new(0.0, p.y / rho, p.z / rho)) else {
            return UnreachedReason::Blocked;
        };
        // Seen from outside along its radius: the tool fits the direction
        // but not the room. Hidden: something of the part is in the way.
        let seen = UnitVec3::new(-up.get()).ok().is_some_and(|down| {
            let far = outside + rho;
            input
                .surface
                .cast_ray(p + up.get() * far, down, far + 1.0)
                .is_some_and(|h| (h.point - p).length() <= VISIBLE_GAP)
        });
        if seen {
            UnreachedReason::TooNarrow
        } else {
            UnreachedReason::Blocked
        }
    }
}

/// Where under its flat bottom the tool is tried for a sample on a surface
/// round the axis, as shares of the flat's radius.
const ROUND_TRIES: [f64; 9] = [0.0, -1.0, 1.0, -0.5, 0.5, -0.75, 0.75, -0.25, 0.25];

/// Samples grouped into areas by reason and by nearness along X; each area
/// with its X range, A range and area, largest first.
fn group(mut found: Vec<(Sample, UnreachedReason)>, spacing: f64) -> Vec<Unreached> {
    let rank = |r: &UnreachedReason| match r {
        UnreachedReason::Blocked => 0,
        UnreachedReason::TooNarrow => 1,
        UnreachedReason::NearHeldRegion => 2,
        UnreachedReason::NeedsLean { .. } => 3,
    };
    found.sort_by(|a, b| {
        rank(&a.1)
            .cmp(&rank(&b.1))
            .then(a.0.point.x.total_cmp(&b.0.point.x))
    });
    let split = (SPLIT * spacing).max(MIN_SPLIT);
    let mut areas: Vec<Unreached> = Vec::new();
    let mut run: Vec<Sample> = Vec::new();
    let mut reason = None;
    let mut flush = |run: &mut Vec<Sample>, reason: Option<UnreachedReason>| {
        if let (Some(r), Some(first), Some(last)) = (reason, run.first(), run.last()) {
            areas.push(Unreached {
                x: (first.point.x, last.point.x),
                a: angles(run),
                area: run.iter().map(|s| s.area).sum(),
                reason: r,
            });
        }
        run.clear();
    };
    for (s, r) in found {
        let joins = reason == Some(r) && run.last().is_some_and(|l| s.point.x - l.point.x <= split);
        if !joins {
            flush(&mut run, reason);
            reason = Some(r);
        }
        run.push(s);
    }
    flush(&mut run, reason);
    areas.sort_by(|a, b| b.area.total_cmp(&a.area).then(a.x.0.total_cmp(&b.x.0)));
    areas
}

/// The A range the samples face, in degrees: `(0, 360)` all round, else
/// from the end of the widest gap between their angles to its start.
fn angles(samples: &[Sample]) -> (f64, f64) {
    let mut a: Vec<f64> = samples
        .iter()
        .map(|s| s.point.z.atan2(s.point.y).to_degrees().rem_euclid(360.0))
        .collect();
    a.sort_by(f64::total_cmp);
    let (Some(&first), Some(&last)) = (a.first(), a.last()) else {
        return (0.0, 0.0);
    };
    // The widest gap, the one across 360 included.
    let mut gap = (first + 360.0 - last, last, first + 360.0);
    for w in a.windows(2) {
        if let [lo, hi] = w
            && hi - lo > gap.0
        {
            gap = (hi - lo, *lo, *hi);
        }
    }
    if gap.0 <= ROUND_GAP {
        return (0.0, 360.0);
    }
    // From the end of the gap round to its start.
    let lo = gap.2.rem_euclid(360.0);
    let mut hi = gap.1;
    if hi < lo {
        hi += 360.0;
    }
    (lo, hi)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn at(x: f64, deg: f64) -> Sample {
        let t = deg.to_radians();
        Sample {
            point: Point3::new(x, 5.0 * t.cos(), 5.0 * t.sin()),
            normal: Vec3::new(0.0, t.cos(), t.sin()),
            area: 0.01,
        }
    }

    #[test]
    fn req_op_09_an_area_all_round_is_said_to_be_all_round() {
        let s: Vec<Sample> = (0..36).map(|k| at(1.0, f64::from(k) * 10.0)).collect();
        assert_eq!(angles(&s), (0.0, 360.0));
    }

    #[test]
    fn req_op_09_an_area_on_one_side_gives_its_angles_across_zero() {
        let s: Vec<Sample> = [350.0, 355.0, 0.0, 5.0, 10.0]
            .iter()
            .map(|&d| at(1.0, d))
            .collect();
        let (lo, hi) = angles(&s);
        assert!(
            (lo - 350.0).abs() < 1e-9 && (hi - 370.0).abs() < 1e-9,
            "{lo} {hi}"
        );
    }

    #[test]
    fn req_op_09_areas_split_by_reason_and_by_distance_along_x() {
        let found = vec![
            (at(1.0, 0.0), UnreachedReason::TooNarrow),
            (at(1.1, 0.0), UnreachedReason::TooNarrow),
            (at(5.0, 0.0), UnreachedReason::TooNarrow),
            (at(1.05, 0.0), UnreachedReason::Blocked),
        ];
        let areas = group(found, 0.05);
        assert_eq!(areas.len(), 3);
        // Largest first: the two samples near X 1.
        assert_eq!(areas[0].reason, UnreachedReason::TooNarrow);
        assert_eq!(areas[0].x, (1.0, 1.1));
        assert!((areas[0].area - 0.02).abs() < 1e-12);
    }
}
