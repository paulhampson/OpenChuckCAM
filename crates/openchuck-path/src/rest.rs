// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rest machining: clean up the material an earlier, larger tool could not
//! reach (OP-09; work breakdown slice 15; decision record 0044).
//!
//! **Remaining material** is found from the stock the setup has when the
//! operation starts, which is the simulated stock the earlier operations left
//! (SIM-06), through the [`StockModel`] port, and the target surface. The
//! surface is sampled no further apart than a fraction of the tool's size; a
//! sample has material left when the stock holds the point `threshold` above
//! it (plus the stock to leave) along the normal and in four directions tilted
//! 50 degrees from it. That approximates the **closest distance** to empty
//! space the simulation's deviation reports (decision record 0025 item 6): a
//! steep wall does not read as a large height difference, and a corner that
//! an earlier tool rounded off reads as thin material, as the report shows
//! it, not as the thick column of stock a probe along one normal would find.
//!
//! **The strategy** is helical finishing with a smaller tool, cut only where
//! the remaining-material map says there is something to take: a point of the
//! helix is cut when a sample with material left lies within half a stepover
//! of its cutter contact. The limits of the wanted stretches are found by
//! bisection, so a pass starts and ends where the material does and the
//! shared services link the pieces (OP-14). The tool is dropped on the whole
//! part, so cutting only part of the helix cannot gouge the rest.
//!
//! The carried stock is what makes the operation "rest": with no earlier
//! operation in the setup the stock is the setup's own definition, and the
//! whole part has material left. The application says so
//! ([`Strategy::needs_earlier_operations`]).

use std::collections::BTreeMap;

use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;

use crate::cutter;
use crate::error::{PathError, Result};
use crate::faces;
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::stock::StockModel;
use crate::strategy::{Pass, Strategy, StrategyInput, StrategyOutput};
use crate::surface::SurfaceSource;
use crate::wrap_fit::{CHORD_FACTOR, Fit, fit_region};

/// The registry name of [`RestMachining`].
pub const NAME: &str = "rest_machining";

/// Parameter names specific to rest machining.
pub mod parameter {
    /// Material thinner than this above the part (and its stock to leave) is
    /// treated as clean.
    pub const MIN_REMAINING: &str = "min_remaining";
    /// Which way A turns as X increases: `positive` or `negative`.
    pub const ROTATION: &str = "rotation";
    /// A at the start of the helix, degrees.
    pub const START_ANGLE: &str = "start_angle";
    /// The largest A step between points, degrees.
    pub const MAX_ANGLE_STEP: &str = "max_angle_step";
}

/// The default of [`parameter::MIN_REMAINING`], in millimetres: more than one
/// simulation cell at the default 0.1 mm, because the stock is only known to
/// a cell (decision record 0033 item 5, OI-152).
pub const DEFAULT_MIN_REMAINING: f64 = 0.15;

/// Most surface samples one detection may make; the spacing grows to fit.
const MAX_SAMPLES: f64 = 2.0e6;

/// Most points one strategy run may make (NFR-07).
const MAX_SEEDS: f64 = 5.0e6;

/// The surface sampled for material left on it: the places an operation
/// still has to cut.
#[derive(Debug, Clone)]
pub struct RemainingMap {
    reach: f64,
    spacing: f64,
    tested: usize,
    count: usize,
    cells: BTreeMap<[i64; 3], Vec<Point3>>,
}

