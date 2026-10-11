// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project document model (architecture section 5.6).
//!
//! A project holds models, setups, tool snapshots, selection sets and
//! operations, each with a stable unique ID (PRJ-05), plus a machine reference.
//! Items are kept in their user-visible order. Spike SP-8 chose TOML files with
//! a per-item order key (decision record 0006); persistence and order keys are
//! not built yet (decision record 0007, OI-37).
//!
//! Every change goes through an edit method that keeps the document valid: IDs
//! stay unique, references point at items that exist, and an item in use cannot
//! be removed. [`Project::validate`] checks the same rules for a whole document,
//! for data that did not come through the edit methods.
//!
//! The project stores no generated data (PRJ-03): no toolpaths, simulation
//! results or G-code. Those live in the cache folder ([`crate::cache`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use openchuck_types::brep::{EdgeId, FaceId};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::hash::ContentHash;
use crate::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};

/// The machine a project is set up for.
///
/// A placeholder until slice 3 settles how a project refers to its machine
/// profile (SET-01, open item OI-38).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineRef {
    /// The machine profile's name.
    pub name: String,
}

/// A model file referenced by the project (PRJ-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRef {
    /// Stable ID.
    pub id: ModelId,
    /// Path relative to the project folder.
    pub path: RelativePath,
    /// Hash of the file's contents when it was added or last accepted.
    pub hash: ContentHash,
}

/// A path relative to the project folder, written with `/` separators on every
/// platform so the project file is the same everywhere.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelativePath(String);

impl RelativePath {
    /// Checks and wraps `text`. It must be non-empty, relative, use `/` as the
    /// separator, and contain no empty or `.` components.
    pub fn new(text: impl Into<String>) -> Result<Self, ProjectError> {
        let text = text.into();
        let invalid = |reason| ProjectError::InvalidPath {
            path: text.clone(),
            reason,
        };
        if text.is_empty() {
            return Err(invalid("the path is empty"));
        }
        if text.contains('\\') {
            return Err(invalid("use '/' as the separator"));
        }
        if text.starts_with('/') || text.as_bytes().get(1) == Some(&b':') {
            return Err(invalid("the path must be relative to the project folder"));
        }
        if text.split('/').any(|part| part.is_empty() || part == ".") {
            return Err(invalid("the path has an empty or '.' component"));
        }
        Ok(Self(text))
    }

    /// Converts a relative path on this platform, such as `models\part.stl`
    /// on Windows, to the project's form.
    pub fn from_path(path: &Path) -> Result<Self, ProjectError> {
        let mut parts = Vec::new();
        for component in path.components() {
            match component {
                Component::Normal(part) => match part.to_str() {
                    Some(part) => parts.push(part.to_owned()),
                    None => {
                        return Err(ProjectError::InvalidPath {
                            path: path.display().to_string(),
                            reason: "the path is not valid Unicode",
                        });
                    }
                },
                Component::ParentDir => parts.push("..".to_owned()),
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) => {
                    return Err(ProjectError::InvalidPath {
                        path: path.display().to_string(),
                        reason: "the path must be relative to the project folder",
                    });
                }
            }
        }
        Self::new(parts.join("/"))
    }

    /// The path as written in the project.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The file's location on this platform, under `project_root`.
    pub fn resolve(&self, project_root: &Path) -> PathBuf {
        self.0
            .split('/')
            .fold(project_root.to_path_buf(), |path, part| path.join(part))
    }
}

impl TryFrom<String> for RelativePath {
    type Error = ProjectError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl From<RelativePath> for String {
    fn from(path: RelativePath) -> Self {
        path.0
    }
}

/// The result of checking a model file against its recorded hash (PRJ-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCheck {
    /// The file matches the recorded hash.
    Unchanged,
    /// The file has changed since it was recorded.
    Changed {
        /// The hash in the project.
        recorded: ContentHash,
        /// The hash of the file now.
        found: ContentHash,
    },
    /// The file is not there.
    Missing,
}

