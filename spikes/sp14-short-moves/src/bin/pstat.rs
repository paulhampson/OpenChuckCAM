// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regenerates each project with both posts, one file per operation, and
//! prints block-time tables from the G-code, the IR's per-kind counts, and
//! the effect of merging. `pstat <project>...`
use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_app::{Application, OutputGrouping, PostSelection, Threads};
use openchuck_types::ir::MoveKind;
use sp14_short_moves::*;

fn main() {
    for folder in std::env::args().skip(1).map(PathBuf::from) {
        let mut app = Application::open(&folder, Threads::All).expect("project opens");
        app.set_output_grouping(OutputGrouping::Operation);
        let ops: BTreeMap<String, (String, String)> = app
            .project()
            .operations()
            .iter()
            .map(|o| (o.id.to_string(), (o.name.clone(), o.strategy.clone())))
            .collect();
        println!("\n## {}", folder.display());
        for post in ["makera-z1", "linuxcnc"] {
            app.set_post(Some(PostSelection::parse(post)));
            let t0 = std::time::Instant::now();
            let r = app.regenerate().expect("regenerates");
            eprintln!(
                "{} {post}: {:.1} s",
                folder.display(),
                t0.elapsed().as_secs_f64()
            );
            if post == "makera-z1" {
                println!("\nIR segments by kind (duration < 1/2/5/10 ms, of total):");
                for (g, info) in r.toolpath.groups.iter().zip(&r.groups) {
                    let (name, strategy) = ops
                        .get(&info.operation.to_string())
                        .cloned()
                        .unwrap_or_default();
                    let mut by: BTreeMap<String, [usize; 5]> = BTreeMap::new();
                    for s in &g.segments {
                        if s.kind == MoveKind::Rapid {
                            continue;
                        }
                        let e = by.entry(format!("{:?}", s.kind)).or_default();
                        e[4] += 1;
                        for (k, lim) in LIMITS_MS.iter().enumerate() {
                            if s.duration.0 * 1000.0 < *lim {
                                e[k] += 1;
                            }
                        }
                    }
                    for (kind, c) in by {
                        println!(
                            "  {name} ({strategy}) {kind}: {} / {} / {} / {} of {}",
                            c[0], c[1], c[2], c[3], c[4]
                        );
                    }
                }
            }
            println!("\npost {post}: file | {}", header());
            for (file, text) in &r.gcode {
                if let Ok(dir) = std::env::var("SP14_OUT") {
                    let d = PathBuf::from(dir).join(folder.file_name().unwrap());
                    std::fs::create_dir_all(&d).unwrap();
                    std::fs::write(d.join(file), text).unwrap();
                }
                let rule = if post == "makera-z1" {
                    Rule::Makera
                } else {
                    Rule::Linuxcnc
                };
                let b = blocks(text, rule);
                println!("{file} | {}", row(&stats(&b)));
                let mut by: BTreeMap<Class, (usize, usize)> = BTreeMap::new();
                for x in &b {
                    let e = by.entry(x.class).or_default();
                    e.1 += 1;
                    if x.class != Class::Rapid && x.t < 0.005 {
                        e.0 += 1;
                    }
                }
                println!("  classes (<5 ms of n): {by:?}");
                for (label, o) in [
                    (
                        "drop zero",
                        MergeOpts {
                            tol: 0.0,
                            below: 0.0,
                            max_run: 1,
                            feed_ratio: 1.0,
                        },
                    ),
                    (
                        "merge 0.002 mm <10 ms",
                        MergeOpts {
                            tol: 0.002,
                            below: 0.010,
                            max_run: 64,
                            feed_ratio: 1.25,
                        },
                    ),
                    (
                        "merge 0.005 mm <10 ms",
                        MergeOpts {
                            tol: 0.005,
                            below: 0.010,
                            max_run: 64,
                            feed_ratio: 1.25,
                        },
                    ),
                    (
                        "merge 0.010 mm <10 ms",
                        MergeOpts {
                            tol: 0.010,
                            below: 0.010,
                            max_run: 64,
                            feed_ratio: 1.25,
                        },
                    ),
                    (
                        "merge 0.005 mm all",
                        MergeOpts {
                            tol: 0.005,
                            below: 0.0,
                            max_run: 64,
                            feed_ratio: 1.25,
                        },
                    ),
                ] {
                    let m = merge(&b, o);
                    println!("  {label}: {}", row(&stats(&m)));
                }
            }
        }
    }
}
