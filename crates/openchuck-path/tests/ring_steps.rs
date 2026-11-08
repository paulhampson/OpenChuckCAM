// SPDX-License-Identifier: AGPL-3.0-or-later
//! Steps between rings (issue #638; decision record 0545; OP-03, OP-04,
//! OP-05, OP-09, OP-11, OP-12, OP-15, OP-24, GC-07, NFR-06, NFR-07).
//!
//! With **Passes** set to **Rings** and **Steps between rings** set to
//! **Blended**, each ring stays a whole turn at one X, and the tool moves to
//! the next ring while A turns on by **Blend over** degrees (a helix piece)
//! instead of stepping along X with A held. With **Sharp**, or with no
//! setting, every strategy cuts exactly as before.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

mod support;
use support::generate;

use std::collections::BTreeMap;

use openchuck_path::facing::parameter as facing;
use openchuck_path::helical::parameter::{
    BLENDED, PASS_SHAPE, RING_BLEND_ANGLE, RING_STEP, RINGS, SHARP,
};
use openchuck_path::params::names;
use openchuck_path::rotary::reversals;
use openchuck_path::roughing::parameter as roughing;
use openchuck_path::{
    AnalyticStock, Facing, GeneratedPath, HelicalFinishing, MeshSurface, ParameterKind,
    ParameterValue, RestMachining, RotaryRoughing, SimultaneousFinishing, Strategy,
};
use openchuck_testkit::engagement::{UnrolledStock, peak_and_median, smoothed};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart, revolve};
use openchuck_types::ir::{MoveKind, Segment, ToolPose};
use openchuck_types::tool::{Tool, ToolShape};
use support::{Fixture, ball, run, tool};

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn values(
    base: &[(&str, f64)],
    extra: &[(&str, ParameterValue)],
) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = base
        .iter()
        .map(|(k, x)| ((*k).to_owned(), num(*x)))
        .collect();
    v.insert(PASS_SHAPE.to_owned(), text(RINGS));
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn stepped(
    mut v: BTreeMap<String, ParameterValue>,
    step: &str,
) -> BTreeMap<String, ParameterValue> {
    v.insert(RING_STEP.to_owned(), text(step));
    v
}

fn path(
    f: &Fixture,
    strategy: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
) -> openchuck_path::Result<GeneratedPath> {
    run(|ctx| generate(strategy, &f.input(), v, ctx))
}

/// The cutting segments with the pose each starts from, in runs: a run
/// ends where anything but a cutting move comes between.
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// A turns at constant X.
    Ring,
    /// X moves with A held.
    Step,
    /// X moves while A turns.
    Blend,
    /// Neither moves (a height change only).
    Other,
}

fn kind(from: &ToolPose, s: &Segment) -> Kind {
    let dx = (s.end.tip.x - from.tip.x).abs() > 1e-9;
    let da = (s.end.a.0 - from.a.0).abs() > 1e-9;
    match (dx, da) {
        (false, true) => Kind::Ring,
        (true, false) => Kind::Step,
        (true, true) => Kind::Blend,
        (false, false) => Kind::Other,
    }
}

/// The pieces of a run: consecutive moves of one kind, with the X and A
/// they cover.
#[derive(Debug, Clone, Copy)]
struct Piece {
    kind: Kind,
    x0: f64,
    x1: f64,
    turned: f64,
}

fn pieces(run: &[(ToolPose, Segment)]) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for (from, s) in run {
        let k = kind(from, s);
        if k == Kind::Other {
            continue;
        }
        let da = s.end.a.0 - from.a.0;
        match out.last_mut() {
            Some(p) if p.kind == k => {
                p.x1 = s.end.tip.x;
                p.turned += da;
            }
            _ => out.push(Piece {
                kind: k,
                x0: from.tip.x,
                x1: s.end.tip.x,
                turned: da,
            }),
        }
    }
    out
}