impl ModelRef {
    /// Checks the file under `project_root` against the recorded hash.
    pub fn check(&self, project_root: &Path) -> Result<ModelCheck, ProjectError> {
        let path = self.path.resolve(project_root);
        match ContentHash::of_file(&path) {
            Ok(found) if found == self.hash => Ok(ModelCheck::Unchanged),
            Ok(found) => Ok(ModelCheck::Changed {
                recorded: self.hash,
                found,
            }),
            Err(ProjectError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(ModelCheck::Missing)
            }
            Err(e) => Err(e),
        }
    }
}

/// A setup: one way the stock is held on the machine. Stock, work offset and
/// the rest arrive in slice 3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// Stable ID.
    pub id: SetupId,
    /// Name shown to the user.
    pub name: String,
}

/// A copy of a tool kept in the project, so the project does not change when
/// the tool library does (TL-07).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolSnapshot {
    /// Stable ID.
    pub id: ToolId,
    /// Name shown to the user.
    pub name: String,
    /// The tool's geometry.
    pub tool: Tool,
}

/// What a selection set selects on its model (PRJ-06).
///
/// B-rep models are selected by face or edge, whose IDs are stable for the same
/// file (architecture section 5.4). Mesh models are selected by triangle index;
/// triangle selections apply to mesh models only, because a B-rep model's
/// triangles change with the tessellation tolerance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionTarget {
    /// B-rep faces.
    Faces(BTreeSet<FaceId>),
    /// B-rep edges.
    Edges(BTreeSet<EdgeId>),
    /// Mesh triangles by index.
    Triangles(BTreeSet<u32>),
}

/// Where a selected entity was, for matching it again after the model is
/// re-exported (PRJ-07, P2).
///
/// Reserved now so the P2 work needs no format change. P1 writes none and
/// keeps any it reads.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    /// A representative point on the entity.
    pub position: Point3,
    /// The surface normal there (zero for an edge).
    pub normal: Vec3,
    /// Area of a face, or length of an edge.
    pub area: f64,
}

/// A named selection that operations refer to by ID (UI-05), so editing the
/// set updates every operation that uses it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionSet {
    /// Stable ID.
    pub id: SelectionSetId,
    /// Name shown to the user.
    pub name: String,
    /// The model selected on.
    pub model: ModelId,
    /// The model's content hash when the selection was made. Face and edge IDs
    /// are relative to this version of the file (architecture section 5.6).
    pub model_hash: ContentHash,
    /// What is selected.
    pub target: SelectionTarget,
    /// Reserved for P2 re-matching (PRJ-07); empty in P1.
    pub descriptors: Vec<Descriptor>,
}

/// A parameter value of an operation. The strategy's parameter schema (OP-16)
/// gives each its meaning and unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterValue {
    /// A number in the unit the schema gives (millimetres, degrees, …).
    Number(f64),
    /// A whole number.
    Integer(i64),
    /// An on or off choice.
    Flag(bool),
    /// Text, including a choice from a list.
    Text(String),
}

/// One machining operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// Stable ID.
    pub id: OperationId,
    /// Name shown to the user.
    pub name: String,
    /// The setup it machines in.
    pub setup: SetupId,
    /// The strategy's registry name (OP-16).
    pub strategy: String,
    /// The tool it cuts with, once chosen.
    pub tool: Option<ToolId>,
    /// The selection sets it machines, in order.
    pub selections: Vec<SelectionSetId>,
    /// Parameter values by name, in name order.
    pub parameters: BTreeMap<String, ParameterValue>,
}

/// Any item with a stable ID, for the generic edits.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A model reference.
    Model(ModelRef),
    /// A setup.
    Setup(Setup),
    /// A tool snapshot.
    Tool(ToolSnapshot),
    /// A selection set.
    SelectionSet(SelectionSet),
    /// An operation.
    Operation(Operation),
}

/// The ID of any item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemId {
    /// A model reference.
    Model(ModelId),
    /// A setup.
    Setup(SetupId),
    /// A tool snapshot.
    Tool(ToolId),
    /// A selection set.
    SelectionSet(SelectionSetId),
    /// An operation.
    Operation(OperationId),
}

impl Item {
    /// The item's ID.
    pub fn id(&self) -> ItemId {
        match self {
            Self::Model(m) => ItemId::Model(m.id),
            Self::Setup(s) => ItemId::Setup(s.id),
            Self::Tool(t) => ItemId::Tool(t.id),
            Self::SelectionSet(s) => ItemId::SelectionSet(s.id),
            Self::Operation(o) => ItemId::Operation(o.id),
        }
    }
}

