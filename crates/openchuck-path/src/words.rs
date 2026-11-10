// SPDX-License-Identifier: AGPL-3.0-or-later
//! The words of a user-visible text that depend on a number (decision
//! record 0644, after OWNERCHECKS2 read "in 1 layers of 1 mm").
//!
//! Every text that prints a count that can be one writes it with
//! [`counted`], so one layer reads "1 layer" and three read "3 layers";
//! `cargo xtask messages` counts the texts that put a placeholder straight
//! before a plural noun instead.

/// `n` and the noun that agrees with it: `one` when `n` is 1, `many`
/// otherwise ("0 layers", "1 layer", "3 layers").
#[must_use]
pub fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_12_a_count_of_one_takes_the_singular() {
        assert_eq!(counted(1, "layer", "layers"), "1 layer");
        assert_eq!(counted(0, "layer", "layers"), "0 layers");
        assert_eq!(counted(2, "layer", "layers"), "2 layers");
        assert_eq!(counted(1, "pass", "passes"), "1 pass");
        assert_eq!(counted(3, "entry", "entries"), "3 entries");
    }
}
