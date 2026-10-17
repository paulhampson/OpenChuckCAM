// SPDX-License-Identifier: AGPL-3.0-or-later
//! Radial drilling (OP-10; work breakdown slice 18; decision record 0051):
//! holes found from cylindrical faces, A positions and depths, pecking, and
//! the order of the holes. The blank is built from closed forms by
//! `openchuck_testkit::brep`, so each expected number is derived here from
//! the hole's definition, not read back from the code under test.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepSurface, CompositeSurface, GeneratedPath, HoleEnd, MeshSurface,
    ParameterValue, Pass, PathError, RadialDrilling, Registry, RotaryMapping, Strategy,
    StrategyInput, StrategyOutput, SurfaceSource, find_holes, generate,
};
use openchuck_testkit::brep::{Built, HoleSpec, drilled_blank};
use openchuck_types::brep::{BrepModel, FaceId};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::{MoveKind, PathTags};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{ARange, Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

const RADIUS: f64 = 12.0;
const LENGTH: f64 = 60.0;
const DIAMETER: f64 = 3.0;

/// The height of a 118 degree point of this diameter.
fn point_height() -> f64 {
    0.5 * DIAMETER / (59.0_f64).to_radians().tan()
}

/// The unit direction at polar angle `phi` degrees from +Y towards +Z.
fn polar(phi: f64) -> [f64; 3] {
    [0.0, phi.to_radians().cos(), phi.to_radians().sin()]
}

fn drill(diameter: f64) -> RevolvedProfile {
    Tool {
        shape: ToolShape::Drill {
            point_angle: Degrees(118.0),
        },
        diameter: Millimetres(diameter),
        flute_length: Millimetres(30.0),
        overall_length: Millimetres(60.0),
        shank_diameter: Millimetres(diameter),
        flute_count: 2,
        material: None,
        centre_cutting: Some(true),
    }
    .profile()
    .unwrap()
}

struct Rig {
    built: Vec<Built>,
    surface: BrepSurface,
    stock: AnalyticStock,
    machine: MachineProfile,
    tags: PathTags,
}

impl Rig {
    fn new(specs: &[HoleSpec]) -> Self {
        let (model, built) = drilled_blank(RADIUS, LENGTH, specs);
        Self::of(&model, built)
    }

    fn of(model: &BrepModel, built: Vec<Built>) -> Self {
        Self {
            built,
            surface: BrepSurface::new(model).unwrap(),
            stock: AnalyticStock::Cylinder {
                radius: RADIUS,
                x_start: 0.0,
                x_end: LENGTH,
            },
            machine: MachineProfile::example_x_rotary(),
            tags: PathTags {
                operation: "op".to_owned(),
                setup: "setup".to_owned(),
                tool: "tool".to_owned(),
            },
        }
    }

    fn input<'a>(
        &'a self,
        surface: &'a dyn SurfaceSource,
        tool: &'a RevolvedProfile,
    ) -> StrategyInput<'a> {
        StrategyInput {
            surface,
            stock: &self.stock,
            collision: &AcceptAll,
            tool,
            rotary: RotaryMapping::of_profile(&self.machine),
            clearance: Clearance {
                radius: Millimetres(17.0),
                safe_z: Millimetres(27.0),
            },
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
        }
    }

    fn output(
        &self,
        tool: &RevolvedProfile,
        v: &BTreeMap<String, ParameterValue>,
    ) -> openchuck_path::Result<StrategyOutput> {
        let parameters = RadialDrilling.schema().resolve(v)?;
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        RadialDrilling.passes_with_notes(&self.input(&self.surface, tool), None, &parameters, &ctx)
    }

    fn path(
        &self,
        tool: &RevolvedProfile,
        v: &BTreeMap<String, ParameterValue>,
    ) -> openchuck_path::Result<GeneratedPath> {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        generate(&RadialDrilling, &self.input(&self.surface, tool), v, &ctx)
    }
}

