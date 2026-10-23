// SPDX-License-Identifier: AGPL-3.0-or-later
//! M4 must fail when determinism, replay, or application equivalence fails.

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
