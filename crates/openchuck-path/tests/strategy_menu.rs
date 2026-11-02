// SPDX-License-Identifier: AGPL-3.0-or-later
//! The strategy list a host shows when an operation is added: each strategy
//! supplies a plain-language description, and the list runs from roughing
//! through finishing to the specialist strategies (OP-16, UI-06, NFR-12;
//! decision record 0204; issues 195 and 242).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use openchuck_path::{Registry, StrategyGroup};

/// Every registered strategy explains itself with its limits, in words
/// a reader who is not a machinist can follow, and none promises safe output.
#[test]
fn req_op_16_every_strategy_has_a_plain_description() {
    for s in Registry::builtin().iter() {
        let text = s.description();
        assert!(text.len() > 40, "{}: too short: {text:?}", s.name());
        assert!(text.ends_with('.'), "{}: {text:?}", s.name());
        assert!(
            !text.to_lowercase().contains("safe to run"),
            "{}: {text:?}",
            s.name()
        );
    }
}

/// OP-07, OP-16, NFR-12: choosing swarf must explain curved wall length,
/// straight flank contact across height, selected STEP faces and overcut
/// warnings, without promising that one pass finishes every wall (#290).
#[test]
fn req_op_07_req_op_16_req_nfr_12_swarf_information_explains_curved_selected_step_walls_and_warnings()
 {
    let registry = Registry::builtin();
    let swarf = registry.iter().find(|s| s.name() == "swarf").unwrap();
    let text = swarf.description();
    for required in [
        "selected wall faces",
        "side of the tool",
        "curve along its length",
        "across its height",
        "STEP",
        "tolerance",
        "warning",
        "depth",
        "location",
        "before running the program",
    ] {
        assert!(
            text.contains(required),
            "swarf information must explain {required:?}: {text}"
        );
    }
    assert!(!text.contains("one pass finishes"));
    assert!(!text.contains("walls that are made of straight lines"));
}

/// OP-16, NFR-12: explanations must not promise complete coverage which
/// selections, gaps, reach or unsupported corners can prevent (OI-986).
#[test]
fn req_op_16_req_nfr_12_strategy_information_does_not_promise_every_surface_or_corner() {
    for strategy in Registry::builtin().iter() {
        let text = strategy.description();
        assert!(
            !text.contains("Cuts the whole surface in one continuous spiral"),
            "{}: {text}",
            strategy.name()
        );
        assert!(
            !text.contains("Follows each sharp inside corner"),
            "{}: {text}",
            strategy.name()
        );
        assert!(
            !text.contains("one pass finishes a tall surface"),
            "{}: {text}",
            strategy.name()
        );
    }
}

/// Roughing first, then finishing, then the specialist strategies; within a
/// group, in the order the registry lists them.
#[test]
fn req_op_16_the_menu_runs_from_roughing_to_finishing_to_specialist() {
    let registry = Registry::builtin();
    let order: Vec<(&str, StrategyGroup)> = registry
        .in_menu_order()
        .map(|s| (s.name(), s.group()))
        .collect();
    assert_eq!(order.len(), registry.iter().count());
    let groups: Vec<StrategyGroup> = order.iter().map(|(_, g)| *g).collect();
    let mut sorted = groups.clone();
    sorted.sort();
    assert_eq!(groups, sorted, "{order:?}");
    assert_eq!(order[0], ("rotary_roughing", StrategyGroup::Roughing));
    let names: Vec<&str> = order.iter().map(|(n, _)| *n).collect();
    let at = |n: &str| names.iter().position(|x| *x == n).unwrap();
    assert!(at("helical_finishing") < at("pencil_finishing"));
    assert!(at("pencil_finishing") < at("swarf"));
    assert_eq!(order.last().unwrap().1, StrategyGroup::Specialist);
}
