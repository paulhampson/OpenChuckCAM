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

use crate::notice::{Level, Notice, topic};
use std::collections::BTreeMap;

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
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

mod walls;

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
    /// The shape of the passes: a spiral or rings, as helical finishing's
    /// (decision records 0390 and 0420).
    pub const PASS_SHAPE: &str = crate::helical::parameter::PASS_SHAPE;
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
    cells: BTreeMap<[i64; 3], Vec<Sample>>,
}

/// A sample of the part with material left: where it is, the part's
/// outward normal there and the area of the part it stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// The point on the part's surface.
    pub point: Point3,
    /// The outward unit normal of the triangle it lies on.
    pub normal: Vec3,
    /// The share of the triangle's area it stands for, square millimetres.
    pub area: f64,
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
            return Err(PathError::refused(
                crate::topic::REST_MACHINING,
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
                return Err(PathError::refused(
                    crate::topic::REST_MACHINING,
                    format!(
                        "remaining-material detection needs at least one sample per triangle, and \
                     the part has {} triangles, more than the {MAX_SAMPLES:.0} allowed",
                        mesh.triangles.len()
                    ),
                ));
            }
            spacing *= 1.25;
        }
        context.report("rest machining: finding material left", 0, None);
        let found: Vec<(usize, Vec<Sample>)> = parallel::map_in_order(&mesh.triangles, |tri| {
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
            let area = 0.5 * ab.cross(ac).length() / (kf * kf);
            let mut tested = 0;
            let mut left = Vec::new();
            let mut visit = |u: f64, v: f64| {
                let p = a + ab * (u / kf) + ac * (v / kf);
                tested += 1;
                if probes.iter().all(|d| stock.contains(p + *d * probe)) {
                    left.push(Sample {
                        point: p,
                        normal: n,
                        area,
                    });
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
            for sample in left {
                map.count += 1;
                map.cells
                    .entry(map.key(sample.point))
                    .or_default()
                    .push(sample);
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
                        && points.iter().any(|q| q.point.distance(p) <= self.reach)
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
        self.cells.values().flatten().map(|s| s.point).collect()
    }

    /// The samples with material left, with their normals and areas, in the
    /// order of [`samples`](Self::samples).
    pub fn samples_with_normals(&self) -> Vec<Sample> {
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
                    description: "The most A may turn between two points of the path; points are \
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
                crate::helical::pass_shape_spec(
                    "Spiral: one continuous spiral that moves along the part as it turns. Rings: \
                     one full turn at a time at a fixed place along the part, with a short step \
                     along the part between turns, at the start angle. Either way only the \
                     places with material left are cut.",
                ),
            ]
            .into_iter()
            .chain(crate::helical::ring_step_specs())
            .chain(crate::finishing::specs(true, false))
            .chain(std::iter::once(crate::order::spec()))
            .chain(std::iter::once(crate::axial::link_height_spec()))
            .collect(),
        )
        })
    }

    /// Steps between rings only with Rings (decision record 0545).
    fn schema_for(
        &self,
        values: &std::collections::BTreeMap<String, crate::params::ParameterValue>,
    ) -> ParameterSchema {
        crate::helical::ring_step_schema_for(self.schema(), values)
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
            return Err(PathError::refused(
                crate::topic::REGION,
                format!("the strategy '{NAME}' cannot yet be limited to a region"),
            ));
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
            return Err(PathError::refused(
                crate::topic::REST_MACHINING,
                "the part has no length along the rotary axis to clean".to_owned(),
            ));
        }

        let spacing = detection_spacing(input.tool, stepover);
        // A flat bottom touches a floor anywhere under it, so its contact
        // can be a whole flat's width from the material it cuts: the reach
        // spans it too (decision record 0505; 0 for a round end).
        let flat = input
            .tool
            .point_for_normal(-std::f64::consts::FRAC_PI_2 + 1e-6)
            .map_or(input.tool.cutting_radius(), |q| q.radius.0);
        let reach = 0.5 * stepover + spacing + 2.0 * flat;
        let probe = leave + min_remaining;
        let map = RemainingMap::detect(input.surface, input.stock, probe, spacing, reach, context)?;
        if map.is_empty() {
            return Err(PathError::refused(
                crate::topic::REST_MACHINING,
                nothing_left("rest machining", min_remaining, leave, &map),
            )
            .with_setting(parameter::MIN_REMAINING));
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
        // The faces across the axis, where the tool's side touches them
        // (decision record 0297; issue 404).
        let found = faces::turns_to_the_ends(input, &grown, (x0, x1), tolerance, context)?;
        let turns: Vec<f64> = found.turns.iter().map(|t| t.x).collect();
        // The walls with material left, where the tool's side meets them
        // (decision record 0505; issue #508).
        let walls = walls::Walls {
            grown: &grown,
            span: (x0, x1),
            stepover,
            target: min_remaining.max(tolerance),
            smallest: tolerance.max(MIN_WALL_SPACING),
            tolerance,
            min_remaining,
        };
        let samples = map.samples_with_normals();
        let (wall_rings, capped) = if walls.modelled() {
            walls.rings(&samples, map.spacing(), input.keep_out)
        } else {
            (Vec::new(), false)
        };
        // A wall ring where a face's turn already is would cut it twice.
        let wall_rings: Vec<f64> = wall_rings
            .into_iter()
            .filter(|w| turns.iter().all(|t| (t - w).abs() >= walls.smallest))
            .collect();
        let mut ring_note = None;
        let mut order_note = None;
        let mut passes = if crate::helical::in_rings(parameters) {
            // Rings and the steps between them, each cut only where material
            // is left; a step with nothing to cut is left to the shared
            // services to link (decision record 0420; OP-14). A face's turn
            // is one of the rings.
            let places: Vec<f64> = turns.iter().chain(&wall_rings).copied().collect();
            // Over the part outside the held regions' bands, with a ring
            // at each band's edge (decision record 0550).
            let (span, inside) = crate::helical::ring_span((x0, x1), input.keep_out, &places);
            let (xs, gap) = crate::helical::ring_places(span, stepover, &inside);
            let blend = crate::helical::ring_blend(parameters)?;
            let (passes, _) = crate::helical::ring_passes(
                &fit,
                &xs,
                step,
                blend,
                false,
                "rest machining",
                context,
            )?;
            let text = match blend {
                None => format!(
                    "rest machining cut rings no more than {gap:.4} mm apart, each a whole turn \
                     at one place along the part, joined by steps along it at A {start}, where \
                     material is left"
                ),
                Some(_) => {
                    let b = input.surface.bounds();
                    let radius = [b.min.y, b.max.y, b.min.z, b.max.z]
                        .into_iter()
                        .fold(0.0_f64, |r, v| r.max(v.abs()));
                    format!(
                        "rest machining cut rings no more than {gap:.4} mm apart, each a whole \
                         turn at one place along the part, where material is left{}",
                        crate::helical::blend_words(blend, gap, radius)
                    )
                }
            };
            ring_note = Some(Notice::information(text).about(topic::RINGS));
            passes
        } else {
            let passes = fit_region(&fit, seeds, "rest machining", context)?;
            // One whole turn at each face across the axis and at each wall
            // ring, cut where material is left, as helical finishing does,
            // after the helix or beside its region (decision record 0520).
            let mut places: Vec<f64> = turns.iter().chain(&wall_rings).copied().collect();
            places.sort_by(f64::total_cmp);
            let mut groups = Vec::with_capacity(places.len());
            for &x in &places {
                context.check_cancelled()?;
                let at = start + sign * 360.0 * (x - x0) / stepover;
                groups.push(crate::helical::turn_passes(
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
            let arranged = crate::order::arrange(
                parameters.text(crate::order::PARAMETER)?,
                passes,
                groups,
                crate::order::Rates::of(&input.max_feeds, input.clearance.radius.0),
                parameters.number(names::FEED)?,
            );
            order_note = arranged.note;
            arranged.passes
        };
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
            return Err(PathError::refused(
                crate::topic::REST_MACHINING,
                "rest machining found material left but the tool cannot reach any of it: the \
                 tool is too large for the places that need cleaning"
                    .to_owned(),
            ));
        }
        let mut notes = vec![
            Notice::information(format!(
                "{} of {} points sampled on the part have more than {} mm of \
                 material left; {} passes cut where it is left",
                map.len(),
                map.tested(),
                min_remaining,
                passes.len()
            ))
            .about(topic::REST_MACHINING),
        ];
        notes.extend(ring_note);
        if let Some(note) = wall_note(&wall_rings, walls.target) {
            notes.push(note);
        }
        let mut warnings = free.warnings;
        if capped {
            warnings.push(Notice {
                level: Level::Warning,
                topic: Some(topic::MATERIAL_LEFT.to_owned()),
                segment: None,
                text: format!(
                    "rest machining placed the most rings it may at walls ({}): walls further \
                     along may keep more than {} mm between rings; check them in the simulation",
                    walls::MAX_WALL_RINGS,
                    walls.target
                ),
                setting: Some(parameter::MIN_REMAINING.to_owned()),
                acknowledgement: None,
            });
        }
        let unreached =
            if walls.modelled() {
                walls.unreached(input, &samples, map.spacing(), context)?
            } else {
                // Uncut areas go unreported, so the user must see it (decision
                // record 0536 step 2).
                warnings.push(Notice::warning(
                "this tool's profile is not convex, so rest machining places no rings at walls \
                 and does not check what the tool cannot reach",
            ).about(topic::UNREACHED_AREAS));
                Vec::new()
            };
        warnings.extend(unreached_warning(
            &unreached,
            2.0 * input.tool.cutting_radius(),
        ));
        notes.extend(faces::note(&turns));
        notes.extend(order_note);
        warnings.extend(faces::held_warning(&found.held));
        notes.extend(free.notes);
        // Every pass may be linked low when asked (decision 0521).
        crate::axial::mark_low_links_if_asked(parameters, &mut passes);
        Ok(StrategyOutput {
            passes,
            notices: crate::strategy::notices(warnings, notes),
            unreached,
            shank_breaks: Vec::new(),
        })
    }
}

/// The closest two rings at walls may be, millimetres (with the toolpath
/// tolerance if that is larger).
const MIN_WALL_SPACING: f64 = 0.01;

/// The note for the rings added at walls; `None` with none.
fn wall_note(rings: &[f64], target: f64) -> Option<Notice> {
    let closest = rings
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some(b - a),
            _ => None,
        })
        .fold(f64::INFINITY, f64::min);
    let (first, last) = (rings.first()?, rings.last()?);
    let apart = if closest.is_finite() {
        format!(", as close as {closest:.4} mm apart,")
    } else {
        String::new()
    };
    Some(
        Notice::information(format!(
            "rest machining added {} rings between X {first:.3} and {last:.3}{apart} where its \
             tool's side meets a wall with material left, so that a steep wall keeps about \
             {target} mm or less between them",
            rings.len()
        ))
        .about(topic::RINGS),
    )
}

/// The warning for the material the tool cannot reach (decision record
/// 0505, in the words of OP-20's report); `None` with none.
fn unreached_warning(areas: &[crate::assemble::Unreached], diameter: f64) -> Option<Notice> {
    if areas.is_empty() {
        return None;
    }
    let plural = if areas.len() == 1 { "" } else { "s" };
    Some(Notice {
        level: Level::Warning,
        topic: Some(topic::UNREACHED_AREAS.to_owned()),
        segment: None,
        text: format!(
            "rest machining cannot reach {} area{plural} of the part where material is left, \
             which stay uncut: {}. Check them in the simulation; a smaller tool or another \
             operation may reach them. Generated G-code must be verified by the user before it \
             is run on a machine.",
            areas.len(),
            crate::simultaneous::unreached_list(areas, diameter, 0.0)
        ),
        setting: None,
        acknowledgement: None,
    })
}
