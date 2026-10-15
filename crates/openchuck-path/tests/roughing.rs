// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rotary roughing (OP-03, OP-11, OP-14; work breakdown slice 14; decision
//! record 0033).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::roughing::{NAME, parameter};
use openchuck_path::{GeneratedPath, MeshSurface, ParameterValue, RotaryRoughing, generate};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::MoveKind;
use openchuck_types::tool::{Tool, ToolShape};
use support::{Fixture, run, tool};

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 3.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.5),
        (parameter::STEPDOWN, 2.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn fixture<P: RadialPart + ?Sized>(part: &P, stock_radius: f64, t: &Tool) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(40, 360)).unwrap();
    Fixture::new(surface, stock_radius, t)
}

fn rough(f: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&RotaryRoughing, &f.input(), v, ctx)).unwrap()
}

fn flat(d: f64) -> Tool {
    tool(ToolShape::FlatEnd, d)
}

fn poses(path: &GeneratedPath) -> Vec<Pose> {
    path.group
        .segments
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                MoveKind::Cutting | MoveKind::Plunge | MoveKind::LeadIn
            )
        })
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect()
}

#[test]
fn smoke_continuous_cone() {
    let part = Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 40.0,
    };
    let f = fixture(&part, 15.0, &flat(6.0));
    let t0 = std::time::Instant::now();
    let path = rough(&f, &values(&[]));
    eprintln!(
        "{} segments in {:?}",
        path.group.segments.len(),
        t0.elapsed()
    );
    let outline = Outline::new(&f.tool);
    let g = max_gouge(&outline, &part, &poses(&path), 0.25);
    eprintln!("gouge {}", g.depth);
    assert!(g.depth < 0.01);
    let _ = (
        NAME,
        Cylinder {
            radius: 1.0,
            length: 1.0,
        },
    );
}
