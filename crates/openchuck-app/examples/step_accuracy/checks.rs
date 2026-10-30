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

/// Two serialized paths that agree in structure (the same keys, lengths,
/// strings and flags) and whose numbers differ by at most `tolerance`: the
/// largest difference, or what differs (OI-1110, decision 0277).
pub fn within_rounding(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    tolerance: f64,
    context: &str,
) -> Result<f64, String> {
    use serde_json::Value;
    // The largest numeric difference and where it is.
    fn walk(a: &Value, b: &Value, at: &str, largest: &mut (f64, String)) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
                (Some(x), Some(y)) => {
                    if (x - y).abs() > largest.0 {
                        *largest = ((x - y).abs(), at.to_owned());
                    }
                    true
                }
                _ => x == y,
            },
            (Value::Array(x), Value::Array(y)) => {
                x.len() == y.len()
                    && x.iter()
                        .zip(y)
                        .enumerate()
                        .all(|(i, (p, q))| walk(p, q, &format!("{at}/{i}"), largest))
            }
            (Value::Object(x), Value::Object(y)) => {
                x.len() == y.len()
                    && x.iter().all(|(k, p)| {
                        y.get(k)
                            .is_some_and(|q| walk(p, q, &format!("{at}/{k}"), largest))
                    })
            }
            _ => a == b,
        }
    }
    let mut largest = (0.0_f64, String::new());
    if walk(expected, actual, "", &mut largest) && largest.0 <= tolerance {
        Ok(largest.0)
    } else {
        Err(format!(
            "M4 failed: {context} differs (largest numeric difference {:e} at {}, tolerance {tolerance:e})",
            largest.0, largest.1
        ))
    }
}

/// "yes" when `value` is a number no larger than `limit`, "NO" otherwise
/// (not a number included): M2's verdict on a cut or a gap (lane
/// STEPALL-X2).
pub fn verdict(value: f64, limit: f64) -> &'static str {
    if value <= limit { "yes" } else { "NO" }
}
