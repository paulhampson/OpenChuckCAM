// SPDX-License-Identifier: AGPL-3.0-or-later
//! The toolpath and the simulation result as plain data for the viewport
//! (SIM-01, SIM-05; work breakdown slice 9 items 3, 5 and 8; decision record
//! 0038).
//!
//! **Toolpaths (SIM-01).** [`ToolpathDisplay`] cuts a regeneration into runs of
//! moves of one [`PathClass`] (rapid, link, entry or cut) in the part's frame,
//! so the viewport can colour the classes apart and show an operation or hide
//! it. A move that turns A a long way is drawn as the arc the tool tip takes
//! (the controller moves X, Y, Z and A together), not as a chord.
//!
//! **Level of detail (item 8).** A path of millions of moves cannot all be
//! drawn. [`ToolpathDisplay::lines`] simplifies each run with the
//! Ramer-Douglas-Peucker rule at one of a few tolerances, from 0 (every
//! point) up to 3 mm, and picks the finest level whose visible points fit a
//! budget. A level is made the first time it is asked for and kept. The error
//! of a level is its tolerance, so the picture is never further than that
//! from the path; the front end shows the level it used.
//!
//! **Simulation (SIM-05).** [`deviation_bands`] sorts the part's triangles into
//! bands by the worst deviation sampled on each, to colour the part by
//! remaining material and overcut; the stock itself is a closed mesh
//! ([`SetupSimulation::stock_mesh`](crate::simulation::SetupSimulation::stock_mesh)).

use std::sync::OnceLock;

use openchuck_geom::select::TriangleSet;
use openchuck_types::geometry::Point3;
use openchuck_types::ir::MoveKind;

use crate::playback::{Timeline, between};

/// The tolerances, in millimetres, of the levels of detail; level 0 draws
/// every point.
pub const LEVEL_TOLERANCES: [f64; 7] = [0.0, 0.01, 0.03, 0.1, 0.3, 1.0, 3.0];

/// A turn of A larger than this, degrees, is drawn as an arc of several
/// points instead of one chord.
const ARC_STEP_DEGREES: f64 = 5.0;

/// What a run of moves is for, in the colours SIM-01 asks to tell apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathClass {
    /// Rapid traverse.
    Rapid,
    /// A link between passes, at feed.
    Link,
    /// A plunge, lead-in or lead-out.
    Entry,
    /// A cutting move.
    Cut,
}

impl PathClass {
    /// Every class.
    pub const ALL: [Self; 4] = [Self::Rapid, Self::Link, Self::Entry, Self::Cut];

    /// The class of a move kind.
    pub const fn of(kind: MoveKind) -> Self {
        match kind {
            MoveKind::Rapid => Self::Rapid,
            MoveKind::Linking => Self::Link,
            MoveKind::Plunge | MoveKind::LeadIn | MoveKind::LeadOut => Self::Entry,
            MoveKind::Cutting => Self::Cut,
        }
    }

    /// The class's name for a legend.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rapid => "Rapid",
            Self::Link => "Link",
            Self::Entry => "Entry",
            Self::Cut => "Cut",
        }
    }
}

/// Consecutive moves of one class in one group, as a polyline.
#[derive(Debug, Clone, PartialEq)]
pub struct PathRun {
    /// The group (operation) it belongs to.
    pub group: usize,
    /// What the moves are for.
    pub class: PathClass,
    /// The points, in the part's frame.
    pub points: Vec<Point3>,
}

/// A polyline chosen for drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct PathLine {
    /// The group it belongs to.
    pub group: usize,
    /// What it is for.
    pub class: PathClass,
    /// The points.
    pub points: Vec<Point3>,
}

/// What [`ToolpathDisplay::lines`] chose.
#[derive(Debug, Clone, PartialEq)]
pub struct PathLines {
    /// The level of detail used, an index into [`LEVEL_TOLERANCES`].
    pub level: usize,
    /// The most the drawn lines are from the path, millimetres.
    pub tolerance: f64,
    /// Points drawn.
    pub points: usize,
    /// Points the visible groups have at full detail.
    pub full_points: usize,
    /// The lines.
    pub lines: Vec<PathLine>,
}