impl ItemId {
    /// What kind of item this names.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Model(_) => ModelId::KIND,
            Self::Setup(_) => SetupId::KIND,
            Self::Tool(_) => ToolId::KIND,
            Self::SelectionSet(_) => SelectionSetId::KIND,
            Self::Operation(_) => OperationId::KIND,
        }
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model(id) => write!(f, "model {id}"),
            Self::Setup(id) => write!(f, "setup {id}"),
            Self::Tool(id) => write!(f, "tool {id}"),
            Self::SelectionSet(id) => write!(f, "selection set {id}"),
            Self::Operation(id) => write!(f, "operation {id}"),
        }
    }
}

/// A project document.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Project {
    name: String,
    machine: Option<MachineRef>,
    models: Vec<ModelRef>,
    setups: Vec<Setup>,
    tools: Vec<ToolSnapshot>,
    selection_sets: Vec<SelectionSet>,
    operations: Vec<Operation>,
}

fn not_found(id: ItemId) -> ProjectError {
    ProjectError::NotFound {
        kind: id.kind(),
        id: id_text(id),
    }
}

fn id_text(id: ItemId) -> String {
    match id {
        ItemId::Model(id) => id.to_string(),
        ItemId::Setup(id) => id.to_string(),
        ItemId::Tool(id) => id.to_string(),
        ItemId::SelectionSet(id) => id.to_string(),
        ItemId::Operation(id) => id.to_string(),
    }
}

fn position<T>(items: &[T], matches: impl Fn(&T) -> bool) -> Option<usize> {
    items.iter().position(matches)
}

/// Inserts at `index`, or appends when `index` is `None`.
fn insert_at<T>(
    items: &mut Vec<T>,
    index: Option<usize>,
    item: T,
    kind: &'static str,
) -> Result<usize, ProjectError> {
    let index = index.unwrap_or(items.len());
    if index > items.len() {
        return Err(ProjectError::BadIndex {
            kind,
            index,
            len: items.len(),
        });
    }
    items.insert(index, item);
    Ok(index)
}

fn move_to<T>(
    items: &mut Vec<T>,
    from: usize,
    to: usize,
    kind: &'static str,
) -> Result<(), ProjectError> {
    if to >= items.len() || from >= items.len() {
        return Err(ProjectError::BadIndex {
            kind,
            index: to,
            len: items.len(),
        });
    }
    let item = items.remove(from);
    items.insert(to, item);
    Ok(())
}

impl Project {
    /// An empty project with this name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// The project's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The machine reference, once set.
    pub fn machine(&self) -> Option<&MachineRef> {
        self.machine.as_ref()
    }

    /// Model references in order.
    pub fn models(&self) -> &[ModelRef] {
        &self.models
    }

    /// Setups in order.
    pub fn setups(&self) -> &[Setup] {
        &self.setups
    }

    /// Tool snapshots in order.
    pub fn tools(&self) -> &[ToolSnapshot] {
        &self.tools
    }

    /// Selection sets in order.
    pub fn selection_sets(&self) -> &[SelectionSet] {
        &self.selection_sets
    }

    /// Operations in machining order.
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// The operation with this ID.
    pub fn operation(&self, id: OperationId) -> Option<&Operation> {
        self.operations.iter().find(|o| o.id == id)
    }

    /// Whether an item with this ID exists.
    pub fn contains(&self, id: ItemId) -> bool {
        self.index_of(id).is_some()
    }

    /// The item's position in its list.
    pub fn index_of(&self, id: ItemId) -> Option<usize> {
        match id {
            ItemId::Model(id) => position(&self.models, |m| m.id == id),
            ItemId::Setup(id) => position(&self.setups, |s| s.id == id),
            ItemId::Tool(id) => position(&self.tools, |t| t.id == id),
            ItemId::SelectionSet(id) => position(&self.selection_sets, |s| s.id == id),
            ItemId::Operation(id) => position(&self.operations, |o| o.id == id),
        }
    }

