// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library window's view model (specification 12.1; TL-01 to TL-07,
//! TL-11, TL-12; decision records 0014, 0081 and 0127).
//!
//! The window lists the user's library grouped by type, with a search and an
//! "in project" mark, and edits library tools: New tool, Duplicate, Delete,
//! the form (name, type, sizes, material), the custom profile, the cutting
//! data by stock material (TL-05) and each machine's tool set number (TL-11).
//! It imports and exports tools (TL-01) and adds a tool to the project
//! (TL-07, TL-11).
//!
//! Every library change is written to the tool's own file at once through
//! [`ToolLibrary`]; the default-tool ledger (decision record 0081) is never
//! touched here, so an edited default stays edited and a deleted default
//! stays deleted (TL-12). Project changes are [`Command`]s, so they undo.
//!
//! The operation inspector's Tool section reads [`operation_tool`]: the
//! project's copy of the tool, its tool number and how it compares with the
//! library (TL-07).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_project::id::{OperationId, ToolId};
use openchuck_project::{Item, ParameterValue, Placement, Project, ProjectError, ToolSnapshot};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolProfile, ToolShape};

use crate::tools::{
    LibraryTool, NumberSource, SnapshotStatus, ToolLibrary, compare_snapshot, default_controller,
    parse_tool_file,
};
use crate::{AppError, Command, tool_edit};

/// A tool type as the form's Type list offers it (TL-02, TL-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolType {
    /// Flat end mill.
    FlatEnd,
    /// Ball end mill.
    BallEnd,
    /// Bull-nose (corner radius) end mill.
    BullNose,
    /// Tapered ball nose.
    TaperedBall,
    /// V-bit.
    VBit,
    /// Chamfer mill.
    Chamfer,
    /// Drill.
    Drill,
    /// A custom revolved profile.
    Custom,
}

impl ToolType {
    /// Every type, in the order the Type list and New tool show them.
    pub const ALL: [Self; 8] = [
        Self::FlatEnd,
        Self::BallEnd,
        Self::BullNose,
        Self::TaperedBall,
        Self::VBit,
        Self::Chamfer,
        Self::Drill,
        Self::Custom,
    ];

    /// The type of `shape`.
    pub fn of(shape: &ToolShape) -> Self {
        match shape {
            ToolShape::BallEnd => Self::BallEnd,
            ToolShape::BullNose { .. } => Self::BullNose,
            ToolShape::TaperedBall { .. } => Self::TaperedBall,
            ToolShape::VBit { .. } => Self::VBit,
            ToolShape::Chamfer { .. } => Self::Chamfer,
            ToolShape::Drill { .. } => Self::Drill,
            ToolShape::Custom { .. } => Self::Custom,
            _ => Self::FlatEnd,
        }
    }

    /// The name shown in the Type list.
    pub const fn label(self) -> &'static str {
        match self {
            Self::FlatEnd => "Flat end mill",
            Self::BallEnd => "Ball end mill",
            Self::BullNose => "Bull-nose end mill",
            Self::TaperedBall => "Tapered ball nose",
            Self::VBit => "V-bit",
            Self::Chamfer => "Chamfer mill",
            Self::Drill => "Drill",
            Self::Custom => "Custom profile",
        }
    }

    /// The list group the type is shown in.
    pub const fn group(self) -> ListGroup {
        match self {
            Self::BallEnd => ListGroup::BallEnd,
            Self::FlatEnd => ListGroup::FlatEnd,
            Self::BullNose | Self::TaperedBall => ListGroup::BullNoseAndTapered,
            Self::VBit | Self::Chamfer | Self::Drill => ListGroup::PointedAndDrill,
            Self::Custom => ListGroup::Custom,
        }
    }

    /// The default standard tool of this type (TL-12), whose nominal sizes a
    /// new tool starts from. A custom profile starts from the flat end
    /// mill's outline.
    fn example(self) -> Result<LibraryTool, AppError> {
        let wanted = if self == Self::Custom {
            Self::FlatEnd
        } else {
            self
        };
        let mut tool = openchuck_project::tools::standard_tools()?
            .into_iter()
            .filter(|t| Self::of(&t.tool.shape) == wanted)
            .max_by(|a, b| a.tool.diameter.0.total_cmp(&b.tool.diameter.0))
            .ok_or_else(|| AppError::Project(not_found(ToolId::from_u128(0))))?;
        if self == Self::Custom {
            let profile = tool_edit::starting_profile(&tool.tool)
                .ok_or_else(|| AppError::Project(not_found(tool.id)))?;
            tool.tool = tool_edit::set_tool_profile(&tool.tool, profile)?;
        }
        Ok(tool)
    }
}

