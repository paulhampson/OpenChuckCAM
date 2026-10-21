// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project document model (architecture section 5.6).
//!
//! A project holds models, setups, tool snapshots, selection sets and
//! operations, each with a stable unique ID (PRJ-05), plus a copy of the
//! machine profile it is set up for (SET-01, decision record 0011).
//! Setups, tool snapshots, selection sets and operations each carry an order
//! key (decision record 0006) and are kept sorted by `(key, id)`. Model
//! references have no key: they are listed in `project.toml` in order
//! (decision record 0008).
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
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::model::{ModelFormat, ModelKind};
use openchuck_types::setup::{SetupFrame, validate_setup};
use openchuck_types::simulation::SimulationSettings;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{ControllerMapping, CuttingTable, Tool};
use openchuck_types::transform::TransformStep;
use openchuck_types::units::LengthUnit;
use openchuck_types::workholding::{Workholding, validate_workholding};
use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::hash::ContentHash;
use crate::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use crate::order::{self, OrderKey};

/// A model file referenced by the project (PRJ-02), with the user's units
/// override (IMP-03) and placement (IMP-05).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRef {
    /// Stable ID.
    pub id: ModelId,
    /// Path relative to the project folder.
    pub path: RelativePath,
    /// Hash of the file's contents when it was added or last accepted.
    pub hash: ContentHash,
    /// The unit the file is in, set by the user; `None` reads the unit from
    /// the file, or assumes millimetres (IMP-03).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<LengthUnit>,
    /// How the model is placed in the model frame, whose X axis is the rotary
    /// axis: steps applied first to last (IMP-05).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transform: Vec<TransformStep>,
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
    /// A reference to the file at `path` with hash `hash`, read in the
    /// file's own units and not moved.
    pub fn new(id: ModelId, path: RelativePath, hash: ContentHash) -> Self {
        Self {
            id,
            path,
            hash,
            units: None,
            transform: Vec::new(),
        }
    }

    /// The model's format, from its file name's extension.
    pub fn format(&self) -> Option<ModelFormat> {
        ModelFormat::from_file_name(self.path.as_str())
    }

    /// Whether the model is a mesh or a B-rep, from its format.
    pub fn kind(&self) -> Option<ModelKind> {
        self.format().map(ModelFormat::kind)
    }

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

/// A setup: one way the stock is held on the machine (SET-03, SET-05,
/// SET-06; decision record 0011).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// Stable ID.
    pub id: SetupId,
    /// Position in its list (PRJ-05). The project sets it when the item is
    /// added at a position or moved.
    pub order: OrderKey,
    /// Name shown to the user.
    pub name: String,
    /// The model machined in this setup, once chosen. Bounding-box stock
    /// and the setup checks measure it.
    pub part: Option<ModelId>,
    /// The stock, once chosen (SET-03).
    pub stock: Option<Stock>,
    /// The stock material, as a plain name (SET-03).
    pub material: Option<String>,
    /// Where the part and the work zero sit on the machine (SET-05).
    pub frame: SetupFrame,
    /// The setup's own clearance geometry; `None` uses the machine
    /// profile's (SET-06).
    pub clearance: Option<Clearance>,
    /// The held regions and retaining tabs (SET-09, OP-21); empty by default.
    pub workholding: Workholding,
}

impl Setup {
    /// A setup with no part or stock, the part origin at the machine's
    /// rotary reference point, the work zero in G54 at the model origin,
    /// and the machine's clearance.
    pub fn new(id: SetupId, name: impl Into<String>) -> Self {
        Self {
            id,
            order: OrderKey::default(),
            name: name.into(),
            part: None,
            stock: None,
            material: None,
            frame: SetupFrame::default(),
            clearance: None,
            workholding: Workholding::default(),
        }
    }
}

/// A copy of a tool kept in the project, so the project does not change when
/// the tool library does (TL-07).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolSnapshot {
    /// Stable ID.
    pub id: ToolId,
    /// Position in its list (PRJ-05). The project sets it when the item is
    /// added at a position or moved.
    pub order: OrderKey,
    /// Name shown to the user.
    pub name: String,
    /// The tool's geometry and physical parameters (TL-02 to TL-04).
    pub tool: Tool,
    /// Controller values for the post (TL-06).
    pub controller: ControllerMapping,
    /// Cutting data by stock material name (TL-05).
    pub cutting_data: CuttingTable,
}

/// What a selection set selects on its model (PRJ-06).
///
/// B-rep models are selected by face or edge, whose IDs are stable for the same
/// file (architecture section 5.4). Mesh models are selected by triangle index,
/// or by the edges of their triangles; triangle and mesh edge selections apply
/// to mesh models only, because a B-rep model's triangles change with the
/// tessellation tolerance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionTarget {
    /// B-rep faces.
    Faces(BTreeSet<FaceId>),
    /// B-rep edges.
    Edges(BTreeSet<EdgeId>),
    /// Mesh triangles by index.
    Triangles(BTreeSet<u32>),
    /// Mesh edges, each named by a triangle that has it (schema 12; OI-204,
    /// decision record 0066).
    MeshEdges(BTreeSet<MeshEdge>),
}

