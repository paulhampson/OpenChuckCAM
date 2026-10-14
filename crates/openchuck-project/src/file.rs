// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project folder on disk (PRJ-01, PRJ-04; decision records 0006 and 0008).
//!
//! ```text
//! project.toml            schema, name, model references
//! machine.toml            the project's copy of its machine profile, once chosen
//! setups/<id>.toml        one file per setup
//! tools/<id>.toml         one file per tool snapshot
//! selections/<id>.toml    one file per selection set
//! operations/<id>.toml    one file per operation
//! .openchuck-cache/       generated data, ignored by Git (PRJ-03)
//! ```
//!
//! Every file is TOML written through `serde` and `toml`, starts with
//! `schema`, the project folder's schema version, and ends with a newline.
//! Files are written only when their text changes, so a load then save leaves
//! the folder untouched, and the files of removed items are deleted. Each file
//! is written to a temporary file and renamed into place.
//!
//! Loading checks, in order: unresolved Git merge conflict markers (two
//! branches that both add a model conflict in `project.toml`), TOML syntax,
//! the schema version (newer is refused, older is migrated step by step on the
//! raw TOML), the typed fields (unknown fields are refused), that each item
//! file's name is its ID, and finally the whole document (`Project::validate`).
//! Files that do not end in `.toml` are ignored.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::hash::ContentHash;
use crate::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use crate::model::{
    Descriptor, ModelRef, Operation, ParameterValue, Project, SelectionSet, SelectionTarget, Setup,
    ToolSnapshot,
};
use crate::order::OrderKey;
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::setup::{SetupFrame, WorkOffset};
use openchuck_types::simulation::SimulationSettings;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{ControllerMapping, CuttingTable};
use openchuck_types::units::Millimetres;

/// The project folder schema version this build writes and reads.
///
/// - 1: slice 1 (decision record 0008).
/// - 2: model references gain the optional `units` override (IMP-03) and
///   `transform` steps (IMP-05) (decision record 0010).
/// - 3: the machine profile is copied into `machine.toml` instead of being
///   named in `project.toml`; setups gain part, stock, material, part and
///   work offsets and clearance (decision record 0011).
/// - 4: tool snapshots gain the TL-04 parameters (overall length in place of
///   stickout, flute count, tool material), the controller mapping (TL-06)
///   and cutting data by stock material (TL-05) (decision record 0014).
/// - 5: `project.toml` gains the optional `[simulation]` table (SIM-07),
///   written only when it differs from the default (decision record 0025).
/// - 6: operations gain `enabled`, written only when false (UI-06,
///   decision record 0029 item 10), and the machine copy in `machine.toml`
///   may hold the `id` of the library profile it came from (OI-54, item 16).
/// - 7: operations gain the optional `boundary`, `avoid` and `check`
///   selection set references (OP-02), written only when set (decision
///   record 0032).
pub const SCHEMA: i64 = 7;

/// Name of the machine profile file in the project folder.
pub const MACHINE_FILE: &str = "machine.toml";

/// Name of the project file in the project folder.
pub const PROJECT_FILE: &str = "project.toml";

/// One schema step: changes a file's raw TOML from version `n` to `n + 1`.
/// Fails with a message when the file cannot be migrated.
pub type Migration = fn(FileKind, &mut toml::Table) -> Result<(), String>;

/// The migrations from each older schema to the next, oldest first. Entry `i`
/// takes schema `SCHEMA - MIGRATIONS.len() + i` one step forward.
const MIGRATIONS: &[Migration] = &[
    schema_1_to_2,
    schema_2_to_3,
    schema_3_to_4,
    schema_4_to_5,
    schema_5_to_6,
    schema_6_to_7,
];

