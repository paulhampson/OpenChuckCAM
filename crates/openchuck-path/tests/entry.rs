// SPDX-License-Identifier: AGPL-3.0-or-later
//! Entry and exit moves: plunge, lead, ramp and helix, run through a stub
//! strategy that cuts a line along X (OP-11, OP-14, OP-15; decision record
//! 0026).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::{
    CutPoint, MeshSurface, ParameterKind, ParameterSchema, ParameterSpec, ParameterValue,
    Parameters, Pass, PathError, Result, Strategy, StrategyInput, Unit, generate,
};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::{MoveKind, Segment, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, ball, run};

/// One pass along X at A = 0, over the top of the part, from `x_start` to
/// `x_end` in 0.5 mm steps. Not a product strategy.
#[derive(Debug)]
struct Line;

impl Strategy for Line {
    fn name(&self) -> &str {
        "test_line"
    }

    fn label(&self) -> &str {
        "Line (test)"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }

    fn schema(&self) -> ParameterSchema {
        let num = |name: &str| ParameterSpec {
            name: name.to_owned(),
            label: name.to_owned(),
            description: String::new(),
            kind: ParameterKind::Number {
                unit: Unit::Millimetres,
                min: None,
                min_exclusive: false,
                max: None,
                default: None,
            },
        };
        ParameterSchema::with_common(vec![num("x_start"), num("x_end")])
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        p: &Parameters,
        _context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let (x0, x1) = (p.number("x_start")?, p.number("x_end")?);
        let n = ((x1 - x0) / 0.5).round() as u32;
        let points = (0..=n)
            .filter_map(|i| {
                let x = x0 + f64::from(i) * 0.5;
                let line = input
                    .rotary
                    .tool_line(Millimetres(x), Degrees(0.0), Millimetres(0.0));
                input
                    .surface
                    .drop_tool(input.tool, &line)
                    .map(|c| CutPoint {
                        tip: c.tip,
                        a: Degrees(0.0),
                        contact: c.contact,
                    })
            })
            .collect();
        Ok(vec![Pass {
            helical_pass_start: false,
            points,
            stock: None,
            rapids: Vec::new(),
        }])
    }
}

fn cylinder() -> (Cylinder, Fixture) {
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 1440)).unwrap(),
        20.0,
        &ball(6.0),
    );
    (part, fixture)
}

/// A cone whose radius falls from 15 mm to 8 mm along X, so the surface
/// rises towards the start of a pass cut towards +X.
fn cone() -> (Cone, Fixture) {
    let part = Cone {
        start_radius: 15.0,
        end_radius: 8.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(40, 1440)).unwrap(),
        20.0,
        &ball(6.0),
    );
    (part, fixture)
}

