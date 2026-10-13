// SPDX-License-Identifier: AGPL-3.0-or-later
//! Helical finishing on the whole model: the first wrap-family strategy
//! (OP-04; work breakdown slice 5 item 5, completed in slice 12; decision
//! record 0018 item 12).
//!
//! A vertical tool over the rotary axis follows a helix about it: A turns
//! once every `stepover` mm of X (the pitch), so the passes are `stepover`
//! apart along the axis. In the development of the cylinder (x, s = R A) the
//! helix is a straight line, which is why the strategy needs no 2D offset of
//! its own; the region offsets of [`crate::unroll`] arrive with selections in
//! slice 12. At each point the tool is dropped onto the surface
//! ([`crate::SurfaceSource::drop_tool`]), so the cutter rides the surface
//! whatever its shape.
//!
//! **Path fitting.** The machine moves in a straight line in X, Y, Z and A
//! between two points, so the points are chosen by bisection: a move is split
//! until the tool, at the middle of the move on the straight machine path,
//! is within [`CHORD_FACTOR`] times the toolpath tolerance of where the tool
//! would be if dropped there, measured **along the surface normal** (the
//! factor settles OI-08; decision record 0018 item 12). The angle step never
//! exceeds `max_angle_step`.
//!
//! **Stock to leave** drops a tool grown by the amount
//! ([`crate::cutter::grow_profile`]), so the surface is left `stock_to_leave`
//! mm proud everywhere, and the cutter contact is taken on the original tool.
//!
//! Where the tool is over no material (past the ends of a part whose
//! surface does not reach under it) the pass is split there and the shared
//! services link the pieces.

use openchuck_types::geometry::Point3;
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::{Degrees, Millimetres};

use crate::cutter::{self, Contact};
use crate::error::{PathError, Result};
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::rotary::AxisPosition;
use crate::strategy::{CutPoint, Pass, Strategy, StrategyInput};

/// The fraction of the toolpath tolerance a move's path-fitting error may
/// use (decision record 0018 item 12, closing OI-08): 0.4 leaves room for the
/// motion between points, which SP-3 measured at 0.0056 mm for a factor of
/// 0.5 against the 0.005 mm path-fitting budget of verification plan 4.1.
pub const CHORD_FACTOR: f64 = 0.4;

/// How many times a move may be halved (the smallest step is the seed step
/// divided by 2^10).
const MAX_DEPTH: u32 = 10;

/// Most points one strategy run may make; a larger job is refused rather
/// than run out of memory (NFR-07).
const MAX_SEEDS: f64 = 5.0e6;

/// Helical finishing of the whole model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HelicalFinishing;

/// The registry name of [`HelicalFinishing`].
pub const NAME: &str = "helical_finishing";

/// Parameter names specific to helical finishing.
pub mod parameter {
    /// Which way A turns as X increases: `positive` or `negative`.
    pub const ROTATION: &str = "rotation";
    /// A at the start of the helix, degrees.
    pub const START_ANGLE: &str = "start_angle";
    /// The largest A step between points, degrees.
    pub const MAX_ANGLE_STEP: &str = "max_angle_step";
}

impl Strategy for HelicalFinishing {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Helical finishing"
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(vec![
            ParameterSpec {
                name: parameter::ROTATION.to_owned(),
                label: "Rotation".to_owned(),
                description: "Which way A turns as X increases along the helix.".to_owned(),
                kind: ParameterKind::Choice {
                    options: vec!["positive".to_owned(), "negative".to_owned()],
                    default: "positive".to_owned(),
                },
            },
            ParameterSpec {
                name: parameter::START_ANGLE.to_owned(),
                label: "Start angle".to_owned(),
                description: "A at the lowest X of the part, where the helix starts.".to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: None,
                    min_exclusive: false,
                    max: None,
                    default: Some(0.0),
                },
            },
            ParameterSpec {
                name: parameter::MAX_ANGLE_STEP.to_owned(),
                label: "Largest angle step".to_owned(),
                description: "The most A may turn between two points of the path; points are \
                              added where the surface needs them."
                    .to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: Some(0.0),
                    min_exclusive: true,
                    max: Some(90.0),
                    default: Some(5.0),
                },
            },
        ])
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let pitch = parameters.number(names::STEPOVER)?;
        let tolerance = parameters.number(names::TOLERANCE)?;
        let leave = parameters.number(names::STOCK_TO_LEAVE)?;
        let sign = if parameters.text(parameter::ROTATION)? == "negative" {
            -1.0
        } else {
            1.0
        };
        let start = parameters.number(parameter::START_ANGLE)?;
        let step = parameters.number(parameter::MAX_ANGLE_STEP)?;

        let bounds = input.surface.bounds();
        let (x0, x1) = (bounds.min.x, bounds.max.x);
        let length = x1 - x0;
        if length.is_nan() || length <= 0.0 {
            return Err(PathError::Strategy(
                "the part has no length along the rotary axis to finish".to_owned(),
            ));
        }
        let turns = length / pitch;
        let total_angle = 360.0 * turns;
        let seeds = (total_angle / step).ceil().max(1.0);
        if seeds > MAX_SEEDS {
            return Err(PathError::Parameter {
                name: names::STEPOVER.to_owned(),
                reason: format!(
                    "{pitch} mm over {length:.1} mm of part needs {seeds:.0} points at {step} \
                     degrees; increase the stepover or the largest angle step"
                ),
            });
        }
        let seeds = seeds as usize;
        let grown = cutter::grow_profile(input.tool, leave)?;
        let fit = Fit {
            input,
            drop_profile: &grown,
            leave,
            x0,
            length,
            start,
            sign,
            total_angle,
            chord: CHORD_FACTOR * tolerance,
        };

        context.report(
            "helical finishing: dropping the tool",
            0,
            Some(seeds as u64),
        );
        let ts: Vec<f64> = (0..=seeds).map(|i| i as f64 / seeds as f64).collect();
        let nodes: Vec<Option<Node>> = parallel::map_in_order(&ts, |t| fit.node(*t));
        context.check_cancelled()?;

        // Runs of consecutive nodes where the tool is over the part.
        let mut pieces: Vec<Vec<Node>> = Vec::new();
        let mut current: Vec<Node> = Vec::new();
        for node in nodes {
            match node {
                Some(n) => current.push(n),
                None => {
                    if !current.is_empty() {
                        pieces.push(std::mem::take(&mut current));
                    }
                }
            }
        }
        if !current.is_empty() {
            pieces.push(current);
        }

        context.report(
            "helical finishing: fitting the path",
            0,
            Some(pieces.len() as u64),
        );
        let mut passes = Vec::new();
        for (k, piece) in pieces.iter().enumerate() {
            context.check_cancelled()?;
            let intervals: Vec<(&Node, &Node)> = piece.iter().zip(piece.iter().skip(1)).collect();
            let refined: Vec<Vec<Node>> = parallel::map_in_order(&intervals, |(a, b)| {
                let mut out = Vec::new();
                fit.refine(a, b, 0, &mut out);
                out
            });
            let mut points = Vec::new();
            if let Some(first) = piece.first() {
                points.push(fit.cut_point(first));
            }
            for run in refined {
                points.extend(run.iter().map(|n| fit.cut_point(n)));
            }
            passes.push(Pass { points });
            context.report(
                "helical finishing: fitting the path",
                k as u64 + 1,
                Some(pieces.len() as u64),
            );
        }
        Ok(passes)
    }
}