fn values(extra: &[(&str, f64)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::FEED, 120.0),
        (names::PLUNGE_FEED, 600.0),
        (names::APPROACH_DISTANCE, 1.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), ParameterValue::Number(*x));
    }
    v
}

fn text(v: &mut BTreeMap<String, ParameterValue>, k: &str, word: &str) {
    v.insert(k.to_owned(), ParameterValue::Text(word.to_owned()));
}

/// The part's A for a hole facing polar angle `phi`, from the machine model
/// (a positive A turns the part counter-clockwise about +X): the hole faces
/// the tool when its polar angle plus A is 90 degrees.
fn expected_a(phi: f64) -> f64 {
    (90.0 - phi).rem_euclid(360.0)
}

/// A model point as it stands in the machine (about the rotary axis) at A
/// degrees: (along, across, height).
fn machine_of(p: Point3, a: f64) -> (f64, f64, f64) {
    let (s, c) = a.to_radians().sin_cos();
    (p.x, p.y * c - p.z * s, p.y * s + p.z * c)
}

fn close(p: Point3, q: Point3) -> bool {
    p.distance(q) < 1e-9
}

fn at(entry: Point3, d: Vec3, depth: f64) -> Point3 {
    entry + d * -depth
}

const SEVEN: [(f64, f64, f64); 7] = [
    // x, polar angle of the hole, depth
    (10.0, 0.0, 8.0),
    (20.0, 90.0, 6.0),
    (15.0, 90.0, 5.0),
    (30.0, 20.0, 0.0),
    (5.0, 20.0, 0.0),
    (40.0, 315.0, 10.0),
    (50.0, 135.0, 7.0),
];

fn several() -> Vec<HoleSpec> {
    SEVEN
        .iter()
        .enumerate()
        .map(|(i, (x, phi, depth))| {
            let mut spec = if *depth == 0.0 {
                HoleSpec::through(*x, polar(*phi), DIAMETER)
            } else {
                HoleSpec::blind(*x, polar(*phi), DIAMETER, *depth)
            };
            // One with a drilled point, one with a split wall.
            spec.point = i == 1;
            spec.split = i == 2;
            spec
        })
        .collect()
}

#[test]
fn req_op_10_holes_are_found_from_cylindrical_faces_with_axis_diameter_and_depth() {
    let specs = several();
    let (model, built) = drilled_blank(RADIUS, LENGTH, &specs);
    let holes = find_holes(&model);
    assert_eq!(holes.len(), specs.len());
    for (hole, b) in holes.iter().zip(&built) {
        assert_eq!(hole.faces, b.walls);
        assert!((hole.diameter - DIAMETER).abs() < 1e-9);
        assert!((hole.length - b.length).abs() < 1e-9, "{hole:?} {b:?}");
        match hole.end {
            HoleEnd::Blind { tip_depth } => {
                assert!(
                    (tip_depth - b.tip_depth).abs() < 1e-9,
                    "{tip_depth} against {}",
                    b.tip_depth
                );
            }
            HoleEnd::Through => assert!((b.tip_depth - b.length).abs() < 1e-9),
            _ => panic!("unexpected end"),
        }
        let d = hole.direction.get();
        // A through hole is entered from the end that faces up; the others
        // from the open end, which the definition fixes.
        let through = matches!(hole.end, HoleEnd::Through);
        if through {
            assert!(d.z > 0.0 || (d.z.abs() < 1e-9 && d.y > 0.0), "{d:?}");
            let same = (d - b.direction).length() < 1e-9;
            let opposite = (d + b.direction).length() < 1e-9;
            assert!(same || opposite);
        } else {
            assert!((d - b.direction).length() < 1e-9);
            assert!(close(hole.entry, b.entry), "{:?} {:?}", hole.entry, b.entry);
        }
    }
    // A through hole is the whole way across the blank.
    let across = holes.iter().find(|h| matches!(h.end, HoleEnd::Through));
    let across = across.unwrap();
    assert!((across.length - 2.0 * RADIUS).abs() < 1e-9);
}

