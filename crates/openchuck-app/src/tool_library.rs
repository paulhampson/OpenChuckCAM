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

use std::collections::{BTreeMap, BTreeSet};

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

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
    /// A short stable name for the group, used in the keys that remember a
    /// fold.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::BallEnd => "ball-end",
            Self::FlatEnd => "flat-end",
            Self::BullNoseAndTapered => "bull-nose-and-tapered",
            Self::PointedAndDrill => "pointed-and-drill",
            Self::Custom => "custom",
        }
    }

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

/// How far apart two sizes may be and still count as equal, in millimetres:
/// well under what a tool size is given to.
const SIZE_TOLERANCE: f64 = 0.0005;

/// A filter on one size of a tool (TL-04), typed as text so that what the
/// user typed is what is kept: an exact value, or a minimum and a maximum
/// (owner's answer on issue 193). A field left empty is not used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SizeFilter {
    /// The size, exactly. When it is given the range is not used.
    pub exact: String,
    /// The smallest size.
    pub min: String,
    /// The largest size.
    pub max: String,
}

/// A number typed in a filter field: nothing, a value, or the sentence for
/// a field that cannot be used.
fn typed_size(text: &str, label: &str, field: &str) -> Result<Option<f64>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let value: f64 = text
        .replace(',', ".")
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
        .ok_or_else(|| format!("{label}: type a number for the {field}."))?;
    if value < 0.0 {
        return Err(format!("{label}: the {field} cannot be negative."));
    }
    Ok(Some(value))
}

impl SizeFilter {
    /// The bounds as (smallest, largest) after the checks, or the sentence
    /// for the first field that cannot be used.
    fn bounds(&self, label: &str) -> Result<(Option<f64>, Option<f64>), String> {
        if let Some(exact) = typed_size(&self.exact, label, "exact value")? {
            return Ok((Some(exact), Some(exact)));
        }
        let (min, max) = (
            typed_size(&self.min, label, "minimum")?,
            typed_size(&self.max, label, "maximum")?,
        );
        if let (Some(lo), Some(hi)) = (min, max)
            && lo > hi
        {
            return Err(format!("{label}: the minimum is above the maximum."));
        }
        Ok((min, max))
    }

    /// Whether nothing is typed.
    pub fn is_empty(&self) -> bool {
        [&self.exact, &self.min, &self.max]
            .iter()
            .all(|t| t.trim().is_empty())
    }

    /// Whether `value` passes. A filter that cannot be read passes
    /// everything (its sentence is [`ToolFilter::problem`]).
    fn accepts(&self, label: &str, value: f64) -> bool {
        let Ok((min, max)) = self.bounds(label) else {
            return true;
        };
        min.is_none_or(|lo| value >= lo - SIZE_TOLERANCE)
            && max.is_none_or(|hi| value <= hi + SIZE_TOLERANCE)
    }
}

/// The filter on the tool list: the diameter and the flute length (TL-04;
/// issue 193). Kept per user between runs and never reset on its own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolFilter {
    /// The cutting diameter.
    pub diameter: SizeFilter,
    /// The length of the fluted part.
    pub flute_length: SizeFilter,
}

impl ToolFilter {
    /// The sentence for the first field that cannot be used, if any.
    pub fn problem(&self) -> Option<String> {
        self.diameter
            .bounds("Diameter")
            .and(self.flute_length.bounds("Flute length"))
            .err()
    }

    /// Whether nothing is typed in any field.
    pub fn is_empty(&self) -> bool {
        self.diameter.is_empty() && self.flute_length.is_empty()
    }

    /// Whether `tool` passes both sizes.
    pub fn accepts(&self, tool: &Tool) -> bool {
        self.diameter.accepts("Diameter", tool.diameter.0)
            && self
                .flute_length
                .accepts("Flute length", tool.flute_length.0)
    }
}

/// What the tool list shows, kept per user (UI-15): which groups are folded
/// and the filter. Both are remembered between runs (owner's answers on
/// issues 192 and 193); neither is in a project or a library file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolLibraryView {
    /// The keys of the folded sections and groups ([`ListSection::key`],
    /// [`SectionGroup::key`]); everything else is unfolded.
    pub folded: BTreeSet<String>,
    /// The size filter.
    pub filter: ToolFilter,
}

