// SPDX-License-Identifier: AGPL-3.0-or-later
//! The toolpath and the simulation result as plain data for the viewport
//! (SIM-01, SIM-05; work breakdown slice 9 items 3, 5 and 8; decision record
//! 0040).
//!
//! **Toolpaths (SIM-01).** [`ToolpathDisplay`] cuts a regeneration into runs of
//! moves of one [`PathClass`] (rapid, link, entry or cut) in the part's frame,
//! so the viewport can colour the classes apart and show an operation or hide
//! it. A move that turns A a long way is drawn as the arc the tool tip takes
//! (the controller moves X, Y, Z and A together), not as a chord.
//!
//! **Level of detail (item 8).** A path of millions of moves cannot all be
//! drawn. [`ToolpathDisplay::lines`] simplifies each run (see [`simplify`])
//! at one of a few tolerances, from 0 (every
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
    /// Which moves each run draws, in the same order as `runs`, for the
    /// playback window (issue 205).
    moves: Vec<RunMoves>,
    groups: usize,
    levels: Vec<OnceLock<Vec<Vec<Point3>>>>,
}

/// The moves of one run: the first, counted across all groups, and the index
/// of the point each move ends at.
#[derive(Debug, Clone, Default)]
struct RunMoves {
    first: usize,
    ends: Vec<usize>,
}

/// The level of detail of the faint trace of the path outside the playback
/// window, or coarser when the budget needs it (issue 205; decision record
/// 0169): 1 mm, enough to keep one's bearings. On the owner's roughing path
/// of 23 311 moves (issue 200) it is 29 384 points of the path's 47 826; a
/// finer level kept 94% of them.
pub const TRACE_LEVEL: usize = 5;

