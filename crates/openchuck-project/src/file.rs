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
    BoundaryOptions, Descriptor, ModelRef, Operation, ParameterValue, Project, RestrictionOptions,
    SelectionSet, SelectionTarget, SetOptions, Setup, ToolSnapshot,
};
use crate::order::OrderKey;
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::setup::{SetupFrame, WorkOffset};
use openchuck_types::simulation::SimulationSettings;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{ControllerMapping, CuttingTable};
use openchuck_types::units::Millimetres;
use openchuck_types::workholding::Workholding;

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
/// - 8: tools gain the optional `centre_cutting` field, written only when
///   set (OI-117, decision record 0033).
/// - 9: selection sets gain the optional `stock_to_leave` (OP-11, per
///   selection), written only when set (decision record 0034 item 4).
/// - 10: setups gain the optional `[workholding]` table, written only when
///   it holds a held region or a tab (SET-09, OP-21, decision record 0041).
/// - 11: selection sets gain the optional `colour`, written only when the
///   user chose one (decision record 0046 item 2).
/// - 12: a selection set's target may be `mesh_edges`, edges of a mesh
///   model's triangles as `[triangle, side]` pairs (OI-204); a set of B-rep
///   edges gains the optional `keep_tangent_edges`, and `project.toml` the
///   project's default for it, `keep_tangent_edges`, written only when true;
///   `project.toml` gains `warn_on_selection_reset`, written only when false
///   (decision record 0066).
/// - 13: the machine copy's feed limits gain the optional `rotary_in_feed`,
///   written only when A counts in the feed word; a copy of the built-in
///   Makera Z1 gains `"makera"` (decision record 0076).
/// - 14: an operation gains the optional `boundary_options`, `avoid_options`
///   and `check_options` tables, each written only when it differs from the
///   default (decision record 0176 item 3).
/// - 15: `project.toml` gains `skip_stock_simulation`, written only when
///   true (decision record 0168), and `hidden_paths`, the operations whose
///   toolpath is hidden in the view, written only when one is (decision
///   record 0169). Neither is an input to a toolpath or the G-code.
/// - 16: an operation gains the optional `spindle_speed`, written only when
///   the user set one (decision record 0196).
/// - 17: a setup's `part_offset` moves only the part; the stock, the
///   workholding and the work zero stay in the setup's frame, whose origin is
///   the machine's rotary reference point (decision record 0201). An older
///   setup with an offset has its stock, tabs and work zero moved by the
///   offset, so it machines exactly as before. A retaining tab gains the
///   optional `auto` (`"head"` or `"tail"`), written only when set (decision
///   record 0202).
/// - 18: an operation gains the optional `material`, the stock material
///   whose cutting data its preset takes, written only when the user chose
///   one for it (decision record 0230); the machine copy may give the
///   spindle speed range, `min_spindle_rpm` and `max_spindle_rpm`, written
///   only when set (decision record 0231).
/// - 19: optional minimum block time, window and mode in machine copies
///   (decision record 0222); absent fields retain Off.
/// - 20: Slow down integration; schema 19 Report settings retain their meaning.
/// - 21: a model reference gains the optional `name`, the file name it had
///   when it was added, written only when recorded (issue 385, decision
///   record 0307 item 4).
pub const SCHEMA: i64 = 21;

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
    schema_7_to_8,
    schema_8_to_9,
    schema_9_to_10,
    schema_10_to_11,
    schema_11_to_12,
    schema_12_to_13,
    schema_13_to_14,
    schema_14_to_15,
    schema_15_to_16,
    schema_16_to_17,
    schema_17_to_18,
    schema_18_to_19,
    schema_19_to_20,
    schema_20_to_21,
];

/// GC-17 is additive. Older project copies retain Off, never the new Z1 default.
fn schema_18_to_19(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind == FileKind::Machine {
        crate::machines::premature_block_time(table, 18)?;
    }
    Ok(())
}

