// SPDX-License-Identifier: AGPL-3.0-or-later
//! Extending past the stock, and facing (OP-24; decision record 0212 part B).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;
use support::generate;

use std::collections::BTreeMap;

use openchuck_path::extend::parameter as extend;
use openchuck_path::facing::parameter as facing;
use openchuck_path::params::names;
use openchuck_path::roughing::parameter as roughing;
use openchuck_path::{
    AnalyticStock, AxialFinishing, Facing, GeneratedPath, HelicalFinishing, IndexedFinishing,
    MeshSurface, ParameterKind, ParameterValue, RotaryRoughing, Strategy,
};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::ir::MoveKind;
use openchuck_types::tool::{Tool, ToolShape};
use support::{Fixture, run, tool};

/// The part: a cylinder of radius 8 from X 0 to 20.
fn part() -> Cylinder {
    Cylinder {
        radius: 8.0,
        length: 20.0,
    }
}

/// Stock of radius 12 from X -6 to 26, longer than the part at both ends.
const STOCK: (f64, f64) = (-6.0, 26.0);

fn fixture(t: &Tool) -> Fixture {
    let surface = MeshSurface::new(&part().mesh(40, 360)).unwrap();
    let mut f = Fixture::new(surface, 12.0, t);
    f.stock = AnalyticStock::Cylinder {
        radius: 12.0,
        x_start: STOCK.0,
        x_end: STOCK.1,
    };
    f
}

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 2.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (roughing::STEPDOWN, 2.0),
        ("max_angle_step", 10.0),
        ("max_x_step", 1.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

/// The values `strategy` takes from `v` (each schema refuses names it does
/// not know).
fn only_known(
    strategy: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
) -> BTreeMap<String, ParameterValue> {
    let schema = strategy.schema();
    v.iter()
        .filter(|(k, _)| schema.get(k).is_some())
        .map(|(k, x)| (k.clone(), x.clone()))
        .collect()
}

fn path(
    f: &Fixture,
    strategy: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
) -> openchuck_path::Result<GeneratedPath> {
    run(|ctx| generate(strategy, &f.input(), &only_known(strategy, v), ctx))
}

/// The lowest and highest X the tool's tip reaches while it cuts, its
/// entries and exits included (a pass's first point is reached by its
/// entry).
fn cut_x(p: &GeneratedPath) -> (f64, f64) {
    p.group
        .segments
        .iter()
        .filter(|s| s.kind.is_fed() && s.kind != MoveKind::Linking)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), s| {
            (lo.min(s.end.tip.x), hi.max(s.end.tip.x))
        })
}

fn flat(d: f64) -> Tool {
    tool(ToolShape::FlatEnd, d)
}

fn strategies() -> Vec<(&'static str, Box<dyn Strategy>)> {
    vec![
        ("roughing", Box::new(RotaryRoughing)),
        ("helical", Box::new(HelicalFinishing)),
        ("axial", Box::new(AxialFinishing)),
        ("indexed", Box::new(IndexedFinishing)),
    ]
}

#[test]
fn req_op_24_req_gc_07_the_option_is_off_by_default_and_changes_nothing() {
    let f = fixture(&flat(4.0));
    for (name, strategy) in strategies() {
        let schema = strategy.schema();
        let spec = schema.get(extend::EXTEND).unwrap();
        assert!(!spec.label.is_empty() && !spec.description.is_empty());
        let ParameterKind::Choice { options, default } = &spec.kind else {
            panic!("{name}: extend is a choice");
        };
        assert_eq!(default, extend::NO, "{name}");
        assert_eq!(
            options,
            &[
                extend::NO,
                extend::BOTH_ENDS,
                extend::START_END,
                extend::FAR_END
            ],
            "{name}"
        );
        // The sides are only Facing's.
        assert!(schema.get(extend::EXTEND_SIDES).is_none(), "{name}");
        let base = values(&[]);
        let before = path(&f, strategy.as_ref(), &base).unwrap();
        let mut off = base.clone();
        off.insert(extend::EXTEND.to_owned(), text(extend::NO));
        off.insert(extend::EXTEND_DISTANCE.to_owned(), num(1.5));
        assert_eq!(path(&f, strategy.as_ref(), &off).unwrap(), before, "{name}");
    }
}

