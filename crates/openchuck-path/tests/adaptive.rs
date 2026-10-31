// SPDX-License-Identifier: AGPL-3.0-or-later
//! Adaptive pass spacing (OP-03, OP-04, OP-05, NFR-06, NFR-07, NFR-12;
//! issue #303; decision record 0239).
//!
//! The material left is measured independently of the strategies: the test
//! kit's coverage check finds, over the exact surface of each part, the
//! farthest point from the tool's outline at every fed pose of the generated
//! path, on the machine's straight moves.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::adaptive::parameter::{ADAPTIVE_SPACING, MIN_SPACING};
use openchuck_path::finishing::parameter::FACE_FREE_ENDS;
use openchuck_path::params::{ParameterKind, names};
use openchuck_path::rotary::AxisPosition;
use openchuck_path::simultaneous::parameter::PATTERN;
use openchuck_path::{
    AxialFinishing, GeneratedPath, HelicalFinishing, IndexedFinishing, MeshSurface, ParameterValue,
    RotaryMapping, SimultaneousFinishing, Strategy, generate,
};
use openchuck_testkit::gouge::{Outline, Pose, coverage, min_clearance};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::MoveKind;
use openchuck_types::units::Degrees;
use support::{Fixture, ball, machine, run};

const STRATEGIES: [&dyn Strategy; 2] = [&HelicalFinishing, &AxialFinishing];

fn label_and_default(
    strategy: &dyn Strategy,
    values: &BTreeMap<String, ParameterValue>,
) -> (String, Option<f64>, bool) {
    let schema = strategy.schema_for(values);
    let height = schema.get("scallop").unwrap();
    let default = match height.kind {
        ParameterKind::Number { default, .. } => default,
        _ => None,
    };
    (
        height.label.clone(),
        default,
        schema.get(MIN_SPACING).is_some(),
    )
}

#[test]
fn req_op_04_adaptive_spacing_is_on_for_a_new_operation_with_one_height_setting() {
    for s in STRATEGIES {
        let spec = s.schema();
        let on = spec.get(ADAPTIVE_SPACING).unwrap();
        assert_eq!(
            on.kind,
            ParameterKind::Flag { default: true },
            "{}",
            s.name()
        );
        // On (a new operation): the height is the Allowed excess, 0.01 mm.
        let (label, default, spacing) = label_and_default(s, &BTreeMap::new());
        assert_eq!(label, "Allowed excess", "{}", s.name());
        assert_eq!(default, Some(0.01), "{}", s.name());
        assert!(spacing, "{}", s.name());
        // Off: the Scallop height as before, 0 mm, and no Smallest spacing.
        let off = BTreeMap::from([(ADAPTIVE_SPACING.to_owned(), ParameterValue::Flag(false))]);
        let (label, default, spacing) = label_and_default(s, &off);
        assert_eq!(label, "Scallop height", "{}", s.name());
        assert_eq!(default, Some(0.0), "{}", s.name());
        assert!(!spacing, "{}", s.name());
    }
}

/// A part with its mesh, finished with a 4 mm ball.
struct Part<P> {
    part: P,
    fixture: Fixture,
}

fn cylinder() -> Part<Cylinder> {
    let part = Cylinder {
        radius: 10.0,
        length: 12.0,
    };
    let mesh = part.mesh(49, 720);
    Part {
        part,
        fixture: Fixture::new(MeshSurface::new(&mesh).unwrap(), 12.0, &ball(4.0)),
    }
}

fn cone() -> Part<Cone> {
    let part = Cone {
        start_radius: 5.0,
        end_radius: 11.0,
        length: 12.0,
    };
    let mesh = part.mesh(97, 720);
    Part {
        part,
        fixture: Fixture::new(MeshSurface::new(&mesh).unwrap(), 13.0, &ball(4.0)),
    }
}

fn values(
    strategy: &dyn Strategy,
    extra: &[(&str, ParameterValue)],
) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    for (k, x) in [
        (names::STEPOVER, 3.0),
        (names::TOLERANCE, 0.002),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.0),
    ] {
        v.insert(k.to_owned(), ParameterValue::Number(x));
    }
    if strategy.schema().get(FACE_FREE_ENDS).is_some() {
        v.insert(FACE_FREE_ENDS.to_owned(), ParameterValue::Flag(false));
    }
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn generated<P>(
    p: &Part<P>,
    strategy: &dyn Strategy,
    extra: &[(&str, ParameterValue)],
) -> GeneratedPath {
    run(|ctx| generate(strategy, &p.fixture.input(), &values(strategy, extra), ctx)).unwrap()
}

