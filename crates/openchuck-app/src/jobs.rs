// SPDX-License-Identifier: AGPL-3.0-or-later
//! The job manager (UI-09, NFR-04): one worker pool, cancellation and progress,
//! and the [`JobLog`] a front end keeps of the jobs it has seen (the Jobs tab
//! of specification section 9; decision record 0117).

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use openchuck_types::job::{CancelToken, JobContext, Progress, ProgressSink};

use crate::{AppError, Event};

/// The total of a run that reports one share of all its work (issue 201):
/// tenths of a per cent. It is the same from the run's first report to its
/// last, so the share shown never starts again at a new stage.
pub const RUN_PROGRESS_TOTAL: u64 = 1000;

/// One share of all a run's work, reported to the run's context as one count
/// out of [`RUN_PROGRESS_TOTAL`] (issue 201). The run divides its work into
/// shares ([`RunProgress::share`]); each passes on the reports of the work it
/// runs as its place in the run. The count never goes back: a report that
/// would move it back is dropped.
pub(crate) struct RunProgress<'a> {
    context: &'a JobContext<'a>,
    /// The count reported last, held while reporting so that reports from
    /// several threads leave in order.
    reached: Mutex<Option<u64>>,
}

impl<'a> RunProgress<'a> {
    /// A run reporting to `context`.
    pub(crate) fn new(context: &'a JobContext<'a>) -> Self {
        Self {
            context,
            reached: Mutex::new(None),
        }
    }

    /// Reports `fraction` (0 to 1) of the run done, in `stage`, unless the
    /// run has reported more already. The first report is always sent, so
    /// the total is known from the start.
    pub(crate) fn reach(&self, stage: &str, fraction: f64) {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let count = (fraction.clamp(0.0, 1.0) * RUN_PROGRESS_TOTAL as f64).floor() as u64;
        if let Ok(mut reached) = self.reached.lock()
            && reached.is_none_or(|r| count > r)
        {
            *reached = Some(count);
            self.context.report(stage, count, Some(RUN_PROGRESS_TOTAL));
        }
    }

    /// The share of the run from `from` to `to` (fractions of the run), in
    /// `stage`.
    pub(crate) fn share(&self, stage: &'static str, from: f64, to: f64) -> ProgressShare<'_, 'a> {
        ProgressShare {
            run: self,
            stage,
            from,
            to,
            within: Mutex::new(Within::default()),
        }
    }
}

/// A share of a run's work, as a [`ProgressSink`] for the work done in it.
///
/// The work reports its own stages, each with its own count, and how many
/// stages it has is not known beforehand. Each stage takes half of what is
/// left of the share: the first the first half, the next the next quarter,
/// and so on. A stage's count moves the figure across its part; a stage with
/// no total stays at its start. The figure is rough, but it always moves
/// forward with the work and never reaches the end of the share before the
/// work does.
pub(crate) struct ProgressShare<'r, 'a> {
    run: &'r RunProgress<'a>,
    stage: &'static str,
    from: f64,
    to: f64,
    within: Mutex<Within>,
}

/// Where the work of a share has got to.
#[derive(Default)]
struct Within {
    /// The stage the work reported last.
    stage: Option<String>,
    /// Where that stage starts, as a fraction of the share.
    start: f64,
}

impl ProgressShare<'_, '_> {
    /// Reports the share as done.
    pub(crate) fn finish(&self) {
        self.run.reach(self.stage, self.to);
    }
}

