// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rest machining reaches the inside corners of steep walls, and says
//! plainly what its tool cannot reach (OP-09, with OP-20's reporting; issue
//! #508; lane RESTCORNERS; decision record 0644).
//!
//! A ball leaves a fillet of its own radius in an inside corner. A flat end
//! of the same diameter, across the rotary axis, reaches that corner where
//! its side meets the wall, but only on a pass at that one place along the
//! part, and on a steep wall it leaves about the spacing of its passes times
//! the wall's slope. Rest machining therefore places its rings where its
//! tool's edge meets the material left on a wall, as close as that slope
//! needs; and material no place of the tool reaches is reported, with where
//! it is and why, instead of being left without a word.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::print_stderr
)]

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::rest::parameter;
use openchuck_path::{
    AcceptAll, GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, RestMachining,
    RotaryMapping, Strategy, StrategyInput, UnreachedReason, generate,
};
use openchuck_sim::{DeviationOptions, DexelStock, Simulator, compare};
use openchuck_testkit::parts::revolve;
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::Millimetres;

const CELL: f64 = 0.1;
const LENGTH: f64 = 24.0;
/// The tools' diameter: the owner's 1/8 inch ball and flat end.
const D: f64 = 3.175;
/// Material thinner than this is clean, as the owner's operation sets it.
const MIN_REMAINING: f64 = 0.05;
/// The stepover the owner's flat end takes from its cutting data.
const COARSE: f64 = 2.2225;

