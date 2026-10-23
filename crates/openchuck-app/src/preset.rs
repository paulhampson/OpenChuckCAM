// SPDX-License-Identifier: AGPL-3.0-or-later
//! The material preset of an operation: the cutting data its tool has for
//! one stock material (spindle speed, feed per tooth, plunge feed, stepover
//! and stepdown), which fills the operation's cutting values (issue 197;
//! TL-05, TL-09; decision record 0230).
//!
//! The preset defaults to the setup's stock material and can be chosen per
//! operation from the materials the operation's tool has cutting data for.
//! A material that is not the setup's is marked with a warning. A tool with
//! no cutting data for the material applies no preset and says so. Editing a
//! cutting value drops the preset: the values it gave become the operation's
//! own, so nothing it machines changes, and the operation shows no preset.
//!
//! The values a preset gives are not written into the operation: they follow
//! the tool's cutting data, as before decision 0230, so a project that
//! posted before posts byte for byte the same (GC-07).

use openchuck_path::Registry;
use openchuck_path::params::ParameterKind;
use openchuck_project::id::{OperationId, SetupId, ToolId};
use openchuck_project::{Item, Operation, ParameterValue, Project, ToolSnapshot};
use openchuck_types::tool::{find_cutting_data, material_key};

use crate::requirements::FROM_CUTTING_DATA;
use crate::{AppError, Command};

/// The stock material `operation`'s cutting data is looked up for: the one
/// chosen for it, else its setup's (decision record 0230).
pub(crate) fn material_of<'a>(project: &'a Project, operation: &'a Operation) -> Option<&'a str> {
    operation.material.as_deref().or_else(|| {
        project
            .setup(operation.setup)
            .and_then(|s| s.material.as_deref())
    })
}

/// One entry of the preset list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetChoice {
    /// The material chosen for the operation; `None` for the setup's.
    pub material: Option<String>,
    /// What the list shows.
    pub label: String,
}

/// Whether a preset fills the operation's cutting values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PresetState {
    /// The tool's cutting data for the material fills them.
    Applied,
    /// A cutting value was edited on the operation, so no preset applies.
    Dropped,
    /// The tool has no cutting data for the material: no preset applies.
    NoData,
    /// Neither the operation nor its setup names a material.
    NoMaterial,
    /// The operation has no tool yet.
    NoTool,
}

/// An operation's material preset as New operation and the operation
/// inspector show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialPreset {
    /// The material the preset is for, in the tool's spelling when the tool
    /// has data for it.
    pub material: Option<String>,
    /// The material chosen for the operation; `None` takes the setup's.
    pub chosen: Option<String>,
    /// The setup's stock material.
    pub setup_material: Option<String>,
    /// The list to choose from: the setup's material first, then each other
    /// material the tool has cutting data for, in name order.
    pub choices: Vec<PresetChoice>,
    /// Whether the preset applies.
    pub state: PresetState,
    /// What to know or do when it does not apply, in plain words.
    pub message: Option<String>,
    /// Set when the chosen material is not the setup's: shown with a warning
    /// triangle.
    pub differs: Option<String>,
}

fn snapshot(project: &Project, tool: Option<ToolId>) -> Option<&ToolSnapshot> {
    tool.and_then(|id| project.tools().iter().find(|t| t.id == id))
}

