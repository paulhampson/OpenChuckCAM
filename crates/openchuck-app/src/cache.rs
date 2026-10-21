// SPDX-License-Identifier: AGPL-3.0-or-later
//! Operation pipeline caches (architecture section 6; work breakdown slice 9
//! item 2).
//!
//! Each operation's result is kept (the latest few, see
//! [`DEFAULT_CACHE_VERSIONS`]) under a key: a hash of everything the
//! operation's generation reads, the engine version, and the previous
//! operation's key. Chaining the keys makes editing operation N change the
//! key of N and of every later operation, and of none before it, so exactly
//! those are regenerated. With a result the cache keeps the stock the
//! operation leaves in its setup's simulation, once a run has simulated it,
//! so an operation taken from the cache is not run through the stock again
//! (decision record 0190; issue 239). The key fixes that stock too: it
//! chains every operation before, and covers the setup, the machine and the
//! simulation settings. The cache lives in memory and is never written to
//! the project (PRJ-03).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use openchuck_project::id::{ModelId, OperationId};
use openchuck_project::{ContentHash, ModelRef, Operation, OrderKey, Project};

use crate::AppError;
use crate::operation::OperationPath;
use crate::simulation::SetupState;

/// The engine version that goes into every key, so a new build never reuses
/// results from an older one.
const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cache key of one operation's result.
pub(crate) type Key = ContentHash;

/// How many results are kept for each operation unless the application asks
/// for another number: the current one plus two earlier states, enough for
/// a couple of undo steps or a parameter toggled back and forth, while a long
/// toolpath is held at most three times.
pub const DEFAULT_CACHE_VERSIONS: usize = 3;

/// One cached result of an operation.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// The operation's path.
    pub(crate) path: Arc<OperationPath>,
    /// Its setup's simulation just after it, once a run has simulated it.
    pub(crate) stock: Option<Arc<SetupState>>,
}

/// Results of one operation, most recently used first.
type Versions = Vec<(Key, Entry)>;

/// The entries and the number kept, under one lock so that changing the
/// number and storing a result cannot overtake each other.
#[derive(Debug)]
struct State {
    versions: usize,
    /// Counts the times the cache was cleared, so that a job started before
    /// a clear cannot store a result made under what was cleared.
    generation: u64,
    entries: BTreeMap<OperationId, Versions>,
}

/// Cached results by operation, shared between the application and its jobs.
#[derive(Debug, Clone)]
pub(crate) struct PipelineCache {
    state: Arc<Mutex<State>>,
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                versions: DEFAULT_CACHE_VERSIONS,
                generation: 0,
                entries: BTreeMap::new(),
            })),
        }
    }
}

impl PipelineCache {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A poisoned lock only means a job panicked; the state is still valid.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn versions(&self) -> usize {
        self.lock().versions
    }

    /// Sets how many results are kept per operation; 0 turns caching off.
    /// Results beyond the new number are dropped, oldest first.
    pub(crate) fn set_versions(&self, count: usize) {
        let mut state = self.lock();
        state.versions = count;
        for versions in state.entries.values_mut() {
            versions.truncate(count);
        }
    }

    /// The result stored for `id` under `key`, now the most recently used.
    /// Nothing is returned while caching is off.
    pub(crate) fn get(&self, id: OperationId, key: Key) -> Option<Entry> {
        let mut state = self.lock();
        if state.versions == 0 {
            return None;
        }
        let versions = state.entries.get_mut(&id)?;
        let at = versions.iter().position(|(k, _)| *k == key)?;
        let hit = versions.remove(at);
        let value = hit.1.clone();
        versions.insert(0, hit);
        Some(value)
    }

    /// The number of clears so far; a job reads it when it starts and hands
    /// it to [`PipelineCache::put`].
    pub(crate) fn generation(&self) -> u64 {
        self.lock().generation
    }

    /// Stores a result. Nothing is stored if the cache was cleared since
    /// `generation` was read, because the result may predate the change.
    pub(crate) fn put(
        &self,
        id: OperationId,
        key: Key,
        value: Arc<OperationPath>,
        generation: u64,
    ) {
        let mut state = self.lock();
        let keep = state.versions;
        if keep == 0 || state.generation != generation {
            return;
        }
        let versions = state.entries.entry(id).or_default();
        versions.retain(|(k, _)| *k != key);
        versions.insert(
            0,
            (
                key,
                Entry {
                    path: value,
                    stock: None,
                },
            ),
        );
        versions.truncate(keep);
    }

