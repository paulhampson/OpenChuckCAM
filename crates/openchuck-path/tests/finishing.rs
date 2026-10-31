// SPDX-License-Identifier: AGPL-3.0-or-later
//! The finishing operations' Stepdown, Finishing layers and Face free ends
//! (decision records 0344 and 0345; OP-04, OP-09, TL-05, NFR-12).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use openchuck_path::finishing::parameter::{FACE_FREE_ENDS, FINISHING_LAYERS, STEPDOWN};
use openchuck_path::params::ParameterKind;
use openchuck_path::{
    AxialFinishing, HelicalFinishing, IndexedFinishing, ParameterValue, RestMachining,
    RotaryRoughing, SimultaneousFinishing, Strategy,
};

fn flag_default(strategy: &dyn Strategy, name: &str) -> Option<bool> {
    match strategy.schema().get(name).map(|p| p.kind.clone()) {
        Some(ParameterKind::Flag { default }) => Some(default),
        _ => None,
    }
}

/// Whether `name` is a number the operation must be given (no default) in
/// the schema active for `values`.
fn required(strategy: &dyn Strategy, values: &BTreeMap<String, ParameterValue>) -> bool {
    matches!(
        strategy
            .schema_for(values)
            .get(STEPDOWN)
            .map(|p| p.kind.clone()),
        Some(ParameterKind::Number { default: None, .. })
    )
}

/// The values every finishing operation needs.
fn common() -> BTreeMap<String, ParameterValue> {
    [("stepover", 1.0), ("feed", 600.0), ("plunge_feed", 150.0)]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
        .collect()
}

#[test]
fn req_op_04_face_free_ends_is_on_for_helical_axial_and_rest_and_nowhere_else() {
    for s in [
        &HelicalFinishing as &dyn Strategy,
        &AxialFinishing,
        &RestMachining,
    ] {
        assert_eq!(flag_default(s, FACE_FREE_ENDS), Some(true), "{}", s.name());
    }
    for s in [
        &IndexedFinishing as &dyn Strategy,
        &SimultaneousFinishing,
        &RotaryRoughing,
    ] {
        assert_eq!(flag_default(s, FACE_FREE_ENDS), None, "{}", s.name());
    }
}

#[test]
fn req_op_04_finishing_layers_are_off_by_default_on_helical_axial_and_indexed() {
    for s in [
        &HelicalFinishing as &dyn Strategy,
        &AxialFinishing,
        &IndexedFinishing,
    ] {
        assert_eq!(
            flag_default(s, FINISHING_LAYERS),
            Some(false),
            "{}",
            s.name()
        );
        assert!(s.schema().get(STEPDOWN).is_some(), "{}", s.name());
    }
    for s in [&RestMachining as &dyn Strategy, &SimultaneousFinishing] {
        assert_eq!(flag_default(s, FINISHING_LAYERS), None, "{}", s.name());
    }
    // Rest machining has a Stepdown for its free ends.
    assert!(RestMachining.schema().get(STEPDOWN).is_some());
}

#[test]
fn req_tl_05_req_nfr_12_stepdown_is_required_only_with_layers_on() {
    // No guessed depth of cut (NFR-12, decision 0344 C1): with layers on the
    // Stepdown must come from the operation or the tool's cutting data
    // (TL-05), which the host fills in; with layers off it is not required,
    // and a free end that needs it says so when the path is made.
    let off = BTreeMap::new();
    let on = BTreeMap::from([(FINISHING_LAYERS.to_owned(), ParameterValue::Flag(true))]);
    for s in [
        &HelicalFinishing as &dyn Strategy,
        &AxialFinishing,
        &IndexedFinishing,
    ] {
        assert!(!required(s, &off), "{}", s.name());
        assert!(required(s, &on), "{}", s.name());
        // A value given with layers on resolves; none is refused.
        let mut given = common();
        given.insert(FINISHING_LAYERS.to_owned(), ParameterValue::Flag(true));
        given.insert(STEPDOWN.to_owned(), ParameterValue::Number(0.5));
        s.resolve_parameters(&given).unwrap();
        given.remove(STEPDOWN);
        let err = s.resolve_parameters(&given).unwrap_err().to_string();
        assert!(err.contains(STEPDOWN), "{}: {err}", s.name());
    }
    assert!(!required(&RestMachining, &off));
    // A Stepdown of zero or less is refused whenever it is given.
    for s in [&HelicalFinishing as &dyn Strategy, &RestMachining] {
        let mut values = common();
        s.resolve_parameters(&values).unwrap();
        values.insert(STEPDOWN.to_owned(), ParameterValue::Number(-1.0));
        assert!(s.resolve_parameters(&values).is_err(), "{}", s.name());
    }
}

// ---- finishing layers (decision record 0345) --------------------------------

mod support;
use support::generate;

use openchuck_path::finishing::parameter::FINISH_ALLOWANCE;
use openchuck_path::{GeneratedPath, MeshSurface};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::ir::MoveKind;
use openchuck_types::tool::ToolShape;
use support::{Fixture, run, tool};

/// A radius 10 cylinder, X 0 to 30, in cylinder stock of `stock` radius.
fn cylinder(stock: f64) -> Fixture {
    let mesh = Cylinder {
        radius: 10.0,
        length: 30.0,
    }
    .mesh(61, 720);
    Fixture::new(
        MeshSurface::new(&mesh).unwrap(),
        stock,
        &tool(ToolShape::FlatEnd, 3.175),
    )
}