/// The preset of a new operation in `setup` with `tool` and the chosen
/// `material` (`None` for the setup's), for the New operation dialog.
pub fn preset_for(
    project: &Project,
    setup: SetupId,
    tool: Option<ToolId>,
    material: Option<&str>,
) -> MaterialPreset {
    let setup_material = project.setup(setup).and_then(|s| s.material.clone());
    let snapshot = snapshot(project, tool);
    let chosen = material.map(str::to_owned);
    let wanted = chosen.clone().or_else(|| setup_material.clone());
    let mut choices = vec![PresetChoice {
        material: None,
        label: match &setup_material {
            Some(m) => format!("Setup's stock material: {m}"),
            None => "Setup's stock material (none set)".to_owned(),
        },
    }];
    if let Some(snapshot) = snapshot {
        let setup_key = setup_material.as_deref().map(material_key);
        choices.extend(
            snapshot
                .cutting_data
                .keys()
                .filter(|name| Some(material_key(name)) != setup_key)
                .map(|name| PresetChoice {
                    material: Some(name.clone()),
                    label: name.clone(),
                }),
        );
    }
    // A choice the tool no longer has (its tool changed) stays in the list,
    // so the list shows what is chosen.
    if let Some(c) = &chosen
        && !choices
            .iter()
            .any(|e| e.material.as_deref().map(material_key) == Some(material_key(c)))
    {
        choices.push(PresetChoice {
            material: Some(c.clone()),
            label: c.clone(),
        });
    }
    let differs = chosen.as_deref().and_then(|c| match &setup_material {
        Some(s) if material_key(s) == material_key(c) => None,
        Some(s) => Some(format!(
            "\"{c}\" is not the setup's stock material, \"{s}\"."
        )),
        None => Some(format!(
            "The setup has no stock material; this preset is for \"{c}\"."
        )),
    });
    let (material, state, message) = match (snapshot, wanted) {
        (None, wanted) => (
            wanted,
            PresetState::NoTool,
            Some("Choose a tool to see its presets.".to_owned()),
        ),
        (Some(t), None) => (
            None,
            PresetState::NoMaterial,
            Some(if t.cutting_data.is_empty() {
                format!(
                    "The setup has no stock material, and the tool \"{}\" has no cutting data. \
                     Set the stock material in the setup and add cutting data for it in the \
                     tool library.",
                    t.name
                )
            } else {
                "The setup has no stock material. Choose one of the tool's materials, or set \
                 the stock material in the setup."
                    .to_owned()
            }),
        ),
        (Some(t), Some(wanted)) => match find_cutting_data(&t.cutting_data, &wanted) {
            Some((name, _)) => (Some(name.to_owned()), PresetState::Applied, None),
            None => {
                let other = if t.cutting_data.is_empty() {
                    String::new()
                } else {
                    "Choose another of its materials, or ".to_owned()
                };
                let add = if other.is_empty() { "Add" } else { "add" };
                (
                    Some(wanted.clone()),
                    PresetState::NoData,
                    Some(format!(
                        "The tool \"{}\" has no cutting data for \"{wanted}\", so no preset is \
                         applied. {other}{add} cutting data for \"{wanted}\" to the tool in \
                         the tool library.",
                        t.name
                    )),
                )
            }
        },
    };
    MaterialPreset {
        material,
        chosen,
        setup_material,
        choices,
        state,
        message,
        differs,
    }
}

/// Whether `operation` sets any cutting value its strategy could take from
/// the cutting data, the spindle speed included.
fn has_own_values(operation: &Operation, registry: &Registry) -> bool {
    let schema = registry
        .get(&operation.strategy)
        .map(|s| s.schema())
        .unwrap_or_default();
    operation.spindle_speed.is_some()
        || cutting_names(&schema)
            .into_iter()
            .any(|name| operation.parameters.contains_key(name))
}

/// The names of `schema`'s numbers that the cutting data can give.
fn cutting_names(schema: &openchuck_path::ParameterSchema) -> Vec<&'static str> {
    FROM_CUTTING_DATA
        .into_iter()
        .filter(|name| {
            matches!(
                schema.get(name).map(|p| &p.kind),
                Some(ParameterKind::Number { .. })
            )
        })
        .collect()
}

