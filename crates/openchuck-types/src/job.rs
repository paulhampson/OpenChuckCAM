// SPDX-License-Identifier: AGPL-3.0-or-later
//! The job context passed into every long-running call (INT-04, UI-09).
//!
//! Libraries never spawn threads of their own or report progress any other
//! way (architecture section 8): a host passes a [`JobContext`], checks the
//! [`CancelToken`] it holds, and receives [`Progress`] through a [`ProgressSink`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Error, Result};

/// A shared flag that asks a running job to stop.
///
/// Clones share the flag. Cancellation is cooperative: the job calls
/// [`CancelToken::check`] at convenient points and returns [`Error::Cancelled`].
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token that is not cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the job to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// True once [`CancelToken::cancel`] has been called on any clone.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Returns [`Error::Cancelled`] once the token is cancelled.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// How far a job has got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// What the job is doing, for display.
    pub stage: String,
    /// Units of work done.
    pub done: u64,
    /// Units of work in total, if known.
    pub total: Option<u64>,
}

/// Receives progress reports. Implementations must be cheap and must not block.
pub trait ProgressSink: Send + Sync {
    /// Called with each progress report.
    fn report(&self, progress: Progress);
}

/// A sink that discards progress.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn report(&self, _progress: Progress) {}
}

/// Everything a long-running library call needs from its host.
#[derive(Clone, Copy)]
pub struct JobContext<'a> {
    /// Cancellation flag.
    pub cancel: &'a CancelToken,
    /// Where progress goes.
    pub progress: &'a dyn ProgressSink,
}

impl std::fmt::Debug for JobContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobContext")
            .field("cancel", self.cancel)
            .finish_non_exhaustive()
    }
}

impl<'a> JobContext<'a> {
    /// A context from a token and a sink.
    pub fn new(cancel: &'a CancelToken, progress: &'a dyn ProgressSink) -> Self {
        Self { cancel, progress }
    }

    /// Returns [`Error::Cancelled`] once the job is cancelled.
    pub fn check_cancelled(&self) -> Result<()> {
        self.cancel.check()
    }

    /// Reports progress.
    pub fn report(&self, stage: &str, done: u64, total: Option<u64>) {
        self.progress.report(Progress {
            stage: stage.to_owned(),
            done,
            total,
        });
    }
}
