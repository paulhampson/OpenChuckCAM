// SPDX-License-Identifier: AGPL-3.0-or-later
//! The flute check on stock earlier operations left reads only the stock
//! within the tool's reach (TL-04, NFR-07, NFR-12; issue #547; decision
//! record 0509).
//!
//! Such stock has no exact answer and is held to a conservative bound. The
//! bound came from the farthest material anywhere, such as a held region
//! the operations never cut, so a finish on a well roughed part was
//! refused. It now comes from the material within the tool's reach along X
//! and towards the tool ([`ReachProfile`]), which still bounds the stock
//! from above, so a cut that reaches tall stock is still refused.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use openchuck_path::flutes::{check, deepest};
use openchuck_path::strategy::{CutPoint, Pass};
use openchuck_path::{MeshSurface, ReachProfile, StockModel};
use openchuck_testkit::parts::revolve;
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::tool::ToolShape;
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, tool};

/// The cells of the stock, mm: as the simulation's.
const CELL: f64 = 0.5;

/// Stock as boxes of material along X, Y and Z, with no exact answer over
/// the tool (as the simulation's stock): only its farthest radius and its
/// reach profile, one slice a cell.
#[derive(Debug, Default)]
struct Cells {
    boxes: Vec<[[f64; 2]; 3]>,
}

impl Cells {
    /// Fills with cells, `CELL` square across Y and Z, the X range `x`
    /// wherever `inside(y, z)` holds at the cell's centre.
    fn fill(&mut self, x: [f64; 2], reach: f64, inside: impl Fn(f64, f64) -> bool) {
        let n = (reach / CELL).ceil() as i32;
        for j in -n..n {
            for k in -n..n {
                let y = f64::from(j) * CELL;
                let z = f64::from(k) * CELL;
                if inside(y + 0.5 * CELL, z + 0.5 * CELL) {
                    self.boxes.push([x, [y, y + CELL], [z, z + CELL]]);
                }
            }
        }
    }
}

impl StockModel for Cells {
    fn contains(&self, p: Point3) -> bool {
        self.boxes.iter().any(|b| {
            (b[0][0]..=b[0][1]).contains(&p.x)
                && (b[1][0]..=b[1][1]).contains(&p.y)
                && (b[2][0]..=b[2][1]).contains(&p.z)
        })
    }
    fn ray_entry(&self, _: Point3, _: UnitVec3, _: f64) -> Option<f64> {
        None
    }
    fn max_radius(&self) -> Option<f64> {
        Some(
            self.boxes
                .iter()
                .flat_map(|b| {
                    [b[1][0], b[1][1]]
                        .into_iter()
                        .flat_map(move |y| [b[2][0], b[2][1]].map(|z| y.hypot(z)))
                })
                .fold(0.0, f64::max),
        )
    }
    fn left_by_earlier_operations(&self) -> bool {
        true
    }
    fn reach_profile(&self) -> Option<ReachProfile> {
        let x0 = -5.0;
        let mut profile = ReachProfile::new(x0, CELL, 60)?;
        for b in &self.boxes {
            let first = ((b[0][0] - x0) / CELL).floor() as usize;
            let last = ((b[0][1] - x0) / CELL).ceil() as usize;
            for i in first..last {
                profile.add(i, b[1], b[2]);
            }
        }
        Some(profile)
    }
}

/// The part: a round bar of radius 5 from X 0 to 20 on the axis.
const BAR: f64 = 5.0;

/// A flat 3.175 mm tool with `flutes` mm of flute on the bar.
fn fixture(flutes: f64) -> Fixture {
    let mut t = tool(ToolShape::FlatEnd, 3.175);
    t.flute_length = Millimetres(flutes);
    t.overall_length = Millimetres(flutes + 20.0);
    Fixture::new(
        MeshSurface::new(&revolve(&[(0.0, BAR), (20.0, BAR)], 96)).unwrap(),
        40.0,
        &t,
    )
}