/// A group of the tool list (the board groups the types this way).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListGroup {
    /// Ball end mills.
    BallEnd,
    /// Flat end mills.
    FlatEnd,
    /// Bull-nose and tapered ball tools.
    BullNoseAndTapered,
    /// V-bits, chamfer mills and drills.
    PointedAndDrill,
    /// Custom profiles.
    Custom,
}

impl ListGroup {
    /// The group's heading.
    pub const fn title(self) -> &'static str {
        match self {
            Self::BallEnd => "Ball end mills",
            Self::FlatEnd => "Flat end mills",
            Self::BullNoseAndTapered => "Bull-nose and tapered",
            Self::PointedAndDrill => "V-bit, chamfer and drill",
            Self::Custom => "Custom profile",
        }
    }
}

/// One tool in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    /// The library tool.
    pub id: ToolId,
    /// Its name.
    pub name: String,
    /// Its type and size in a few words, for the tooltip.
    pub summary: String,
    /// The open project holds a copy of it (the "in project" chip).
    pub in_project: bool,
}

/// The tool list: the library's tools whose name, type or material contains
/// `search` (ignoring case), grouped by type in the board's order, each
/// group's tools by name. Empty groups are left out.
pub fn tool_list(
    library: &ToolLibrary,
    project: Option<&Project>,
    search: &str,
) -> Vec<(ListGroup, Vec<ListRow>)> {
    let needle = search.trim().to_lowercase();
    let mut groups: BTreeMap<ListGroup, Vec<ListRow>> = BTreeMap::new();
    for t in library.tools() {
        let kind = ToolType::of(&t.tool.shape);
        let haystack = format!(
            "{}\n{}\n{}",
            t.name,
            kind.label(),
            t.tool.material.as_deref().unwrap_or_default()
        )
        .to_lowercase();
        if !needle.is_empty() && !haystack.contains(&needle) {
            continue;
        }
        groups.entry(kind.group()).or_default().push(ListRow {
            id: t.id,
            name: t.name.clone(),
            summary: summary(&t.tool),
            in_project: project.is_some_and(|p| p.tools().iter().any(|s| s.id == t.id)),
        });
    }
    groups
        .into_iter()
        .map(|(g, mut rows)| {
            rows.sort_by_key(|r| natural_key(&r.name));
            (g, rows)
        })
        .collect()
}

/// A sort key that orders "2 mm" before "10 mm".
fn natural_key(name: &str) -> Vec<(String, u64)> {
    let mut key = Vec::new();
    let mut text = String::new();
    let mut digits = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            if !digits.is_empty() {
                key.push((
                    std::mem::take(&mut text),
                    digits.parse().unwrap_or(u64::MAX),
                ));
                digits.clear();
            }
            text.push(c);
        }
    }
    key.push((text, digits.parse().unwrap_or(0)));
    key
}

/// A tool's type and size in a few words: "Ball end mill · Ø3 · 2 flutes".
pub fn summary(tool: &Tool) -> String {
    format!(
        "{} · Ø{} · {} {}",
        ToolType::of(&tool.shape).label(),
        tool.diameter.0,
        tool.flute_count,
        if tool.flute_count == 1 {
            "flute"
        } else {
            "flutes"
        }
    )
}

