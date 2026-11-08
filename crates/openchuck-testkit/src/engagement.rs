// SPDX-License-Identifier: AGPL-3.0-or-later
//! The independent width-of-cut check (OP-03, OP-04, OP-05, OP-09; decision
//! record 0545, issue #638).
//!
//! Used only in tests, and written separately from the strategies, the
//! drop-cutter and the stock simulation, so a strategy's engagement is not
//! graded by its own code. It shares nothing with `openchuck-path` or
//! `openchuck-sim` but the point type.
//!
//! The stock is a field of radial dexels over the unrolled cylinder: X along
//! the rotary axis, and the angle round it, each cell holding the radius of
//! the material there. A flat end mill whose axis points at the rotary axis
//! (the tool over the axis) removes, at a tool position, every cell within
//! its radius in the unrolled plane at the tip's radius that stands above
//! the tip. The **width of cut** (radial engagement, `a_e`) at a position is
//! the area removed per millimetre of path: about the stepover on a spiral
//! or a ring beside a cut ring, and the tool's diameter in a slot.
//!
//! The tool positions are the cutting points' tips in the **model frame**:
//! X is the tip's X, the angle is the tip's own angle round the X axis and
//! the radius its distance from the axis. That needs no A convention: a tip
//! that goes round the part with A goes round it in the model frame. It
//! holds for a tool over the axis; a tool standing off it (indexed passes
//! beside a position) is not what this check measures.

use std::f64::consts::TAU;

use openchuck_types::geometry::Point3;

/// One measured position along the path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidthSample {
    /// The index of the move the position lies on: move `i` runs from tip
    /// `i` to tip `i + 1` of the path given.
    pub segment: usize,
    /// Path length from the start of the path to here, mm (in the unrolled
    /// plane at the tip's radius).
    pub along: f64,
    /// The width of cut here, mm: the area removed per mm of path.
    pub width: f64,
}

/// Radial dexel stock over the unrolled cylinder, for measuring the width of
/// cut of a flat end mill over the rotary axis.
#[derive(Debug, Clone)]
pub struct UnrolledStock {
    x0: f64,
    cell: f64,
    nx: usize,
    /// Cells round the axis.
    nth: usize,
    /// The radius at which a cell is `cell` mm wide round the axis.
    reference: f64,
    /// The radius of the material in each cell, X-major.
    heights: Vec<f64>,
}

impl UnrolledStock {
    /// A stock from `x_range.0` to `x_range.1` along X with its material up
    /// to radius `top` everywhere, in square cells `cell` mm wide at radius
    /// `reference` (the cut radius to be measured at).
    ///
    /// # Panics
    ///
    /// If the range, cell, reference or top is not finite and positive where
    /// it must be, or the grid would have more than 50 million cells: this
    /// is a test helper, and a bad set-up is a bug in the test.
    #[must_use]
    pub fn new(x_range: (f64, f64), cell: f64, reference: f64, top: f64) -> Self {
        let length = x_range.1 - x_range.0;
        assert!(
            length.is_finite() && length > 0.0,
            "the stock needs a length"
        );
        assert!(cell.is_finite() && cell > 0.0, "the cell needs a size");
        assert!(
            reference.is_finite() && reference > 0.0,
            "the reference radius must be positive"
        );
        assert!(top.is_finite(), "the stock's top must be finite");
        let nx = (length / cell).ceil() as usize + 1;
        let nth = (TAU * reference / cell).round().max(8.0) as usize;
        assert!(
            (nx as f64) * (nth as f64) <= 5.0e7,
            "the grid is too fine for a test"
        );
        Self {
            x0: x_range.0,
            cell,
            nx,
            nth,
            reference,
            heights: vec![top; nx * nth],
        }
    }

    /// The radius of the material at `x` and `angle` (radians), or `None`
    /// off the stock along X.
    #[must_use]
    pub fn height_at(&self, x: f64, angle: f64) -> Option<f64> {
        let i = ((x - self.x0) / self.cell).round();
        if !(0.0..self.nx as f64).contains(&i) {
            return None;
        }
        let j = self.column(angle);
        self.heights.get(i as usize * self.nth + j).copied()
    }