/// Every run is whole rings joined by blends over `phi` degrees the way
/// `sign` says, with no sharp step, A turning one way, and at least one
/// blend somewhere.
fn assert_blended(p: &GeneratedPath, phi: f64, sign: f64, what: &str) {
    let mut blends = 0;
    for run in cut_runs(p) {
        let a: Vec<_> = std::iter::once(run[0].0.a)
            .chain(run.iter().map(|(_, s)| s.end.a))
            .collect();
        assert_eq!(reversals(&a, 1e-9), 0, "{what}: A reverses in a cut");
        let ps = pieces(&run);
        // The first and last pieces of a cut may be cut short where the
        // material or the region ends; the pieces between are whole.
        let inner = |n: usize| n > 0 && n + 1 < ps.len();
        for (i, piece) in ps.iter().enumerate() {
            assert_ne!(piece.kind, Kind::Step, "{what}: a sharp step {piece:?}");
            if piece.kind == Kind::Blend {
                assert!(piece.x1 > piece.x0, "{what}: a blend goes back along X");
                assert!(piece.turned * sign > 0.0, "{what}: A turns the wrong way");
                if !inner(i) {
                    continue;
                }
                blends += 1;
                assert!(
                    (piece.turned * sign - phi).abs() < 1e-6,
                    "{what}: a blend turned {} (expected {phi})",
                    piece.turned
                );
                // A blend sits between two whole rings.
                for n in [i - 1, i + 1] {
                    let ring = ps[n];
                    assert_eq!(ring.kind, Kind::Ring, "{what}: {ring:?} beside a blend");
                    if inner(n) {
                        assert!(
                            (ring.turned.abs() - 360.0).abs() < 1e-6,
                            "{what}: a ring turned {}",
                            ring.turned
                        );
                    }
                }
            } else {
                assert!(piece.turned * sign > 0.0, "{what}: A turns the wrong way");
            }
        }
    }
    assert!(blends > 0, "{what}: no blend");
}

/// The cutting moves of each blend are linear: X in proportion to A.
fn assert_linear_blends(p: &GeneratedPath, what: &str) {
    for run in cut_runs(p) {
        let mut lead: Option<f64> = None;
        for (from, s) in &run {
            if kind(from, s) != Kind::Blend {
                lead = None;
                continue;
            }
            let l = (s.end.tip.x - from.tip.x) / (s.end.a.0 - from.a.0);
            if let Some(before) = lead {
                assert!(
                    (l - before).abs() < 1e-6 * before.abs().max(1.0),
                    "{what}: lead {l} {before}"
                );
            }
            lead = Some(l);
        }
    }
}

fn identical_at_any_thread_count(
    f: &Fixture,
    strategy: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
) {
    let runs = openchuck_testkit::determinism::check(|| {
        format!("{:?}", path(f, strategy, v).unwrap().group).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

fn flat(d: f64) -> Tool {
    tool(ToolShape::FlatEnd, d)
}

fn mesh_fixture<P: RadialPart + ?Sized>(part: &P, stock_radius: f64, t: &Tool) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(40, 360)).unwrap();
    Fixture::new(surface, stock_radius, t)
}

// ------------------------------------------------------------ fixtures

fn rough(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    values(
        &[
            (names::STEPOVER, 1.27),
            (names::FEED, 600.0),
            (names::PLUNGE_FEED, 200.0),
            (roughing::STEPDOWN, 1.0),
        ],
        extra,
    )
}

/// One roughing layer as the #638 model has it: a 3.175 mm flat end mill at
/// a 1.27 mm (40 %) stepover, a 10 mm cut radius.
fn rough_fixture() -> Fixture {
    let part = Cylinder {
        radius: 10.0,
        length: 10.16,
    };
    mesh_fixture(&part, 11.0, &flat(3.175))
}

fn helical_values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    values(
        &[
            (names::STEPOVER, 2.0),
            (names::FEED, 300.0),
            (names::PLUNGE_FEED, 100.0),
        ],
        extra,
    )
}

fn cone() -> Cone {
    Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 20.0,
    }
}

fn finish_fixture<P: RadialPart + ?Sized>(part: &P) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(40, 360)).unwrap();
    let b = openchuck_path::SurfaceSource::bounds(&surface);
    let r = [b.max.y, b.max.z, b.min.y, b.min.z]
        .into_iter()
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    Fixture::new(surface, r + 1.0, &ball(6.0))
}

/// A shaft of radius 6 to X 12, a flank up to radius 12 at X 13.6, then
/// radius 12 to X 24: stock of radius 12 leaves material only on the small
/// shaft and the flank.
const SHAFT: [(f64, f64); 4] = [(0.0, 6.0), (12.0, 6.0), (13.6, 12.0), (24.0, 12.0)];

