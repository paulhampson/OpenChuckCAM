// SPDX-License-Identifier: AGPL-3.0-or-later
//! A lead on a pass that turns A stays in the plane of the cut (OP-11,
//! OP-13, OP-15, SET-09, NFR-07; decision record 0544, proposed; issue
//! #640). Synthetic fixtures: the cylinder and cone of the issue's probe,
//! not the owner's projects.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::roughing::parameter as roughing;
use openchuck_path::{
    CutPoint, GeneratedPath, HelicalFinishing, MeshSurface, ParameterSchema, ParameterValue,
    Parameters, Pass, Result, RotaryRoughing, Strategy, StrategyInput,
};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::{MoveKind, Segment, ToolPose};
use openchuck_types::job::JobContext;
use openchuck_types::machine::ARange;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, Rings, ball, blisk, generate, ring_values, run, tool};

const FEED: f64 = 300.0;

fn n(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn t(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn finishing(shape: &str) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    v.insert(names::STEPOVER.into(), n(2.0));
    v.insert(names::FEED.into(), n(FEED));
    v.insert(names::PLUNGE_FEED.into(), n(100.0));
    v.insert(names::ENTRY.into(), t("lead"));
    v.insert("pass_shape".into(), t(shape));
    v
}

fn roughing_values(shape: &str) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    v.insert(names::STEPOVER.into(), n(3.0));
    v.insert(names::FEED.into(), n(600.0));
    v.insert(names::PLUNGE_FEED.into(), n(200.0));
    v.insert(names::STOCK_TO_LEAVE.into(), n(0.5));
    v.insert(roughing::STEPDOWN.into(), n(2.0));
    v.insert(names::ENTRY.into(), t("lead"));
    v.insert("pass_shape".into(), t(shape));
    v
}

fn cylinder() -> Fixture {
    let cyl = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    Fixture::new(
        MeshSurface::new(&cyl.mesh(3, 72)).unwrap(),
        16.0,
        &ball(6.0),
    )
}

fn cone() -> Fixture {
    let cone = Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 30.0,
    };
    Fixture::new(
        MeshSurface::new(&cone.mesh(40, 180)).unwrap(),
        16.0,
        &tool(ToolShape::FlatEnd, 6.0),
    )
}

fn make(
    f: &Fixture,
    s: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
    keep: &[(f64, f64)],
) -> std::result::Result<GeneratedPath, String> {
    let mut input = f.input();
    input.keep_out = keep;
    run(|ctx| generate(s, &input, v, ctx)).map_err(|e| e.to_string())
}

/// A lead: the pose before it, its moves (the plunge to its start and its
/// `LeadIn` moves, or its `LeadOut` moves) and the cutting moves beside it
/// (after a lead-in, before a lead-out, nearest first).
struct Lead {
    before: ToolPose,
    moves: Vec<Segment>,
    cut: Vec<ToolPose>,
}

fn leads(path: &GeneratedPath, kinds: &[MoveKind]) -> Vec<Lead> {
    let segs = &path.group.segments;
    let mut out = Vec::new();
    let mut prev = path.group.start;
    let mut i = 0;
    while i < segs.len() {
        if kinds.contains(&segs[i].kind) {
            let before = prev;
            let mut moves = Vec::new();
            let first = i;
            while i < segs.len() && kinds.contains(&segs[i].kind) {
                moves.push(segs[i]);
                prev = segs[i].end;
                i += 1;
            }
            let cut = if kinds.contains(&MoveKind::LeadOut) {
                let mut c: Vec<ToolPose> = segs[..first]
                    .iter()
                    .rev()
                    .take_while(|s| s.kind == MoveKind::Cutting)
                    .take(10)
                    .map(|s| s.end)
                    .collect();
                c.insert(0, before);
                c
            } else {
                segs[i..]
                    .iter()
                    .take_while(|s| s.kind == MoveKind::Cutting)
                    .take(10)
                    .map(|s| s.end)
                    .collect()
            };
            out.push(Lead { before, moves, cut });
            continue;
        }
        prev = segs[i].end;
        i += 1;
    }
    out
}