/// The tool's poses along the cutting moves, each move cut into `k` straight
/// steps on the machine, or more where A turns more than a degree a step.
fn poses(path: &GeneratedPath, k: u32) -> Vec<Pose> {
    let rotary = RotaryMapping::of_profile(&machine());
    let mut out = Vec::new();
    let mut from = path.group.start;
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            let a = rotary.to_axis(from.tip, from.a);
            let b = rotary.to_axis(s.end.tip, s.end.a);
            let k = k.max((s.end.a.0 - from.a.0).abs().ceil() as u32);
            for i in 1..=k {
                let t = f64::from(i) / f64::from(k);
                let lerp = |p: f64, q: f64| p + t * (q - p);
                let angle = Degrees(lerp(from.a.0, s.end.a.0));
                let tip = rotary.to_model(
                    AxisPosition {
                        along: lerp(a.along, b.along),
                        across: lerp(a.across, b.across),
                        height: lerp(a.height, b.height),
                    },
                    angle,
                );
                out.push(Pose {
                    tip,
                    axis: rotary.tool_axis(angle).get(),
                });
            }
        }
        from = s.end;
    }
    out
}

/// The most material left on the exact surface of `part` from X `x0` to
/// `x1`: the farthest a sample is from the tool at every fed pose.
fn left<P: RadialPart>(p: &Part<P>, path: &GeneratedPath, (x0, x1): (f64, f64)) -> f64 {
    let outline = Outline::new(&p.fixture.tool);
    coverage(&outline, &p.part, &poses(path, 8), (x0, x1), 0.1, 0.0).worst
}

fn has_note(path: &GeneratedPath, words: &str) -> bool {
    path.notes.iter().any(|n| n.message.contains(words))
}

#[test]
fn req_op_04_req_op_05_axial_passes_are_added_until_the_allowed_excess_is_met() {
    let p = cylinder();
    let off = generated(
        &p,
        &AxialFinishing,
        &[(ADAPTIVE_SPACING, ParameterValue::Flag(false))],
    );
    let on = generated(&p, &AxialFinishing, &[]);
    let (before, after) = (left(&p, &off, (2.0, 10.0)), left(&p, &on, (2.0, 10.0)));
    // A 3 mm stepover leaves about 0.5 mm with a 4 mm ball; the default
    // Allowed excess is 0.01 mm, the measure's sampling adds under 0.002.
    assert!(before > 0.3, "off left {before}");
    assert!(after <= 0.012, "on left {after}");
    assert!(has_note(&on, "adaptive spacing:"), "{:?}", on.notes);
    assert!(on.warnings.is_empty(), "{:?}", on.warnings);
}

#[test]
fn req_op_04_axial_passes_are_added_only_where_needed() {
    // On a cone the passes are further apart round the wide end: at a 0.5 mm
    // stepover a 4 mm ball leaves more than 0.01 mm there only (about
    // 0.016 mm at radius 11, under 0.004 mm at radius 5), so the passes
    // added need not run the whole length.
    let p = cone();
    let on = generated(
        &p,
        &AxialFinishing,
        &[(names::STEPOVER, ParameterValue::Number(0.5))],
    );
    let after = left(&p, &on, (2.0, 10.0));
    assert!(after <= 0.012, "on left {after}");
    assert!(has_note(&on, "added"), "{:?}", on.notes);
    let lengths = pass_lengths(&on);
    let longest = lengths.iter().copied().fold(0.0, f64::max);
    assert!(
        lengths.iter().any(|&l| l < longest - 1.0),
        "every pass runs {longest} mm: {lengths:?}"
    );
}

