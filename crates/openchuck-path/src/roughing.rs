// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rotary roughing: bulk removal aware of the material left (OP-03; work
//! breakdown slice 14; decision record 0033).
//!
//! The stock is removed in **radial layers**, from the outside in. A layer
//! cuts with the tip at `max(level, floor)`, where `level` is the layer's
//! radius (measured along the tool axis) and `floor` is the lowest the tip may
//! go over the part with the stock to leave added. So the layers above the
//! part are flat, and where the part rises into a layer's band the tool
//! follows it, which leaves the stock to leave all over the part and
//! never more than one stepdown of material in a cut.
//!
//! - **Continuous** roughing makes each layer one helix about the rotary
//!   axis, A turning once every pitch of X (at most the stepover), like
//!   helical finishing, with the tool across the axis. A is continuous.
//! - **Indexed** (3+1) roughing holds A at fixed positions, `index_angle`
//!   apart, and cuts each layer as lines along X at steps of the stepover
//!   across the sector. A does not move while it cuts (OP-15).
//!
//! **Aware of the remaining stock.** The stock is read once into a
//! [`HeightStock`]; a node of a layer is cut only where more than `min_cut`
//! of material stands above the tool's underside, so material already gone
//! (carried from an earlier operation, or cut by an earlier layer) is never
//! cut again, and a second run over its own result cuts nothing. Each
//! layer's passes carry the field as it was when the layer started
//! ([`PassStock`]), so the shared services can enter, leave and link against
//! the stock that is really there (OP-14).
//!
//! The roughing covers the part's length along the rotary axis plus three
//! tool radii and the stock to leave at each end. Stock beyond that is not
//! touched.
//!
//! **Regions (OP-02, slice 14 item 5, decision 0043).** Roughing cuts away
//! from the surface, so a [`RegionConstraint`] is applied to the part under
//! the tool: a site (X, A and the offset across the axis) is attributed to the
//! cutter contact of the tool dropped on the whole part with its stock to
//! leave, the same contact at every layer. A site is cut only where
//! [`RegionConstraint::allows`] says that contact may be cut, so material
//! above the selected region, inside the boundary and off the avoid surfaces
//! is removed and the rest is left. The tool is still dropped on the whole
//! part, so it never gouges the unselected geometry beside the region. A
//! stretch of path between two cut nodes is kept only if every finer node
//! between them is allowed as well; otherwise the nodes are cut as separate
//! passes and the shared services link them.

use std::sync::Arc;

use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::cutter;
use crate::error::{PathError, Result};
use crate::field::HeightStock;
use crate::helical::CHORD_FACTOR;
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::rotary::{AxisPosition, RotaryMapping};
use crate::strategy::{CutPoint, Pass, PassStock, Strategy, StrategyInput};

/// The registry name of [`RotaryRoughing`].
pub const NAME: &str = "rotary_roughing";

/// Parameter names specific to rotary roughing.
pub mod parameter {
    /// `continuous` or `indexed`.
    pub const FORM: &str = "form";
    /// The depth of a layer, measured along the tool axis.
    pub const STEPDOWN: &str = "stepdown";
    /// The angle between index positions of the indexed form.
    pub const INDEX_ANGLE: &str = "index_angle";
    /// A of the first index position, or where the helix starts.
    pub const START_ANGLE: &str = "start_angle";
    /// The most A may turn between two points of a continuous layer.
    pub const MAX_ANGLE_STEP: &str = "max_angle_step";
    /// Material thinner than this above the tool is treated as air.
    pub const MIN_CUT: &str = "min_cut";
}

/// Fine nodes per coarse node: the path between two nodes is refined down to
/// a quarter of their spacing where the tool follows the part.
const SUB: usize = 4;

/// Halvings when the edge of a region is found between two nodes.
const EDGE_STEPS: usize = 12;

/// Most layers one operation may have.
const MAX_LAYERS: usize = 5_000;

/// Most nodes one layer may have.
const MAX_NODES: f64 = 2.0e6;

/// Where a line of nodes meets the edge of the allowed region: whether each
/// fine node is allowed, and the edge found between neighbours that differ.
/// Without a region, both are empty and a run is not extended.
struct RunEnds<'e> {
    allowed: &'e [bool],
    edges: &'e [Option<(Site, Option<f64>)>],
}