    /// The item with this ID, cloned.
    pub fn item(&self, id: ItemId) -> Option<Item> {
        let index = self.index_of(id)?;
        match id {
            ItemId::Model(_) => self.models.get(index).cloned().map(Item::Model),
            ItemId::Setup(_) => self.setups.get(index).cloned().map(Item::Setup),
            ItemId::Tool(_) => self.tools.get(index).cloned().map(Item::Tool),
            ItemId::SelectionSet(_) => self
                .selection_sets
                .get(index)
                .cloned()
                .map(Item::SelectionSet),
            ItemId::Operation(_) => self.operations.get(index).cloned().map(Item::Operation),
        }
    }

    /// Renames the project and returns the old name.
    pub fn set_name(&mut self, name: impl Into<String>) -> String {
        std::mem::replace(&mut self.name, name.into())
    }

    /// Sets the machine reference and returns the old one.
    pub fn set_machine(&mut self, machine: Option<MachineRef>) -> Option<MachineRef> {
        std::mem::replace(&mut self.machine, machine)
    }

    /// Adds `item` at `index` in its list (at the end for `None`) and returns
    /// where it went. Fails if the ID is taken, a reference is unknown, or a
    /// value is invalid.
    pub fn insert(&mut self, item: Item, index: Option<usize>) -> Result<usize, ProjectError> {
        let id = item.id();
        if self.contains(id) {
            return Err(ProjectError::DuplicateId {
                kind: id.kind(),
                id: id_text(id),
            });
        }
        self.check_item(&item)?;
        match item {
            Item::Model(m) => insert_at(&mut self.models, index, m, ModelId::KIND),
            Item::Setup(s) => insert_at(&mut self.setups, index, s, SetupId::KIND),
            Item::Tool(t) => insert_at(&mut self.tools, index, t, ToolId::KIND),
            Item::SelectionSet(s) => {
                insert_at(&mut self.selection_sets, index, s, SelectionSetId::KIND)
            }
            Item::Operation(o) => insert_at(&mut self.operations, index, o, OperationId::KIND),
        }
    }

    /// Removes an item that nothing uses, and returns it with its old position.
    pub fn remove(&mut self, id: ItemId) -> Result<(usize, Item), ProjectError> {
        let index = self.index_of(id).ok_or_else(|| not_found(id))?;
        if let Some(by) = self.first_user_of(id) {
            return Err(ProjectError::InUse {
                kind: id.kind(),
                id: id_text(id),
                by: by.to_string(),
            });
        }
        let item = match id {
            ItemId::Model(_) => Item::Model(self.models.remove(index)),
            ItemId::Setup(_) => Item::Setup(self.setups.remove(index)),
            ItemId::Tool(_) => Item::Tool(self.tools.remove(index)),
            ItemId::SelectionSet(_) => Item::SelectionSet(self.selection_sets.remove(index)),
            ItemId::Operation(_) => Item::Operation(self.operations.remove(index)),
        };
        Ok((index, item))
    }

    /// Replaces an item with a new version that has the same ID, and returns
    /// the old version. Its position is unchanged.
    pub fn replace(&mut self, item: Item) -> Result<Item, ProjectError> {
        let id = item.id();
        let index = self.index_of(id).ok_or_else(|| not_found(id))?;
        self.check_item(&item)?;
        let old = match item {
            Item::Model(m) => self
                .models
                .get_mut(index)
                .map(|slot| Item::Model(std::mem::replace(slot, m))),
            Item::Setup(s) => self
                .setups
                .get_mut(index)
                .map(|slot| Item::Setup(std::mem::replace(slot, s))),
            Item::Tool(t) => self
                .tools
                .get_mut(index)
                .map(|slot| Item::Tool(std::mem::replace(slot, t))),
            Item::SelectionSet(s) => self
                .selection_sets
                .get_mut(index)
                .map(|slot| Item::SelectionSet(std::mem::replace(slot, s))),
            Item::Operation(o) => self
                .operations
                .get_mut(index)
                .map(|slot| Item::Operation(std::mem::replace(slot, o))),
        };
        old.ok_or_else(|| not_found(id))
    }