fn not_found(id: ToolId) -> ProjectError {
    ProjectError::NotFound {
        kind: ToolId::KIND,
        id: id.to_string(),
    }
}

fn get(library: &ToolLibrary, id: ToolId) -> Result<LibraryTool, AppError> {
    library
        .get(id)
        .cloned()
        .ok_or_else(|| AppError::Project(not_found(id)))
}

/// `wanted`, or with " 2", " 3" and so on after it, whichever no library
/// tool is named (ignoring case).
fn free_name(library: &ToolLibrary, wanted: &str) -> String {
    let taken = |n: &str| library.tools().any(|t| t.name.eq_ignore_ascii_case(n));
    if !taken(wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|n| format!("{wanted} {n}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| wanted.to_owned())
}

/// New tool (12.1): a library tool of type `kind`, saved to its own file at
/// once, named "New <type>" (with a number when taken). It starts from the
/// nominal sizes of the default standard tool of that type (TL-12), with no
/// cutting data and no tool material. Returns its ID.
pub fn new_tool(library: &mut ToolLibrary, kind: ToolType) -> Result<ToolId, AppError> {
    let example = kind.example()?;
    let tool = LibraryTool {
        id: ToolId::new(),
        name: free_name(library, &format!("New {}", kind.label().to_lowercase())),
        tool: Tool {
            material: None,
            ..example.tool
        },
        cutting_data: openchuck_types::tool::CuttingTable::new(),
    };
    let id = tool.id;
    library.save_tool(tool)?;
    Ok(id)
}

/// Duplicate (12.1): a copy of library tool `id` with a new ID, named
/// "<name> copy", saved to its own file. Tool set numbers stay with the
/// original (a number belongs to one tool). Returns the copy's ID.
pub fn duplicate_tool(library: &mut ToolLibrary, id: ToolId) -> Result<ToolId, AppError> {
    let original = get(library, id)?;
    let copy = LibraryTool {
        id: ToolId::new(),
        name: free_name(library, &format!("{} copy", original.name)),
        ..original
    };
    let new = copy.id;
    library.save_tool(copy)?;
    Ok(new)
}

/// What deleting a library tool touches, for the confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteImpact {
    /// The tool's name.
    pub name: String,
    /// The open project holds a copy, which it keeps (TL-07).
    pub in_project: bool,
    /// The tool sets that list it: the machine and the number.
    pub tool_sets: Vec<(String, u32)>,
}

/// What deleting library tool `id` would touch.
pub fn delete_impact(
    library: &ToolLibrary,
    project: Option<&Project>,
    id: ToolId,
) -> Result<DeleteImpact, AppError> {
    let tool = get(library, id)?;
    Ok(DeleteImpact {
        name: tool.name,
        in_project: project.is_some_and(|p| p.tools().iter().any(|t| t.id == id)),
        tool_sets: library
            .tool_sets()
            .filter_map(|s| s.mapping_for(id).map(|m| (s.machine.clone(), m.number)))
            .collect(),
    })
}

/// Deletes library tool `id`: its file, and its entries in the machine tool
/// sets, so no set names a tool the library does not have (TL-11). A project
/// copy is untouched (TL-07). A deleted default tool is not installed again,
/// because the ledger already lists it (TL-12).
pub fn delete_tool(library: &mut ToolLibrary, id: ToolId) -> Result<(), AppError> {
    get(library, id)?;
    let sets: Vec<_> = library
        .tool_sets()
        .filter(|s| s.mapping_for(id).is_some())
        .cloned()
        .collect();
    for mut set in sets {
        set.entries.retain(|e| e.tool != id);
        library.save_tool_set(set)?;
    }
    library.remove_tool(id)?;
    Ok(())
}

/// Writes `changed` over library tool `id` after checking it.
fn save(library: &mut ToolLibrary, changed: LibraryTool) -> Result<(), AppError> {
    changed.validate()?;
    library.save_tool(changed)?;
    Ok(())
}

/// Renames library tool `id`. An empty name is refused.
pub fn set_name(library: &mut ToolLibrary, id: ToolId, name: &str) -> Result<(), AppError> {
    let mut tool = get(library, id)?;
    tool.name = name.trim().to_owned();
    save(library, tool)
}

/// Sets the tool's own material ("carbide", "HSS"); empty clears it (TL-04).
pub fn set_material(library: &mut ToolLibrary, id: ToolId, material: &str) -> Result<(), AppError> {
    let mut tool = get(library, id)?;
    let material = material.trim();
    tool.tool.material = (!material.is_empty()).then(|| material.to_owned());
    save(library, tool)
}

/// The tool with its type changed to `kind`, keeping its sizes. The new
/// type's own values (corner radius, angles) come from the default standard
/// tool of that type; a custom profile starts from the tool's own outline.
/// Fails naming the problem when the sizes do not fit the new type, for
/// example a V-bit whose cone cannot reach the diameter within the flutes.
pub fn with_type(tool: &Tool, kind: ToolType) -> Result<Tool, AppError> {
    if ToolType::of(&tool.shape) == kind {
        return Ok(tool.clone());
    }
    if kind == ToolType::Custom {
        let profile = tool_edit::starting_profile(tool).ok_or_else(|| {
            AppError::Engine(openchuck_types::Error::invalid(
                "tool.shape",
                "the tool must be valid before it can become a custom profile",
            ))
        })?;
        return tool_edit::set_tool_profile(tool, profile);
    }
    let mut changed = tool.clone();
    changed.shape = kind.example()?.tool.shape;
    changed.validate()?;
    Ok(changed)
}

/// Changes library tool `id` to type `kind` ([`with_type`]).
pub fn set_type(library: &mut ToolLibrary, id: ToolId, kind: ToolType) -> Result<(), AppError> {
    let mut tool = get(library, id)?;
    tool.tool = with_type(&tool.tool, kind)?;
    save(library, tool)
}

/// Sets one size of library tool `id` (the fields of
/// [`tool_edit::tool_form`]), after checking the whole tool and its cutting
/// data.
pub fn set_field(
    library: &mut ToolLibrary,
    id: ToolId,
    name: &str,
    value: &ParameterValue,
) -> Result<(), AppError> {
    let mut tool = get(library, id)?;
    tool.tool = tool_edit::set_tool_field(&tool.tool, name, value)?;
    save(library, tool)
}

/// Gives library tool `id` the custom `profile` (TL-03), with the same checks
/// as a profile file.
pub fn set_profile(
    library: &mut ToolLibrary,
    id: ToolId,
    profile: ToolProfile,
) -> Result<(), AppError> {
    let mut tool = get(library, id)?;
    tool.tool = tool_edit::set_tool_profile(&tool.tool, profile)?;
    save(library, tool)
}

/// One stock material's cutting data, as the right-hand table shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct CuttingRow {
    /// The stock material name, matched exactly (decision record 0014 item 5).
    pub material: String,
    /// The values.
    pub data: CuttingData,
    /// The cutting feed they give, mm/min (spindle speed × flutes × feed per
    /// tooth), shown as a derived value.
    pub feed: f64,
}