#[test]
fn req_op_24_each_strategy_carries_its_passes_past_the_chosen_ends_of_the_stock() {
    let t = flat(4.0);
    let f = fixture(&t);
    let (r, d) = (2.0, 0.5);
    for (name, strategy) in strategies() {
        let before = cut_x(&path(&f, strategy.as_ref(), &values(&[])).unwrap());
        for (ends, start, end) in [
            (extend::BOTH_ENDS, true, true),
            (extend::START_END, true, false),
            (extend::FAR_END, false, true),
        ] {
            let v = values(&[
                (extend::EXTEND, text(ends)),
                (extend::EXTEND_DISTANCE, num(d)),
            ]);
            let p = path(&f, strategy.as_ref(), &v).unwrap();
            let (lo, hi) = cut_x(&p);
            // The tool's near side is the distance past the stock's edge:
            // its tip is a radius and the distance beyond it.
            if start {
                assert!((lo - (STOCK.0 - r - d)).abs() < 1e-6, "{name} {ends}: {lo}");
            } else {
                assert!(lo >= before.0 - 1e-9, "{name} {ends}: {lo} vs {}", before.0);
            }
            if end {
                assert!((hi - (STOCK.1 + r + d)).abs() < 1e-6, "{name} {ends}: {hi}");
            } else {
                assert!(hi <= before.1 + 1e-9, "{name} {ends}: {hi} vs {}", before.1);
            }
            // Never into the part (OP-12).
            let poses: Vec<Pose> = p
                .group
                .segments
                .iter()
                .filter(|s| s.kind.is_fed())
                .map(|s| Pose {
                    tip: s.end.tip,
                    axis: s.end.axis.get(),
                })
                .collect();
            let g = max_gouge(&Outline::new(&f.tool), &part(), &poses, 0.25);
            assert!(g.depth < 0.02, "{name} {ends}: gouge {}", g.depth);
        }
    }
}

#[test]
fn req_op_24_req_nfr_07_a_distance_beyond_the_tool_diameter_is_refused_by_name() {
    let f = fixture(&flat(4.0));
    for (name, strategy) in strategies() {
        let v = values(&[
            (extend::EXTEND, text(extend::BOTH_ENDS)),
            (extend::EXTEND_DISTANCE, num(4.5)),
        ]);
        let err = path(&f, strategy.as_ref(), &v).unwrap_err().to_string();
        assert!(
            err.contains(extend::EXTEND_DISTANCE) && err.contains("diameter"),
            "{name}: {err}"
        );
        // The full diameter is allowed.
        let v = values(&[
            (extend::EXTEND, text(extend::BOTH_ENDS)),
            (extend::EXTEND_DISTANCE, num(4.0)),
        ]);
        assert!(path(&f, strategy.as_ref(), &v).is_ok(), "{name}");
        let v = values(&[(extend::EXTEND_DISTANCE, num(-0.5))]);
        let err = path(&f, strategy.as_ref(), &v).unwrap_err().to_string();
        assert!(err.contains(extend::EXTEND_DISTANCE), "{name}: {err}");
    }
}

#[test]
fn req_op_24_req_set_09_an_end_held_by_the_workholding_is_refused_not_cut_short() {
    let f = fixture(&flat(4.0));
    // The start of the stock is held: X -6 to -2, widened by the tool.
    let keep_out = [(-8.0, 0.0)];
    let input = openchuck_path::StrategyInput {
        facing: None,
        keep_out: &keep_out,
        ..f.input()
    };
    for (name, strategy) in strategies() {
        for ends in [extend::BOTH_ENDS, extend::START_END] {
            let v = only_known(strategy.as_ref(), &values(&[(extend::EXTEND, text(ends))]));
            let err = run(|ctx| generate(strategy.as_ref(), &input, &v, ctx))
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("start of the stock is held") && err.contains(extend::EXTEND),
                "{name} {ends}: {err}"
            );
        }
        // The far end is free.
        let v = only_known(
            strategy.as_ref(),
            &values(&[(extend::EXTEND, text(extend::FAR_END))]),
        );
        let p = run(|ctx| generate(strategy.as_ref(), &input, &v, ctx)).unwrap();
        assert!(cut_x(&p).1 > STOCK.1, "{name}");
    }
}

