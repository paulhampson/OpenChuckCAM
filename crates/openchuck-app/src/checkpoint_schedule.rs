// SPDX-License-Identifier: AGPL-3.0-or-later
//! Dirty-project checkpoint timing (PRJ-08, UI-15).
//!
//! The caller supplies monotonic elapsed time and the committed document
//! revision. Field buffers and running generation do not enter this API.
//! Publication and retention are separate: an interim completion never
//! advances the periodic deadline. At most one request is in flight.

use std::time::Duration;

/// Which independently retained checkpoint series a job belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One of the three regularly spaced snapshots.
    Periodic,
    /// The separate short-term snapshot, replacing only itself.
    Interim,
}

/// An immutable capture request. Complete the same ticket after publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    /// Retention series.
    pub kind: Kind,
    /// The committed revision to capture.
    pub revision: u64,
    captured_at: Duration,
    ticket: u128,
    epoch: u128,
}

/// The interval is a whole number of minutes from 1 through 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Checkpoint interval must be from 1 to 15 whole minutes")]
pub struct InvalidInterval;

/// A scheduler for one open project session; it does no filesystem work.
#[derive(Debug)]
pub struct Schedule {
    interval: Duration,
    interval_changed_at: Duration,
    periodic_due: Duration,
    revision: Option<u64>,
    changed_at: Duration,
    dirty: bool,
    interim_revision: Option<u64>,
    periodic_revision: Option<u64>,
    pending: Option<Request>,
    ticket: u128,
    epoch: u128,
    retry_at: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Self::with_interval(Duration::from_secs(180), Duration::ZERO)
    }
}

impl Schedule {
    /// Start the independent periodic clock at `now`.
    pub fn new(minutes: u16, now: Duration) -> Result<Self, InvalidInterval> {
        if !(1..=15).contains(&minutes) {
            return Err(InvalidInterval);
        }
        Ok(Self::with_interval(
            Duration::from_secs(u64::from(minutes) * 60),
            now,
        ))
    }

    fn with_interval(interval: Duration, now: Duration) -> Self {
        Self {
            interval,
            interval_changed_at: now,
            periodic_due: now.saturating_add(interval),
            revision: None,
            changed_at: now,
            dirty: false,
            interim_revision: None,
            periodic_revision: None,
            pending: None,
            ticket: 0,
            epoch: 0,
            retry_at: now,
        }
    }

    /// The validated periodic interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Apply a validated interval without replacing an in-flight ticket or
    /// resetting the interim debounce. Only the periodic deadline restarts.
    pub fn configure(&mut self, minutes: u16, now: Duration) -> Result<(), InvalidInterval> {
        if !(1..=15).contains(&minutes) {
            return Err(InvalidInterval);
        }
        let interval = Duration::from_secs(u64::from(minutes) * 60);
        if interval != self.interval {
            self.interval = interval;
            self.interval_changed_at = now;
            self.periodic_due = now.saturating_add(interval);
        }
        Ok(())
    }

    /// Observe committed state and issue a due capture, if none is running.
    /// Polling must continue during a job so edits retain their own debounce.
    pub fn next(&mut self, now: Duration, revision: u64, dirty: bool) -> Option<Request> {
        if !dirty {
            if self.dirty {
                self.epoch = self.epoch.saturating_add(1);
                self.interim_revision = None;
                self.periodic_revision = None;
            }
            self.dirty = false;
            self.revision = Some(revision);
            return None;
        }
        if !self.dirty || self.revision != Some(revision) {
            self.changed_at = now;
        }
        self.dirty = true;
        self.revision = Some(revision);
        if self.pending.is_some() || now < self.retry_at {
            return None;
        }
        let kind = if now >= self.periodic_due && self.periodic_revision != Some(revision) {
            Kind::Periodic
        } else if now.saturating_sub(self.changed_at) >= Duration::from_secs(15)
            && self.interim_revision != Some(revision)
        {
            Kind::Interim
        } else {
            return None;
        };
        self.ticket = self.ticket.saturating_add(1);
        let request = Request {
            kind,
            revision,
            captured_at: now,
            ticket: self.ticket,
            epoch: self.epoch,
        };
        self.pending = Some(request);
        Some(request)
    }

    /// Acknowledge publication. A failed write retries after five seconds;
    /// an obsolete completion cannot release or alter a newer job.
    pub fn complete(&mut self, request: Request, success: bool, now: Duration) -> bool {
        if self.pending != Some(request) {
            return false;
        }
        self.pending = None;
        if !success {
            self.retry_at = now.saturating_add(Duration::from_secs(5));
        } else if request.epoch == self.epoch {
            match request.kind {
                Kind::Periodic => {
                    self.periodic_revision = Some(request.revision);
                    if request.captured_at >= self.interval_changed_at {
                        self.periodic_due = request.captured_at.saturating_add(self.interval);
                    }
                }
                Kind::Interim => self.interim_revision = Some(request.revision),
            }
        }
        true
    }
}