fn lead_ins(path: &GeneratedPath) -> Vec<Lead> {
    leads(path, &[MoveKind::Plunge, MoveKind::LeadIn])
}

fn lead_outs(path: &GeneratedPath) -> Vec<Lead> {
    leads(path, &[MoveKind::LeadOut])
}

/// The direction of the move from `p` to `q` in the unrolled frame at the
/// radius `rho`: (distance round, height), unit length.
fn unrolled(f: &Fixture, p: ToolPose, q: ToolPose, rho: f64) -> (f64, f64) {
    let (a, b) = (f.rotary.to_axis(p.tip, p.a), f.rotary.to_axis(q.tip, q.a));
    let s = (q.a.0 - p.a.0).to_radians() * rho;
    let h = b.height - a.height;
    let l = s.hypot(h);
    (s / l, h / l)
}

/// Checks a lead-in on a pass that turns A: every move holds the first cut
/// point's machine X and Y; A turns one way only, the pass's; the arc ends
/// at the first cut point, tangent to the cut there; it starts above the
/// cut. Returns the way A turns.
fn assert_turning_lead_in(f: &Fixture, lead: &Lead) -> f64 {
    let head = lead.moves.last().unwrap().end;
    let goal = f.rotary.to_axis(head.tip, head.a);
    let sign = (lead.cut[0].a.0 - head.a.0).signum();
    assert!(sign != 0.0, "the pass turns A");
    let lead_in: Vec<&Segment> = lead
        .moves
        .iter()
        .filter(|s| s.kind == MoveKind::LeadIn)
        .collect();
    assert!(lead_in.len() > 1, "an arc of several moves");
    let mut p = lead.moves[0].end;
    for s in &lead.moves {
        let a = f.rotary.to_axis(s.end.tip, s.end.a);
        assert!(
            (a.along - goal.along).abs() < 1e-6,
            "X held: {} vs {}",
            a.along,
            goal.along
        );
        assert!((a.across - goal.across).abs() < 1e-6, "Y held");
        let da = s.end.a.0 - p.a.0;
        assert!(da * sign >= -1e-9, "A turns the pass's way only: {da}");
        p = s.end;
    }
    // The arc starts one lead radius (the tool radius, 3 mm) above.
    let start = f.rotary.to_axis(lead.moves[0].end.tip, lead.moves[0].end.a);
    assert!(start.height > goal.height + 2.0, "{}", start.height);
    // Tangent where it meets the cut: the last arc move and the first cut
    // move point the same way in the unrolled frame, to within the angle of
    // one arc segment.
    let rho = goal.across.hypot(goal.height);
    let before = lead.moves[lead.moves.len() - 2].end;
    let into = unrolled(f, before, head, rho);
    let cut = lead
        .cut
        .iter()
        .find(|q| (q.a.0 - head.a.0).abs() > 1e-6)
        .unwrap();
    let along = unrolled(f, head, *cut, rho);
    let angle = (into.0 * along.0 + into.1 * along.1)
        .clamp(-1.0, 1.0)
        .acos();
    assert!(angle < 0.2, "tangent to the cut: {angle} rad");
    sign
}