fn shaft() -> Fixture {
    let surface = MeshSurface::new(&revolve(&SHAFT, 180)).unwrap();
    let mut f = Fixture::new(surface, 12.0, &ball(3.0));
    f.stock = AnalyticStock::Cylinder {
        radius: 12.0,
        x_start: 0.0,
        x_end: 24.0,
    };
    f
}

fn rest_values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    values(
        &[
            (names::STEPOVER, 1.0),
            (names::FEED, 400.0),
            (names::PLUNGE_FEED, 200.0),
        ],
        extra,
    )
}

fn facing_fixture() -> Fixture {
    let part = Cylinder {
        radius: 8.0,
        length: 20.0,
    };
    let surface = MeshSurface::new(&part.mesh(40, 360)).unwrap();
    let mut f = Fixture::new(surface, 12.0, &flat(4.0));
    f.stock = AnalyticStock::Cylinder {
        radius: 12.0,
        x_start: -6.0,
        x_end: 26.0,
    };
    f
}

fn round_values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v = values(
        &[
            (names::STEPOVER, 2.0),
            (names::FEED, 600.0),
            (names::PLUNGE_FEED, 200.0),
            (roughing::STEPDOWN, 2.0),
            (facing::SIZE, 20.0),
        ],
        extra,
    );
    v.insert(facing::SHAPE.to_owned(), text(facing::ROUND));
    v
}

fn simultaneous_values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    values(
        &[
            (names::STEPOVER, 2.0),
            (names::FEED, 300.0),
            (names::PLUNGE_FEED, 100.0),
        ],
        extra,
    )
}

/// Each ring strategy with a fixture and its values, for the checks every
/// strategy shares.
fn every_strategy() -> Vec<(
    &'static str,
    Box<dyn Strategy>,
    Fixture,
    BTreeMap<String, ParameterValue>,
)> {
    vec![
        (
            "roughing",
            Box::new(RotaryRoughing),
            rough_fixture(),
            rough(&[]),
        ),
        (
            "helical",
            Box::new(HelicalFinishing),
            finish_fixture(&cone()),
            helical_values(&[]),
        ),
        ("rest", Box::new(RestMachining), shaft(), rest_values(&[])),
        (
            "round facing",
            Box::new(Facing),
            facing_fixture(),
            round_values(&[]),
        ),
        (
            "simultaneous",
            Box::new(SimultaneousFinishing),
            finish_fixture(&cone()),
            simultaneous_values(&[]),
        ),
    ]
}

// --------------------------------------------------------------- tests

#[test]
fn req_op_11_every_ring_strategy_offers_steps_between_rings_only_with_rings() {
    for (what, strategy, _, v) in every_strategy() {
        let schema = strategy.schema();
        let step = schema
            .get(RING_STEP)
            .unwrap_or_else(|| panic!("{what}: no Steps between rings"));
        assert_eq!(step.label, "Steps between rings");
        match &step.kind {
            ParameterKind::Choice { options, default } => {
                assert_eq!(options, &[SHARP.to_owned(), BLENDED.to_owned()]);
                // A saved operation without the setting keeps sharp steps.
                assert_eq!(default, SHARP, "{what}");
            }
            other => panic!("{what}: {other:?}"),
        }
        let blend = schema
            .get(RING_BLEND_ANGLE)
            .unwrap_or_else(|| panic!("{what}: no Blend over"));
        assert_eq!(blend.label, "Blend over");
        match &blend.kind {
            ParameterKind::Number {
                min,
                min_exclusive,
                max,
                default,
                ..
            } => {
                assert_eq!(
                    (*min, *min_exclusive, *max, *default),
                    (Some(0.0), true, Some(180.0), Some(45.0))
                );
            }
            other => panic!("{what}: {other:?}"),
        }
        // Shown with Rings only, and Blend over only when blended.
        let shown = |v: &BTreeMap<String, ParameterValue>, name: &str| {
            strategy.schema_for(v).get(name).is_some()
        };
        assert!(shown(&v, RING_STEP), "{what}");
        assert!(
            !shown(&v, RING_BLEND_ANGLE),
            "{what}: Blend over with sharp steps"
        );
        assert!(
            shown(&stepped(v.clone(), BLENDED), RING_BLEND_ANGLE),
            "{what}"
        );
        let mut spiral = stepped(v.clone(), BLENDED);
        spiral.remove(PASS_SHAPE);
        assert!(!shown(&spiral, RING_STEP), "{what}: shown with the spiral");
        assert!(
            !shown(&spiral, RING_BLEND_ANGLE),
            "{what}: shown with the spiral"
        );
    }
    // Not for the forms and shapes that cut no spiral.
    let mut v = stepped(rough(&[]), BLENDED);
    v.insert(roughing::FORM.to_owned(), text("indexed"));
    assert!(RotaryRoughing.schema_for(&v).get(RING_STEP).is_none());
    let mut v = stepped(round_values(&[]), BLENDED);
    v.insert(facing::SHAPE.to_owned(), text(facing::FLAT));
    assert!(Facing.schema_for(&v).get(RING_STEP).is_none());
}