fn values(x0: f64, x1: f64, extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    for (k, x) in [
        ("x_start", x0),
        ("x_end", x1),
        (names::STEPOVER, 1.0),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ] {
        v.insert(k.to_owned(), ParameterValue::Number(x));
    }
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn entry(style: &str) -> (&'static str, ParameterValue) {
    (names::ENTRY, ParameterValue::Text(style.to_owned()))
}

fn number(name: &'static str, x: f64) -> (&'static str, ParameterValue) {
    (name, ParameterValue::Number(x))
}

fn path(
    fixture: &Fixture,
    values: &BTreeMap<String, ParameterValue>,
) -> std::result::Result<ToolpathGroup, PathError> {
    run(|ctx| generate(&Line, &fixture.input(), values, ctx)).map(|g| g.group)
}

/// The kinds of the segments, with runs of one kind collapsed.
fn kinds(group: &ToolpathGroup) -> Vec<MoveKind> {
    let mut out: Vec<MoveKind> = Vec::new();
    for s in &group.segments {
        if out.last() != Some(&s.kind) {
            out.push(s.kind);
        }
    }
    out
}

fn of_kind(group: &ToolpathGroup, kind: MoveKind) -> Vec<&Segment> {
    group.segments.iter().filter(|s| s.kind == kind).collect()
}

/// Horizontal position in the fixed frame: (along, across).
fn flat(fixture: &Fixture, s: &Segment) -> (f64, f64) {
    let p = fixture.rotary.to_axis(s.end.tip, s.end.a);
    (p.along, p.across)
}

fn height(fixture: &Fixture, s: &Segment) -> f64 {
    fixture.rotary.to_axis(s.end.tip, s.end.a).height
}

/// No entry moves other than the plunge, and the plunge is the default.
#[test]
fn req_op_11_plunge_is_the_default_entry_and_changes_nothing() {
    let (_, fixture) = cylinder();
    let plain = path(&fixture, &values(10.0, 20.0, &[])).unwrap();
    let named = path(&fixture, &values(10.0, 20.0, &[entry("plunge")])).unwrap();
    assert_eq!(plain, named);
    assert_eq!(
        kinds(&plain),
        [
            MoveKind::Rapid,
            MoveKind::Plunge,
            MoveKind::Cutting,
            MoveKind::Linking,
            MoveKind::Rapid
        ]
    );
}

#[test]
fn req_op_11_entry_parameters_are_validated() {
    let (_, fixture) = cylinder();
    for (extra, text) in [
        (entry("spiral"), "not one of"),
        (number(names::RAMP_ANGLE, 0.0), "greater than 0"),
        (number(names::RAMP_ANGLE, 90.0), "at most 89"),
        (number(names::HELIX_DIAMETER, 0.0), "greater than 0"),
        (number(names::LEAD_RADIUS, -1.0), "at least 0"),
    ] {
        let err = path(&fixture, &values(10.0, 20.0, &[extra])).unwrap_err();
        assert!(err.to_string().contains(text), "{err}");
    }
}

#[test]
fn req_op_11_ramp_reaches_the_first_cut_point_at_the_requested_angle() {
    let (part, fixture) = cone();
    let v = values(
        30.0,
        38.0,
        &[entry("ramp"), number(names::RAMP_ANGLE, 15.0)],
    );
    let group = path(&fixture, &v).unwrap();
    assert!(of_kind(&group, MoveKind::Plunge).is_empty());
    let ramps = of_kind(&group, MoveKind::LeadIn);
    assert_eq!(ramps.len(), 1);
    let ramp = ramps[0];
    assert_eq!(ramp.contact_feed.unwrap().0, 100.0);
    let first_cut = of_kind(&group, MoveKind::Cutting)[0];
    let index = |s: &Segment| {
        group
            .segments
            .iter()
            .position(|x| std::ptr::eq(x, s))
            .unwrap()
    };
    assert_eq!(index(first_cut), index(ramp) + 1);
    // It ends at the first cut point, and the move before it ends at its start.
    let before = &group.segments[index(ramp) - 1];
    let (a0, c0) = flat(&fixture, before);
    let (a1, c1) = flat(&fixture, ramp);
    let run = (a1 - a0).hypot(c1 - c0);
    let drop = height(&fixture, before) - height(&fixture, ramp);
    assert!(
        (drop / run - 15.0f64.to_radians().tan()).abs() < 1e-9,
        "{}",
        drop / run
    );
    // The descent stops the approach distance above the stock, and the ramp
    // runs back along the cut (towards the lower X).
    assert!((height(&fixture, before) - 21.0).abs() < 1e-9);
    assert!(a0 < a1);
    // Nothing gouges, entry moves included.
    let outline = Outline::new(&fixture.tool);
    let poses: Vec<Pose> = group
        .segments
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::LeadIn | MoveKind::Cutting))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect();
    assert!(max_gouge(&outline, &part, &poses, 0.1).depth <= 0.001);
}

/// A pass that is a single cut point (the part's tip just reaches this
/// layer) has no horizontal move to ramp or lead along: it is entered by a
/// plunge, as the owner's blisk roughing at 3 degrees needs (issue 199;
/// decision 0204), and the lead-out has nothing to follow either.
#[test]
fn req_op_11_nfr_07_a_single_point_pass_is_entered_by_plunge_for_ramp_and_lead() {
    let (_, fixture) = cylinder();
    let plain = path(&fixture, &values(10.0, 10.0, &[])).unwrap();
    for style in ["ramp", "lead"] {
        let v = values(10.0, 10.0, &[entry(style), number(names::RAMP_ANGLE, 3.0)]);
        let group = path(&fixture, &v).unwrap_or_else(|e| panic!("{style}: {e}"));
        assert_eq!(group, plain, "{style}");
        assert_eq!(of_kind(&group, MoveKind::Plunge).len(), 1, "{style}");
        assert!(of_kind(&group, MoveKind::LeadIn).is_empty(), "{style}");
        assert!(of_kind(&group, MoveKind::LeadOut).is_empty(), "{style}");
    }
}