/// The tool's cutting data by stock material, in name order (TL-05).
pub fn cutting_rows(tool: &LibraryTool) -> Vec<CuttingRow> {
    tool.cutting_data
        .iter()
        .map(|(material, data)| CuttingRow {
            material: material.clone(),
            data: *data,
            feed: data.cutting_feed(&tool.tool).0,
        })
        .collect()
}

/// Sets the cutting data of library tool `id` for stock `material`, adding
/// the material when it is new (TL-05). Every value must be positive, the
/// stepover no more than the diameter and the stepdown no more than the
/// flute length; a refused entry leaves the file unchanged.
pub fn set_cutting_data(
    library: &mut ToolLibrary,
    id: ToolId,
    material: &str,
    data: CuttingData,
) -> Result<(), AppError> {
    let mut tool = get(library, id)?;
    tool.cutting_data.insert(material.trim().to_owned(), data);
    save(library, tool)
}

/// Removes the cutting data of library tool `id` for stock `material`.
pub fn remove_cutting_data(
    library: &mut ToolLibrary,
    id: ToolId,
    material: &str,
) -> Result<(), AppError> {
    let mut tool = get(library, id)?;
    tool.cutting_data.remove(material);
    save(library, tool)
}

/// Library tool `id`'s number in each machine tool set of the library
/// (TL-11), by machine name; `None` where the set does not hold it.
pub fn tool_set_numbers(library: &ToolLibrary, id: ToolId) -> Vec<(String, Option<u32>)> {
    library
        .tool_sets()
        .map(|s| (s.machine.clone(), s.mapping_for(id).map(|m| m.number)))
        .collect()
}

