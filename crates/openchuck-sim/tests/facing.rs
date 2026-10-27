// SPDX-License-Identifier: AGPL-3.0-or-later
//! Facing and extending past the stock against the simulated stock (OP-24;
//! decision record 0212 part B).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use openchuck_path::extend::parameter as extend;
use openchuck_path::facing::parameter as facing;
use openchuck_path::params::names;
use openchuck_path::roughing::parameter as roughing;
use openchuck_path::{
    AcceptAll, Facing, MeshSurface, ParameterValue, RotaryMapping, RotaryRoughing, Strategy,
    StrategyInput, generate,
};
use openchuck_sim::{DexelStock, Simulator};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

const CELL: f64 = 0.2;

/// Stock of radius 12 from X -12 to 32, longer than roughing's margin past
/// the part at each end; the part, radius 8, from 0 to 20.
const STOCK: (f64, f64) = (-12.0, 32.0);

fn run(strategy: &dyn Strategy, v: &BTreeMap<String, ParameterValue>) -> DexelStock {
    let machine = MachineProfile::example_x_rotary();
    let part = Cylinder {
        radius: 8.0,
        length: 20.0,
    };
    let surface = MeshSurface::new(&part.mesh(20, 180)).unwrap();
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(4.0),
        flute_length: Millimetres(20.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(4.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let profile = tool.profile().unwrap();
    let stock = DexelStock::from_stock(
        &Stock::Cylinder {
            diameter: Millimetres(24.0),
            length: Millimetres(STOCK.1 - STOCK.0),
            x_start: Millimetres(STOCK.0),
        },
        None,
        CELL,
    )
    .unwrap();
    let tags = PathTags {
        operation: "op".to_owned(),
        setup: "setup".to_owned(),
        tool: "tool".to_owned(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &profile,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(17.0),
            safe_z: Millimetres(27.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let path = generate(strategy, &input, v, &ctx).unwrap();
    let mut sim = Simulator::without_checkpoints(stock.clone(), &machine);
    let result = sim.run_group(&path.group, &profile, &ctx).unwrap();
    assert!(result.rapid_cuts.is_empty(), "{:?}", result.rapid_cuts);
    sim.into_stock()
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 2.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.5),
        (roughing::STEPDOWN, 2.0),
        (roughing::MAX_ANGLE_STEP, 10.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

/// How many sample points between radius `r` and the stock's outside, at
/// the stock's last millimetre of each end, still hold material.
fn skin(stock: &DexelStock, r: f64) -> (usize, usize) {
    let mut out = (0, 0);
    for (end, x) in [(0, STOCK.0 + 0.5), (1, STOCK.1 - 0.5)] {
        for k in 0..36 {
            let angle = f64::from(k).to_radians() * 10.0;
            let mut radius = r + 3.0 * CELL;
            while radius < 11.5 {
                let p = Point3::new(x, radius * angle.cos(), radius * angle.sin());
                if stock.contains_point(p) {
                    if end == 0 {
                        out.0 += 1;
                    } else {
                        out.1 += 1;
                    }
                }
                radius += 0.5;
            }
        }
    }
    out
}

#[test]
fn req_op_24_facing_round_past_both_ends_leaves_no_skin_at_the_stock_ends() {
    let stock = run(
        &Facing,
        &values(&[
            (
                facing::SHAPE,
                ParameterValue::Text(facing::ROUND.to_owned()),
            ),
            (facing::SIZE, ParameterValue::Number(20.0)),
            (
                extend::EXTEND,
                ParameterValue::Text(extend::BOTH_ENDS.to_owned()),
            ),
            (extend::EXTEND_DISTANCE, ParameterValue::Number(0.4)),
        ]),
    );
    assert_eq!(skin(&stock, 10.0), (0, 0));
}

#[test]
fn req_op_24_roughing_past_the_far_end_clears_the_stock_there_and_leaves_the_start() {
    let rough = |ends: &str| {
        run(
            &RotaryRoughing,
            &values(&[
                (extend::EXTEND, ParameterValue::Text(ends.to_owned())),
                (extend::EXTEND_DISTANCE, ParameterValue::Number(0.4)),
            ]),
        )
    };
    // Without the option the stock beyond the part's margin is not touched
    // (decision 0033): the ends keep their skin.
    let (start, end) = skin(&rough(extend::NO), 8.5);
    assert!(start > 0 && end > 0, "{start} {end}");
    // Past the far end, the far end is cleared down to the layers' floor and
    // the start keeps its stock.
    let (start, end) = skin(&rough(extend::FAR_END), 8.5);
    assert!(start > 0, "{start}");
    assert_eq!(end, 0);
}