fn tool(shape: ToolShape, d: f64) -> Tool {
    Tool {
        shape,
        diameter: Millimetres(d),
        flute_length: Millimetres(20.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(d),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

fn ball() -> RevolvedProfile {
    tool(ToolShape::BallEnd, D).profile().unwrap()
}

fn flat() -> RevolvedProfile {
    tool(ToolShape::FlatEnd, D).profile().unwrap()
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 1.0),
        (names::FEED, 400.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

/// The rest operation of the owner's project: a coarse stepover, a fine
/// threshold, in rings or a spiral.
fn rest_values(shape: &str) -> BTreeMap<String, ParameterValue> {
    values(&[
        (names::STEPOVER, ParameterValue::Number(COARSE)),
        (
            parameter::MIN_REMAINING,
            ParameterValue::Number(MIN_REMAINING),
        ),
        (
            parameter::PASS_SHAPE,
            ParameterValue::Text(shape.to_owned()),
        ),
    ])
}

struct Rig {
    machine: MachineProfile,
    target: TriangleMesh,
    surface: MeshSurface,
    tags: PathTags,
    keep_out: Vec<(f64, f64)>,
}

impl Rig {
    fn new(profile: &[(f64, f64)]) -> Self {
        let target = revolve(profile, 180);
        Self {
            machine: MachineProfile::example_x_rotary(),
            surface: MeshSurface::new(&target).unwrap(),
            target,
            tags: PathTags {
                operation: "op".to_owned(),
                setup: "setup".to_owned(),
                tool: "tool".to_owned(),
            },
            keep_out: Vec::new(),
        }
    }

    /// A cylinder of radius 6 that steps up to radius 10 at X 12 through a
    /// wall 80 degrees from the axis: the bead's foot on the owner's part.
    fn steep_wall() -> Self {
        let rise = 4.0 / 80_f64.to_radians().tan();
        Self::new(&[(0.0, 6.0), (12.0, 6.0), (12.0 + rise, 10.0), (LENGTH, 10.0)])
    }

    /// A bead, a half round of radius 3 standing on a cylinder of radius 8
    /// from X 6 to 12, then a cove, a half round groove of radius 3 from X
    /// 12 to 18, in eight facets each as the Baluster example has them: the
    /// bead's flank runs down into the cove's wall, near vertical, where a
    /// ball's passes along X leave steps.
    fn cove() -> Self {
        let mut profile = vec![(0.0, 8.0), (6.0, 8.0)];
        for i in 1..8 {
            let a = std::f64::consts::PI * f64::from(i) / 8.0;
            profile.push((9.0 - 3.0 * a.cos(), 8.0 + 3.0 * a.sin()));
        }
        profile.push((12.0, 8.0));
        for i in 1..8 {
            let a = std::f64::consts::PI * f64::from(i) / 8.0;
            profile.push((15.0 - 3.0 * a.cos(), 8.0 - 3.0 * a.sin()));
        }
        profile.extend([(18.0, 8.0), (LENGTH, 8.0)]);
        Self::new(&profile)
    }

    /// A V groove 1 mm wide at the top and 4 mm deep: narrower than the
    /// flat end, whose corners cannot go down it.
    fn narrow_vee() -> Self {
        Self::new(&[
            (0.0, 10.0),
            (11.5, 10.0),
            (12.0, 6.0),
            (12.5, 10.0),
            (LENGTH, 10.0),
        ])
    }

    /// A shoulder square to the axis at X 12, from radius 6 to 10.
    fn shoulder() -> Self {
        Self::new(&[(0.0, 6.0), (12.0, 6.0), (12.0, 10.0), (LENGTH, 10.0)])
    }

    fn stock(&self) -> DexelStock {
        let stock = Stock::Cylinder {
            diameter: Millimetres(22.0),
            length: Millimetres(LENGTH),
            x_start: Millimetres(0.0),
        };
        DexelStock::from_stock(&stock, None, CELL).unwrap()
    }

    fn generate(
        &self,
        strategy: &dyn Strategy,
        tool: &RevolvedProfile,
        stock: &DexelStock,
        v: &BTreeMap<String, ParameterValue>,
    ) -> openchuck_path::Result<GeneratedPath> {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let input = StrategyInput {
            surface: &self.surface,
            stock,
            collision: &AcceptAll,
            tool,
            rotary: RotaryMapping::of_profile(&self.machine),
            clearance: Clearance {
                radius: Millimetres(19.0),
                safe_z: Millimetres(29.0),
            },
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
            entry_safety: None,
            facing: None,
            keep_out: &self.keep_out,
            shank: None,
        };
        generate(strategy, &input, v, &ctx)
    }

    fn simulate(
        &self,
        stock: DexelStock,
        tool: &RevolvedProfile,
        path: &GeneratedPath,
    ) -> DexelStock {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let mut sim = Simulator::without_checkpoints(stock, &self.machine);
        let result = sim.run_group(&path.group, tool, &ctx).unwrap();
        assert!(result.rapid_cuts.is_empty(), "a rapid move cuts");
        sim.into_stock()
    }

    /// The most material left on the part between X `lo` and `hi`, and the
    /// samples overcut anywhere, as the simulation's deviation reports them.
    fn worst_between(&self, stock: &DexelStock, lo: f64, hi: f64) -> (f64, usize) {
        let d = compare(
            stock,
            &self.target,
            &DeviationOptions {
                spacing: 0.05,
                ..DeviationOptions::for_cell(CELL)
            },
        )
        .unwrap();
        let worst = d
            .points
            .iter()
            .filter(|p| p.counts() && p.point.x >= lo && p.point.x <= hi)
            .map(|p| p.value)
            .fold(0.0, f64::max);
        (worst, d.overcut_count)
    }

    /// The stock after helical finishing with the ball, which leaves a
    /// fillet of its radius in every inside corner.
    fn after_ball(&self) -> DexelStock {
        self.after_ball_at(0.3)
    }

    /// [`after_ball`](Self::after_ball) with the ball's passes `stepover`
    /// apart and no adaptive spacing between them, as the owner's
    /// simultaneous finishing had none.
    fn after_ball_at(&self, stepover: f64) -> DexelStock {
        let b = ball();
        let v = values(&[
            (names::STEPOVER, ParameterValue::Number(stepover)),
            ("adaptive_spacing", ParameterValue::Flag(false)),
        ]);
        let path = self
            .generate(&HelicalFinishing, &b, &self.stock(), &v)
            .unwrap();
        self.simulate(self.stock(), &b, &path)
    }
}

/// The ball leaves more than a quarter of a millimetre in the corner near
/// X `at`, the flat end's rest operation cleans it to the threshold, within
/// what the simulation can know (a cell), and nothing is overcut.
fn assert_rest_cleans(rig: &Rig, shape: &str, (lo, hi): (f64, f64)) -> GeneratedPath {
    assert_rest_cleans_after(rig, rig.after_ball(), shape, (lo, hi))
}

fn assert_rest_cleans_after(
    rig: &Rig,
    stock: DexelStock,
    shape: &str,
    (lo, hi): (f64, f64),
) -> GeneratedPath {
    let (before, _) = rig.worst_between(&stock, lo, hi);
    let f = flat();
    let path = rig
        .generate(&RestMachining, &f, &stock, &rest_values(shape))
        .unwrap();
    let after_stock = rig.simulate(stock, &f, &path);
    let (after, overcut) = rig.worst_between(&after_stock, lo, hi);
    eprintln!("{shape}: X {lo} to {hi}: {before:.3} -> {after:.3} mm left, {overcut} overcut");
    assert!(before > 0.25, "the ball should leave the corner: {before}");
    assert!(
        after <= MIN_REMAINING + CELL,
        "rest machining leaves {after:.3} mm in a corner its flat end reaches"
    );
    assert_eq!(overcut, 0, "the rest operation gouges");
    path
}

#[test]
fn req_op_09_rest_reaches_the_inside_corner_of_a_steep_wall_in_rings() {
    let rig = Rig::steep_wall();
    let path = assert_rest_cleans(&rig, "rings", (10.0, 13.0));
    assert!(path.unreached.is_empty(), "{:?}", path.unreached);
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("where its tool's side meets")),
        "{:?}",
        path.notes
    );
}

#[test]
fn req_op_09_rest_reaches_the_inside_corner_of_a_steep_wall_in_a_spiral() {
    let rig = Rig::steep_wall();
    let path = assert_rest_cleans(&rig, "helix", (10.0, 13.0));
    assert!(path.unreached.is_empty(), "{:?}", path.unreached);
}

#[test]
fn req_op_09_rest_cleans_the_walls_of_a_cove_with_a_flat_end() {
    let rig = Rig::cove();
    // The owner's ball passes were 0.45 mm apart; on the near-vertical
    // walls they leave steps.
    let path = assert_rest_cleans_after(&rig, rig.after_ball_at(0.45), "rings", (10.0, 14.0));
    // The cove's round bottom is rounder than a flat end can follow: at its
    // middle the flat bottom, 3.175 mm across, stands 3 - (3^2 - 1.5875^2)^0.5
    // = 0.45 mm above it. That is said, and only there.
    assert!(!path.unreached.is_empty());
    for u in &path.unreached {
        assert_eq!(u.reason, UnreachedReason::TooNarrow, "{u:?}");
        assert!(u.x.0 > 13.0 && u.x.1 < 17.0, "{u:?}");
    }
}

#[test]
fn req_op_09_rest_reports_a_groove_too_narrow_for_its_tool() {
    let rig = Rig::narrow_vee();
    let stock = rig.after_ball();
    let path = rig
        .generate(&RestMachining, &flat(), &stock, &rest_values("rings"))
        .unwrap();
    let narrow: Vec<_> = path
        .unreached
        .iter()
        .filter(|u| u.reason == UnreachedReason::TooNarrow)
        .collect();
    assert!(!narrow.is_empty(), "{:?}", path.unreached);
    for u in &narrow {
        assert!(u.x.0 >= 11.4 && u.x.1 <= 12.6, "{:?}", u.x);
        assert!(u.area > 0.0);
        assert!(
            u.a.1 - u.a.0 >= 359.0,
            "the groove goes all round: {:?}",
            u.a
        );
    }
    let warning = path
        .warnings
        .iter()
        .map(|w| w.message.as_str())
        .find(|m| m.contains("cannot reach"))
        .expect("a warning says what the tool cannot reach");
    assert!(warning.contains("too narrow"), "{warning}");
    assert!(
        warning.contains("X 11.") || warning.contains("X 12."),
        "{warning}"
    );
    assert!(warning.contains("verified by the user"), "{warning}");
}

#[test]
fn req_op_09_req_set_09_rest_reports_a_corner_the_held_region_keeps_the_tool_from() {
    // The tool's tip never goes below X 11, so its side cannot come within
    // 1 mm of the shoulder at X 12 (decision record 0161).
    let mut rig = Rig::shoulder();
    rig.keep_out = vec![(-100.0, 11.0)];
    let stock = rig.after_ball();
    let path = rig
        .generate(&RestMachining, &flat(), &stock, &rest_values("rings"))
        .unwrap();
    let held: Vec<_> = path
        .unreached
        .iter()
        .filter(|u| u.reason == UnreachedReason::NearHeldRegion)
        .collect();
    assert!(!held.is_empty(), "{:?}", path.unreached);
    // The part under the keep-out, up to the shoulder's corner at X 12.
    assert!(
        held.iter().any(|u| u.x.1 >= 11.9 && u.x.1 <= 12.1),
        "{held:?}"
    );
    let warning = path
        .warnings
        .iter()
        .map(|w| w.message.as_str())
        .find(|m| m.contains("cannot reach"))
        .expect("a warning says what the tool cannot reach");
    assert!(warning.contains("held region"), "{warning}");
}

#[test]
fn req_op_09_req_set_09_rest_cuts_up_to_a_held_region_and_reports_the_strip_it_cannot() {
    // A cylinder with 1 mm of stock round it, held below X 3: the tool's tip
    // keeps out of X < 3 + 1.5875 + 0.5 (decision record 0161), so its flat
    // reaches down to X 3.5 and no further.
    let mut rig = Rig::new(&[(0.0, 9.0), (LENGTH, 9.0)]);
    let reach = 0.5 * D + openchuck_path::HELD_CLEARANCE;
    rig.keep_out = vec![(-100.0, 3.0 + reach)];
    let stock = {
        let stock = Stock::Cylinder {
            diameter: Millimetres(20.0),
            length: Millimetres(LENGTH),
            x_start: Millimetres(0.0),
        };
        DexelStock::from_stock(&stock, None, CELL).unwrap()
    };
    let f = flat();
    let path = rig
        .generate(&RestMachining, &f, &stock, &rest_values("rings"))
        .unwrap();
    let after = rig.simulate(stock, &f, &path);
    // Reached: from the flat's edge, X 3.5, on.
    let (left, overcut) = rig.worst_between(&after, 3.5 + CELL, LENGTH - 0.5);
    eprintln!("beside the held region: {left:.3} mm left, {overcut} overcut");
    assert!(left <= MIN_REMAINING + CELL, "{left}");
    assert_eq!(overcut, 0);
    // Not reached: the strip from the grip's own edge, X 3, to X 3.5.
    let held: Vec<_> = path
        .unreached
        .iter()
        .filter(|u| u.reason == UnreachedReason::NearHeldRegion)
        .collect();
    assert_eq!(held.len(), 1, "{:?}", path.unreached);
    assert!(
        held[0].x.0 >= 3.0 - 1e-9 && held[0].x.1 <= 3.5 + CELL,
        "{held:?}"
    );
    assert!(held[0].a.1 - held[0].a.0 >= 359.0, "{held:?}");
    // The part under the grip is not reported.
    assert!(
        path.unreached.iter().all(|u| u.x.0 >= 3.0 - 1e-9),
        "{:?}",
        path.unreached
    );
}

#[test]
fn req_op_09_rest_adds_no_wall_ring_where_no_wall_holds_material() {
    // A plain cylinder under a thicker one: material is left all along, on
    // a surface round the axis, with no wall to meet.
    let rig = Rig::new(&[(0.0, 10.0), (LENGTH, 10.0)]);
    let stock = rig.stock();
    let path = rig
        .generate(&RestMachining, &flat(), &stock, &rest_values("rings"))
        .unwrap();
    assert!(
        !path
            .notes
            .iter()
            .any(|n| n.message.contains("where its tool's side meets")),
        "{:?}",
        path.notes
    );
    assert!(path.unreached.is_empty(), "{:?}", path.unreached);
}

#[test]
fn req_op_09_rest_does_not_claim_its_passes_clean_the_material() {
    let rig = Rig::steep_wall();
    let stock = rig.after_ball();
    let path = rig
        .generate(&RestMachining, &flat(), &stock, &rest_values("rings"))
        .unwrap();
    let note = path
        .notes
        .iter()
        .map(|n| n.message.as_str())
        .find(|m| m.contains("points sampled on the part"))
        .unwrap();
    assert!(!note.contains("clean them"), "{note}");
    assert!(note.contains("cut where"), "{note}");
}

#[test]
fn req_nfr_06_req_op_09_rest_wall_rings_are_identical_at_1_2_and_n_threads() {
    let rig = Rig::cove();
    let stock = rig.after_ball_at(0.45);
    let f = flat();
    let runs = openchuck_testkit::determinism::check(|| {
        let rings = rig
            .generate(&RestMachining, &f, &stock, &rest_values("rings"))
            .unwrap();
        let spiral = rig
            .generate(&RestMachining, &f, &stock, &rest_values("helix"))
            .unwrap();
        format!("{:?}{:?}{:?}", rings.group, spiral.group, rings.unreached).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}