/// Gives library tool `id` the number `number` in `machine`'s tool set, or
/// takes it out of the set (`None`) (TL-11). A number another tool of the
/// set holds is refused, naming that tool; the offsets follow the number.
pub fn set_tool_set_number(
    library: &mut ToolLibrary,
    machine: &str,
    id: ToolId,
    number: Option<u32>,
) -> Result<(), AppError> {
    get(library, id)?;
    let mut set = library
        .tool_set(machine)
        .cloned()
        .ok_or_else(|| AppError::Project(not_found_set(machine)))?;
    match number {
        None => set.entries.retain(|e| e.tool != id),
        Some(n) => {
            if n == 0 {
                return Err(refuse("tool numbers start at 1"));
            }
            if let Some(other) = set.tool_for(n).filter(|t| *t != id) {
                let name = library
                    .get(other)
                    .map_or_else(|| other.to_string(), |t| t.name.clone());
                return Err(refuse(&format!(
                    "T{n} is already \"{name}\" in the {} tool set",
                    set.machine
                )));
            }
            set.assign(id, ControllerMapping::numbered(n));
        }
    }
    library.save_tool_set(set)?;
    Ok(())
}

fn refuse(reason: &str) -> AppError {
    AppError::Engine(openchuck_types::Error::invalid(
        "tool number",
        reason.to_owned(),
    ))
}

fn not_found_set(machine: &str) -> ProjectError {
    ProjectError::NotFound {
        kind: "tool set",
        id: machine.to_owned(),
    }
}

/// Imports a custom profile from a profile text file into library tool `id`
/// (TL-03), with the file's checks; a refused file leaves the tool as it was.
pub fn import_profile(library: &mut ToolLibrary, id: ToolId, file: &Path) -> Result<(), AppError> {
    let profile = crate::tools::load_profile(file)?;
    set_profile(library, id, profile)
}

/// Writes `profile` to the profile text file `file` (TL-03), after the same
/// checks as reading one, so an invalid profile is never written.
pub fn export_profile(tool: &Tool, profile: &ToolProfile, file: &Path) -> Result<(), AppError> {
    tool_edit::set_tool_profile(tool, profile.clone())?;
    let text = crate::tools::render_profile(profile)?;
    std::fs::write(file, text).map_err(|source| AppError::Io {
        path: file.to_path_buf(),
        source,
    })
}

/// A tool an import would bring in that differs from the library's tool with
/// the same ID.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportConflict {
    /// The ID both share.
    pub id: ToolId,
    /// The library's name for it.
    pub name: String,
    /// The fields that differ, for example `tool.diameter`.
    pub fields: Vec<String>,
}