impl RunEnds<'_> {
    /// The edge point, if there is one between nodes `f` and `f + 1`.
    fn edge(&self, f: usize, level: f64) -> Option<(Site, f64)> {
        let (site, floor) = self.edges.get(f).copied().flatten()?;
        Some((site, floor.map_or(level, |v| v.max(level))))
    }

    /// The points before the run's first node `from` back to the edge of the
    /// region, if the region ends within a coarse step; in path order.
    fn before(
        &self,
        from: usize,
        level: f64,
        tip: &impl Fn(usize) -> f64,
        site: &impl Fn(usize) -> Site,
    ) -> Vec<(Site, f64)> {
        let mut out = Vec::new();
        for k in 1..=SUB {
            let Some(f) = from.checked_sub(k) else { break };
            match self.allowed.get(f) {
                Some(true) => out.push((site(f), tip(f))),
                Some(false) => {
                    // Between `f` (not allowed) and `f + 1` (allowed).
                    if let Some(edge) = self.edge(f, level) {
                        out.push(edge);
                        out.reverse();
                        return out;
                    }
                    break;
                }
                None => break,
            }
        }
        Vec::new()
    }

    /// The points after the run's last node `from` on to the edge of the
    /// region, if the region ends within a coarse step.
    fn after(
        &self,
        from: usize,
        level: f64,
        tip: &impl Fn(usize) -> f64,
        site: &impl Fn(usize) -> Site,
    ) -> Vec<(Site, f64)> {
        let mut out = Vec::new();
        for k in 1..=SUB {
            let f = from + k;
            match self.allowed.get(f) {
                Some(true) => out.push((site(f), tip(f))),
                Some(false) => {
                    if let Some(edge) = self.edge(f - 1, level) {
                        out.push(edge);
                        return out;
                    }
                    break;
                }
                None => break,
            }
        }
        Vec::new()
    }
}

/// Rotary roughing of the whole model: bulk removal down to the stock to
/// leave, in indexed and continuous forms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RotaryRoughing;

/// A number parameter. `range` is the lowest value, whether it is excluded,
/// and the highest value if any.
fn number(
    name: &str,
    label: &str,
    description: &str,
    unit: Unit,
    range: (f64, bool, Option<f64>),
    default: Option<f64>,
) -> ParameterSpec {
    let (min, min_exclusive, max) = range;
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Number {
            unit,
            min: Some(min),
            min_exclusive,
            max,
            default,
        },
    }
}

impl Strategy for RotaryRoughing {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Rotary roughing"
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(vec![
            ParameterSpec {
                name: parameter::FORM.to_owned(),
                label: "Form".to_owned(),
                description: "Continuous layers turn A as the tool cuts; indexed layers hold A at \
                              fixed positions (3+1)."
                    .to_owned(),
                kind: ParameterKind::Choice {
                    options: vec!["continuous".to_owned(), "indexed".to_owned()],
                    default: "continuous".to_owned(),
                },
            },
            number(
                parameter::STEPDOWN,
                "Stepdown",
                "The depth of each layer, along the tool axis. Layers are made equal, so the \
                 last is not a thin one.",
                Unit::Millimetres,
                (0.0, true, None),
                None,
            ),
            number(
                parameter::INDEX_ANGLE,
                "Index angle",
                "Indexed form: the angle between positions of A. The tool cuts the sector each \
                 position faces.",
                Unit::Degrees,
                (0.0, true, Some(180.0)),
                Some(30.0),
            ),
            ParameterSpec {
                name: parameter::START_ANGLE.to_owned(),
                label: "Start angle".to_owned(),
                description: "A of the first index position (indexed form) or at the start of \
                              the helix (continuous form)."
                    .to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: None,
                    min_exclusive: false,
                    max: None,
                    default: Some(0.0),
                },
            },
            number(
                parameter::MAX_ANGLE_STEP,
                "Largest angle step",
                "Continuous form: the most A may turn between two points of a layer.",
                Unit::Degrees,
                (0.0, true, Some(90.0)),
                Some(5.0),
            ),
            number(
                parameter::MIN_CUT,
                "Smallest cut",
                "Material standing less than this above the tool is treated as air and not cut; \
                 this also stops a second run from cutting what the first left. Keep it at \
                 least the simulation cell (0.1 mm by default), which is how finely the \
                 remaining stock is known.",
                Unit::Millimetres,
                (0.0, false, None),
                Some(0.15),
            ),
        ])
    }

    fn honours_region(&self) -> bool {
        true
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        rough(input, None, parameters, context)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        rough(input, Some(region), parameters, context)
    }
}