/// A regeneration's toolpath cut into runs, with simplified copies made on
/// demand.
#[derive(Debug)]
pub struct ToolpathDisplay {
    runs: Vec<PathRun>,
    groups: usize,
    levels: Vec<OnceLock<Vec<Vec<Point3>>>>,
}

impl ToolpathDisplay {
    /// Cuts the timeline's toolpath into runs. Linear in the number of moves.
    pub fn build(timeline: &Timeline) -> Self {
        let path = &timeline.run().toolpath;
        let mut runs: Vec<PathRun> = Vec::new();
        for (g, group) in path.groups.iter().enumerate() {
            let frame = timeline.work_frame(g);
            let mut here = group.start;
            let mut open: Option<PathRun> = None;
            for segment in &group.segments {
                let class = PathClass::of(segment.kind);
                let run = match open.take() {
                    Some(run) if run.class == class => run,
                    other => {
                        runs.extend(other);
                        PathRun {
                            group: g,
                            class,
                            points: vec![here.tip],
                        }
                    }
                };
                let mut run = run;
                let turn = (segment.end.a.0 - here.a.0).abs();
                let steps = if turn > ARC_STEP_DEGREES {
                    ((turn / ARC_STEP_DEGREES).ceil() as usize).min(144)
                } else {
                    1
                };
                for k in 1..steps {
                    if let Some(frame) = &frame {
                        let (tip, ..) =
                            between(frame, &here, &segment.end, k as f64 / steps as f64);
                        run.points.push(tip);
                    }
                }
                run.points.push(segment.end.tip);
                here = segment.end;
                open = Some(run);
            }
            runs.extend(open);
        }
        Self {
            runs,
            groups: path.groups.len(),
            levels: LEVEL_TOLERANCES.iter().map(|_| OnceLock::new()).collect(),
        }
    }

    /// Number of groups.
    pub fn group_count(&self) -> usize {
        self.groups
    }

    /// The runs at full detail.
    pub fn runs(&self) -> &[PathRun] {
        &self.runs
    }

    /// Points in the runs of the visible groups at full detail.
    pub fn full_points(&self, visible: &dyn Fn(usize) -> bool) -> usize {
        self.runs
            .iter()
            .filter(|r| visible(r.group))
            .map(|r| r.points.len())
            .sum()
    }

    fn level(&self, level: usize) -> Option<&Vec<Vec<Point3>>> {
        let tolerance = *LEVEL_TOLERANCES.get(level)?;
        self.levels.get(level).map(|cell| {
            cell.get_or_init(|| {
                self.runs
                    .iter()
                    .map(|r| simplify(&r.points, tolerance))
                    .collect()
            })
        })
    }

    /// The lines of the groups `visible` says to draw, at the finest level of
    /// detail whose points fit `max_points` (the coarsest if none does).
    pub fn lines(&self, visible: &dyn Fn(usize) -> bool, max_points: usize) -> PathLines {
        let full_points = self.full_points(visible);
        let mut chosen = 0;
        for level in 0..LEVEL_TOLERANCES.len() {
            chosen = level;
            let count: usize = match self.level(level) {
                Some(runs) => self
                    .runs
                    .iter()
                    .zip(runs)
                    .filter(|(r, _)| visible(r.group))
                    .map(|(_, p)| p.len())
                    .sum(),
                None => usize::MAX,
            };
            if count <= max_points {
                break;
            }
        }
        let runs = self.level(chosen);
        let lines: Vec<PathLine> = self
            .runs
            .iter()
            .zip(runs.into_iter().flatten())
            .filter(|(r, _)| visible(r.group))
            .map(|(r, points)| PathLine {
                group: r.group,
                class: r.class,
                points: points.clone(),
            })
            .collect();
        PathLines {
            level: chosen,
            tolerance: LEVEL_TOLERANCES.get(chosen).copied().unwrap_or(0.0),
            points: lines.iter().map(|l| l.points.len()).sum(),
            full_points,
            lines,
        }
    }
}