/// Schema 2 only adds optional fields to model references, which a schema 1
/// file does not have, so a schema 1 file needs no change but its number.
fn schema_1_to_2(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// Schema 2 named the machine in `project.toml` (`[machine] name = …`), a
/// placeholder no front end could set; schema 3 keeps a full copy of the
/// profile in `machine.toml` instead. A name cannot become a profile, so it is
/// dropped and the machine is chosen again (decision record 0011). Setups gain
/// only fields with defaults.
fn schema_2_to_3(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind == FileKind::Project {
        table.remove("machine");
    }
    Ok(())
}

/// Schema 4 tool snapshots need a flute count and a tool number that a
/// schema 3 snapshot does not hold, and replace the stickout (a holder
/// value) with the overall length. None can be inferred safely, so a schema 3
/// tool file is refused with what to do; no front end could write one
/// (decision record 0014). Other files need no change.
fn schema_3_to_4(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind == FileKind::Tool {
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("unnamed");
        return Err(format!(
            "tool snapshot \"{name}\" was written before tools had an overall length, flute count \
             and tool number (schema 3); add the tool again, or edit the file to schema 4 \
             with `overall_length` in place of `stickout`, `flute_count` under [tool] and \
             a [controller] number"
        ));
    }
    Ok(())
}

/// Schema 5 only adds the optional `[simulation]` table, which a schema 4
/// file does not have, so it needs no change but its number.
fn schema_4_to_5(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// Schema 6 only adds `enabled` to operations, written only when false, so a
/// schema 5 file needs no change but its number: its operations are enabled.
fn schema_5_to_6(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// Schema 7 only adds optional operation fields (`boundary`, `avoid`,
/// `check`), which a schema 6 file does not have, so it needs no change but
/// its number.
fn schema_6_to_7(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// The kinds of file in a project folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileKind {
    /// `project.toml`.
    Project,
    /// `machine.toml`, or a machine profile file in a library.
    Machine,
    /// A setup file.
    Setup,
    /// A tool snapshot file.
    Tool,
    /// A selection set file.
    SelectionSet,
    /// An operation file.
    Operation,
}

impl FileKind {
    /// The folder that holds this kind of item file, if it is one.
    pub fn folder(self) -> Option<&'static str> {
        match self {
            Self::Project | Self::Machine => None,
            Self::Setup => Some("setups"),
            Self::Tool => Some("tools"),
            Self::SelectionSet => Some("selections"),
            Self::Operation => Some("operations"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    schema: i64,
    name: String,
    #[serde(default, skip_serializing_if = "SimulationSettings::is_default")]
    simulation: SimulationSettings,
    #[serde(default)]
    models: Vec<ModelRef>,
}

/// `machine.toml`, and a machine profile file in a library
/// ([`crate::machines`]).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MachineFile {
    pub(crate) schema: i64,
    /// The ID shared with the library profile the copy came from (OI-128).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    pub(crate) machine: MachineProfile,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupFile {
    schema: i64,
    id: SetupId,
    order: OrderKey,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    part: Option<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    material: Option<String>,
    #[serde(default)]
    part_offset: Millimetres,
    #[serde(default)]
    work_offset: WorkOffset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stock: Option<Stock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clearance: Option<Clearance>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolFile {
    schema: i64,
    id: ToolId,
    order: OrderKey,
    name: String,
    tool: openchuck_types::tool::Tool,
    controller: ControllerMapping,
    #[serde(default, skip_serializing_if = "CuttingTable::is_empty")]
    cutting_data: CuttingTable,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionFile {
    schema: i64,
    id: SelectionSetId,
    order: OrderKey,
    name: String,
    model: ModelId,
    model_hash: ContentHash,
    target: SelectionTarget,
    #[serde(default)]
    descriptors: Vec<Descriptor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationFile {
    schema: i64,
    id: OperationId,
    order: OrderKey,
    name: String,
    setup: SetupId,
    strategy: String,
    tool: Option<ToolId>,
    #[serde(default)]
    selections: Vec<SelectionSetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boundary: Option<SelectionSetId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    avoid: Vec<SelectionSetId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    check: Vec<SelectionSetId>,
    #[serde(default)]
    parameters: BTreeMap<String, ParameterValue>,
    #[serde(default = "enabled_default", skip_serializing_if = "is_enabled")]
    enabled: bool,
}

const fn enabled_default() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes a reference
const fn is_enabled(enabled: &bool) -> bool {
    *enabled
}

/// A hash of everything an operation takes from selections: the sets it uses
/// in each role and, for each, the model, the model's hash and the selected
/// geometry. It changes when a set the operation uses is edited or the
/// operation is pointed at other sets, and not when an unrelated set is
/// edited or a set is only renamed or moved in the list. The operation's
/// cache key (architecture section 6) combines it with the operation's other
/// inputs (UI-05).
pub fn selection_inputs_hash(
    project: &Project,
    operation: OperationId,
) -> Result<ContentHash, ProjectError> {
    #[derive(Serialize)]
    struct Used<'a> {
        model: ModelId,
        model_hash: ContentHash,
        target: &'a SelectionTarget,
    }
    #[derive(Serialize)]
    struct Inputs<'a> {
        selections: &'a [SelectionSetId],
        boundary: Option<SelectionSetId>,
        avoid: &'a [SelectionSetId],
        check: &'a [SelectionSetId],
        sets: BTreeMap<String, Used<'a>>,
    }
    let o = project
        .operation(operation)
        .ok_or_else(|| ProjectError::NotFound {
            kind: OperationId::KIND,
            id: operation.to_string(),
        })?;
    let sets = o
        .selection_refs()
        .filter_map(|id| project.selection_sets().iter().find(|s| s.id == id))
        .map(|s| {
            (
                s.id.to_string(),
                Used {
                    model: s.model,
                    model_hash: s.model_hash,
                    target: &s.target,
                },
            )
        })
        .collect();
    let inputs = Inputs {
        selections: &o.selections,
        boundary: o.boundary,
        avoid: &o.avoid,
        check: &o.check,
        sets,
    };
    Ok(ContentHash::of_bytes(to_text(&inputs)?.as_bytes()))
}

/// Every file of `project` as (path relative to the project folder, text),
/// in path order. Paths use `/`.
pub fn render(project: &Project) -> Result<BTreeMap<String, String>, ProjectError> {
    project.validate()?;
    let mut files = BTreeMap::new();
    let project_file = ProjectFile {
        schema: SCHEMA,
        name: project.name().to_owned(),
        simulation: project.simulation(),
        models: project.models().to_vec(),
    };
    files.insert(PROJECT_FILE.to_owned(), to_text(&project_file)?);
    if let Some(machine) = project.machine() {
        let file = MachineFile {
            schema: SCHEMA,
            id: project.machine_id().map(str::to_owned),
            machine: machine.clone(),
        };
        files.insert(MACHINE_FILE.to_owned(), to_text(&file)?);
    }
    for s in project.setups() {
        let file = SetupFile {
            schema: SCHEMA,
            id: s.id,
            order: s.order.clone(),
            name: s.name.clone(),
            part: s.part,
            material: s.material.clone(),
            part_offset: s.frame.part_offset,
            work_offset: s.frame.work_offset,
            stock: s.stock.clone(),
            clearance: s.clearance,
        };
        files.insert(item_path(FileKind::Setup, &s.id), to_text(&file)?);
    }
    for t in project.tools() {
        let file = ToolFile {
            schema: SCHEMA,
            id: t.id,
            order: t.order.clone(),
            name: t.name.clone(),
            tool: t.tool.clone(),
            controller: t.controller,
            cutting_data: t.cutting_data.clone(),
        };
        files.insert(item_path(FileKind::Tool, &t.id), to_text(&file)?);
    }
    for s in project.selection_sets() {
        let file = SelectionFile {
            schema: SCHEMA,
            id: s.id,
            order: s.order.clone(),
            name: s.name.clone(),
            model: s.model,
            model_hash: s.model_hash,
            target: s.target.clone(),
            descriptors: s.descriptors.clone(),
        };
        files.insert(item_path(FileKind::SelectionSet, &s.id), to_text(&file)?);
    }
    for o in project.operations() {
        let file = OperationFile {
            schema: SCHEMA,
            id: o.id,
            order: o.order.clone(),
            name: o.name.clone(),
            setup: o.setup,
            strategy: o.strategy.clone(),
            tool: o.tool,
            selections: o.selections.clone(),
            boundary: o.boundary,
            avoid: o.avoid.clone(),
            check: o.check.clone(),
            parameters: o.parameters.clone(),
            enabled: o.enabled,
        };
        files.insert(item_path(FileKind::Operation, &o.id), to_text(&file)?);
    }
    Ok(files)
}

/// Saves `project` into the folder `root`, creating it if needed. Only files
/// whose text changes are written, and the files of removed items are deleted.
pub fn save(project: &Project, root: &Path) -> Result<(), ProjectError> {
    let files = render(project)?;
    std::fs::create_dir_all(root).map_err(io(root))?;
    for (relative, text) in &files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io(parent))?;
        }
        write_if_changed(&path, text)?;
    }
    if !files.contains_key(MACHINE_FILE) {
        let path = root.join(MACHINE_FILE);
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io(&path)(e)),
            _ => {}
        }
    }
    for kind in [
        FileKind::Setup,
        FileKind::Tool,
        FileKind::SelectionSet,
        FileKind::Operation,
    ] {
        let Some(folder) = kind.folder() else {
            continue;
        };
        let dir = root.join(folder);
        for (stem, path) in item_files(&dir)? {
            if !files.contains_key(&format!("{folder}/{stem}.toml")) && is_item_name(kind, &stem) {
                std::fs::remove_file(&path).map_err(io(&path))?;
            }
        }
        // An emptied folder goes too; Git does not track empty folders.
        if dir.is_dir() && item_files(&dir)?.is_empty() {
            let _ = std::fs::remove_dir(&dir);
        }
    }
    Ok(())
}