/// The X extent of each run of cutting moves at one A.
fn pass_lengths(path: &GeneratedPath) -> Vec<f64> {
    let mut out = Vec::new();
    let mut run: Option<(f64, f64, f64)> = None;
    for s in &path.group.segments {
        if s.kind != MoveKind::Cutting {
            if let Some((lo, hi, _)) = run.take() {
                out.push(hi - lo);
            }
            continue;
        }
        let (x, a) = (s.end.tip.x, s.end.a.0);
        run = match run {
            Some((lo, hi, at)) if (at - a).abs() < 1e-9 => Some((lo.min(x), hi.max(x), at)),
            Some((lo, hi, _)) => {
                out.push(hi - lo);
                Some((x, x, a))
            }
            None => Some((x, x, a)),
        };
    }
    if let Some((lo, hi, _)) = run {
        out.push(hi - lo);
    }
    out
}

#[test]
fn req_op_04_req_op_11_the_stock_to_leave_is_kept_and_never_refined_below() {
    let p = cylinder();
    let leave = 0.3;
    let path = generated(
        &p,
        &AxialFinishing,
        &[(names::STOCK_TO_LEAVE, ParameterValue::Number(leave))],
    );
    let outline = Outline::new(&p.fixture.tool);
    let all = poses(&path, 4);
    let closest = all
        .iter()
        .filter(|q| (1.0..=11.0).contains(&q.tip.x))
        .map(|q| min_clearance(&outline, &p.part, q, 0.1))
        .fold(f64::INFINITY, f64::min);
    assert!(closest >= leave - 0.003, "closest {closest}");
    let after = left(&p, &path, (2.0, 10.0));
    assert!(after <= leave + 0.012, "left {after}");
}

#[test]
fn req_nfr_06_req_gc_07_adaptive_spacing_is_identical_at_1_2_and_n_threads() {
    let p = cylinder();
    for s in STRATEGIES {
        let runs = openchuck_testkit::determinism::check(|| {
            let path = generated(&p, s, &[]);
            format!("{:?}{:?}{:?}", path.group, path.notes, path.warnings).into_bytes()
        })
        .unwrap();
        assert_eq!(runs.len(), 4, "{}", s.name());
    }
}

#[test]
fn req_op_04_a_stepover_below_the_smallest_spacing_is_not_enlarged() {
    let p = cylinder();
    let fine = generated(
        &p,
        &AxialFinishing,
        &[
            (names::STEPOVER, ParameterValue::Number(0.25)),
            (MIN_SPACING, ParameterValue::Number(0.5)),
        ],
    );
    let off = generated(
        &p,
        &AxialFinishing,
        &[
            (names::STEPOVER, ParameterValue::Number(0.25)),
            (ADAPTIVE_SPACING, ParameterValue::Flag(false)),
        ],
    );
    assert_eq!(pass_lengths(&fine).len(), pass_lengths(&off).len());
}

#[test]
fn req_op_04_req_op_05_clean_up_arcs_are_added_between_the_wraps_of_a_helix() {
    // Away from the helix's ends, where only one wrap passes and nothing is
    // measured (as before).
    let p = cylinder();
    let off = generated(
        &p,
        &HelicalFinishing,
        &[(ADAPTIVE_SPACING, ParameterValue::Flag(false))],
    );
    let on = generated(&p, &HelicalFinishing, &[]);
    let (before, after) = (left(&p, &off, (4.0, 8.0)), left(&p, &on, (4.0, 8.0)));
    assert!(before > 0.3, "off left {before}");
    assert!(after <= 0.012, "on left {after}");
    assert!(has_note(&on, "adaptive spacing:"), "{:?}", on.notes);
    assert!(on.warnings.is_empty(), "{:?}", on.warnings);
}

#[test]
fn req_op_04_rings_are_added_between_rings_until_the_allowed_excess_is_met() {
    // Rings reach both ends of the part, so the whole length is measured
    // (decision record 0390 item 5).
    let p = cylinder();
    let rings = (
        openchuck_path::helical::parameter::PASS_SHAPE,
        ParameterValue::Text("rings".to_owned()),
    );
    let off = generated(
        &p,
        &HelicalFinishing,
        &[
            rings.clone(),
            (ADAPTIVE_SPACING, ParameterValue::Flag(false)),
        ],
    );
    let on = generated(&p, &HelicalFinishing, &[rings]);
    let (before, after) = (left(&p, &off, (1.0, 11.0)), left(&p, &on, (1.0, 11.0)));
    assert!(before > 0.3, "off left {before}");
    assert!(after <= 0.012, "on left {after}");
    assert!(has_note(&on, "adaptive spacing:"), "{:?}", on.notes);
    assert!(on.warnings.is_empty(), "{:?}", on.warnings);
}