#[test]
fn req_gc_07_sharp_steps_are_unchanged() {
    for (what, strategy, f, v) in every_strategy() {
        let before = format!("{:?}", path(&f, strategy.as_ref(), &v).unwrap().group);
        let sharp = format!(
            "{:?}",
            path(&f, strategy.as_ref(), &stepped(v.clone(), SHARP))
                .unwrap()
                .group
        );
        assert_eq!(before, sharp, "{what}: Sharp is not today's path");
        // The blend angle does nothing with sharp steps.
        let mut angled = stepped(v.clone(), SHARP);
        angled.insert(RING_BLEND_ANGLE.to_owned(), num(90.0));
        let angled = format!("{:?}", path(&f, strategy.as_ref(), &angled).unwrap().group);
        assert_eq!(before, angled, "{what}");
    }
}

#[test]
fn req_op_04_req_op_11_blended_rings_are_whole_turns_joined_by_helix_pieces() {
    for (what, strategy, f, v) in every_strategy() {
        if what == "simultaneous" {
            continue; // its drive smooths A: its own test below
        }
        for phi in [45.0, 90.0] {
            let mut b = stepped(v.clone(), BLENDED);
            b.insert(RING_BLEND_ANGLE.to_owned(), num(phi));
            let p = path(&f, strategy.as_ref(), &b).unwrap();
            assert_blended(&p, phi, 1.0, what);
            assert_linear_blends(&p, what);
            assert!(
                p.notices
                    .iter()
                    .any(|n| n.text.contains(&format!("blended over {phi}"))),
                "{what}: {:?}",
                p.notices
            );
        }
    }
}

#[test]
fn req_op_15_blended_rings_turn_one_way() {
    // Negative rotation in helical finishing: the blends turn the other way
    // and A never reverses.
    let f = finish_fixture(&cone());
    let v = stepped(
        helical_values(&[(
            openchuck_path::helical::parameter::ROTATION,
            text("negative"),
        )]),
        BLENDED,
    );
    let p = path(&f, &HelicalFinishing, &v).unwrap();
    assert_blended(&p, 45.0, -1.0, "helical, negative");
    // Roughing at a start angle: the first ring starts there, each blend
    // 45 degrees on.
    let f = rough_fixture();
    let v = stepped(rough(&[(roughing::START_ANGLE, num(30.0))]), BLENDED);
    let p = path(&f, &RotaryRoughing, &v).unwrap();
    assert_blended(&p, 45.0, 1.0, "roughing at 30");
    // Ring k starts at 30 + k (360 + 45) degrees: each whole ring after a
    // blend starts at 30 degrees plus a whole number of 45s.
    let mut checked = 0;
    for run in cut_runs(&p) {
        let mut a = run[0].0.a.0;
        let mut previous = Kind::Other;
        for (from, s) in &run {
            let k = kind(from, s);
            if k == Kind::Ring && previous == Kind::Blend {
                let off = (a - 30.0) / 45.0;
                assert!((off - off.round()).abs() < 1e-9, "a ring starts at {a}");
                checked += 1;
            }
            if k != Kind::Other {
                previous = k;
            }
            a = s.end.a.0;
        }
    }
    assert!(checked > 0);
}