impl RemainingMap {
    /// Samples `surface` no further apart than `spacing` (larger when that
    /// would pass the limit on samples) and keeps those that have material
    /// left: where `stock` holds the five points `probe` mm out along the
    /// normal and tilted from it.
    /// `reach` is how near a point must be to a kept sample for
    /// [`near`](Self::near) to say yes.
    pub fn detect(
        surface: &dyn SurfaceSource,
        stock: &dyn StockModel,
        probe: f64,
        spacing: f64,
        reach: f64,
        context: &JobContext<'_>,
    ) -> Result<Self> {
        if !(probe.is_finite() && probe >= 0.0 && spacing > 0.0 && reach > 0.0) {
            return Err(PathError::Strategy(
                "remaining-material detection needs a threshold of zero or more and a \
                 positive spacing and reach"
                    .to_owned(),
            ));
        }
        let mesh = surface.tessellation();
        let corner = |i: u32| mesh.vertices.get(i as usize).copied();
        let mut spacing = spacing;
        let subdivisions = |tri: &[u32; 3], spacing: f64| -> usize {
            let [Some(a), Some(b), Some(c)] = tri.map(corner) else {
                return 0;
            };
            let longest = a.distance(b).max(b.distance(c)).max(c.distance(a));
            (longest / spacing).ceil().clamp(1.0, 400.0) as usize
        };
        loop {
            context.check_cancelled()?;
            let total: f64 = mesh
                .triangles
                .iter()
                .map(|t| {
                    let k = subdivisions(t, spacing) as f64;
                    k * k
                })
                .sum();
            if total <= MAX_SAMPLES {
                break;
            }
            // Every triangle is already down to one sample: a coarser
            // spacing cannot help, and fewer samples than triangles would
            // miss whole faces.
            if total <= mesh.triangles.len() as f64 {
                return Err(PathError::Strategy(format!(
                    "remaining-material detection needs at least one sample per triangle, and \
                     the part has {} triangles, more than the {MAX_SAMPLES:.0} allowed",
                    mesh.triangles.len()
                )));
            }
            spacing *= 1.25;
        }
        context.report("rest machining: finding material left", 0, None);
        let found: Vec<(usize, Vec<Point3>)> = parallel::map_in_order(&mesh.triangles, |tri| {
            let [Some(a), Some(b), Some(c)] = tri.map(corner) else {
                return (0, Vec::new());
            };
            let Ok(normal) = UnitVec3::new((b - a).cross(c - a)) else {
                return (0, Vec::new());
            };
            let n = normal.get();
            // Directions to look for empty space in: along the normal and
            // tilted 50 degrees from it towards each side along and across
            // the face.
            let t1 = UnitVec3::new(b - a).map_or(n, |t| t.get());
            let t2 = n.cross(t1);
            let (up, side) = (50.0_f64.to_radians().cos(), 50.0_f64.to_radians().sin());
            let probes = [
                n,
                n * up + t1 * side,
                n * up + t1 * -side,
                n * up + t2 * side,
                n * up + t2 * -side,
            ];
            let k = subdivisions(tri, spacing);
            let (ab, ac) = (b - a, c - a);
            let kf = k as f64;
            let mut tested = 0;
            let mut left = Vec::new();
            let mut visit = |u: f64, v: f64| {
                let p = a + ab * (u / kf) + ac * (v / kf);
                tested += 1;
                if probes.iter().all(|d| stock.contains(p + *d * probe)) {
                    left.push(p);
                }
            };
            for i in 0..k {
                for j in 0..(k - i) {
                    visit(i as f64 + 1.0 / 3.0, j as f64 + 1.0 / 3.0);
                    if i + j + 1 < k {
                        visit(i as f64 + 2.0 / 3.0, j as f64 + 2.0 / 3.0);
                    }
                }
            }
            (tested, left)
        });
        context.check_cancelled()?;
        let mut map = Self {
            reach,
            spacing,
            tested: 0,
            count: 0,
            cells: BTreeMap::new(),
        };
        for (tested, left) in found {
            map.tested += tested;
            for p in left {
                map.count += 1;
                map.cells.entry(map.key(p)).or_default().push(p);
            }
        }
        Ok(map)
    }

    fn key(&self, p: Point3) -> [i64; 3] {
        let k = |v: f64| (v / self.reach).floor() as i64;
        [k(p.x), k(p.y), k(p.z)]
    }