#[test]
fn req_op_11_ramp_that_would_gouge_is_refused_naming_its_size() {
    let (_, fixture) = cone();
    let v = values(30.0, 38.0, &[entry("ramp"), number(names::RAMP_ANGLE, 3.0)]);
    let err = path(&fixture, &v).unwrap_err().to_string();
    assert!(err.contains("ramp"), "{err}");
    assert!(err.contains("a run of"), "{err}");
    assert!(err.contains("into the finished surface"), "{err}");
    // The style is never changed for the user.
    assert!(err.contains("try another entry style"), "{err}");
}

#[test]
fn req_op_11_helix_descends_on_a_circle_and_ends_tangent_at_the_first_cut_point() {
    let (part, fixture) = cylinder();
    let v = values(
        10.0,
        20.0,
        &[entry("helix"), number(names::RAMP_ANGLE, 5.0)],
    );
    let group = path(&fixture, &v).unwrap();
    assert!(of_kind(&group, MoveKind::Plunge).is_empty());
    let helix = of_kind(&group, MoveKind::LeadIn);
    assert!(helix.len() > 20, "{}", helix.len());
    assert!(helix.iter().all(|s| s.contact_feed.unwrap().0 == 100.0));
    // It ends where the plunge would have: the pass's first point.
    let plunged = path(&fixture, &values(10.0, 20.0, &[])).unwrap();
    let goal = flat(&fixture, of_kind(&plunged, MoveKind::Plunge)[0]);
    let end = flat(&fixture, helix[helix.len() - 1]);
    assert!((end.0 - goal.0).abs() < 1e-9 && (end.1 - goal.1).abs() < 1e-9);
    // The circle is 0.75 of the 6 mm tool across, its centre beside the first
    // cut point; every vertex lies on it.
    let rho = 0.75 * 3.0;
    let far = helix
        .iter()
        .map(|s| flat(&fixture, s))
        .max_by(|p, q| {
            let d = |r: &(f64, f64)| (r.0 - goal.0).hypot(r.1 - goal.1);
            d(p).partial_cmp(&d(q)).unwrap()
        })
        .unwrap();
    let centre = ((far.0 + goal.0) / 2.0, (far.1 + goal.1) / 2.0);
    for s in &helix {
        let p = flat(&fixture, s);
        let r = (p.0 - centre.0).hypot(p.1 - centre.1);
        assert!((r - rho).abs() < 1e-6, "{r}");
    }
    // Height never rises, and falls no faster than the requested angle.
    let mut previous = group
        .segments
        .iter()
        .take_while(|s| s.kind != MoveKind::LeadIn)
        .last()
        .map(|s| height(&fixture, s))
        .unwrap();
    let mut from = flat(
        &fixture,
        group
            .segments
            .iter()
            .take_while(|s| s.kind != MoveKind::LeadIn)
            .last()
            .unwrap(),
    );
    for s in &helix {
        let h = height(&fixture, s);
        let p = flat(&fixture, s);
        let run = (p.0 - from.0).hypot(p.1 - from.1);
        assert!(h <= previous + 1e-9);
        assert!(previous - h <= run * 5.0f64.to_radians().tan() + 1e-9);
        previous = h;
        from = p;
    }
    // The tool never rises above the link height, and nothing gouges.
    assert!(helix.iter().all(|s| height(&fixture, s) <= 25.0 + 1e-9));
    let outline = Outline::new(&fixture.tool);
    let poses: Vec<Pose> = group
        .segments
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::LeadIn | MoveKind::Cutting))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect();
    assert!(max_gouge(&outline, &part, &poses, 0.1).depth <= 0.001);
    // A has not moved since the link rapid.
    assert!(helix.iter().all(|s| s.end.a == Degrees(0.0)));
}

#[test]
fn req_op_11_helix_that_would_gouge_is_refused_naming_its_size() {
    let (_, fixture) = cone();
    let v = values(30.0, 38.0, &[entry("helix")]);
    let err = path(&fixture, &v).unwrap_err().to_string();
    assert!(err.contains("helix"), "{err}");
    assert!(err.contains("a circle 4.500 mm across"), "{err}");
    assert!(err.contains("into the finished surface"), "{err}");
}

