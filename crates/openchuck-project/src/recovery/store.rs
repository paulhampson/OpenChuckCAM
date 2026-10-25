// SPDX-License-Identifier: AGPL-3.0-or-later
//! Serialized publication, ownership and retention for project recovery.

use super::{Kind, MAX_BYTES, Metadata, Snapshot, invalid};
use crate::ProjectError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Recovery lives beside, never among, tracked project files.
pub const DIRECTORY: &str = ".openchuck-recovery";

/// A validated recovery choice. Its ID is a safe basename, not a path.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Immutable published file's basename.
    pub id: String,
    /// Project name from the validated captured document.
    pub project_name: String,
    /// Publication ordering, independent of the wall clock.
    pub sequence: u64,
    /// Captured project/session/revision/time and retention type.
    pub metadata: Metadata,
    /// Already covered by a successful Save or clean exit.
    pub retired: bool,
}

/// Choices and independent errors from inspecting one project folder.
#[derive(Debug, Default)]
pub struct Catalog {
    /// Stable recovery identity, when a recovery folder exists.
    pub project_id: Option<String>,
    /// Valid choices in publication order, newest first.
    pub snapshots: Vec<Entry>,
    /// Corrupt or partial files are preserved and named here.
    pub problems: Vec<String>,
    /// Another instance currently holds the recovery writer lock.
    pub live: bool,
    /// An unexpected session left an unretired, undismissed state.
    pub recovery_needed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    #[serde(default)]
    sequence: u64,
    retired: BTreeMap<String, u64>,
    dismissed_through: u64,
    clean: bool,
}

/// Exclusive writer for one canonical project folder. Dropping it without
/// `close` releases the OS lock while retaining unexpected-session evidence.
#[derive(Debug)]
pub struct Store {
    dir: PathBuf,
    identity: String,
    session: String,
    state: State,
    closed: bool,
    _lock: File,
}

fn io(path: &Path, source: std::io::Error) -> ProjectError {
    ProjectError::Io {
        path: path.to_owned(),
        source,
    }
}

fn read_text(path: &Path) -> Result<String, ProjectError> {
    if std::fs::symlink_metadata(path)
        .map_err(|e| io(path, e))?
        .file_type()
        .is_symlink()
    {
        return Err(invalid(format!(
            "recovery file '{}' is a symbolic link",
            path.display()
        )));
    }
    let mut text = String::new();
    File::open(path)
        .map_err(|e| io(path, e))?
        .take((MAX_BYTES + 1) as u64)
        .read_to_string(&mut text)
        .map_err(|e| io(path, e))?;
    if text.len() > MAX_BYTES {
        return Err(invalid(format!(
            "'{}' exceeds the recovery size limit",
            path.display()
        )));
    }
    Ok(text)
}

fn state(dir: &Path) -> Result<State, ProjectError> {
    let path = dir.join("state.toml");
    match read_text(&path) {
        Ok(text) => toml::from_str(&text).map_err(invalid),
        Err(ProjectError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(State::default())
        }
        Err(e) => Err(e),
    }
}

fn sync_dir(dir: &Path) -> Result<(), ProjectError> {
    // Windows does not expose directory handles through File::open. File
    // contents are flushed there; power-loss durability has that limitation.
    #[cfg(unix)]
    File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| io(dir, e))?;
    Ok(())
}

fn stage(dir: &Path, text: &str) -> Result<PathBuf, ProjectError> {
    let path = dir.join(format!("{}.part", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|e| io(&path, e))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| io(&path, e))?;
    Ok(path)
}

fn replace(dir: &Path, name: &str, text: &str) -> Result<(), ProjectError> {
    let temp = stage(dir, text)?;
    let path = dir.join(name);
    std::fs::rename(&temp, &path).map_err(|e| io(&path, e))?;
    sync_dir(dir)
}