    /// Whether a sample with material left lies within the reach of `p`.
    pub fn near(&self, p: Point3) -> bool {
        let [kx, ky, kz] = self.key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(points) = self.cells.get(&[kx + dx, ky + dy, kz + dz])
                        && points.iter().any(|q| q.distance(p) <= self.reach)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// How many samples have material left.
    pub fn len(&self) -> usize {
        self.count
    }

    /// True when no sample has material left: there is nothing to clean.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// How many samples were tested.
    pub fn tested(&self) -> usize {
        self.tested
    }

    /// The spacing of the samples, in millimetres.
    pub fn spacing(&self) -> f64 {
        self.spacing
    }

    /// The samples with material left, in a deterministic order (by their
    /// cell, then as they were found).
    pub fn samples(&self) -> Vec<Point3> {
        self.cells.values().flatten().copied().collect()
    }
}

/// The distance between samples for a tool and a stepover: fine enough that
/// a strip a quarter of the tool's radius wide is seen.
pub(crate) fn detection_spacing(tool: &RevolvedProfile, stepover: f64) -> f64 {
    (0.5 * stepover).min(0.5 * tool.cutting_radius()).max(0.02)
}

/// A sentence for a clean part, shared by the rest and pencil strategies.
pub(crate) fn nothing_left(strategy: &str, probe: f64, leave: f64, map: &RemainingMap) -> String {
    format!(
        "{strategy} has nothing to clean: none of {} points sampled on the part has more than \
         {probe} mm of material above it with its stock to leave of {leave} mm. Run it after \
         the operation whose leftovers it should cut, or lower {}",
        map.tested(),
        parameter::MIN_REMAINING
    )
}

/// The common description of the threshold, for the schemas.
pub(crate) fn min_remaining_spec() -> ParameterSpec {
    ParameterSpec {
        name: parameter::MIN_REMAINING.to_owned(),
        label: "Smallest material to clean".to_owned(),
        description: "Material thinner than this above the part (and its stock to leave) is \
                      taken as clean. The stock is only known to a simulation cell, so keep \
                      this above one and a half cells. 0 treats all of the part as having \
                      material left."
            .to_owned(),
        kind: ParameterKind::Number {
            unit: Unit::Millimetres,
            min: Some(0.0),
            min_exclusive: false,
            max: None,
            default: Some(DEFAULT_MIN_REMAINING),
        },
    }
}

/// Rest machining of the whole model: helical finishing where earlier
/// operations left material.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestMachining;

impl Strategy for RestMachining {
    /// Exact contacts on a STEP part for a ball-end tool, on by default:
    /// measured within the limits of OI-835 (decision 0277).
    fn exact_contact(&self) -> Option<crate::contact::ExactContact> {
        Some(crate::contact::ExactContact::Ball)
    }

    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Rest machining"
    }

    fn description(&self) -> &str {
        "Uses the stock left by earlier operations in the same setup to find and cut remaining material. Use a smaller tool to reach areas a larger tool left behind, such as inside corners. The remaining material is estimated from the stock simulation."
    }

