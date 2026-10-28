// SPDX-License-Identifier: AGPL-3.0-or-later
//! Edge following replayed in the stock simulation (OP-08, SIM-05; work
//! breakdown slice 16 review checklist; decision record 0047). The strategy
//! on its own is in `crates/openchuck-path/tests/edge_following.rs`.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use openchuck_path::edge_following::parameter as p;
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, EdgeFollowing, MeshSurface, ParameterValue, RotaryMapping, StrategyInput, generate,
};
use openchuck_sim::{DeviationOptions, DexelStock, Simulator, compare};
use openchuck_testkit::parts::revolve;
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

const CELL: f64 = 0.1;
const RADIUS: f64 = 6.0;
const LENGTH: f64 = 8.0;

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

/// Engraves the two rims of a cylinder `depth` deep with a 1 mm ball into
/// stock that is the cylinder itself, and returns the stock afterwards, the
/// number of cuts made on a rapid and the deviation from the part.
fn engrave(
    depth: f64,
    extra: &[(&str, ParameterValue)],
) -> (openchuck_sim::Deviation, usize, DexelStock) {
    let machine = MachineProfile::example_x_rotary();
    let target = revolve(&[(0.0, RADIUS), (LENGTH, RADIUS)], 180);
    let surface = MeshSurface::new(&target).unwrap();
    let stock = Stock::Cylinder {
        diameter: Millimetres(2.0 * RADIUS),
        length: Millimetres(LENGTH),
        x_start: Millimetres(0.0),
    };
    let dexel = DexelStock::from_stock(&stock, None, CELL).unwrap();
    let tool = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(1.0),
        flute_length: Millimetres(6.0),
        overall_length: Millimetres(30.0),
        shank_diameter: Millimetres(1.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap();
    let tags = PathTags {
        operation: "op".to_owned(),
        setup: "setup".to_owned(),
        tool: "tool".to_owned(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &dexel,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(RADIUS + 4.0),
            safe_z: Millimetres(RADIUS + 10.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
    };
    let mut values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 0.4),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
        (p::DEPTH, depth),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), num(v)))
    .collect();
    for (k, v) in extra {
        values.insert((*k).to_owned(), v.clone());
    }
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let path = generate(&EdgeFollowing::default(), &input, &values, &ctx).unwrap();
    let mut sim = Simulator::without_checkpoints(dexel.clone(), &machine);
    let result = sim.run_group(&path.group, &tool, &ctx).unwrap();
    let stock = sim.into_stock();
    let deviation = compare(
        &stock,
        &target,
        &DeviationOptions {
            spacing: 0.25,
            ..DeviationOptions::for_cell(CELL)
        },
    )
    .unwrap();
    (deviation, result.rapid_cuts.len(), stock)
}

#[test]
fn req_op_08_req_sim_05_replay_engraves_the_depth_asked_for_and_nothing_else() {
    let depth = 0.3;
    let (deviation, rapid_cuts, _) = engrave(depth, &[]);
    assert_eq!(rapid_cuts, 0, "the tool cuts only at the feed");
    // The groove is `depth` deep at the rim, within a simulation cell.
    let worst = deviation.worst_overcut.expect("the rims are cut").value;
    assert!(
        (-worst - depth).abs() <= CELL,
        "the deepest overcut is {} mm, the depth {depth} mm",
        -worst
    );
    // Every overcut place is at a rim; the rest of the part is untouched
    // (the stock is the part, so nothing is left above it either), and both
    // rims are cut.
    assert!(!deviation.overcut_places.is_empty());
    let (mut low, mut high) = (false, false);
    for place in &deviation.overcut_places {
        low |= place.point.x < 0.6;
        high |= place.point.x > LENGTH - 0.6;
    }
    assert!(low && high, "{:?}", deviation.overcut_places);
    for place in &deviation.overcut_places {
        assert!(
            place.point.x < 0.6 || place.point.x > LENGTH - 0.6,
            "overcut away from a rim at x = {}",
            place.point.x
        );
    }
    assert!(deviation.worst_remaining.is_none_or(|r| r.value <= CELL));
}

#[test]
fn req_op_08_req_sim_05_a_deeper_engraving_replays_deeper() {
    let (shallow, _, _) = engrave(0.2, &[]);
    let (deep, _, _) = engrave(0.4, &[]);
    let (a, b) = (
        -shallow.worst_overcut.unwrap().value,
        -deep.worst_overcut.unwrap().value,
    );
    assert!((b - a - 0.2).abs() <= CELL, "{a} then {b}");
}

#[test]
fn req_op_08_req_sim_05_levels_reach_the_same_depth_as_one_pass() {
    let (one, _, _) = engrave(0.3, &[]);
    let (levels, rapid_cuts, _) = engrave(0.3, &[(p::DEPTH_PER_PASS, num(0.1))]);
    assert_eq!(rapid_cuts, 0);
    let (a, b) = (
        -one.worst_overcut.unwrap().value,
        -levels.worst_overcut.unwrap().value,
    );
    assert!((a - b).abs() <= CELL, "{a} vs {b}");
}
