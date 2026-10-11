// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-5 throwaway spike: does a G-code reader handle A words and the LinuxCNC dialect?
//! `cargo run --release` prints the measurements recorded in RESULT.md.

mod fuzz;
mod interp;
mod model;
mod oracle;
mod readers;
#[cfg(test)]
mod tests;

use model::Reader;
use std::time::Instant;

pub const FIXTURES: &[(&str, &str)] = &[
    ("sp3_g94.ngc", include_str!("../fixtures/sp3_g94.ngc")),
    ("sp3_g93.ngc", include_str!("../fixtures/sp3_g93.ngc")),
    ("sp3_naive.ngc", include_str!("../fixtures/sp3_naive.ngc")),
    ("linuxcnc_post_subset.ngc", include_str!("../fixtures/linuxcnc_post_subset.ngc")),
    ("linuxcnc_dialect_extra.ngc", include_str!("../fixtures/linuxcnc_dialect_extra.ngc")),
];

pub fn readers() -> Vec<Box<dyn Reader>> {
    vec![
        Box::new(readers::GcodeCrate { f64_layer: false }),
        Box::new(readers::GcodeCrate { f64_layer: true }),
        Box::new(readers::GCodeCrate),
        Box::new(readers::InHouse),
    ]
}

fn main() {
    let rs = readers();
    println!("## Fixtures\n");
    for (name, src) in FIXTURES {
        let truth = oracle::split(src);
        println!("### {name} ({} lines)", src.lines().count());
        for r in &rs {
            match r.read(src) {
                Ok(p) => {
                    let words: Vec<_> = p.words().cloned().collect();
                    let a_words = words.iter().filter(|w| w.letter == 'A').count();
                    let max_err = words.iter().map(|w| (w.value - w.text.parse::<f64>().unwrap_or(f64::NAN)).abs()).fold(0.0, f64::max);
                    let vs_truth = match &truth {
                        Some(t) => format!("matches split oracle: {}", oracle::compare(t, &words).map_or_else(|e| format!("NO ({e})"), |()| "yes".into())),
                        None => "no oracle".into(),
                    };
                    let rep = interp::replay(&p).map_or_else(|e| format!("replay error: {e}"), |m| format!("{} moves", m.len()));
                    println!(
                        "- {}: ok, {} blocks, {} words ({a_words} A), {} comments, max |value - text| = {max_err:.3e}, {vs_truth}, {rep}",
                        r.name(),
                        p.blocks.len(),
                        words.len(),
                        p.comment_count()
                    );
                }
                Err(e) => println!("- {}: error ({} diagnostics), first: {}", r.name(), e.len(), e[0]),
            }
        }
        println!();
    }

    println!("## Replay agreement on the SP-3 outputs\n");
    for (name, src) in FIXTURES.iter().take(4) {
        let runs: Vec<_> = rs.iter().map(|r| (r.name(), r.read(src).ok().and_then(|p| interp::replay(&p).ok()))).collect();
        let base = runs.iter().find(|(n, _)| *n == "in-house subset reader").and_then(|(_, m)| m.clone());
        for (n, m) in &runs {
            let d = match (m, &base) {
                (Some(m), Some(b)) if m.len() == b.len() => {
                    let max = m.iter().zip(b).flat_map(|(x, y)| (0..4).map(move |i| (x.to[i] - y.to[i]).abs())).fold(0.0, f64::max);
                    let fmax = m.iter().zip(b).map(|(x, y)| (x.feed.unwrap_or(0.0) - y.feed.unwrap_or(0.0)).abs()).fold(0.0, f64::max);
                    format!("max position difference {max:.3e}, max feed difference {fmax:.3e}")
                }
                _ => "not comparable".into(),
            };
            println!("- {name}, {n}: {d}");
        }
    }

    println!("\n## Precision on stress values\n");
    let stress = oracle::stress_program();
    for r in &rs {
        match r.read(&stress) {
            Ok(p) => {
                let worst = p
                    .words()
                    .map(|w| ((w.value - w.text.parse::<f64>().unwrap_or(f64::NAN)).abs(), format!("{}{}", w.letter, w.text)))
                    .fold((0.0, String::new()), |a, b| if b.0 > a.0 { b } else { a });
                println!("- {}: worst |value - text| = {:.3e} at {}", r.name(), worst.0, worst.1);
            }
            Err(e) => println!("- {}: error {}", r.name(), e[0]),
        }
    }

    println!("\n## Malformed input\n");
    for (what, src) in oracle::malformed() {
        println!("- `{}` ({what})", src.replace('\n', "\\n"));
        for r in &rs {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.read(src)));
            let s = match res {
                Err(_) => "PANIC".to_string(),
                Ok(Ok(p)) => format!("accepted: {:?}", p.words().map(|w| format!("{}{}", w.letter, w.value)).collect::<Vec<_>>()),
                Ok(Err(e)) => format!("error at {}", e[0]),
            };
            println!("  - {}: {s}", r.name());
        }
    }

    println!("\n## Robustness (mutation fuzz, no panics allowed: NFR-07)\n");
    std::panic::set_hook(Box::new(|_| {}));
    for r in &rs {
        let f = fuzz::run(r.as_ref(), 20_000, 0x5eed);
        println!("- {}: {} inputs, {} panics, {} errors{}", r.name(), f.cases, f.panics, f.errors, f.example.map_or(String::new(), |e| format!(", e.g. `{e}`")));
    }
    let _ = std::panic::take_hook();

    println!("\n## LinuxCNC dialect features beyond the post subset, line by line\n");
    for line in include_str!("../fixtures/linuxcnc_dialect_extra.ngc").lines().filter(|l| !l.starts_with('(')) {
        println!("- `{line}`");
        for r in &rs {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.read(line)));
            let s = match res {
                Err(_) => "PANIC".to_string(),
                Ok(Ok(p)) => format!("accepted: {:?}", p.words().map(|w| format!("{}{}", w.letter, w.value)).collect::<Vec<_>>()),
                Ok(Err(e)) => format!("error at {}", e[0]),
            };
            println!("  - {}: {s}", r.name());
        }
    }

    println!("\n## Speed (1,000,000-line SP-3-style program, release build)\n");
    let big = oracle::big_program(1_000_000);
    println!("- input {:.1} MB", big.len() as f64 / 1e6);
    for r in &rs {
        // Best of three, so allocation warm-up does not favour later readers.
        let mut dt = std::time::Duration::MAX;
        let mut res = Err(Vec::new());
        for _ in 0..3 {
            let t = Instant::now();
            res = r.read(&big);
            dt = dt.min(t.elapsed());
        }
        let n = res.as_ref().map_or(0, |p| p.blocks.len());
        println!("- {}: {:.3} s, {} blocks{}", r.name(), dt.as_secs_f64(), n, if res.is_err() { " (error)" } else { "" });
    }
}
