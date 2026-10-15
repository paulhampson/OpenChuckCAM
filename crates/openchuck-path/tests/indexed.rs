// SPDX-License-Identifier: AGPL-3.0-or-later
//! Indexed finishing (OP-04, with OP-14; work breakdown slice 12 item 3;
//! decision record 0034).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::indexed::parameter;
use openchuck_path::params::names;
use openchuck_path::{
    GeneratedPath, IndexedFinishing, MeshSurface, ParameterValue, SurfaceSource, generate,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::ir::MoveKind;
use support::{Fixture, ball, run};

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::FEED.to_owned(), ParameterValue::Number(300.0)),
        (names::PLUNGE_FEED.to_owned(), ParameterValue::Number(100.0)),
    ]
    .into_iter()
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn fixture() -> Fixture {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let surface = MeshSurface::new(&part.mesh(3, 720)).unwrap();
    let b = surface.bounds();
    Fixture::new(surface, b.max.y.abs().max(b.max.z.abs()) + 1.0, &ball(6.0))
}

fn indexed(f: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&IndexedFinishing, &f.input(), v, ctx)).unwrap()
}

/// A of every cutting segment, grouped into runs at one A.
fn cut_angles(path: &GeneratedPath) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            let a = s.end.a.0;
            if out.last().is_none_or(|l| (l - a).abs() > 1e-9) {
                out.push(a);
            } else {
                // Within a pass A never moves.
                assert!((out[out.len() - 1] - a).abs() < 1e-9);
            }
        }
    }
    out
}

/// Whole turns apart count as the same position.
fn same_angle(a: f64, b: f64) -> bool {
    (a - b).rem_euclid(360.0).min((b - a).rem_euclid(360.0)) < 1e-9
}

#[test]
fn req_op_04_indexed_positions_land_on_the_requested_angles_in_order() {
    let f = fixture();
    let want = [90.0, 0.0, 270.0, 45.5, 180.0];
    let path = indexed(
        &f,
        &values(&[(parameter::ANGLES, text("90, 0 270; 45.5 180"))]),
    );
    let got = cut_angles(&path);
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(same_angle(*g, w), "{g} vs {w}");
    }
}

#[test]
fn req_op_04_indexed_count_spaces_positions_equally_from_the_start_angle() {
    let f = fixture();
    let path = indexed(
        &f,
        &values(&[
            (parameter::COUNT, ParameterValue::Integer(6)),
            (parameter::START_ANGLE, ParameterValue::Number(15.0)),
        ]),
    );
    let got = cut_angles(&path);
    assert_eq!(got.len(), 6);
    for (i, g) in got.iter().enumerate() {
        assert!(same_angle(*g, 15.0 + 60.0 * i as f64), "{g}");
    }
}

#[test]
fn req_op_04_req_op_14_indexed_positioning_never_feeds_across_the_stock() {
    let f = fixture();
    let path = indexed(&f, &values(&[(parameter::ANGLES, text("0, 120, 240"))]));
    // A changes only on moves that are not cutting or plunging, and every
    // change of position is preceded by a move away from the surface.
    let mut prev = path.group.start;
    let mut positioned = 0;
    for s in &path.group.segments {
        if (s.end.a.0 - prev.a.0).abs() > 1e-9 {
            assert!(
                !matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge),
                "A moved on a {:?} move",
                s.kind
            );
            let r = prev
                .tip
                .y
                .hypot(prev.tip.z)
                .min(s.end.tip.y.hypot(s.end.tip.z));
            assert!(
                r > 16.0,
                "positioning at radius {r} is not clear of the part"
            );
            positioned += 1;
        }
        prev = s.end;
    }
    assert!(positioned >= 2, "{positioned}");
}

#[test]
fn req_op_04_indexed_refuses_a_bad_angle_list() {
    let f = fixture();
    for bad in ["0, x", "nan", "inf"] {
        let err = run(|ctx| {
            generate(
                &IndexedFinishing,
                &f.input(),
                &values(&[(parameter::ANGLES, text(bad))]),
                ctx,
            )
        })
        .unwrap_err();
        assert!(err.to_string().contains("angles"), "{bad}: {err}");
    }
}