/// Schema 20 keeps schema 19 Off/Report settings and adds the Slow down policy.
fn schema_19_to_20(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind == FileKind::Machine {
        crate::machines::premature_slow_down(table, 19, 20)?;
    }
    Ok(())
}

/// Schema 21 adds the optional `name` of a model reference, which a schema 20
/// file does not have, so it needs no change but its number: the model shows
/// its path's file name. A schema 20 file that already holds one was not
/// written by any version that knew it, so it is refused rather than guessed
/// at.
fn schema_20_to_21(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    let named = kind == FileKind::Project
        && table
            .get("models")
            .and_then(toml::Value::as_array)
            .is_some_and(|models| {
                models
                    .iter()
                    .any(|m| m.as_table().is_some_and(|m| m.contains_key("name")))
            });
    if named {
        return Err(
            "this schema 20 file has a model `name`, a field schema 21 introduced; remove it, \
             or set `schema = 21` if the file was written for schema 21"
                .to_owned(),
        );
    }
    Ok(())
}

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

/// Schema 8 adds the optional `centre_cutting` tool field, written only when
/// set, so a schema 7 file needs no change but its number: its tools state
/// nothing and follow their shape. A schema 7 tool file that already holds the
/// field was not written by any version that knew it, so it is refused rather
/// than guessed at.
fn schema_7_to_8(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    let stated = kind == FileKind::Tool
        && table
            .get("tool")
            .and_then(toml::Value::as_table)
            .is_some_and(|t| t.contains_key("centre_cutting"));
    if stated {
        return Err(
            "this schema 7 tool file has `centre_cutting`, a field schema 8 introduced; \
             remove it, or set `schema = 8` if the file was written for schema 8"
                .to_owned(),
        );
    }
    Ok(())
}