/// Creates a new project folder at `root` holding an empty project.
/// Fails if `root` already holds a project.
pub fn create(root: &Path, name: &str) -> Result<Project, ProjectError> {
    let project_file = root.join(PROJECT_FILE);
    if project_file.exists() {
        return Err(ProjectError::BadFile {
            path: project_file,
            reason: "a project already exists here".to_owned(),
        });
    }
    let project = Project::new(name);
    save(&project, root)?;
    Ok(project)
}

/// Loads the project in the folder `root`.
pub fn load(root: &Path) -> Result<Project, ProjectError> {
    load_with(root, MIGRATIONS, SCHEMA)
}

/// Parses and checks the text of one project file of `kind`, as loading does,
/// without the cross-file checks. For fuzzing and tools.
pub fn parse_file(kind: FileKind, text: &str) -> Result<(), ProjectError> {
    let path = Path::new(match kind {
        FileKind::Project => PROJECT_FILE,
        FileKind::Machine => MACHINE_FILE,
        _ => "item.toml",
    });
    match kind {
        FileKind::Project => parse::<ProjectFile>(path, text, kind, MIGRATIONS, SCHEMA).map(drop),
        FileKind::Machine => parse::<MachineFile>(path, text, kind, MIGRATIONS, SCHEMA)
            .and_then(|f| Ok(f.machine.validate()?)),
        FileKind::Setup => parse::<SetupFile>(path, text, kind, MIGRATIONS, SCHEMA).map(drop),
        FileKind::Tool => parse::<ToolFile>(path, text, kind, MIGRATIONS, SCHEMA).and_then(|f| {
            Ok(openchuck_types::tool::validate_entry(
                &f.tool,
                &f.controller,
                &f.cutting_data,
            )?)
        }),
        FileKind::SelectionSet => {
            parse::<SelectionFile>(path, text, kind, MIGRATIONS, SCHEMA).map(drop)
        }
        FileKind::Operation => {
            parse::<OperationFile>(path, text, kind, MIGRATIONS, SCHEMA).map(drop)
        }
    }
}

