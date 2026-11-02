// SPDX-License-Identifier: AGPL-3.0-or-later
//! Owner-approved emitted-cut scope for legacy Facing. Original stock, field
//! range, layer arithmetic and cut history remain owned by the shared engine.

use super::{Job, Line, LineKind, MAX_NODES, SUB};
use crate::error::{PathError, Result};
use crate::facing::geometry::CoverageRectangle;
use crate::params::Parameters;
use openchuck_types::job::JobContext;

#[derive(Debug, Clone)]
pub(crate) enum FacingScope {
    /// One X/transverse rectangle for every listed A, in unchanged list order.
    Flat(Vec<CoverageRectangle>),
    /// Full-circumference Round coverage over this model-X span.
    Round((f64, f64)),
}
impl FacingScope {
    pub(super) fn validate(&self, round: bool, parameters: &Parameters) -> Result<()> {
        match (self, round) {
            (Self::Round(x), true) => positive_span(*x),
            (Self::Flat(rectangles), false) => {
                if rectangles.is_empty()
                    || rectangles.len()
                        != parameters.angles(crate::facing::parameter::ANGLES)?.len()
                {
                    return Err(refuse("Flat coverage must have one rectangle per listed A"));
                }
                for rect in rectangles {
                    positive_span((rect.min()[0], rect.max()[0]))?;
                    positive_span((rect.min()[1], rect.max()[1]))?;
                }
                Ok(())
            }
            _ => Err(refuse("coverage geometry does not match the Facing mode")),
        }
    }
}

impl Job<'_> {
    /// Intersect emitted cuts with existing held limits, leaving ORIGINAL
    /// x_range/HeightStock untouched. No whole-range fallback on an empty scope.
    pub(super) fn scoped_range(&self, requested: (f64, f64)) -> Result<(f64, f64)> {
        positive_span(requested)?;
        let mut requested = requested;
        if let Some(e) = self.extension {
            if e.start.is_some() && requested.0 > e.stock.0
                || e.end.is_some() && requested.1 < e.stock.1
            {
                return Err(refuse(
                    "selected extent does not reach a stock edge requested for extension",
                ));
            }
            // Extension is still stock edge to cutter near side. It does not
            // move to the chosen part boundary or change its distance.
            if let Some(x) = e.start {
                requested.0 = requested.0.min(x);
            }
            if let Some(x) = e.end {
                requested.1 = requested.1.max(x);
            }
        }
        let (lo, hi, _, _) = self.cut_range();
        let mut range = (lo.max(requested.0), hi.min(requested.1));
        // The legacy midpoint classification can deliberately fall back when
        // held limits meet. Scoped Facing must independently reject fully held
        // spans, including an interval that crosses the original midpoint.
        for &(a, b) in self.input.keep_out {
            if !a.is_finite() || !b.is_finite() || b < a {
                return Err(refuse("held limits must be finite and ordered"));
            }
            if b <= range.0 || a >= range.1 {
                continue;
            }
            if a <= range.0 {
                range.0 = range.0.max(b);
            } else if b >= range.1 {
                range.1 = range.1.min(a);
            } else {
                return Err(refuse(
                    "selected extent crosses an internal held interval requiring unsupported split coverage",
                ));
            }
        }
        if range.1 <= range.0 {
            return Err(refuse(
                "selected extent has no reachable axial span outside held regions",
            ));
        }
        Ok(range)
    }

    pub(super) fn scoped_flat_lines(
        &self,
        angles: &[f64],
        rectangles: &[CoverageRectangle],
        level: f64,
        context: &JobContext<'_>,
    ) -> Result<Vec<Line>> {
        let mut all = Vec::new();
        let mut nodes = 0.;
        let half = (self.r_top * self.r_top - level * level).max(0.).sqrt();
        for (&a, rect) in angles.iter().zip(rectangles) {
            context.check_cancelled()?;
            let (x0, x1) = self.scoped_range((rect.min()[0], rect.max()[0]))?;
            let (mut u0, mut u1) = (rect.min()[1], rect.max()[1]);
            if let Some(e) = self.extension
                && e.sides
            {
                if u0 > -half || u1 < half {
                    return Err(refuse(
                        "selected extent does not reach the stock sides requested for extension",
                    ));
                }
                let reach = half + e.radius + e.distance;
                u0 = u0.min(-reach);
                u1 = u1.max(reach);
            }
            let width = u1 - u0;
            let length = x1 - x0;
            let lines = (width / self.stepover).ceil().max(1.);
            let fine = (length / self.line_spacing()).ceil().max(1.) * SUB as f64;
            nodes += (lines + 1.) * (fine + 1.);
            if !nodes.is_finite() || nodes > MAX_NODES {
                return Err(refuse(
                    "scoped Flat line budget exceeded; increase stepover",
                ));
            }
            let (lines, fine) = (lines as usize, fine as usize);
            for line in 0..=lines {
                let across = if line == lines {
                    u1
                } else {
                    u0 + width * line as f64 / lines as f64
                };
                all.push(Line {
                    kind: LineKind::AlongX {
                        x0,
                        length,
                        a,
                        across,
                        reverse: false,
                    },
                    fine,
                });
            }
        }
        Ok(all)
    }
}
fn positive_span(x: (f64, f64)) -> Result<()> {
    if !x.0.is_finite() || !x.1.is_finite() || x.1 <= x.0 || !(x.1 - x.0).is_finite() {
        return Err(refuse("coverage bounds must be finite with positive width"));
    }
    Ok(())
}
fn refuse(reason: &str) -> PathError {
    PathError::Strategy(format!("Facing extent: {reason}"))
}