impl ToolLibraryView {
    /// Whether the section or group with `key` is folded.
    pub fn is_folded(&self, key: &str) -> bool {
        self.folded.contains(key)
    }

    /// Folds or unfolds the section or group with `key`.
    pub fn set_folded(&mut self, key: &str, folded: bool) {
        if folded {
            self.folded.insert(key.to_owned());
        } else {
            self.folded.remove(key);
        }
    }
}

/// What the tool list is asked for: a search text and the size filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    /// Text in the name, type or material (ignoring case).
    pub search: String,
    /// The size filter.
    pub filter: ToolFilter,
}

impl ListQuery {
    /// A query for `text` alone.
    pub fn searching(text: &str) -> Self {
        Self {
            search: text.to_owned(),
            filter: ToolFilter::default(),
        }
    }
}

/// The type group of a section: its tools by name.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionGroup {
    /// A stable key, distinct across the list, for remembering a fold.
    pub key: String,
    /// The type group.
    pub group: ListGroup,
    /// Its tools.
    pub rows: Vec<ListRow>,
}

/// A section of the tool list: a machine's tools, or the tools no machine
/// holds (TL-11; issue 192).
#[derive(Debug, Clone, PartialEq)]
pub struct ListSection {
    /// A stable key for remembering a fold.
    pub key: String,
    /// The machine's name, or "Standard tools".
    pub title: String,
    /// The section's type groups in the board's order.
    pub groups: Vec<SectionGroup>,
}

/// The heading of the section for the tools no machine holds.
pub const STANDARD_TOOLS: &str = "Standard tools";