fn load_with(root: &Path, migrations: &[Migration], schema: i64) -> Result<Project, ProjectError> {
    let project_path = root.join(PROJECT_FILE);
    let text = match std::fs::read_to_string(&project_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && root.is_dir() => {
            return Err(ProjectError::NotAProject(root.to_path_buf()));
        }
        Err(e) => return Err(io(&project_path)(e)),
    };
    let file: ProjectFile = parse(&project_path, &text, FileKind::Project, migrations, schema)?;
    let machine_path = root.join(MACHINE_FILE);
    let mut machine_id = None;
    let machine = match std::fs::read_to_string(&machine_path) {
        Ok(text) => {
            let file: MachineFile =
                parse(&machine_path, &text, FileKind::Machine, migrations, schema)?;
            if let Some(id) = &file.id {
                crate::machines::validate_machine_id(id).map_err(|e| ProjectError::BadFile {
                    path: machine_path.clone(),
                    reason: e.to_string(),
                })?;
            }
            machine_id = file.id;
            Some(file.machine)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io(&machine_path)(e)),
    };

    let setups = read_items::<SetupFile>(root, FileKind::Setup, migrations, schema)?
        .into_iter()
        .map(|(path, f)| {
            check_name(&path, &f.id)?;
            Ok(Setup {
                id: f.id,
                order: f.order,
                name: f.name,
                part: f.part,
                stock: f.stock,
                material: f.material,
                frame: SetupFrame {
                    part_offset: f.part_offset,
                    work_offset: f.work_offset,
                },
                clearance: f.clearance,
            })
        })
        .collect::<Result<Vec<_>, ProjectError>>()?;
    let tools = read_items::<ToolFile>(root, FileKind::Tool, migrations, schema)?
        .into_iter()
        .map(|(path, f)| {
            check_name(&path, &f.id)?;
            Ok(ToolSnapshot {
                id: f.id,
                order: f.order,
                name: f.name,
                tool: f.tool,
                controller: f.controller,
                cutting_data: f.cutting_data,
            })
        })
        .collect::<Result<Vec<_>, ProjectError>>()?;
    let selection_sets =
        read_items::<SelectionFile>(root, FileKind::SelectionSet, migrations, schema)?
            .into_iter()
            .map(|(path, f)| {
                check_name(&path, &f.id)?;
                Ok(SelectionSet {
                    id: f.id,
                    order: f.order,
                    name: f.name,
                    model: f.model,
                    model_hash: f.model_hash,
                    target: f.target,
                    descriptors: f.descriptors,
                })
            })
            .collect::<Result<Vec<_>, ProjectError>>()?;
    let operations = read_items::<OperationFile>(root, FileKind::Operation, migrations, schema)?
        .into_iter()
        .map(|(path, f)| {
            check_name(&path, &f.id)?;
            Ok(Operation {
                id: f.id,
                order: f.order,
                name: f.name,
                setup: f.setup,
                strategy: f.strategy,
                tool: f.tool,
                selections: f.selections,
                boundary: f.boundary,
                avoid: f.avoid,
                check: f.check,
                parameters: f.parameters,
                enabled: f.enabled,
            })
        })
        .collect::<Result<Vec<_>, ProjectError>>()?;

    let mut project = Project::from_parts(
        file.name,
        machine,
        file.models,
        setups,
        tools,
        selection_sets,
        operations,
    )
    .map_err(|e| ProjectError::BadFile {
        path: root.to_path_buf(),
        reason: e.to_string(),
    })?;
    project.machine_id = machine_id;
    project
        .set_simulation(file.simulation)
        .map_err(|e| ProjectError::BadFile {
            path: project_path.clone(),
            reason: e.to_string(),
        })?;
    Ok(project)
}