fn rough(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<Vec<Pass>> {
    let job = Job::new(input, region, parameters)?;
    match parameters.text(parameter::FORM)? {
        "indexed" => job.indexed(parameters, context),
        _ => job.continuous(parameters, context),
    }
    .and_then(|passes| {
        if passes.is_empty() {
            Err(PathError::Strategy(if job.region.is_some() {
                format!(
                    "rotary roughing has no cut to make for this selection: no material stands \
                     more than {} mm above the part with its stock to leave where the \
                     selection, boundary and avoid surfaces allow a cut",
                    job.min_cut
                )
            } else {
                format!(
                    "rotary roughing has no cut to make: no material stands more than {} mm \
                     above the part with its stock to leave",
                    job.min_cut
                )
            }))
        } else {
            Ok(passes)
        }
    })
}

/// One cut position: model X, A (degrees) and the offset across the axis.
#[derive(Debug, Clone, Copy)]
struct Site {
    x: f64,
    a: f64,
    across: f64,
}

struct Job<'a> {
    input: &'a StrategyInput<'a>,
    region: Option<&'a RegionConstraint<'a>>,
    rotary: RotaryMapping,
    grown: RevolvedProfile,
    radius: f64,
    leave: f64,
    stepover: f64,
    stepdown: f64,
    chord: f64,
    tolerance: f64,
    min_cut: f64,
    start: f64,
    r_top: f64,
    x_range: (f64, f64),
}

impl<'a> Job<'a> {
    fn new(
        input: &'a StrategyInput<'a>,
        region: Option<&'a RegionConstraint<'a>>,
        p: &Parameters,
    ) -> Result<Self> {
        let radius = input.tool.cutting_radius();
        let leave = p.number(names::STOCK_TO_LEAVE)?;
        let tolerance = p.number(names::TOLERANCE)?;
        let r_top = input.stock.max_radius().ok_or_else(|| {
            PathError::Strategy(
                "rotary roughing needs the stock's size, and the stock model gives none".to_owned(),
            )
        })?;
        let bounds = input.surface.bounds();
        // The tool starts clear of the part's ends, with room for an entry,
        // rather than tangent to them.
        let margin = 3.0 * radius + leave;
        let (x0, x1) = (bounds.min.x - margin, bounds.max.x + margin);
        if x0.is_nan() || x1.is_nan() || x1 <= x0 {
            return Err(PathError::Strategy(
                "the part has no length along the rotary axis to rough".to_owned(),
            ));
        }
        Ok(Self {
            input,
            region: region.filter(|r| !r.is_empty()),
            rotary: input.rotary,
            grown: cutter::grow_profile(input.tool, leave)?,
            radius,
            leave,
            stepover: p.number(names::STEPOVER)?,
            stepdown: p.number(parameter::STEPDOWN)?,
            chord: CHORD_FACTOR * tolerance,
            tolerance,
            min_cut: p.number(parameter::MIN_CUT)?,
            start: p.number(parameter::START_ANGLE)?,
            r_top,
            x_range: (x0, x1),
        })
    }