/// As [`assert_turning_lead_in`], for a lead-out: it leaves the last cut
/// point tangent to the cut, holding X and Y, turning A the pass's way and
/// rising.
fn assert_turning_lead_out(f: &Fixture, lead: &Lead) {
    let last = lead.before;
    let goal = f.rotary.to_axis(last.tip, last.a);
    let previous = lead
        .cut
        .iter()
        .skip(1)
        .find(|q| (q.a.0 - last.a.0).abs() > 1e-6)
        .unwrap();
    let sign = (last.a.0 - previous.a.0).signum();
    let mut p = last;
    for s in &lead.moves {
        let a = f.rotary.to_axis(s.end.tip, s.end.a);
        assert!((a.along - goal.along).abs() < 1e-6, "X held");
        assert!((a.across - goal.across).abs() < 1e-6, "Y held");
        let da = s.end.a.0 - p.a.0;
        assert!(da * sign >= -1e-9, "A turns the pass's way only: {da}");
        p = s.end;
    }
    let end = f.rotary.to_axis(p.tip, p.a);
    assert!(end.height > goal.height + 2.0, "it rises: {}", end.height);
    let rho = goal.across.hypot(goal.height);
    let out = unrolled(f, last, lead.moves[0].end, rho);
    let along = unrolled(f, *previous, last, rho);
    let angle = (out.0 * along.0 + out.1 * along.1).clamp(-1.0, 1.0).acos();
    assert!(angle < 0.2, "tangent to the cut: {angle} rad");
}

#[test]
fn req_op_11_req_op_15_a_lead_on_rings_and_the_spiral_stays_at_the_pass_x_and_meets_the_cut() {
    let f = cylinder();
    for shape in ["rings", "helix"] {
        let path = make(&f, &HelicalFinishing, &finishing(shape), &[]).unwrap();
        let ins = lead_ins(&path);
        let outs = lead_outs(&path);
        assert!(!ins.is_empty() && !outs.is_empty(), "{shape}");
        for lead in &ins {
            assert_turning_lead_in(&f, lead);
        }
        for lead in &outs {
            assert_turning_lead_out(&f, lead);
        }
    }
}

#[test]
fn req_op_11_a_pass_that_only_turns_a_gets_a_lead_not_a_silent_plunge() {
    // The rings stub on a round part: every pass is a whole turn at one X
    // and one height, so it has no sideways move at all.
    let cyl = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let f = Fixture::new(
        MeshSurface::new(&cyl.mesh(3, 360)).unwrap(),
        16.0,
        &ball(6.0),
    );
    let mut v = ring_values(10.0, 14.0, 2.0, 0.0);
    v.insert(names::ENTRY.into(), t("lead"));
    let path = make(&f, &Rings, &v, &[]).unwrap();
    let ins = lead_ins(&path);
    let outs = lead_outs(&path);
    assert_eq!(ins.len(), 3);
    assert_eq!(outs.len(), 3);
    for lead in &ins {
        assert_turning_lead_in(&f, lead);
    }
    for lead in &outs {
        assert_turning_lead_out(&f, lead);
    }
    // No note says a pass was plunged.
    assert!(
        path.notices
            .iter()
            .all(|n| !n.text.contains("single point")),
        "{:?}",
        path.notices
    );
}

#[test]
fn req_op_11_req_op_13_a_turning_lead_is_fed_at_the_cutting_feed_with_a_contact_only_where_it_meets_the_surface()
 {
    let f = cylinder();
    let path = make(&f, &HelicalFinishing, &finishing("rings"), &[]).unwrap();
    let rotary = f.machine.max_feeds.rotary.0;
    for lead in lead_ins(&path).iter().chain(&lead_outs(&path)) {
        let mut p = lead.before;
        let fed: Vec<&Segment> = lead
            .moves
            .iter()
            .filter(|s| matches!(s.kind, MoveKind::LeadIn | MoveKind::LeadOut))
            .collect();
        let last_in = fed
            .iter()
            .rposition(|s| s.kind == MoveKind::LeadIn)
            .map(|i| i + (lead.moves.len() - fed.len()));
        for (k, s) in lead.moves.iter().enumerate() {
            if matches!(s.kind, MoveKind::LeadIn | MoveKind::LeadOut) {
                assert_eq!(s.contact_feed.unwrap().0, FEED, "the cutting feed");
                let tip = p.tip.distance(s.end.tip);
                if Some(k) != last_in {
                    // Above the surface: no contact point, so the length is
                    // the tip's own.
                    assert!(
                        (s.contact_length.0 - tip).abs() < 1e-9,
                        "{} vs {tip}",
                        s.contact_length.0
                    );
                }
                // Timed for the feed and for A's speed limit (OP-13).
                let at_feed = 60.0 * s.contact_length.0 / FEED;
                let a_time = 60.0 * (s.end.a.0 - p.a.0).abs() / rotary;
                assert!(s.duration.0 >= at_feed - 1e-9, "{}", s.duration.0);
                assert!(s.duration.0 >= a_time - 1e-9, "{}", s.duration.0);
            }
            p = s.end;
        }
    }
}