    fn column(&self, angle: f64) -> usize {
        let per = TAU / self.nth as f64;
        ((angle / per).round() as i64).rem_euclid(self.nth as i64) as usize
    }

    /// Sweeps a flat end mill of radius `tool_radius` along the tips of a
    /// cutting path, in order, sampled every `step` mm of path, removing
    /// what it meets, and gives the width of cut at each sample. A cell
    /// counts as cut where it stood more than `min_depth` above the tip.
    /// Points with no length between them add no samples.
    ///
    /// # Panics
    ///
    /// If `tool_radius` or `step` is not finite and positive.
    pub fn sweep(
        &mut self,
        tool_radius: f64,
        tips: &[Point3],
        step: f64,
        min_depth: f64,
    ) -> Vec<WidthSample> {
        assert!(
            tool_radius.is_finite() && tool_radius > 0.0,
            "the tool needs a radius"
        );
        assert!(step.is_finite() && step > 0.0, "the step must be positive");
        // Each tip as (x, unwrapped angle, radius).
        let mut polar: Vec<(f64, f64, f64)> = Vec::with_capacity(tips.len());
        for p in tips {
            let angle = p.z.atan2(p.y);
            let radius = p.y.hypot(p.z);
            let angle = match polar.last() {
                Some(&(_, before, _)) => {
                    let turn = ((angle - before) / TAU).round();
                    angle - turn * TAU
                }
                None => angle,
            };
            polar.push((p.x, angle, radius));
        }
        let mut out = Vec::new();
        let mut along = 0.0;
        let mut disc: Option<(f64, Vec<(isize, isize)>)> = None;
        for (segment, (&(xa, ta, ra), &(xb, tb, rb))) in
            polar.iter().zip(polar.iter().skip(1)).enumerate()
        {
            let mid = 0.5 * (ra + rb);
            let length = (xb - xa).hypot(mid * (tb - ta));
            if length < 1e-12 {
                continue;
            }
            let n = (length / step).ceil().max(1.0) as usize;
            let ds = length / n as f64;
            for k in 1..=n {
                let u = k as f64 / n as f64;
                let (x, t, r) = (xa + (xb - xa) * u, ta + (tb - ta) * u, ra + (rb - ra) * u);
                if disc.as_ref().is_none_or(|(at, _)| (*at - r).abs() >= 1e-9) {
                    disc = Some((r, self.disc(tool_radius, r)));
                }
                let removed = disc
                    .as_ref()
                    .map_or(0, |(_, offsets)| self.cut(x, t, r, offsets, min_depth));
                along += ds;
                // A cell is `cell` mm along X and, at the tip's radius,
                // `cell * r / reference` mm round.
                let area = self.cell * self.cell * r / self.reference;
                out.push(WidthSample {
                    segment,
                    along,
                    width: removed as f64 * area / ds,
                });
            }
        }
        out
    }

    /// The cells within `tool_radius` of a tool position at radius `r`, as
    /// offsets (along X, round the axis).
    fn disc(&self, tool_radius: f64, r: f64) -> Vec<(isize, isize)> {
        let round = self.cell * r.max(1e-9) / self.reference;
        let ki = (tool_radius / self.cell).ceil() as isize + 1;
        let kj = ((tool_radius / round).ceil() as isize + 1).min(self.nth as isize / 2);
        let mut cells = Vec::new();
        for di in -ki..=ki {
            for dj in -kj..=kj {
                let (ex, ey) = (di as f64 * self.cell, dj as f64 * round);
                if ex * ex + ey * ey <= tool_radius * tool_radius {
                    cells.push((di, dj));
                }
            }
        }
        cells
    }