impl SelectionTarget {
    /// How many entities the target selects.
    pub fn len(&self) -> usize {
        match self {
            Self::Faces(f) => f.len(),
            Self::Edges(e) => e.len(),
            Self::Triangles(t) => t.len(),
            Self::MeshEdges(e) => e.len(),
        }
    }

    /// Whether the target selects nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entities' name for the user: "faces", "edges" or "triangles".
    /// Mesh edges and B-rep edges are both "edges".
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Faces(_) => "faces",
            Self::Edges(_) | Self::MeshEdges(_) => "edges",
            Self::Triangles(_) => "triangles",
        }
    }
}

impl From<BTreeSet<FaceId>> for SelectionTarget {
    fn from(faces: BTreeSet<FaceId>) -> Self {
        Self::Faces(faces)
    }
}

impl From<BTreeSet<EdgeId>> for SelectionTarget {
    fn from(edges: BTreeSet<EdgeId>) -> Self {
        Self::Edges(edges)
    }
}

impl From<BTreeSet<u32>> for SelectionTarget {
    fn from(triangles: BTreeSet<u32>) -> Self {
        Self::Triangles(triangles)
    }
}

impl From<BTreeSet<MeshEdge>> for SelectionTarget {
    fn from(edges: BTreeSet<MeshEdge>) -> Self {
        Self::MeshEdges(edges)
    }
}

/// An edge of a mesh model, named by one triangle that has it: side `side` of
/// facet `triangle` of the model file, from corner `side` to corner
/// `(side + 1) % 3` (schema 12; decision record 0066).
///
/// Triangle `i` is facet `i` of the file whatever the placement (decision
/// record 0032), so the name is relative to the file's content hash, as a
/// triangle selection is. An edge shared by two triangles has two names; a
/// front end stores the smaller, so a set holds each edge once. Written as a
/// pair `[triangle, side]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "[u32; 2]", into = "[u32; 2]")]
pub struct MeshEdge {
    /// The facet's index in the model file.
    pub triangle: u32,
    /// The side, 0 to 2.
    side: u8,
}

impl MeshEdge {
    /// Side `side` (0 to 2) of triangle `triangle`, or `None` for a side
    /// above 2.
    pub fn new(triangle: u32, side: u8) -> Option<Self> {
        (side < 3).then_some(Self { triangle, side })
    }

    /// The side, 0 to 2: from corner `side` to corner `(side + 1) % 3`.
    pub fn side(self) -> u8 {
        self.side
    }

    /// The indices, 0 to 2, of the triangle's corners at the edge's ends.
    pub fn corners(self) -> [usize; 2] {
        let a = usize::from(self.side);
        [a, (a + 1) % 3]
    }
}

impl TryFrom<[u32; 2]> for MeshEdge {
    type Error = String;

    fn try_from([triangle, side]: [u32; 2]) -> Result<Self, String> {
        u8::try_from(side)
            .ok()
            .and_then(|s| Self::new(triangle, s))
            .ok_or_else(|| {
                format!("mesh edge [{triangle}, {side}]: the side of a triangle is 0, 1 or 2")
            })
    }
}

impl From<MeshEdge> for [u32; 2] {
    fn from(edge: MeshEdge) -> Self {
        [edge.triangle, u32::from(edge.side)]
    }
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
    /// Position in its list (PRJ-05). The project sets it when the item is
    /// added at a position or moved.
    pub order: OrderKey,
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
    /// Material to leave on this selection, in millimetres, in place of the
    /// operation's stock to leave where the set is a machined region (OP-11,
    /// decision record 0034 item 4). `None` uses the operation's value.
    pub stock_to_leave: Option<f64>,
    /// The colour the set is shown in, as `#rrggbb` in lower case, or `None`
    /// for the default swatch of its place in the list (decision record 0046
    /// item 2). It does not change what an operation cuts.
    pub colour: Option<String>,
    /// For a set of B-rep edges: whether edges where two faces meet smoothly
    /// (tangent edges) are kept, or `None` for the project's default
    /// ([`Project::keep_tangent_edges`]). Only an `Edges` set may hold one
    /// (schema 12, decision record 0066).
    pub keep_tangent_edges: Option<bool>,
}

/// Whether `text` is a colour as a set stores it: `#rrggbb` in lower case.
pub fn is_set_colour(text: &str) -> bool {
    text.len() == 7
        && text.starts_with('#')
        && text[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A parameter value of an operation. The strategy's parameter schema (OP-16)
/// gives each its meaning and unit.
///
/// Written as a plain value (`stepover = 0.5`), so a one-parameter edit is a
/// one-line diff. A whole number is an integer and a number with a decimal
/// point or exponent is a number, as in TOML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    /// A whole number.
    Integer(i64),
    /// A number in the unit the schema gives (millimetres, degrees, …).
    Number(f64),
    /// An on or off choice.
    Flag(bool),
    /// Text, including a choice from a list.
    Text(String),
}

/// What a strategy does where the tool would break an avoid or check set
/// (decision record 0176 item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnBreak {
    /// The pass stops short of the set and starts again after it.
    #[default]
    CutShort,
    /// The path is kept, with a warning saying how far the tool breaks the
    /// set.
    Warn,
    /// The set limits nothing, with a warning saying so.
    Ignore,
}