    fn schema(&self) -> ParameterSchema {
        crate::contact::with_switch(crate::contact::ExactContact::Ball, {
            ParameterSchema::with_common(
                vec![
                    min_remaining_spec(),
                    ParameterSpec {
                        name: parameter::ROTATION.to_owned(),
                        label: "Rotation".to_owned(),
                        description: "Which way A turns as X increases along the helix.".to_owned(),
                        kind: ParameterKind::Choice {
                            options: vec!["positive".to_owned(), "negative".to_owned()],
                            default: "positive".to_owned(),
                        },
                    },
                    ParameterSpec {
                        name: parameter::START_ANGLE.to_owned(),
                        label: "Start angle".to_owned(),
                        description: "A at the lowest X of the part, where the helix starts."
                            .to_owned(),
                        kind: ParameterKind::Number {
                            unit: Unit::Degrees,
                            min: None,
                            min_exclusive: false,
                            max: None,
                            default: Some(0.0),
                        },
                    },
                    ParameterSpec {
                        name: parameter::MAX_ANGLE_STEP.to_owned(),
                        label: "Largest angle step".to_owned(),
                        description:
                            "The most A may turn between two points of the path; points are \
                              added where the surface needs them."
                                .to_owned(),
                        kind: ParameterKind::Number {
                            unit: Unit::Degrees,
                            min: Some(0.0),
                            min_exclusive: true,
                            max: Some(90.0),
                            default: Some(2.0),
                        },
                    },
                ]
                .into_iter()
                .chain(crate::finishing::specs(true, false))
                .collect(),
            )
        })
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
        let stepover = parameters.number(names::STEPOVER)?;
        let tolerance = parameters.number(names::TOLERANCE)?;
        let leave = parameters.number(names::STOCK_TO_LEAVE)?;
        let min_remaining = parameters.number(parameter::MIN_REMAINING)?;
        let sign = if parameters.text(parameter::ROTATION)? == "negative" {
            -1.0
        } else {
            1.0
        };
        let start = parameters.number(parameter::START_ANGLE)?;
        let step = parameters.number(parameter::MAX_ANGLE_STEP)?;

        let bounds = input.surface.bounds();
        let (x0, x1) = (bounds.min.x, bounds.max.x);
        let length = x1 - x0;
        if length.is_nan() || length <= 0.0 {
            return Err(PathError::Strategy(
                "the part has no length along the rotary axis to clean".to_owned(),
            ));
        }

        let spacing = detection_spacing(input.tool, stepover);
        let reach = 0.5 * stepover + spacing;
        let probe = leave + min_remaining;
        let map = RemainingMap::detect(input.surface, input.stock, probe, spacing, reach, context)?;
        if map.is_empty() {
            return Err(PathError::Strategy(nothing_left(
                "rest machining",
                min_remaining,
                leave,
                &map,
            )));
        }

        let turns = length / stepover;
        let total_angle = 360.0 * turns;
        let seeds = (total_angle / step).ceil().max(1.0);
        if seeds > MAX_SEEDS {
            return Err(PathError::Parameter {
                name: names::STEPOVER.to_owned(),
                reason: format!(
                    "{stepover} mm over {length:.1} mm of part needs {seeds:.0} points at {step} \
                     degrees; increase the stepover or the largest angle step"
                ),
            });
        }
        let seeds = seeds as usize;
        let grown = cutter::grow_profile(input.tool, leave)?;
        let wanted = |contact: Point3| map.near(contact);
        let fit = Fit {
            input,
            drop_profile: &grown,
            leave,
            zones: &[],
            x0,
            length,
            start,
            sign,
            total_angle,
            chord: CHORD_FACTOR * tolerance,
            region: None,
            constrained: true,
            candidates: None,
            spacing: tolerance,
            thin: false,
            max_dt: 1.0 / seeds as f64,
            wanted: Some(&wanted),
            across: 0.0,
        };
        let mut passes = fit_region(&fit, seeds, "rest machining", context)?;
        // One whole turn at each face across the axis, where the tool's side
        // touches it, cut where material is left, as helical finishing does
        // (decision record 0297; issue 404).
        let found = faces::turns_to_the_ends(input, &grown, (x0, x1), tolerance, context)?;
        let turns: Vec<f64> = found.turns.iter().map(|t| t.x).collect();
        for &x in &turns {
            context.check_cancelled()?;
            let at = start + sign * 360.0 * (x - x0) / stepover;
            passes.extend(crate::helical::turn_passes(
                input,
                None,
                (&grown, leave, &[]),
                tolerance,
                (x, at, sign),
                step,
                Some(&wanted),
                context,
            )?);
        }
        // The free ends, in layers down beside them (decision record 0344).
        let free = crate::finishing::free_ends(
            input,
            &grown,
            &found.free,
            parameters,
            (start, sign, step),
        )?;
        passes.extend(free.passes);
        if passes.is_empty() {
            return Err(PathError::Strategy(
                "rest machining found material left but the tool cannot reach any of it: the \
                 tool is too large for the places that need cleaning"
                    .to_owned(),
            ));
        }
        let mut notes = vec![format!(
            "rest machining: {} of {} points sampled on the part have more than {} mm of \
             material left; {} passes clean them",
            map.len(),
            map.tested(),
            min_remaining,
            passes.len()
        )];
        notes.extend(faces::note(&turns));
        notes.extend(faces::held_note(&found.held));
        notes.extend(free.notes);
        Ok(StrategyOutput {
            passes,
            notes,
            warnings: free.warnings,
            unreached: Vec::new(),
            shank_breaks: Vec::new(),
        })
    }
}