/// Ramer-Douglas-Peucker: the points of `points` that keep every dropped
/// point within `tolerance` of the line through its neighbours kept. The ends
/// are always kept. Deterministic.
pub fn simplify(points: &[Point3], tolerance: f64) -> Vec<Point3> {
    let n = points.len();
    if n <= 2 || tolerance.is_nan() || tolerance <= 0.0 {
        return points.to_vec();
    }
    let mut keep = vec![false; n];
    if let Some(first) = keep.first_mut() {
        *first = true;
    }
    if let Some(last) = keep.last_mut() {
        *last = true;
    }
    let mut stack = vec![(0usize, n - 1)];
    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (Some(&a), Some(&b)) = (points.get(i), points.get(j)) else {
            continue;
        };
        let mut worst = (0.0_f64, i);
        for (k, &p) in points.iter().enumerate().take(j).skip(i + 1) {
            let d = distance_to_segment(p, a, b);
            if d > worst.0 {
                worst = (d, k);
            }
        }
        if worst.0 > tolerance {
            if let Some(flag) = keep.get_mut(worst.1) {
                *flag = true;
            }
            stack.push((i, worst.1));
            stack.push((worst.1, j));
        }
    }
    points
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(p, _)| *p)
        .collect()
}

fn distance_to_segment(p: Point3, a: Point3, b: Point3) -> f64 {
    let ab = b - a;
    let length_squared = ab.dot(ab);
    if length_squared <= 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// How far a part triangle is from the target, in the bands the viewport
/// colours (SIM-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviationBand {
    /// Cut deeper than the simulation's own error.
    Overcut,
    /// Within the simulation's error of the target surface.
    OnTarget,
    /// Up to a quarter of a millimetre of material left.
    Thin,
    /// A quarter to one millimetre left.
    Medium,
    /// More than a millimetre left, or the search reached its limit.
    Thick,
    /// No sample (a degenerate triangle).
    Unmeasured,
}

impl DeviationBand {
    /// Every band, in the order of a legend.
    pub const ALL: [Self; 6] = [
        Self::Overcut,
        Self::OnTarget,
        Self::Thin,
        Self::Medium,
        Self::Thick,
        Self::Unmeasured,
    ];

    /// The band's name for a legend.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Overcut => "Overcut",
            Self::OnTarget => "On the surface",
            Self::Thin => "Up to 0.25 mm left",
            Self::Medium => "0.25 to 1 mm left",
            Self::Thick => "More than 1 mm left",
            Self::Unmeasured => "Not measured",
        }
    }
}

/// The part's triangles sorted into bands.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviationBands {
    /// The error of the simulation that bounds `OnTarget`, millimetres: one
    /// cell (verification plan 4.1).
    pub tolerance: f64,
    /// The triangles of each band, for the bands that have any.
    pub bands: Vec<(DeviationBand, TriangleSet)>,
}

/// Sorts triangles by their worst sampled deviation (see
/// `openchuck_sim::triangle_values`): `values[i]` is triangle `i`'s value in
/// millimetres (positive for material left) and whether it is only a bound.
pub fn deviation_bands(values: &[Option<(f64, bool)>], tolerance: f64) -> DeviationBands {
    let mut sets: [TriangleSet; 6] = Default::default();
    for (i, value) in values.iter().enumerate() {
        let band = match value {
            None => DeviationBand::Unmeasured,
            Some((v, bound)) => {
                if *v < -tolerance {
                    DeviationBand::Overcut
                } else if *v <= tolerance {
                    DeviationBand::OnTarget
                } else if *bound || *v > 1.0 {
                    DeviationBand::Thick
                } else if *v > 0.25 {
                    DeviationBand::Medium
                } else {
                    DeviationBand::Thin
                }
            }
        };
        let index = DeviationBand::ALL
            .iter()
            .position(|b| *b == band)
            .unwrap_or(0);
        if let Some(set) = sets.get_mut(index) {
            set.insert(i as u32);
        }
    }
    DeviationBands {
        tolerance,
        bands: DeviationBand::ALL
            .into_iter()
            .zip(sets)
            .filter(|(_, s)| !s.is_empty())
            .collect(),
    }
}