/// The options of an operation's boundary set (decision record 0176 item 3).
/// Both are on by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BoundaryOptions {
    /// The boundary limits the pass across the rulings (limit A).
    pub limits_pass: bool,
    /// The boundary limits the cut length at the tool tip (limit B).
    pub limits_tip: bool,
}

impl Default for BoundaryOptions {
    fn default() -> Self {
        Self {
            limits_pass: true,
            limits_tip: true,
        }
    }
}

impl BoundaryOptions {
    /// True when the options are the defaults, so nothing is written.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// True when the boundary limits nothing.
    pub fn limits_nothing(&self) -> bool {
        !self.limits_pass && !self.limits_tip
    }
}

/// The options of one avoid or check set of an operation (decision record
/// 0176 item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SetOptions {
    /// What happens where the tool would break the set.
    pub on_break: OnBreak,
}

impl SetOptions {
    /// True when the options are the defaults, so nothing is written.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The options of an operation's boundary, avoid and check sets, each
/// attached to that set in that role (decision record 0176 item 3; schema
/// 14). A set with no entry takes the defaults; an entry equal to the
/// defaults is not written.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RestrictionOptions {
    /// The boundary set's options.
    pub boundary: BoundaryOptions,
    /// The options of avoid sets, by set.
    pub avoid: BTreeMap<SelectionSetId, SetOptions>,
    /// The options of check sets, by set.
    pub check: BTreeMap<SelectionSetId, SetOptions>,
}

impl RestrictionOptions {
    /// The options of avoid set `set`.
    pub fn avoid(&self, set: SelectionSetId) -> SetOptions {
        self.avoid.get(&set).copied().unwrap_or_default()
    }

    /// The options of check set `set`.
    pub fn check(&self, set: SelectionSetId) -> SetOptions {
        self.check.get(&set).copied().unwrap_or_default()
    }

    /// Sets the options of avoid set `set`, keeping no entry for the
    /// defaults.
    pub fn set_avoid(&mut self, set: SelectionSetId, options: SetOptions) {
        put(&mut self.avoid, set, options);
    }

    /// Sets the options of check set `set`, keeping no entry for the
    /// defaults.
    pub fn set_check(&mut self, set: SelectionSetId, options: SetOptions) {
        put(&mut self.check, set, options);
    }
}

fn put(map: &mut BTreeMap<SelectionSetId, SetOptions>, set: SelectionSetId, options: SetOptions) {
    if options.is_default() {
        map.remove(&set);
    } else {
        map.insert(set, options);
    }
}

/// One machining operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// Stable ID.
    pub id: OperationId,
    /// Position in its list (PRJ-05). The project sets it when the item is
    /// added at a position or moved.
    pub order: OrderKey,
    /// Name shown to the user.
    pub name: String,
    /// The setup it machines in.
    pub setup: SetupId,
    /// The strategy's registry name (OP-16).
    pub strategy: String,
    /// The tool it cuts with, once chosen.
    pub tool: Option<ToolId>,
    /// The selection sets it machines, in order. Empty machines the whole
    /// part.
    pub selections: Vec<SelectionSetId>,
    /// A set that bounds where the tool may cut (OP-02, containment
    /// boundary).
    pub boundary: Option<SelectionSetId>,
    /// Sets that are never cut (OP-02, avoid surfaces).
    pub avoid: Vec<SelectionSetId>,
    /// Sets the tool must stop short of (OP-02, check surfaces).
    pub check: Vec<SelectionSetId>,
    /// The options of the boundary, avoid and check sets (decision record
    /// 0176 item 3).
    pub restriction_options: RestrictionOptions,
    /// Parameter values by name, in name order.
    pub parameters: BTreeMap<String, ParameterValue>,
    /// False when the user has disabled it: it stays in the project and the
    /// tree but is left out of regeneration, simulation and posting, as if it
    /// were absent (UI-06; decision record 0029 item 10).
    pub enabled: bool,
}

impl Operation {
    /// Every selection set the operation refers to, in any role: the sets it
    /// machines, its boundary, then its avoid and check sets.
    pub fn selection_refs(&self) -> impl Iterator<Item = SelectionSetId> + '_ {
        self.selections
            .iter()
            .copied()
            .chain(self.boundary)
            .chain(self.avoid.iter().copied())
            .chain(self.check.iter().copied())
    }
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

/// One of the project's lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListKind {
    /// Model references (ordered by position, no keys).
    Models,
    /// Setups.
    Setups,
    /// Tool snapshots.
    Tools,
    /// Selection sets.
    SelectionSets,
    /// Operations.
    Operations,
}

