// SPDX-License-Identifier: AGPL-3.0-or-later
//! The determinism harness (NFR-06, GC-07, work breakdown slice 0 item 9).
//!
//! Runs the same job on rayon pools of 1, 2 and N threads, and twice at N,
//! and compares a BLAKE3 hash of each output. The acceptance limit is zero
//! difference, byte for byte (verification plan section 4.1).
//!
//! Rules that keep parallel code deterministic (architecture section 8):
//!
//! - Chunk work by a fixed size that does not depend on the thread count
//!   (never on `rayon::current_num_threads()`).
//! - Collect results in input order (`par_iter().map().collect()`), never by
//!   pushing to a shared container from several threads.
//! - Do not reduce floating-point values in parallel in an order that depends
//!   on scheduling; reduce per fixed chunk, then sequentially across chunks.
//! - Use ordered containers (`BTreeMap`, `BTreeSet`) on any path that affects
//!   output; clippy denies `HashMap` and `HashSet` by default.
//! - Write numbers with fixed formatting.

/// One run of the job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Worker threads in the pool.
    pub threads: usize,
    /// BLAKE3 hash of the output, in hex.
    pub hash: String,
}

/// Thread counts the harness uses: 1, 2 and N, where N is the larger of the
/// available parallelism and 4, so the N run differs from the others even on
/// a small CI runner.
pub fn thread_counts() -> [usize; 3] {
    let available = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    [1, 2, available.max(4)]
}

/// Runs `job` at 1, 2 and N threads and twice at N. Returns the runs if every
/// output is identical, or a message listing the hashes if not.
pub fn check<F>(job: F) -> Result<Vec<Run>, String>
where
    F: Fn() -> Vec<u8> + Sync,
{
    let [one, two, many] = thread_counts();
    let mut runs = Vec::new();
    for threads in [one, two, many, many] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|e| format!("cannot build a pool of {threads} threads: {e}"))?;
        let output = pool.install(&job);
        runs.push(Run {
            threads,
            hash: blake3::hash(&output).to_hex().to_string(),
        });
    }
    let first = runs.first().map(|r| r.hash.clone()).unwrap_or_default();
    if runs.iter().all(|r| r.hash == first) {
        Ok(runs)
    } else {
        let detail: Vec<String> = runs
            .iter()
            .map(|r| format!("{} threads: {}", r.threads, r.hash))
            .collect();
        Err(format!(
            "output depends on the thread count or scheduling (NFR-06):\n  {}",
            detail.join("\n  ")
        ))
    }
}