#[test]
fn req_op_10_a_convex_cylinder_and_a_mesh_have_no_holes() {
    let (blank, _) = drilled_blank(RADIUS, LENGTH, &[]);
    assert!(find_holes(&blank).is_empty());
    assert!(BrepSurface::new(&blank).unwrap().holes().is_empty());
    let mesh = MeshSurface::new(&TriangleMesh {
        vertices: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ],
        triangles: vec![[0, 1, 2]],
    })
    .unwrap();
    assert!(mesh.holes().is_empty());
    let rig = Rig::new(&[HoleSpec::blind(10.0, polar(90.0), DIAMETER, 5.0)]);
    let tool = drill(DIAMETER);
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let parameters = RadialDrilling.schema().resolve(&values(&[])).unwrap();
    let error = RadialDrilling
        .passes(&rig.input(&mesh, &tool), &parameters, &ctx)
        .unwrap_err()
        .to_string();
    assert!(error.contains("no hole to drill"), "{error}");
}

#[test]
fn req_op_10_overlapping_partial_walls_are_not_a_full_round_hole() {
    let mut spec = HoleSpec::blind(10.0, polar(90.0), DIAMETER, 5.0);
    spec.split = true;
    let (mut model, built) = drilled_blank(RADIUS, LENGTH, &[spec]);
    assert_eq!(find_holes(&model).len(), 1);
    // Both halves now cover the same half turn: together they are half a hole.
    let first = model.faces[built[0].walls[0].0 as usize].uv_bounds;
    model.faces[built[0].walls[1].0 as usize].uv_bounds = first;
    assert!(find_holes(&model).is_empty());
}

#[test]
fn req_op_10_a_face_selection_picks_the_holes_and_the_composite_keeps_them() {
    let specs = several();
    let (model, built) = drilled_blank(RADIUS, LENGTH, &specs);
    // Any face of a hole's wall selects it; the split hole needs only one half.
    let split = &built[2].walls;
    assert_eq!(split.len(), 2);
    let chosen: Vec<FaceId> = vec![built[0].walls[0], split[1]];
    let surface = BrepSurface::with_faces(&model, &chosen).unwrap();
    let holes = surface.holes();
    assert_eq!(holes.len(), 2);
    assert_eq!(holes[0].faces, built[0].walls);
    assert_eq!(holes[1].faces, built[2].walls);
    // The ends were still found with the whole model's neighbours.
    assert!(matches!(holes[0].end, HoleEnd::Blind { .. }));
    let composite = CompositeSurface::new(std::sync::Arc::new(surface), Vec::new());
    assert_eq!(composite.holes().len(), 2);
}

/// The drilling passes grouped by hole, in order, with each hole matched to
/// what was built.
fn passes_by_hole<'a>(rig: &'a Rig, passes: &'a [Pass]) -> Vec<(&'a Built, &'a Pass)> {
    passes
        .iter()
        .map(|p| {
            let x = p.points[0].tip.x;
            let b = rig
                .built
                .iter()
                .find(|b| (b.entry.x - x).abs() < 0.2)
                .unwrap();
            (b, p)
        })
        .collect()
}