/// Reads every item file of `kind`, in file name order.
fn read_items<T: DeserializeOwned>(
    root: &Path,
    kind: FileKind,
    migrations: &[Migration],
    schema: i64,
) -> Result<Vec<(PathBuf, T)>, ProjectError> {
    let Some(folder) = kind.folder() else {
        return Ok(Vec::new());
    };
    let mut items = Vec::new();
    for (stem, path) in item_files(&root.join(folder))? {
        if !is_item_name(kind, &stem) {
            return Err(ProjectError::BadFile {
                path,
                reason: format!("the file name is not a {} ID", kind_name(kind)),
            });
        }
        let text = std::fs::read_to_string(&path).map_err(io(&path))?;
        let item = parse(&path, &text, kind, migrations, schema)?;
        items.push((path, item));
    }
    Ok(items)
}

/// The `.toml` files directly in `dir` as (file stem, path), sorted by name.
/// A missing folder has none.
fn item_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, ProjectError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(dir)(e)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(io(dir))?.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        files.push((stem, path));
    }
    files.sort();
    Ok(files)
}

fn kind_name(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Project => "project",
        FileKind::Machine => "machine",
        FileKind::Setup => SetupId::KIND,
        FileKind::Tool => ToolId::KIND,
        FileKind::SelectionSet => SelectionSetId::KIND,
        FileKind::Operation => OperationId::KIND,
    }
}

