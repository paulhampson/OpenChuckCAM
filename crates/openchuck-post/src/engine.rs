// SPDX-License-Identifier: AGPL-3.0-or-later
//! The template environment (decision record 0019 items 2 and 3).
//!
//! Strict undefined variables, no auto-escaping, no loader (a template cannot
//! read files), debug on so errors show the line, a recursion limit, and fuel
//! set per render in proportion to the program. The numeric surface is
//! IEEE-exact only (rounding, `sqrt`, `floor`, `trunc`) so output is the same
//! on every platform (NFR-06).

use minijinja::syntax::SyntaxConfig;
use minijinja::{AutoEscape, Environment, Error, ErrorKind, UndefinedBehavior, Value};

use crate::error::{PostError, Result};

/// Template recursion limit.
pub const RECURSION_LIMIT: usize = 64;
/// Fuel for a render that has no steps.
pub const FUEL_BASE: u64 = 100_000;
/// Fuel per step of the program (decision 0019 item 3 measured about 323).
pub const FUEL_PER_STEP: u64 = 400;

/// Fixed-decimal text: round half to even on the exact binary value, never `-0`.
pub fn fixed_str(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    match s.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
        _ => s,
    }
}

/// The fewest significant figures a feed word is written with (decision
/// record 0076): half a unit of the last of three is at most 0.5 % of the
/// value, so rounding alone cannot take a move's time past gate 2's 1 %.
pub const FEED_SIGNIFICANT_FIGURES: usize = 3;

/// A feed as [`fixed_str`] writes it with at least `decimals` places, and
/// more (up to 9) where the value would otherwise keep fewer than
/// [`FEED_SIGNIFICANT_FIGURES`] significant figures: `4.237` at one decimal
/// is `4.24`, `1200.04` stays `1200.0`. Only multiplication by a power of
/// ten is used, so the text is the same on every platform (NFR-06).
pub fn feed_str(v: f64, decimals: usize) -> String {
    // Powers of ten by repeated multiplication, exact at these sizes: `powi`
    // is not guaranteed the same everywhere.
    let power = |n: usize| (0..n).fold(1.0_f64, |p, _| p * 10.0);
    let least = power(FEED_SIGNIFICANT_FIGURES - 1);
    let mut places = decimals.min(9);
    while places < 9 && v.abs() * power(places) < least {
        places += 1;
    }
    fixed_str(v, places)
}

fn number(v: &Value, what: &str) -> std::result::Result<f64, Error> {
    let x = f64::try_from(v.clone()).map_err(|_| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("{what}: expected a number, got {}", v.kind()),
        )
    })?;
    if x.is_finite() {
        Ok(x)
    } else {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("{what}: {x} is not a finite number"),
        ))
    }
}

fn decimals(d: &Value) -> std::result::Result<usize, Error> {
    let n = i64::try_from(d.clone())
        .map_err(|_| Error::new(ErrorKind::InvalidOperation, "decimals must be an integer"))?;
    usize::try_from(n).ok().filter(|n| *n <= 9).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("decimals {n} is outside 0..=9"),
        )
    })
}

/// A fresh environment for one render of `steps` steps.
pub fn environment(steps: usize) -> Result<Environment<'static>> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_auto_escape_callback(|_| AutoEscape::None);
    let syntax = SyntaxConfig::builder()
        .trim_blocks(true)
        .lstrip_blocks(true)
        .keep_trailing_newline(true)
        .build()
        .map_err(|e| PostError::Template(e.to_string()))?;
    env.set_syntax(syntax);
    env.set_debug(true);
    let fuel = FUEL_BASE.saturating_add(FUEL_PER_STEP.saturating_mul(steps as u64));
    env.set_fuel(Some(fuel));
    env.set_recursion_limit(RECURSION_LIMIT);
    env.add_filter(
        "fixed",
        |v: Value, d: Value| -> std::result::Result<String, Error> {
            Ok(fixed_str(number(&v, "fixed")?, decimals(&d)?))
        },
    );
    env.add_filter(
        "feed",
        |v: Value, d: Value| -> std::result::Result<String, Error> {
            Ok(feed_str(number(&v, "feed")?, decimals(&d)?))
        },
    );
    env.add_filter(
        "rounded",
        |v: Value, d: Value| -> std::result::Result<f64, Error> {
            let text = fixed_str(number(&v, "rounded")?, decimals(&d)?);
            text.parse::<f64>()
                .map_err(|e| Error::new(ErrorKind::InvalidOperation, e.to_string()))
        },
    );
    env.add_filter("sqrt", |v: Value| -> std::result::Result<f64, Error> {
        let x = number(&v, "sqrt")?;
        if x < 0.0 {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "sqrt of a negative number",
            ));
        }
        Ok(x.sqrt())
    });
    env.add_filter("floor", |v: Value| -> std::result::Result<f64, Error> {
        Ok(number(&v, "floor")?.floor())
    });
    env.add_filter("trunc", |v: Value| -> std::result::Result<f64, Error> {
        Ok(number(&v, "trunc")?.trunc())
    });
    env.add_global("pi", std::f64::consts::PI);
    Ok(env)
}

/// Turns an engine error into a message naming the template and line.
pub fn describe(error: &Error) -> String {
    let line = error
        .line()
        .map_or(String::new(), |l| format!(" at line {l}"));
    let name = error.name().unwrap_or("template");
    let detail = error.detail().unwrap_or("");
    format!("{name}{line}: {} {detail}", error.kind())
        .trim_end()
        .to_string()
}