impl ItemId {
    /// The list the item belongs to.
    pub fn list(self) -> ListKind {
        match self {
            Self::Model(_) => ListKind::Models,
            Self::Setup(_) => ListKind::Setups,
            Self::Tool(_) => ListKind::Tools,
            Self::SelectionSet(_) => ListKind::SelectionSets,
            Self::Operation(_) => ListKind::Operations,
        }
    }
}

/// Where [`Project::insert`] puts an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// At this position in its list; a keyed item gets a new order key.
    Index(usize),
    /// At the end of its list; a keyed item gets a new order key.
    End,
    /// Where the item's own order key puts it (used to restore a removed
    /// item). A model reference goes at the end.
    Keep,
}

/// Order keys as they were before an edit, so it can be undone exactly.
pub type OldKeys = Vec<(ItemId, OrderKey)>;

/// How to put an item back where it was before [`Project::move_item`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OldPlace {
    /// Restore these order keys (keyed lists).
    Keys(OldKeys),
    /// Move it back to this position (model references).
    Index(usize),
}

/// A project document.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    name: String,
    machine: Option<MachineProfile>,
    /// The ID of the library profile the machine copy came from (OI-128).
    pub(crate) machine_id: Option<String>,
    simulation: SimulationSettings,
    /// Whether B-rep edge sets keep tangent edges when they do not say
    /// (schema 12, decision record 0066). Off by default.
    keep_tangent_edges: bool,
    /// Whether a plain click of the other kind warns before it resets the
    /// working selection (schema 12, decision record 0066 item 3). On by
    /// default.
    warn_on_selection_reset: bool,
    models: Vec<ModelRef>,
    setups: Vec<Setup>,
    tools: Vec<ToolSnapshot>,
    selection_sets: Vec<SelectionSet>,
    operations: Vec<Operation>,
}

/// What the generic list edits need from an item.
trait Entry: Clone {
    fn item_id(&self) -> ItemId;
    fn key(&self) -> Option<&OrderKey>;
    fn key_mut(&mut self) -> Option<&mut OrderKey>;
    fn into_item(self) -> Item;
    fn from_item(item: Item) -> Option<Self>;
}

macro_rules! entry {
    ($ty:ty, $variant:ident, keyed) => {
        impl Entry for $ty {
            fn item_id(&self) -> ItemId {
                ItemId::$variant(self.id)
            }
            fn key(&self) -> Option<&OrderKey> {
                Some(&self.order)
            }
            fn key_mut(&mut self) -> Option<&mut OrderKey> {
                Some(&mut self.order)
            }
            fn into_item(self) -> Item {
                Item::$variant(self)
            }
            fn from_item(item: Item) -> Option<Self> {
                match item {
                    Item::$variant(x) => Some(x),
                    _ => None,
                }
            }
        }
    };
}

impl Entry for ModelRef {
    fn item_id(&self) -> ItemId {
        ItemId::Model(self.id)
    }
    fn key(&self) -> Option<&OrderKey> {
        None
    }
    fn key_mut(&mut self) -> Option<&mut OrderKey> {
        None
    }
    fn into_item(self) -> Item {
        Item::Model(self)
    }
    fn from_item(item: Item) -> Option<Self> {
        match item {
            Item::Model(x) => Some(x),
            _ => None,
        }
    }
}

entry!(Setup, Setup, keyed);
entry!(ToolSnapshot, Tool, keyed);
entry!(SelectionSet, SelectionSet, keyed);
entry!(Operation, Operation, keyed);

/// Runs `$body` with `$list` bound to the project's list of kind `$kind`.
macro_rules! on_list {
    ($project:expr, $kind:expr, $list:ident => $body:expr) => {
        match $kind {
            ListKind::Models => {
                let $list = &mut $project.models;
                $body
            }
            ListKind::Setups => {
                let $list = &mut $project.setups;
                $body
            }
            ListKind::Tools => {
                let $list = &mut $project.tools;
                $body
            }
            ListKind::SelectionSets => {
                let $list = &mut $project.selection_sets;
                $body
            }
            ListKind::Operations => {
                let $list = &mut $project.operations;
                $body
            }
        }
    };
}

fn kind_name(kind: ListKind) -> &'static str {
    match kind {
        ListKind::Models => ModelId::KIND,
        ListKind::Setups => SetupId::KIND,
        ListKind::Tools => ToolId::KIND,
        ListKind::SelectionSets => SelectionSetId::KIND,
        ListKind::Operations => OperationId::KIND,
    }
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

fn wrong_kind(id: ItemId) -> ProjectError {
    ProjectError::NotFound {
        kind: id.kind(),
        id: id_text(id),
    }
}

fn sort_keyed<T: Entry>(list: &mut [T]) {
    list.sort_by(|a, b| (a.key(), a.item_id()).cmp(&(b.key(), b.item_id())));
}

fn is_sorted_keyed<T: Entry>(list: &[T]) -> bool {
    list.windows(2).all(|pair| match pair {
        [a, b] => (a.key(), a.item_id()) < (b.key(), b.item_id()),
        _ => true,
    })
}

