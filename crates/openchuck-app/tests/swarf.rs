// SPDX-License-Identifier: AGPL-3.0-or-later
//! Swarf end to end (OP-07; work breakdown slice 17 review checklist;
//! decision record 0061): the wall of a boss cut with the flank of a flat
//! end mill leaves the simulated stock at the wall and no deeper, and the
//! posted program replays to the IR.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_gcode::{
    CheckConfig, Dialect, FeedLimits, TimingRule, Tolerances, compare, read, replay, static_check,
};
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepSurface, ParameterValue, RotaryMapping, StrategyInput, Swarf,
    generate,
};
use openchuck_post::PostProcessor;
use openchuck_sim::{DexelStock, Simulator};
use openchuck_testkit::ruled::boss_cylinder;
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{PathTags, Toolpath};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::stock::Stock;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;
use support::{builtin_post, job};

const RADIUS: f64 = 20.0;
const LENGTH: f64 = 60.0;
const BOSS_X: f64 = 30.0;
const BOSS_RADIUS: f64 = 8.0;
const FOOT: f64 = 10.0;
const TOP: f64 = 18.0;
const TOOL_DIAMETER: f64 = 6.0;

fn tool() -> RevolvedProfile {
    Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(TOOL_DIAMETER),
        flute_length: Millimetres(15.0),
        overall_length: Millimetres(60.0),
        shank_diameter: Millimetres(TOOL_DIAMETER),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap()
}

fn values() -> BTreeMap<String, ParameterValue> {
    [(names::FEED, 600.0), (names::PLUNGE_FEED, 200.0)]
        .into_iter()
        .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
        .collect()
}

/// The boss wall swarfed on the example machine: a cylinder of `BOSS_RADIUS`
/// about an axis parallel to Z through (`BOSS_X`, 0, 0), from `FOOT` to
/// `TOP` above the rotary axis, inside a round blank of `RADIUS`.
fn make() -> (Toolpath, MachineProfile) {
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, FOOT, TOP);
    let surface = BrepSurface::new(&model).unwrap();
    let machine = MachineProfile::example_x_rotary();
    let tool = tool();
    let stock = AnalyticStock::Cylinder {
        radius: RADIUS,
        x_start: 0.0,
        x_end: LENGTH,
    };
    let tags = PathTags {
        operation: "swarf".to_owned(),
        setup: "s1".to_owned(),
        tool: "t1".to_owned(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(30.0),
            safe_z: Millimetres(40.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let path = generate(&Swarf, &input, &values(), &ctx).unwrap();
    (
        Toolpath {
            groups: vec![path.group],
        },
        machine,
    )
}

#[test]
fn req_op_07_sim_05_the_simulated_stock_is_cut_to_the_boss_wall_and_no_deeper() {
    let (toolpath, machine) = make();
    let stock = Stock::Cylinder {
        diameter: Millimetres(2.0 * RADIUS),
        length: Millimetres(LENGTH),
        x_start: Millimetres(0.0),
    };
    let cell = 0.1;
    let stock = DexelStock::from_stock(&stock, None, cell).unwrap();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let mut sim = Simulator::without_checkpoints(stock, &machine);
    let result = sim.run_group(&toolpath.groups[0], &tool(), &ctx).unwrap();
    assert_eq!(result.rapid_cuts.len(), 0, "a rapid cuts material");
    let stock = sim.into_stock();
    let at = |angle_degrees: f64, r: f64, z: f64| {
        let a = angle_degrees.to_radians();
        Point3::new(BOSS_X + r * a.cos(), r * a.sin(), z)
    };
    for angle in [0.0, 37.0, 90.0, 141.0, 180.0, 222.0, 270.0, 333.0] {
        for z in [FOOT + 1.0, 0.5 * (FOOT + TOP), TOP - 1.0] {
            // The wall is cut: just outside it is empty, just inside it is
            // the boss, and the tool's whole width has passed outside it.
            assert!(
                !stock.contains_point(at(angle, BOSS_RADIUS + 2.0 * cell, z)),
                "the wall at {angle} degrees, z {z} is not cut"
            );
            assert!(
                !stock.contains_point(at(angle, BOSS_RADIUS + TOOL_DIAMETER - 2.0 * cell, z)),
                "the ring round the wall at {angle} degrees, z {z} is not cut"
            );
            assert!(
                stock.contains_point(at(angle, BOSS_RADIUS - 2.0 * cell, z)),
                "the boss at {angle} degrees, z {z} is cut into"
            );
        }
        // Below the foot of the wall the blank is untouched: the tip stops
        // at the foot.
        assert!(
            stock.contains_point(at(angle, BOSS_RADIUS + 1.0, FOOT - 3.0 * cell)),
            "the blank below the wall at {angle} degrees is cut"
        );
        // Beyond the tool's reach the blank is untouched.
        assert!(
            stock.contains_point(at(
                angle,
                BOSS_RADIUS + TOOL_DIAMETER + 3.0 * cell,
                FOOT + 1.0
            )),
            "the blank beyond the tool at {angle} degrees is cut"
        );
    }
}

#[test]
fn req_op_07_gc_08_the_posted_swarf_program_replays_to_the_ir() {
    let (toolpath, _) = make();
    let posted = builtin_post("linuxcnc")
        .post(&toolpath, &job())
        .unwrap()
        .text;
    let program = read(&posted).unwrap();
    let replayed = replay(&program, Dialect::Linuxcnc).unwrap();
    let c = compare(
        &replayed,
        &toolpath,
        &job().frame,
        TimingRule::Linuxcnc,
        Tolerances::for_decimals(3, 3),
    )
    .unwrap();
    assert!(c.poses > 50, "{c:?}");
    assert!(c.max_position_error < 0.0009, "{c:?}");
    assert!(c.max_a_error < 0.001, "{c:?}");
    let config = CheckConfig {
        travel: [(-10.0, 200.0), (-200.0, 200.0), (-200.0, 100.0)],
        a_range: None,
        max_feed: FeedLimits {
            linear: 3000.0,
            z: 2000.0,
            rotary: 7200.0,
        },
        safe_z: 40.0,
        spindle_off_at_tool_change: true,
        require_work_offset: true,
        allow_inverse_time: false,
    };
    assert_eq!(static_check(&replayed, &config), []);
}