#[test]
fn req_op_11_req_set_09_a_turning_lead_never_reaches_a_held_region_at_the_starting_end() {
    // The arcs on main reach 3 mm beyond the part's end along X; here the
    // held region starts 0.5 mm beyond it.
    let f = cylinder();
    for shape in ["rings", "helix"] {
        let path = make(&f, &HelicalFinishing, &finishing(shape), &[(-25.0, -0.5)])
            .unwrap_or_else(|e| panic!("{shape}: {e}"));
        for lead in lead_ins(&path).iter().chain(&lead_outs(&path)) {
            for s in &lead.moves {
                let x = f.rotary.to_axis(s.end.tip, s.end.a).along;
                assert!(x > -0.5, "{shape}: a lead at X {x}");
            }
        }
    }
}

#[test]
fn req_op_11_req_op_14_continuous_roughing_of_a_rising_part_with_a_lead_is_no_longer_refused() {
    // On main: "pass 3: the lead-out needs an arc of radius 3.000 mm, and
    // the tool would cut 0.317 mm into the finished surface".
    let f = cone();
    for shape in ["rings", "helix"] {
        let path = make(&f, &RotaryRoughing, &roughing_values(shape), &[])
            .unwrap_or_else(|e| panic!("{shape}: {e}"));
        let ins = lead_ins(&path);
        assert!(ins.len() >= 3, "{shape}: {} lead-ins", ins.len());
        for lead in ins.iter().take(3) {
            assert_turning_lead_in(&f, lead);
        }
        for lead in lead_outs(&path).iter().take(3) {
            assert_turning_lead_out(&f, lead);
        }
    }
}

#[test]
fn req_op_11_req_nfr_07_a_turning_lead_outside_the_a_range_is_refused_naming_it() {
    let mut f = cylinder();
    f.rotary.range = ARange::Limited {
        min: Degrees(0.0),
        max: Degrees(360.0),
    };
    let mut v = ring_values(10.0, 10.0, 2.0, 0.0);
    v.insert(names::ENTRY.into(), t("lead"));
    let err = make(&f, &Rings, &v, &[]).unwrap_err();
    assert!(err.contains("Lead entry"), "{err}");
    assert!(err.contains("A range"), "{err}");
    assert!(err.contains("Machine settings"), "{err}");
    // Plunge fits the same range.
    v.insert(names::ENTRY.into(), t("plunge"));
    make(&f, &Rings, &v, &[]).unwrap();
}

#[test]
fn req_op_11_req_nfr_06_turning_leads_are_the_same_at_one_two_and_many_threads() {
    let f = cone();
    let v = roughing_values("rings");
    let at = |threads: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| make(&f, &RotaryRoughing, &v, &[]).unwrap().group.segments)
    };
    let one = at(1);
    assert_eq!(one, at(2));
    assert_eq!(one, at(8));
}