/// Inserts `item` into `list` and returns the old keys of any neighbour that
/// had to be re-keyed.
fn place<T: Entry>(
    list: &mut Vec<T>,
    mut item: T,
    placement: Placement,
    kind: ListKind,
) -> Result<OldKeys, ProjectError> {
    let index = match placement {
        Placement::Index(index) => index,
        Placement::End => list.len(),
        Placement::Keep => {
            if item.key().is_some() {
                list.push(item);
                sort_keyed(list);
            } else {
                list.push(item);
            }
            return Ok(Vec::new());
        }
    };
    if index > list.len() {
        return Err(ProjectError::BadIndex {
            kind: kind_name(kind),
            index,
            len: list.len(),
        });
    }
    let mut old = Vec::new();
    if item.key().is_some() {
        let lo = index
            .checked_sub(1)
            .and_then(|i| list.get(i))
            .and_then(Entry::key)
            .cloned();
        let mut hi = list.get(index).and_then(Entry::key).cloned();
        if let (Some(l), Some(h)) = (&lo, &hi)
            && l == h
        {
            // The neighbours share a key (two branches inserted at the same
            // spot). Give the upper run of equal keys new, increasing keys
            // below the next different key, keeping their order.
            let next = list
                .iter()
                .skip(index)
                .filter_map(Entry::key)
                .find(|k| *k > h)
                .cloned();
            let mut previous = h.clone();
            let mut first = None;
            for entry in list.iter_mut().skip(index) {
                if entry.key() != Some(h) {
                    break;
                }
                let new = OrderKey::between(Some(&previous), next.as_ref())?;
                let id = entry.item_id();
                if let Some(key) = entry.key_mut() {
                    old.push((id, std::mem::replace(key, new.clone())));
                }
                first.get_or_insert_with(|| new.clone());
                previous = new;
            }
            hi = first;
        }
        let key = OrderKey::between(lo.as_ref(), hi.as_ref())?;
        if let Some(slot) = item.key_mut() {
            *slot = key;
        }
    }
    list.insert(index, item);
    Ok(old)
}

/// Replaces the entry at `index`, keeping its order key.
fn replace_at<T: Entry>(
    list: &mut [T],
    index: usize,
    item: Item,
    id: ItemId,
) -> Result<Item, ProjectError> {
    let mut entry = T::from_item(item).ok_or_else(|| wrong_kind(id))?;
    let slot = list.get_mut(index).ok_or_else(|| not_found(id))?;
    if let (Some(new), Some(current)) = (entry.key_mut(), slot.key()) {
        *new = current.clone();
    }
    Ok(std::mem::replace(slot, entry).into_item())
}

impl Default for Project {
    /// An empty project with no name; the reset warning is on (decision
    /// record 0066 item 3).
    fn default() -> Self {
        Self {
            name: String::new(),
            machine: None,
            machine_id: None,
            simulation: SimulationSettings::default(),
            keep_tangent_edges: false,
            warn_on_selection_reset: true,
            models: Vec::new(),
            setups: Vec::new(),
            tools: Vec::new(),
            selection_sets: Vec::new(),
            operations: Vec::new(),
        }
    }
}

impl Project {
    /// An empty project with this name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Builds a project from loaded parts, sorting keyed lists by `(key, id)`,
    /// then validates it.
    pub(crate) fn from_parts(
        name: String,
        machine: Option<MachineProfile>,
        models: Vec<ModelRef>,
        mut setups: Vec<Setup>,
        mut tools: Vec<ToolSnapshot>,
        mut selection_sets: Vec<SelectionSet>,
        mut operations: Vec<Operation>,
    ) -> Result<Self, ProjectError> {
        sort_keyed(&mut setups);
        sort_keyed(&mut tools);
        sort_keyed(&mut selection_sets);
        sort_keyed(&mut operations);
        let project = Self {
            name,
            machine,
            machine_id: None,
            simulation: SimulationSettings::default(),
            keep_tangent_edges: false,
            warn_on_selection_reset: true,
            models,
            setups,
            tools,
            selection_sets,
            operations,
        };
        project.validate()?;
        Ok(project)
    }

    /// The project's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The project's copy of its machine profile, once chosen. The copy wins
    /// over the machine library, so the project gives the same G-code on any
    /// computer (GC-07, decision record 0011).
    pub fn machine(&self) -> Option<&MachineProfile> {
        self.machine.as_ref()
    }

    /// How the material removal simulation runs (SIM-07). Part of the
    /// project because later operations are generated against the simulated
    /// stock (SIM-06), so the G-code depends on it (GC-07, decision record 0025).
    pub fn simulation(&self) -> SimulationSettings {
        self.simulation
    }

    /// Whether a set of B-rep edges that does not say keeps the edges where
    /// two faces meet smoothly (tangent edges) when an operation follows it
    /// (OP-08; decision record 0066). Off unless the project sets it.
    pub fn keep_tangent_edges(&self) -> bool {
        self.keep_tangent_edges
    }

