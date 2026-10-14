// SPDX-License-Identifier: AGPL-3.0-or-later
//! The determinism harness catches scheduling-dependent work (NFR-06, GC-07;
//! work breakdown slice 0 review checklist).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_testkit::determinism;
use rayon::prelude::*;

fn values() -> Vec<f64> {
    (0..100_000)
        .map(|i| (f64::from(i) * 0.731).sin() * 1e3 + 1e-3 * f64::from(i))
        .collect()
}

#[test]
fn req_nfr_06_gc_07_fixed_chunking_is_deterministic() {
    let data = values();
    let runs = determinism::check(|| {
        // Fixed chunk size, ordered collection, sequential final reduction.
        let partial: Vec<f64> = data
            .par_chunks(1024)
            .map(|c| c.iter().sum::<f64>())
            .collect();
        partial.iter().sum::<f64>().to_le_bytes().to_vec()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
    assert_eq!(runs[0].threads, 1);
}

#[test]
fn req_nfr_06_gc_07_thread_dependent_chunking_is_caught() {
    let data = values();
    let result = determinism::check(|| {
        // The mistake the rules forbid: chunk size from the thread count, so
        // the floating-point sum is grouped differently at each count.
        let chunk = data.len().div_ceil(rayon::current_num_threads());
        let partial: Vec<f64> = data
            .par_chunks(chunk)
            .map(|c| c.iter().sum::<f64>())
            .collect();
        partial.iter().sum::<f64>().to_le_bytes().to_vec()
    });
    let message = result.unwrap_err();
    assert!(message.contains("NFR-06"), "{message}");
}

#[test]
fn req_nfr_06_gc_07_shared_push_order_is_caught() {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    let result = determinism::check(|| {
        // Results pushed to a shared container in completion order.
        let out = Mutex::new(Vec::new());
        let pushed = AtomicUsize::new(0);
        (0..64u32).into_par_iter().for_each(|i| {
            if i == 0 {
                // Item 0 finishes last whenever another thread is working: it
                // waits until something else has been pushed. This makes the
                // order depend on the thread count without relying on how
                // long each item takes, which a slow-starting thread on a
                // loaded machine can defeat. At one thread nothing else can
                // run, so the wait gives up (the limit only bounds that run).
                let start = Instant::now();
                while pushed.load(Ordering::Acquire) == 0
                    && start.elapsed() < Duration::from_millis(500)
                {
                    std::thread::yield_now();
                }
            }
            out.lock().unwrap().push(i);
            pushed.fetch_add(1, Ordering::Release);
        });
        let out = out.into_inner().unwrap();
        out.iter().map(|i| *i as u8).collect()
    });
    // At one thread the order is 0..64; with two or more, item 0 is pushed
    // after at least one other item.
    assert!(result.is_err());
}
