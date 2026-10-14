// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mutation run for the STEP importer (NFR-07; decision record 0003; open item OI-27).
//!
//! `cargo run -p openchuck-import --features kernel --example step_mutations -- <count> [seed]`
//!
//! The OpenCASCADE reader segfaults on damaged files, which is why the
//! importer runs it in a child process. This takes the valid corpus files,
//! damages them at random (bit flips, deletions, duplicated runs, changed
//! digits), imports each mutant through the importer, and counts how it ended.
//! The application process must survive every one: a panic here, or a mutant
//! the importer accepts but whose model fails validation, is a failure (exit
//! code 1). Worker crashes and timeouts are expected outcomes; they are
//! reported as errors, never as a crash of this process. Mutants that crashed
//! or hung the worker are kept in `target/step-mutations` for study.
//!
//! The generator is a fixed xorshift, so the same seed gives the same mutants.

// A test program, not library code: it reports on the terminal and indexes its own fixed tables.
#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use openchuck_import::step::StepImporter;
use openchuck_import::{ImportError, ImportOptions, ModelImporter};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate(data: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut b = data.to_vec();
    let edits = [1, 3, 10][rng.below(3)];
    for _ in 0..edits {
        if b.is_empty() {
            break;
        }
        let k = rng.below(b.len());
        match rng.below(4) {
            0 => b[k] ^= 1 << rng.below(8),
            1 => {
                let end = (k + 1 + rng.below(40)).min(b.len());
                b.drain(k..end);
            }
            2 => {
                let end = (k + 1 + rng.below(40)).min(b.len());
                let run = b[k..end].to_vec();
                b.splice(k..k, run);
            }
            _ => b[k] = b"0123456789#.,;()"[rng.below(16)],
        }
    }
    b
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/step/valid")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let count: usize = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(100);
    let seed: u64 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(1).max(1);
    let seeds: Vec<(String, Vec<u8>)> = ["cylinder", "box", "filleted_box", "rotary_blank_drilled"]
        .iter()
        .filter_map(|n| {
            let path = corpus_dir().join(format!("{n}.step"));
            std::fs::read(&path).ok().map(|d| ((*n).to_owned(), d))
        })
        .collect();
    if seeds.is_empty() {
        eprintln!("no corpus files found in {}", corpus_dir().display());
        return ExitCode::from(2);
    }
    let importer = StepImporter::new().with_timeout(Duration::from_secs(30));
    if importer.locate_worker().is_none() {
        eprintln!("the worker program was not found: build it with --features kernel --bins");
        return ExitCode::from(2);
    }
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/step-mutations");
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut outcomes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut failed = false;
    for i in 0..count {
        let (name, data) = &seeds[rng.below(seeds.len())];
        let mutant = mutate(data, &mut rng);
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        let context = JobContext::new(&cancel, &progress);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            importer.import(&mutant, &ImportOptions::default(), &context)
        }));
        let key = match result {
            Ok(Ok(_)) => "read",
            Ok(Err(ImportError::Invalid { .. })) => "clean error",
            Ok(Err(ImportError::Worker(reason))) => {
                if reason.contains("did not finish") {
                    save(&out_dir, name, i, &mutant);
                    "worker timed out"
                } else {
                    save(&out_dir, name, i, &mutant);
                    "worker crashed"
                }
            }
            Ok(Err(_)) => "other error",
            Err(_) => {
                eprintln!("PANIC in the importer on mutant {i} of {name}");
                save(&out_dir, name, i, &mutant);
                failed = true;
                "panic"
            }
        };
        *outcomes.entry(key).or_default() += 1;
    }
    println!("{count} mutants (seed {seed}):");
    for (key, n) in &outcomes {
        println!("  {key}: {n}");
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn save(dir: &Path, name: &str, i: usize, data: &[u8]) {
    if std::fs::create_dir_all(dir).is_ok() {
        let _ = std::fs::write(dir.join(format!("mutant_{name}_{i}.step")), data);
    }
}