    /// Moves an item to `index` in its list and returns its old position.
    pub fn move_item(&mut self, id: ItemId, index: usize) -> Result<usize, ProjectError> {
        let from = self.index_of(id).ok_or_else(|| not_found(id))?;
        let kind = id.kind();
        match id {
            ItemId::Model(_) => move_to(&mut self.models, from, index, kind),
            ItemId::Setup(_) => move_to(&mut self.setups, from, index, kind),
            ItemId::Tool(_) => move_to(&mut self.tools, from, index, kind),
            ItemId::SelectionSet(_) => move_to(&mut self.selection_sets, from, index, kind),
            ItemId::Operation(_) => move_to(&mut self.operations, from, index, kind),
        }?;
        Ok(from)
    }

    /// Checks the whole document: unique IDs, known references and valid
    /// values. Edits keep these rules, so this is for documents built another
    /// way, such as a loaded file.
    pub fn validate(&self) -> Result<(), ProjectError> {
        let mut seen = BTreeSet::new();
        for item in self.items() {
            let id = item.id();
            if !seen.insert(id) {
                return Err(ProjectError::DuplicateId {
                    kind: id.kind(),
                    id: id_text(id),
                });
            }
            self.check_item(&item)?;
        }
        Ok(())
    }

    /// Every item, cloned, in list order.
    fn items(&self) -> impl Iterator<Item = Item> + '_ {
        let models = self.models.iter().cloned().map(Item::Model);
        let setups = self.setups.iter().cloned().map(Item::Setup);
        let tools = self.tools.iter().cloned().map(Item::Tool);
        let sets = self.selection_sets.iter().cloned().map(Item::SelectionSet);
        let operations = self.operations.iter().cloned().map(Item::Operation);
        models
            .chain(setups)
            .chain(tools)
            .chain(sets)
            .chain(operations)
    }

    /// Checks an item's values and references against this document.
    fn check_item(&self, item: &Item) -> Result<(), ProjectError> {
        let from = item.id();
        let require = |target: ItemId| {
            if self.contains(target) {
                Ok(())
            } else {
                Err(ProjectError::UnknownReference {
                    from: from.to_string(),
                    kind: target.kind(),
                    id: id_text(target),
                })
            }
        };
        match item {
            Item::Model(_) | Item::Setup(_) => Ok(()),
            Item::Tool(t) => Ok(t.tool.validate()?),
            Item::SelectionSet(s) => {
                require(ItemId::Model(s.model))?;
                for (i, d) in s.descriptors.iter().enumerate() {
                    let field = format!("selection set {}.descriptors[{i}]", s.id);
                    d.position.validate(&format!("{field}.position"))?;
                    let finite = [d.normal.x, d.normal.y, d.normal.z, d.area]
                        .iter()
                        .all(|v| v.is_finite());
                    if !finite {
                        return Err(openchuck_types::Error::invalid(
                            field,
                            "a descriptor value is not a finite number",
                        )
                        .into());
                    }
                }
                Ok(())
            }
            Item::Operation(o) => {
                require(ItemId::Setup(o.setup))?;
                if let Some(tool) = o.tool {
                    require(ItemId::Tool(tool))?;
                }
                for set in &o.selections {
                    require(ItemId::SelectionSet(*set))?;
                }
                for (name, value) in &o.parameters {
                    if let ParameterValue::Number(v) = value
                        && !v.is_finite()
                    {
                        return Err(openchuck_types::Error::invalid(
                            format!("operation {}.parameters.{name}", o.id),
                            format!("{v} is not a finite number"),
                        )
                        .into());
                    }
                }
                Ok(())
            }
        }
    }

    /// The first item that refers to `id`, if any.
    fn first_user_of(&self, id: ItemId) -> Option<ItemId> {
        match id {
            ItemId::Model(model) => self
                .selection_sets
                .iter()
                .find(|s| s.model == model)
                .map(|s| ItemId::SelectionSet(s.id)),
            ItemId::Setup(setup) => self
                .operations
                .iter()
                .find(|o| o.setup == setup)
                .map(|o| ItemId::Operation(o.id)),
            ItemId::Tool(tool) => self
                .operations
                .iter()
                .find(|o| o.tool == Some(tool))
                .map(|o| ItemId::Operation(o.id)),
            ItemId::SelectionSet(set) => self
                .operations
                .iter()
                .find(|o| o.selections.contains(&set))
                .map(|o| ItemId::Operation(o.id)),
            ItemId::Operation(_) => None,
        }
    }
}