/// A roughed bar: 0.5 mm left over the bar from X 0 to 22, and the held
/// region of 35 x 35 box stock, never cut, from X -3 to -1.
fn roughed() -> Cells {
    let mut stock = Cells::default();
    stock.fill([0.0, 22.0], BAR + 1.0, |y, z| y.hypot(z) <= BAR + 0.5);
    stock.fill([-3.0, -1.0], 17.5, |_, _| true);
    stock
}

/// A pass at model X from `x0` to `x1` with the tip on the bar's surface,
/// at each A of `angles` in turn (the check reads between them).
fn pass(f: &Fixture, x0: f64, x1: f64, angles: &[f64]) -> Vec<Pass> {
    let mut points = Vec::new();
    for &a in angles {
        for x in [x0, x1] {
            let a = Degrees(a);
            let mut at = f.rotary.to_axis(Point3::new(x, 0.0, 0.0), a);
            at.across = 0.0;
            at.height = BAR;
            let tip = f.rotary.to_model(at, a);
            points.push(CutPoint {
                tip,
                a,
                contact: tip,
            });
        }
    }
    vec![Pass {
        points,
        ..Pass::default()
    }]
}

/// A full turn, every 10 degrees.
fn full_turn() -> Vec<f64> {
    (0..=36).map(|k| 10.0 * f64::from(k)).collect()
}

#[test]
fn req_tl_04_req_nfr_12_stock_out_of_the_tools_reach_does_not_refuse_a_finish() {
    // The finish turns about the bar 4 mm and more from the held region:
    // the stock within the tool's reach stands about 1 mm above the tip
    // (0.5 left, the cells' corners and the bound's 1 mm), well within 2.5
    // mm of flute, though the held region's corners stand 24.749 mm from
    // the axis. On the code before (#547) the bound came from those corners
    // (24.749 - 5 + 1 mm), and this was warned of.
    let f = fixture(2.5);
    let stock = roughed();
    let input = openchuck_path::StrategyInput {
        stock: &stock,
        ..f.input()
    };
    let angles = full_turn();
    let warning = check(&input, &pass(&f, 1.0 + 3.175, 20.0, &angles), None);
    assert!(warning.is_none(), "{warning:?}");
    let worst = deepest(&input, &pass(&f, 1.0 + 3.175, 20.0, &angles), 0.0).unwrap();
    assert!(!worst.exact);
    assert!(worst.depth < 2.5, "{worst:?}");
}

#[test]
fn req_tl_04_req_nfr_12_stock_in_the_tools_reach_is_still_held_to_the_bound() {
    // The same finish taken to X 0, where the tool's 1.5875 mm radius
    // reaches the held region (its face at X -1): warned (decision 0522), at most at the
    // held region's corner over the tip (24.749 mm from the axis, less the
    // tip's 5 mm, plus 1 mm).
    let f = fixture(2.5);
    let stock = roughed();
    let input = openchuck_path::StrategyInput {
        stock: &stock,
        ..f.input()
    };
    let e = check(&input, &pass(&f, 0.0, 20.0, &full_turn()), None)
        .expect("warned")
        .text;
    assert!(e.contains("may stand up to"), "{e}");
    assert!(e.contains("2.500 mm of flute"), "{e}");
    // A full turn at X 0.5 has each corner over the tip in turn.
    let worst = deepest(&input, &pass(&f, 0.5, 0.5, &full_turn()), 0.0).unwrap();
    let corner = 17.5 * std::f64::consts::SQRT_2;
    assert!(worst.depth >= corner - BAR + 1.0 - 1e-9, "{worst:?}");
}