    fn cut(&mut self, x: f64, angle: f64, r: f64, cells: &[(isize, isize)], min: f64) -> usize {
        let i0 = ((x - self.x0) / self.cell).round() as isize;
        let j0 = self.column(angle) as isize;
        let nth = self.nth as isize;
        let mut removed = 0;
        for &(di, dj) in cells {
            let i = i0 + di;
            if i < 0 || i >= self.nx as isize {
                continue;
            }
            let j = (j0 + dj).rem_euclid(nth);
            let Some(h) = self.heights.get_mut(i as usize * self.nth + j as usize) else {
                continue;
            };
            if *h > r + min {
                removed += 1;
                *h = r;
            }
        }
        removed
    }
}

/// The widths of `samples` averaged over a window of `window` samples
/// centred on each, as the grid removes a whole column of cells at once
/// when the tool crosses a cell boundary. Ends use the samples there are.
#[must_use]
pub fn smoothed(samples: &[WidthSample], window: usize) -> Vec<WidthSample> {
    let w = window.max(1);
    let half = w / 2;
    let mut prefix = Vec::with_capacity(samples.len() + 1);
    prefix.push(0.0);
    for s in samples {
        prefix.push(prefix.last().copied().unwrap_or(0.0) + s.width);
    }
    samples
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let lo = n.saturating_sub(half);
            let hi = (n + w - half).min(samples.len());
            WidthSample {
                width: (prefix.get(hi).copied().unwrap_or(0.0)
                    - prefix.get(lo).copied().unwrap_or(0.0))
                    / (hi - lo) as f64,
                ..*s
            }
        })
        .collect()
}

/// The largest and the median of `widths`, or `None` for none.
#[must_use]
pub fn peak_and_median(widths: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    let mut v: Vec<f64> = widths.into_iter().collect();
    v.sort_by(f64::total_cmp);
    let median = v.get(v.len() / 2).copied()?;
    v.last().map(|&peak| (peak, median))
}

#[cfg(test)]
mod tests {
    //! The check against the issue #638 model (decision record 0545,
    //! Context): one roughing layer, a 3.175 mm flat end mill at a 1.27 mm
    //! stepover and a 10 mm cut radius over nine rings, the first turn left
    //! out because it is a slot on every path.
    use super::{UnrolledStock, WidthSample, peak_and_median, smoothed};
    use openchuck_types::geometry::Point3;
    use std::f64::consts::TAU;

    const R: f64 = 1.5875;
    const S: f64 = 1.27;
    const RADIUS: f64 = 10.0;
    const RINGS: usize = 9;
    const CELL: f64 = 0.02;
    const STEP: f64 = 0.02;

    fn at(x: f64, degrees: f64) -> Point3 {
        let t = degrees.to_radians();
        Point3::new(x, RADIUS * t.cos(), RADIUS * t.sin())
    }

    /// A ring path, each ring a whole turn in one-degree moves, then a step
    /// to the next ring over `blend` degrees (0 for a sharp step). Gives the
    /// tips and which moves are steps.
    fn rings(blend: f64) -> (Vec<Point3>, Vec<bool>) {
        let mut tips = vec![at(0.0, 0.0)];
        let mut step = Vec::new();
        let mut a = 0.0;
        for k in 0..RINGS {
            let x = k as f64 * S;
            for _ in 0..360 {
                a += 1.0;
                tips.push(at(x, a));
                step.push(false);
            }
            if k + 1 < RINGS {
                let pieces = blend.ceil().max(1.0) as usize;
                for p in 1..=pieces {
                    let u = p as f64 / pieces as f64;
                    tips.push(at(x + S * u, a + blend * u));
                    step.push(true);
                }
                a += blend;
            }
        }
        (tips, step)
    }

    fn spiral() -> Vec<Point3> {
        let mut tips: Vec<Point3> = (0..=360).map(|d| at(0.0, f64::from(d))).collect();
        let turns = (RINGS - 1) as f64;
        let n = (360.0 * turns) as usize;
        for d in 1..=n {
            let a = 360.0 + d as f64;
            tips.push(at(S * d as f64 / 360.0, a));
        }
        tips
    }