    /// Whether `set` keeps tangent edges: its own choice, else the project's
    /// default. Meaningful for a set of B-rep edges.
    pub fn keeps_tangent_edges(&self, set: &SelectionSet) -> bool {
        set.keep_tangent_edges.unwrap_or(self.keep_tangent_edges)
    }

    /// Whether a plain click that picks the other kind (an edge on a
    /// selection of faces, say) asks before it resets the working selection
    /// (decision record 0066 item 3). On unless the user turned it off.
    pub fn warn_on_selection_reset(&self) -> bool {
        self.warn_on_selection_reset
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

    /// The operations that use selection set `id` in any role, in order. These
    /// are the operations to regenerate when the set is edited (UI-05).
    pub fn operations_using(&self, id: SelectionSetId) -> Vec<OperationId> {
        self.operations
            .iter()
            .filter(|o| o.selection_refs().any(|s| s == id))
            .map(|o| o.id)
            .collect()
    }

    /// Whether an item with this ID exists.
    pub fn contains(&self, id: ItemId) -> bool {
        self.index_of(id).is_some()
    }

    /// The item's position in its list.
    pub fn index_of(&self, id: ItemId) -> Option<usize> {
        fn find<T: Entry>(list: &[T], id: ItemId) -> Option<usize> {
            list.iter().position(|e| e.item_id() == id)
        }
        match id.list() {
            ListKind::Models => find(&self.models, id),
            ListKind::Setups => find(&self.setups, id),
            ListKind::Tools => find(&self.tools, id),
            ListKind::SelectionSets => find(&self.selection_sets, id),
            ListKind::Operations => find(&self.operations, id),
        }
    }

    /// The item with this ID, cloned.
    pub fn item(&self, id: ItemId) -> Option<Item> {
        fn get<T: Entry>(list: &[T], id: ItemId) -> Option<Item> {
            list.iter()
                .find(|e| e.item_id() == id)
                .cloned()
                .map(Entry::into_item)
        }
        match id.list() {
            ListKind::Models => get(&self.models, id),
            ListKind::Setups => get(&self.setups, id),
            ListKind::Tools => get(&self.tools, id),
            ListKind::SelectionSets => get(&self.selection_sets, id),
            ListKind::Operations => get(&self.operations, id),
        }
    }

    /// Items whose order key is long enough that their list should be
    /// renumbered (OI-35).
    pub fn long_order_keys(&self) -> Vec<ItemId> {
        fn long<T: Entry>(list: &[T]) -> impl Iterator<Item = ItemId> + '_ {
            list.iter()
                .filter(|e| e.key().is_some_and(OrderKey::is_long))
                .map(Entry::item_id)
        }
        long(&self.setups)
            .chain(long(&self.tools))
            .chain(long(&self.selection_sets))
            .chain(long(&self.operations))
            .collect()
    }

    /// Renames the project and returns the old name.
    pub fn set_name(&mut self, name: impl Into<String>) -> String {
        std::mem::replace(&mut self.name, name.into())
    }

    /// Sets the machine profile, with no record of a source, and returns the
    /// old one. Fails, leaving the project unchanged, if the profile is not
    /// valid.
    pub fn set_machine(
        &mut self,
        machine: Option<MachineProfile>,
    ) -> Result<Option<MachineProfile>, ProjectError> {
        self.set_machine_from(machine, None).map(|(old, _)| old)
    }

    /// Sets the machine profile and the ID of the library profile it came
    /// from (OI-128), and returns the old pair. Without a machine there is no
    /// ID. Fails, leaving the project unchanged, if either is not valid.
    pub fn set_machine_from(
        &mut self,
        machine: Option<MachineProfile>,
        id: Option<String>,
    ) -> Result<(Option<MachineProfile>, Option<String>), ProjectError> {
        if let Some(profile) = &machine {
            profile.validate()?;
        }
        if let Some(id) = &id {
            crate::machines::validate_machine_id(id)?;
        }
        let id = if machine.is_some() { id } else { None };
        Ok((
            std::mem::replace(&mut self.machine, machine),
            std::mem::replace(&mut self.machine_id, id),
        ))
    }

    /// The ID of the library profile the machine copy came from, if it
    /// records one (OI-128).
    pub fn machine_id(&self) -> Option<&str> {
        self.machine_id.as_deref()
    }

    /// Sets the project's default for keeping tangent edges and returns the
    /// old one (decision record 0066).
    pub fn set_keep_tangent_edges(&mut self, keep: bool) -> bool {
        std::mem::replace(&mut self.keep_tangent_edges, keep)
    }

    /// Sets whether a plain click of the other kind warns before it resets
    /// the working selection, and returns the old value (decision record 0066).
    pub fn set_warn_on_selection_reset(&mut self, warn: bool) -> bool {
        std::mem::replace(&mut self.warn_on_selection_reset, warn)
    }

    /// Sets the simulation settings and returns the old ones. Fails, leaving
    /// the project unchanged, if they are out of range.
    pub fn set_simulation(
        &mut self,
        settings: SimulationSettings,
    ) -> Result<SimulationSettings, ProjectError> {
        settings.validate()?;
        Ok(std::mem::replace(&mut self.simulation, settings))
    }