#[test]
fn req_op_11_lead_arcs_are_tangent_to_the_cut_and_leave_the_surface_alone() {
    let (part, fixture) = cylinder();
    let v = values(10.0, 20.0, &[entry("lead")]);
    let group = path(&fixture, &v).unwrap();
    assert_eq!(
        kinds(&group),
        [
            MoveKind::Rapid,
            MoveKind::Plunge,
            MoveKind::LeadIn,
            MoveKind::Cutting,
            MoveKind::LeadOut,
            MoveKind::Linking,
            MoveKind::Rapid
        ]
    );
    let lead_in = of_kind(&group, MoveKind::LeadIn);
    let lead_out = of_kind(&group, MoveKind::LeadOut);
    let cuts = of_kind(&group, MoveKind::Cutting);
    assert!(
        lead_in
            .iter()
            .chain(&lead_out)
            .all(|s| s.contact_feed.unwrap().0 == 300.0)
    );
    // Quarter circles of the tool radius, 3 mm.
    let p0 = flat(&fixture, lead_in[lead_in.len() - 1]);
    let r = 3.0;
    let first_dir = {
        let q = flat(&fixture, cuts[0]);
        let n = (q.0 - p0.0).hypot(q.1 - p0.1);
        ((q.0 - p0.0) / n, (q.1 - p0.1) / n)
    };
    let normal = (-first_dir.1, first_dir.0);
    let centre = (p0.0 + r * normal.0, p0.1 + r * normal.1);
    for s in &lead_in {
        let p = flat(&fixture, s);
        assert!(((p.0 - centre.0).hypot(p.1 - centre.1) - r).abs() < 1e-9);
    }
    // The last chord of the arc runs along the cut, within half a step.
    let before_last = flat(&fixture, lead_in[lead_in.len() - 2]);
    let chord = (p0.0 - before_last.0, p0.1 - before_last.1);
    let n = chord.0.hypot(chord.1);
    let dot = (chord.0 * first_dir.0 + chord.1 * first_dir.1) / n;
    assert!(dot > 0.99, "{dot}");
    // The exit mirrors it.
    let last_cut = flat(&fixture, cuts[cuts.len() - 1]);
    let end = flat(&fixture, lead_out[lead_out.len() - 1]);
    assert!(
        (end.0 - last_cut.0 - r).abs() < 1e-9,
        "{end:?} {last_cut:?}"
    );
    assert!(((end.1 - last_cut.1).abs() - r).abs() < 1e-9);
    // Same side in and out would lie on one side of the pass; both arcs stay
    // out of the surface.
    let outline = Outline::new(&fixture.tool);
    let poses: Vec<Pose> = group
        .segments
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                MoveKind::Plunge | MoveKind::LeadIn | MoveKind::Cutting | MoveKind::LeadOut
            )
        })
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect();
    assert!(max_gouge(&outline, &part, &poses, 0.1).depth <= 0.001);
}

#[test]
fn req_op_11_entry_output_is_identical_on_repeat() {
    let (_, fixture) = cylinder();
    for style in ["lead", "ramp", "helix"] {
        let v = values(10.0, 20.0, &[entry(style), number(names::RAMP_ANGLE, 8.0)]);
        let a = path(&fixture, &v).unwrap();
        let b = path(&fixture, &v).unwrap();
        assert_eq!(a, b, "{style}");
    }
}

#[test]
fn req_op_11_entry_moves_are_fed_moves_with_positive_durations() {
    let (_, fixture) = cylinder();
    let group = path(
        &fixture,
        &values(
            10.0,
            20.0,
            &[entry("helix"), number(names::RAMP_ANGLE, 8.0)],
        ),
    )
    .unwrap();
    for s in of_kind(&group, MoveKind::LeadIn) {
        assert!(s.duration.0 > 0.0);
        assert!(s.contact_length.0 > 0.0);
        assert!(s.contact_feed.is_some());
    }
}