fn parse_name(name: &str) -> Option<(Kind, u64)> {
    let stem = name.strip_suffix(".checkpoint")?;
    let (kind, rest) = stem.split_once('-')?;
    let (sequence, id) = rest.split_once('-')?;
    if sequence.len() != 20 || uuid::Uuid::parse_str(id).ok()?.to_string() != id {
        return None;
    }
    let kind = match kind {
        "p" => Kind::Periodic,
        "i" => Kind::Interim,
        _ => return None,
    };
    Some((kind, sequence.parse().ok()?))
}

/// Decode a selected immutable checkpoint without requiring writable
/// recovery storage. This function never changes the saved project.
pub fn read_choice(root: &Path, id: &str, project_id: &str) -> Result<Snapshot, ProjectError> {
    let (kind, _) = parse_name(id).ok_or_else(|| invalid("invalid recovery choice"))?;
    let dir = root.join(DIRECTORY);
    if std::fs::symlink_metadata(&dir)
        .map_err(|e| io(&dir, e))?
        .file_type()
        .is_symlink()
    {
        return Err(invalid("recovery directory is a symbolic link"));
    }
    if read_text(&dir.join("identity"))?.trim() != project_id {
        return Err(invalid("recovery folder identity changed"));
    }
    let snapshot = Snapshot::decode(&read_text(&dir.join(id))?)?;
    if snapshot.metadata().project_id != project_id || snapshot.metadata().kind != kind {
        return Err(invalid("recovery choice identity or type changed"));
    }
    Ok(snapshot)
}

fn scan(dir: &Path, identity: &str, state: &State) -> Result<Catalog, ProjectError> {
    let mut catalog = Catalog {
        project_id: Some(identity.to_owned()),
        ..Catalog::default()
    };
    for item in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let item = item.map_err(|e| io(dir, e))?;
        let name = item.file_name().to_string_lossy().into_owned();
        if name.ends_with(".part") {
            catalog
                .problems
                .push(format!("Incomplete recovery file '{name}' was preserved."));
            continue;
        }
        if name.ends_with(".corrupt") {
            catalog
                .problems
                .push(format!("Corrupt recovery metadata '{name}' was preserved."));
            continue;
        }
        if !name.ends_with(".checkpoint") {
            continue;
        }
        let read = || {
            let (kind, sequence) =
                parse_name(&name).ok_or_else(|| invalid("invalid checkpoint filename"))?;
            let (snapshot, project) = Snapshot::decode_with_project(&read_text(&item.path())?)?;
            let metadata = snapshot.metadata().clone();
            if metadata.project_id != identity || metadata.kind != kind {
                return Err(invalid(
                    "checkpoint identity or retention type does not match this folder",
                ));
            }
            let retired = state
                .retired
                .get(&metadata.session_id)
                .is_some_and(|r| metadata.revision <= *r);
            Ok(Entry {
                id: name.clone(),
                project_name: project.name().to_owned(),
                sequence,
                metadata,
                retired,
            })
        };
        match read() {
            Ok(entry) => catalog.snapshots.push(entry),
            Err(e) => catalog
                .problems
                .push(format!("Cannot recover '{name}': {e}. File preserved.")),
        }
    }
    catalog
        .snapshots
        .sort_by(|a, b| b.sequence.cmp(&a.sequence).then_with(|| a.id.cmp(&b.id)));
    catalog.recovery_needed = !state.clean
        && catalog
            .snapshots
            .iter()
            .any(|s| !s.retired && s.sequence > state.dismissed_through);
    Ok(catalog)
}

impl Catalog {
    /// Inspect without taking ownership or rewriting recovery data. A live
    /// writer suppresses crash prompts but does not hide retained choices.
    pub fn read(root: &Path) -> Result<Self, ProjectError> {
        let dir = root.join(DIRECTORY);
        if !dir.exists() {
            return Ok(Self::default());
        }
        if std::fs::symlink_metadata(&dir)
            .map_err(|e| io(&dir, e))?
            .file_type()
            .is_symlink()
        {
            return Err(invalid("recovery directory is a symbolic link"));
        }
        let identity = read_text(&dir.join("identity"))?;
        let (state, problem) = match state(&dir) {
            Ok(s) => (s, None),
            Err(e) => (State::default(), Some(e.to_string())),
        };
        let mut catalog = scan(&dir, identity.trim(), &state)?;
        if let Some(problem) = problem {
            catalog.problems.push(problem);
        }
        let lock_path = dir.join("writer.lock");
        if lock_path.exists() {
            let file = File::open(&lock_path).map_err(|e| io(&lock_path, e))?;
            catalog.live = file.try_lock_shared().is_err();
        }
        if catalog.live {
            catalog.recovery_needed = false;
        }
        Ok(catalog)
    }
}