/// The width of cut along the blends (or steps) of a roughing path, graded
/// by the test kit's independent check: the peak and median, mm.
fn blend_widths(p: &GeneratedPath) -> (f64, f64) {
    // The fixture's stock: radius 11 from 1 mm before the part to 1 mm
    // after it.
    let mut stock = UnrolledStock::new((-1.0, 11.16), 0.02, 10.0, 11.0);
    let mut on_steps = Vec::new();
    for run in cut_runs(p) {
        let tips: Vec<_> = std::iter::once(run[0].0.tip)
            .chain(run.iter().map(|(_, s)| s.end.tip))
            .collect();
        // Material counts as cut where more than 0.01 mm of it is taken:
        // a shave of a micrometre is not engagement.
        let raw = stock.sweep(3.175 / 2.0, &tips, 0.02, 0.01);
        // The first cutting point of a run is where the tool arrives in the
        // stock after its entry, which is not a cutting move: the whole
        // footprint is removed there. Left out, as the model leaves out the
        // first turn.
        for s in smoothed(&raw, 25)
            .into_iter()
            .filter(|s| s.along > 3.175 + 0.5)
        {
            let (from, seg) = &run[s.segment];
            // Away from the stock's ends (-1 and 11.16), where the tool
            // stands partly past the stock and the first ring is a slot
            // cut in pieces: the model has no ends.
            let inside = from.tip.x > 0.5 && seg.end.tip.x < 9.66;
            if inside && matches!(kind(from, seg), Kind::Blend | Kind::Step) {
                on_steps.push(s.width);
            }
        }
    }
    peak_and_median(on_steps).unwrap()
}

#[test]
fn req_op_03_blended_ring_steps_cut_no_wider_than_near_the_stepover() {
    let f = rough_fixture();
    let sharp = path(&f, &RotaryRoughing, &stepped(rough(&[]), SHARP)).unwrap();
    let blended = path(&f, &RotaryRoughing, &stepped(rough(&[]), BLENDED)).unwrap();
    let (sharp_peak, sharp_median) = blend_widths(&sharp);
    let (peak, median) = blend_widths(&blended);
    // The model (decision record 0545): a slot on the sharp step, near the
    // stepover on the blend.
    assert!(
        sharp_peak > 2.6 && sharp_median > 2.0,
        "sharp {sharp_peak} {sharp_median}"
    );
    assert!(peak <= 1.25 * 1.27, "blended peak {peak} (median {median})");
}

#[test]
fn req_op_12_blended_rings_leave_no_more_than_sharp() {
    let part = cone();
    let f = finish_fixture(&part);
    let outline = Outline::new(&f.tool);
    for (what, strategy) in [
        ("helical", &HelicalFinishing as &dyn Strategy),
        ("simultaneous", &SimultaneousFinishing),
    ] {
        let v = helical_values(&[]);
        let fed = |p: &GeneratedPath| -> Vec<Pose> {
            p.group
                .segments
                .iter()
                .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge))
                .map(|s| Pose {
                    tip: s.end.tip,
                    axis: s.end.axis.get(),
                })
                .collect()
        };
        let sharp = fed(&path(&f, strategy, &stepped(v.clone(), SHARP)).unwrap());
        let blended = fed(&path(&f, strategy, &stepped(v.clone(), BLENDED)).unwrap());
        let g = max_gouge(&outline, &part, &blended, 0.15);
        assert!(g.depth <= 0.002, "{what}: gouge {}", g.depth);
        // Between the end rings, where both paths are whole rings 2 mm apart.
        let left = |poses: &[Pose]| coverage(&outline, &part, poses, (4.0, 16.0), 0.2, 0.02);
        let (s, b) = (left(&sharp), left(&blended));
        // The scallop of a 2 mm pitch with a 6 mm ball is about 0.17 mm, so
        // both leave samples over 0.02 mm: blended leaves no more of them,
        // and its worst is no worse.
        assert!(
            b.uncovered <= s.uncovered,
            "{what}: blended {b:?}, sharp {s:?}"
        );
        assert!(
            b.worst <= s.worst + 0.005,
            "{what}: blended {b:?}, sharp {s:?}"
        );
    }
}