/// Schema 9 adds the optional `stock_to_leave` to selection sets, which a
/// schema 8 file does not have, so it needs no change but its number.
fn schema_8_to_9(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// Schema 10 adds the optional `[workholding]` table to setup files, which a
/// schema 9 file does not have, so it needs no change but its number.
fn schema_9_to_10(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// Schema 11 adds the optional `colour` to selection sets, which a schema 10
/// file does not have (the set then takes the default swatch of its place in
/// the list), so it needs no change but its number.
fn schema_10_to_11(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// Schema 12 adds the `mesh_edges` selection target and the optional
/// `keep_tangent_edges` of edge sets and of the project, and the project's
/// optional `warn_on_selection_reset`, which a schema 11
/// file does not have, so it needs no change but its number: its edge sets
/// follow the project's default, off. A schema 11 file that already holds one
/// was not written by any version that knew it, so it is refused rather than
/// guessed at.
fn schema_11_to_12(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    let premature = match kind {
        FileKind::SelectionSet => {
            if table
                .get("target")
                .and_then(toml::Value::as_table)
                .is_some_and(|t| t.contains_key("mesh_edges"))
            {
                Some("selects `mesh_edges`, a target")
            } else if table.contains_key("keep_tangent_edges") {
                Some("has `keep_tangent_edges`, a field")
            } else {
                None
            }
        }
        FileKind::Project if table.contains_key("keep_tangent_edges") => {
            Some("has `keep_tangent_edges`, a field")
        }
        FileKind::Project if table.contains_key("warn_on_selection_reset") => {
            Some("has `warn_on_selection_reset`, a field")
        }
        _ => None,
    };
    match premature {
        Some(what) => Err(format!(
            "this schema 11 file {what} schema 12 introduced; remove it, or set `schema = 12` \
             if the file was written for schema 12"
        )),
        None => Ok(()),
    }
}

/// Schema 13 adds the machine copy's optional `max_feeds.rotary_in_feed`. A
/// copy of the built-in Makera Z1, known by its library `id`, gains
/// `"makera"`, so an older Z1 project is timed for the Z1's feed word; every
/// other file is unchanged (decision record 0076).
fn schema_12_to_13(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind == FileKind::Machine {
        crate::machines::fill_rotary_in_feed(table);
    }
    Ok(())
}

/// Schema 14 adds an operation's optional `boundary_options`, `avoid_options`
/// and `check_options` (decision record 0176 item 3), which a schema 13 file
/// does not have, so its sets take the defaults and it needs no change but its
/// number. A schema 13 file that already holds one was not written by any
/// version that knew it, so it is refused rather than guessed at.
fn schema_13_to_14(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    let premature = ["boundary_options", "avoid_options", "check_options"]
        .into_iter()
        .find(|key| kind == FileKind::Operation && table.contains_key(*key));
    match premature {
        Some(key) => Err(format!(
            "this schema 13 file has `{key}`, a table schema 14 introduced; remove it, or set \
             `schema = 14` if the file was written for schema 14"
        )),
        None => Ok(()),
    }
}

/// Schema 15 adds the optional `skip_stock_simulation` and `hidden_paths` to
/// `project.toml`, which a schema 14 file does not have, so it needs no
/// change but its number: the stock is simulated and every path is shown. A
/// schema 14 file that already holds one was not written by any version
/// that knew it, so it is refused rather than guessed at.
fn schema_14_to_15(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    let premature = ["skip_stock_simulation", "hidden_paths"]
        .into_iter()
        .find(|field| kind == FileKind::Project && table.contains_key(*field));
    match premature {
        Some(field) => Err(format!(
            "this schema 14 file has `{field}`, a field schema 15 introduced; remove it, or set \
             `schema = 15` if the file was written for schema 15"
        )),
        None => Ok(()),
    }
}

/// Schema 16 adds the optional `spindle_speed` to operation files, which a
/// schema 15 file does not have, so it needs no change but its number: the
/// operation takes the tool's cutting data as before. A schema 15 operation
/// file that already holds one was not written by any version that knew it,
/// so it is refused rather than guessed at.
fn schema_15_to_16(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind == FileKind::Operation && table.contains_key("spindle_speed") {
        return Err(
            "this schema 15 file has `spindle_speed`, a field schema 16 introduced; \
             remove it, or set `schema = 16` if the file was written for schema 16"
                .to_owned(),
        );
    }
    Ok(())
}

/// Schema 18 adds the optional `material` to operation files and the
/// optional spindle speed range to the machine copy, which a schema 17 file
/// does not have, so it needs no change but its number: the operation takes
/// the setup's stock material and the machine checks no spindle speed, as
/// before. A schema 17 file that already holds one was not written by any
/// version that knew it, so it is refused rather than guessed at.
fn schema_17_to_18(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind == FileKind::Machine {
        return crate::machines::premature_spindle_range(table, 17);
    }
    if kind == FileKind::Operation && table.contains_key("material") {
        return Err(
            "this schema 17 file has `material`, a field schema 18 introduced; \
             remove it, or set `schema = 18` if the file was written for schema 18"
                .to_owned(),
        );
    }
    Ok(())
}

/// Schema 17 (decision record 0201): before it, a setup's `part_offset`
/// placed the whole setup on the machine, the part, the stock, the tabs and
/// the work zero together, so it changed no toolpath and no G-code. From 17
/// it moves only the part, against a stock, workholding and work zero that
/// are measured from the machine's rotary reference point. So that an older
/// setup machines exactly as it did, its cylinder or box stock, its tabs and
/// its work zero are moved along X by its offset (stock that follows the
/// part, the bounding box plus an offset, moves with it already, and the held
/// regions are measured from the ends of the stock). A setup with no offset
/// is unchanged. A retaining tab's `auto` is new in 17 (decision record
/// 0202), so a schema 16 file that holds one is refused rather than guessed
/// at.
fn schema_16_to_17(kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    if kind != FileKind::Setup {
        return Ok(());
    }
    let tabs_key = |t: &toml::Table| {
        t.get("workholding")
            .and_then(toml::Value::as_table)
            .and_then(|w| w.get("tabs"))
            .and_then(toml::Value::as_array)
            .is_some_and(|tabs| {
                tabs.iter()
                    .any(|t| t.as_table().is_some_and(|t| t.contains_key("auto")))
            })
    };
    if tabs_key(table) {
        return Err(
            "this schema 16 setup file has a tab with `auto`, a field schema 17 introduced; \
             remove it, or set `schema = 17` if the file was written for schema 17"
                .to_owned(),
        );
    }
    let offset = match table.get("part_offset") {
        None => return Ok(()),
        Some(value) => number(value).ok_or("`part_offset` is not a number")?,
    };
    if offset == 0.0 {
        return Ok(());
    }
    let shift = |value: &mut toml::Value, what: &str| -> Result<(), String> {
        let v = number(value).ok_or_else(|| format!("`{what}` is not a number"))?;
        *value = toml::Value::Float(v + offset);
        Ok(())
    };
    // The work zero, which defaults to model X 0.
    let work = table
        .entry("work_offset")
        .or_insert_with(|| {
            let mut t = toml::Table::new();
            t.insert("register".to_owned(), toml::Value::String("g54".to_owned()));
            t.insert("x".to_owned(), toml::Value::Float(0.0));
            toml::Value::Table(t)
        })
        .as_table_mut()
        .ok_or("`work_offset` is not a table")?;
    shift(
        work.entry("x").or_insert(toml::Value::Float(0.0)),
        "work_offset.x",
    )?;
    if let Some(stock) = table.get_mut("stock").and_then(toml::Value::as_table_mut) {
        match stock.get("type").and_then(toml::Value::as_str) {
            Some("cylinder") => {
                if let Some(x) = stock.get_mut("x_start") {
                    shift(x, "stock.x_start")?;
                }
            }
            Some("box") => {
                for corner in ["min", "max"] {
                    if let Some(x) = stock
                        .get_mut("bounds")
                        .and_then(toml::Value::as_table_mut)
                        .and_then(|b| b.get_mut(corner))
                        .and_then(toml::Value::as_table_mut)
                        .and_then(|c| c.get_mut("x"))
                    {
                        shift(x, &format!("stock.bounds.{corner}.x"))?;
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(tabs) = table
        .get_mut("workholding")
        .and_then(toml::Value::as_table_mut)
        .and_then(|w| w.get_mut("tabs"))
        .and_then(toml::Value::as_array_mut)
    {
        for tab in tabs.iter_mut().filter_map(toml::Value::as_table_mut) {
            if let Some(x) = tab.get_mut("x_start") {
                shift(x, "workholding.tabs.x_start")?;
            }
        }
    }
    Ok(())
}

/// A TOML number as a float: hand-written files may give a whole number.
fn number(value: &toml::Value) -> Option<f64> {
    match value {
        toml::Value::Float(v) => Some(*v),
        toml::Value::Integer(v) => Some(*v as f64),
        _ => None,
    }
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
    #[serde(default, skip_serializing_if = "is_false")]
    keep_tangent_edges: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    warn_on_selection_reset: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    skip_stock_simulation: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hidden_paths: Vec<OperationId>,
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
    #[serde(default, skip_serializing_if = "Workholding::is_empty")]
    workholding: Workholding,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stock_to_leave: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    colour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    keep_tangent_edges: Option<bool>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes a reference
const fn is_false(value: &bool) -> bool {
    !*value
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes a reference
const fn is_true(value: &bool) -> bool {
    *value
}

const fn yes() -> bool {
    true
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
    #[serde(default, skip_serializing_if = "BoundaryOptions::is_default")]
    boundary_options: BoundaryOptions,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    avoid_options: BTreeMap<SelectionSetId, SetOptions>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    check_options: BTreeMap<SelectionSetId, SetOptions>,
    #[serde(default)]
    parameters: BTreeMap<String, ParameterValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spindle_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    material: Option<String>,
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

/// The entries of `options` that differ from the defaults, which are the only
/// ones written or kept (decision record 0176).
fn non_default(
    options: &BTreeMap<SelectionSetId, SetOptions>,
) -> BTreeMap<SelectionSetId, SetOptions> {
    options
        .iter()
        .filter(|(_, o)| !o.is_default())
        .map(|(s, o)| (*s, *o))
        .collect()
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
        #[serde(skip_serializing_if = "Option::is_none")]
        stock_to_leave: Option<f64>,
        /// Whether an edge set keeps tangent edges, as resolved; absent for
        /// other kinds, so their hashes are unchanged (decision record 0066).
        #[serde(skip_serializing_if = "Option::is_none")]
        keep_tangent_edges: Option<bool>,
    }
    #[derive(Serialize)]
    struct Inputs<'a> {
        selections: &'a [SelectionSetId],
        boundary: Option<SelectionSetId>,
        avoid: &'a [SelectionSetId],
        check: &'a [SelectionSetId],
        /// The sets' options, absent while they are the defaults, so the
        /// hashes of older operations are unchanged (decision record 0176).
        #[serde(skip_serializing_if = "Option::is_none")]
        options: Option<&'a RestrictionOptions>,
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
                    stock_to_leave: s.stock_to_leave,
                    keep_tangent_edges: matches!(s.target, SelectionTarget::Edges(_))
                        .then(|| project.keeps_tangent_edges(s)),
                },
            )
        })
        .collect();
    let inputs = Inputs {
        selections: &o.selections,
        boundary: o.boundary,
        avoid: &o.avoid,
        check: &o.check,
        options: Some(&o.restriction_options).filter(|r| **r != RestrictionOptions::default()),
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
        keep_tangent_edges: project.keep_tangent_edges(),
        warn_on_selection_reset: project.warn_on_selection_reset(),
        skip_stock_simulation: project.skip_stock_simulation(),
        // In machining order, and only the operations the project has, so
        // the file is the same however the choice was reached.
        hidden_paths: project
            .operations()
            .iter()
            .map(|o| o.id)
            .filter(|id| !project.path_shown(*id))
            .collect(),
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
            workholding: s.workholding.clone(),
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
            stock_to_leave: s.stock_to_leave,
            colour: s.colour.clone(),
            keep_tangent_edges: s.keep_tangent_edges,
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
            boundary_options: o.restriction_options.boundary,
            avoid_options: non_default(&o.restriction_options.avoid),
            check_options: non_default(&o.restriction_options.check),
            parameters: o.parameters.clone(),
            spindle_speed: o.spindle_speed.map(|s| s.0),
            material: o.material.clone(),
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
/// Fails if `root` already holds a project or any file or folder a project
/// owns.
pub fn create(root: &Path, name: &str) -> Result<Project, ProjectError> {
    let project_file = root.join(PROJECT_FILE);
    if project_file.exists() {
        return Err(ProjectError::BadFile {
            path: project_file,
            reason: "a project already exists here".to_owned(),
        });
    }
    // Saving removes a machine file and item files the project does not own,
    // so a folder that holds any of them is not written over (NFR-07).
    let owned = crate::new_folder::owned_names_in(root);
    if !owned.is_empty() {
        return Err(ProjectError::BadFile {
            path: root.to_path_buf(),
            reason: format!("the folder already holds {}", owned.join(", ")),
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

/// Loads a project from the texts of its files, keyed by their path in the
/// project folder with `/` between parts (`project.toml`,
/// `setups/<id>.toml`), with every check `load` makes, the cross-file ones
/// included. As in a folder, only `.toml` files directly in an item folder
/// are read and anything else is ignored. Messages name the files by these
/// keys. For fuzzing and tools (NFR-07).
pub fn load_files(files: &BTreeMap<String, String>) -> Result<Project, ProjectError> {
    let project = files
        .get(PROJECT_FILE)
        .ok_or_else(|| ProjectError::NotAProject(PathBuf::new()))?;
    let items = |kind: FileKind| -> Vec<Item> {
        let Some(folder) = kind.folder() else {
            return Vec::new();
        };
        files
            .iter()
            .filter_map(|(key, text)| {
                let name = key.strip_prefix(folder)?.strip_prefix('/')?;
                let stem = name.strip_suffix(".toml")?;
                (!stem.is_empty() && !stem.contains('/')).then(|| Item {
                    stem: stem.to_owned(),
                    path: PathBuf::from(key),
                    text: text.clone(),
                })
            })
            .collect()
    };
    let texts = Texts {
        root: PathBuf::new(),
        project: (PathBuf::from(PROJECT_FILE), project.clone()),
        machine: files
            .get(MACHINE_FILE)
            .map(|text| (PathBuf::from(MACHINE_FILE), text.clone())),
        setups: items(FileKind::Setup),
        tools: items(FileKind::Tool),
        selections: items(FileKind::SelectionSet),
        operations: items(FileKind::Operation),
    };
    assemble(texts, MIGRATIONS, SCHEMA)
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
    assemble(read_texts(root)?, migrations, schema)
}

/// One item file: its file stem, its path and its text.
struct Item {
    stem: String,
    path: PathBuf,
    text: String,
}

/// The texts of a project's files, read before any of them is checked, so
/// the same checks run on a folder and on texts in memory.
struct Texts {
    root: PathBuf,
    project: (PathBuf, String),
    machine: Option<(PathBuf, String)>,
    setups: Vec<Item>,
    tools: Vec<Item>,
    selections: Vec<Item>,
    operations: Vec<Item>,
}

/// Reads the texts of the project in the folder `root`.
fn read_texts(root: &Path) -> Result<Texts, ProjectError> {
    let project_path = root.join(PROJECT_FILE);
    let project = match std::fs::read_to_string(&project_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && root.is_dir() => {
            return Err(ProjectError::NotAProject(root.to_path_buf()));
        }
        Err(e) => return Err(io(&project_path)(e)),
    };
    let machine_path = root.join(MACHINE_FILE);
    let machine = match std::fs::read_to_string(&machine_path) {
        Ok(text) => Some((machine_path, text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io(&machine_path)(e)),
    };
    Ok(Texts {
        root: root.to_path_buf(),
        project: (project_path, project),
        machine,
        setups: read_items(root, FileKind::Setup)?,
        tools: read_items(root, FileKind::Tool)?,
        selections: read_items(root, FileKind::SelectionSet)?,
        operations: read_items(root, FileKind::Operation)?,
    })
}

/// Checks the texts of a project's files and builds the project.
fn assemble(texts: Texts, migrations: &[Migration], schema: i64) -> Result<Project, ProjectError> {
    let (project_path, text) = &texts.project;
    let file: ProjectFile = parse(project_path, text, FileKind::Project, migrations, schema)?;
    let mut machine_id = None;
    let machine = match &texts.machine {
        Some((machine_path, text)) => {
            let file: MachineFile =
                parse(machine_path, text, FileKind::Machine, migrations, schema)?;
            if let Some(id) = &file.id {
                crate::machines::validate_machine_id(id).map_err(|e| ProjectError::BadFile {
                    path: machine_path.clone(),
                    reason: e.to_string(),
                })?;
            }
            machine_id = file.id;
            Some(file.machine)
        }
        None => None,
    };
    let setups = parse_items::<SetupFile>(&texts.setups, FileKind::Setup, migrations, schema)?
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
                workholding: f.workholding,
            })
        })
        .collect::<Result<Vec<_>, ProjectError>>()?;
    let tools = parse_items::<ToolFile>(&texts.tools, FileKind::Tool, migrations, schema)?
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
    let selection_sets = parse_items::<SelectionFile>(
        &texts.selections,
        FileKind::SelectionSet,
        migrations,
        schema,
    )?
    .into_iter()
    .map(|(path, f)| {
        check_name(&path, &f.id)?;
        if f.stock_to_leave.is_some_and(|v| !v.is_finite() || v < 0.0) {
            return Err(ProjectError::BadFile {
                path: path.clone(),
                reason: "stock_to_leave must be a number of millimetres, zero or more".to_owned(),
            });
        }
        if f.colour
            .as_deref()
            .is_some_and(|c| !crate::model::is_set_colour(c))
        {
            return Err(ProjectError::BadFile {
                path: path.clone(),
                reason: "colour must be written #rrggbb in lower case".to_owned(),
            });
        }
        Ok(SelectionSet {
            id: f.id,
            order: f.order,
            name: f.name,
            model: f.model,
            model_hash: f.model_hash,
            target: f.target,
            descriptors: f.descriptors,
            stock_to_leave: f.stock_to_leave,
            colour: f.colour,
            keep_tangent_edges: f.keep_tangent_edges,
        })
    })
    .collect::<Result<Vec<_>, ProjectError>>()?;
    let operations =
        parse_items::<OperationFile>(&texts.operations, FileKind::Operation, migrations, schema)?
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
                    restriction_options: RestrictionOptions {
                        boundary: f.boundary_options,
                        avoid: non_default(&f.avoid_options),
                        check: non_default(&f.check_options),
                    },
                    parameters: f.parameters,
                    spindle_speed: f.spindle_speed.map(openchuck_types::units::SpindleSpeed),
                    material: f.material,
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
        path: texts.root.clone(),
        reason: e.to_string(),
    })?;
    project.machine_id = machine_id;
    project.set_keep_tangent_edges(file.keep_tangent_edges);
    project.set_warn_on_selection_reset(file.warn_on_selection_reset);
    project.set_skip_stock_simulation(file.skip_stock_simulation);
    project.set_paths_shown(&file.hidden_paths, false);
    project
        .set_simulation(file.simulation)
        .map_err(|e| ProjectError::BadFile {
            path: project_path.clone(),
            reason: e.to_string(),
        })?;
    Ok(project)
}

/// Reads every item file of `kind`, in file name order.
fn read_items(root: &Path, kind: FileKind) -> Result<Vec<Item>, ProjectError> {
    let Some(folder) = kind.folder() else {
        return Ok(Vec::new());
    };
    let mut items = Vec::new();
    for (stem, path) in item_files(&root.join(folder))? {
        let text = std::fs::read_to_string(&path).map_err(io(&path))?;
        items.push(Item { stem, path, text });
    }
    Ok(items)
}

/// Checks the names of item files of `kind` and parses them, in file name
/// order.
fn parse_items<T: DeserializeOwned>(
    items: &[Item],
    kind: FileKind,
    migrations: &[Migration],
    schema: i64,
) -> Result<Vec<(PathBuf, T)>, ProjectError> {
    let mut parsed = Vec::new();
    for item in items {
        if !is_item_name(kind, &item.stem) {
            return Err(ProjectError::BadFile {
                path: item.path.clone(),
                reason: format!("the file name is not a {} ID", kind_name(kind)),
            });
        }
        let value = parse(&item.path, &item.text, kind, migrations, schema)?;
        parsed.push((item.path.clone(), value));
    }
    Ok(parsed)
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
        let first = format!("schema = {SCHEMA}\n");
        let mut project = Project::new("p");
        project
            .insert(
                crate::Item::Setup(Setup::new(SetupId::from_u128(1), "s")),
                crate::Placement::End,
            )
            .unwrap();
        for (path, text) in render(&project).unwrap() {
            assert!(text.starts_with(&first), "{path}: {text}");
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