fn axial_pattern() -> (&'static str, ParameterValue) {
    (PATTERN, ParameterValue::Text("axial".to_owned()))
}

#[test]
fn req_op_04_simultaneous_finishing_spaces_its_axial_pattern_only() {
    let s = SimultaneousFinishing;
    let spiral = s.schema_for(&BTreeMap::new());
    assert!(spiral.get(ADAPTIVE_SPACING).is_none());
    assert!(spiral.get("scallop").is_none());
    let axial = BTreeMap::from([(PATTERN.to_owned(), axial_pattern().1)]);
    let (label, default, spacing) = label_and_default(&s, &axial);
    assert_eq!(label, "Allowed excess");
    assert_eq!(default, Some(0.01));
    assert!(spacing);
    assert_eq!(
        s.schema_for(&axial).get(ADAPTIVE_SPACING).unwrap().kind,
        ParameterKind::Flag { default: true }
    );
    let mut off = axial.clone();
    off.insert(ADAPTIVE_SPACING.to_owned(), ParameterValue::Flag(false));
    let schema = s.schema_for(&off);
    assert!(schema.get("scallop").is_none());
    assert!(schema.get(MIN_SPACING).is_none());
}

#[test]
fn req_op_05_req_op_04_simultaneous_axial_passes_are_added_until_the_allowed_excess_is_met() {
    let p = cylinder();
    let off = generated(
        &p,
        &SimultaneousFinishing,
        &[
            axial_pattern(),
            (ADAPTIVE_SPACING, ParameterValue::Flag(false)),
        ],
    );
    let on = generated(&p, &SimultaneousFinishing, &[axial_pattern()]);
    let (before, after) = (left(&p, &off, (2.0, 10.0)), left(&p, &on, (2.0, 10.0)));
    assert!(before > 0.3, "off left {before}");
    assert!(after <= 0.012, "on left {after}: {:?}", on.notes);
    assert!(has_note(&on, "adaptive spacing:"), "{:?}", on.notes);
    assert!(on.warnings.is_empty(), "{:?}", on.warnings);
}

fn positions(count: i64) -> (&'static str, ParameterValue) {
    ("count", ParameterValue::Integer(count))
}

/// The A of every cutting move, and whether any turns A while it cuts.
fn cutting_angles(path: &GeneratedPath) -> (Vec<i64>, bool) {
    let mut from = path.group.start;
    let mut angles = Vec::new();
    let mut turns = false;
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            turns |= (s.end.a.0 - from.a.0).abs() > 1e-9;
            angles.push((s.end.a.0.rem_euclid(360.0) * 1000.0).round() as i64 % 360_000);
        }
        from = s.end;
    }
    angles.sort_unstable();
    angles.dedup();
    (angles, turns)
}

#[test]
fn req_op_04_req_op_15_indexed_passes_are_added_beside_the_positions_at_their_own_a() {
    // The owner's choice on indexed finishing (decision record 0239 item
    // 3): passes are added at the same A, offset across the axis, so no
    // angle is added and nothing cuts while A turns.
    let p = cylinder();
    let off = generated(
        &p,
        &IndexedFinishing,
        &[
            positions(4),
            (ADAPTIVE_SPACING, ParameterValue::Flag(false)),
        ],
    );
    let on = generated(&p, &IndexedFinishing, &[positions(4)]);
    let (before, after) = (left(&p, &off, (2.0, 10.0)), left(&p, &on, (2.0, 10.0)));
    assert!(before > 0.3, "off left {before}");
    assert!(after <= 0.012, "on left {after}: {:?}", on.notes);
    assert!(on.warnings.is_empty(), "{:?}", on.warnings);
    let (angles, turns) = cutting_angles(&on);
    assert_eq!(angles, vec![0, 90_000, 180_000, 270_000]);
    assert!(!turns, "no cutting move turns A");
    assert!(pass_lengths(&on).len() > pass_lengths(&off).len());
}