#[test]
fn req_tl_04_req_nfr_12_stock_beside_the_tool_counts_when_the_turn_brings_it_over() {
    // A rib of stock 12 mm from the axis, to one side of the tool at A 0,
    // runs along the bar. A cut along X at A 0 does not have it above the
    // tip (it is square to the tool axis): no warning with 2.5 mm of flute. A
    // turn brings it over the tip, 7 mm above: warned.
    let f = fixture(2.5);
    let side = f.rotary.tool_axis(Degrees(0.0)).get();
    // Square to the tool axis at A 0, about the rotary axis.
    let (ny, nz) = (-side.z, side.y);
    let mut stock = roughed();
    stock.fill([0.0, 22.0], 13.0, |y, z| {
        let along = y * ny + z * nz;
        let up = y * side.y + z * side.z;
        (10.0..=12.0).contains(&along) && up.abs() <= 1.0
    });
    let input = openchuck_path::StrategyInput {
        stock: &stock,
        ..f.input()
    };
    let warning = check(&input, &pass(&f, 5.0, 15.0, &[0.0]), None);
    assert!(warning.is_none(), "{warning:?}");
    let e = check(&input, &pass(&f, 5.0, 15.0, &full_turn()), None)
        .expect("warned")
        .text;
    assert!(e.contains("may stand up to"), "{e}");
    let worst = deepest(&input, &pass(&f, 5.0, 15.0, &full_turn()), 0.0).unwrap();
    assert!(worst.depth >= 12.0 - BAR, "{worst:?}");
}

#[test]
fn req_tl_04_req_nfr_12_a_reach_profile_never_reads_below_its_material() {
    // Boxes of material in every quarter, across the axis and off it, and
    // points within them: the profile's highest over any X range holding a
    // point, towards any direction within its spread, is never below the
    // point's height along that direction.
    let mut profile = ReachProfile::new(-10.0, 1.0, 20).unwrap();
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut boxes = Vec::new();
    for _ in 0..200 {
        let slice = (next() * 20.0) as usize;
        let y0 = next() * 40.0 - 20.0;
        let z0 = next() * 40.0 - 20.0;
        let y = [y0, y0 + next() * 6.0];
        let z = [z0, z0 + next() * 6.0];
        profile.add(slice, y, z);
        boxes.push((slice, y, z));
    }
    for (slice, y, z) in boxes {
        let x = -10.0 + slice as f64 + 0.5;
        for _ in 0..20 {
            let py = y[0] + next() * (y[1] - y[0]);
            let pz = z[0] + next() * (z[1] - z[0]);
            let towards = next() * std::f64::consts::TAU - std::f64::consts::PI;
            let spread = next() * 0.2;
            let off = (next() * 2.0 - 1.0) * spread;
            let d = towards + off;
            let height = py * d.cos() + pz * d.sin();
            let highest = profile.highest([x - 0.1, x + 0.1], towards, spread);
            assert!(
                highest >= height - 1e-9,
                "{highest} < {height} at ({py}, {pz}) towards {towards} +- {spread}"
            );
        }
    }
    // Past the profile's material: nothing.
    assert_eq!(profile.highest([20.0, 30.0], 0.0, 0.0), f64::NEG_INFINITY);
    // A place that is not a number is no bound (NFR-07).
    assert_eq!(profile.highest([f64::NAN, 0.0], 0.0, 0.0), f64::INFINITY);
}

#[test]
fn req_nfr_07_a_reach_profile_of_no_size_is_none() {
    assert!(ReachProfile::new(0.0, 0.0, 10).is_none());
    assert!(ReachProfile::new(f64::NAN, 1.0, 10).is_none());
    assert!(ReachProfile::new(0.0, 1.0, 0).is_none());
    // A box that is not a number fills its slice at infinite reach.
    let mut profile = ReachProfile::new(0.0, 1.0, 1).unwrap();
    profile.add(0, [f64::NAN, 1.0], [0.0, 1.0]);
    profile.add(5, [1.0, 2.0], [0.0, 1.0]);
    assert_eq!(profile.highest([0.0, 0.5], 1.0, 0.0), f64::INFINITY);
}