    /// The setup with this ID.
    pub fn setup(&self, id: SetupId) -> Option<&Setup> {
        self.setups.iter().find(|s| s.id == id)
    }

    /// Adds `item` and returns the old keys of any neighbour that had to be
    /// re-keyed. Fails if the ID is taken, a reference is unknown, or a value
    /// is invalid; the project is then unchanged.
    pub fn insert(&mut self, item: Item, placement: Placement) -> Result<OldKeys, ProjectError> {
        let id = item.id();
        if self.contains(id) {
            return Err(ProjectError::DuplicateId {
                kind: id.kind(),
                id: id_text(id),
            });
        }
        self.check_item(&item)?;
        on_list!(self, id.list(), list => {
            let entry = Entry::from_item(item).ok_or_else(|| wrong_kind(id))?;
            place(list, entry, placement, id.list())
        })
    }

    /// Removes an item that nothing uses, and returns its old position and the
    /// item, order key included.
    pub fn remove(&mut self, id: ItemId) -> Result<(usize, Item), ProjectError> {
        let index = self.index_of(id).ok_or_else(|| not_found(id))?;
        if let Some(by) = self.first_user_of(id) {
            return Err(ProjectError::InUse {
                kind: id.kind(),
                id: id_text(id),
                by: by.to_string(),
            });
        }
        on_list!(self, id.list(), list => Ok((index, list.remove(index).into_item())))
    }

    /// Replaces an item with a new version that has the same ID, and returns
    /// the old version. The item keeps its order key and position.
    pub fn replace(&mut self, item: Item) -> Result<Item, ProjectError> {
        let id = item.id();
        let index = self.index_of(id).ok_or_else(|| not_found(id))?;
        self.check_item(&item)?;
        on_list!(self, id.list(), list => replace_at(list, index, item, id))
    }

    /// Moves an item to `index` in its list and returns how to put it back.
    pub fn move_item(&mut self, id: ItemId, index: usize) -> Result<OldPlace, ProjectError> {
        let from = self.index_of(id).ok_or_else(|| not_found(id))?;
        on_list!(self, id.list(), list => {
            if index >= list.len() {
                return Err(ProjectError::BadIndex {
                    kind: id.kind(),
                    index,
                    len: list.len(),
                });
            }
            let entry = list.remove(from);
            let old_key = entry.key().cloned();
            let before = list.clone();
            match place(list, entry.clone(), Placement::Index(index), id.list()) {
                Ok(mut old) => Ok(match old_key {
                    Some(key) => {
                        old.insert(0, (id, key));
                        OldPlace::Keys(old)
                    }
                    None => OldPlace::Index(from),
                }),
                Err(e) => {
                    *list = before;
                    list.insert(from, entry);
                    Err(e)
                }
            }
        })
    }

    /// Sets the order keys of keyed items, re-sorts, and returns the old keys.
    /// Used to undo a move or a renumber exactly.
    pub fn set_order_keys(&mut self, keys: &[(ItemId, OrderKey)]) -> Result<OldKeys, ProjectError> {
        for (id, _) in keys {
            if !self.contains(*id) || id.list() == ListKind::Models {
                return Err(not_found(*id));
            }
        }
        let mut old = Vec::new();
        for (id, key) in keys {
            on_list!(self, id.list(), list => {
                if let Some(entry) = list.iter_mut().find(|e| e.item_id() == *id)
                    && let Some(slot) = entry.key_mut()
                {
                    old.push((*id, std::mem::replace(slot, key.clone())));
                }
            });
        }
        for kind in [
            ListKind::Setups,
            ListKind::Tools,
            ListKind::SelectionSets,
            ListKind::Operations,
        ] {
            on_list!(self, kind, list => sort_keyed(list));
        }
        Ok(old)
    }

    /// Gives every item in a keyed list a short, evenly spaced order key,
    /// keeping the order, and returns the old keys (OI-35). It changes every
    /// item's file, so it is an explicit user action.
    pub fn renumber(&mut self, kind: ListKind) -> Result<OldKeys, ProjectError> {
        if kind == ListKind::Models {
            return Ok(Vec::new());
        }
        on_list!(self, kind, list => {
            let keys = order::renumber(list.len());
            let mut old = Vec::new();
            for (entry, key) in list.iter_mut().zip(keys) {
                let id = entry.item_id();
                if let Some(slot) = entry.key_mut() {
                    old.push((id, std::mem::replace(slot, key)));
                }
            }
            Ok(old)
        })
    }