impl ProgressSink for ProgressShare<'_, '_> {
    fn report(&self, progress: Progress) {
        let at = {
            let Ok(mut within) = self.within.lock() else {
                return;
            };
            if within.stage.as_deref() != Some(progress.stage.as_str()) {
                if within.stage.is_some() {
                    within.start += (1.0 - within.start) / 2.0;
                }
                within.stage = Some(progress.stage);
            }
            #[allow(clippy::cast_precision_loss)]
            let done = progress
                .total
                .filter(|t| *t > 0)
                .map_or(0.0, |t| (progress.done as f64 / t as f64).min(1.0));
            within.start + (1.0 - within.start) / 2.0 * done
        };
        self.run
            .reach(self.stage, self.from + (self.to - self.from) * at);
    }
}

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
    /// The cancel tokens of the jobs still running, so a job can be cancelled
    /// by its ID from a list of jobs (specification section 9, Jobs).
    running: Arc<Mutex<BTreeMap<JobId, CancelToken>>>,
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
            running: Arc::default(),
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
        if let Ok(mut running) = self.running.lock() {
            running.insert(id, cancel.clone());
        }
        let registry = Arc::clone(&self.running);
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
            let cancelled = result.as_ref().err().is_some_and(AppError::is_cancelled);
            if let Ok(mut running) = registry.lock() {
                running.remove(&id);
            }
            let _ = events.send(Event::JobFinished {
                job: id,
                error,
                cancelled,
            });
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

impl JobManager {
    /// Asks the running job `id` to stop, as [`JobHandle::cancel`] does.
    /// Returns false when no such job is running.
    pub fn cancel(&self, id: JobId) -> bool {
        self.running
            .lock()
            .ok()
            .and_then(|running| running.get(&id).map(CancelToken::cancel))
            .is_some()
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

/// How a job the log has seen ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    /// It finished; the summary, when its owner gave one ("Generated 2,
    /// reused 1", decision record 0030 item 6).
    Done(Option<String>),
    /// It failed, with the message.
    Failed(String),
    /// It was cancelled on request.
    Cancelled,
}

/// One job in a [`JobLog`].
#[derive(Debug, Clone, PartialEq)]
pub struct JobEntry {
    /// The job.
    pub id: JobId,
    /// The name it was submitted under (internal; [`JobEntry::label`] is the
    /// user's wording).
    pub name: String,
    /// What it does, in the user's words ("Generating").
    pub label: String,
    /// When the log first saw it.
    pub started: Instant,
    /// How long it ran, once it has finished.
    pub took: Option<Duration>,
    /// The latest progress it reported.
    pub progress: Option<Progress>,
    /// How it ended; `None` while it runs.
    pub outcome: Option<JobOutcome>,
}

impl JobEntry {
    /// The share done, 0 to 1, when the job reported a total.
    pub fn fraction(&self) -> Option<f32> {
        let p = self.progress.as_ref()?;
        let total = p.total.filter(|t| *t > 0)?;
        #[allow(clippy::cast_precision_loss)]
        let fraction = p.done as f32 / total as f32;
        Some(fraction.clamp(0.0, 1.0))
    }

    /// The whole per cent done, when the job reported a total: rounded
    /// down, so a job reads 100 % only when all its work is done (issue
    /// 201).
    pub fn percent(&self) -> Option<u64> {
        let p = self.progress.as_ref()?;
        let total = p.total.filter(|t| *t > 0)?;
        Some(p.done.min(total) * 100 / total)
    }

    /// Whether the job is still running.
    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }
}

/// The jobs a front end shows, folded from the event stream
/// ([`Application::drain_events`](crate::Application::drain_events)): the
/// running ones with their progress, and the latest finished ones with their
/// outcome (UI-09; specification section 9, Jobs; decision record 0117).
///
/// Quick jobs that run while the user points or plays (picking a region, the
/// stock at a move) are not listed: only the jobs [`job_label`] names are.
/// The front end drains the events once per frame and hands them to
/// [`JobLog::update`], so every panel reads progress from the same place.
#[derive(Debug, Default, Clone)]
pub struct JobLog {
    running: Vec<JobEntry>,
    finished: VecDeque<JobEntry>,
    /// Summaries given before their job's finish was read: a job hands its
    /// result over just after it reports finishing, so its owner can see the
    /// result first.
    pending: Vec<(JobId, String)>,
}

/// How many finished jobs a [`JobLog`] keeps.
pub const FINISHED_JOBS_KEPT: usize = 20;