    fn measure(tips: &[Point3]) -> Vec<WidthSample> {
        let length = (RINGS - 1) as f64 * S;
        let mut stock =
            UnrolledStock::new((-R - 1.0, length + R + 1.0), CELL, RADIUS, RADIUS + 1.0);
        let raw = stock.sweep(R, tips, STEP, 1e-9);
        let window = (0.5 / STEP) as usize;
        let skip = TAU * RADIUS + 0.5;
        smoothed(&raw, window)
            .into_iter()
            .filter(|s| s.along > skip)
            .collect()
    }

    fn split(samples: &[WidthSample], step: &[bool]) -> ((f64, f64), (f64, f64)) {
        let ring = peak_and_median(samples.iter().filter(|s| !step[s.segment]).map(|s| s.width));
        let steps = peak_and_median(samples.iter().filter(|s| step[s.segment]).map(|s| s.width));
        (ring.expect("ring samples"), steps.expect("step samples"))
    }

    #[test]
    fn req_op_03_the_width_of_cut_on_a_spiral_is_the_stepover() {
        let samples = measure(&spiral());
        let (peak, median) = peak_and_median(samples.iter().map(|s| s.width)).expect("samples");
        assert!((median - S).abs() < 0.06, "median {median}");
        assert!(peak < S + 0.1, "peak {peak}");
    }

    #[test]
    fn req_op_03_a_sharp_ring_step_is_a_slot() {
        // The model gives 2.45 mm median and 2.95 mm peak on the step.
        let (tips, step) = rings(0.0);
        let ((_, ring_median), (peak, median)) = split(&measure(&tips), &step);
        assert!((ring_median - S).abs() < 0.06, "ring median {ring_median}");
        assert!(peak > 2.8 && peak <= 2.0 * R + 0.05, "step peak {peak}");
        assert!(median > 2.2, "step median {median}");
    }

    #[test]
    fn req_op_03_a_blend_over_45_degrees_keeps_near_the_stepover() {
        // The model gives 0.86 mm median and 1.52 mm peak on the blend, and
        // 1.41 mm the peak on the rings.
        let (tips, step) = rings(45.0);
        let ((ring_peak, ring_median), (peak, median)) = split(&measure(&tips), &step);
        assert!((ring_median - S).abs() < 0.06, "ring median {ring_median}");
        assert!(ring_peak < 1.5, "ring peak {ring_peak}");
        assert!(peak < 1.6, "blend peak {peak}");
        assert!(median < 1.0, "blend median {median}");
    }

    #[test]
    fn req_op_03_nothing_left_to_cut_is_no_width() {
        let mut stock = UnrolledStock::new((0.0, 5.0), 0.05, RADIUS, RADIUS);
        let tips = [at(2.0, 0.0), at(2.0, 90.0)];
        let samples = stock.sweep(R, &tips, 0.05, 1e-9);
        assert!(!samples.is_empty());
        assert!(samples.iter().all(|s| s.width == 0.0));
        assert_eq!(stock.height_at(2.0, 0.5), Some(RADIUS));
        assert_eq!(stock.height_at(-1.0, 0.0), None);
    }

    #[test]
    fn req_op_03_a_cut_lowers_the_stock_to_the_tip() {
        let mut stock = UnrolledStock::new((0.0, 5.0), 0.05, RADIUS, RADIUS + 1.0);
        let samples = stock.sweep(R, &[at(2.0, 0.0), at(2.0, 0.0), at(2.0, 30.0)], 0.05, 1e-9);
        // The repeated point adds no samples; every sample is on move 1.
        assert!(samples.iter().all(|s| s.segment == 1));
        // A slot: the first samples after the plunge cut the full diameter.
        let (peak, median) = peak_and_median(samples.iter().map(|s| s.width)).expect("samples");
        assert!((median - 2.0 * R).abs() < 0.1, "median {median}");
        assert!(peak > median);
        assert_eq!(stock.height_at(2.0, 15f64.to_radians()), Some(RADIUS));
        assert_eq!(stock.height_at(4.5, 15f64.to_radians()), Some(RADIUS + 1.0));
        assert_eq!(peak_and_median(std::iter::empty()), None);
    }
}
