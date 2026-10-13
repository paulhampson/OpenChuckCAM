//! The engine wrapper under test: minijinja 3.0.0 configured the way a `PostProcessor`
//! implementation would configure it. Everything here is engine-generic: no axis letter, no
//! controller name (GC-04: machine conventions live in the post definition, not the engine).

use minijinja::syntax::SyntaxConfig;
use minijinja::{AutoEscape, Environment, Error, ErrorKind, UndefinedBehavior, Value};

/// Instruction budget per render (NFR-07: a user template cannot hang the application).
pub const FUEL: u64 = 50_000_000;
pub const RECURSION_LIMIT: usize = 64;

/// Fixed-decimal text for a number: round half to even on the exact binary value (Rust's
/// `format!` is exact), and never "-0.000".
pub fn fixed_str(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    match s.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
        _ => s,
    }
}

fn number(v: &Value, what: &str) -> Result<f64, Error> {
    let x = f64::try_from(v.clone()).map_err(|_| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("{what}: expected a number, got {}", v.kind()),
        )
    })?;
    if !x.is_finite() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("{what}: {x} is not a finite number"),
        ));
    }
    Ok(x)
}

fn decimals(d: &Value) -> Result<usize, Error> {
    let n = i64::try_from(d.clone())
        .map_err(|_| Error::new(ErrorKind::InvalidOperation, "decimals must be an integer"))?;
    if !(0..=9).contains(&n) {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("decimals {n} is outside 0..=9"),
        ));
    }
    Ok(n as usize)
}

pub fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    // A misspelt name is an error that names the line, never silent empty output.
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    // G-code is not HTML: never escape `<`, `&` in comments.
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.set_syntax(
        SyntaxConfig::builder()
            .trim_blocks(true)
            .lstrip_blocks(true)
            .keep_trailing_newline(true)
            .build()
            .unwrap(),
    );
    // Source context and referenced values in errors (off by default in release builds).
    env.set_debug(true);
    env.set_fuel(Some(FUEL));
    env.set_recursion_limit(RECURSION_LIMIT);
    // Engine-generic numeric helpers: the whole numeric surface beyond the built-ins. Every one is
    // IEEE-exact (rounding, sqrt, floor, trunc), so results agree across platforms (NFR-06). No
    // `hypot`, `sin` or `cos`: those go through the platform's maths library.
    env.add_filter("fixed", |v: Value, d: Value| -> Result<String, Error> {
        Ok(fixed_str(number(&v, "fixed")?, decimals(&d)?))
    });
    env.add_filter("rounded", |v: Value, d: Value| -> Result<f64, Error> {
        Ok(fixed_str(number(&v, "rounded")?, decimals(&d)?)
            .parse::<f64>()
            .unwrap())
    });
    env.add_filter("sqrt", |v: Value| -> Result<f64, Error> {
        let x = number(&v, "sqrt")?;
        if x < 0.0 {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "sqrt of a negative number",
            ));
        }
        Ok(x.sqrt())
    });
    env.add_filter("floor", |v: Value| -> Result<f64, Error> {
        Ok(number(&v, "floor")?.floor())
    });
    env.add_filter("trunc", |v: Value| -> Result<f64, Error> {
        Ok(number(&v, "trunc")?.trunc())
    });
    env.add_global("pi", std::f64::consts::PI);
    env
}

/// Render a named template source with a serialisable context.
pub fn render(
    env: &mut Environment<'static>,
    name: &str,
    source: &str,
    ctx: &impl serde::Serialize,
) -> Result<String, Error> {
    env.add_template_owned(name.to_string(), source.to_string())?;
    env.get_template(name)?.render(minijinja::value::Serde(ctx))
}
