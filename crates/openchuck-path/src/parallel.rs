// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deterministic parallel maps (NFR-06; architecture section 8; decision
//! records 0012 and 0015 item 4).
//!
//! Work is cut into chunks of a fixed size that never depends on the thread
//! count, each item is computed alone, and results come back in input order.
//! The work runs on the caller's rayon pool: the library spawns no threads of
//! its own, so the host's job manager decides how many there are.

use rayon::prelude::*;

/// Items per chunk. Fixed, so results never depend on the thread count.
pub const CHUNK: usize = 64;

/// `f` applied to every item, in input order, in parallel on the current
/// rayon pool.
pub fn map_in_order<T, U, F>(items: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    F: Fn(&T) -> U + Sync,
{
    items
        .par_chunks(CHUNK)
        .map(|chunk| chunk.iter().map(&f).collect::<Vec<U>>())
        .collect::<Vec<Vec<U>>>()
        .into_iter()
        .flatten()
        .collect()
}