#[test]
fn req_op_11_back_and_forth_ramp_descends_on_multiple_legs() {
    use openchuck_path::entry_safety::EntrySafety;
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        20.0,
        &ball(6.0),
    );
    let safety = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let mut input = fixture.input();
    input.entry_safety = Some(&safety);
    let v = values(
        20.0,
        30.0,
        &[
            entry("ramp"),
            number(names::RAMP_ANGLE, 3.0),
            (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
        ],
    );
    let path = run(|ctx| generate(&Line, &input, &v, ctx)).unwrap();
    let ramps = of_kind(&path.group, MoveKind::LeadIn);
    assert!(ramps.len() > 2, "expected repeated descent");
    let mut previous = path.group.start;
    let mut descending = 0;
    let mut reversals = 0;
    let mut direction = 0.0_f64;
    for segment in &path.group.segments {
        if segment.kind == MoveKind::LeadIn {
            let a = fixture.rotary.to_axis(previous.tip, previous.a);
            let z = fixture.rotary.to_axis(segment.end.tip, segment.end.a);
            assert_eq!(segment.end.a, previous.a);
            assert_eq!(segment.contact_feed.unwrap().0, 100.0);
            let travel = (z.along - a.along).hypot(z.across - a.across);
            let drop = a.height - z.height;
            assert!(drop >= -1e-10 && drop <= travel * 3.0_f64.to_radians().tan() + 1e-10);
            assert!(segment.duration.0 > 0.0);
            if drop > 1e-10 {
                descending += 1;
            }
            let next = (z.along - a.along).signum();
            if direction * next < 0.0 {
                reversals += 1;
            }
            direction = next;
        }
        previous = segment.end;
    }
    assert!(descending > 2 && reversals > 1);
    let last = ramps.last().unwrap().end;
    let first_cut = path
        .group
        .segments
        .iter()
        .find(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    assert!((fixture.rotary.to_axis(last.tip, last.a).height - 15.0).abs() < 1e-9);
    assert!(first_cut.end.tip.x > last.tip.x);
}

#[test]
fn req_op_11_req_nfr_07_reversing_ramp_refusal_names_pass_with_no_direction() {
    use openchuck_path::entry_safety::EntrySafety;
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        20.0,
        &ball(6.0),
    );
    let safety = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let mut input = fixture.input();
    input.entry_safety = Some(&safety);
    let v = values(
        20.0,
        20.0,
        &[
            entry("ramp"),
            (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
        ],
    );
    let error = run(|ctx| generate(&Line, &input, &v, ctx))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("pass 1") && error.contains("no usable straight entry direction"),
        "{error}"
    );
}

