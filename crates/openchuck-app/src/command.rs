// SPDX-License-Identifier: AGPL-3.0-or-later
//! Commands and the undo and redo history (UI-08, architecture section 5.7).
//!
//! Every change to the project is a [`Command`]. Applying one returns its
//! inverse, the command that puts the project back exactly as it was, and the
//! history keeps those inverses. Undo applies the latest inverse and keeps the
//! inverse of that for redo. A new command clears the redo list. The history
//! is linear and unbounded.

use openchuck_project::id::{ModelId, OperationId};
use openchuck_project::{
    Item, ItemId, ListKind, ModelRef, OldPlace, OrderKey, ParameterValue, Placement, Project,
};
use openchuck_types::machine::MachineProfile;
use openchuck_types::transform::TransformStep;
use openchuck_types::units::LengthUnit;

use crate::AppError;

/// A change to the project. Every command is undoable (UI-08).
///
/// Undo restores order keys exactly, never by computing new ones, so a saved
/// project is byte for byte what it was (decision record 0008).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// Renames the project.
    RenameProject(String),
    /// Sets or clears the project's copy of its machine profile (SET-01).
    SetMachine(Option<Box<MachineProfile>>),
    /// Adds an item.
    Add {
        /// The new item, with a fresh ID.
        item: Item,
        /// Where it goes.
        placement: Placement,
    },
    /// Removes an item that nothing uses.
    Remove(ItemId),
    /// Replaces an item with a new version that has the same ID. Its order key
    /// and position stay.
    Replace(Item),
    /// Moves an item to a position in its list.
    Move {
        /// The item.
        id: ItemId,
        /// Its new position.
        index: usize,
    },
    /// Sets the order keys of keyed items.
    SetOrderKeys(Vec<(ItemId, OrderKey)>),
    /// Gives a list short, evenly spaced order keys (OI-35). Every item in the
    /// list changes, so this is only ever an explicit user action.
    Renumber(ListKind),
    /// Sets an operation parameter, or removes it for `None`.
    SetParameter {
        /// The operation.
        operation: OperationId,
        /// The parameter's name.
        name: String,
        /// The new value.
        value: Option<ParameterValue>,
    },
    /// Sets the unit a model file is read in, or `None` to use the file's
    /// own unit (IMP-03).
    SetModelUnits {
        /// The model.
        model: ModelId,
        /// The unit.
        units: Option<LengthUnit>,
    },
    /// Replaces a model's placement steps (IMP-05).
    SetModelTransform {
        /// The model.
        model: ModelId,
        /// The steps, applied first to last.
        transform: Vec<TransformStep>,
    },
    /// Several commands applied in order as one step; all or none.
    Batch(Vec<Command>),
}

impl Command {
    /// Applies the command and returns its inverse. On error the project is
    /// unchanged.
    pub(crate) fn apply(self, project: &mut Project) -> Result<Command, AppError> {
        Ok(match self {
            Self::RenameProject(name) => Self::RenameProject(project.set_name(name)),
            Self::SetMachine(machine) => {
                Self::SetMachine(project.set_machine(machine.map(|m| *m))?.map(Box::new))
            }
            Self::Add { item, placement } => {
                let id = item.id();
                let rekeyed = project.insert(item, placement)?;
                if rekeyed.is_empty() {
                    Self::Remove(id)
                } else {
                    Self::Batch(vec![Self::Remove(id), Self::SetOrderKeys(rekeyed)])
                }
            }
            Self::Remove(id) => {
                let (index, item) = project.remove(id)?;
                let placement = if id.list() == ListKind::Models {
                    Placement::Index(index)
                } else {
                    Placement::Keep
                };
                Self::Add { item, placement }
            }
            Self::Replace(item) => Self::Replace(project.replace(item)?),
            Self::Move { id, index } => match project.move_item(id, index)? {
                OldPlace::Keys(keys) => Self::SetOrderKeys(keys),
                OldPlace::Index(from) => Self::Move { id, index: from },
            },
            Self::SetOrderKeys(keys) => Self::SetOrderKeys(project.set_order_keys(&keys)?),
            Self::Renumber(kind) => Self::SetOrderKeys(project.renumber(kind)?),
            Self::SetParameter {
                operation,
                name,
                value,
            } => {
                let mut edited = project.operation(operation).cloned().ok_or_else(|| {
                    openchuck_project::ProjectError::NotFound {
                        kind: OperationId::KIND,
                        id: operation.to_string(),
                    }
                })?;
                let old = match value {
                    Some(value) => edited.parameters.insert(name.clone(), value),
                    None => edited.parameters.remove(&name),
                };
                project.replace(Item::Operation(edited))?;
                Self::SetParameter {
                    operation,
                    name,
                    value: old,
                }
            }
            Self::SetModelUnits { model, units } => {
                let mut edited = model_ref(project, model)?;
                let old = std::mem::replace(&mut edited.units, units);
                project.replace(Item::Model(edited))?;
                Self::SetModelUnits { model, units: old }
            }
            Self::SetModelTransform { model, transform } => {
                let mut edited = model_ref(project, model)?;
                let old = std::mem::replace(&mut edited.transform, transform);
                project.replace(Item::Model(edited))?;
                Self::SetModelTransform {
                    model,
                    transform: old,
                }
            }
            Self::Batch(commands) => {
                let mut inverses = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(project) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(e) => {
                            // Put back what was done so far, newest first.
                            for inverse in inverses.into_iter().rev() {
                                inverse.apply(project)?;
                            }
                            return Err(e);
                        }
                    }
                }
                inverses.reverse();
                Self::Batch(inverses)
            }
        })
    }
}

/// A copy of a model reference, to edit.
fn model_ref(project: &Project, model: ModelId) -> Result<ModelRef, AppError> {
    project
        .models()
        .iter()
        .find(|m| m.id == model)
        .cloned()
        .ok_or_else(|| {
            openchuck_project::ProjectError::NotFound {
                kind: ModelId::KIND,
                id: model.to_string(),
            }
            .into()
        })
}

/// The undo and redo lists.
#[derive(Debug, Clone, Default)]
pub(crate) struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    /// Applies a new command and records its inverse.
    pub(crate) fn execute(
        &mut self,
        project: &mut Project,
        command: Command,
    ) -> Result<(), AppError> {
        let inverse = command.apply(project)?;
        self.undo.push(inverse);
        self.redo.clear();
        Ok(())
    }

    /// Undoes the latest command. Returns false when there is nothing to undo.
    pub(crate) fn undo(&mut self, project: &mut Project) -> Result<bool, AppError> {
        Self::step(project, &mut self.undo, &mut self.redo)
    }

    /// Redoes the latest undone command. Returns false when there is nothing to redo.
    pub(crate) fn redo(&mut self, project: &mut Project) -> Result<bool, AppError> {
        Self::step(project, &mut self.redo, &mut self.undo)
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn step(
        project: &mut Project,
        from: &mut Vec<Command>,
        to: &mut Vec<Command>,
    ) -> Result<bool, AppError> {
        let Some(command) = from.pop() else {
            return Ok(false);
        };
        // An inverse always applies to the state it was recorded against. If it
        // ever fails, keep it so the history stays consistent with the project.
        match command.clone().apply(project) {
            Ok(inverse) => {
                to.push(inverse);
                Ok(true)
            }
            Err(e) => {
                from.push(command);
                Err(e)
            }
        }
    }
}