    /// Keeps `stock`, the setup's simulation just after the operation, with
    /// the result stored for `id` under `key`. Nothing is kept if there is no
    /// such result (it was not cached, or has been dropped since) or the cache
    /// was cleared since `generation` was read.
    pub(crate) fn put_stock(
        &self,
        id: OperationId,
        key: Key,
        stock: Arc<SetupState>,
        generation: u64,
    ) {
        let mut state = self.lock();
        if state.versions == 0 || state.generation != generation {
            return;
        }
        if let Some((_, entry)) = state
            .entries
            .get_mut(&id)
            .and_then(|versions| versions.iter_mut().find(|(k, _)| *k == key))
        {
            entry.stock = Some(stock);
        }
    }

    /// Drops results of operations that are not in `keep`.
    pub(crate) fn retain_only(&self, keep: &[OperationId]) {
        self.lock().entries.retain(|id, _| keep.contains(id));
    }

    /// Drops everything (for example when the strategies change).
    pub(crate) fn clear(&self) {
        let mut state = self.lock();
        state.generation += 1;
        state.entries.clear();
    }
}

/// The contents hash of every model file, taken once per run. A file that
/// cannot be read has no hash, and nothing made from it is cached.
#[derive(Debug, Default)]
pub(crate) struct ModelFiles(BTreeMap<ModelId, Option<ContentHash>>);

impl ModelFiles {
    pub(crate) fn read(root: &Path, project: &Project) -> Self {
        Self(
            project
                .models()
                .iter()
                .map(|m| (m.id, ContentHash::of_file(&m.path.resolve(root)).ok()))
                .collect(),
        )
    }

    /// The hashes the project recorded for its model files, without reading
    /// them: what the status keys use, so they can be worked out on the
    /// interface's thread (decision record 0117).
    pub(crate) fn recorded(project: &Project) -> Self {
        Self(
            project
                .models()
                .iter()
                .map(|m| (m.id, Some(m.hash)))
                .collect(),
        )
    }

    /// The part model of `operation`, if it has one.
    fn part<'a>(project: &'a Project, operation: &Operation) -> Option<&'a ModelRef> {
        let id = project.setup(operation.setup)?.part?;
        project.models().iter().find(|m| m.id == id)
    }

    /// Whether every model that an operation uses as its part still has the
    /// hash it had at the start of the run. Each file is read once.
    pub(crate) fn parts_unchanged(&self, root: &Path, project: &Project) -> bool {
        let used: std::collections::BTreeSet<ModelId> = project
            .operations()
            .iter()
            .filter_map(|o| Self::part(project, o))
            .map(|m| m.id)
            .collect();
        project
            .models()
            .iter()
            .filter(|m| used.contains(&m.id))
            .all(|m| match self.0.get(&m.id) {
                Some(Some(before)) => {
                    ContentHash::of_file(&m.path.resolve(root)).ok() == Some(*before)
                }
                _ => true,
            })
    }

    /// Whether what `operation` generates from can be cached: its part file
    /// was hashed, and still has that hash. Generation reads the file itself,
    /// so a file that changed while it ran must not be stored under the key
    /// made from the old contents.
    pub(crate) fn unchanged(&self, root: &Path, project: &Project, operation: &Operation) -> bool {
        let Some(model) = Self::part(project, operation) else {
            return false;
        };
        match self.0.get(&model.id) {
            Some(Some(before)) => {
                ContentHash::of_file(&model.path.resolve(root)).ok() == Some(*before)
            }
            _ => false,
        }
    }
}

fn feed<T: serde::Serialize + ?Sized>(
    hasher: &mut Vec<u8>,
    label: &str,
    value: &T,
) -> Result<(), AppError> {
    let json = serde_json::to_vec(value)
        .map_err(|e| AppError::Cache(format!("cannot hash {label}: {e}")))?;
    hasher.extend_from_slice(label.as_bytes());
    hasher.extend_from_slice(&(json.len() as u64).to_le_bytes());
    hasher.extend_from_slice(&json);
    Ok(())
}