#[test]
fn req_op_05_blended_simultaneous_rings_follow_the_spiral_drive() {
    let f = finish_fixture(&cone());
    for lead in [0.0, 5.0, -5.0] {
        let v = stepped(
            simultaneous_values(&[(
                openchuck_path::simultaneous::parameter::LEAD_ANGLE,
                num(lead),
            )]),
            BLENDED,
        );
        let p = path(&f, &SimultaneousFinishing, &v).unwrap();
        // No sharp step; the blends move along X while A turns, A never
        // reverses, and the path is one cut where nothing splits it.
        let runs = cut_runs(&p);
        let mut blends = 0;
        for run in &runs {
            let a: Vec<_> = std::iter::once(run[0].0.a)
                .chain(run.iter().map(|(_, s)| s.end.a))
                .collect();
            assert_eq!(reversals(&a, 1e-9), 0, "lead {lead}");
            for piece in pieces(run) {
                assert_ne!(piece.kind, Kind::Step, "lead {lead}: a sharp step");
                if piece.kind == Kind::Blend && piece.x1 - piece.x0 > 0.5 {
                    blends += 1;
                }
            }
        }
        assert!(blends > 0, "lead {lead}: no blend");
        assert!(
            p.notices.iter().any(|n| n.text.contains("blended over 45")),
            "{:?}",
            p.notices
        );
    }
}

#[test]
fn req_op_09_blended_rest_rings_cut_only_where_material_is_left() {
    let f = shaft();
    let p = path(&f, &RestMachining, &stepped(rest_values(&[]), BLENDED)).unwrap();
    // The stock stands at radius 12, so nothing past the flank is cut: the
    // tool's tip stays where the part is below the stock.
    for run in cut_runs(&p) {
        for (_, s) in run {
            assert!(s.end.tip.x <= 13.6 + 1.5 + 1e-6, "cut at X {}", s.end.tip.x);
        }
    }
    assert_blended(&p, 45.0, 1.0, "rest");
}

#[test]
fn req_op_24_blended_round_facing_reaches_the_diameter_and_the_ends() {
    let f = facing_fixture();
    let sharp = path(&f, &Facing, &stepped(round_values(&[]), SHARP)).unwrap();
    let blended = path(&f, &Facing, &stepped(round_values(&[]), BLENDED)).unwrap();
    let extent = |p: &GeneratedPath| {
        let xs: Vec<f64> = cut_runs(p)
            .iter()
            .flatten()
            .map(|(_, s)| s.end.tip.x)
            .collect();
        let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let r = cut_runs(p)
            .iter()
            .flatten()
            .map(|(_, s)| f.rotary.to_axis(s.end.tip, s.end.a).height)
            .fold(f64::INFINITY, f64::min);
        (lo, hi, r)
    };
    let (s, b) = (extent(&sharp), extent(&blended));
    assert!(
        (s.0 - b.0).abs() < 1e-9 && (s.1 - b.1).abs() < 1e-9,
        "{s:?} {b:?}"
    );
    assert!((s.2 - b.2).abs() < 1e-9, "{s:?} {b:?}");
}

#[test]
fn req_gc_07_req_nfr_06_blended_rings_identical_at_1_2_and_n_threads() {
    for (_, strategy, f, v) in every_strategy() {
        identical_at_any_thread_count(&f, strategy.as_ref(), &stepped(v, BLENDED));
    }
}

#[test]
fn req_nfr_07_a_bad_blend_angle_is_refused_by_name() {
    for (what, strategy, f, v) in every_strategy() {
        for bad in [0.0, -1.0, 180.0001, f64::NAN, f64::INFINITY] {
            let mut b = stepped(v.clone(), BLENDED);
            b.insert(RING_BLEND_ANGLE.to_owned(), num(bad));
            let e = path(&f, strategy.as_ref(), &b).unwrap_err().to_string();
            assert!(e.contains(RING_BLEND_ANGLE), "{what}, {bad}: {e}");
        }
        let mut b = v.clone();
        b.insert(RING_STEP.to_owned(), text("rounded"));
        let e = path(&f, strategy.as_ref(), &b).unwrap_err().to_string();
        assert!(e.contains(RING_STEP), "{what}: {e}");
    }
}

#[test]
fn req_nfr_07_blended_roughing_counts_its_blends_against_the_node_limit() {
    // A stepover too fine to cut is refused by name with the blends
    // counted, as with sharp steps.
    let f = rough_fixture();
    for stepover in [1e-7, f64::MIN_POSITIVE] {
        let mut v = stepped(rough(&[]), BLENDED);
        v.insert(names::STEPOVER.to_owned(), num(stepover));
        let e = path(&f, &RotaryRoughing, &v).unwrap_err().to_string();
        assert!(e.contains(names::STEPOVER), "{stepover}: {e}");
    }
}
