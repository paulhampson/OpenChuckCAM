// SPDX-License-Identifier: AGPL-3.0-or-later
//! An operation limited by a selection cuts on its selected faces only: a
//! contact on the edge between a selected face and an unselected one is not
//! on the selection, so the tool does not roll over that edge (OP-02; issue
//! #555; decision record 0527).
//!
//! The case: a cylinder (radius 15, X 0 to 40) joined to a tab on the axis
//! (radius 5, X -6 to 0), as the host joins a setup's tabs to the target
//! (decision 0035 item 1), and a 6 mm V-bit of 20 degrees, which keeps a
//! contact on the rim of the end face while its tip sweeps down beside it.
//! The operation selects the cylinder's side only. The tab is where the roll
//! showed in the owner's project; it needs no rule of its own.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;
use support::{Fixture, generate_in_region, run};

use std::collections::BTreeMap;
use std::sync::Arc;

use openchuck_path::params::names;
use openchuck_path::{
    AxialFinishing, CompositeSurface, GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue,
    PathError, RegionConstraint, Solid, Strategy, StrategyInput,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

const RADIUS: f64 = 15.0;
const LENGTH: f64 = 40.0;
/// The tab: on the axis, X -6 to 0, radius 5.
const TAB: (f64, f64, f64) = (-6.0, 0.0, 5.0);
/// The V-bit's included angle and tip diameter.
const V_ANGLE: f64 = 20.0;
const V_TIP: f64 = 0.3;

fn v_bit() -> Tool {
    Tool {
        shape: ToolShape::VBit {
            included_angle: Degrees(V_ANGLE),
            tip_diameter: Millimetres(V_TIP),
        },
        diameter: Millimetres(6.0),
        flute_length: Millimetres(40.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

fn cylinder() -> TriangleMesh {
    Cylinder {
        radius: RADIUS,
        length: LENGTH,
    }
    .mesh(41, 96)
}

fn tab() -> Solid {
    Solid::cylinder(
        Point3::new(TAB.0, 0.0, 0.0),
        UnitVec3::X,
        TAB.1 - TAB.0,
        TAB.2,
    )
    .unwrap()
}

/// The triangles of `mesh` that `keep` takes, on the same vertices.
fn pick(mesh: &TriangleMesh, keep: impl Fn([Point3; 3]) -> bool) -> TriangleMesh {
    let triangles = (0..mesh.triangles.len())
        .filter(|&t| keep(mesh.triangle(t).unwrap()))
        .map(|t| mesh.triangles[t])
        .collect();
    TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles,
    }
}

/// A triangle of an end disc: all its corners at one X.
fn flat(c: [Point3; 3]) -> bool {
    c.iter().all(|p| (p.x - c[0].x).abs() < 1e-9)
}

fn values(strategy: &dyn Strategy, stepover: f64) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, stepover),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for flag in ["face_free_ends", "adaptive_spacing"] {
        if strategy.schema().get(flag).is_some() {
            v.insert(flag.to_owned(), ParameterValue::Flag(false));
        }
    }
    v
}

struct Case {
    fixture: Fixture,
    composite: CompositeSurface,
    side: MeshSurface,
    ends: MeshSurface,
}

impl Case {
    fn new() -> Self {
        let mesh = cylinder();
        let fixture = Fixture::new(MeshSurface::new(&mesh).unwrap(), RADIUS + 1.0, &v_bit());
        let composite =
            CompositeSurface::new(Arc::new(MeshSurface::new(&mesh).unwrap()), vec![tab()]);
        Self {
            fixture,
            composite,
            side: MeshSurface::new(&pick(&mesh, |c| !flat(c))).unwrap(),
            ends: MeshSurface::new(&pick(&mesh, flat)).unwrap(),
        }
    }

    fn input(&self) -> StrategyInput<'_> {
        StrategyInput {
            surface: &self.composite,
            ..self.fixture.input()
        }
    }

    fn run(
        &self,
        strategy: &dyn Strategy,
        stepover: f64,
        region: &RegionConstraint<'_>,
    ) -> Result<GeneratedPath, PathError> {
        run(|ctx| {
            generate_in_region(
                strategy,
                &self.input(),
                region,
                &values(strategy, stepover),
                ctx,
            )
        })
    }
}