#[test]
fn req_op_11_req_op_15_helical_finishing_of_the_blisk_in_rings_gives_each_end_the_lead_its_shape_asks_for()
 {
    // The repository's public stand-in, `testdata/stl/blisk.stl`: a real
    // part whose rings are not round. Each end of a pass gets the lead its
    // shape asks for (decision 0544 item 2): the lead in the plane of the
    // cut where it turns A, the unchanged line lead where it is a line.
    let f = Fixture::new(MeshSurface::new(&blisk()).unwrap(), 21.0, &ball(3.0));
    let mut v = finishing("rings");
    v.insert(names::STEPOVER.into(), n(5.0));
    let path = make(&f, &HelicalFinishing, &v, &[]).unwrap();
    let ins = lead_ins(&path);
    let outs = lead_outs(&path);
    // The rings are one pass, each stepping to the next without lifting.
    assert!(!ins.is_empty() && !outs.is_empty());
    for lead in &ins {
        let head = lead.moves.last().unwrap().end;
        if (lead.cut[0].a.0 - head.a.0).abs() > 1e-9 {
            assert_turning_lead_in(&f, lead);
        } else {
            // This pass starts with a short line along X up the blisk's end
            // before its rings, so it keeps the line lead (decision 0544
            // item 2): A fixed through the arc.
            assert!(
                lead.moves
                    .iter()
                    .all(|s| (s.end.a.0 - head.a.0).abs() < 1e-9)
            );
        }
    }
    for lead in &outs {
        if (lead.cut[1].a.0 - lead.before.a.0).abs() > 1e-9 {
            assert_turning_lead_out(&f, lead);
        } else {
            // As at the start, the pass ends with a line down the blisk's
            // other end, so it keeps the line lead-out.
            let a = lead.before.a.0;
            assert!(lead.moves.iter().all(|s| (s.end.a.0 - a).abs() < 1e-9));
        }
    }
}

/// One pass of one point over the top of the part. Not a product strategy.
#[derive(Debug)]
struct OnePoint;

impl Strategy for OnePoint {
    fn name(&self) -> &str {
        "test_one_point"
    }

    fn label(&self) -> &str {
        "One point (test)"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        _p: &Parameters,
        _context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let line = input
            .rotary
            .tool_line(Millimetres(10.0), Degrees(0.0), Millimetres(0.0));
        let c = input.surface.drop_tool(input.tool, &line).unwrap();
        Ok(vec![Pass {
            points: vec![CutPoint {
                tip: c.tip,
                a: Degrees(0.0),
                contact: c.contact,
            }],
            ..Pass::default()
        }])
    }
}

#[test]
fn req_op_11_a_single_point_pass_under_lead_is_plunged_with_a_note_naming_it() {
    let f = cylinder();
    let mut v = BTreeMap::new();
    v.insert(names::STEPOVER.into(), n(2.0));
    v.insert(names::FEED.into(), n(FEED));
    v.insert(names::PLUNGE_FEED.into(), n(100.0));
    v.insert(names::ENTRY.into(), t("lead"));
    let path = make(&f, &OnePoint, &v, &[]).unwrap();
    let kinds: Vec<MoveKind> = path.group.segments.iter().map(|s| s.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == MoveKind::Plunge).count(), 1);
    assert!(!kinds.contains(&MoveKind::LeadIn));
    assert!(!kinds.contains(&MoveKind::LeadOut));
    assert!(
        path.notices
            .iter()
            .any(|n| n.topic.as_deref() == Some("Lead entry")
                && n.text.starts_with("pass 1 is a single point")),
        "{:?}",
        path.notices
    );
}

/// One pass at `x` that turns A from 0 to `a_end` degrees in 2 degree
/// steps, dropped onto the part, or held at `height` above the axis when
/// set; it carries `stock` when set, as roughing does. Not a product
/// strategy.
#[derive(Debug)]
struct Arc {
    x: f64,
    a_end: f64,
    height: Option<f64>,
    stock: Option<openchuck_path::AnalyticStock>,
}

impl Strategy for Arc {
    fn name(&self) -> &str {
        "test_arc"
    }