fn layered(
    f: &Fixture,
    strategy: &dyn Strategy,
    extra: &[(&str, ParameterValue)],
) -> Result<GeneratedPath, String> {
    let mut values = common();
    values.insert("tolerance".to_owned(), ParameterValue::Number(0.01));
    if strategy.schema().get(FACE_FREE_ENDS).is_some() {
        values.insert(FACE_FREE_ENDS.to_owned(), ParameterValue::Flag(false));
    }
    for (k, v) in extra {
        values.insert((*k).to_owned(), v.clone());
    }
    run(|ctx| generate(strategy, &f.input(), &values, ctx)).map_err(|e| e.to_string())
}

fn layers_on(stepdown: f64, allowance: f64) -> Vec<(&'static str, ParameterValue)> {
    vec![
        (FINISHING_LAYERS, ParameterValue::Flag(true)),
        (STEPDOWN, ParameterValue::Number(stepdown)),
        (FINISH_ALLOWANCE, ParameterValue::Number(allowance)),
    ]
}

/// The radii of the tool's tip over the part's length on cutting moves, to a
/// micrometre: the levels the operation cuts at.
fn levels(path: &GeneratedPath) -> Vec<i64> {
    let mut out: Vec<i64> = path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting && (1.0..=29.0).contains(&s.end.tip.x))
        .map(|s| (s.end.tip.y.hypot(s.end.tip.z) * 1000.0).round() as i64)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn req_op_04_finishing_layers_step_down_to_a_light_final_pass() {
    // 2 mm of stock over the target, Stepdown 0.8, Finish allowance 0.1: the
    // layers are at 0.1 + 0.8 k above the target where stock is there to
    // cut (0.1, 0.9 and 1.7), then the final pass on the target (decision
    // record 0345; OI-1391 option 1, OI-1392 option 3).
    let f = cylinder(12.0);
    for s in [
        &HelicalFinishing as &dyn Strategy,
        &AxialFinishing,
        &IndexedFinishing,
    ] {
        let path = layered(&f, s, &layers_on(0.8, 0.1)).unwrap();
        assert_eq!(
            levels(&path),
            vec![10_000, 10_100, 10_900, 11_700],
            "{}",
            s.name()
        );
        // The deepest level last: the final pass ends the operation's cuts.
        let last = path
            .group
            .segments
            .iter()
            .rev()
            .find(|s| s.kind == MoveKind::Cutting)
            .unwrap();
        assert!(
            (last.end.tip.y.hypot(last.end.tip.z) - 10.0).abs() < 1e-3,
            "{}",
            s.name()
        );
        let notes: String = path.notes.iter().map(|n| n.message.clone()).collect();
        assert!(
            notes.contains("3 layers of 0.8 mm") && notes.contains("finish allowance of 0.1 mm"),
            "{}: {notes}",
            s.name()
        );
    }
}

#[test]
fn req_op_04_no_layer_cuts_air() {
    // Only 0.5 mm over the target: one layer at 0.1, none at 0.9.
    let f = cylinder(10.5);
    let path = layered(&f, &HelicalFinishing, &layers_on(0.8, 0.1)).unwrap();
    assert_eq!(levels(&path), vec![10_000, 10_100]);
}

#[test]
fn req_op_04_a_zero_finish_allowance_puts_the_last_layer_on_the_target() {
    let f = cylinder(12.0);
    let path = layered(&f, &HelicalFinishing, &layers_on(0.8, 0.0)).unwrap();
    assert_eq!(levels(&path), vec![10_000, 10_800, 11_600]);
}

#[test]
fn req_op_04_layers_off_is_the_path_as_before() {
    let f = cylinder(12.0);
    for s in [
        &HelicalFinishing as &dyn Strategy,
        &AxialFinishing,
        &IndexedFinishing,
    ] {
        let plain = layered(&f, s, &[]).unwrap();
        let off = layered(
            &f,
            s,
            &[
                (FINISHING_LAYERS, ParameterValue::Flag(false)),
                (STEPDOWN, ParameterValue::Number(0.8)),
                (FINISH_ALLOWANCE, ParameterValue::Number(0.1)),
            ],
        )
        .unwrap();
        assert_eq!(
            format!("{:?}", plain.group),
            format!("{:?}", off.group),
            "{}",
            s.name()
        );
        assert_eq!(levels(&plain), vec![10_000], "{}", s.name());
    }
}

#[test]
fn req_nfr_07_a_finish_allowance_not_below_the_stepdown_or_a_stepdown_past_the_flutes_is_refused() {
    let f = cylinder(12.0);
    let err = layered(&f, &HelicalFinishing, &layers_on(0.8, 0.8)).unwrap_err();
    assert!(err.contains(FINISH_ALLOWANCE), "{err}");
    let err = layered(&f, &HelicalFinishing, &layers_on(0.8, -0.1)).unwrap_err();
    assert!(err.contains(FINISH_ALLOWANCE), "{err}");
    // The test tool has 12 mm of flute.
    let err = layered(&f, &HelicalFinishing, &layers_on(13.0, 0.1)).unwrap_err();
    assert!(err.contains("flute"), "{err}");
}

#[test]
fn req_nfr_06_req_gc_07_finishing_layers_are_identical_at_1_2_and_n_threads() {
    let f = cylinder(12.0);
    for s in [&HelicalFinishing as &dyn Strategy, &IndexedFinishing] {
        let runs = openchuck_testkit::determinism::check(|| {
            let path = layered(&f, s, &layers_on(0.8, 0.1)).unwrap();
            format!("{:?}{:?}", path.group, path.notes).into_bytes()
        })
        .unwrap();
        assert_eq!(runs.len(), 4, "{}", s.name());
    }
}