/// The tool list: the library's tools that match `query`, in a section for
/// each machine whose tool set holds them (in name order), then "Standard
/// tools" for the rest; in each section the types in the board's order, each
/// group's tools by name. A tool in several machines' sets is listed under
/// each. Empty groups and sections are left out.
pub fn tool_list(
    library: &ToolLibrary,
    project: Option<&Project>,
    query: &ListQuery,
) -> Vec<ListSection> {
    let needle = query.search.trim().to_lowercase();
    // Section key to (title, groups).
    let mut sections: BTreeMap<String, (String, BTreeMap<ListGroup, Vec<ListRow>>)> =
        BTreeMap::new();
    for t in library.tools() {
        let kind = ToolType::of(&t.tool.shape);
        let haystack = format!(
            "{}\n{}\n{}",
            t.name,
            kind.label(),
            t.tool.material.as_deref().unwrap_or_default()
        )
        .to_lowercase();
        if (!needle.is_empty() && !haystack.contains(&needle)) || !query.filter.accepts(&t.tool) {
            continue;
        }
        let row = ListRow {
            id: t.id,
            name: t.name.clone(),
            summary: summary(&t.tool),
            in_project: project.is_some_and(|p| p.tools().iter().any(|s| s.id == t.id)),
        };
        let machines: Vec<&str> = library
            .tool_sets()
            .filter(|set| set.entries.iter().any(|e| e.tool == t.id))
            .map(|set| set.machine.as_str())
            .collect();
        if machines.is_empty() {
            // The empty key sorts before every machine, so it is taken out
            // again and put last below.
            sections
                .entry(String::new())
                .or_insert_with(|| (STANDARD_TOOLS.to_owned(), BTreeMap::new()))
                .1
                .entry(kind.group())
                .or_default()
                .push(row);
            continue;
        }
        for machine in machines {
            sections
                .entry(machine.to_lowercase())
                .or_insert_with(|| (machine.to_owned(), BTreeMap::new()))
                .1
                .entry(kind.group())
                .or_default()
                .push(row.clone());
        }
    }
    let build = |key: String, title: String, groups: BTreeMap<ListGroup, Vec<ListRow>>| {
        let key = if key.is_empty() {
            "standard".to_owned()
        } else {
            format!("machine:{key}")
        };
        ListSection {
            groups: groups
                .into_iter()
                .map(|(group, mut rows)| {
                    rows.sort_by_key(|r| natural_key(&r.name));
                    SectionGroup {
                        key: format!("{key}/{}", group.slug()),
                        group,
                        rows,
                    }
                })
                .collect(),
            key,
            title,
        }
    };
    // Machines in name order, the tools no machine holds last.
    let standard = sections.remove("");
    let mut out: Vec<ListSection> = sections
        .into_iter()
        .map(|(key, (title, groups))| build(key, title, groups))
        .collect();
    out.extend(standard.map(|(title, groups)| build(String::new(), title, groups)));
    out
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

/// Whether `tool` is a default tool as the application ships it (TL-12):
/// the same ID and the same content. The window shows it read-only, with a
/// copy to edit (issue 194); a default the user has already changed is the
/// user's tool and stays editable.
pub fn is_shipped_default(tool: &LibraryTool) -> bool {
    static SHIPPED: OnceLock<Vec<LibraryTool>> = OnceLock::new();
    SHIPPED
        .get_or_init(|| openchuck_project::tools::default_tools().unwrap_or_default())
        .iter()
        .any(|shipped| shipped == tool)
}

/// The dimensions that make up `tool`, as (label, value) in words and units
/// for the read-only view of a default tool (TL-03, TL-04; issue 194): the
/// sizes every tool has, then the ones its shape adds.
pub fn default_dimensions(tool: &Tool) -> Vec<(String, String)> {
    let mm = |label: &str, v: f64| (label.to_owned(), format!("{v} mm"));
    let deg = |label: &str, v: f64, note: &str| (label.to_owned(), format!("{v}°{note}"));
    let mut rows = vec![
        mm("Diameter", tool.diameter.0),
        mm("Flute length", tool.flute_length.0),
        mm("Overall length", tool.overall_length.0),
        mm("Shank diameter", tool.shank_diameter.0),
    ];
    match &tool.shape {
        ToolShape::BullNose { corner_radius } => rows.push(mm("Corner radius", corner_radius.0)),
        ToolShape::TaperedBall {
            tip_radius,
            taper_half_angle,
        } => {
            rows.push(mm("Tip radius", tip_radius.0));
            rows.push(deg(
                "Taper angle",
                taper_half_angle.0,
                " (half angle from the axis)",
            ));
        }
        ToolShape::VBit {
            included_angle,
            tip_diameter,
        }
        | ToolShape::Chamfer {
            included_angle,
            tip_diameter,
        } => {
            rows.push(deg("Included angle", included_angle.0, ""));
            rows.push(mm("Tip diameter", tip_diameter.0));
        }
        ToolShape::Drill { point_angle } => rows.push(deg("Point angle", point_angle.0, "")),
        _ => {}
    }
    rows
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

/// New tool (12.1): a draft library tool of type `kind`, not yet saved,
/// named "New <type>" (with a number when taken). It starts from the
/// nominal sizes of the default standard tool of that type (TL-12), with no
/// cutting data and no tool material. [`save_draft`] writes it (decision
/// record 0127 item 4: library tools are drafts until Save).
pub fn new_draft(library: &ToolLibrary, kind: ToolType) -> Result<LibraryTool, AppError> {
    let example = kind.example()?;
    Ok(LibraryTool {
        id: ToolId::new(),
        name: free_name(library, &format!("New {}", kind.label().to_lowercase())),
        tool: Tool {
            material: None,
            ..example.tool
        },
        cutting_data: openchuck_types::tool::CuttingTable::new(),
    })
}

/// Duplicate (12.1): a draft copy of library tool `id` with a new ID, named
/// "<name> copy", not yet saved. Tool set numbers stay with the original (a
/// number belongs to one tool), and the copy is not in the project.
pub fn duplicate_draft(library: &ToolLibrary, id: ToolId) -> Result<LibraryTool, AppError> {
    let original = get(library, id)?;
    Ok(LibraryTool {
        id: ToolId::new(),
        name: free_name(library, &format!("{} copy", original.name)),
        ..original
    })
}

/// Whether `draft` holds changes the library does not have: a new tool, or
/// one that differs from its saved file.
pub fn is_dirty(library: &ToolLibrary, draft: &LibraryTool) -> bool {
    library.get(draft.id) != Some(draft)
}

/// Save (12.1): writes `draft` to its file, and its number in each machine
/// tool set `numbers` names (`None`: out of that set) (TL-11). Everything is
/// checked first, so a refused number or an invalid tool writes nothing.
/// Returns the tool file.
pub fn save_draft(
    library: &mut ToolLibrary,
    draft: &LibraryTool,
    numbers: &[(String, Option<u32>)],
) -> Result<PathBuf, AppError> {
    draft.validate()?;
    for (machine, number) in numbers {
        check_tool_set_number(library, machine, draft.id, *number)?;
    }
    let path = library.save_tool(draft.clone())?;
    for (machine, number) in numbers {
        set_tool_set_number(library, machine, draft.id, *number)?;
    }
    Ok(path)
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

/// Applies `edit` to a copy of `tool`, checks the whole tool and its
/// cutting data, and keeps the copy only when it is valid.
fn edited(
    tool: &mut LibraryTool,
    edit: impl FnOnce(&mut LibraryTool) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let mut copy = tool.clone();
    edit(&mut copy)?;
    copy.validate()?;
    *tool = copy;
    Ok(())
}

/// Renames `tool`. An empty name is refused.
pub fn edit_name(tool: &mut LibraryTool, name: &str) -> Result<(), AppError> {
    edited(tool, |t| {
        t.name = name.trim().to_owned();
        Ok(())
    })
}

/// Sets the tool's own material ("carbide", "HSS"); empty clears it (TL-04).
pub fn edit_material(tool: &mut LibraryTool, material: &str) -> Result<(), AppError> {
    let material = material.trim();
    edited(tool, |t| {
        t.tool.material = (!material.is_empty()).then(|| material.to_owned());
        Ok(())
    })
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

/// Changes `tool` to type `kind` ([`with_type`]).
pub fn edit_type(tool: &mut LibraryTool, kind: ToolType) -> Result<(), AppError> {
    edited(tool, |t| {
        t.tool = with_type(&t.tool, kind)?;
        Ok(())
    })
}

/// Sets one size of `tool` (the fields of [`tool_edit::tool_form`]).
pub fn edit_field(
    tool: &mut LibraryTool,
    name: &str,
    value: &ParameterValue,
) -> Result<(), AppError> {
    edited(tool, |t| {
        t.tool = tool_edit::set_tool_field(&t.tool, name, value)?;
        Ok(())
    })
}

/// Gives `tool` the custom `profile` (TL-03), with the same checks as a
/// profile file.
pub fn edit_profile(tool: &mut LibraryTool, profile: ToolProfile) -> Result<(), AppError> {
    edited(tool, |t| {
        t.tool = tool_edit::set_tool_profile(&t.tool, profile)?;
        Ok(())
    })
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

/// Sets `tool`'s cutting data for stock `material`, adding the material when
/// it is new, and taking out `replacing` when the material was renamed
/// (TL-05). Every value must be positive, the stepover no more than the
/// diameter and the stepdown no more than the flute length.
pub fn edit_cutting_data(
    tool: &mut LibraryTool,
    material: &str,
    data: CuttingData,
    replacing: Option<&str>,
) -> Result<(), AppError> {
    let material = material.trim();
    if material.is_empty() {
        return Err(AppError::Engine(openchuck_types::Error::invalid(
            "cutting_data",
            "name the stock material",
        )));
    }
    edited(tool, |t| {
        // A rename never lands on another entry; adding a material that is
        // there under the same name replaces it, as it always has.
        let key = openchuck_types::tool::material_key(material);
        let taken = t.cutting_data.keys().find(|k| {
            Some(k.as_str()) != replacing
                && openchuck_types::tool::material_key(k) == key
                && (replacing.is_some() || k.as_str() != material)
        });
        if let Some(other) = taken {
            return Err(AppError::Engine(openchuck_types::Error::invalid(
                "cutting_data",
                format!(
                    "the tool already has cutting data for {other:?}, which {material:?} would match too"
                ),
            )));
        }
        if let Some(old) = replacing {
            t.cutting_data.remove(old);
        }
        t.cutting_data.insert(material.to_owned(), data);
        Ok(())
    })
}

/// Removes `tool`'s cutting data for stock `material`.
pub fn remove_cutting_data(tool: &mut LibraryTool, material: &str) {
    tool.cutting_data.remove(material);
}

/// Library tool `id`'s number in each machine tool set of the library
/// (TL-11), by machine name; `None` where the set does not hold it.
pub fn tool_set_numbers(library: &ToolLibrary, id: ToolId) -> Vec<(String, Option<u32>)> {
    library
        .tool_sets()
        .map(|s| (s.machine.clone(), s.mapping_for(id).map(|m| m.number)))
        .collect()
}

/// Checks that tool `id` can have `number` in `machine`'s tool set: the set
/// exists, the number is at least 1 and no other tool of the set holds it.
pub fn check_tool_set_number(
    library: &ToolLibrary,
    machine: &str,
    id: ToolId,
    number: Option<u32>,
) -> Result<(), AppError> {
    let set = library
        .tool_set(machine)
        .ok_or_else(|| AppError::Project(not_found_set(machine)))?;
    let Some(n) = number else { return Ok(()) };
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
    Ok(())
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
    check_tool_set_number(library, machine, id, number)?;
    let mut set = library
        .tool_set(machine)
        .cloned()
        .ok_or_else(|| AppError::Project(not_found_set(machine)))?;
    let before = set.clone();
    match number {
        None => set.entries.retain(|e| e.tool != id),
        Some(n) => set.assign(id, ControllerMapping::numbered(n)),
    }
    if set != before {
        library.save_tool_set(set)?;
    }
    Ok(())
}

fn refuse(reason: &str) -> AppError {
    AppError::Engine(openchuck_types::Error::invalid(
        "tool number",
        reason.to_owned(),
    ))
}

fn refuse_spindle(reason: &str) -> AppError {
    AppError::Engine(openchuck_types::Error::invalid(
        "spindle speed",
        reason.to_owned(),
    ))
}

fn not_found_set(machine: &str) -> ProjectError {
    ProjectError::NotFound {
        kind: "tool set",
        id: machine.to_owned(),
    }
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
            Self::NotInLibrary => "Not in the library (kept in the project)".into(),
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

/// How `snapshot` stands against `library` (TL-07).
fn copy_state(snapshot: &ToolSnapshot, library: Option<&ToolLibrary>) -> CopyState {
    match library.map(|l| compare_snapshot(snapshot, l)) {
        None => CopyState::Unknown,
        Some(SnapshotStatus::Same) => CopyState::Same,
        Some(SnapshotStatus::Differs(fields)) => CopyState::Differs(fields),
        Some(SnapshotStatus::NotInLibrary) => CopyState::NotInLibrary,
    }
}

/// One of the project's tools, in the window's "This project" group
/// (decision record 0127 item 1).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRow {
    /// The project tool.
    pub id: ToolId,
    /// Its name.
    pub name: String,
    /// Its project tool number (TL-06).
    pub number: u32,
    /// Its type and size, for the tooltip.
    pub summary: String,
    /// The operations that use it, by name.
    pub used_by: Vec<String>,
    /// How it stands against the library (TL-07).
    pub copy: CopyState,
}

/// The project's tools whose name, type or material contains `search`, in
/// tool number order.
pub fn project_rows(
    project: &Project,
    library: Option<&ToolLibrary>,
    search: &str,
) -> Vec<ProjectRow> {
    let needle = search.trim().to_lowercase();
    let mut rows: Vec<ProjectRow> = project
        .tools()
        .iter()
        .filter(|t| {
            let haystack = format!(
                "{}\n{}\n{}",
                t.name,
                ToolType::of(&t.tool.shape).label(),
                t.tool.material.as_deref().unwrap_or_default()
            )
            .to_lowercase();
            needle.is_empty() || haystack.contains(&needle)
        })
        .map(|t| ProjectRow {
            id: t.id,
            name: t.name.clone(),
            number: t.controller.number,
            summary: summary(&t.tool),
            used_by: project
                .operations()
                .iter()
                .filter(|o| o.tool == Some(t.id))
                .map(|o| o.name.clone())
                .collect(),
            copy: copy_state(t, library),
        })
        .collect();
    rows.sort_by_key(|r| r.number);
    rows
}

/// The project's copy of tool `id` as an editable tool (its geometry, name
/// and cutting data; the number stays with the project).
pub fn project_tool_as_edit(project: &Project, id: ToolId) -> Result<LibraryTool, AppError> {
    Ok(LibraryTool::from_snapshot(project_tool(project, id)?))
}

/// The command that writes `edited` over the project's copy of the tool with
/// its ID, keeping the project's tool number and place: one undoable step
/// per change (decision record 0127 item 1). `None` when nothing changes.
/// An invalid tool is refused and the project is unchanged.
pub fn snapshot_command(
    project: &Project,
    edited: &LibraryTool,
) -> Result<Option<Command>, AppError> {
    edited.validate()?;
    let snapshot = project_tool(project, edited.id)?;
    let mut changed = edited.snapshot(snapshot.controller);
    changed.order = snapshot.order.clone();
    Ok((changed != *snapshot).then_some(Command::Replace(Item::Tool(changed))))
}

/// What "Save to library" would overwrite, for its confirmation: the fields
/// of the library tool that differ, in words; `None` when the library has no
/// such tool and the copy is added as a new one.
pub fn save_to_library_changes(
    project: &Project,
    library: &ToolLibrary,
    id: ToolId,
) -> Result<Option<Vec<String>>, AppError> {
    let snapshot = project_tool(project, id)?;
    Ok(match compare_snapshot(snapshot, library) {
        SnapshotStatus::NotInLibrary => None,
        SnapshotStatus::Same => Some(Vec::new()),
        SnapshotStatus::Differs(fields) => Some(fields.iter().map(|f| field_words(f)).collect()),
    })
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
    let copy = copy_state(snapshot, library);
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

/// An operation's spindle speed as its inspector's Tool section shows it
/// (decision record 0196).
#[derive(Debug, Clone, PartialEq)]
pub struct OperationSpindle {
    /// The speed the user gave the operation, in rpm.
    pub set: Option<f64>,
    /// The speed the tool's cutting data gives for the setup's material, in
    /// rpm, with the material's name as the tool holds it. Applies while
    /// `set` is empty.
    pub from_tool: Option<(f64, String)>,
    /// What to do about it when neither gives a speed, in plain words.
    pub missing: Option<String>,
    /// Why the speed is outside the machine's spindle speed range and what
    /// to do, in plain words (decision record 0231).
    pub out_of_range: Option<String>,
}

/// Operation `operation`'s spindle speed, where it comes from, or what is
/// missing. `None` when the project has no such operation.
pub fn operation_spindle(project: &Project, operation: OperationId) -> Option<OperationSpindle> {
    let op = project.operation(operation)?;
    let from_tool =
        crate::requirements::cutting_data(project, op).and_then(|(material, snapshot, data)| {
            openchuck_types::tool::find_cutting_data(&snapshot.cutting_data, material)
                .map(|(name, _)| (data.spindle_speed.0, name.to_owned()))
        });
    let missing = (op.spindle_speed.is_none() && from_tool.is_none())
        .then(|| crate::requirements::missing_spindle_message(project, op));
    Some(OperationSpindle {
        set: op.spindle_speed.map(|s| s.0),
        from_tool,
        missing,
        out_of_range: crate::requirements::spindle_out_of_range(project, op),
    })
}

/// The command that gives operation `operation` the spindle speed in the
/// typed `text` (revolutions per minute), or takes its own speed away when
/// `text` is `None`, so the tool's cutting data applies again. Text that is
/// not a number above 0 is refused in plain words (NFR-07). A speed the user
/// sets drops the operation's material preset (decision record 0230).
pub fn set_operation_spindle(
    project: &Project,
    registry: &openchuck_path::Registry,
    operation: OperationId,
    text: Option<&str>,
) -> Result<Command, AppError> {
    let speed = match text {
        None => None,
        Some(text) => {
            let text = text.trim();
            let number = text.strip_suffix("rpm").unwrap_or(text).trim();
            let value: f64 = number
                .parse()
                .map_err(|_| refuse_spindle("type the spindle speed as a number in rpm"))?;
            if !(value.is_finite() && value > 0.0) {
                return Err(refuse_spindle(
                    "the spindle speed must be a number above 0 rpm",
                ));
            }
            Some(openchuck_types::units::SpindleSpeed(value))
        }
    };
    let mut changed = project
        .operation(operation)
        .cloned()
        .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
    changed.spindle_speed = speed;
    if speed.is_some() {
        crate::preset::drop_preset(project, registry, &mut changed);
    }
    Ok(Command::Replace(Item::Operation(changed)))
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