    /// The lowest the tool's tip may go at `site`, along the tool axis, with
    /// the stock to leave, and whether the region lets the site be cut. The
    /// floor is `None` where the part is not under the tool.
    fn probe(&self, site: Site) -> (Option<f64>, bool) {
        let line = self.rotary.tool_line(
            openchuck_types::units::Millimetres(site.x),
            Degrees(site.a),
            openchuck_types::units::Millimetres(site.across),
        );
        let Some(contact) = self.input.surface.drop_tool(&self.grown, &line) else {
            // Over no part: there is nothing to attribute the site to, so a
            // selected region excludes it, and a boundary decides by where it is.
            let allowed = self.region.is_none_or(|r| {
                r.region.is_none()
                    && r.boundary
                        .is_none_or(|b| b.contains(self.cut_point(site, self.r_top).tip))
            });
            // A check surface beyond the part still keeps the tool above it.
            let floor = self.region.and_then(|r| r.check).and_then(|check| {
                let touch = check.drop_tool(&self.grown, &line)?;
                Some(self.rotary.to_axis(touch.tip, Degrees(site.a)).height + self.leave)
            });
            return (floor, allowed);
        };
        let height = |tip| self.rotary.to_axis(tip, Degrees(site.a)).height + self.leave;
        let mut floor = height(contact.tip);
        let mut allowed = true;
        if let Some(region) = self.region {
            // A check surface keeps the tool above it rather than excluding
            // the site: the floor is the higher of the part's and the check
            // surface's (decision 0043 item 3).
            if let Some(check) = region.check
                && let Some(touch) = check.drop_tool(&self.grown, &line)
            {
                floor = floor.max(height(touch.tip));
            }
            let without_check = RegionConstraint {
                check: None,
                ..*region
            };
            allowed = without_check.allows(contact.contact, contact.tip, &self.grown, &line);
        }
        (Some(floor), allowed)
    }

