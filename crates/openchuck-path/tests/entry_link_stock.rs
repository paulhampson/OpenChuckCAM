// SPDX-License-Identifier: AGPL-3.0-or-later
//! The move to an entry's start is checked against the stock along its whole
//! line (OP-14, OP-11, NFR-07; issue #642).
//!
//! A link ends above the pass's first cut point at a height checked against
//! the stock under the tool along the link. A lead or ramp entry then moves
//! sideways at that height to its own start. These tests put a raised
//! feature on the stock beside the first cut point, where the link's check
//! does not look, link the pass low (decision 0496) and assert that no rapid
//! sweeps the tool through the stock. Synthetic fixtures only.
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
    CutPoint, MeshSurface, ParameterSchema, ParameterValue, Parameters, Pass, Result, StockModel,
    Strategy, StrategyInput,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::ir::MoveKind;
use openchuck_types::job::JobContext;
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, ball, generate, run};

/// Round stock of radius 16 mm along the part, with a block raised beside
/// the second pass's first cut point. The block is given in the machine's
/// fixed frame at A = 0 (along, across, up to `top`), which is where the
/// passes cut.
#[derive(Debug)]
struct Bumpy {
    along: (f64, f64),
    across: (f64, f64),
    top: f64,
}

impl Bumpy {
    const RADIUS: f64 = 16.0;
}

impl StockModel for Bumpy {
    fn contains(&self, p: Point3) -> bool {
        let fixture_rotary = openchuck_path::RotaryMapping::of_profile(&support::machine());
        let q = fixture_rotary.to_axis(p, Degrees(0.0));
        let round = (-1.0..=41.0).contains(&q.along) && q.across.hypot(q.height) <= Self::RADIUS;
        let block = (self.along.0..=self.along.1).contains(&q.along)
            && (self.across.0..=self.across.1).contains(&q.across)
            && (0.0..=self.top).contains(&q.height);
        round || block
    }