#[test]
fn req_op_10_holes_at_several_angles_have_the_right_a_position_and_depth() {
    let rig = Rig::new(&several());
    let tool = drill(DIAMETER);
    let out = rig.output(&tool, &values(&[])).unwrap();
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    assert_eq!(out.passes.len(), SEVEN.len());
    for (b, pass) in passes_by_hole(&rig, &out.passes) {
        // The hole entered from the up-facing end if through; the blind ones
        // from the built entry.
        let blind = (b.tip_depth - b.length).abs() > 1e-9 || b.length < 2.0 * RADIUS - 1e-9;
        let first = pass.points.first().unwrap();
        let last = pass.points.last().unwrap();
        let depth = b.length + point_height() + if blind { 0.0 } else { 0.5 };
        // The tool axis is the way out of the hole: the retract point and the
        // bottom lie on one line, 2 mm above the opening and `depth` below it.
        let line = first.tip - last.tip;
        assert!((line.length() - (depth + 2.0)).abs() < 1e-9, "{line:?}");
        let up = line * (1.0 / line.length());
        if blind {
            assert!((up - b.direction).length() < 1e-9);
            assert!(close(first.tip, b.entry + b.direction * 2.0));
            assert!(close(last.tip, at(b.entry, b.direction, depth)));
        }
        assert_eq!(first.a.0, last.a.0);
        // In the machine the hole axis is vertical at A: the same across
        // position, and the bottom lower.
        let (fx, fy, fz) = machine_of(first.tip, first.a.0);
        let (lx, ly, lz) = machine_of(last.tip, last.a.0);
        assert!((fx - lx).abs() < 1e-9 && (fy - ly).abs() < 1e-9);
        assert!((fz - lz - (depth + 2.0)).abs() < 1e-9);
        if blind {
            let phi = b.direction.z.atan2(b.direction.y).to_degrees();
            let a = first.a.0.rem_euclid(360.0);
            let want = expected_a(phi);
            let gap = ((a - want + 180.0).rem_euclid(360.0) - 180.0).abs();
            assert!(gap < 1e-9, "A {a} against {want} for polar angle {phi}");
        }
        // A drilled hole is on the axis line of the model's hole.
        assert!(pass.rapids.is_empty());
    }
}

#[test]
fn req_op_10_angles_are_visited_in_one_sweep_after_the_largest_gap() {
    let rig = Rig::new(&several());
    let tool = drill(DIAMETER);
    let out = rig.output(&tool, &values(&[])).unwrap();
    let angles: Vec<f64> = out.passes.iter().map(|p| p.points[0].a.0).collect();
    // A is 0 for the two holes facing +Z, 70 for the through holes (they
    // are entered from the up-facing end), 90, 135 and 315 for the rest. The
    // largest gap is 135 to 315 (180 degrees), so the sweep starts at 315
    // and rises through the turn to 495, 180 degrees in all.
    assert!(angles.windows(2).all(|w| w[0] <= w[1]), "{angles:?}");
    assert!((angles[0] - 315.0).abs() < 1e-6, "{angles:?}");
    assert!(
        (angles[angles.len() - 1] - 495.0).abs() < 1e-6,
        "{angles:?}"
    );
    // Holes at one angle are cut together along X, alternating direction
    // from one angle to the next.
    let along = |a: f64| -> Vec<f64> {
        out.passes
            .iter()
            .filter(|p| (p.points[0].a.0 - a).abs() < 1e-6)
            .map(|p| p.points[0].tip.x)
            .collect()
    };
    assert_eq!(along(360.0), vec![20.0, 15.0], "descending at the second");
    assert_eq!(along(430.0), vec![5.0, 30.0], "ascending at the third");
}

#[test]
fn req_op_10_op_15_a_limited_range_is_swept_from_its_low_end_or_refused() {
    let mut rig = Rig::new(&several());
    rig.machine.a_range = ARange::Limited {
        min: Degrees(-10.0),
        max: Degrees(330.0),
    };
    let tool = drill(DIAMETER);
    let out = rig.output(&tool, &values(&[])).unwrap();
    let angles: Vec<f64> = out.passes.iter().map(|p| p.points[0].a.0).collect();
    assert!(angles.windows(2).all(|w| w[0] <= w[1]));
    assert!(angles.iter().all(|a| (-10.0..=330.0).contains(a)));
    // A hole that needs A = 250 is out of reach: refused, naming it.
    let rig = {
        let mut r = Rig::new(&[HoleSpec::blind(10.0, polar(200.0), DIAMETER, 5.0)]);
        r.machine.a_range = ARange::Limited {
            min: Degrees(-10.0),
            max: Degrees(200.0),
        };
        r
    };
    let error = rig.output(&tool, &values(&[])).unwrap_err();
    assert!(matches!(error, PathError::RotaryLimit(_)), "{error:?}");
    assert!(error.to_string().contains("250.000"), "{error}");
}

