// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local checkpoint jobs and startup discovery (PRJ-08, UI-15, NFR-04).
//! A serialized worker owns each project's recovery lease. Rendering,
//! hashing, disk writes and model checks stay off the front-end thread.

use crate::checkpoint_schedule::{Kind, Request, Schedule};
use crate::{AppError, Application, Threads};
use openchuck_project::recovery::{Catalog, Snapshot, Store};
use openchuck_project::{ContentHash, ModelCheck, Project};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A worker result for presentation, never a request to write saved files.
#[derive(Debug)]
pub enum Notice {
    /// Ownership and startup indexing succeeded.
    Ready,
    /// A capture was safely published.
    Published {
        /// Retention series.
        kind: Kind,
        /// Committed revision captured.
        revision: u64,
    },
    /// A successful explicit Save retired only covered states.
    Saved,
    /// Clean-exit processing finished; an error preserves recovery data.
    Closed {
        /// Cleanup failure to report before exiting.
        problem: Option<String>,
    },
    /// A user-selected checkpoint was decoded without touching saved files.
    Restored {
        /// Recovered committed document.
        project: Box<Project>,
        /// Missing/changed model references, checked on the worker.
        warnings: Vec<String>,
    },
    /// A recoverable write/read/discovery failure.
    Problem(String),
}

enum Command {
    Capture(Request, Box<Project>),
    Saved(u64),
    Close(u64),
    Restore(String),
    Dismiss,
    Discard,
}
enum Event {
    Notice(Notice),
    Complete(Request, Result<(), String>),
}

/// A nonblocking controller. Dropping it without `close` deliberately leaves
/// recovery evidence; the worker releases its lease when queued work ends.
#[derive(Debug)]
pub struct Session {
    tx: Sender<Command>,
    rx: Receiver<Event>,
    schedule: Schedule,
    started: Instant,
    ready: bool,
    closing: bool,
    worker_done: bool,
}