/// A point of the helix with the tool dropped on the surface there.
#[derive(Debug, Clone, Copy)]
struct Node {
    /// Position along the helix, 0 at the start and 1 at the end.
    t: f64,
    a: Degrees,
    /// The drop with the (grown) tool.
    contact: Contact,
}

struct Fit<'a> {
    input: &'a StrategyInput<'a>,
    drop_profile: &'a RevolvedProfile,
    leave: f64,
    x0: f64,
    length: f64,
    start: f64,
    sign: f64,
    total_angle: f64,
    chord: f64,
}

impl Fit<'_> {
    fn x_at(&self, t: f64) -> f64 {
        self.x0 + self.length * t
    }

    fn a_at(&self, t: f64) -> Degrees {
        Degrees(self.start + self.sign * self.total_angle * t)
    }

    /// The tool dropped at parameter `t`, or `None` where it is over no
    /// material.
    fn node(&self, t: f64) -> Option<Node> {
        let x = self.x_at(t);
        let a = self.a_at(t);
        let line = self
            .input
            .rotary
            .tool_line(Millimetres(x), a, Millimetres(0.0));
        let contact = self.input.surface.drop_tool(self.drop_profile, &line)?;
        Some(Node { t, a, contact })
    }

    /// The original tool's tip for a grown-tool drop: it lies `leave` above
    /// the grown tip along the tool axis.
    fn tip(&self, n: &Node) -> Point3 {
        let axis = self.input.rotary.tool_axis(n.a);
        n.contact.tip + axis.get() * self.leave
    }

    /// Where the pass makes a cut.
    fn cut_point(&self, n: &Node) -> CutPoint {
        let tip = self.tip(n);
        let axis = self.input.rotary.tool_axis(n.a);
        let contact = if self.leave > 0.0 {
            cutter::cutter_contact(self.input.tool, tip, axis, n.contact.contact)
        } else {
            n.contact.contact
        };
        CutPoint {
            tip,
            a: n.a,
            contact,
        }
    }

    /// The error of the straight machine move from `a` to `b` at its middle,
    /// along the surface normal at the dropped contact.
    fn deviation(&self, a: &Node, b: &Node, mid: &Node) -> f64 {
        let rotary = &self.input.rotary;
        let (pa, pb) = (
            rotary.to_axis(self.tip(a), a.a),
            rotary.to_axis(self.tip(b), b.a),
        );
        let lin = AxisPosition {
            along: 0.5 * (pa.along + pb.along),
            across: 0.5 * (pa.across + pb.across),
            height: 0.5 * (pa.height + pb.height),
        };
        let lin_tip = rotary.to_model(lin, mid.a);
        let ideal = self.tip(mid);
        let delta = ideal - lin_tip;
        match self.input.surface.project(mid.contact.contact) {
            Some(sp) => delta.dot(sp.normal.get()).abs(),
            None => delta.length(),
        }
    }

    /// Bisects the move `a` to `b` until the straight machine path is within
    /// the chord tolerance at its middle; pushes the points after `a`, ending
    /// with `b`. Where the middle has no material under it (the surface falls
    /// away between two points that touch it) the move is accepted as it is.
    fn refine(&self, a: &Node, b: &Node, depth: u32, out: &mut Vec<Node>) {
        if depth < MAX_DEPTH
            && let Some(mid) = self.node(0.5 * (a.t + b.t))
            && self.deviation(a, b, &mid) > self.chord
        {
            self.refine(a, &mid, depth + 1, out);
            self.refine(&mid, b, depth + 1, out);
            return;
        }
        out.push(*b);
    }
}