    fn label(&self) -> &str {
        "Arc (test)"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        _p: &Parameters,
        _context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let steps = (self.a_end / 2.0).round() as u32;
        let points = (0..=steps)
            .filter_map(|j| {
                let a = Degrees(f64::from(j) * 2.0);
                match self.height {
                    Some(height) => {
                        let tip = input.rotary.to_model(
                            openchuck_path::rotary::AxisPosition {
                                along: self.x,
                                across: 0.0,
                                height,
                            },
                            a,
                        );
                        Some(CutPoint {
                            tip,
                            a,
                            contact: tip,
                        })
                    }
                    None => {
                        let line = input
                            .rotary
                            .tool_line(Millimetres(self.x), a, Millimetres(0.0));
                        input
                            .surface
                            .drop_tool(input.tool, &line)
                            .map(|c| CutPoint {
                                tip: c.tip,
                                a,
                                contact: c.contact,
                            })
                    }
                }
            })
            .collect();
        Ok(vec![Pass {
            points,
            stock: self
                .stock
                .map(|s| openchuck_path::PassStock(std::sync::Arc::new(s))),
            ..Pass::default()
        }])
    }
}

fn lead_values() -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    v.insert(names::STEPOVER.into(), n(2.0));
    v.insert(names::FEED.into(), n(FEED));
    v.insert(names::PLUNGE_FEED.into(), n(100.0));
    v.insert(names::ENTRY.into(), t("lead"));
    v
}

#[test]
fn req_op_11_req_op_15_a_partial_ring_gets_the_lead_in_the_plane_of_the_cut_at_both_ends() {
    let f = cylinder();
    let s = Arc {
        x: 20.0,
        a_end: 180.0,
        height: None,
        stock: None,
    };
    let path = make(&f, &s, &lead_values(), &[]).unwrap();
    let ins = lead_ins(&path);
    let outs = lead_outs(&path);
    assert_eq!((ins.len(), outs.len()), (1, 1));
    assert_turning_lead_in(&f, &ins[0]);
    assert_turning_lead_out(&f, &outs[0]);
}

#[test]
fn req_op_11_req_op_14_in_roughing_a_lead_beside_a_partial_ring_through_uncut_stock_is_refused_naming_entry()
 {
    // The pass carries stock 16 mm from the axis; the arc rises 3 mm above
    // the 15 mm part beside the half turn, which the pass does not cut.
    let f = cylinder();
    let s = Arc {
        x: 20.0,
        a_end: 180.0,
        height: None,
        stock: Some(openchuck_path::AnalyticStock::Cylinder {
            radius: 16.0,
            x_start: -1.0,
            x_end: 41.0,
        }),
    };
    let err = make(&f, &s, &lead_values(), &[]).unwrap_err();
    assert!(err.starts_with("Lead entry: pass 1"), "{err}");
    assert!(err.contains("stock the pass has not cleared"), "{err}");
    assert!(err.contains("Set Entry to Plunge, Ramp or Helix"), "{err}");
    // A whole ring in the same stock lies over stock the pass itself
    // removes, so it is allowed (decision 0544, the owner's answer).
    let ring = Arc { a_end: 360.0, ..s };
    make(&f, &ring, &lead_values(), &[]).unwrap();
}

/// A part with twelve lobes. With `lobe` negative a ring at A = 0 starts in
/// a valley, where the ball's path has a cusp: behind the first point it
/// rises faster than any arc tangent to the cut. With `lobe` positive it
/// starts on a crest.
fn lobed(lobe: f64) -> Fixture {
    let part = openchuck_testkit::parts::LobedVase {
        base: 15.0,
        wave: 0.0,
        wave_length: 40.0,
        lobe,
        lobes: 12,
        length: 40.0,
    };
    Fixture::new(
        MeshSurface::new(&part.mesh(40, 720)).unwrap(),
        18.0,
        &ball(6.0),
    )
}