/// What importing a folder or tool file would do, before anything is
/// written (TL-01).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportPreview {
    /// Where the tools come from.
    pub from: PathBuf,
    /// Tools new to the library.
    pub added: Vec<LibraryTool>,
    /// Tools whose ID the library has, with different contents.
    pub conflicts: Vec<(LibraryTool, ImportConflict)>,
    /// Tools the library already has exactly.
    pub unchanged: Vec<ToolId>,
}

/// Reads `from` (a library folder or one tool file) and compares it with
/// `library`, writing nothing. Fails on any file it cannot read, naming it.
pub fn preview_import(library: &ToolLibrary, from: &Path) -> Result<ImportPreview, AppError> {
    let incoming: Vec<LibraryTool> = if from.is_file() {
        let text = std::fs::read_to_string(from).map_err(|source| AppError::Io {
            path: from.to_path_buf(),
            source,
        })?;
        vec![parse_tool_file(from, &text)?]
    } else {
        if !from.is_dir() {
            return Err(AppError::Project(ProjectError::BadFile {
                path: from.to_path_buf(),
                reason: "there is no tool file or library folder here".into(),
            }));
        }
        let other = ToolLibrary::open(from)?;
        if let Some(problem) = other.problems().first() {
            return Err(AppError::Project(ProjectError::BadFile {
                path: from.to_path_buf(),
                reason: format!("nothing imported: {problem}"),
            }));
        }
        other.tools().cloned().collect()
    };
    let mut preview = ImportPreview {
        from: from.to_path_buf(),
        ..ImportPreview::default()
    };
    for tool in incoming {
        match library.get(tool.id) {
            Some(mine) if *mine == tool => preview.unchanged.push(tool.id),
            Some(mine) => {
                let snapshot = tool.snapshot(ControllerMapping::numbered(1));
                let fields = match compare_snapshot(&snapshot, library) {
                    SnapshotStatus::Differs(fields) => fields,
                    _ => Vec::new(),
                };
                let conflict = ImportConflict {
                    id: tool.id,
                    name: mine.name.clone(),
                    fields,
                };
                preview.conflicts.push((tool, conflict));
            }
            None => preview.added.push(tool),
        }
    }
    Ok(preview)
}

/// What to do with one import conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    /// The imported tool replaces the library's.
    Replace,
    /// The library's tool stays; the imported one is not written.
    KeepMine,
    /// Both stay: the imported tool is added with a new ID and " (imported)"
    /// after its name.
    KeepBoth,
}

/// What [`apply_import`] wrote.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportOutcome {
    /// New tools, including copies kept beside the library's own.
    pub added: Vec<ToolId>,
    /// Library tools replaced by the import.
    pub replaced: Vec<ToolId>,
    /// Tools left as they were: identical, or kept by choice.
    pub kept: Vec<ToolId>,
}

/// Writes `preview` into `library`, each conflict as `choice` gives it (a
/// conflict it does not name keeps the library's tool). Tool sets are not
/// imported.
pub fn apply_import(
    library: &mut ToolLibrary,
    preview: &ImportPreview,
    choice: impl Fn(ToolId) -> ConflictChoice,
) -> Result<ImportOutcome, AppError> {
    let mut outcome = ImportOutcome {
        kept: preview.unchanged.clone(),
        ..ImportOutcome::default()
    };
    for tool in &preview.added {
        library.save_tool(tool.clone())?;
        outcome.added.push(tool.id);
    }
    for (tool, conflict) in &preview.conflicts {
        match choice(conflict.id) {
            ConflictChoice::Replace => {
                library.save_tool(tool.clone())?;
                outcome.replaced.push(tool.id);
            }
            ConflictChoice::KeepMine => outcome.kept.push(tool.id),
            ConflictChoice::KeepBoth => {
                let copy = LibraryTool {
                    id: ToolId::new(),
                    name: free_name(library, &format!("{} (imported)", tool.name)),
                    ..tool.clone()
                };
                outcome.added.push(copy.id);
                outcome.kept.push(tool.id);
                library.save_tool(copy)?;
            }
        }
    }
    Ok(outcome)
}