impl Session {
    /// Start a project-local writer and register its canonical folder in
    /// `config`. Errors acquiring ownership arrive as a Problem notice.
    pub fn open(folder: PathBuf, config: PathBuf, minutes: u16) -> Result<Self, String> {
        let schedule = Schedule::new(minutes, Duration::ZERO).map_err(|e| e.to_string())?;
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("project recovery".into())
            .spawn(move || {
                let opened = Store::open(&folder)
                    .map_err(|e| e.to_string())
                    .and_then(|store| {
                        register(
                            &config,
                            &folder,
                            &store
                                .metadata(openchuck_project::recovery::Kind::Periodic, 0, 0)
                                .project_id,
                        )?;
                        Ok(store)
                    });
                let mut store = match opened {
                    Ok(store) => store,
                    Err(e) => {
                        let _ = events.send(Event::Notice(Notice::Problem(e)));
                        return;
                    }
                };
                let _ = events.send(Event::Notice(Notice::Ready));
                while let Ok(command) = commands.recv() {
                    let notice = match command {
                        Command::Capture(request, project) => {
                            let kind = match request.kind {
                                Kind::Periodic => openchuck_project::recovery::Kind::Periodic,
                                Kind::Interim => openchuck_project::recovery::Kind::Interim,
                            };
                            let time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
                            let result = Snapshot::capture(
                                &project,
                                store.metadata(kind, request.revision, time),
                            )
                            .and_then(|s| store.publish(&s))
                            .map(|_| ())
                            .map_err(|e| e.to_string());
                            if events.send(Event::Complete(request, result)).is_err() {
                                break;
                            }
                            continue;
                        }
                        Command::Saved(revision) => match store.retire_saved(revision) {
                            Ok(()) => Notice::Saved,
                            Err(e) => Notice::Problem(e.to_string()),
                        },
                        Command::Close(revision) => {
                            let problem = store.close(revision).err().map(|e| e.to_string());
                            drop(store);
                            let _ = events.send(Event::Notice(Notice::Closed { problem }));
                            break;
                        }
                        Command::Restore(id) => match store.read(&id).and_then(|s| s.project()) {
                            Ok(project) => Notice::Restored {
                                warnings: model_warnings(&project, &folder),
                                project: Box::new(project),
                            },
                            Err(e) => Notice::Problem(e.to_string()),
                        },
                        Command::Dismiss => match store.dismiss() {
                            Ok(()) => Notice::Saved,
                            Err(e) => Notice::Problem(e.to_string()),
                        },
                        Command::Discard => match store.discard() {
                            Ok(()) => Notice::Saved,
                            Err(e) => Notice::Problem(e.to_string()),
                        },
                    };
                    if events.send(Event::Notice(notice)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tx,
            rx,
            schedule,
            started: Instant::now(),
            ready: false,
            closing: false,
            worker_done: false,
        })
    }

    /// Elapsed monotonic time used by production front ends.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Whether this session owns a ready writer and can schedule work.
    pub fn is_ready(&self) -> bool {
        self.ready && !self.closing && !self.worker_done
    }

    /// Apply a per-user interval change without replacing in-flight ownership.
    pub fn configure(&mut self, minutes: u16) -> Result<(), String> {
        self.schedule
            .configure(minutes, self.elapsed())
            .map_err(|e| e.to_string())
    }

    /// Drain ready results without waiting for filesystem work.
    pub fn poll(&mut self, now: Duration) -> Vec<Notice> {
        let mut notices = Vec::new();
        loop {
            let event = match self.rx.try_recv() {
                Ok(event) => event,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !self.worker_done {
                        if self.closing {
                            notices.push(Notice::Closed {problem:Some("Recovery worker stopped before cleanup completed. Existing checkpoints were preserved.".into())});
                        } else if self.ready {
                            notices.push(Notice::Problem(
                                "Recovery worker stopped. Existing checkpoints were preserved."
                                    .into(),
                            ));
                        }
                        self.worker_done = true;
                        self.ready = false;
                    }
                    break;
                }
            };
            match event {
                Event::Notice(notice) => {
                    if matches!(notice, Notice::Ready) {
                        self.ready = true;
                    }
                    if matches!(notice, Notice::Closed { .. }) {
                        self.worker_done = true;
                        self.ready = false;
                    }
                    notices.push(notice);
                }
                Event::Complete(request, result) => {
                    self.schedule.complete(request, result.is_ok(), now);
                    notices.push(match result {
                        Ok(()) => Notice::Published {
                            kind: request.kind,
                            revision: request.revision,
                        },
                        Err(e) => Notice::Problem(e),
                    });
                }
            }
        }
        notices
    }

    /// Observe committed state each frame. Only a due request clones the
    /// project; all serialization and file I/O happen on the worker.
    pub fn tick(&mut self, now: Duration, project: &Project, revision: u64, dirty: bool) {
        if self.closing || !self.ready {
            return;
        }
        if let Some(request) = self.schedule.next(now, revision, dirty)
            && self
                .tx
                .send(Command::Capture(request, Box::new(project.clone())))
                .is_err()
        {
            self.schedule.complete(request, false, now);
            self.ready = false;
        }
    }

    fn send(&self, command: Command) -> Result<(), String> {
        self.tx.send(command).map_err(|_| {
            "Recovery worker is unavailable; existing checkpoints were preserved.".to_owned()
        })
    }
    /// Call only after the ordinary explicit Save succeeds.
    pub fn saved(&self, revision: u64) -> Result<(), String> {
        self.send(Command::Saved(revision))
    }
    /// Begin cleanup only after pending fields and Save/discard are resolved.
    /// Wait for a Closed notice before destroying the native window.
    pub fn close(&mut self, revision: u64) -> Result<(), String> {
        self.closing = true;
        self.send(Command::Close(revision))
    }
    /// Read exactly the user's selected checkpoint.
    pub fn restore(&self, id: String) -> Result<(), String> {
        self.send(Command::Restore(id))
    }
    /// Keep the saved project without deleting retained choices.
    pub fn dismiss(&self) -> Result<(), String> {
        self.send(Command::Dismiss)
    }
    /// Explicitly discard validated recovery snapshots.
    pub fn discard(&self) -> Result<(), String> {
        self.send(Command::Discard)
    }
}

fn model_warnings(project: &Project, folder: &Path) -> Vec<String> {
    project.models().iter().filter_map(|m| {
        match m.check(folder) {
            Ok(ModelCheck::Unchanged)=>None,
            Ok(ModelCheck::Missing)=>Some(format!("Referenced model '{}' is missing; its geometry cannot be recovered from a project checkpoint.",m.label())),
            Ok(_)=>Some(format!("Referenced model '{}' has changed since this checkpoint. Check selections and regenerate before using output.",m.label())),
            Err(e)=>Some(format!("Referenced model '{}' cannot be checked: {e}",m.label())),
        }
    }).collect()
}

/// A chosen checkpoint decoded without acquiring a writable recovery store.
#[derive(Debug)]
pub struct Recovered {
    /// Original project folder, still used by relative model references.
    pub folder: PathBuf,
    /// Committed state to install as unsaved memory.
    pub project: Box<Project>,
    /// Model content that is missing, changed or unreadable.
    pub warnings: Vec<String>,
}

/// Read-only recovery job. Front ends run this on a worker so a read-only
/// project can still be recovered into memory; future checkpoint failures
/// are reported independently by the writer controller.
pub fn recover(folder: &Path, id: &str, identity: &str) -> Result<Recovered, String> {
    let catalog = Catalog::read(folder).map_err(|e| e.to_string())?;
    if catalog.live {
        return Err(
            "Another instance is using this project's recovery data. Close it before recovering."
                .into(),
        );
    }
    let project = openchuck_project::recovery::read_choice(folder, id, identity)
        .and_then(|s| s.project())
        .map_err(|e| e.to_string())?;
    Ok(Recovered {
        folder: folder.to_owned(),
        warnings: model_warnings(&project, folder),
        project: Box::new(project),
    })
}

impl Application {
    /// Recover a validated project into unsaved memory. The last saved files
    /// remain untouched even when the recovered state equals those files.
    pub fn from_checkpoint(
        folder: &Path,
        project: Project,
        threads: Threads,
    ) -> Result<Self, AppError> {
        project.validate()?;
        let app = Self::with_project(folder, project, threads)?;
        app.recovered_unsaved
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(app)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registered {
    folder: PathBuf,
    project_id: String,
}

fn register(config: &Path, folder: &Path, project_id: &str) -> Result<(), String> {
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    let dir = config.join("checkpoint-projects");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let key = index_key(&folder, project_id);
    let path = dir.join(format!("{key}.toml"));
    let temporary = dir.join(format!("{key}-{}.part", std::process::id()));
    let text = toml::to_string(&Registered {
        folder,
        project_id: project_id.into(),
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(&temporary, text)
        .and_then(|()| std::fs::rename(&temporary, &path))
        .map_err(|e| e.to_string())
}

/// A registered folder, including moved/inaccessible locations needing Locate.
#[derive(Debug)]
pub struct Candidate {
    /// Last known canonical folder path.
    pub folder: PathBuf,
    /// Valid choices and independently preserved read failures.
    pub catalog: Option<Catalog>,
    /// A folder-level error, suitable for the recovery list.
    pub problem: Option<String>,
    /// Expected recovery identity when the user locates a moved project.
    pub project_id: String,
}
/// Startup discovery result. Keep failures beside other recovery choices.
#[derive(Debug, Default)]
pub struct Discovery {
    /// Registered projects, canonical aliases deduplicated.
    pub projects: Vec<Candidate>,
    /// Unreadable registry entries, preserved on disk.
    pub problems: Vec<String>,
}

/// Inspect the per-user index. Run this on a worker, never during UI drawing.
pub fn discover(config: &Path) -> Discovery {
    let mut found = Discovery::default();
    let dir = config.join("checkpoint-projects");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return found,
        Err(e) => {
            found
                .problems
                .push(format!("Cannot inspect recovery locations: {e}"));
            return found;
        }
    };
    let mut seen = BTreeSet::new();
    for entry in entries {
        let read = || {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                return Ok(None);
            }
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
                return Err(format!(
                    "Recovery index '{}' is too large; preserved.",
                    path.display()
                ));
            }
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            toml::from_str::<Registered>(&text)
                .map(Some)
                .map_err(|e| format!("Cannot read '{}': {e}. File preserved.", path.display()))
        };
        let registered = match read() {
            Ok(Some(registered)) => registered,
            Ok(None) => continue,
            Err(e) => {
                found.problems.push(e);
                continue;
            }
        };
        let canonical = registered.folder.canonicalize();
        let key = canonical.as_ref().unwrap_or(&registered.folder).clone();
        if !seen.insert((key, registered.project_id.clone())) {
            continue;
        }
        let read=canonical.map_err(|e|e.to_string()).and_then(|path|Catalog::read(&path).map_err(|e|e.to_string())).and_then(|catalog| {
            if catalog.project_id.as_deref()!=Some(registered.project_id.as_str()) {
                Err("The folder no longer contains recovery data for this project. Locate the original folder; existing data was preserved.".into())
            } else {Ok(catalog)}
        });
        let (catalog, problem) = match read {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e)),
        };
        found.projects.push(Candidate {
            folder: registered.folder,
            project_id: registered.project_id,
            catalog,
            problem,
        });
    }
    found.projects.sort_by(|a, b| a.folder.cmp(&b.folder));
    found
}