/// The user's words for a job submitted as `name`, and whether it is kept in
/// the finished list once it ends. `None` for quick jobs the Jobs list leaves
/// out. The project checks run after every edit, so only their failures are
/// kept.
pub fn job_label(name: &str) -> Option<(&'static str, bool)> {
    Some(match name {
        "build scene" => ("Building the view", false),
        "regenerate" => ("Generating", true),
        "simulate" => ("Generating and simulating", true),
        "post" => ("Posting G-code", true),
        "import model" => ("Adding the model", true),
        "check model" => ("Checking the model file", false),
        "inspect model" => ("Checking the model", false),
        "default tabs" => ("Working out the default tabs", false),
        "check workholding" => ("Checking the workholding", false),
        "workholding warnings" => ("Checking selections against the workholding", false),
        "project checks" => ("Checking the project", false),
        "colour by deviation" => ("Colouring by deviation", false),
        "prepare playback" => ("Simulating the cut part for playback", false),
        "save selection" => ("Saving the selection set", true),
        "edit selection" => ("Changing the selection set", true),
        _ => return None,
    })
}

impl JobLog {
    /// Folds `events` into the log.
    pub fn update(&mut self, events: impl IntoIterator<Item = Event>) {
        self.update_at(events, Instant::now());
    }

    fn update_at(&mut self, events: impl IntoIterator<Item = Event>, now: Instant) {
        for event in events {
            match event {
                Event::JobStarted { job, name } => {
                    if let Some((label, _)) = job_label(&name) {
                        self.running.push(JobEntry {
                            id: job,
                            label: label.to_owned(),
                            name,
                            started: now,
                            took: None,
                            progress: None,
                            outcome: None,
                        });
                    }
                }
                Event::JobProgress { job, progress } => {
                    if let Some(entry) = self.running.iter_mut().find(|e| e.id == job) {
                        entry.progress = Some(progress);
                    }
                }
                Event::JobFinished {
                    job,
                    error,
                    cancelled,
                } => {
                    let Some(index) = self.running.iter().position(|e| e.id == job) else {
                        continue;
                    };
                    let mut entry = self.running.remove(index);
                    entry.took = Some(now.saturating_duration_since(entry.started));
                    let failed = error.is_some() && !cancelled;
                    let summary = self
                        .pending
                        .iter()
                        .position(|(id, _)| *id == job)
                        .map(|i| self.pending.remove(i).1);
                    entry.outcome = Some(match error {
                        _ if cancelled => JobOutcome::Cancelled,
                        Some(e) => JobOutcome::Failed(e),
                        None => JobOutcome::Done(summary),
                    });
                    let kept = job_label(&entry.name).is_some_and(|(_, kept)| kept);
                    if kept || failed {
                        self.finished.push_front(entry);
                        self.finished.truncate(FINISHED_JOBS_KEPT);
                    } else if !cancelled {
                        // A job that is not kept clears the failures of
                        // earlier runs of its kind by succeeding.
                        let name = entry.name;
                        self.finished.retain(|e| e.name != name);
                    }
                }
            }
        }
    }

    /// The running jobs, oldest first.
    pub fn running(&self) -> &[JobEntry] {
        &self.running
    }

    /// The finished jobs kept, newest first.
    pub fn finished(&self) -> impl Iterator<Item = &JobEntry> {
        self.finished.iter()
    }

    /// The job `id`, running or finished, if the log has it.
    pub fn get(&self, id: JobId) -> Option<&JobEntry> {
        self.running
            .iter()
            .chain(self.finished.iter())
            .find(|e| e.id == id)
    }

    /// Gives the finished job `id` the summary its owner worked out from the
    /// result ("Generated 2, reused 1").
    pub fn set_summary(&mut self, id: JobId, summary: String) {
        if let Some(entry) = self.finished.iter_mut().find(|e| e.id == id) {
            entry.outcome = Some(JobOutcome::Done(Some(summary)));
        } else {
            // Running, or not read yet: a fast job can start and finish
            // between two frames. The finish takes the summary.
            self.pending.push((id, summary));
            if self.pending.len() > FINISHED_JOBS_KEPT {
                self.pending.remove(0);
            }
        }
    }