fn is_item_name(kind: FileKind, stem: &str) -> bool {
    match kind {
        FileKind::Project | FileKind::Machine => false,
        FileKind::Setup => stem.parse::<SetupId>().is_ok(),
        FileKind::Tool => stem.parse::<ToolId>().is_ok(),
        FileKind::SelectionSet => stem.parse::<SelectionSetId>().is_ok(),
        FileKind::Operation => stem.parse::<OperationId>().is_ok(),
    }
}

/// Fails unless the file at `path` is named after `id`.
fn check_name(path: &Path, id: &impl std::fmt::Display) -> Result<(), ProjectError> {
    let id = id.to_string();
    if path.file_stem().and_then(|s| s.to_str()) == Some(id.as_str()) {
        Ok(())
    } else {
        Err(ProjectError::BadFile {
            path: path.to_path_buf(),
            reason: format!("the file holds item {id}, but is not named after it"),
        })
    }
}

fn item_path(kind: FileKind, id: &impl std::fmt::Display) -> String {
    format!("{}/{id}.toml", kind.folder().unwrap_or_default())
}

pub(crate) fn to_text<T: Serialize>(value: &T) -> Result<String, ProjectError> {
    let mut text = toml::to_string_pretty(value).map_err(|e| ProjectError::BadFile {
        path: PathBuf::new(),
        reason: format!("cannot write TOML: {e}"),
    })?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// The line (from 1) of the first Git merge conflict marker in `text`.
fn conflict_line(text: &str) -> Option<usize> {
    text.lines()
        .position(|line| {
            let line = line.trim_end_matches('\r');
            line.starts_with("<<<<<<<")
                || line.starts_with(">>>>>>>")
                || line.starts_with("|||||||")
                || line == "======="
        })
        .map(|i| i + 1)
}

/// Checks, migrates and deserialises one file's text.
pub(crate) fn parse<T: DeserializeOwned>(
    path: &Path,
    text: &str,
    kind: FileKind,
    migrations: &[Migration],
    current: i64,
) -> Result<T, ProjectError> {
    let bad = |reason: String| ProjectError::BadFile {
        path: path.to_path_buf(),
        reason,
    };
    // Conflict markers are not valid TOML, so look for them only once the
    // text fails to parse. A string value written over several lines can
    // legitimately have a line that starts like a marker, and this file's
    // own writer can produce one (found by the nightly fuzzer).
    let mut table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(e) => {
            return Err(match conflict_line(text) {
                Some(line) => ProjectError::MergeConflict {
                    path: path.to_path_buf(),
                    line,
                },
                None => bad(e.to_string()),
            });
        }
    };
    let schema = table
        .get("schema")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| bad("the file has no schema version".to_owned()))?;
    if schema > current {
        return Err(ProjectError::SchemaTooNew {
            path: path.to_path_buf(),
            found: schema,
            supported: current,
        });
    }
    let oldest = current - migrations.len() as i64;
    if schema < oldest {
        return Err(bad(format!(
            "schema {schema} is not a version this build can read (oldest {oldest})"
        )));
    }
    for step in migrations.iter().skip((schema - oldest) as usize) {
        step(kind, &mut table).map_err(&bad)?;
    }
    table.insert("schema".to_owned(), toml::Value::Integer(current));
    table
        .try_into()
        .map_err(|e: toml::de::Error| bad(e.to_string()))
}