/// Associate a user-located folder only when its recovery identity matches.
/// Publish the new index entry before retiring the inaccessible old path.
pub fn locate(config: &Path, missing: &Candidate, folder: &Path) -> Result<Candidate, String> {
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    let catalog = Catalog::read(&folder).map_err(|e| e.to_string())?;
    if catalog.project_id.as_deref() != Some(missing.project_id.as_str()) {
        return Err(
            "That folder belongs to another recovery project. Choose the original moved folder."
                .into(),
        );
    }
    register(config, &folder, &missing.project_id)?;
    if folder != missing.folder {
        let key = index_key(&missing.folder, &missing.project_id);
        let path = config
            .join("checkpoint-projects")
            .join(format!("{key}.toml"));
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(Candidate {
        folder,
        catalog: Some(catalog),
        problem: None,
        project_id: missing.project_id.clone(),
    })
}

fn index_key(folder: &Path, project_id: &str) -> String {
    let mut bytes = folder.as_os_str().as_encoded_bytes().to_vec();
    bytes.push(0);
    bytes.extend_from_slice(project_id.as_bytes());
    ContentHash::of_bytes(&bytes)
        .to_string()
        .replace("blake3:", "")
}

#[cfg(test)]
mod failure_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    #[test]
    fn req_prj_08_req_nfr_07_worker_failure_during_close_reports_completion_instead_of_hanging() {
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::channel();
        let mut session = Session {
            tx,
            rx,
            schedule: Schedule::default(),
            started: Instant::now(),
            ready: true,
            closing: false,
            worker_done: false,
        };
        session.close(3).unwrap();
        // The worker dies after accepting Close but before acknowledgement.
        // Exit must finish with an error, preserving recovery data.
        drop(commands);
        drop(events);
        assert!(
            session
                .poll(Duration::ZERO)
                .iter()
                .any(|n| matches!(n, Notice::Closed { problem: Some(_) }))
        );
    }
}
