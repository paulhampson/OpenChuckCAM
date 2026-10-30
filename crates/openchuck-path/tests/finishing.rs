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