/// Writes `text` unless the file already holds it (line endings aside, so a
/// checkout with Windows line endings is not rewritten).
pub(crate) fn write_if_changed(path: &Path, text: &str) -> Result<(), ProjectError> {
    match std::fs::read_to_string(path) {
        Ok(current) if current.replace("\r\n", "\n") == text => return Ok(()),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {}
        Err(e) => return Err(io(path)(e)),
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, text).map_err(io(&temp))?;
    std::fs::rename(&temp, path).map_err(io(path))
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> ProjectError + '_ {
    move |source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    /// A test-only step from schema 0, where the project name was called
    /// `title`, to schema 1. The real list is empty because schema 1 is the
    /// first (decision record 0008).
    fn rename_title(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
        if kind == FileKind::Project {
            let title = table
                .remove("title")
                .ok_or_else(|| "a schema 0 project file needs a title".to_owned())?;
            table.insert("name".to_owned(), title);
        }
        Ok(())
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-0")
    }

    #[test]
    fn req_prj_04_older_schema_migrates_on_load() {
        let project = load_with(&fixture(), &[rename_title], 1).unwrap();
        assert_eq!(project.name(), "Schema 0 fixture");
        assert_eq!(project.operations().len(), 1);
        assert_eq!(project.setups().len(), 1);
        // Without the step the same folder is refused with a clear message.
        let err = load_with(&fixture(), &[], 1).unwrap_err();
        assert!(err.to_string().contains("schema 0"), "{err}");
    }

    #[test]
    fn req_prj_04_newer_schema_is_refused() {
        let newer = SCHEMA + 1;
        let err = parse::<ProjectFile>(
            Path::new(PROJECT_FILE),
            &format!("schema = {newer}\nname = \"x\"\n"),
            FileKind::Project,
            MIGRATIONS,
            SCHEMA,
        )
        .unwrap_err();
        assert!(
            matches!(err, ProjectError::SchemaTooNew { found, .. } if found == newer),
            "{err}"
        );
        assert!(err.to_string().contains("newer OpenChuckCAM"), "{err}");
    }

    #[test]
    fn req_prj_04_every_file_records_the_schema() {
        assert_eq!(SCHEMA, 7);
        let mut project = Project::new("p");
        project
            .insert(
                crate::Item::Setup(Setup::new(SetupId::from_u128(1), "s")),
                crate::Placement::End,
            )
            .unwrap();
        for (path, text) in render(&project).unwrap() {
            assert!(text.starts_with("schema = 7\n"), "{path}: {text}");
        }
    }

    #[test]
    fn conflict_markers_are_found_by_line() {
        let text = "schema = 1\n<<<<<<< HEAD\nname = \"a\"\n=======\nname = \"b\"\n>>>>>>> other\n";
        assert_eq!(conflict_line(text), Some(2));
        assert_eq!(conflict_line("name = \"=======\"\n"), None);
    }

    #[test]
    fn req_nfr_07_marker_like_line_inside_a_string_is_not_a_conflict() {
        // Regression from the nightly fuzzer (tool_library_toml): a value with
        // a newline is written as a multi-line string, and a line of it that
        // starts like a marker made the file unreadable.
        for marker in ["|||||||", "<<<<<<< x", ">>>>>>> x", "======="] {
            let text = format!("schema = 1\nname = \"\"\"\na\n{marker}\n\"\"\"\n");
            let table = parse::<toml::Table>(Path::new("x.toml"), &text, FileKind::Project, &[], 1)
                .unwrap();
            assert_eq!(
                table["name"].as_str(),
                Some(format!("a\n{marker}\n").as_str())
            );
        }
        // A real conflict still names its line.
        let err = parse::<toml::Table>(
            Path::new("x.toml"),
            "schema = 1\n<<<<<<< HEAD\nname = \"a\"\n=======\n>>>>>>> o\n",
            FileKind::Project,
            &[],
            1,
        )
        .unwrap_err();
        assert!(
            matches!(err, ProjectError::MergeConflict { line: 2, .. }),
            "{err}"
        );
    }
}