#[test]
fn req_op_04_req_nfr_12_what_a_vertical_tool_at_one_a_cannot_reach_is_said() {
    // Two positions half a turn apart: the surface a quarter turn from both
    // is past what a vertical tool at either A reaches, so it stays as it is
    // and the warning says so; the result must be acknowledged.
    let p = cylinder();
    let on = generated(&p, &IndexedFinishing, &[positions(2)]);
    assert_eq!(on.warnings.len(), 1, "{:?}", on.notes);
    let warning = &on.warnings[0];
    assert!(warning.requires_acknowledgement);
    assert!(
        warning.message.contains("cannot reach there"),
        "{}",
        warning.message
    );
    let (angles, turns) = cutting_angles(&on);
    assert_eq!(angles, vec![0, 180_000]);
    assert!(!turns);
}

#[test]
fn req_op_04_indexed_finishing_has_the_switch_and_the_allowed_excess_only_with_it_on() {
    let s = IndexedFinishing;
    assert_eq!(
        s.schema().get(ADAPTIVE_SPACING).unwrap().kind,
        ParameterKind::Flag { default: true }
    );
    let (label, default, spacing) = label_and_default(&s, &BTreeMap::new());
    assert_eq!(label, "Allowed excess");
    assert_eq!(default, Some(0.01));
    assert!(spacing);
    let off = BTreeMap::from([(ADAPTIVE_SPACING.to_owned(), ParameterValue::Flag(false))]);
    let schema = s.schema_for(&off);
    assert!(schema.get("scallop").is_none());
    assert!(schema.get(MIN_SPACING).is_none());
}

/// `mesh`'s triangles whose corners all pass `keep`.
fn pick(
    mesh: &openchuck_types::mesh::TriangleMesh,
    keep: impl Fn(openchuck_types::geometry::Point3) -> bool,
) -> openchuck_types::mesh::TriangleMesh {
    let triangles = (0..mesh.triangles.len())
        .filter(|&t| mesh.triangle(t).unwrap().into_iter().all(&keep))
        .map(|t| mesh.triangles[t])
        .collect();
    openchuck_types::mesh::TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles,
    }
}

#[test]
fn req_op_04_req_op_11_each_zone_keeps_its_own_stock_to_leave_with_adaptive_spacing() {
    // Decision record 0239 item 1: the excess is measured above each part's
    // own target, so a zone with stock to leave is never refined below it,
    // and the passes are added where the material over either target is too
    // high (OP-11 per selection).
    let part = Cylinder {
        radius: 10.0,
        length: 12.0,
    };
    let mesh = part.mesh(49, 720);
    let p = cylinder();
    let low = MeshSurface::new(&pick(&mesh, |q| q.x <= 6.0 + 1e-9)).unwrap();
    let high = MeshSurface::new(&pick(&mesh, |q| q.x >= 6.0 - 1e-9)).unwrap();
    let leave = 0.3;
    let zones = [
        openchuck_path::LeaveZone {
            surface: &low,
            leave: 0.0,
        },
        openchuck_path::LeaveZone {
            surface: &high,
            leave,
        },
    ];
    let path = run(|ctx| {
        openchuck_path::generate_in_zones(
            &AxialFinishing,
            &p.fixture.input(),
            None,
            &zones,
            &values(&AxialFinishing, &[]),
            ctx,
        )
    })
    .unwrap();
    assert!(has_note(&path, "adaptive spacing:"), "{:?}", path.notes);
    assert!(path.warnings.is_empty(), "{:?}", path.warnings);
    // Over the zone with none, at most the Allowed excess is left.
    let bare = left(&p, &path, (1.5, 4.5));
    assert!(bare <= 0.012, "bare zone left {bare}");
    // Over the zone with 0.3 mm, its stock plus at most the Allowed excess,
    // and the tool never comes closer than the stock to leave.
    let kept = left(&p, &path, (7.5, 10.5));
    assert!(kept <= leave + 0.012, "zone with stock left {kept}");
    let outline = Outline::new(&p.fixture.tool);
    let closest = poses(&path, 4)
        .iter()
        .filter(|q| (8.5..=11.0).contains(&q.tip.x))
        .map(|q| min_clearance(&outline, &p.part, q, 0.1))
        .fold(f64::INFINITY, f64::min);
    assert!(closest >= leave - 0.003, "closest {closest}");
}