#[test]
fn req_op_10_a_hole_off_the_rotary_axis_is_drilled_with_the_tool_offset() {
    let mut spec = HoleSpec::blind(25.0, polar(90.0), DIAMETER, 6.0);
    spec.offset = 4.0;
    let rig = Rig::new(&[spec]);
    let tool = drill(DIAMETER);
    let out = rig.output(&tool, &values(&[])).unwrap();
    let p = &out.passes[0];
    let b = &rig.built[0];
    // The line of the hole is the tool line: the bottom is `depth` along the
    // hole's axis from where the axis meets the surface.
    let depth = b.length + point_height();
    assert!(close(p.points[1].tip, at(b.entry, b.direction, depth)));
    let (_, across, _) = machine_of(p.points[1].tip, p.points[1].a.0);
    assert!((across.abs() - 4.0).abs() < 1e-9, "{across}");
}

#[test]
fn req_op_10_peck_and_retract_heights_are_correct_and_stay_clear_of_the_part() {
    let rig = Rig::new(&[HoleSpec::blind(30.0, polar(90.0), DIAMETER, 10.0)]);
    let tool = drill(DIAMETER);
    let b = &rig.built[0];
    let total = b.length + point_height();
    let depth_of = |p: &openchuck_path::CutPoint| (b.entry - p.tip).dot(b.direction);

    // Full retract: three equal pecks no deeper than 4 mm.
    let mut v = values(&[(openchuck_path::drill::parameter::PECK_DEPTH, 4.0)]);
    let out = rig.output(&tool, &v).unwrap();
    let pass = &out.passes[0];
    let depths: Vec<f64> = pass.points.iter().map(|p| -depth_of(p)).collect();
    let q = total / 3.0;
    let want = [
        2.0,        // the retract height above the opening
        -q,         // first peck
        2.0,        // out
        -(q - 0.5), // back in, 0.5 above the last depth
        -2.0 * q,
        2.0,
        -(2.0 * q - 0.5),
        -3.0 * q,
    ];
    assert_eq!(depths.len(), want.len(), "{depths:?}");
    for (got, want) in depths.iter().zip(want) {
        assert!((got - want).abs() < 1e-9, "{depths:?}");
    }
    assert_eq!(pass.rapids, vec![2, 3, 5, 6]);
    assert!(q <= 4.0);
    // Never above the retract height.
    assert!(depths.iter().all(|d| *d <= 2.0 + 1e-9));

    // Chip breaking: back out 0.5 and feed on.
    text(
        &mut v,
        openchuck_path::drill::parameter::PECK_MODE,
        "chip_break",
    );
    let out = rig.output(&tool, &v).unwrap();
    let pass = &out.passes[0];
    let depths: Vec<f64> = pass.points.iter().map(|p| -depth_of(p)).collect();
    let want = [2.0, -q, -(q - 0.5), -2.0 * q, -(2.0 * q - 0.5), -3.0 * q];
    assert_eq!(depths.len(), want.len(), "{depths:?}");
    for (got, want) in depths.iter().zip(want) {
        assert!((got - want).abs() < 1e-9, "{depths:?}");
    }
    assert_eq!(pass.rapids, vec![2, 4]);
}