fn facing(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v = values(&[(names::STOCK_TO_LEAVE, num(0.5))]);
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

/// The height (along the tool axis, from the rotary axis) of each cutting
/// move's end.
fn heights(f: &Fixture, p: &GeneratedPath) -> Vec<f64> {
    p.group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| f.rotary.to_axis(s.end.tip, s.end.a).height)
        .collect()
}

#[test]
fn req_op_24_facing_flat_cuts_the_whole_stock_down_to_the_face() {
    let f = fixture(&flat(4.0));
    let v = facing(&[(facing::SIZE, num(10.0)), (facing::ANGLES, text("0, 180"))]);
    let p = path(&f, &Facing, &v).unwrap();
    let h = heights(&f, &p);
    let lowest = h.iter().copied().fold(f64::INFINITY, f64::min);
    // Down to the face, never below it.
    assert!((lowest - 10.0).abs() < 1e-6, "{lowest}");
    // A held, at the two listed angles.
    for s in p
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
    {
        let a = s.end.a.0.rem_euclid(360.0);
        assert!(a.abs() < 1e-9 || (a - 180.0).abs() < 1e-9, "{a}");
    }
    // The whole stock's length, not only the part's: the tool's tip reaches
    // the stock's ends.
    let (lo, hi) = cut_x(&p);
    assert!(lo <= STOCK.0 + 1e-6 && hi >= STOCK.1 - 1e-6, "{lo} {hi}");
}

#[test]
fn req_op_24_facing_round_turns_the_stock_to_the_diameter_and_extends_past_its_ends() {
    let f = fixture(&flat(4.0));
    let v = facing(&[
        (facing::SHAPE, text(facing::ROUND)),
        (facing::SIZE, num(20.0)),
        (extend::EXTEND, text(extend::BOTH_ENDS)),
        (extend::EXTEND_DISTANCE, num(0.4)),
    ]);
    let p = path(&f, &Facing, &v).unwrap();
    let h = heights(&f, &p);
    let lowest = h.iter().copied().fold(f64::INFINITY, f64::min);
    assert!((lowest - 10.0).abs() < 1e-6, "{lowest}");
    let (lo, hi) = cut_x(&p);
    assert!(lo <= STOCK.0 - 2.0 - 0.4 + 1e-6, "{lo}");
    assert!(hi >= STOCK.1 + 2.0 + 0.4 - 1e-6, "{hi}");
}

#[test]
fn req_op_24_req_op_12_facing_follows_the_part_where_it_rises_above_the_face() {
    // A face 6 mm from the axis is inside the part (radius 8): over the part
    // the tool stays at the part with its stock to leave.
    let f = fixture(&flat(4.0));
    let v = facing(&[(facing::SIZE, num(6.0))]);
    let p = path(&f, &Facing, &v).unwrap();
    let poses: Vec<Pose> = p
        .group
        .segments
        .iter()
        .filter(|s| s.kind.is_fed())
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect();
    let g = max_gouge(&Outline::new(&f.tool), &part(), &poses, 0.25);
    assert!(g.depth < 1e-3, "gouge {}", g.depth);
    // Beyond the part's ends it does go down to the face.
    let lowest = heights(&f, &p).into_iter().fold(f64::INFINITY, f64::min);
    assert!((lowest - 6.0).abs() < 1e-6, "{lowest}");
}

#[test]
fn req_op_24_restricted_extent_is_not_silently_ignored_without_whole_setup_context() {
    let t = tool(ToolShape::FlatEnd, 4.0);
    let f = fixture(&t);
    let v = facing(&[(facing::SIZE, num(10.0)), ("extent", text("part"))]);
    let err = path(&f, &Facing, &v).unwrap_err().to_string();
    assert!(err.contains("whole-setup geometry"), "{err}");
}