/// Availability matrix from Phase 1; every formerly budget-limited case
/// must certify under accepted 0248, with the same tool, geometry and angle.
/// Verification plan sections 4 (OP-11/12/14 and acceptance thresholds),
/// 5 (pathing tests) and 8 (environment/tools).
#[test]
fn req_op_11_req_op_12_req_op_14_req_nfr_04_req_nfr_07_rampbudget_characterization() {
    use openchuck_path::entry_safety::{EntrySafety, RotaryPrecision};
    for (diameter, tolerance, angle) in [
        (3.175, 0.01, 3.0),
        (6.0, 0.01, 3.0),
        (6.0, 0.02, 3.0),
        (3.175, 0.01, 6.0),
    ] {
        for descent in [8.0, 10.0, 12.0, 14.0, 24.0] {
            let part = Cylinder {
                radius: 15.0,
                length: 40.0,
            };
            // Explicit approach is 2 mm; descent is stock - target + approach.
            let fixture = Fixture::new(
                MeshSurface::new(&part.mesh(3, 36)).unwrap(),
                15.0 + descent - 2.0,
                &ball(diameter),
            );
            let safety = EntrySafety {
                rotary_precision: RotaryPrecision::Float32,
                target: &fixture.surface,
                protected: &[],
                body: &fixture.tool,
                machine: &fixture.machine,
            };
            let mut input = fixture.input();
            input.entry_safety = Some(&safety);
            let v = values(
                20.0,
                30.0,
                &[
                    entry("ramp"),
                    number(names::RAMP_ANGLE, angle),
                    number(names::TOLERANCE, tolerance),
                    number(names::APPROACH_DISTANCE, 2.0),
                    (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
                ],
            );
            let start = std::time::Instant::now();
            let result = run(|ctx| generate(&Line, &input, &v, ctx));
            eprintln!(
                "RAMPBUDGET diameter={diameter} tolerance={tolerance} angle={angle} descent={descent} seconds={:.6} result={}",
                start.elapsed().as_secs_f64(),
                result
                    .as_ref()
                    .map(|_| "certified".to_owned())
                    .unwrap_or_else(|e| e.to_string())
            );
            result.unwrap();
        }
    }
}

#[derive(Debug)]
struct RepeatedLine;
impl Strategy for RepeatedLine {
    fn name(&self) -> &str {
        "repeated_line"
    }
    fn label(&self) -> &str {
        "Repeated line (measurement)"
    }
    fn description(&self) -> &str {
        "Three identical passes to measure reusable entry queries"
    }
    fn schema(&self) -> ParameterSchema {
        Line.schema()
    }
    fn passes(
        &self,
        input: &StrategyInput<'_>,
        p: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let passes = Line.passes(input, p, context)?;
        Ok((0..3).flat_map(|_| passes.clone()).collect())
    }
}

#[test]
fn req_op_11_req_nfr_04_rampbudget_unchanged_pass_inputs() {
    use openchuck_path::entry_safety::{EntrySafety, RotaryPrecision};
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        21.0,
        &ball(3.175),
    );
    let safety = EntrySafety {
        rotary_precision: RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let mut input = fixture.input();
    input.entry_safety = Some(&safety);
    let v = values(
        20.0,
        30.0,
        &[
            entry("ramp"),
            number(names::RAMP_ANGLE, 3.0),
            number(names::APPROACH_DISTANCE, 2.0),
            (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
        ],
    );
    run(|ctx| generate(&RepeatedLine, &input, &v, ctx)).unwrap();
}

#[test]
fn req_op_11_req_nfr_07_rampbudget_exact_subinterval_boundary() {
    use openchuck_path::entry_safety::{EntrySafety, RotaryPrecision};
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        21.0,
        &ball(3.175),
    );
    let safety = EntrySafety {
        rotary_precision: RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let pose = |x| openchuck_types::ir::ToolPose {
        tip: openchuck_types::geometry::Point3::new(x, 0.0, 35.0),
        a: Degrees(0.0),
        axis: fixture.rotary.tool_axis(Degrees(0.0)),
    };
    // Charge the same shared budget across repeated clear sweeps: below,
    // at and beyond the 100000-attempt cap. Adaptive proof does not precharge
    // an obsolete uniform grid's theoretical length.
    let mut budget = 100_000;
    for _ in 0..99_999 {
        run(|ctx| safety.verify(pose(20.0), pose(21.0), 0.0, 0.00004, &mut budget, ctx)).unwrap();
    }
    assert_eq!(budget, 1);
    run(|ctx| safety.verify(pose(20.0), pose(21.0), 0.0, 0.00004, &mut budget, ctx)).unwrap();
    assert_eq!(budget, 0);
    let error = run(|ctx| safety.verify(pose(20.0), pose(21.0), 0.0, 0.00004, &mut budget, ctx))
        .unwrap_err();
    assert!(error.to_string().contains("budget"));
}

#[test]
fn req_op_11_req_op_12_req_nfr_04_deep_rampbudget_entry_is_available() {
    use openchuck_path::entry_safety::{EntrySafety, RotaryPrecision};
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        25.0,
        &ball(3.175),
    );
    let safety = EntrySafety {
        rotary_precision: RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let mut input = fixture.input();
    input.entry_safety = Some(&safety);
    let v = values(
        20.0,
        30.0,
        &[
            entry("ramp"),
            number(names::RAMP_ANGLE, 3.0),
            number(names::APPROACH_DISTANCE, 2.0),
            (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
        ],
    );
    run(|ctx| generate(&Line, &input, &v, ctx)).unwrap();
}

#[derive(Default)]
struct EntryCounts(std::sync::Mutex<BTreeMap<String, usize>>);
impl openchuck_types::job::ProgressSink for EntryCounts {
    fn report(&self, _: openchuck_types::job::Progress) {}
    fn wants_timings(&self) -> bool {
        true
    }
    fn timing(&self, stage: &str, _: std::time::Duration) {
        *self.0.lock().unwrap().entry(stage.to_owned()).or_default() += 1;
    }
}

#[test]
fn req_op_11_req_op_12_req_nfr_04_every_leg_reversal_approach_and_connection_is_certified_with_memoisation()
 {
    use openchuck_path::entry_safety::{EntrySafety, RotaryPrecision};
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        21.0,
        &ball(3.175),
    );
    let safety = EntrySafety {
        rotary_precision: RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let mut input = fixture.input();
    input.entry_safety = Some(&safety);
    let v = values(
        20.0,
        30.0,
        &[
            entry("ramp"),
            number(names::RAMP_ANGLE, 3.0),
            number(names::APPROACH_DISTANCE, 2.0),
            (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
        ],
    );
    let counts = EntryCounts::default();
    let cancel = openchuck_types::job::CancelToken::new();
    let ctx = JobContext::new(&cancel, &counts);
    let generated = generate(&RepeatedLine, &input, &v, &ctx).unwrap();
    let counts = counts.0.lock().unwrap();
    let ramps = of_kind(&generated.group, MoveKind::LeadIn);
    // Each ramp segment is either a descending leg (including every reversal)
    // or its final connection. Plus each entry's approach and corridor.
    assert_eq!(counts["entry clearance sweep"], ramps.len() + 3 * 2);
    assert!(counts["entry clearance cache hit"] > 0);
    eprintln!("RAMPBUDGET adaptive three passes: {:?}", *counts);
    // Diagnostics do not affect the generated motion.
    assert_eq!(
        generated.group,
        run(|ctx| generate(&RepeatedLine, &input, &v, ctx))
            .unwrap()
            .group
    );
}

#[test]
fn req_gc_07_req_nfr_06_deep_reversing_entry_is_identical_at_1_2_and_n_threads() {
    use openchuck_path::entry_safety::{EntrySafety, RotaryPrecision};
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        25.0,
        &ball(3.175),
    );
    let safety = EntrySafety {
        rotary_precision: RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let mut input = fixture.input();
    input.entry_safety = Some(&safety);
    let v = values(
        20.0,
        30.0,
        &[
            entry("ramp"),
            number(names::RAMP_ANGLE, 3.0),
            number(names::APPROACH_DISTANCE, 2.0),
            (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
        ],
    );
    let mut expected = None;
    for threads in [
        1,
        2,
        std::thread::available_parallelism().map_or(4, usize::from),
    ] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let generated = pool
            .install(|| run(|ctx| generate(&RepeatedLine, &input, &v, ctx)))
            .unwrap();
        let bytes = serde_json::to_vec(&generated.group).unwrap();
        if let Some(expected) = &expected {
            assert_eq!(expected, &bytes);
        } else {
            expected = Some(bytes);
        }
    }
}

#[derive(Debug)]
struct ClearedLocalStock;
impl Strategy for ClearedLocalStock {
    fn name(&self) -> &str {
        "cleared_local_stock"
    }
    fn label(&self) -> &str {
        "Cleared local stock (test)"
    }
    fn description(&self) -> &str {
        "Prior removal does not waive incoming stock clearance"
    }
    fn schema(&self) -> ParameterSchema {
        Line.schema()
    }
    fn passes(
        &self,
        input: &StrategyInput<'_>,
        p: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let mut passes = RepeatedLine.passes(input, p, context)?;
        for pass in &mut passes {
            pass.stock = Some(openchuck_path::PassStock(std::sync::Arc::new(
                openchuck_path::AnalyticStock::Cylinder {
                    radius: 15.0,
                    x_start: 0.0,
                    x_end: 40.0,
                },
            )));
        }
        Ok(passes)
    }
}
#[test]
fn req_op_14_req_op_11_local_stock_removal_does_not_lower_the_unproven_incoming_rapid() {
    use openchuck_path::entry_safety::{EntrySafety, RotaryPrecision};
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(3, 36)).unwrap(),
        25.0,
        &ball(3.175),
    );
    let safety = EntrySafety {
        rotary_precision: RotaryPrecision::Float32,
        target: &fixture.surface,
        protected: &[],
        body: &fixture.tool,
        machine: &fixture.machine,
    };
    let mut input = fixture.input();
    input.entry_safety = Some(&safety);
    let v = values(
        20.0,
        30.0,
        &[
            entry("ramp"),
            number(names::RAMP_ANGLE, 3.0),
            number(names::APPROACH_DISTANCE, 2.0),
            (names::RAMP_BACK_AND_FORTH, ParameterValue::Flag(true)),
        ],
    );
    let generated = run(|ctx| generate(&ClearedLocalStock, &input, &v, ctx)).unwrap();
    let mut previous = generated.group.start;
    let mut previous_kind = MoveKind::Rapid;
    let mut starts = 0;
    for s in &generated.group.segments {
        if s.kind == MoveKind::LeadIn && previous_kind != MoveKind::LeadIn {
            starts += 1;
            assert!(
                previous.tip.z >= 27.0,
                "local stock does not prove the incoming rapid: {previous:?}"
            );
        }
        previous = s.end;
        previous_kind = s.kind;
    }
    assert_eq!(starts, 3);
}