impl ToolpathDisplay {
    /// Cuts the timeline's toolpath into runs. Linear in the number of moves.
    pub fn build(timeline: &Timeline) -> Self {
        let path = &timeline.run().toolpath;
        let mut runs: Vec<PathRun> = Vec::new();
        let mut moves: Vec<RunMoves> = Vec::new();
        let mut flat = 0_usize;
        for (g, group) in path.groups.iter().enumerate() {
            let frame = timeline.work_frame(g);
            let mut here = group.start;
            let mut open: Option<(PathRun, RunMoves)> = None;
            for segment in &group.segments {
                let class = PathClass::of(segment.kind);
                let (mut run, mut marks) = match open.take() {
                    Some((run, marks)) if run.class == class => (run, marks),
                    other => {
                        if let Some((run, marks)) = other {
                            runs.push(run);
                            moves.push(marks);
                        }
                        (
                            PathRun {
                                group: g,
                                class,
                                points: vec![here.tip],
                            },
                            RunMoves {
                                first: flat,
                                ends: Vec::new(),
                            },
                        )
                    }
                };
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
                marks.ends.push(run.points.len() - 1);
                flat += 1;
                here = segment.end;
                open = Some((run, marks));
            }
            if let Some((run, marks)) = open {
                runs.push(run);
                moves.push(marks);
            }
        }
        Self {
            runs,
            moves,
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
        self.lines_for(visible, max_points, 0)
    }

    /// The lines of the groups `visible` says to draw, at `min_level` (the
    /// level the zoom asks for, [`zoom_level`]) or at the level the budget
    /// of `max_points` needs, whichever is coarser (decision record 0167).
    /// A level past the coarsest is the coarsest.
    pub fn lines_for(
        &self,
        visible: &dyn Fn(usize) -> bool,
        max_points: usize,
        min_level: usize,
    ) -> PathLines {
        let full_points = self.full_points(visible);
        let chosen = self
            .budget_level(visible, max_points)
            .max(min_level)
            .min(LEVEL_TOLERANCES.len() - 1);
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

    /// The finest level whose visible points fit `max_points`, or the
    /// coarsest if none does.
    fn budget_level(&self, visible: &dyn Fn(usize) -> bool, max_points: usize) -> usize {
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
        chosen
    }
}

/// How much of a pixel a level's error may span: half (decision record
/// 0167).
pub const PIXEL_SHARE: f64 = 0.5;

/// How far past a coarser level's tolerance half a pixel must reach before
/// the drawing switches to it.
const COARSEN_MARGIN: f64 = 1.25;

/// The level of detail for a view where one pixel spans `mm_per_pixel` at
/// the camera's target, starting from the level `current` drawn now
/// (decision record 0167, OI-177): the coarsest level whose tolerance is
/// within half a pixel. It goes finer at once when the current level is
/// coarser than that, and coarser only once half a pixel is 1.25 times the
/// coarser level's tolerance, so a zoom that hovers at a boundary does not
/// swap the path back and forth, and a zoom within a level asks for no new
/// path. A size that is not a positive finite number gives level 0 (every
/// point; the budget still applies). The level is for drawing only: the
/// runs, playback and the output never see it.
pub fn zoom_level(current: usize, mm_per_pixel: f64) -> usize {
    if !(mm_per_pixel.is_finite() && mm_per_pixel > 0.0) {
        return 0;
    }
    let half = PIXEL_SHARE * mm_per_pixel;
    let coarsest = |margin: f64| {
        (0..LEVEL_TOLERANCES.len())
            .rev()
            .find(|&l| LEVEL_TOLERANCES.get(l).is_some_and(|t| t * margin <= half))
            .unwrap_or(0)
    };
    let within = coarsest(1.0);
    let current = current.min(LEVEL_TOLERANCES.len() - 1);
    if within < current {
        within
    } else {
        coarsest(COARSEN_MARGIN).max(current)
    }
}

impl ToolpathDisplay {
    /// The points of each visible run within the moves `window` (counted
    /// from 0 across all groups), at full detail: the run's index and the
    /// slice.
    fn window_slices<'a>(
        &'a self,
        visible: &'a dyn Fn(usize) -> bool,
        window: std::ops::Range<usize>,
    ) -> impl Iterator<Item = (usize, &'a [Point3])> + 'a {
        self.runs
            .iter()
            .zip(&self.moves)
            .enumerate()
            .filter(move |(_, (r, _))| visible(r.group))
            .filter_map(move |(i, (run, marks))| {
                let end = marks.first + marks.ends.len();
                let (from, to) = (window.start.max(marks.first), window.end.min(end));
                if from >= to {
                    return None;
                }
                // A move starts where the one before it ended; the run's
                // first starts at its first point.
                let start = match from - marks.first {
                    0 => 0,
                    k => *marks.ends.get(k - 1)?,
                };
                let stop = *marks.ends.get(to - marks.first - 1)?;
                Some((i, run.points.get(start..=stop)?))
            })
    }

    /// The lines of the visible groups within the moves `window` (issue 205;
    /// decision record 0169): every point when the window's points fit
    /// `max_points` and `min_level` asks for no coarser level, else
    /// simplified at the coarser of `min_level` and the level the whole
    /// path needs for the budget. A window cuts a run at a move's end, so
    /// the drawing starts and stops where the play head can.
    pub fn window_lines(
        &self,
        visible: &dyn Fn(usize) -> bool,
        window: std::ops::Range<usize>,
        max_points: usize,
        min_level: usize,
    ) -> PathLines {
        let full_points: usize = self
            .window_slices(visible, window.clone())
            .map(|(_, p)| p.len())
            .sum();
        let chosen = if full_points <= max_points {
            min_level
        } else {
            self.level_for_budget(visible, max_points).max(min_level)
        }
        .min(LEVEL_TOLERANCES.len() - 1);
        let tolerance = LEVEL_TOLERANCES.get(chosen).copied().unwrap_or(0.0);
        let lines: Vec<PathLine> = self
            .window_slices(visible, window)
            .filter_map(|(i, points)| {
                let run = self.runs.get(i)?;
                Some(PathLine {
                    group: run.group,
                    class: run.class,
                    points: simplify(points, tolerance),
                })
            })
            .collect();
        PathLines {
            level: chosen,
            tolerance,
            points: lines.iter().map(|l| l.points.len()).sum(),
            full_points,
            lines,
        }
    }

    /// The faint trace of the whole visible path, drawn under the playback
    /// window's part (issue 205; decision record 0169): at [`TRACE_LEVEL`],
    /// or coarser when the budget needs it. It is made from the cached level
    /// and depends only on the path and the visible groups, so moving or
    /// resizing the window never makes it again.
    pub fn trace_lines(&self, visible: &dyn Fn(usize) -> bool, max_points: usize) -> PathLines {
        let chosen = self
            .level_for_budget(visible, max_points)
            .max(TRACE_LEVEL)
            .min(LEVEL_TOLERANCES.len() - 1);
        let full_points = self.full_points(visible);
        let lines: Vec<PathLine> = self
            .runs
            .iter()
            .zip(self.level(chosen).into_iter().flatten())
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

    /// The finest level whose visible points fit `max_points`, or the
    /// coarsest if none does.
    fn level_for_budget(&self, visible: &dyn Fn(usize) -> bool, max_points: usize) -> usize {
        (0..LEVEL_TOLERANCES.len())
            .find(|&level| {
                self.level(level).is_some_and(|runs| {
                    self.runs
                        .iter()
                        .zip(runs)
                        .filter(|(r, _)| visible(r.group))
                        .map(|(_, p)| p.len())
                        .sum::<usize>()
                        <= max_points
                })
            })
            .unwrap_or(LEVEL_TOLERANCES.len() - 1)
    }
}

/// A simplified copy of `points`: some of them, in order, with the first and
/// last kept, such that every point is within `tolerance` of the polyline
/// through the points kept. Linear in the number of points and
/// deterministic.
///
/// This is a sleeve method (Reumann and Witkam), chosen because the usual
/// Ramer-Douglas-Peucker rule splits a long spiral unevenly and took over a
/// minute on two million points. From a kept point it takes a direction to
/// the first point at least `tolerance` away, and goes on while the points
/// stay within a strip of half-width `tolerance / 3` about that line and do
/// not turn back along it by more than `tolerance / 4`. Every point dropped
/// then lies within `tolerance / 3` of that line, and so within `0.72 *
/// tolerance` of the segment to the last point of the strip (whose own offset
/// is also within `tolerance / 3`): the stated tolerance holds with room.
pub fn simplify(points: &[Point3], tolerance: f64) -> Vec<Point3> {
    let n = points.len();
    if n <= 2 || tolerance.is_nan() || tolerance <= 0.0 {
        return points.to_vec();
    }
    let half_width = tolerance / 3.0;
    let slack = tolerance / 4.0;
    let mut kept = vec![0usize];
    let mut i = 0usize;
    while i + 1 < n {
        let Some(&start) = points.get(i) else { break };
        // The first point far enough away to give a direction.
        let anchor = (i + 1..n).find(|&j| {
            points
                .get(j)
                .is_some_and(|p| p.distance(start) >= tolerance)
        });
        let Some(anchor) = anchor else { break };
        let (Some(&a), true) = (points.get(anchor), anchor > i) else {
            break;
        };
        let along = a - start;
        let length = along.length();
        if !(length.is_finite() && length > 0.0) {
            kept.push(anchor);
            i = anchor;
            continue;
        }
        let direction = along * (1.0 / length);
        let mut last = anchor;
        let mut farthest = length;
        for k in anchor + 1..n {
            let Some(&p) = points.get(k) else { break };
            let offset = p - start;
            let s = offset.dot(direction);
            let h = (offset - direction * s).length();
            if h > half_width || s < farthest - slack {
                break;
            }
            farthest = farthest.max(s);
            last = k;
        }
        kept.push(last);
        i = last;
    }
    if kept.last() != Some(&(n - 1)) {
        kept.push(n - 1);
    }
    kept.into_iter()
        .filter_map(|k| points.get(k).copied())
        .collect()
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
    /// Each triangle's value as given to [`deviation_bands`], the value it is
    /// coloured by, for the probe that reads it under the pointer (SIM-11
    /// in part, decision record 0162).
    pub values: std::sync::Arc<[Option<(f64, bool)>]>,
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
        values: values.into(),
    }
}

/// A regeneration prepared for display and playback: its timeline and its
/// toolpath cut into runs.
#[derive(Debug, Clone)]
pub struct PlaybackData {
    /// The moves laid end to end in time.
    pub timeline: std::sync::Arc<Timeline>,
    /// The toolpath as runs, with levels of detail.
    pub display: std::sync::Arc<ToolpathDisplay>,
}

impl crate::Application {
    /// Prepares `run` for display and playback as a job (SIM-01, SIM-04):
    /// linear in the number of moves, so a path of millions of moves does not
    /// stall the interface.
    pub fn start_playback_data(
        &self,
        run: std::sync::Arc<crate::Regeneration>,
    ) -> crate::JobHandle<PlaybackData> {
        let project = self.document.project.clone();
        self.jobs
            .submit("prepare display", self.events_tx.clone(), move |context| {
                let timeline = std::sync::Arc::new(Timeline::new(&project, run)?);
                context.check_cancelled()?;
                let display = std::sync::Arc::new(ToolpathDisplay::build(&timeline));
                Ok(PlaybackData { timeline, display })
            })
    }

    /// [`ToolpathDisplay::window_lines`] as a job (issue 205).
    pub fn start_window_lines(
        &self,
        display: std::sync::Arc<ToolpathDisplay>,
        visible: Vec<bool>,
        window: std::ops::Range<usize>,
        max_points: usize,
        min_level: usize,
    ) -> crate::JobHandle<PathLines> {
        self.jobs.submit(
            "choose path detail",
            self.events_tx.clone(),
            move |context| {
                context.check_cancelled()?;
                Ok(display.window_lines(
                    &|group| visible.get(group).copied().unwrap_or(false),
                    window,
                    max_points,
                    min_level,
                ))
            },
        )
    }

    /// [`ToolpathDisplay::trace_lines`] as a job (issue 205): the first time
    /// it simplifies every run at the trace's level.
    pub fn start_trace_lines(
        &self,
        display: std::sync::Arc<ToolpathDisplay>,
        visible: Vec<bool>,
        max_points: usize,
    ) -> crate::JobHandle<PathLines> {
        self.jobs
            .submit("draw path trace", self.events_tx.clone(), move |context| {
                context.check_cancelled()?;
                Ok(display.trace_lines(
                    &|group| visible.get(group).copied().unwrap_or(false),
                    max_points,
                ))
            })
    }

    /// Chooses the lines of the toolpath to draw, as a job: the groups with
    /// `visible[group]` true, at the finest level of detail that fits
    /// `max_points` (item 8), or at `min_level` (the zoom's level,
    /// [`zoom_level`]) when that is coarser. Making a level the first time
    /// simplifies every run, which takes a while on a path of millions of
    /// moves.
    pub fn start_path_lines(
        &self,
        display: std::sync::Arc<ToolpathDisplay>,
        visible: Vec<bool>,
        max_points: usize,
        min_level: usize,
    ) -> crate::JobHandle<PathLines> {
        self.jobs.submit(
            "choose path detail",
            self.events_tx.clone(),
            move |context| {
                context.check_cancelled()?;
                let lines = display.lines_for(
                    &|group| visible.get(group).copied().unwrap_or(false),
                    max_points,
                    min_level,
                );
                context.check_cancelled()?;
                Ok(lines)
            },
        )
    }
}