    /// The latest failure of a job submitted as `name`, unless a later job of
    /// that name finished without failing (a failure clears when a later run
    /// succeeds; specification section 9, "Clearing").
    pub fn latest_failure(&self, name: &str) -> Option<&JobEntry> {
        self.finished
            .iter()
            .find(|e| e.name == name && !matches!(e.outcome, Some(JobOutcome::Cancelled)))
            .filter(|e| matches!(e.outcome, Some(JobOutcome::Failed(_))))
    }

    /// Forgets every job, for a project that was closed.
    pub fn clear(&mut self) {
        *self = Self::default();
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

    fn started(job: u64, name: &str) -> Event {
        Event::JobStarted {
            job: JobId(job),
            name: name.to_owned(),
        }
    }

    fn finished(job: u64, error: Option<&str>, cancelled: bool) -> Event {
        Event::JobFinished {
            job: JobId(job),
            error: error.map(str::to_owned),
            cancelled,
        }
    }

    #[test]
    fn req_ui_09_the_job_log_lists_running_and_finished_jobs() {
        let mut log = JobLog::default();
        let t0 = Instant::now();
        log.update_at(
            [
                started(1, "regenerate"),
                started(2, "stock at move"),
                Event::JobProgress {
                    job: JobId(1),
                    progress: Progress {
                        stage: "regenerate".into(),
                        done: 1,
                        total: Some(4),
                    },
                },
            ],
            t0,
        );
        // Quick jobs are not listed.
        assert_eq!(log.running().len(), 1);
        let entry = log.get(JobId(1)).unwrap();
        assert_eq!(entry.label, "Generating");
        assert_eq!(entry.fraction(), Some(0.25));
        assert_eq!(entry.percent(), Some(25));
        assert!(entry.is_running());
        // The owner may see the result before the log reads the finish.
        log.set_summary(JobId(1), "Generated 2, reused 1".into());
        log.update_at([finished(1, None, false)], t0 + Duration::from_secs(3));
        assert!(log.running().is_empty());
        let done = log.finished().next().unwrap();
        assert_eq!(done.took, Some(Duration::from_secs(3)));
        assert_eq!(
            done.outcome,
            Some(JobOutcome::Done(Some("Generated 2, reused 1".into())))
        );
    }

    /// A fast job can start, finish and hand over its result between two
    /// frames: its owner then gives the summary before the log has read
    /// either event, and the summary must not be lost.
    #[test]
    fn req_ui_09_a_summary_given_before_the_log_reads_the_job_is_kept() {
        let mut log = JobLog::default();
        let t0 = Instant::now();
        log.set_summary(JobId(1), "Generated 1".into());
        log.update_at(
            [started(1, "regenerate"), finished(1, None, false)],
            t0 + Duration::from_secs(1),
        );
        assert_eq!(
            log.finished().next().unwrap().outcome,
            Some(JobOutcome::Done(Some("Generated 1".into())))
        );
    }

    #[test]
    fn req_ui_12_a_job_failure_clears_when_a_later_run_succeeds() {
        let mut log = JobLog::default();
        log.update([
            started(1, "build scene"),
            finished(1, Some("bad mesh"), false),
        ]);
        assert!(log.latest_failure("build scene").is_some());
        // A cancelled run neither fails nor clears the failure.
        log.update([
            started(2, "build scene"),
            finished(2, Some("cancelled"), true),
        ]);
        assert!(log.latest_failure("build scene").is_some());
        // A successful build is not listed, but it clears the failure.
        log.update([started(3, "build scene"), finished(3, None, false)]);
        assert!(log.latest_failure("build scene").is_none());
        log.update([started(6, "regenerate"), finished(6, None, false)]);
        assert!(log.latest_failure("regenerate").is_none());
        log.update([
            started(4, "regenerate"),
            finished(4, Some("no tool"), false),
        ]);
        assert!(log.latest_failure("regenerate").is_some());
        log.update([started(5, "regenerate"), finished(5, None, false)]);
        assert!(log.latest_failure("regenerate").is_none());
        log.clear();
        assert_eq!(log.finished().count(), 0);
    }

    #[test]
    fn req_ui_09_a_job_is_cancelled_by_its_id() {
        let manager = JobManager::new(Threads::Exactly(1)).unwrap();
        let (events, rx) = channel();
        let handle = manager.submit("regenerate", events, |context| {
            loop {
                context.check_cancelled()?;
                std::thread::yield_now();
            }
        });
        assert!(manager.cancel(handle.id()));
        let result: Result<(), AppError> = handle.wait();
        assert!(result.unwrap_err().is_cancelled());
        let finished = rx
            .iter()
            .find(|e| matches!(e, Event::JobFinished { .. }))
            .unwrap();
        assert!(matches!(
            finished,
            Event::JobFinished {
                cancelled: true,
                ..
            }
        ));
        assert!(!manager.cancel(JobId(999)));
    }

    /// Keeps every count reported.
    #[derive(Default)]
    struct Counts(Mutex<Vec<(String, u64, Option<u64>)>>);

    impl ProgressSink for Counts {
        fn report(&self, p: Progress) {
            self.0.lock().unwrap().push((p.stage, p.done, p.total));
        }
    }

    fn report(sink: &dyn ProgressSink, stage: &str, done: u64, total: Option<u64>) {
        sink.report(Progress {
            stage: stage.to_owned(),
            done,
            total,
        });
    }

    #[test]
    fn req_ui_09_a_share_moves_forward_through_stages_it_cannot_count_and_never_back() {
        // Issue 201: a run's work reports stages whose number is not known,
        // each with its own count. The run shows one figure out of one total,
        // which moves with the work and never goes back.
        let counts = Counts::default();
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &counts);
        let run = RunProgress::new(&context);
        run.reach("run", 0.0);
        let share = run.share("run", 0.5, 1.0);
        report(&share, "first", 0, Some(10));
        report(&share, "first", 10, Some(10));
        // A stage's count that goes back, and one that is over its total,
        // move nothing back or past the stage.
        report(&share, "first", 3, Some(10));
        report(&share, "first", 20, Some(10));
        report(&share, "second", 0, None);
        report(&share, "second", 1, Some(2));
        report(&share, "third", 1, Some(1));
        share.finish();
        // Reported again at the same place: nothing new is sent.
        run.reach("run", 1.0);
        let counts = counts.0.into_inner().unwrap();
        let done: Vec<u64> = counts.iter().map(|c| c.1).collect();
        // The share is the second half: the first stage takes its first
        // half (500 to 750), the second the next quarter (750 to 875), and
        // so on, up to the end of the share only when it is done.
        assert_eq!(done, [0, 500, 750, 812, 937, 1000]);
        assert!(
            counts
                .iter()
                .all(|c| c.0 == "run" && c.2 == Some(RUN_PROGRESS_TOTAL))
        );
    }