#[test]
fn req_op_10_the_expanded_moves_are_rapid_retracts_and_fed_pecks() {
    let rig = Rig::new(&[HoleSpec::blind(30.0, polar(90.0), DIAMETER, 10.0)]);
    let tool = drill(DIAMETER);
    let v = values(&[(openchuck_path::drill::parameter::PECK_DEPTH, 4.0)]);
    let path = rig.path(&tool, &v).unwrap();
    let b = &rig.built[0];
    let total = b.length + point_height();
    let segments = &path.group.segments;
    // Cutting moves are the pecks, at the cutting feed; the retracts and
    // returns inside the hole are rapids.
    let cuts: Vec<_> = segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .collect();
    assert_eq!(cuts.len(), 3);
    let q = total / 3.0;
    let first = cuts[0].contact_length.0;
    assert!((first - (2.0 + q)).abs() < 1e-6, "{first}");
    for cut in &cuts[1..] {
        assert!((cut.contact_length.0 - (q + 0.5)).abs() < 1e-6);
        assert_eq!(cut.contact_feed.unwrap().0, 120.0);
        assert!((cut.duration.0 - 60.0 * (q + 0.5) / 120.0).abs() < 0.05);
    }
    // The deepest the tool goes is the hole's depth, and no cut is deeper.
    let bottom = at(b.entry, b.direction, total);
    let deepest = cuts
        .iter()
        .map(|s| s.end.tip)
        .min_by(|p, q| (*p - bottom).length().total_cmp(&(*q - bottom).length()))
        .unwrap();
    assert!(close(deepest, bottom), "{deepest:?} {bottom:?}");
    // Within the hole the rapids stay on the axis between the retract height
    // and the bottom.
    let on_axis = |p: Point3| {
        let v = p - b.entry;
        (v + b.direction * -v.dot(b.direction)).length() < 1e-6
    };
    for s in segments.iter().filter(|s| on_axis(s.end.tip)) {
        let height = (s.end.tip - b.entry).dot(b.direction);
        assert!(height <= 2.0 + 1e-9 || s.kind == MoveKind::Rapid || s.kind == MoveKind::Linking);
        assert!(height >= -total - 1e-9);
    }
    // The tool leaves the hole upwards, never across the part: links are at
    // the clearance radius.
    for s in segments {
        let r = s.end.tip.y.hypot(s.end.tip.z);
        if !on_axis(s.end.tip) {
            assert!(r >= 17.0 - 1e-9 || s.kind == MoveKind::Rapid, "{s:?}");
        }
    }
}

#[test]
fn req_op_10_stock_above_the_opening_raises_the_retract_height() {
    let mut rig = Rig::new(&[HoleSpec::blind(30.0, polar(90.0), DIAMETER, 5.0)]);
    rig.stock = AnalyticStock::Cylinder {
        radius: RADIUS + 3.0,
        x_start: 0.0,
        x_end: LENGTH,
    };
    let tool = drill(DIAMETER);
    let out = rig.output(&tool, &values(&[])).unwrap();
    let b = &rig.built[0];
    let p = &out.passes[0];
    // The stock stands 3 mm above the opening: the retract height is 2 mm
    // above that, and the bottom is where the model's hole ends.
    assert!(close(p.points[0].tip, b.entry + b.direction * 5.0));
    assert!(close(
        p.points[1].tip,
        at(b.entry, b.direction, 5.0 + point_height())
    ));
}

#[test]
fn req_op_10_blind_depth_may_be_to_the_tip_and_a_point_is_found_in_the_model() {
    let mut spec = HoleSpec::blind(30.0, polar(90.0), DIAMETER, 6.0);
    spec.point = true;
    let rig = Rig::new(&[spec]);
    let tool = drill(DIAMETER);
    let b = &rig.built[0];
    let mut v = values(&[]);
    let full = rig.output(&tool, &v).unwrap();
    assert!(close(
        full.passes[0].points[1].tip,
        at(b.entry, b.direction, b.length + point_height())
    ));
    text(
        &mut v,
        openchuck_path::drill::parameter::DEPTH_REFERENCE,
        "tip",
    );
    let tip = rig.output(&tool, &v).unwrap();
    // The model's cone ends at its apex, a point's height below the full
    // diameter: the tip of a matching drill is there.
    assert!(close(
        tip.passes[0].points[1].tip,
        at(b.entry, b.direction, b.tip_depth)
    ));
    assert!((b.tip_depth - b.length - point_height()).abs() < 1e-9);
}