#[test]
fn req_op_24_facing_flat_extends_past_the_sides_when_asked() {
    let f = fixture(&flat(4.0));
    let across = |p: &GeneratedPath| {
        p.group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .map(|s| f.rotary.to_axis(s.end.tip, s.end.a).across.abs())
            .fold(0.0_f64, f64::max)
    };
    let base = facing(&[(facing::SIZE, num(10.0))]);
    let without = across(&path(&f, &Facing, &base).unwrap());
    let mut v = base.clone();
    v.insert(extend::EXTEND.to_owned(), text(extend::BOTH_ENDS));
    v.insert(extend::EXTEND_SIDES.to_owned(), ParameterValue::Flag(true));
    v.insert(extend::EXTEND_DISTANCE.to_owned(), num(0.5));
    let with = across(&path(&f, &Facing, &v).unwrap());
    // The face is the chord of the stock at 10 mm: half width sqrt(144 -
    // 100); past the side the tip goes a radius and the distance further.
    let half = (144.0_f64 - 100.0).sqrt();
    assert!(without <= half + 1e-6, "{without}");
    assert!(with >= half + 2.0 + 0.5 - 1e-6, "{with}");
}

#[test]
fn req_op_24_req_nfr_07_facing_refuses_a_size_with_nothing_to_face() {
    let f = fixture(&flat(4.0));
    for (shape, size) in [(facing::FLAT, 12.0), (facing::ROUND, 26.0)] {
        let v = facing(&[(facing::SHAPE, text(shape)), (facing::SIZE, num(size))]);
        let err = path(&f, &Facing, &v).unwrap_err().to_string();
        assert!(
            err.contains(facing::SIZE) && err.contains("nothing to face"),
            "{shape}: {err}"
        );
    }
    let v = facing(&[(facing::ANGLES, text(""))]);
    let mut v = v;
    v.insert(facing::SIZE.to_owned(), num(10.0));
    let err = path(&f, &Facing, &v).unwrap_err().to_string();
    assert!(err.contains(facing::ANGLES), "{err}");
}

#[test]
fn req_op_24_req_op_16_facing_is_registered_under_roughing() {
    let registry = openchuck_path::Registry::builtin();
    let strategy = registry.get(openchuck_path::facing::NAME).unwrap();
    assert_eq!(strategy.group(), openchuck_path::StrategyGroup::Roughing);
    assert!(!strategy.description().is_empty());
    let schema = strategy.schema();
    for name in [
        facing::SHAPE,
        facing::SIZE,
        facing::ANGLES,
        roughing::STEPDOWN,
        extend::EXTEND,
        extend::EXTEND_SIDES,
        extend::EXTEND_DISTANCE,
    ] {
        assert!(schema.get(name).is_some(), "{name}");
    }
}

/// The part with a tab beyond its far end: radius 8 from X 0 to 20, then a
/// tab of radius 4 from 20 to 23 joining it towards the stock's end.
struct WithTab;

impl RadialPart for WithTab {
    fn name(&self) -> &'static str {
        "cylinder with a tab"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, 23.0)
    }
    fn radius_at(&self, x: f64, _angle: f64) -> f64 {
        if x <= 20.0 { 8.0 } else { 4.0 }
    }
}

#[test]
fn req_op_24_req_op_21_an_extended_path_rises_over_a_tab_and_never_cuts_it() {
    use openchuck_path::{CompositeSurface, Solid};
    use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
    use std::sync::Arc;
    let t = flat(4.0);
    let f = fixture(&t);
    let tab = Solid::cylinder(
        Point3::new(20.0, 0.0, 0.0),
        UnitVec3::new(Vec3::new(1.0, 0.0, 0.0)).unwrap(),
        3.0,
        4.0,
    )
    .unwrap();
    let model: Arc<dyn openchuck_path::SurfaceSource> =
        Arc::new(MeshSurface::new(&part().mesh(40, 360)).unwrap());
    let composite = CompositeSurface::new(model, vec![tab]);
    let input = openchuck_path::StrategyInput {
        surface: &composite,
        ..f.input()
    };
    for (name, strategy) in strategies() {
        let v = only_known(
            strategy.as_ref(),
            &values(&[(extend::EXTEND, text(extend::FAR_END))]),
        );
        let p = run(|ctx| generate(strategy.as_ref(), &input, &v, ctx)).unwrap();
        assert!(cut_x(&p).1 > STOCK.1, "{name}");
        let poses: Vec<Pose> = p
            .group
            .segments
            .iter()
            .filter(|s| s.kind.is_fed())
            .map(|s| Pose {
                tip: s.end.tip,
                axis: s.end.axis.get(),
            })
            .collect();
        let g = max_gouge(&Outline::new(&f.tool), &WithTab, &poses, 0.25);
        assert!(g.depth < 0.02, "{name}: gouge {} at {:?}", g.depth, g.at);
    }
}