    #[test]
    fn req_ui_09_reports_from_many_threads_leave_in_order() {
        // The passes of a strategy report from several threads at once
        // (decision 0195): the figure sent never goes back.
        let counts = Counts::default();
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &counts);
        let run = RunProgress::new(&context);
        let share = run.share("run", 0.0, 1.0);
        std::thread::scope(|scope| {
            for t in 0..4 {
                let share = &share;
                scope.spawn(move || {
                    for i in 0..250 {
                        report(share, "passes", t * 250 + i, Some(1000));
                    }
                });
            }
        });
        let counts = counts.0.into_inner().unwrap();
        assert!(counts.windows(2).all(|w| w[0].1 < w[1].1), "{counts:?}");
    }

    #[test]
    fn req_ui_09_a_job_reads_100_per_cent_only_when_done() {
        // Issue 201: the percentage is rounded down, so 99.9 % is not shown
        // as done while the last of the work runs.
        let entry = |done, total| JobEntry {
            id: JobId(1),
            name: "regenerate".into(),
            label: "Generating".into(),
            started: Instant::now(),
            took: None,
            progress: Some(Progress {
                stage: "regenerate".into(),
                done,
                total,
            }),
            outcome: None,
        };
        assert_eq!(entry(999, Some(RUN_PROGRESS_TOTAL)).percent(), Some(99));
        assert_eq!(entry(1000, Some(RUN_PROGRESS_TOTAL)).percent(), Some(100));
        assert_eq!(entry(5, Some(0)).percent(), None);
        assert_eq!(entry(5, None).percent(), None);
    }
}
