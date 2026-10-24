// SPDX-License-Identifier: AGPL-3.0-or-later
//! Measurements must reject failed equivalence, replay and sampling criteria.

pub fn identical<T: PartialEq + ?Sized>(
    expected: &T,
    actual: &T,
    context: &str,
) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!("M4 failed: {context} differs"))
    }
}

pub fn clean(is_clean: bool, context: &str) -> Result<(), String> {
    if is_clean {
        Ok(())
    } else {
        Err(format!("M4 failed: {context} is not clean"))
    }
}

/// A bound on clearance error, not nominal sample spacing. Decision 0209 M2
/// requires sampling to add strictly less than 0.001 mm for every cutter used.
pub fn sampling_bound(proven_error_mm: Option<f64>) -> Result<(), String> {
    if proven_error_mm.is_some_and(|e| e.is_finite() && (0.0..0.001).contains(&e)) {
        Ok(())
    } else {
        Err("M2 unqualified: physical sampling error must be proven below 0.001 mm; the current nominal grid has no such bound (decision 0211)".to_owned())
    }
}
