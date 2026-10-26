// SPDX-License-Identifier: AGPL-3.0-or-later
//! Successful-run medians required by decisions 0209 and 0211.

/// Return a benchmark only when at least five complete successful samples exist.
/// Refusals stay in the sample sequence as `None`; never discard them to make
/// a successful benchmark. Inputs are seconds measured on equivalent runs.
pub fn successful_median(samples: &[Option<f64>]) -> Option<f64> {
    if samples.len() < 5 {
        return None;
    }
    let mut values: Vec<f64> = samples.iter().copied().collect::<Option<_>>()?;
    if values.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    let upper = *values.get(middle)?;
    Some(if values.len().is_multiple_of(2) {
        *values.get(middle.checked_sub(1)?)? / 2.0 + upper / 2.0
    } else {
        upper
    })
}