/// A stock model that does not know its ends.
#[derive(Debug)]
struct NoEnds(AnalyticStock);

impl openchuck_path::StockModel for NoEnds {
    fn contains(&self, p: openchuck_types::geometry::Point3) -> bool {
        self.0.contains(p)
    }
    fn ray_entry(
        &self,
        origin: openchuck_types::geometry::Point3,
        direction: openchuck_types::geometry::UnitVec3,
        max_distance: f64,
    ) -> Option<f64> {
        self.0.ray_entry(origin, direction, max_distance)
    }
    fn max_radius(&self) -> Option<f64> {
        self.0.max_radius()
    }
}

#[test]
fn req_op_24_req_nfr_07_a_stock_with_no_known_ends_is_refused_and_box_stock_has_them() {
    use openchuck_path::StockModel;
    use openchuck_types::geometry::{Aabb, Point3};
    let f = fixture(&flat(4.0));
    let stock = NoEnds(f.stock);
    let input = openchuck_path::StrategyInput {
        stock: &stock,
        ..f.input()
    };
    let mut strategies = strategies();
    strategies.push(("facing", Box::new(Facing)));
    for (name, strategy) in strategies {
        let v = only_known(
            strategy.as_ref(),
            &facing(&[
                (extend::EXTEND, text(extend::BOTH_ENDS)),
                (facing::SIZE, num(10.0)),
            ]),
        );
        let err = run(|ctx| generate(strategy.as_ref(), &input, &v, ctx))
            .unwrap_err()
            .to_string();
        assert!(err.contains("no ends"), "{name}: {err}");
    }
    let boxed = AnalyticStock::Box(Aabb {
        min: Point3::new(-3.0, -10.0, -10.0),
        max: Point3::new(24.0, 10.0, 10.0),
    });
    assert_eq!(boxed.x_extent(), Some((-3.0, 24.0)));
    assert_eq!(Facing.label(), "Facing");
}

// ------------------------------------------------- ring and spiral run-outs
//
// Decision record 0546 (issue #644; the owner's answers Q1 and Q2, relayed
// 8 Nov 2026): on helical finishing's ring and spiral passes the run-out
// past the stock continues the pass's own motion, the spiral at its pitch
// and rings at the ring spacing joined by the operation's own steps, out to
// the extended end, instead of a line along X.

use openchuck_path::helical::parameter as helical;
use openchuck_path::rotary::reversals;
use openchuck_testkit::engagement::UnrolledStock;
use openchuck_types::ir::{Segment, ToolPose};

/// The cutting moves of each run of cutting moves, with the pose each
/// starts from.
fn cut_runs(p: &GeneratedPath) -> Vec<Vec<(ToolPose, Segment)>> {
    let mut runs: Vec<Vec<(ToolPose, Segment)>> = Vec::new();
    let mut prev = p.group.start;
    let mut cutting = false;
    for s in &p.group.segments {
        let now = s.kind == MoveKind::Cutting;
        if now {
            if !cutting {
                runs.push(Vec::new());
            }
            if let Some(r) = runs.last_mut() {
                r.push((prev, *s));
            }
        }
        cutting = now;
        prev = s.end;
    }
    runs
}

