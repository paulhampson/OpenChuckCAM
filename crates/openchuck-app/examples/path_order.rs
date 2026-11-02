// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regenerates a project and reports how each operation's passes are
//! ordered and linked (issue #469): the time cutting and not cutting, the
//! non-cutting travel along X and in A, and the back-and-forth of the
//! passes along X (how much further the cutting passes' X centres travel
//! than the span they cover, and how often their direction along X turns).
//!
//! ```text
//! cargo run --release -p openchuck-app --example path_order -- PROJECT [--passes]
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;

use openchuck_app::{Application, Threads};
use openchuck_types::ir::MoveKind;

/// A run of cutting moves (cut, plunge, lead in and out) between other moves.
#[derive(Debug, Clone, Copy)]
struct Pass {
    x_lo: f64,
    x_hi: f64,
    a_lo: f64,
    a_hi: f64,
    length: f64,
    moves: usize,
}

impl Pass {
    fn centre(&self) -> f64 {
        0.5 * (self.x_lo + self.x_hi)
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Totals {
    time: f64,
    cut_time: f64,
    air_time: f64,
    air_x: f64,
    air_a: f64,
    air_r: f64,
    cut_len: f64,
    passes: usize,
    sweep: f64,
    span: f64,
    reversals: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: path_order PROJECT [--passes]";
    let project = PathBuf::from(args.next().ok_or(usage)?);
    let show = args.any(|a| a == "--passes");

    let app = Application::open(&project, Threads::All)?;
    let run = app.regenerate()?;
    for note in &run.notes {
        eprintln!("note: {note}");
    }
    println!("project: {}", project.display());
    println!(
        "{:<28} {:>8} {:>8} {:>8} {:>9} {:>9} {:>8} {:>6} {:>8} {:>7} {:>5}",
        "operation",
        "time s",
        "cut s",
        "air s",
        "air X mm",
        "air A deg",
        "air R mm",
        "passes",
        "X sweep",
        "X span",
        "revs"
    );
    let mut all = Totals::default();
    for (group, info) in run.toolpath.groups.iter().zip(&run.groups) {
        let name = app
            .project()
            .operation(info.operation)
            .map_or_else(|| group.tags.operation.clone(), |o| o.name.clone());
        let mut t = Totals::default();
        let mut passes: Vec<Pass> = Vec::new();
        let mut open: Option<Pass> = None;
        let mut prev = group.start;
        for s in &group.segments {
            let d = s.duration.0.max(0.0);
            t.time += d;
            let (p, q) = (prev.tip, s.end.tip);
            let rp = (p.y * p.y + p.z * p.z).sqrt();
            let rq = (q.y * q.y + q.z * q.z).sqrt();
            let cutting = !matches!(s.kind, MoveKind::Rapid | MoveKind::Linking);
            if cutting {
                t.cut_time += d;
                let len = (q - p).length();
                t.cut_len += len;
                let pass = open.get_or_insert(Pass {
                    x_lo: p.x,
                    x_hi: p.x,
                    a_lo: prev.a.0,
                    a_hi: prev.a.0,
                    length: 0.0,
                    moves: 0,
                });
                pass.x_lo = pass.x_lo.min(q.x);
                pass.x_hi = pass.x_hi.max(q.x);
                pass.a_lo = pass.a_lo.min(s.end.a.0);
                pass.a_hi = pass.a_hi.max(s.end.a.0);
                pass.length += len;
                pass.moves += 1;
            } else {
                t.air_time += d;
                t.air_x += (q.x - p.x).abs();
                t.air_a += (s.end.a.0 - prev.a.0).abs();
                t.air_r += (rq - rp).abs();
                passes.extend(open.take());
            }
            prev = s.end;
        }
        passes.extend(open.take());
        t.passes = passes.len();
        let centres: Vec<f64> = passes.iter().map(Pass::centre).collect();
        let mut dir = 0.0_f64;
        for w in centres.windows(2) {
            let dx = w[1] - w[0];
            t.sweep += dx.abs();
            if dx.abs() > 0.5 {
                if dir != 0.0 && dx.signum() != dir {
                    t.reversals += 1;
                }
                dir = dx.signum();
            }
        }
        let (lo, hi) = centres
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &c| {
                (l.min(c), h.max(c))
            });
        t.span = if centres.is_empty() { 0.0 } else { hi - lo };
        println!(
            "{:<28} {:>8.1} {:>8.1} {:>8.1} {:>9.1} {:>9.0} {:>8.1} {:>6} {:>8.1} {:>7.1} {:>5}",
            name,
            t.time,
            t.cut_time,
            t.air_time,
            t.air_x,
            t.air_a,
            t.air_r,
            t.passes,
            t.sweep,
            t.span,
            t.reversals
        );
        if show {
            for (i, p) in passes.iter().enumerate() {
                println!(
                    "    {i:>4} X {:>7.2}..{:>7.2}  A {:>8.1}..{:>8.1}  {:>8.1} mm {:>5} moves",
                    p.x_lo, p.x_hi, p.a_lo, p.a_hi, p.length, p.moves
                );
            }
        }
        all.time += t.time;
        all.cut_time += t.cut_time;
        all.air_time += t.air_time;
        all.air_x += t.air_x;
        all.air_a += t.air_a;
        all.air_r += t.air_r;
        all.passes += t.passes;
        all.sweep += t.sweep;
        all.reversals += t.reversals;
    }
    println!(
        "{:<28} {:>8.1} {:>8.1} {:>8.1} {:>9.1} {:>9.0} {:>8.1} {:>6} {:>8.1} {:>7} {:>5}",
        "TOTAL",
        all.time,
        all.cut_time,
        all.air_time,
        all.air_x,
        all.air_a,
        all.air_r,
        all.passes,
        all.sweep,
        "",
        all.reversals
    );
    Ok(())
}