/// Operation `operation`'s preset. `None` when the project has no such
/// operation.
pub fn operation_preset(
    project: &Project,
    registry: &Registry,
    operation: OperationId,
) -> Option<MaterialPreset> {
    let op = project.operation(operation)?;
    let mut preset = preset_for(project, op.setup, op.tool, op.material.as_deref());
    if preset.state == PresetState::Applied && has_own_values(op, registry) {
        preset.state = PresetState::Dropped;
        preset.message = Some(
            "No preset: a cutting value was changed on this operation. Choose a material to \
             apply its preset again."
                .to_owned(),
        );
    }
    Some(preset)
}

/// The command that gives operation `operation` the preset of `material`
/// (`None`, or the setup's own material, for the setup's). When the tool has
/// cutting data for it, the operation's own cutting values are taken away so
/// the preset fills them; when it has none, the values are left as they are
/// and no preset applies. A material the tool has no data for, other than
/// the setup's, cannot be chosen.
pub fn set_operation_preset(
    project: &Project,
    registry: &Registry,
    operation: OperationId,
    material: Option<&str>,
) -> Result<Command, AppError> {
    let mut changed = project
        .operation(operation)
        .cloned()
        .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
    changed.material =
        chosen_material(project, changed.setup, changed.tool, material).map_err(|reason| {
            AppError::Operation {
                name: changed.name.clone(),
                reason,
            }
        })?;
    let snapshot = snapshot(project, changed.tool);
    let applies = snapshot
        .zip(material_of(project, &changed))
        .is_some_and(|(t, m)| find_cutting_data(&t.cutting_data, m).is_some());
    if applies {
        changed.spindle_speed = None;
        let schema = registry.get(&changed.strategy)?.schema();
        for name in cutting_names(&schema) {
            changed.parameters.remove(name);
        }
    }
    Ok(Command::Replace(Item::Operation(changed)))
}

/// The material an operation in `setup` with `tool` stores for the preset
/// of `material`: `None` for none or the setup's own material, else the
/// tool's spelling of it. Fails, saying why, for a material the tool has no
/// cutting data for.
pub(crate) fn chosen_material(
    project: &Project,
    setup: SetupId,
    tool: Option<ToolId>,
    material: Option<&str>,
) -> Result<Option<String>, String> {
    let setup_material = project.setup(setup).and_then(|s| s.material.as_deref());
    let Some(m) = material
        .map(str::trim)
        .filter(|m| setup_material.is_none_or(|s| material_key(s) != material_key(m)))
    else {
        return Ok(None);
    };
    snapshot(project, tool)
        .and_then(|t| find_cutting_data(&t.cutting_data, m))
        .map(|(name, _)| Some(name.to_owned()))
        .ok_or_else(|| format!("its tool has no cutting data for \"{m}\" to use as a preset"))
}

/// Drops the preset of `edited`, an operation one of whose cutting values
/// the user has just set: each cutting value the preset still gives becomes
/// the operation's own, as the preset gave it, so nothing it machines
/// changes. The feed is worked out at the operation's spindle speed after the
/// edit, so a new speed keeps the preset's chip load (decision record 0196
/// item 2). Without a preset there is nothing to copy.
pub(crate) fn drop_preset(project: &Project, registry: &Registry, edited: &mut Operation) {
    let Some(data) = material_of(project, edited)
        .zip(snapshot(project, edited.tool))
        .and_then(|(m, t)| find_cutting_data(&t.cutting_data, m))
        .map(|(_, d)| *d)
    else {
        return;
    };
    if edited.spindle_speed.is_none() {
        edited.spindle_speed = Some(data.spindle_speed);
    }
    let Ok(resolved) = crate::operation::parameter_values(project, edited, registry) else {
        return;
    };
    let Ok(strategy) = registry.get(&edited.strategy) else {
        return;
    };
    for name in cutting_names(&strategy.schema()) {
        if edited.parameters.contains_key(name) {
            continue;
        }
        if let Some(openchuck_path::ParameterValue::Number(v)) = resolved.values.get(name) {
            edited
                .parameters
                .insert(name.to_owned(), ParameterValue::Number(*v));
        }
    }
}