#[test]
fn req_op_11_req_nfr_07_a_turning_lead_that_would_cut_the_finished_surface_is_refused_naming_lead_radius()
 {
    let f = lobed(-2.0);
    let s = Arc {
        x: 20.0,
        a_end: 360.0,
        height: None,
        stock: None,
    };
    let err = make(&f, &s, &lead_values(), &[]).unwrap_err();
    assert!(err.starts_with("Lead entry: pass 1"), "{err}");
    assert!(err.contains("into the finished surface"), "{err}");
    assert!(err.contains("Make Lead radius smaller"), "{err}");
    // The same ring starting on a crest gets its lead.
    let path = make(&lobed(2.0), &s, &lead_values(), &[]).unwrap();
    assert_turning_lead_in(&lobed(2.0), &lead_ins(&path)[0]);
}

#[test]
fn req_op_11_a_turning_pass_with_its_tip_on_the_rotary_axis_is_plunged_with_a_note_and_no_lead_out()
{
    // An end face's ring: a lead has no plane of the cut at the axis, so it
    // is entered as a single-point pass is (decision 0543 point 2, the
    // owner's answer, relayed; 0544 item 4).
    let f = cylinder();
    let s = Arc {
        x: 20.0,
        a_end: 90.0,
        height: Some(0.0),
        stock: None,
    };
    let path = make(&f, &s, &lead_values(), &[]).unwrap();
    let kinds: Vec<MoveKind> = path.group.segments.iter().map(|s| s.kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == MoveKind::Plunge).count(), 1);
    assert!(!kinds.contains(&MoveKind::LeadIn));
    assert!(!kinds.contains(&MoveKind::LeadOut));
    // One note says so, in its own words (not "a single point"), and no
    // second note for its end.
    let lead: Vec<&str> = path
        .notices
        .iter()
        .filter(|n| n.topic.as_deref() == Some("Lead entry"))
        .map(|n| n.text.as_str())
        .collect();
    assert_eq!(lead.len(), 1, "{lead:?}");
    assert!(
        lead[0].starts_with("pass 1 starts on the rotary axis, where a lead arc has no plane"),
        "{lead:?}"
    );
}

#[test]
fn req_op_11_a_turning_pass_that_ends_on_the_rotary_axis_has_no_lead_out_and_a_note() {
    // A cut that spirals in to the axis: it starts off the axis and gets its
    // lead-in, and ends on it, with no lead-out.
    let f = cylinder();
    let s = Inward;
    let path = make(&f, &s, &lead_values(), &[]).unwrap();
    let kinds: Vec<MoveKind> = path.group.segments.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&MoveKind::LeadIn));
    assert!(!kinds.contains(&MoveKind::LeadOut));
    assert!(
        path.notices
            .iter()
            .any(|n| n.topic.as_deref() == Some("Lead entry")
                && n.text == "pass 1 ends on the rotary axis, so it has no lead-out"),
        "{:?}",
        path.notices
    );
}

/// One pass at X 60, beyond the test cylinder, that turns A from 0 to 180
/// degrees at 5 mm from the axis, then on to 360 while the tip comes in to
/// the axis. Not a product strategy.
#[derive(Debug)]
struct Inward;

impl Strategy for Inward {
    fn name(&self) -> &str {
        "test_inward"
    }

    fn label(&self) -> &str {
        "Inward (test)"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        _p: &Parameters,
        _context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let points = (0..=180)
            .map(|j| {
                let a = f64::from(j) * 2.0;
                let height = if a <= 180.0 {
                    5.0
                } else {
                    5.0 * (360.0 - a) / 180.0
                };
                let tip = input.rotary.to_model(
                    openchuck_path::rotary::AxisPosition {
                        along: 60.0,
                        across: 0.0,
                        height,
                    },
                    Degrees(a),
                );
                CutPoint {
                    tip,
                    a: Degrees(a),
                    contact: tip,
                }
            })
            .collect();
        Ok(vec![Pass {
            points,
            ..Pass::default()
        }])
    }
}
