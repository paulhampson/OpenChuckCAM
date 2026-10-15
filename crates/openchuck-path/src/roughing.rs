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
//! tool radii and the stock to leave at each end. Stock beyond that is not touched, and there is no
//! containment to selections yet (OP-02, slice 14 item 5).

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

/// Most layers one operation may have.
const MAX_LAYERS: usize = 5_000;

/// Most nodes one layer may have.
const MAX_NODES: f64 = 2.0e6;

/// Rotary roughing of the whole model: bulk removal down to the stock to
/// leave, in indexed and continuous forms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RotaryRoughing;

fn number(
    name: &str,
    label: &str,
    description: &str,
    unit: Unit,
    min: f64,
    min_exclusive: bool,
    max: Option<f64>,
    default: Option<f64>,
) -> ParameterSpec {
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
                0.0,
                true,
                None,
                None,
            ),
            number(
                parameter::INDEX_ANGLE,
                "Index angle",
                "Indexed form: the angle between positions of A. The tool cuts the sector each \
                 position faces.",
                Unit::Degrees,
                0.0,
                true,
                Some(180.0),
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
                0.0,
                true,
                Some(90.0),
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
                0.0,
                false,
                None,
                Some(0.15),
            ),
        ])
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let job = Job::new(input, parameters)?;
        match parameters.text(parameter::FORM)? {
            "indexed" => job.indexed(parameters, context),
            _ => job.continuous(parameters, context),
        }
    }
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
    fn new(input: &'a StrategyInput<'a>, p: &Parameters) -> Result<Self> {
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
    /// the stock to leave; `None` where the part is not under the tool.
    fn floor(&self, site: Site) -> Option<f64> {
        let line = self.rotary.tool_line(
            openchuck_types::units::Millimetres(site.x),
            Degrees(site.a),
            openchuck_types::units::Millimetres(site.across),
        );
        let contact = self.input.surface.drop_tool(&self.grown, &line)?;
        Some(self.rotary.to_axis(contact.tip, Degrees(site.a)).height + self.leave)
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
    fn run_points(
        &self,
        first: usize,
        last: usize,
        level: f64,
        floors: &[Option<f64>],
        site: &impl Fn(usize) -> Site,
    ) -> Vec<(Site, f64)> {
        let tip = |f: usize| -> f64 {
            match floors.get(f).copied().flatten() {
                Some(v) => v.max(level),
                None => level,
            }
        };
        let mut out = vec![(site(first * SUB), tip(first * SUB))];
        for i in first..last {
            self.refine(i * SUB, (i + 1) * SUB, &tip, site, &mut out);
        }
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

    /// Splits the nodes `0..=count` into runs where `active` is true.
    fn runs(active: &[bool]) -> Vec<(usize, usize)> {
        let mut runs = Vec::new();
        let mut start = None;
        for (i, on) in active.iter().enumerate() {
            match (on, start) {
                (true, None) => start = Some(i),
                (false, Some(s)) => {
                    runs.push((s, i - 1));
                    start = None;
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
        let site = move |f: usize| {
            let t = f as f64 / fine as f64;
            Site {
                x: x0 + length * t,
                a: self.start + 360.0 * turns * t,
                across: 0.0,
            }
        };

        context.report("rotary roughing: finding the part", 0, Some(fine as u64));
        let indices: Vec<usize> = (0..=fine).collect();
        let floors: Vec<Option<f64>> = parallel::map_in_order(&indices, |f| self.floor(site(*f)));
        context.check_cancelled()?;
        let bottom = self.bottom(&floors)?;
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
            let active: Vec<bool> = excess.iter().map(|e| *e > self.min_cut).collect();
            let mut layer = Vec::new();
            for (first, last) in Self::runs(&active) {
                let sites = self.run_points(first, last, *level, &floors, &site);
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
        let at = |position: usize, line: usize, f: usize| Site {
            x: x0 + length * f as f64 / fine as f64,
            a: self.start + index * position as f64,
            across: if lines == 0 {
                0.0
            } else {
                -reach + 2.0 * reach * line as f64 / lines as f64
            },
        };

        // The floor of every line of every position, once.
        let mut floors: Vec<Vec<Option<f64>>> = Vec::new();
        let indices: Vec<usize> = (0..=fine).collect();
        for position in 0..positions {
            for line in 0..=lines {
                context.check_cancelled()?;
                floors.push(parallel::map_in_order(&indices, |f| {
                    self.floor(at(position, line, *f))
                }));
            }
        }
        let flat: Vec<Option<f64>> = floors.iter().flatten().copied().collect();
        let bottom = self.bottom(&flat)?;
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
                    let site = |f: usize| at(position, line, f);
                    let excess: Vec<f64> = parallel::map_in_order(&coarse_nodes, |i| {
                        let s = site(i * SUB);
                        let tip = line_floors
                            .get(i * SUB)
                            .copied()
                            .flatten()
                            .map_or(*level, |v| v.max(*level));
                        field.excess(self.input.tool, s.x, s.a, s.across, tip)
                    });
                    let active: Vec<bool> = excess.iter().map(|e| *e > self.min_cut).collect();
                    for (first, last) in Self::runs(&active) {
                        let sites = self.run_points(first, last, *level, line_floors, &site);
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

    /// The lowest the tool goes: the lowest floor, never below the axis.
    fn bottom(&self, floors: &[Option<f64>]) -> Result<f64> {
        floors
            .iter()
            .flatten()
            .copied()
            .reduce(f64::min)
            .map(|b| b.max(0.0))
            .ok_or_else(|| {
                PathError::Strategy(
                    "the part is nowhere under the tool, so there is no floor to rough down to"
                        .to_owned(),
                )
            })
    }
}
