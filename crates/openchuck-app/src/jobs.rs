// SPDX-License-Identifier: AGPL-3.0-or-later
//! The job manager (UI-09, NFR-04): one worker pool, cancellation and progress.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use openchuck_types::job::{CancelToken, JobContext, Progress, ProgressSink};

use crate::{AppError, Event};

/// How many worker threads the pool uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Threads {
    /// One per available core.
    #[default]
    All,
    /// Exactly this many (at least one).
    Exactly(usize),
}

/// Identifies a job on the event stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

/// Runs jobs on one worker pool. Libraries take a context rather than
/// spawning threads of their own (architecture section 8).
#[derive(Debug)]
pub struct JobManager {
    pool: rayon::ThreadPool,
    next_id: AtomicU64,
}

/// A running job: wait for it or cancel it.
#[derive(Debug)]
pub struct JobHandle<T> {
    id: JobId,
    name: String,
    cancel: CancelToken,
    result: Receiver<Result<T, AppError>>,
    /// Set once [`JobHandle::try_result`] has handed the result over. The
    /// worker drops its end of the channel only after sending, so the
    /// channel alone would read as still running in between.
    taken: AtomicBool,
}

struct EventProgress {
    job: JobId,
    events: Sender<Event>,
}

impl ProgressSink for EventProgress {
    fn report(&self, progress: Progress) {
        // A closed event stream only means nobody is listening.
        let _ = self.events.send(Event::JobProgress {
            job: self.job,
            progress,
        });
    }
}

impl JobManager {
    /// A manager with a pool of `threads` workers.
    pub fn new(threads: Threads) -> Result<Self, AppError> {
        let count = match threads {
            Threads::All => 0, // rayon picks one per core
            Threads::Exactly(n) => n.max(1),
        };
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(count)
            .thread_name(|i| format!("openchuck-worker-{i}"))
            .build()
            .map_err(|e| AppError::WorkerPool(e.to_string()))?;
        Ok(Self {
            pool,
            next_id: AtomicU64::new(1),
        })
    }

    /// Number of worker threads.
    pub fn threads(&self) -> usize {
        self.pool.current_num_threads()
    }

    /// Runs `work` on the pool. Parallel code inside `work` uses this pool.
    pub fn submit<T, F>(&self, name: &str, events: Sender<Event>, work: F) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(&JobContext<'_>) -> Result<T, AppError> + Send + 'static,
    {
        let id = JobId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let cancel = CancelToken::new();
        let (tx, rx) = channel();
        let job_cancel = cancel.clone();
        let job_name = name.to_owned();
        let _ = events.send(Event::JobStarted {
            job: id,
            name: job_name.clone(),
        });
        let handle_name = job_name.clone();
        self.pool.spawn(move || {
            let progress = EventProgress {
                job: id,
                events: events.clone(),
            };
            let context = JobContext::new(&job_cancel, &progress);
            // A panic must not abort the application (NFR-07): rayon aborts the
            // process on a panic in a spawned task, so catch it here.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&context)))
                .unwrap_or_else(|_| Err(AppError::JobPanicked(job_name.clone())));
            let error = result.as_ref().err().map(ToString::to_string);
            let _ = events.send(Event::JobFinished { job: id, error });
            let _ = tx.send(result);
        });
        JobHandle {
            id,
            name: handle_name,
            cancel,
            result: rx,
            taken: AtomicBool::new(false),
        }
    }
}

impl<T> JobHandle<T> {
    /// The job's identifier.
    pub fn id(&self) -> JobId {
        self.id
    }

    /// Asks the job to stop. It returns a cancelled error at its next check.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// The result if the job has ended, without waiting; `None` while it
    /// runs. A front end polls this each frame. After it has returned a
    /// result it returns a lost-job error.
    pub fn try_result(&self) -> Option<Result<T, AppError>> {
        if self.taken.load(Ordering::Acquire) {
            return Some(Err(AppError::JobLost(self.name.clone())));
        }
        match self.result.try_recv() {
            Ok(result) => {
                self.taken.store(true, Ordering::Release);
                Some(result)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(Err(AppError::JobLost(self.name.clone())))
            }
        }
    }

    /// Blocks until the job ends and returns its result.
    pub fn wait(self) -> Result<T, AppError> {
        self.result
            .recv()
            .unwrap_or_else(|_| Err(AppError::JobLost(self.name)))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn req_nfr_04_polling_never_waits_for_a_running_job() {
        // The job is held on a gate the test opens only after polling, so it
        // is certainly running throughout. A poll that waited for it would
        // never return; the polling runs on its own thread so that this
        // reads as a failure at the deadline rather than a hung test.
        use std::time::{Duration, Instant};
        let manager = JobManager::new(Threads::Exactly(2)).unwrap();
        let (events, events_rx) = channel();
        let (open, gate) = channel::<()>();
        let handle = manager.submit("held", events, move |context| {
            context.report("held", 0, Some(1));
            gate.recv().ok();
            Ok(7)
        });
        let (done, polled) = channel();
        std::thread::spawn(move || {
            let mut seen = 0;
            for _ in 0..1000 {
                assert!(handle.try_result().is_none());
                seen += events_rx.try_iter().count();
            }
            done.send((handle, seen, events_rx)).unwrap();
        });
        let (handle, mut seen, events_rx) = polled
            .recv_timeout(Duration::from_secs(60))
            .expect("a poll waited for the running job");
        open.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let result = loop {
            if let Some(result) = handle.try_result() {
                break result;
            }
            assert!(Instant::now() < deadline, "the released job never ended");
            std::thread::yield_now();
        };
        assert_eq!(result.unwrap(), 7);
        seen += events_rx.try_iter().count();
        // Started, the progress report and finished.
        assert_eq!(seen, 3);
    }

    #[test]
    fn req_nfr_04_a_polled_result_is_handed_over_once_with_no_gap() {
        // The worker sends the result and drops its end of the channel a
        // moment later; a poll in between must not read as "still running"
        // (`try_result` documents a lost-job error once the result is taken).
        let manager = JobManager::new(Threads::Exactly(2)).unwrap();
        let (events, _rx) = channel();
        for i in 0..2000 {
            let handle = manager.submit("quick", events.clone(), move |_| Ok(i));
            let result = loop {
                if let Some(result) = handle.try_result() {
                    break result;
                }
                std::hint::spin_loop();
            };
            assert_eq!(result.unwrap(), i);
            match handle.try_result() {
                Some(Err(AppError::JobLost(_))) => {}
                other => panic!("run {i}: a second poll gave {other:?}"),
            }
        }
    }
}