    fn ray_entry(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<f64> {
        let step = 0.01;
        let n = (max_distance / step).ceil() as usize;
        (0..=n)
            .map(|i| (i as f64 * step).min(max_distance))
            .find(|&t| self.contains(origin + direction.get() * t))
    }

    fn max_radius(&self) -> Option<f64> {
        let far = self.across.0.abs().max(self.across.1.abs());
        Some(Self::RADIUS.max(far.hypot(self.top)))
    }
}

/// Passes along X at A = 0 over the top of the part, each from the first X
/// to the second in 0.5 mm steps; every pass after the first asks for a low
/// link (decision 0496).
#[derive(Debug)]
struct Lines(Vec<(f64, f64)>);

impl Strategy for Lines {
    fn name(&self) -> &str {
        "test_lines"
    }
    fn label(&self) -> &str {
        "Lines (test)"
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
        Ok(self
            .0
            .iter()
            .enumerate()
            .map(|(k, &(x0, x1))| {
                let n = ((x1 - x0).abs() / 0.5).round() as u32;
                let s = (x1 - x0).signum();
                let points = (0..=n)
                    .filter_map(|i| {
                        let x = x0 + s * f64::from(i) * 0.5;
                        let line =
                            input
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
                Pass {
                    helical_pass_start: false,
                    low_link: k > 0,
                    points,
                    stock: None,
                    rapids: Vec::new(),
                    plunge_fed: Vec::new(),
                }
            })
            .collect())
    }
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    for (k, x) in [
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

/// Where a rapid sweeps the tool (a 6 mm ball, its body up to 10 mm above
/// the tip) through `stock`: the first offending rapid's index and position.
fn rapid_into_stock(
    fixture: &Fixture,
    stock: &dyn StockModel,
    path: &openchuck_path::GeneratedPath,
) -> Option<(usize, f64, f64, f64)> {
    let r = 3.0;
    let mut prev = path.group.start;
    for (i, s) in path.group.segments.iter().enumerate() {
        if s.kind == MoveKind::Rapid {
            let from = fixture.rotary.to_axis(prev.tip, prev.a);
            let to = fixture.rotary.to_axis(s.end.tip, s.end.a);
            let length = (to.along - from.along)
                .hypot(to.across - from.across)
                .hypot(to.height - from.height);
            let steps = ((length / 0.1).ceil() as usize).max(1);
            for k in 0..=steps {
                let f = k as f64 / steps as f64;
                let along = from.along + f * (to.along - from.along);
                let across = from.across + f * (to.across - from.across);
                let height = from.height + f * (to.height - from.height);
                // The ball and the shank above it, probed on rings.
                for h10 in 0..=100 {
                    let h = f64::from(h10) * 0.1;
                    let ring = if h < r {
                        (r * r - (r - h) * (r - h)).sqrt()
                    } else {
                        r
                    };
                    for j in 0..16 {
                        let t = f64::from(j) * std::f64::consts::TAU / 16.0;
                        for frac in [0.0, 0.5, 0.98] {
                            let q = openchuck_path::rotary::AxisPosition {
                                along: along + frac * ring * t.cos(),
                                across: across + frac * ring * t.sin(),
                                height: height + h,
                            };
                            if stock.contains(fixture.rotary.to_model(q, Degrees(0.0))) {
                                return Some((i, along, across, height));
                            }
                        }
                    }
                }
            }
        }
        prev = s.end;
    }
    None
}

fn fixture() -> Fixture {
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    // Stock radius 20 mm in the fixture only sets the clearance (25 mm);
    // the stock itself is `Bumpy`.
    Fixture::new(
        MeshSurface::new(&part.mesh(3, 720)).unwrap(),
        20.0,
        &ball(6.0),
    )
}

/// Generates `lines` with `entry` on `stock` and says where a rapid enters
/// it, if one does. `Err` is a refusal, which is a correct outcome.
fn try_entry(
    lines: Vec<(f64, f64)>,
    stock: &Bumpy,
    extra: &[(&str, ParameterValue)],
) -> std::result::Result<Option<(usize, f64, f64, f64)>, String> {
    let f = fixture();
    let mut input = f.input();
    input.stock = stock;
    let v = values(extra);
    let path = run(|ctx| generate(&Lines(lines), &input, &v, ctx)).map_err(|e| e.to_string())?;
    Ok(rapid_into_stock(&f, stock, &path))
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

#[test]
fn req_op_14_req_op_11_the_rapid_to_a_lead_in_start_never_enters_the_stock() {
    // Pass 2 starts at X 20 going +X: the lead-in arc starts 3 mm back and
    // 3 mm to the side (X 17, across +3), beyond the one tool radius the
    // link from X 40 checked. The block stands there, above the low link.
    let stock = Bumpy {
        along: (15.5, 18.5),
        across: (3.5, 6.5),
        top: 19.5,
    };
    let outcome = try_entry(
        vec![(30.0, 40.0), (20.0, 28.0)],
        &stock,
        &[(names::ENTRY, text("lead"))],
    );
    // Generated (a refusal would not show the rapid), with no rapid through
    // the stock.
    assert_eq!(
        outcome,
        Ok(None),
        "a rapid sweeps the tool through the stock"
    );
}

#[test]
fn req_op_14_req_op_11_the_rapid_to_a_ramp_start_never_enters_the_stock() {
    // Pass 1 runs from X 40 to X 30. Written against the straight ramp, which
    // started pass 2's ramp about 11 mm behind its first cut point (X 20,
    // going +X), over the block the link from X 30 never crossed; it failed
    // first on main. The ramp now goes back and forth over pass 2's own path
    // (X 20 to 28, decision 0543), and the move to its start must still stay
    // clear of the stock.
    let stock = Bumpy {
        along: (12.0, 16.0),
        across: (-2.0, 2.0),
        top: 19.5,
    };
    let outcome = try_entry(
        vec![(40.0, 30.0), (20.0, 28.0)],
        &stock,
        &[
            (names::ENTRY, text("ramp")),
            (names::RAMP_ANGLE, ParameterValue::Number(10.0)),
        ],
    );
    // Generated (a refusal would not show the rapid), with no rapid through
    // the stock.
    assert_eq!(
        outcome,
        Ok(None),
        "a rapid sweeps the tool through the stock"
    );
}

#[test]
fn req_op_14_a_plunge_entry_on_the_same_stock_stays_clear() {
    // The control: the plunge comes straight down over the first cut point,
    // where the link checked.
    for stock in [
        Bumpy {
            along: (15.5, 18.5),
            across: (3.5, 6.5),
            top: 19.5,
        },
        Bumpy {
            along: (12.0, 16.0),
            across: (-2.0, 2.0),
            top: 19.5,
        },
    ] {
        let outcome = try_entry(vec![(30.0, 40.0), (20.0, 28.0)], &stock, &[]);
        assert_eq!(outcome, Ok(None), "{stock:?}");
    }
}

/// Generates `lines` with `extra` on `stock`: the path, or the refusal.
fn generated(
    lines: Vec<(f64, f64)>,
    stock: &Bumpy,
    extra: &[(&str, ParameterValue)],
) -> std::result::Result<openchuck_path::GeneratedPath, String> {
    let f = fixture();
    let mut input = f.input();
    input.stock = stock;
    let v = values(extra);
    run(|ctx| generate(&Lines(lines), &input, &v, ctx)).map_err(|e| e.to_string())
}

/// The note on raised entries (decision 0544 item 6), if the path has one.
fn raised_note(path: &openchuck_path::GeneratedPath) -> Option<&str> {
    path.notices
        .iter()
        .find(|n| {
            n.text.contains("start higher than the link")
                || n.text.contains("starts higher than the link")
        })
        .map(|n| n.text.as_str())
}

#[test]
fn req_op_14_req_op_11_a_lead_in_raised_to_clear_the_stock_is_counted_in_a_note() {
    // Issue #642, decision 0544 item 6: the lead-in of pass 2 starts beside
    // the block, where the low link did not look, so the tool rises first
    // and a note counts the entry. Without the rise the rapid enters the
    // block; without the note the owner's check has nothing to read.
    let stock = Bumpy {
        along: (15.5, 18.5),
        across: (3.5, 6.5),
        top: 19.5,
    };
    let path = generated(
        vec![(30.0, 40.0), (20.0, 28.0)],
        &stock,
        &[(names::ENTRY, text("lead"))],
    )
    .unwrap();
    assert_eq!(
        raised_note(&path),
        Some(
            "1 entry starts higher than the link before it ends, so the move to the entry's \
             start stays clear of the stock beside the first cut point"
        ),
        "{:?}",
        path.notices
    );
    let note = path
        .notices
        .iter()
        .find(|n| n.text.starts_with("1 entry starts higher"))
        .unwrap();
    assert_eq!(note.topic.as_deref(), Some(openchuck_path::topic::ENTRY));
    assert!(!note.is_warning());
    assert_eq!(rapid_into_stock(&fixture(), &stock, &path), None);
}

#[test]
fn req_op_14_req_op_11_a_ramp_needs_no_rise_as_its_link_ends_at_its_start() {
    // A ramp follows the pass (decision 0543): the link goes straight to the
    // ramp's start and is checked against the stock as a link, so there is
    // no move across at the link's height to raise, and no note.
    let stock = Bumpy {
        along: (21.0, 27.0),
        across: (-2.0, 2.0),
        top: 19.5,
    };
    let path = generated(
        vec![(40.0, 30.0), (20.0, 28.0)],
        &stock,
        &[
            (names::ENTRY, text("ramp")),
            (names::RAMP_ANGLE, ParameterValue::Number(10.0)),
        ],
    )
    .unwrap();
    assert_eq!(raised_note(&path), None, "{:?}", path.notices);
    assert_eq!(rapid_into_stock(&fixture(), &stock, &path), None);
}

#[test]
fn req_op_14_req_nfr_07_stock_at_the_retract_height_is_refused() {
    // Decision 0544 item 6 refuses only where even the retract height is not
    // clear. Stock that reaches it is refused before any entry is made, by
    // the clearance check of the links (OP-14), naming the clearance.
    let stock = Bumpy {
        along: (15.5, 18.5),
        across: (3.5, 6.5),
        top: 40.0,
    };
    let err = generated(
        vec![(30.0, 40.0), (20.0, 28.0)],
        &stock,
        &[(names::ENTRY, text("lead"))],
    )
    .unwrap_err();
    assert!(
        err.contains("beyond the retract height") && err.contains("enlarge the setup's clearance"),
        "{err}"
    );
}
