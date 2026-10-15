// SPDX-License-Identifier: AGPL-3.0-or-later
//! The remaining-stock heightfield rotary roughing plans against (OP-03,
//! OP-14; decision record 0033 item 2).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use openchuck_path::{AnalyticStock, HeightStock, RotaryMapping, StockModel};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::tool::ToolShape;
use support::{machine, tool};

fn field(radius: f64) -> (HeightStock, RotaryMapping) {
    let rotary = RotaryMapping::of_profile(&machine());
    let stock = AnalyticStock::Cylinder {
        radius,
        x_start: 0.0,
        x_end: 30.0,
    };
    let f = HeightStock::from_stock(&stock, rotary, (-5.0, 35.0), radius, 2.0, 0.5).unwrap();
    (f, rotary)
}

fn down(rotary: &RotaryMapping, a: f64) -> UnitVec3 {
    UnitVec3::new(-rotary.tool_axis(openchuck_types::units::Degrees(a)).get()).unwrap()
}

#[test]
fn req_op_03_the_field_reads_the_stock_it_is_built_from() {
    let (f, _) = field(12.0);
    assert!(f.max_radius().unwrap() > 11.9 && f.max_radius().unwrap() <= 12.0 + 1e-6);
    assert!(f.contains(Point3::new(10.0, 0.0, 5.0)));
    assert!(f.contains(Point3::new(10.0, 8.0, 8.0)));
    assert!(
        !f.contains(Point3::new(10.0, 0.0, 13.5)),
        "outside the radius"
    );
    assert!(!f.contains(Point3::new(-3.0, 0.0, 5.0)), "before the stock");
    assert!(!f.contains(Point3::new(32.0, 0.0, 5.0)), "past the stock");
    // A ray from above meets the top, within a cell or so (the queries are
    // dilated by a cell on the safe side).
    let rotary = RotaryMapping::of_profile(&machine());
    for a in [0.0, 37.0, 140.0, 290.0] {
        let origin = rotary.to_model(
            openchuck_path::rotary::AxisPosition {
                along: 15.0,
                across: 0.0,
                height: 20.0,
            },
            openchuck_types::units::Degrees(a),
        );
        let d = f.ray_entry(origin, down(&rotary, a), 40.0).unwrap();
        assert!((d - 8.0).abs() < 0.6, "{a}: {d}");
    }
    // A ray that stays clear of the stock.
    let miss = f.ray_entry(
        Point3::new(15.0, 0.0, 20.0),
        UnitVec3::new(Vec3::new(1.0, 0.0, 0.0)).unwrap(),
        40.0,
    );
    assert!(miss.is_none());
}

#[test]
fn req_op_03_a_cut_removes_the_cells_the_tool_covers_and_no_others() {
    let (mut f, rotary) = field(12.0);
    let profile = tool(ToolShape::FlatEnd, 6.0).profile().unwrap();
    // The tool over the top (A = 0 faces +Z... wherever it is, the same
    // point on the axis line), tip at height 10, a flat bottom 3 mm in radius.
    let at = |along: f64, across: f64, height: f64, a: f64| {
        rotary.to_model(
            openchuck_path::rotary::AxisPosition {
                along,
                across,
                height,
            },
            openchuck_types::units::Degrees(a),
        )
    };
    let before = f.excess(&profile, 15.0, 0.0, 0.0, 10.0);
    assert!(before > 1.5, "{before}");
    f.cut(&profile, 15.0, 0.0, 0.0, 10.0, 0.0);
    // The material above the tip under the tool's centre is gone ...
    assert!(!f.contains(at(15.0, 0.0, 10.6, 0.0)));
    assert!(f.contains(at(15.0, 0.0, 9.0, 0.0)), "below the tip stays");
    // ... and none is gone beyond its reach along the axis or across it.
    assert!(
        f.contains(at(20.0, 0.0, 11.5, 0.0)),
        "beyond the radius along X"
    );
    assert!(f.contains(at(15.0, 0.0, 11.5, 40.0)), "round the part");
    // Nothing left under the tool to cut at that depth.
    assert!(f.excess(&profile, 15.0, 0.0, 0.0, 10.0) <= 0.05);
    // Cutting again changes nothing.
    let g = f.clone();
    f.cut(&profile, 15.0, 0.0, 0.0, 10.0, 0.0);
    for ix in 0..f.cells().0 {
        for ia in 0..f.cells().1 {
            assert_eq!(f.top_at(ix, ia), g.top_at(ix, ia));
        }
    }
}

#[test]
fn req_op_14_a_cell_the_tool_only_partly_covers_is_not_taken_as_cut() {
    let (mut f, _) = field(12.0);
    let profile = tool(ToolShape::FlatEnd, 6.0).profile().unwrap();
    // Centred on a cell edge at the tool's reach: the outermost cells are not
    // wholly inside, so their material stays (the safe side).
    f.cut(&profile, 15.0, 0.0, 0.0, 10.0, 0.0);
    let (nx, na) = f.cells();
    let mut cut_cells = 0;
    let mut partial = 0;
    for ix in 0..nx {
        for ia in 0..na {
            let top = f.top_at(ix, ia);
            if top < 11.0 {
                cut_cells += 1;
            } else if top < 11.99 {
                partial += 1;
            }
        }
    }
    assert!(cut_cells > 20, "{cut_cells}");
    assert_eq!(partial, 0, "a cell is either cut to the tool or untouched");
    // A larger tool cuts more cells.
    let (mut g, _) = field(12.0);
    let big = tool(ToolShape::FlatEnd, 10.0).profile().unwrap();
    g.cut(&big, 15.0, 0.0, 0.0, 10.0, 0.0);
    let more = (0..nx)
        .flat_map(|ix| (0..na).map(move |ia| (ix, ia)))
        .filter(|(ix, ia)| g.top_at(*ix, *ia) < 11.0)
        .count();
    assert!(more > cut_cells);
}

#[test]
fn req_op_03_the_field_is_refused_for_a_degenerate_size() {
    let rotary = RotaryMapping::of_profile(&machine());
    let stock = AnalyticStock::Cylinder {
        radius: 5.0,
        x_start: 0.0,
        x_end: 10.0,
    };
    assert!(HeightStock::from_stock(&stock, rotary, (5.0, 5.0), 5.0, 1.0, 0.5).is_err());
    assert!(HeightStock::from_stock(&stock, rotary, (0.0, 10.0), 0.0, 1.0, 0.5).is_err());
    assert!(HeightStock::from_stock(&stock, rotary, (0.0, 10.0), 5.0, 1.0, 0.0).is_err());
    // A huge part gets coarser cells, not an out-of-memory.
    let big = HeightStock::from_stock(&stock, rotary, (0.0, 10_000.0), 5.0, 1.0, 0.01).unwrap();
    let (nx, na) = big.cells();
    assert!(nx * na <= openchuck_path::field::MAX_CELLS);
}
