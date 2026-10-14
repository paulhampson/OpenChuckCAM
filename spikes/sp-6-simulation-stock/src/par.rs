// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deterministic parallel map with std threads: fixed contiguous chunks,
//! results collected in input order. Each item is computed alone, so the
//! result never depends on the thread count.

pub fn map_items<T: Send, F: Fn(usize) -> T + Sync>(n: usize, threads: usize, f: F) -> Vec<T> {
    let threads = threads.max(1).min(n.max(1));
    let chunk = n.div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|k| {
                let f = &f;
                s.spawn(move || {
                    ((k * chunk)..((k + 1) * chunk).min(n))
                        .map(f)
                        .collect::<Vec<T>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("worker panicked"))
            .collect()
    })
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}