/// Export library (TL-01): writes the tools `ids` (all when empty) into the
/// folder `to` as a library another user can import. Returns the files.
pub fn export_tools(
    library: &ToolLibrary,
    ids: &[ToolId],
    to: &Path,
) -> Result<Vec<PathBuf>, AppError> {
    Ok(library.export(ids, to)?)
}

/// The command that adds library tool `id` to `project` ("Add to project",
/// TL-07): a copy of the tool, numbered from the project machine's tool set
/// or the next free number (TL-11). Fails when the project has it already.
pub fn add_to_project(
    project: &Project,
    library: &ToolLibrary,
    id: ToolId,
) -> Result<(Command, ControllerMapping, NumberSource), AppError> {
    let tool = get(library, id)?;
    if project.tools().iter().any(|t| t.id == id) {
        return Err(AppError::Engine(openchuck_types::Error::invalid(
            "tool",
            format!("the project already has \"{}\"", tool.name),
        )));
    }
    let (controller, source) = default_controller(project, Some(library), id);
    Ok((
        Command::Add {
            item: Item::Tool(tool.snapshot(controller)),
            placement: Placement::End,
        },
        controller,
        source,
    ))
}

/// The command that replaces the project's copy of tool `id` with the
/// library's tool ("Update from library", TL-07), keeping the project's tool
/// number and offsets. `None` when they are already the same.
pub fn update_from_library(
    project: &Project,
    library: &ToolLibrary,
    id: ToolId,
) -> Result<Option<Command>, AppError> {
    let snapshot = project_tool(project, id)?;
    let entry = get(library, id)?;
    let mut updated = entry.snapshot(snapshot.controller);
    updated.order = snapshot.order.clone();
    Ok((updated != *snapshot).then_some(Command::Replace(Item::Tool(updated))))
}

/// Writes the project's copy of tool `id` into the library ("Save to
/// library"), over the library tool with the same ID or as a new one. The
/// project's tool number stays with the project.
pub fn save_to_library(
    project: &Project,
    library: &mut ToolLibrary,
    id: ToolId,
) -> Result<PathBuf, AppError> {
    let snapshot = project_tool(project, id)?;
    Ok(library.save_tool(LibraryTool::from_snapshot(snapshot))?)
}

fn project_tool(project: &Project, id: ToolId) -> Result<&ToolSnapshot, AppError> {
    project
        .tools()
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| AppError::Project(not_found(id)))
}

/// How the project's copy of a tool stands against the library (TL-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyState {
    /// No library could be read, so nothing was compared.
    Unknown,
    /// The library holds the same tool.
    Same,
    /// The library's tool differs in these fields.
    Differs(Vec<String>),
    /// The library has no such tool; the project's copy is all there is.
    NotInLibrary,
}

impl CopyState {
    /// The state in words, for the operation inspector.
    pub fn text(&self) -> String {
        match self {
            Self::Unknown => "Kept in the project; the tool library could not be read".into(),
            Self::Same => "Same as the library".into(),
            Self::Differs(fields) => {
                let names: Vec<String> = fields.iter().map(|f| field_words(f)).collect();
                format!("Differs from the library: {}", names.join(", "))
            }
            Self::NotInLibrary => "Not in the library; the project keeps its copy".into(),
        }
    }
}

/// A tool file field path in words: `tool.flute_length` gives "flute
/// length", `cutting_data.Oak.stepover` gives "Oak stepover".
pub fn field_words(path: &str) -> String {
    let path = path
        .strip_prefix("tool.")
        .or_else(|| path.strip_prefix("cutting_data."))
        .unwrap_or(path);
    match path {
        "cutting_data" => "cutting data".to_owned(),
        "shape" => "type".to_owned(),
        "flute_count" => "flutes".to_owned(),
        _ => path.replace(['_', '.'], " ").replace("shape ", ""),
    }
}