/// Helical finishing on the cylinder, with a 4 mm flat end at a 2 mm
/// pitch, extended past both ends by 0.5 mm.
fn helical_extended(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v = values(&[
        (extend::EXTEND, text(extend::BOTH_ENDS)),
        (extend::EXTEND_DISTANCE, num(0.5)),
    ]);
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

/// The extended ends: the stock's ends, a tool radius (2) and the distance
/// (0.5) beyond.
const REACH: (f64, f64) = (STOCK.0 - 2.5, STOCK.1 + 2.5);

/// Whether a move lies past the part (X 0 to 20) by more than a tool
/// radius and a millimetre, where only the run-out cuts: the free ends'
/// passes (decision record 0344) turn A beside the end faces, a radius
/// past the part.
fn past_the_part(from: &ToolPose, s: &Segment) -> bool {
    let (a, b) = (from.tip.x, s.end.tip.x);
    a.max(b) < -3.0 || a.min(b) > 23.0
}

#[test]
fn req_op_24_a_spiral_run_out_continues_the_helix_at_its_pitch() {
    let f = fixture(&flat(4.0));
    for rotation in ["positive", "negative"] {
        let v = helical_extended(&[(helical::ROTATION, text(rotation))]);
        let p = path(&f, &HelicalFinishing, &v).unwrap();
        let sign = if rotation == "negative" { -1.0 } else { 1.0 };
        let mut moves = 0;
        for run in cut_runs(&p) {
            for (from, s) in &run {
                if !past_the_part(from, s) {
                    continue;
                }
                moves += 1;
                let dx = s.end.tip.x - from.tip.x;
                let da = s.end.a.0 - from.a.0;
                // The helix: one pitch (2 mm) of X a turn, the Rotation's
                // way, never a move along X with A held.
                assert!(da * sign > 0.0, "{rotation}: A held or reversed ({da})");
                assert!(
                    (dx / da * sign - 2.0 / 360.0).abs() < 1e-6,
                    "{rotation}: lead {} mm a turn",
                    dx / da * sign * 360.0
                );
            }
        }
        assert!(moves > 0, "{rotation}: no run-out");
        let (lo, hi) = cut_x(&p);
        assert!(
            (lo - REACH.0).abs() < 1e-6 && (hi - REACH.1).abs() < 1e-6,
            "{lo} {hi}"
        );
        assert!(
            p.notices.iter().any(|n| n.text.contains("past the part")),
            "{:?}",
            p.notices
        );
    }
}

#[test]
fn req_op_24_a_ring_run_out_adds_rings_out_to_the_extended_end() {
    let f = fixture(&flat(4.0));
    for step in [helical::SHARP, helical::BLENDED] {
        let v = helical_extended(&[
            (helical::PASS_SHAPE, text(helical::RINGS)),
            (helical::RING_STEP, text(step)),
        ]);
        let p = path(&f, &HelicalFinishing, &v).unwrap();
        let mut xs: Vec<f64> = Vec::new();
        for run in cut_runs(&p) {
            for (from, s) in &run {
                if !past_the_part(from, s) {
                    continue;
                }
                let dx = s.end.tip.x - from.tip.x;
                let da = s.end.a.0 - from.a.0;
                assert!(da >= -1e-9, "{step}: A reversed");
                if dx.abs() < 1e-9 {
                    xs.push(s.end.tip.x);
                } else if step == helical::BLENDED {
                    // No move along X with A held: a blend.
                    assert!(da > 1e-9, "{step}: a sharp step past the part");
                } else {
                    assert!(da.abs() < 1e-9, "{step}: a sharp step turns A");
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|b, a| (*b - *a).abs() < 1e-9);
        // Rings no further apart than the 2 mm spacing, out to each
        // extended end exactly.
        assert!(xs.len() >= 4, "{step}: {xs:?}");
        assert!((xs[0] - REACH.0).abs() < 1e-9, "{step}: {xs:?}");
        assert!((xs[xs.len() - 1] - REACH.1).abs() < 1e-9, "{step}: {xs:?}");
        for w in xs.windows(2) {
            assert!(
                w[1] - w[0] <= 2.0 + 1e-9 || (w[0] < 0.0 && w[1] > 20.0),
                "{step}: gap {w:?}"
            );
        }
    }
}

#[test]
fn req_op_24_ring_and_spiral_run_outs_leave_no_skin_past_the_part() {
    // The stock past the part's ends, beyond the end ring's reach, is
    // cleared all round to the part's radius (8), graded by the test kit's
    // independent dexel stock. A line along X at one A, as before, leaves
    // it standing at every other angle.
    let f = fixture(&flat(4.0));
    for shape in [helical::HELIX, helical::RINGS] {
        let v = helical_extended(&[(helical::PASS_SHAPE, text(shape))]);
        let p = path(&f, &HelicalFinishing, &v).unwrap();
        let mut stock = UnrolledStock::new(STOCK, 0.1, 8.0, 12.0);
        for run in cut_runs(&p) {
            let tips: Vec<_> = std::iter::once(run[0].0.tip)
                .chain(run.iter().map(|(_, s)| s.end.tip))
                .collect();
            stock.sweep(2.0, &tips, 0.05, 0.0);
        }
        for end in [(STOCK.0 + 0.2, -2.5), (22.5, STOCK.1 - 0.2)] {
            let mut x = end.0;
            while x <= end.1 {
                for d in 0..360 {
                    let h = stock.height_at(x, f64::from(d).to_radians()).unwrap();
                    assert!(h <= 8.0 + 1e-6, "{shape}: {h} left at X {x}, {d} degrees");
                }
                x += 0.25;
            }
        }
    }
}

#[test]
fn req_op_15_run_outs_turn_one_way() {
    let f = fixture(&flat(4.0));
    for shape in [helical::HELIX, helical::RINGS] {
        for rotation in ["positive", "negative"] {
            let v = helical_extended(&[
                (helical::PASS_SHAPE, text(shape)),
                (helical::ROTATION, text(rotation)),
                (helical::RING_STEP, text(helical::BLENDED)),
            ]);
            let p = path(&f, &HelicalFinishing, &v).unwrap();
            for run in cut_runs(&p) {
                let a: Vec<_> = std::iter::once(run[0].0.a)
                    .chain(run.iter().map(|(_, s)| s.end.a))
                    .collect();
                assert_eq!(reversals(&a, 1e-9), 0, "{shape} {rotation}");
            }
        }
    }
}

#[test]
fn req_op_24_axial_and_indexed_run_outs_stay_along_x() {
    // Their cut runs along X, so their run-out is a line along X at the
    // pass's A, as before (decision record 0546 item 5).
    let f = fixture(&flat(4.0));
    for strategy in [&AxialFinishing as &dyn Strategy, &IndexedFinishing] {
        let p = path(&f, strategy, &helical_extended(&[])).unwrap();
        let mut moves = 0;
        for run in cut_runs(&p) {
            for (from, s) in &run {
                if past_the_part(from, s) {
                    moves += 1;
                    assert!((s.end.a.0 - from.a.0).abs() < 1e-9, "{}", strategy.name());
                }
            }
        }
        assert!(moves > 0, "{}", strategy.name());
    }
}

#[test]
fn req_gc_07_req_nfr_06_run_outs_are_identical_at_1_2_and_n_threads() {
    let f = fixture(&flat(4.0));
    for shape in [helical::HELIX, helical::RINGS] {
        let v = helical_extended(&[
            (helical::PASS_SHAPE, text(shape)),
            (helical::RING_STEP, text(helical::BLENDED)),
        ]);
        let runs = openchuck_testkit::determinism::check(|| {
            format!("{:?}", path(&f, &HelicalFinishing, &v).unwrap().group).into_bytes()
        })
        .unwrap();
        assert_eq!(runs.len(), 4);
    }
}

#[test]
fn req_op_24_req_op_21_a_ring_run_out_rises_over_a_tab() {
    use openchuck_path::{CompositeSurface, Solid};
    use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
    use std::sync::Arc;
    let f = fixture(&flat(4.0));
    let tab = Solid::cylinder(
        Point3::new(20.0, 0.0, 0.0),
        UnitVec3::new(Vec3::new(1.0, 0.0, 0.0)).unwrap(),
        3.0,
        4.0,
    )
    .unwrap();
    let model: Arc<dyn openchuck_path::SurfaceSource> =
        Arc::new(MeshSurface::new(&part().mesh(40, 360)).unwrap());
    let composite = CompositeSurface::new(model, vec![tab]);
    let input = openchuck_path::StrategyInput {
        surface: &composite,
        ..f.input()
    };
    for step in [helical::SHARP, helical::BLENDED] {
        let v = only_known(
            &HelicalFinishing,
            &values(&[
                (extend::EXTEND, text(extend::FAR_END)),
                (helical::PASS_SHAPE, text(helical::RINGS)),
                (helical::RING_STEP, text(step)),
            ]),
        );
        let p = run(|ctx| generate(&HelicalFinishing, &input, &v, ctx)).unwrap();
        assert!(cut_x(&p).1 > STOCK.1, "{step}");
        let poses: Vec<Pose> = p
            .group
            .segments
            .iter()
            .filter(|s| s.kind.is_fed())
            .map(|s| Pose {
                tip: s.end.tip,
                axis: s.end.axis.get(),
            })
            .collect();
        let g = max_gouge(&Outline::new(&f.tool), &WithTab, &poses, 0.25);
        assert!(g.depth < 0.02, "{step}: gouge {} at {:?}", g.depth, g.at);
    }
}