/// How far the V-bit at tip `x`, `height` above the rotary axis (its axis
/// through the rotary axis) reaches into the tab grown by `margin`: zero when
/// it stays clear. The V's side reaches `tip + h tan(half angle)` from its
/// axis at `h` above its tip; the tab is closest in the plane through both
/// axes.
fn into_tab(x: f64, height: f64, margin: f64) -> f64 {
    let (lo, hi, r) = (TAB.0 - margin, TAB.1 + margin, TAB.2 + margin);
    if height >= r {
        return 0.0;
    }
    let tan = (V_ANGLE / 2.0).to_radians().tan();
    let tip = V_TIP / 2.0;
    let dx = if (lo..=hi).contains(&x) {
        0.0
    } else {
        (x - lo).abs().min((x - hi).abs())
    };
    if dx <= tip {
        return r - height;
    }
    (r - (height + (dx - tip) / tan)).max(0.0)
}

/// The most the tool comes into the tab grown by `margin` over every move
/// of `path`, fed or rapid, sampled along each move as the machine makes it.
fn deepest_into_tab(f: &Fixture, path: &GeneratedPath, margin: f64) -> f64 {
    let rotary = f.rotary;
    let mut from = path.group.start;
    let mut worst = 0.0_f64;
    for s in &path.group.segments {
        let (p, q) = (
            rotary.to_axis(from.tip, from.a),
            rotary.to_axis(s.end.tip, s.end.a),
        );
        // The axial strategies keep the tool over the axis.
        assert!(p.across.abs() < 1e-6 && q.across.abs() < 1e-6);
        for k in 0..=32 {
            let t = f64::from(k) / 32.0;
            let x = p.along + t * (q.along - p.along);
            let h = p.height + t * (q.height - p.height);
            worst = worst.max(into_tab(x, h, margin));
        }
        from = s.end;
    }
    worst
}

/// The lowest X of the tool's tip while it cuts.
fn lowest_cut(path: &GeneratedPath) -> f64 {
    path.group
        .segments
        .iter()
        .filter(|s| s.kind.is_fed())
        .map(|s| s.end.tip.x)
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn req_op_02_a_selection_stops_at_its_edge_with_an_unselected_face() {
    // Without the faces outside the selection the V-bit keeps its contact on
    // the rim and its tip goes down beside the end face to the tab (#555).
    // With them, a contact on the rim (shared with the end face) is not on
    // the selection, so the pass ends where the tool sits on the side.
    let case = Case::new();
    let rolled = case
        .run(
            &AxialFinishing,
            20.0,
            &RegionConstraint {
                region: Some(&case.side),
                ..RegionConstraint::default()
            },
        )
        .unwrap();
    assert!(
        lowest_cut(&rolled) < -1.0,
        "the case must show the roll over the rim: {}",
        lowest_cut(&rolled)
    );
    for strategy in [&AxialFinishing as &dyn Strategy, &HelicalFinishing] {
        let path = case
            .run(
                strategy,
                if strategy.name() == "axial_finishing" {
                    20.0
                } else {
                    2.0
                },
                &RegionConstraint {
                    region: Some(&case.side),
                    outside: Some(&case.ends),
                    ..RegionConstraint::default()
                },
            )
            .unwrap();
        let lowest = lowest_cut(&path);
        assert!(lowest >= -1e-3, "{}: cut at X {lowest}", strategy.name());
        assert!(
            lowest <= 0.5,
            "{}: stops short at X {lowest}",
            strategy.name()
        );
        assert_eq!(
            deepest_into_tab(&case.fixture, &path, 0.5),
            0.0,
            "{}",
            strategy.name()
        );
    }
}

#[test]
fn req_op_02_faces_outside_the_selection_far_from_it_change_nothing() {
    // A band of the side from X 10 to 30 never reaches the end faces: giving
    // them as faces outside the selection changes nothing (NFR-06).
    let case = Case::new();
    let mesh = cylinder();
    let band = MeshSurface::new(&pick(&mesh, |c| {
        !flat(c) && c.iter().all(|p| (10.0 - 1e-9..=30.0 + 1e-9).contains(&p.x))
    }))
    .unwrap();
    let plain = case
        .run(
            &AxialFinishing,
            20.0,
            &RegionConstraint {
                region: Some(&band),
                ..RegionConstraint::default()
            },
        )
        .unwrap();
    let kept = case
        .run(
            &AxialFinishing,
            20.0,
            &RegionConstraint {
                region: Some(&band),
                outside: Some(&case.ends),
                ..RegionConstraint::default()
            },
        )
        .unwrap();
    assert_eq!(plain.group, kept.group);
}