/// An operation's tool as its inspector's Tool section shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationTool {
    /// The project tool.
    pub id: ToolId,
    /// Its name.
    pub name: String,
    /// Its type and sizes in a line.
    pub description: String,
    /// Its tool number, with offsets, as the post writes them (TL-06).
    pub controller: ControllerMapping,
    /// The number the project machine's tool set gives it, when it differs
    /// from the project's (TL-11).
    pub tool_set_number: Option<(String, u32)>,
    /// How the project's copy compares with the library (TL-07).
    pub copy: CopyState,
}

/// Operation `operation`'s tool, with its state against `library`. `None`
/// when the operation has no tool or the project has no such operation.
pub fn operation_tool(
    project: &Project,
    library: Option<&ToolLibrary>,
    operation: OperationId,
) -> Option<OperationTool> {
    let id = project.operation(operation)?.tool?;
    let snapshot = project_tool(project, id).ok()?;
    let copy = match library.map(|l| compare_snapshot(snapshot, l)) {
        None => CopyState::Unknown,
        Some(SnapshotStatus::Same) => CopyState::Same,
        Some(SnapshotStatus::Differs(fields)) => CopyState::Differs(fields),
        Some(SnapshotStatus::NotInLibrary) => CopyState::NotInLibrary,
    };
    let tool_set_number = project.machine().and_then(|m| {
        let set = library?.tool_set(&m.name)?;
        let n = set.mapping_for(id)?.number;
        (n != snapshot.controller.number).then(|| (set.machine.clone(), n))
    });
    Some(OperationTool {
        id,
        name: snapshot.name.clone(),
        description: crate::tools::describe(&snapshot.tool),
        controller: snapshot.controller,
        tool_set_number,
        copy,
    })
}

/// The command that gives operation `operation` the project tool `tool`.
pub fn set_operation_tool(
    project: &Project,
    operation: OperationId,
    tool: ToolId,
) -> Result<Command, AppError> {
    project_tool(project, tool)?;
    let mut changed = project
        .operation(operation)
        .cloned()
        .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
    changed.tool = Some(tool);
    Ok(Command::Replace(Item::Operation(changed)))
}

/// The command that sets the project's tool number of `tool` (TL-06). A
/// number another project tool uses is refused, naming it.
pub fn set_project_tool_number(
    project: &Project,
    tool: ToolId,
    number: u32,
) -> Result<Command, AppError> {
    if number == 0 {
        return Err(refuse("tool numbers start at 1"));
    }
    if let Some(other) = project
        .tools()
        .iter()
        .find(|t| t.id != tool && t.controller.number == number)
    {
        return Err(refuse(&format!(
            "T{number} is already \"{}\" in this project",
            other.name
        )));
    }
    let mut changed = project_tool(project, tool)?.clone();
    changed.controller = ControllerMapping {
        number,
        ..changed.controller
    };
    Ok(Command::Replace(Item::Tool(changed)))
}

/// The command that removes the project's copy of `tool` ("Remove from
/// project"). Refused while an operation uses it, naming the operations.
pub fn remove_from_project(project: &Project, tool: ToolId) -> Result<Command, AppError> {
    let snapshot = project_tool(project, tool)?;
    let users: Vec<String> = project
        .operations()
        .iter()
        .filter(|o| o.tool == Some(tool))
        .map(|o| format!("\"{}\"", o.name))
        .collect();
    if !users.is_empty() {
        return Err(AppError::Engine(openchuck_types::Error::invalid(
            "tool",
            format!(
                "\"{}\" is used by {}; give those operations another tool first",
                snapshot.name,
                users.join(", ")
            ),
        )));
    }
    Ok(Command::Remove(openchuck_project::ItemId::Tool(tool)))
}
