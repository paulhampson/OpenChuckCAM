// SPDX-License-Identifier: AGPL-3.0-or-later
//! Accepted 0247: pass-boundary counter resets preserve physical motion (OP-15).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

#[path = "../../openchuck-path/tests/support/mod.rs"]
mod path_support;
#[path = "../../openchuck-post/tests/support/mod.rs"]
mod post_support;

use openchuck_path::params::names;
use openchuck_path::{MeshSurface, ParameterValue, RotaryRoughing, generate};
use openchuck_post::PostProcessor;
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::ir::{MoveKind, Toolpath};
use openchuck_types::tool::ToolShape;
use std::collections::BTreeMap;

fn layers() -> Toolpath {
    let part = Cylinder {
        radius: 8.0,
        length: 6.0,
    };
    let tool = path_support::tool(ToolShape::FlatEnd, 3.0);
    let surface = MeshSurface::new(&part.mesh(4, 72)).unwrap();
    let mut fixture = path_support::Fixture::new(surface, 10.0, &tool);
    fixture.tags.tool = "t1".into();
    let values: BTreeMap<_, _> = [
        (names::STEPOVER, 3.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.5),
        ("stepdown", 0.5),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), ParameterValue::Number(v)))
    .collect();
    let result =
        path_support::run(|ctx| generate(&RotaryRoughing, &fixture.input(), &values, ctx)).unwrap();
    Toolpath {
        groups: vec![result.group],
    }
}

#[test]
fn req_op_15_req_gc_09_long_helical_operation_resets_between_complete_passes_without_rewinds() {
    let path = layers();
    assert!(
        path.groups[0].segments.last().unwrap().end.a.0 > 2000.0,
        "fixture must accumulate many turns before the fix"
    );
    let mut job = post_support::job();
    job.safe_z = 25.0;
    let text = post_support::builtin_post("makera-z1")
        .post(&path, &job)
        .unwrap()
        .text;
    assert!(
        text.matches("G92.4 A0 S0").count() > 1,
        "reset between helical passes, not just at operation end: {text}"
    );
    // Compare this fixture's counter magnitude before/after pass resets.
    // This does not claim a hardware limit or solve an uninterrupted helix.
    let program = openchuck_gcode::read(&text).unwrap();
    let replay = openchuck_gcode::replay(&program, openchuck_gcode::Dialect::MakeraZ1).unwrap();
    assert!(
        replay
            .moves()
            .filter(|m| !m.machine && m.motion == openchuck_gcode::Motion::Linear)
            .all(|m| m.spindle_on),
        "counter resets must preserve spindle state before the next cut"
    );
    let peak = replay
        .moves()
        .filter(|m| !m.machine)
        .flat_map(|m| [m.from[3].abs(), m.to[3].abs()])
        .fold(0.0_f64, f64::max);
    let unreset_peak = path.groups[0]
        .segments
        .iter()
        .map(|s| s.end.a.0.abs())
        .fold(0.0_f64, f64::max);
    assert!(
        peak < unreset_peak,
        "pass resets must reduce this fixture's controller coordinate magnitude"
    );
    let comparison = openchuck_gcode::compare(
        &replay,
        &path,
        &job.frame,
        openchuck_gcode::TimingRule::Makera,
        openchuck_gcode::Tolerances::for_decimals(3, 3),
    )
    .unwrap();
    assert!((comparison.ir_a_travel - comparison.gcode_a_travel).abs() < 0.01);
    assert!(
        path.groups[0]
            .segments
            .iter()
            .any(|s| s.kind == MoveKind::Cutting)
    );
}

#[test]
fn req_gc_07_req_nfr_06_pass_resets_are_identical_at_1_2_and_n_threads() {
    let mut expected = None;
    for count in openchuck_testkit::determinism::thread_counts() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(count)
            .build()
            .unwrap();
        let path = pool.install(layers);
        let text = post_support::builtin_post("makera-z1")
            .post(&path, &post_support::job())
            .unwrap()
            .text;
        if let Some((before_path, before_text)) = &expected {
            assert_eq!(&path, before_path, "{count} workers");
            assert_eq!(&text, before_text, "{count} workers");
        } else {
            expected = Some((path, text));
        }
    }
}

#[test]
fn req_gc_07_req_nfr_06_single_pass_without_a_reset_boundary_retains_golden_bytes() {
    for (id, name) in [
        ("makera-z1", "post_makera_z1_small_part.ngc"),
        ("linuxcnc", "post_linuxcnc_small_part.ngc"),
    ] {
        let text = post_support::builtin_post(id)
            .post(&post_support::small_part(), &post_support::job())
            .unwrap()
            .text;
        let before =
            std::fs::read_to_string(post_support::repo("testdata/golden").join(name)).unwrap();
        assert_eq!(openchuck_testkit::golden::mask_generator(&text), before);
    }
}

#[test]
fn req_gc_01_req_op_15_an_explicit_boundary_is_nonmoving_and_rebases_the_next_cut() {
    let mut path = post_support::small_part();
    // Two complete helices joined by an existing safe retract/position/entry.
    // Preserve the first pass and add a second copy offset by three turns.
    let mut second = path.groups[0].segments.clone();
    for segment in &mut second {
        segment.end.a.0 += 1080.0;
    }
    let boundary = path.groups[0].segments.len() - 1;
    path.groups[0].segments.extend(second);
    let mut json = serde_json::to_value(&path).unwrap();
    json["groups"][0]["rotary_reset_after"] = serde_json::json!([boundary]);
    let path: Toolpath =
        serde_json::from_value(json).expect("explicit pass boundary must survive IR roundtrip");
    let text = post_support::builtin_post("makera-z1")
        .post(&path, &post_support::job())
        .unwrap()
        .text;
    assert_eq!(text.matches("G92.4 A0 S0").count(), 2);
    let replay = openchuck_gcode::replay(
        &openchuck_gcode::read(&text).unwrap(),
        openchuck_gcode::Dialect::MakeraZ1,
    )
    .unwrap();
    let c = openchuck_gcode::compare(
        &replay,
        &path,
        &post_support::job().frame,
        openchuck_gcode::TimingRule::Makera,
        openchuck_gcode::Tolerances::for_decimals(3, 3),
    )
    .unwrap();
    assert!((c.ir_a_travel - c.gcode_a_travel).abs() < 0.001);
    let linux = post_support::builtin_post("linuxcnc")
        .post(&path, &post_support::job())
        .unwrap()
        .text;
    let mut unmarked = path.clone();
    let mut json = serde_json::to_value(&unmarked).unwrap();
    json["groups"][0]["rotary_reset_after"] = serde_json::json!([]);
    unmarked = serde_json::from_value(json).unwrap();
    assert_eq!(
        linux,
        post_support::builtin_post("linuxcnc")
            .post(&unmarked, &post_support::job())
            .unwrap()
            .text,
        "rotate-only post must omit bookkeeping, not insert a physical unwind"
    );
}