impl Store {
    /// Acquire portable OS ownership; lock files remain but cannot stay
    /// locked after a process exits. Only recovery files are created here.
    pub fn open(root: &Path) -> Result<Self, ProjectError> {
        let dir = root.join(DIRECTORY);
        if let Ok(meta) = std::fs::symlink_metadata(&dir)
            && meta.file_type().is_symlink()
        {
            return Err(invalid("recovery directory is a symbolic link"));
        }
        std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
        // Match the cache's self-ignoring convention without changing any
        // user-owned .gitignore or tracked project file.
        replace(
            &dir,
            ".gitignore",
            "# Local project recovery; do not commit.\n*\n",
        )?;
        let lock_path = dir.join("writer.lock");
        if std::fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(invalid("recovery writer lock is a symbolic link"));
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|e| io(&lock_path, e))?;
        lock.try_lock().map_err(|e| {
            invalid(format!(
                "another application owns recovery for '{}': {e}",
                root.display()
            ))
        })?;
        let path = dir.join("identity");
        let identity = match read_text(&path) {
            Ok(s) => s.trim().to_owned(),
            Err(ProjectError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                let id = uuid::Uuid::new_v4().to_string();
                replace(&dir, "identity", &id)?;
                id
            }
            Err(e) => return Err(e),
        };
        uuid::Uuid::parse_str(&identity).map_err(invalid)?;
        let mut state = match state(&dir) {
            Ok(state) => state,
            Err(error) => {
                let original = dir.join("state.toml");
                let preserved = dir.join(format!("state-{}.corrupt", uuid::Uuid::new_v4()));
                std::fs::rename(&original, &preserved).map_err(|source| io(&original, source))?;
                // The raw metadata and a diagnostic are preserved. Treat all
                // intact snapshots as unretired rather than hiding recovery.
                let diagnostic = dir.join(format!("state-{}.corrupt", uuid::Uuid::new_v4()));
                std::fs::write(&diagnostic, error.to_string())
                    .map_err(|source| io(&diagnostic, source))?;
                State::default()
            }
        };
        state.clean = false;
        let store = Self {
            dir,
            identity,
            session: uuid::Uuid::new_v4().to_string(),
            state,
            closed: false,
            _lock: lock,
        };
        store.save_state()?;
        Ok(store)
    }

    /// Capture metadata for this owner's session.
    pub fn metadata(&self, kind: Kind, revision: u64, timestamp_ms: u64) -> Metadata {
        Metadata {
            project_id: self.identity.clone(),
            session_id: self.session.clone(),
            kind,
            revision,
            timestamp_ms,
        }
    }

    fn save_state(&self) -> Result<(), ProjectError> {
        replace(
            &self.dir,
            "state.toml",
            &toml::to_string(&self.state).map_err(invalid)?,
        )
    }

    /// Validate choices while retaining the writer lease.
    pub fn catalog(&self) -> Result<Catalog, ProjectError> {
        scan(&self.dir, &self.identity, &self.state)
    }

    /// Load one selected immutable checkpoint into memory.
    pub fn read(&self, id: &str) -> Result<Snapshot, ProjectError> {
        let root = self
            .dir
            .parent()
            .ok_or_else(|| invalid("recovery folder has no project parent"))?;
        read_choice(root, id, &self.identity)
    }

    /// Flush a complete container, publish its unique name, then retain three
    /// periodic states and one interim state. Never rotate corrupt data.
    pub fn publish(&mut self, snapshot: &Snapshot) -> Result<Entry, ProjectError> {
        self.publish_steps(snapshot, |_| {})
    }

    fn publish_steps(
        &mut self,
        snapshot: &Snapshot,
        mut after: impl FnMut(&str),
    ) -> Result<Entry, ProjectError> {
        if self.closed
            || snapshot.metadata().session_id != self.session
            || snapshot.metadata().project_id != self.identity
        {
            return Err(invalid("stale or foreign recovery writer"));
        }
        let existing = self.catalog()?;
        if existing.snapshots.iter().any(|entry| {
            entry.metadata.session_id == self.session
                && entry.metadata.kind == snapshot.metadata().kind
                && entry.metadata.revision > snapshot.metadata().revision
        }) {
            return Err(invalid("a newer checkpoint is already published"));
        }
        if let Some(entry) = existing.snapshots.iter().find(|entry| {
            entry.metadata.session_id == self.session
                && entry.metadata.kind == snapshot.metadata().kind
                && entry.metadata.revision == snapshot.metadata().revision
        }) {
            if self.read(&entry.id)?.files() != snapshot.files() {
                return Err(invalid(
                    "checkpoint revision was reused for different project contents",
                ));
            }
            // A previous attempt may have published successfully and then
            // failed in cleanup. Retry cleanup, not another full write.
            self.rotate(&existing, &mut after)?;
            return Ok(entry.clone());
        }
        let mut sequence = self.state.sequence;
        for item in std::fs::read_dir(&self.dir).map_err(|e| io(&self.dir, e))? {
            let item = item.map_err(|e| io(&self.dir, e))?;
            if let Some((_, n)) = parse_name(&item.file_name().to_string_lossy()) {
                sequence = sequence.max(n);
            }
        }
        sequence = sequence
            .checked_add(1)
            .ok_or_else(|| invalid("recovery sequence exhausted"))?;
        let kind = match snapshot.metadata().kind {
            Kind::Periodic => "p",
            Kind::Interim => "i",
        };
        let id = format!("{kind}-{sequence:020}-{}.checkpoint", uuid::Uuid::new_v4());
        // Reserve an ordering token durably, even if all older files were
        // discarded or this write fails. A dismissal watermark cannot hide
        // future checkpoints after a sequence reset.
        self.state.sequence = sequence;
        self.state.clean = false;
        self.save_state()?;
        let temp = stage(&self.dir, &snapshot.encode()?)?;
        after("staged");
        let path = self.dir.join(&id);
        std::fs::rename(&temp, &path).map_err(|e| io(&path, e))?;
        sync_dir(&self.dir)?;
        after("published");
        let catalog = self.catalog()?;
        let entry = catalog
            .snapshots
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or_else(|| invalid("published checkpoint was not validated"))?;
        self.rotate(&catalog, &mut after)?;
        Ok(entry)
    }

    fn rotate(&self, catalog: &Catalog, after: &mut impl FnMut(&str)) -> Result<(), ProjectError> {
        let mut periodic = 0;
        let mut interim = 0;
        for item in &catalog.snapshots {
            let excess = match item.metadata.kind {
                Kind::Periodic => {
                    periodic += 1;
                    periodic > 3
                }
                Kind::Interim => {
                    interim += 1;
                    interim > 1
                }
            };
            if excess {
                let path = self.dir.join(&item.id);
                std::fs::remove_file(&path).map_err(|e| io(&path, e))?;
                after("rotation");
            }
        }
        sync_dir(&self.dir)?;
        Ok(())
    }

    /// Retire only states covered by a successful explicit Save. An older
    /// Save completion cannot retire a newer committed checkpoint.
    pub fn retire_saved(&mut self, revision: u64) -> Result<(), ProjectError> {
        for entry in self.catalog()?.snapshots {
            let limit = if entry.metadata.session_id == self.session {
                revision
            } else {
                entry.metadata.revision
            };
            self.state
                .retired
                .entry(entry.metadata.session_id)
                .and_modify(|r| *r = (*r).max(limit))
                .or_insert(limit);
        }
        self.state
            .retired
            .entry(self.session.clone())
            .and_modify(|r| *r = (*r).max(revision))
            .or_insert(revision);
        self.save_state()
    }

    /// Mark a resolved clean exit, retaining snapshots as history choices.
    pub fn close(&mut self, revision: u64) -> Result<(), ProjectError> {
        self.retire_saved(revision)?;
        self.state.clean = true;
        self.save_state()?;
        self.closed = true;
        Ok(())
    }

    /// Keep the saved project: dismiss existing choices without deleting them.
    pub fn dismiss(&mut self) -> Result<(), ProjectError> {
        if let Some(max) = self.catalog()?.snapshots.iter().map(|e| e.sequence).max() {
            self.state.dismissed_through = self.state.dismissed_through.max(max);
        }
        self.save_state()
    }

    /// Explicitly discard recovery data. Never invoked for Keep saved or
    /// reader errors. Only validated checkpoint basenames are removed.
    pub fn discard(&mut self) -> Result<(), ProjectError> {
        for entry in self.catalog()?.snapshots {
            let path = self.dir.join(entry.id);
            std::fs::remove_file(&path).map_err(|e| io(&path, e))?;
        }
        self.state.clean = true;
        self.save_state()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::Project;
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    // This test is also the isolated child entry point. The parent kills
    // the process while it is blocked at a publication boundary, so neither
    // unwinding nor a clean Store::close can make the crash look successful.
    #[test]
    fn req_prj_08_forced_termination_child() {
        let Some(root) = std::env::var_os("OPENCHUCK_CHECKPOINT_CRASH_ROOT") else {
            return;
        };
        let stop = std::env::var("OPENCHUCK_CHECKPOINT_CRASH_STAGE").unwrap();
        let mut store = Store::open(Path::new(&root)).unwrap();
        let snapshot = Snapshot::capture(
            &Project::new("new state"),
            store.metadata(Kind::Periodic, 4, 999),
        )
        .unwrap();
        store
            .publish_steps(&snapshot, |stage| {
                if stage == stop {
                    println!("STOP {stage}");
                    std::io::stdout().flush().unwrap();
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap();
                }
            })
            .unwrap();
    }

    #[test]
    fn req_prj_08_forced_termination_during_publication_and_rotation_preserves_good_choices() {
        for phase in ["staged", "published", "rotation"] {
            let root =
                std::env::temp_dir().join(format!("openchuck-crash-{}", uuid::Uuid::new_v4()));
            {
                let mut store = Store::open(&root).unwrap();
                for rev in 1..=3 {
                    let snapshot = Snapshot::capture(
                        &Project::new(format!("old {rev}")),
                        store.metadata(Kind::Periodic, rev, rev),
                    )
                    .unwrap();
                    store.publish(&snapshot).unwrap();
                }
            }
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "recovery::store::tests::req_prj_08_forced_termination_child",
                    "--nocapture",
                ])
                .env("OPENCHUCK_CHECKPOINT_CRASH_ROOT", &root)
                .env("OPENCHUCK_CHECKPOINT_CRASH_STAGE", phase)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
            let mut output = BufReader::new(child.stdout.take().unwrap());
            let mut stopped = false;
            for line in output.by_ref().lines() {
                let line = line.unwrap();
                if line == format!("STOP {phase}") {
                    stopped = true;
                    break;
                }
            }
            child.kill().unwrap();
            assert!(!child.wait().unwrap().success());
            assert!(stopped, "child did not reach {phase}");
            let catalog = Catalog::read(&root).unwrap();
            assert!(!catalog.live);
            assert!(catalog.recovery_needed);
            assert!(catalog.snapshots.len() >= 3);
            let mut store = Store::open(&root).unwrap();
            for choice in &catalog.snapshots {
                assert!(store.read(&choice.id).unwrap().project().is_ok());
            }
            let snapshot = Snapshot::capture(
                &Project::new("after crash"),
                store.metadata(Kind::Periodic, 5, 1000),
            )
            .unwrap();
            store.publish(&snapshot).unwrap();
            assert_eq!(store.catalog().unwrap().snapshots.len(), 3);
            drop(store);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
