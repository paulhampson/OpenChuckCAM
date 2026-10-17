// SPDX-License-Identifier: AGPL-3.0-or-later
//! Radial drilling end to end (OP-10; work breakdown slice 18 review
//! checklist; decision record 0051): holes found from the B-rep faces are
//! drilled to the model's depth in the simulated stock, and the posted G-code
//! replays to the IR.
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
    AcceptAll, AnalyticStock, BrepSurface, ParameterValue, RadialDrilling, RotaryMapping,
    StrategyInput, generate,
};
use openchuck_post::PostProcessor;
use openchuck_sim::{DexelStock, Simulator};
use openchuck_testkit::brep::{HoleSpec, drilled_blank};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{PathTags, Toolpath};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};
use support::{builtin_post, job};

const RADIUS: f64 = 12.0;
const LENGTH: f64 = 60.0;
const DIAMETER: f64 = 3.0;

fn polar(phi: f64) -> [f64; 3] {
    [0.0, phi.to_radians().cos(), phi.to_radians().sin()]
}

fn values(extra: &[(&str, f64)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> =
        [(names::FEED, 120.0), (names::PLUNGE_FEED, 300.0)]
            .into_iter()
            .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
            .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), ParameterValue::Number(*x));
    }
    v
}

fn specs() -> Vec<HoleSpec> {
    let mut point = HoleSpec::blind(30.0, polar(200.0), DIAMETER, 6.0);
    point.point = true;
    vec![
        HoleSpec::blind(10.0, polar(90.0), DIAMETER, 8.0),
        HoleSpec::blind(20.0, polar(0.0), DIAMETER, 5.0),
        point,
        HoleSpec::through(40.0, polar(45.0), DIAMETER),
        HoleSpec::blind(50.0, polar(300.0), DIAMETER, 7.0),
    ]
}

struct Job {
    toolpath: Toolpath,
    built: Vec<openchuck_testkit::brep::Built>,
    machine: MachineProfile,
}

fn make(extra: &[(&str, f64)]) -> Job {
    let (model, built) = drilled_blank(RADIUS, LENGTH, &specs());
    let surface = BrepSurface::new(&model).unwrap();
    let machine = MachineProfile::example_x_rotary();
    let tool = Tool {
        shape: ToolShape::Drill {
            point_angle: Degrees(118.0),
        },
        diameter: Millimetres(DIAMETER),
        flute_length: Millimetres(30.0),
        overall_length: Millimetres(60.0),
        shank_diameter: Millimetres(DIAMETER),
        flute_count: 2,
        material: None,
        centre_cutting: Some(true),
    }
    .profile()
    .unwrap();
    let stock = AnalyticStock::Cylinder {
        radius: RADIUS,
        x_start: 0.0,
        x_end: LENGTH,
    };
    let tags = PathTags {
        operation: "drill".to_owned(),
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
            radius: Millimetres(17.0),
            safe_z: Millimetres(27.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let path = generate(&RadialDrilling, &input, &values(extra), &ctx).unwrap();
    Job {
        toolpath: Toolpath {
            groups: vec![path.group],
        },
        built,
        machine,
    }
}

fn point_height() -> f64 {
    0.5 * DIAMETER / (59.0_f64).to_radians().tan()
}

#[test]
fn req_op_10_the_simulation_shows_each_hole_at_the_model_depth() {
    for extra in [vec![], vec![("peck_depth", 3.0)]] {
        let job = make(&extra);
        let stock = Stock::Cylinder {
            diameter: Millimetres(2.0 * RADIUS),
            length: Millimetres(LENGTH),
            x_start: Millimetres(0.0),
        };
        let cell = 0.1;
        let stock = DexelStock::from_stock(&stock, None, cell).unwrap();
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let mut sim = Simulator::without_checkpoints(stock, &job.machine);
        let tool = Tool {
            shape: ToolShape::Drill {
                point_angle: Degrees(118.0),
            },
            diameter: Millimetres(DIAMETER),
            flute_length: Millimetres(30.0),
            overall_length: Millimetres(60.0),
            shank_diameter: Millimetres(DIAMETER),
            flute_count: 2,
            material: None,
            centre_cutting: Some(true),
        }
        .profile()
        .unwrap();
        let result = sim.run_group(&job.toolpath.groups[0], &tool, &ctx).unwrap();
        assert_eq!(result.rapid_cuts.len(), 0, "a rapid cuts material");
        let stock = sim.into_stock();
        for (i, b) in job.built.iter().enumerate() {
            let blind = b.length < 2.0 * RADIUS - 1e-9;
            let reach = if blind {
                b.length + point_height()
            } else {
                b.length
            };
            let along = |depth: f64, off: f64| -> Point3 {
                // A point `depth` below the entry on the hole's axis, `off`
                // to the side of it along the rotary axis.
                let on = b.entry + b.direction * -depth;
                Point3::new(on.x + off, on.y, on.z)
            };
            // Empty along the axis down to the full diameter, and across the
            // hole's radius there.
            for fraction in [0.1, 0.5, 0.9] {
                let depth = b.length * fraction;
                assert!(
                    !stock.contains_point(along(depth, 0.0)),
                    "hole {i} is not open at {depth}"
                );
                assert!(
                    !stock.contains_point(along(depth, 0.5 * DIAMETER - 2.0 * cell)),
                    "hole {i} is narrower than the tool at {depth}"
                );
                assert!(
                    stock.contains_point(along(depth, 0.5 * DIAMETER + 2.0 * cell)),
                    "hole {i} is wider than the tool at {depth}"
                );
            }
            if blind {
                // The full diameter reaches the model's depth, no more.
                assert!(
                    !stock.contains_point(along(reach - 2.0 * cell, 0.0)),
                    "hole {i} is shallow"
                );
                assert!(
                    stock.contains_point(along(reach + 3.0 * cell, 0.0)),
                    "hole {i} is deep"
                );
                assert!(
                    stock.contains_point(along(
                        b.length + 0.25 * point_height(),
                        0.5 * DIAMETER - cell
                    )),
                    "hole {i} is flat-bottomed, not drilled to a point"
                );
            }
        }
    }
}

#[test]
fn req_op_10_gc_08_the_posted_drilling_program_replays_to_the_ir() {
    let job_data = make(&[("peck_depth", 3.0)]);
    let posted = builtin_post("linuxcnc")
        .post(&job_data.toolpath, &job())
        .unwrap()
        .text;
    let program = read(&posted).unwrap();
    let replayed = replay(&program, Dialect::Linuxcnc).unwrap();
    let c = compare(
        &replayed,
        &job_data.toolpath,
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
        safe_z: 27.0,
        spindle_off_at_tool_change: true,
        require_work_offset: true,
        allow_inverse_time: false,
    };
    assert_eq!(static_check(&replayed, &config), []);
}