#[test]
fn req_op_10_holes_the_tool_cannot_drill_are_left_out_with_a_note() {
    let mut tilted = HoleSpec::blind(40.0, [0.2, 0.0, 1.0], DIAMETER, 4.0);
    tilted.offset = 0.0;
    let specs = [
        HoleSpec::blind(10.0, polar(90.0), DIAMETER, 5.0),
        HoleSpec::blind(20.0, polar(0.0), 5.0, 5.0),
        tilted,
    ];
    let rig = Rig::new(&specs);
    let tool = drill(DIAMETER);
    let out = rig.output(&tool, &values(&[])).unwrap();
    assert_eq!(out.passes.len(), 1);
    assert_eq!(out.notes.len(), 2, "{:?}", out.notes);
    assert!(
        out.notes
            .iter()
            .any(|n| n.contains("hole 2") && n.contains("diameter"))
    );
    assert!(
        out.notes
            .iter()
            .any(|n| n.contains("hole 3") && n.contains("radial plane"))
    );
    // A larger tolerance drills the 5 mm hole as a pilot.
    let v = values(&[(openchuck_path::drill::parameter::DIAMETER_TOLERANCE, 3.0)]);
    let out = rig.output(&tool, &v).unwrap();
    assert_eq!(out.passes.len(), 2);
    // With nothing drillable the error says why.
    let big = drill(8.0);
    let error = rig.output(&big, &values(&[])).unwrap_err().to_string();
    assert!(error.contains("no hole can be drilled"), "{error}");
}

#[test]
fn req_op_10_the_schema_is_uniform_and_the_strategy_is_registered() {
    let registry = Registry::builtin();
    let strategy = registry.get("radial_drilling").unwrap();
    let schema = strategy.schema();
    for common in [
        names::TOLERANCE,
        names::FEED,
        names::PLUNGE_FEED,
        names::RETRACT,
        names::APPROACH_DISTANCE,
        names::ENTRY,
    ] {
        assert!(schema.get(common).is_some(), "{common}");
    }
    // A drill enters straight down: plunge is the only entry.
    let entry = schema.get(names::ENTRY).unwrap();
    assert!(matches!(
        &entry.kind,
        openchuck_path::ParameterKind::Choice { options, .. } if options == &vec!["plunge".to_owned()]
    ));
    // Defaults resolve with only the feeds given.
    schema.resolve(&values(&[])).unwrap();
    let bad = values(&[("peck_depth", -1.0)]);
    let error = schema.resolve(&bad).unwrap_err().to_string();
    assert!(error.contains("peck_depth"), "{error}");
    assert!(!strategy.honours_region());
}

#[test]
fn req_nfr_06_req_op_10_drilling_is_identical_at_1_2_and_n_threads() {
    let rig = Rig::new(&several());
    let tool = drill(DIAMETER);
    let v = values(&[(openchuck_path::drill::parameter::PECK_DEPTH, 3.0)]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = rig.path(&tool, &v).unwrap();
        format!("{:?}", path.group).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_nfr_07_an_empty_or_cancelled_drilling_job_is_a_clear_error() {
    let rig = Rig::new(&[HoleSpec::blind(10.0, polar(90.0), DIAMETER, 5.0)]);
    let tool = drill(DIAMETER);
    let cancel = CancelToken::new();
    cancel.cancel();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let error = generate(
        &RadialDrilling,
        &rig.input(&rig.surface, &tool),
        &values(&[]),
        &ctx,
    )
    .unwrap_err();
    assert!(matches!(error, PathError::Cancelled));
}