/// The key of `operation`, given the key of the operation before it (`None`
/// for the first). It covers the operation, its setup, the machine, its
/// tool snapshot, its part's reference and the part file's contents as
/// hashed at the start of the run, the selection sets it uses, the strategy's name and the engine
/// version.
///
/// A missing piece is hashed as absent: generation then fails with its own
/// message, and failures are never cached.
pub(crate) fn operation_key(
    project: &Project,
    operation: &Operation,
    previous: Option<Key>,
    files: &ModelFiles,
) -> Result<Key, AppError> {
    let mut bytes = Vec::new();
    feed(&mut bytes, "engine", ENGINE_VERSION)?;
    feed(
        &mut bytes,
        "previous",
        &previous.map(|k| k.as_bytes().to_vec()),
    )?;
    // What the operation is called, where it sits in the list and whether it is
    // on are not inputs to its path: the name is only a label (the post reads
    // it when writing, and the path's tags carry the ID), the position reaches
    // the key through `previous`, and a disabled operation has no key. Leaving
    // them out keeps a rename from making the operation stale (issue 209).
    let mut operation_inputs = operation.clone();
    operation_inputs.name.clear();
    operation_inputs.order = OrderKey::default();
    operation_inputs.enabled = true;
    feed(&mut bytes, "operation", &operation_inputs)?;
    // The same for the setup's name and position.
    let setup = project.setup(operation.setup).map(|setup| {
        let mut setup = setup.clone();
        setup.name.clear();
        setup.order = OrderKey::default();
        setup
    });
    feed(&mut bytes, "setup", &setup)?;
    feed(&mut bytes, "machine", &project.machine())?;
    feed(&mut bytes, "simulation", &project.simulation())?;
    let tool = operation
        .tool
        .and_then(|id| project.tools().iter().find(|t| t.id == id));
    // The controller mapping (tool number and offsets) is read when posting, not
    // when the path is made, so renumbering tools (GC-14) leaves results valid.
    // The name and the position in the list are labels, as an operation's are
    // (OI-693): the post reads the name when writing.
    let tool = tool.map(|t| {
        let mut t = t.clone();
        t.name.clear();
        t.order = OrderKey::default();
        t.controller = openchuck_types::tool::ControllerMapping {
            number: 0,
            length_offset: None,
            diameter_offset: None,
        };
        t
    });
    feed(&mut bytes, "tool", &tool)?;
    let model = ModelFiles::part(project, operation);
    feed(&mut bytes, "model", &model)?;
    // The file itself, not only the hash the project recorded: a changed file
    // must not reuse an old result.
    let file = model
        .and_then(|m| files.0.get(&m.id))
        .and_then(|h| h.as_ref())
        .map(|h| h.as_bytes().to_vec());
    feed(&mut bytes, "model file", &file)?;
    // The sets the operation uses in each role (UI-05): editing one changes
    // the key. A set that cannot be read is hashed as absent; generation then
    // stops with its own message and failures are never cached.
    let sets = openchuck_project::file::selection_inputs_hash(project, operation.id)
        .ok()
        .map(|h| h.as_bytes().to_vec());
    feed(&mut bytes, "selections", &sets)?;
    Ok(ContentHash::of_bytes(&bytes))
}

/// The keys that say whether an operation is up to date, for the item states
/// of the project tree (UI-11; decision record 0117).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StatusKey {
    /// The key chained from the enabled operations before it, as
    /// [`operation_key`] makes it, with the model files taken at their
    /// recorded hashes: it changes when the operation or anything before it
    /// changes.
    pub(crate) chained: Key,
    /// The key of the operation on its own: it changes only when the
    /// operation's own inputs change, which says which operation made a
    /// later one stale.
    pub(crate) own: Key,
}

/// The status keys of `project`'s enabled operations, in machining order.
/// Reads no file. An operation whose inputs cannot be hashed is left out,
/// and reads as not generated.
pub(crate) fn status_keys(project: &Project) -> Vec<(OperationId, StatusKey)> {
    let files = ModelFiles::recorded(project);
    let mut previous = None;
    let mut keys = Vec::new();
    for operation in project.operations().iter().filter(|o| o.enabled) {
        let (Ok(chained), Ok(own)) = (
            operation_key(project, operation, previous, &files),
            operation_key(project, operation, None, &files),
        ) else {
            continue;
        };
        previous = Some(chained);
        keys.push((operation.id, StatusKey { chained, own }));
    }
    keys
}