    /// Checks the whole document: unique IDs, known references, valid values
    /// and keyed lists sorted by `(key, id)`. Edits keep these rules, so this
    /// is for documents built another way, such as a loaded file.
    pub fn validate(&self) -> Result<(), ProjectError> {
        if let Some(profile) = &self.machine {
            profile.validate()?;
        }
        self.simulation.validate()?;
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
        let sorted = is_sorted_keyed(&self.setups)
            && is_sorted_keyed(&self.tools)
            && is_sorted_keyed(&self.selection_sets)
            && is_sorted_keyed(&self.operations);
        if !sorted {
            return Err(openchuck_types::Error::invalid(
                "project",
                "a list is not in order-key order",
            )
            .into());
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
            Item::Model(m) => {
                TransformStep::validate_all(&m.transform, &format!("model {}.transform", m.id))?;
                // A changed path must not change the kind its selections need.
                self.selection_sets
                    .iter()
                    .filter(|s| s.model == m.id)
                    .try_for_each(|s| check_selection_kind(s, m.kind()))
            }
            Item::Setup(s) => {
                if let Some(model) = s.part {
                    require(ItemId::Model(model))?;
                }
                if s.material.as_ref().is_some_and(|m| m.trim().is_empty()) {
                    return Err(openchuck_types::Error::invalid(
                        format!("setup {}.material", s.id),
                        "the material name is empty; leave it unset instead",
                    )
                    .into());
                }
                let field = format!("setup {}", s.id);
                validate_setup(&s.frame, s.stock.as_ref(), s.clearance.as_ref(), &field)?;
                Ok(validate_workholding(
                    &s.workholding,
                    s.stock.as_ref(),
                    &field,
                )?)
            }
            Item::Tool(t) => Ok(openchuck_types::tool::validate_entry(
                &t.tool,
                &t.controller,
                &t.cutting_data,
            )?),
            Item::SelectionSet(s) => {
                require(ItemId::Model(s.model))?;
                let kind = self
                    .models
                    .iter()
                    .find(|m| m.id == s.model)
                    .and_then(ModelRef::kind);
                check_selection_kind(s, kind)?;
                if s.stock_to_leave.is_some_and(|v| !v.is_finite() || v < 0.0) {
                    return Err(openchuck_types::Error::invalid(
                        format!("selection set {}.stock_to_leave", s.id),
                        "stock_to_leave must be a number of millimetres, zero or more",
                    )
                    .into());
                }
                if s.colour.as_deref().is_some_and(|c| !is_set_colour(c)) {
                    return Err(openchuck_types::Error::invalid(
                        format!("selection set {}.colour", s.id),
                        "colour must be written #rrggbb in lower case",
                    )
                    .into());
                }
                if s.keep_tangent_edges.is_some() && !matches!(s.target, SelectionTarget::Edges(_))
                {
                    return Err(openchuck_types::Error::invalid(
                        format!("selection set {}.keep_tangent_edges", s.id),
                        "keep_tangent_edges applies to a set of a STEP model's edges only",
                    )
                    .into());
                }
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
                for set in o.selection_refs() {
                    require(ItemId::SelectionSet(set))?;
                }
                let options = &o.restriction_options;
                if o.boundary.is_none() && !options.boundary.is_default() {
                    return Err(openchuck_types::Error::invalid(
                        format!("operation {}.boundary_options", o.id),
                        "the operation has no boundary set for these options to apply to",
                    )
                    .into());
                }
                for (role, map, sets) in [
                    ("avoid", &options.avoid, &o.avoid),
                    ("check", &options.check, &o.check),
                ] {
                    if let Some(set) = map.keys().find(|s| !sets.contains(s)) {
                        return Err(openchuck_types::Error::invalid(
                            format!("operation {}.{role}_options", o.id),
                            format!(
                                "selection set {set} has {role} options but is not one of the \
                                 operation's {role} sets"
                            ),
                        )
                        .into());
                    }
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
                .setups
                .iter()
                .find(|s| s.part == Some(model))
                .map(|s| ItemId::Setup(s.id))
                .or_else(|| {
                    self.selection_sets
                        .iter()
                        .find(|s| s.model == model)
                        .map(|s| ItemId::SelectionSet(s.id))
                }),
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
                .find(|o| o.selection_refs().any(|s| s == set))
                .map(|o| ItemId::Operation(o.id)),
            ItemId::Operation(_) => None,
        }
    }
}

/// Triangle and mesh edge selections apply to mesh models only, and face and
/// edge selections to B-rep models only (architecture section 5.6). A model whose
/// format is not known is not checked.
fn check_selection_kind(set: &SelectionSet, kind: Option<ModelKind>) -> Result<(), ProjectError> {
    let reason = match (&set.target, kind) {
        (SelectionTarget::Triangles(_), Some(ModelKind::Brep)) => {
            "a STEP model is selected by face and edge, not by triangle, because its \
             triangles change with the tessellation tolerance"
        }
        (SelectionTarget::MeshEdges(_), Some(ModelKind::Brep)) => {
            "a STEP model is selected by its own edges, not by the edges of its triangles, \
             because its triangles change with the tessellation tolerance"
        }
        (SelectionTarget::Faces(_) | SelectionTarget::Edges(_), Some(ModelKind::Mesh)) => {
            "a mesh model has no faces or B-rep edges to select; select triangles or \
             triangle edges"
        }
        _ => return Ok(()),
    };
    Err(openchuck_types::Error::invalid(format!("selection set {}.target", set.id), reason).into())
}