    /// Where `allowed` changes between neighbouring fine nodes `f` and
    /// `f + 1`, the site between them found by bisection, with its floor.
    /// The edge of a region does not depend on the layer, so this is done
    /// once for a line of nodes. Empty without a region.
    fn edges(
        &self,
        allowed: &[bool],
        site: &impl Fn(f64) -> Site,
    ) -> Vec<Option<(Site, Option<f64>)>> {
        if self.region.is_none() {
            return Vec::new();
        }
        allowed
            .windows(2)
            .enumerate()
            .map(|(f, pair)| {
                let (&here, &next) = (pair.first()?, pair.get(1)?);
                if here == next {
                    return None;
                }
                // Keep `lo` on the allowed side and `hi` on the other.
                let (mut lo, mut hi) = if here {
                    (f as f64, (f + 1) as f64)
                } else {
                    ((f + 1) as f64, f as f64)
                };
                for _ in 0..EDGE_STEPS {
                    let mid = 0.5 * (lo + hi);
                    if self.probe(site(mid)).1 {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let at = site(lo);
                Some((at, self.probe(at).0))
            })
            .collect()
    }

    /// The layer levels from the top of the stock to `bottom`, equal steps no
    /// deeper than the stepdown.
    fn levels(&self, bottom: f64) -> Result<Vec<f64>> {
        let depth = self.r_top - bottom;
        if depth <= self.min_cut {
            return Err(PathError::Strategy(format!(
                "there is no material to rough: the stock reaches {:.3} mm and the part with its \
                 stock to leave reaches down to {bottom:.3} mm",
                self.r_top
            )));
        }
        let count = (depth / self.stepdown).ceil().max(1.0);
        if count > MAX_LAYERS as f64 {
            return Err(PathError::Parameter {
                name: parameter::STEPDOWN.to_owned(),
                reason: format!(
                    "{:.3} mm of depth in steps of {} mm is {count:.0} layers; the most is \
                     {MAX_LAYERS}",
                    depth, self.stepdown
                ),
            });
        }
        let step = depth / count;
        Ok((1..=count as usize)
            .map(|k| self.r_top - step * k as f64)
            .collect())
    }

    fn field(&self, bottom: f64) -> Result<HeightStock> {
        HeightStock::from_stock(
            self.input.stock,
            self.rotary,
            self.x_range,
            self.r_top,
            bottom.max(0.25 * self.radius),
            (self.radius / 3.0).max(0.05),
        )
    }

    fn cut_point(&self, site: Site, tip: f64) -> CutPoint {
        let p = self.rotary.to_model(
            AxisPosition {
                along: site.x,
                across: site.across,
                height: tip,
            },
            Degrees(site.a),
        );
        CutPoint {
            tip: p,
            a: Degrees(site.a),
            contact: p,
        }
    }

    /// Removes from `field` what the tool cuts along `points`, a point every
    /// half cell at most.
    fn apply(&self, field: &mut HeightStock, sites: &[(Site, f64)]) {
        let margin = self.tolerance;
        let spacing = 0.5 * field.cell_x();
        let mut previous: Option<(Site, f64)> = None;
        for &(site, tip) in sites {
            if let Some((p, ptip)) = previous {
                let gap = (site.x - p.x)
                    .hypot((site.across - p.across) + (site.a - p.a).to_radians() * self.r_top);
                let n = (gap / spacing).ceil().max(1.0) as usize;
                for k in 1..n {
                    let f = k as f64 / n as f64;
                    field.cut(
                        self.input.tool,
                        p.x + f * (site.x - p.x),
                        p.a + f * (site.a - p.a),
                        p.across + f * (site.across - p.across),
                        ptip + f * (tip - ptip),
                        margin,
                    );
                }
            }
            field.cut(self.input.tool, site.x, site.a, site.across, tip, margin);
            previous = Some((site, tip));
        }
    }

    /// The points of one run of coarse nodes `first..=last`: the nodes, with
    /// the path between them refined where the tool follows the part.
    ///
    /// Within a region the run is carried on to the edge of the allowed
    /// region at either end (`ends`), found by bisection, so a cut reaches
    /// the edge and not only the last node before it (OI-187).
    fn run_points(
        &self,
        run: (usize, usize),
        level: f64,
        floors: &[Option<f64>],
        site: &impl Fn(usize) -> Site,
        ends: &RunEnds<'_>,
    ) -> Vec<(Site, f64)> {
        let (first, last) = run;
        let tip = |f: usize| -> f64 {
            match floors.get(f).copied().flatten() {
                Some(v) => v.max(level),
                None => level,
            }
        };
        let mut out = ends.before(first * SUB, level, &tip, site);
        out.push((site(first * SUB), tip(first * SUB)));
        for i in first..last {
            self.refine(i * SUB, (i + 1) * SUB, &tip, site, &mut out);
        }
        out.extend(ends.after(last * SUB, level, &tip, site));
        out
    }

    fn refine(
        &self,
        lo: usize,
        hi: usize,
        tip: &impl Fn(usize) -> f64,
        site: &impl Fn(usize) -> Site,
        out: &mut Vec<(Site, f64)>,
    ) {
        if hi - lo >= 2 {
            let mid = (lo + hi) / 2;
            let lin = 0.5 * (tip(lo) + tip(hi));
            if (tip(mid) - lin).abs() > self.chord {
                self.refine(lo, mid, tip, site, out);
                self.refine(mid, hi, tip, site, out);
                return;
            }
        }
        out.push((site(hi), tip(hi)));
    }

    fn pass(&self, sites: &[(Site, f64)], stock: &PassStock) -> Pass {
        Pass {
            points: sites.iter().map(|(s, t)| self.cut_point(*s, *t)).collect(),
            stock: Some(stock.clone()),
        }
    }

    /// Splits the nodes `0..=count` into runs where `active` is true and
    /// `joined(i)` says the stretch from node `i` to the next may be cut.
    fn runs(active: &[bool], joined: impl Fn(usize) -> bool) -> Vec<(usize, usize)> {
        let mut runs = Vec::new();
        let mut start = None;
        for (i, on) in active.iter().enumerate() {
            match (on, start) {
                (true, None) => start = Some(i),
                (false, Some(s)) => {
                    runs.push((s, i - 1));
                    start = None;
                }
                (true, Some(s)) if !joined(i - 1) => {
                    runs.push((s, i - 1));
                    start = Some(i);
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            runs.push((s, active.len() - 1));
        }
        runs
    }

    fn continuous(&self, p: &Parameters, context: &JobContext<'_>) -> Result<Vec<Pass>> {
        let (x0, x1) = self.x_range;
        let length = x1 - x0;
        let turns = (length / self.stepover).ceil().max(1.0);
        let step = p.number(parameter::MAX_ANGLE_STEP)?;
        let per_turn = (360.0 / step).ceil().max(4.0);
        let nodes = turns * per_turn;
        if nodes > MAX_NODES {
            return Err(PathError::Parameter {
                name: names::STEPOVER.to_owned(),
                reason: format!(
                    "{:.3} mm of length at {} mm needs {nodes:.0} points per layer at {step} \
                     degrees; increase the stepover or the largest angle step",
                    length, self.stepover
                ),
            });
        }
        let coarse = nodes as usize;
        let fine = coarse * SUB;
        let site_at = move |f: f64| {
            let t = f / fine as f64;
            Site {
                x: x0 + length * t,
                a: self.start + 360.0 * turns * t,
                across: 0.0,
            }
        };
        let site = move |f: usize| site_at(f as f64);

        context.report("rotary roughing: finding the part", 0, Some(fine as u64));
        let indices: Vec<usize> = (0..=fine).collect();
        let probes: Vec<(Option<f64>, bool)> =
            parallel::map_in_order(&indices, |f| self.probe(site(*f)));
        let floors: Vec<Option<f64>> = probes.iter().map(|p| p.0).collect();
        let allowed: Vec<bool> = probes.iter().map(|p| p.1).collect();
        context.check_cancelled()?;
        let edges = self.edges(&allowed, &site_at);
        let ends = RunEnds {
            allowed: if edges.is_empty() { &[] } else { &allowed },
            edges: &edges,
        };
        let bottom = self.bottom(floors.iter().zip(&allowed))?;
        let levels = self.levels(bottom)?;
        let mut field = self.field(bottom)?;
        context.check_cancelled()?;

        let mut passes = Vec::new();
        let coarse_nodes: Vec<usize> = (0..=coarse).collect();
        for (k, level) in levels.iter().enumerate() {
            context.check_cancelled()?;
            context.report(
                "rotary roughing: layers",
                k as u64,
                Some(levels.len() as u64),
            );
            let snapshot = PassStock(Arc::new(field.clone()));
            let excess: Vec<f64> = parallel::map_in_order(&coarse_nodes, |i| {
                let s = site(i * SUB);
                let tip = floors
                    .get(i * SUB)
                    .copied()
                    .flatten()
                    .map_or(*level, |v| v.max(*level));
                field.excess(self.input.tool, s.x, s.a, s.across, tip)
            });
            let active: Vec<bool> = excess
                .iter()
                .enumerate()
                .map(|(i, e)| *e > self.min_cut && allowed.get(i * SUB).copied().unwrap_or(false))
                .collect();
            let mut layer = Vec::new();
            for (first, last) in Self::runs(&active, |i| Self::joined(&allowed, i)) {
                let sites = self.run_points((first, last), *level, &floors, &site, &ends);
                passes.push(self.pass(&sites, &snapshot));
                layer.push(sites);
            }
            for sites in &layer {
                self.apply(&mut field, sites);
            }
        }
        context.report(
            "rotary roughing: layers",
            levels.len() as u64,
            Some(levels.len() as u64),
        );
        Ok(passes)
    }

    fn indexed(&self, p: &Parameters, context: &JobContext<'_>) -> Result<Vec<Pass>> {
        let (x0, x1) = self.x_range;
        let length = x1 - x0;
        let index = p.number(parameter::INDEX_ANGLE)?;
        let positions = (360.0 / index).ceil().max(1.0) as usize;
        let half = (0.5 * index).min(90.0).to_radians();
        let reach = self.r_top * half.sin();
        let lines = (2.0 * reach / self.stepover).ceil().max(1.0) as usize;
        let spacing = (0.5 * self.radius).max(self.stepover.min(self.radius));
        let coarse = (length / spacing).ceil().max(1.0) as usize;
        let fine = coarse * SUB;
        if ((lines + 1) * positions * (fine + 1)) as f64 > MAX_NODES {
            return Err(PathError::Parameter {
                name: parameter::INDEX_ANGLE.to_owned(),
                reason: format!(
                    "{positions} positions of {} lines of {fine} points is too many; use a \
                     larger stepover or a larger index angle",
                    lines + 1
                ),
            });
        }
        let at = |position: usize, line: usize, f: f64| Site {
            x: x0 + length * f / fine as f64,
            a: self.start + index * position as f64,
            across: if lines == 0 {
                0.0
            } else {
                -reach + 2.0 * reach * line as f64 / lines as f64
            },
        };

        // The floor of every line of every position, once.
        let mut floors: Vec<Vec<Option<f64>>> = Vec::new();
        let mut allowed: Vec<Vec<bool>> = Vec::new();
        let indices: Vec<usize> = (0..=fine).collect();
        for position in 0..positions {
            for line in 0..=lines {
                context.check_cancelled()?;
                let probes: Vec<(Option<f64>, bool)> =
                    parallel::map_in_order(&indices, |f| self.probe(at(position, line, *f as f64)));
                floors.push(probes.iter().map(|p| p.0).collect());
                allowed.push(probes.iter().map(|p| p.1).collect());
            }
        }
        // The edge of the region on each line, once: it does not depend on the layer.
        let mut all_edges = Vec::new();
        for position in 0..positions {
            for line in 0..=lines {
                let site_at = |f: f64| at(position, line, f);
                let line_allowed = allowed
                    .get(position * (lines + 1) + line)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                all_edges.push(self.edges(line_allowed, &site_at));
            }
        }
        let bottom = self.bottom(floors.iter().flatten().zip(allowed.iter().flatten()))?;
        let levels = self.levels(bottom)?;
        let mut field = self.field(bottom)?;

        let coarse_nodes: Vec<usize> = (0..=coarse).collect();
        let mut passes = Vec::new();
        for (k, level) in levels.iter().enumerate() {
            let snapshot = PassStock(Arc::new(field.clone()));
            for position in 0..positions {
                for line in 0..=lines {
                    context.check_cancelled()?;
                    let line_floors = floors
                        .get(position * (lines + 1) + line)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let site_at = |f: f64| at(position, line, f);
                    let site = |f: usize| site_at(f as f64);
                    let excess: Vec<f64> = parallel::map_in_order(&coarse_nodes, |i| {
                        let s = site(i * SUB);
                        let tip = line_floors
                            .get(i * SUB)
                            .copied()
                            .flatten()
                            .map_or(*level, |v| v.max(*level));
                        field.excess(self.input.tool, s.x, s.a, s.across, tip)
                    });
                    let line_allowed = allowed
                        .get(position * (lines + 1) + line)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let edges = all_edges
                        .get(position * (lines + 1) + line)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let ends = RunEnds {
                        allowed: if edges.is_empty() { &[] } else { line_allowed },
                        edges,
                    };
                    let active: Vec<bool> = excess
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            *e > self.min_cut && line_allowed.get(i * SUB).copied().unwrap_or(false)
                        })
                        .collect();
                    for (first, last) in Self::runs(&active, |i| Self::joined(line_allowed, i)) {
                        let sites =
                            self.run_points((first, last), *level, line_floors, &site, &ends);
                        passes.push(self.pass(&sites, &snapshot));
                        self.apply(&mut field, &sites);
                    }
                }
            }
            context.report(
                "rotary roughing: layers",
                k as u64 + 1,
                Some(levels.len() as u64),
            );
        }
        Ok(passes)
    }

    /// Whether every fine node from coarse node `i` to the next may be cut.
    fn joined(allowed: &[bool], i: usize) -> bool {
        allowed
            .get(i * SUB..=(i + 1) * SUB)
            .is_some_and(|w| w.iter().all(|a| *a))
    }

    /// The lowest the tool goes where the region allows a cut: the lowest
    /// floor, never below the axis.
    fn bottom<'f>(&self, floors: impl Iterator<Item = (&'f Option<f64>, &'f bool)>) -> Result<f64> {
        floors
            .filter(|(_, allowed)| **allowed)
            .filter_map(|(floor, _)| *floor)
            .reduce(f64::min)
            .map(|b| b.max(0.0))
            .ok_or_else(|| {
                PathError::Strategy(if self.region.is_some() {
                    "the selection is nowhere under the tool, so there is no floor to rough \
                     down to"
                        .to_owned()
                } else {
                    "the part is nowhere under the tool, so there is no floor to rough down to"
                        .to_owned()
                })
            })
    }
}
