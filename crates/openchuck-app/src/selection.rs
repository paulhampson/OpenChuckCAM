// SPDX-License-Identifier: AGPL-3.0-or-later
//! Selecting triangles, saving them as named sets, and limiting an operation
//! to them (UI-04, UI-05, PRJ-06, OP-02; work breakdown slice 10 items 2 to
//! 5).
//!
//! **Making a selection.** [`Application::select_region`] answers a
//! [`RegionRequest`] (flood fill, flood fill by angle, select by direction)
//! on the model as it is placed, and returns the triangles. Triangle `i` is
//! the `i`th facet of the model file, whatever the placement, so the same
//! numbers stay right when the model is moved, turned or mirrored; a
//! direction is given in the placed model frame.
//!
//! **Sets.** [`Application::save_selection`] stores triangles as a named set
//! on the model, [`Application::edit_selection`] changes one, and both are
//! undoable commands like any other edit. A set records the model's content
//! hash when it was made (PRJ-06). An operation refers to sets by ID in four
//! roles (the sets it machines, a containment boundary, avoid sets and check
//! sets), so editing a set changes every operation that uses it:
//! [`Application::edit_selection`] returns those operations, and
//! [`openchuck_project::file::selection_inputs_hash`] changes for exactly
//! them.
//!
//! **Using a set.** When an operation is generated each set is resolved
//! against the model it names. A set that cannot be resolved stops the
//! operation with an error that names the set and says why: a different
//! version of the model (the hash differs), a different model from the
//! setup's part, a triangle the model does not have, a set that selects
//! nothing, or a face or edge selection on a mesh. Nothing is dropped
//! silently (verification policy 8).
//!
//! **Seeing them (slice 10 items 1 and 6).** [`Application::selection_summaries`]
//! lists the sets for the selection panel with what each selects, the
//! operations that use it and any problem that can be seen without reading
//! the model (UI-05, OP-02); [`Application::set_operation_selection`] puts a
//! set in or takes it out of one of an operation's four roles; and
//! [`Application::delete_selection`] removes a set nothing uses. What a click
//! in the viewport selects is in [`crate::pick`].

use std::collections::{BTreeMap, BTreeSet};

use openchuck_geom::edges::EdgeMap;
use openchuck_geom::select;
use openchuck_path::{
    BrepSurface, LeaveZone, MeshSurface, PathWarning, Protection, RegionConstraint, SurfaceSource,
    WatchedSurface, Window,
};
use openchuck_project::id::{ModelId, OperationId, SelectionSetId};
use openchuck_project::{
    BoundaryOptions, ContentHash, Item, ItemId, ModelRef, OnBreak, Operation, Placement, Project,
    SelectionSet, SelectionTarget, SetOptions,
};
use openchuck_types::brep::{BrepModel, FaceId};
use openchuck_types::geometry::Vec3;
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;

use crate::{AppError, Application, Command, JobHandle};

pub use openchuck_geom::select::TriangleSet;

/// A way to pick triangles of a mesh model (UI-04).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RegionRequest {
    /// Every triangle connected to the seeds through shared edges.
    FloodFill {
        /// The triangles to start from.
        seeds: Vec<u32>,
    },
    /// The connected triangles reached without crossing an edge where the
    /// surface turns by more than `max_edge_angle` degrees.
    FloodFillByAngle {
        /// The triangles to start from.
        seeds: Vec<u32>,
        /// The most the normals of two neighbours may differ, degrees, 0 to 180.
        max_edge_angle: f64,
    },
    /// Every triangle whose normal is within `max_angle` degrees of
    /// `direction`, in the placed model frame.
    ByDirection {
        /// The direction; it need not be a unit vector.
        direction: Vec3,
        /// The widest angle from the direction, degrees, 0 to 180.
        max_angle: f64,
    },
}

/// A change to what a selection set selects, or to its name (UI-05). What is
/// added, taken out or put in place must be of the set's kind: triangles,
/// mesh edges, faces or B-rep edges.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SelectionEdit {
    /// Adds to the selection.
    Add(SelectionTarget),
    /// Takes out of the selection.
    Remove(SelectionTarget),
    /// Replaces the selection.
    Replace(SelectionTarget),
    /// Renames the set.
    Rename(String),
    /// Sets the stock to leave on the set, in millimetres, or clears it so the
    /// operation's value applies (OP-11).
    StockToLeave(Option<f64>),
    /// Sets the colour the set is shown in, as `#rrggbb` in lower case, or
    /// clears it so the default swatch applies (decision record 0046). It
    /// does not change what an operation cuts.
    Colour(Option<String>),
    /// For a set of a STEP model's edges: keeps the edges where two faces
    /// meet smoothly (`Some(true)`), drops them (`Some(false)`), or follows
    /// the project's default (`None`) (decision record 0066).
    KeepTangentEdges(Option<bool>),
}

/// The part an operation gives a selection set (OP-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionRole {
    /// The set is machined: the operation cuts only there.
    Machine,
    /// The set bounds where the tool may cut (a window of X and angle).
    Boundary,
    /// The set is never cut.
    Avoid,
    /// The tool must not touch the set.
    Check,
}

impl SelectionRole {
    /// Every role, in the order the panel lists them.
    pub const ALL: [Self; 4] = [Self::Machine, Self::Boundary, Self::Avoid, Self::Check];

    /// The role's name for a panel, as the interface specification names
    /// it (section 8; decision record 0114 C10). The stored roles keep their
    /// names.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Machine => "Cut on",
            Self::Boundary => "Contain within",
            Self::Avoid => "Avoid",
            Self::Check => "Check",
        }
    }

    /// What the role does, for a tooltip (decision record 0032 item 5).
    pub const fn description(self) -> &'static str {
        match self {
            Self::Machine => "The operation cuts only on these surfaces.",
            Self::Boundary => {
                "The tool may cut only inside the window of X and angle this set spans."
            }
            Self::Avoid => "These surfaces are never cut; the tool may pass over them.",
            Self::Check => "The tool must not touch these surfaces.",
        }
    }
}

/// An operation that uses a selection set, and how.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionUse {
    /// The operation.
    pub operation: OperationId,
    /// Its name.
    pub name: String,
    /// The roles the set has in it.
    pub roles: Vec<SelectionRole>,
}

/// One selection set as the selection panel shows it (slice 10 item 6).
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionSummary {
    /// The set.
    pub id: SelectionSetId,
    /// Its name.
    pub name: String,
    /// The model it was made on.
    pub model: ModelId,
    /// That model's path in the project, or `None` if the model is gone.
    pub model_path: Option<String>,
    /// What it holds: "triangles", "faces" or "edges".
    pub kind: &'static str,
    /// How many.
    pub count: usize,
    /// The operations that use it.
    pub uses: Vec<SelectionUse>,
    /// Why it cannot be used as it is, worded for the user.
    pub problem: Option<String>,
    /// The colour the user chose for it (`#rrggbb`), or `None` for the
    /// default swatch of its place in the list.
    pub colour: Option<String>,
}

/// A selection set whose target has been checked against its model, ready to
/// be stored by [`Application::finish_save_selection`] or
/// [`Application::finish_edit_selection`].
#[derive(Debug, Clone)]
pub struct CheckedSet {
    set: SelectionSet,
    /// The set an edit was made from, or `None` for a new set.
    base: Option<SelectionSet>,
}

/// The start of an edit: ready at once, or checking against the model as a
/// background job.
#[derive(Debug)]
pub enum EditStart {
    /// The edit needs no model, so there is nothing to wait for.
    Done(Box<CheckedSet>),
    /// The edited target is being checked against the model.
    Checking(JobHandle<CheckedSet>),
}

/// Checks an edited set's target against its model as read now, which must
/// still be the file the set was made on: its IDs name that file.
fn check_edited(
    root: &std::path::Path,
    reference: &openchuck_project::ModelRef,
    set: &SelectionSet,
    context: &JobContext<'_>,
) -> Result<(), AppError> {
    let placed = crate::model::placed_part(root, reference, context)?;
    context.check_cancelled()?;
    if placed.hash != set.model_hash {
        return Err(selection_error(
            &set.name,
            format!(
                "'{}' has changed since the set was made; select again on the current model \
                 (PRJ-06)",
                reference.label()
            ),
        ));
    }
    check_target(&set.name, &placed.mesh, placed.brep.as_ref(), &set.target)
}

/// Checks the target that replaces a set's whole selection, read from the
/// model as it is now, and makes the set current: the new target was picked
/// on this file, so the set takes the file's hash. Unlike [`check_edited`],
/// it does not need the set to have been made on this file, because nothing
/// of the old target is kept (a stale set is repaired by picking again).
/// The file must still be the one the project holds.
fn check_replaced(
    root: &std::path::Path,
    reference: &openchuck_project::ModelRef,
    set: &mut SelectionSet,
    context: &JobContext<'_>,
) -> Result<(), AppError> {
    let placed = crate::model::placed_part(root, reference, context)?;
    context.check_cancelled()?;
    if placed.hash != reference.hash {
        return Err(selection_error(
            &set.name,
            format!(
                "'{}' has changed since it was added to the project; accept the change before \
                 selecting on it (PRJ-02)",
                reference.label()
            ),
        ));
    }
    set.model_hash = placed.hash;
    check_target(&set.name, &placed.mesh, placed.brep.as_ref(), &set.target)
}

fn selection_error(name: &str, reason: impl Into<String>) -> AppError {
    AppError::Selection {
        name: name.to_owned(),
        reason: reason.into(),
    }
}

impl Application {
    /// The triangles `request` picks on the mesh model `model`, placed as the
    /// project places it (UI-04). Nothing is stored: save the result with
    /// [`save_selection`](Self::save_selection). Waits for
    /// [`start_select_region`](Self::start_select_region).
    pub fn select_region(
        &self,
        model: ModelId,
        request: &RegionRequest,
    ) -> Result<TriangleSet, AppError> {
        self.start_select_region(model, request)?.wait()
    }

    /// Starts [`select_region`](Self::select_region) as a background job, so
    /// loading the mesh, building the edge map and filling are cancellable
    /// and reported through the job events (UI-09, NFR-04). Fails at once if
    /// the project has no such model.
    pub fn start_select_region(
        &self,
        model: ModelId,
        request: &RegionRequest,
    ) -> Result<JobHandle<TriangleSet>, AppError> {
        let reference = self.model_ref(model)?.clone();
        let root = self.document.location.clone();
        let request = request.clone();
        Ok(self
            .jobs
            .submit("select region", self.events_tx.clone(), move |context| {
                let (mesh, _) = crate::model::placed_mesh_hashed(&root, &reference, context)?;
                context.check_cancelled()?;
                Ok(match &request {
                    RegionRequest::FloodFill { seeds } => {
                        select::flood_fill(&mesh, &EdgeMap::new(&mesh)?, seeds)?
                    }
                    RegionRequest::FloodFillByAngle {
                        seeds,
                        max_edge_angle,
                    } => select::flood_fill_by_angle(
                        &mesh,
                        &EdgeMap::new(&mesh)?,
                        seeds,
                        *max_edge_angle,
                    )?,
                    RegionRequest::ByDirection {
                        direction,
                        max_angle,
                    } => select::select_by_direction(&mesh, *direction, *max_angle)?,
                })
            }))
    }

    /// Saves `target` as a new named set on `model`, at the end of the list
    /// (UI-05, PRJ-06): triangles or edges of a mesh model's triangles, or
    /// faces or edges of a STEP model (slice 11 item 7; decision record
    /// 0066). Fails if it selects nothing, names something the model does not
    /// have, is of a kind the model is not selected by, or the model file has
    /// changed since it was added to the project. Undoable. Waits for
    /// [`start_save_selection`](Self::start_save_selection).
    pub fn save_selection(
        &mut self,
        name: &str,
        model: ModelId,
        target: impl Into<SelectionTarget>,
    ) -> Result<SelectionSetId, AppError> {
        let job = self.start_save_selection(name, model, target)?;
        self.finish_save_selection(job.wait()?)
    }

    /// Starts the check of [`save_selection`](Self::save_selection) as a
    /// background job, so reading and importing the model never blocks the
    /// caller (NFR-04). The job is cancellable and reported through the job
    /// events. Give its result to
    /// [`finish_save_selection`](Self::finish_save_selection), which stores
    /// the set. Fails at once if the project has no such model.
    pub fn start_save_selection(
        &self,
        name: &str,
        model: ModelId,
        target: impl Into<SelectionTarget>,
    ) -> Result<JobHandle<CheckedSet>, AppError> {
        let target = target.into();
        let reference = self.model_ref(model)?.clone();
        let root = self.document.location.clone();
        let name = name.to_owned();
        Ok(self
            .jobs
            .submit("save selection", self.events_tx.clone(), move |context| {
                // A STEP model's faces and edges are checked against its
                // placed B-rep.
                let placed = crate::model::placed_part(&root, &reference, context)?;
                context.check_cancelled()?;
                check_target(&name, &placed.mesh, placed.brep.as_ref(), &target)?;
                if placed.hash != reference.hash {
                    return Err(selection_error(
                        &name,
                        format!(
                            "'{}' has changed since it was added to the project; accept the \
                             change before selecting on it (PRJ-02)",
                            reference.label()
                        ),
                    ));
                }
                Ok(CheckedSet {
                    set: SelectionSet {
                        id: SelectionSetId::new(),
                        order: Default::default(),
                        name,
                        model,
                        model_hash: placed.hash,
                        target,
                        descriptors: Vec::new(),
                        stock_to_leave: None,
                        colour: None,
                        keep_tangent_edges: None,
                    },
                    base: None,
                })
            }))
    }

    /// Stores a set whose check [`start_save_selection`](Self::start_save_selection)
    /// finished, as one undoable step. Fails if the model was taken out of the
    /// project, or its recorded file changed, while the check ran.
    pub fn finish_save_selection(
        &mut self,
        checked: CheckedSet,
    ) -> Result<SelectionSetId, AppError> {
        let set = checked.set;
        let reference = self.model_ref(set.model)?;
        if reference.hash != set.model_hash {
            return Err(selection_error(
                &set.name,
                format!(
                    "'{}' changed in the project while the selection was checked; select again",
                    reference.label()
                ),
            ));
        }
        let id = set.id;
        self.execute(Command::Add {
            item: Item::SelectionSet(set),
            placement: Placement::End,
        })?;
        Ok(id)
    }

    /// Changes selection set `set` (UI-05) and returns the operations that
    /// use it, which now need regenerating. The set's model hash is kept as it
    /// was: a set made on one version of the model is not made current by
    /// adding to it or taking from it (those are refused for a set made on an
    /// older version). Replacing the whole selection makes the set current,
    /// since nothing of the old target is kept: it takes the hash of the
    /// model file the new target was picked on (slice 22 lane X6, decision
    /// record 0125). Undoable. Waits for
    /// [`start_edit_selection`](Self::start_edit_selection).
    pub fn edit_selection(
        &mut self,
        set: SelectionSetId,
        edit: SelectionEdit,
    ) -> Result<Vec<openchuck_project::id::OperationId>, AppError> {
        match self.start_edit_selection(set, edit)? {
            EditStart::Done(checked) => self.finish_edit_selection(*checked),
            EditStart::Checking(job) => {
                let checked = job.wait()?;
                self.finish_edit_selection(checked)
            }
        }
    }

    /// Prepares an edit of selection set `set`. An edit that does not change
    /// what the set selects (a rename, the stock to leave, the colour, the
    /// tangent choice) needs no model and comes back ready. `Add`, `Remove`
    /// and `Replace` are checked against the model as a background job, so
    /// importing it never blocks the caller (NFR-04); the job is cancellable
    /// and reported through the job events. Give the result to
    /// [`finish_edit_selection`](Self::finish_edit_selection).
    pub fn start_edit_selection(
        &self,
        set: SelectionSetId,
        edit: SelectionEdit,
    ) -> Result<EditStart, AppError> {
        let base = self
            .document
            .project
            .selection_sets()
            .iter()
            .find(|s| s.id == set)
            .cloned()
            .ok_or_else(|| AppError::UnknownSelection(set.to_string()))?;
        let replaces = matches!(edit, SelectionEdit::Replace(_));
        let mut edited = base.clone();
        match edit {
            SelectionEdit::Rename(name) => edited.name = name,
            SelectionEdit::StockToLeave(leave) => {
                if leave.is_some_and(|v| !v.is_finite() || v < 0.0) {
                    return Err(selection_error(
                        &edited.name,
                        "the stock to leave must be zero or more millimetres",
                    ));
                }
                edited.stock_to_leave = leave;
            }
            SelectionEdit::Colour(colour) => {
                if colour
                    .as_deref()
                    .is_some_and(|c| !openchuck_project::is_set_colour(c))
                {
                    return Err(selection_error(
                        &edited.name,
                        "the colour must be written #rrggbb in lower case",
                    ));
                }
                edited.colour = colour;
            }
            SelectionEdit::KeepTangentEdges(keep) => {
                if keep.is_some() && !matches!(edited.target, SelectionTarget::Edges(_)) {
                    return Err(selection_error(
                        &edited.name,
                        "keeping tangent edges applies to a set of a STEP model's edges only",
                    ));
                }
                edited.keep_tangent_edges = keep;
            }
            SelectionEdit::Add(other) => combine(&mut edited, other, Join::Add)?,
            SelectionEdit::Remove(other) => combine(&mut edited, other, Join::Remove)?,
            SelectionEdit::Replace(other) => combine(&mut edited, other, Join::Replace)?,
        }
        let changed_target = edited.target != base.target;
        let checked = CheckedSet {
            set: edited,
            base: Some(base),
        };
        // A replacement is always checked against the file, even with the
        // same target: it makes a set made on an older version current.
        if !changed_target && !replaces {
            return Ok(EditStart::Done(Box::new(checked)));
        }
        let reference = self.model_ref(checked.set.model)?.clone();
        let root = self.document.location.clone();
        Ok(EditStart::Checking(self.jobs.submit(
            "edit selection",
            self.events_tx.clone(),
            move |context| {
                let mut checked = checked;
                if replaces {
                    check_replaced(&root, &reference, &mut checked.set, context)?;
                } else {
                    check_edited(&root, &reference, &checked.set, context)?;
                }
                Ok(checked)
            },
        )))
    }

    /// Stores an edit prepared by [`start_edit_selection`](Self::start_edit_selection)
    /// as one undoable step and returns the operations that use the set.
    /// Fails if the set was changed or removed meanwhile: the edit was made
    /// from the set as it stood when it started.
    pub fn finish_edit_selection(
        &mut self,
        checked: CheckedSet,
    ) -> Result<Vec<openchuck_project::id::OperationId>, AppError> {
        let set = checked.set;
        let current = self
            .document
            .project
            .selection_sets()
            .iter()
            .find(|s| s.id == set.id);
        if current != checked.base.as_ref() {
            return match current {
                None => Err(AppError::UnknownSelection(set.id.to_string())),
                Some(_) => Err(selection_error(
                    &set.name,
                    "the set changed while the edit was checked; edit it again",
                )),
            };
        }
        // A replacement re-stamps the set with the file's hash; the file must
        // still be the one the project holds.
        if checked
            .base
            .as_ref()
            .is_some_and(|b| b.model_hash != set.model_hash)
        {
            let reference = self.model_ref(set.model)?;
            if reference.hash != set.model_hash {
                return Err(selection_error(
                    &set.name,
                    format!(
                        "'{}' changed in the project while the selection was checked; select again",
                        reference.label()
                    ),
                ));
            }
        }
        let id = set.id;
        self.execute(Command::Replace(Item::SelectionSet(set)))?;
        Ok(self.document.project.operations_using(id))
    }

    /// Puts `set` in, or takes it out of, `role` of `operation` (OP-02, UI-05).
    /// One undoable step. A boundary holds one set, so putting one in replaces
    /// the other, and taking out a set that is not the boundary changes
    /// nothing. Fails if the operation or set does not exist.
    pub fn set_operation_selection(
        &mut self,
        operation: OperationId,
        role: SelectionRole,
        set: SelectionSetId,
        included: bool,
    ) -> Result<(), AppError> {
        let found = self
            .document
            .project
            .selection_sets()
            .iter()
            .find(|s| s.id == set)
            .ok_or_else(|| AppError::UnknownSelection(set.to_string()))?;
        let mut edited = self
            .document
            .project
            .operation(operation)
            .cloned()
            .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
        // A set made on another model can never be resolved against this
        // operation's part (generation refuses it), so it is not put in.
        // Taking one out is always allowed.
        if included
            && let Some(part) = self
                .document
                .project
                .setup(edited.setup)
                .and_then(|s| s.part)
            && found.model != part
        {
            return Err(AppError::Selection {
                name: found.name.clone(),
                reason: format!(
                    "it was made on a different model from the part of the setup of '{}'",
                    edited.name
                ),
            });
        }
        let list = match role {
            SelectionRole::Machine => &mut edited.selections,
            SelectionRole::Avoid => &mut edited.avoid,
            SelectionRole::Check => &mut edited.check,
            SelectionRole::Boundary => {
                // The boundary's options belong to the set: another set, or
                // none, starts from the defaults (decision 0176).
                match (included, edited.boundary == Some(set)) {
                    (true, true) => {}
                    (true, false) => {
                        edited.boundary = Some(set);
                        edited.restriction_options.boundary = BoundaryOptions::default();
                    }
                    (false, true) => {
                        edited.boundary = None;
                        edited.restriction_options.boundary = BoundaryOptions::default();
                    }
                    (false, false) => {}
                }
                return self.replace_operation(edited);
            }
        };
        match (included, list.contains(&set)) {
            (true, false) => list.push(set),
            (false, true) => list.retain(|s| *s != set),
            _ => return Ok(()),
        }
        // A set taken out of a role takes its options with it.
        if !included {
            edited
                .restriction_options
                .avoid
                .retain(|s, _| edited.avoid.contains(s));
            edited
                .restriction_options
                .check
                .retain(|s, _| edited.check.contains(s));
        }
        self.replace_operation(edited)
    }

    /// Sets the options of the boundary set of `operation` (decision 0176
    /// item 3): whether it limits the pass and the cut at the tool tip. One
    /// undoable step. Fails if the operation does not exist or has no
    /// boundary set.
    pub fn set_boundary_options(
        &mut self,
        operation: OperationId,
        options: BoundaryOptions,
    ) -> Result<(), AppError> {
        let mut edited = self.operation_to_edit(operation)?;
        if edited.boundary.is_none() {
            return Err(openchuck_types::Error::invalid(
                format!("operation '{}' boundary options", edited.name),
                "the operation has no boundary set for these options to apply to",
            )
            .into());
        }
        edited.restriction_options.boundary = options;
        self.replace_operation(edited)
    }

    /// Sets what happens where the tool would break avoid or check set `set`
    /// of `operation` (decision 0176 item 3). One undoable step. Fails if the
    /// operation does not exist, `role` is not avoid or check, or the set is
    /// not in that role.
    pub fn set_on_break(
        &mut self,
        operation: OperationId,
        role: SelectionRole,
        set: SelectionSetId,
        on_break: OnBreak,
    ) -> Result<(), AppError> {
        let mut edited = self.operation_to_edit(operation)?;
        let options = SetOptions { on_break };
        let (sets, name) = match role {
            SelectionRole::Avoid => (&edited.avoid, "avoid"),
            SelectionRole::Check => (&edited.check, "check"),
            _ => {
                return Err(openchuck_types::Error::invalid(
                    format!("operation '{}' set options", edited.name),
                    "only an avoid or check set has this option",
                )
                .into());
            }
        };
        if !sets.contains(&set) {
            return Err(openchuck_types::Error::invalid(
                format!("operation '{}' {name} options", edited.name),
                format!("selection set {set} is not one of the operation's {name} sets"),
            )
            .into());
        }
        if role == SelectionRole::Avoid {
            edited.restriction_options.set_avoid(set, options);
        } else {
            edited.restriction_options.set_check(set, options);
        }
        self.replace_operation(edited)
    }

    fn operation_to_edit(&self, operation: OperationId) -> Result<Operation, AppError> {
        self.document
            .project
            .operation(operation)
            .cloned()
            .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))
    }

    fn replace_operation(&mut self, edited: Operation) -> Result<(), AppError> {
        if self.document.project.operation(edited.id) == Some(&edited) {
            return Ok(());
        }
        self.execute(Command::Replace(Item::Operation(edited)))
    }

    /// Removes selection set `set` (UI-05). Fails, naming the operations, if
    /// any operation still uses it. Undoable.
    pub fn delete_selection(&mut self, set: SelectionSetId) -> Result<(), AppError> {
        let found = self
            .document
            .project
            .selection_sets()
            .iter()
            .find(|s| s.id == set)
            .map(|s| s.name.clone())
            .ok_or_else(|| AppError::UnknownSelection(set.to_string()))?;
        let users: Vec<String> = self
            .document
            .project
            .operations_using(set)
            .into_iter()
            .filter_map(|id| self.document.project.operation(id).map(|o| o.name.clone()))
            .collect();
        if !users.is_empty() {
            return Err(selection_error(
                &found,
                format!(
                    "it is used by {}; take it out of the operation first",
                    users.join(", ")
                ),
            ));
        }
        self.execute(Command::Remove(ItemId::SelectionSet(set)))
    }

    /// The selection sets for a panel (slice 10 item 6; UI-05): each with what
    /// it selects, the operations that use it and why, and a problem if one
    /// can be seen without reading the model. Checked here: the model is in
    /// the project, the set was made on the version of its file the project
    /// holds, it selects something, and the
    /// file on disk is still the one added. A triangle beyond the end of the
    /// model is found when the operation is generated (decision record 0032
    /// item 7). Nothing is dropped silently (OI-139).
    ///
    /// `file_warnings` is [`model_file_warnings`](Self::model_file_warnings),
    /// which reads and hashes every model file: a front end calls that when
    /// a project opens or the user asks, and passes the answer here for each
    /// redraw.
    pub fn selection_summaries(&self, file_warnings: &[String]) -> Vec<SelectionSummary> {
        let project = &self.document.project;
        let changed = file_warnings;
        project
            .selection_sets()
            .iter()
            .map(|set| {
                let model = project.models().iter().find(|m| m.id == set.model);
                let problem = match (&set.target, model) {
                    (_, None) => Some("its model is no longer in the project".to_owned()),
                    (target, Some(_)) if target.is_empty() => {
                        Some(format!("it selects no {}", target.kind_name()))
                    }
                    (_, Some(m)) if m.hash != set.model_hash => Some(format!(
                        "it was made on a different version of '{}'; select again on the current model",
                        m.label()
                    )),
                    (_, Some(m)) => changed
                        .iter()
                        .find(|w| w.starts_with(&format!("'{}' ", m.label())))
                        .map(|w| format!("{w}, so the selection may not match it")),
                };
                let count = set.target.len();
                let kind = set.target.kind_name();
                let uses = project
                    .operations()
                    .iter()
                    .filter_map(|o| {
                        let roles: Vec<SelectionRole> = [
                            (SelectionRole::Machine, o.selections.contains(&set.id)),
                            (SelectionRole::Boundary, o.boundary == Some(set.id)),
                            (SelectionRole::Avoid, o.avoid.contains(&set.id)),
                            (SelectionRole::Check, o.check.contains(&set.id)),
                        ]
                        .into_iter()
                        .filter_map(|(r, used)| used.then_some(r))
                        .collect();
                        (!roles.is_empty()).then(|| SelectionUse {
                            operation: o.id,
                            name: o.name.clone(),
                            roles,
                        })
                    })
                    .collect();
                SelectionSummary {
                    id: set.id,
                    name: set.name.clone(),
                    model: set.model,
                    model_path: model.map(|m| m.label()),
                    kind,
                    count,
                    uses,
                    problem,
                    colour: set.colour.clone(),
                }
            })
            .collect()
    }

    fn model_ref(&self, model: ModelId) -> Result<&ModelRef, AppError> {
        self.document
            .project
            .models()
            .iter()
            .find(|m| m.id == model)
            .ok_or_else(|| AppError::UnknownModel(model.to_string()))
    }
}

fn check_triangles(
    name: &str,
    mesh: &TriangleMesh,
    triangles: &TriangleSet,
) -> Result<(), AppError> {
    if triangles.is_empty() {
        return Err(selection_error(name, "it selects no triangles"));
    }
    let count = mesh.triangles.len();
    match triangles.iter().next_back() {
        Some(&last) if last as usize >= count => Err(selection_error(
            name,
            format!("triangle {last} does not exist; the model has {count} triangles"),
        )),
        _ => Ok(()),
    }
}

/// Whether `operation` uses a set of mesh edges in any role. Edge following
/// follows them instead of honouring a region, and
/// [`mesh_edge_segments`] refuses them anywhere else, naming the set
/// (OI-204; decision record 0066).
pub(crate) fn selects_mesh_edges(project: &Project, operation: &Operation) -> bool {
    operation.selection_refs().any(|id| {
        project
            .selection_sets()
            .iter()
            .any(|s| s.id == id && matches!(s.target, SelectionTarget::MeshEdges(_)))
    })
}

/// The mesh edges `operation` selects, as segments of its placed part `mesh`
/// (`part`, read now with hash `hash`), in the sets' order; `None` when it
/// selects none (OI-204; decision record 0066). Only edge following uses
/// them, and only as the sets it machines; anything else is an error naming
/// the set, never a silent drop (OP-02).
pub(crate) fn mesh_edge_segments(
    project: &Project,
    operation: &Operation,
    part: &ModelRef,
    hash: ContentHash,
    mesh: &TriangleMesh,
) -> Result<Option<Vec<[openchuck_types::geometry::Point3; 2]>>, AppError> {
    let fail = |set: &SelectionSet, reason: String| AppError::Operation {
        name: operation.name.clone(),
        reason: format!("selection set '{}' cannot be used: {reason}", set.name),
    };
    let find = |id: SelectionSetId| {
        project
            .selection_sets()
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::UnknownSelection(id.to_string()))
    };
    for id in operation
        .boundary
        .iter()
        .chain(&operation.avoid)
        .chain(&operation.check)
    {
        let set = find(*id)?;
        if matches!(set.target, SelectionTarget::MeshEdges(_)) {
            return Err(fail(
                set,
                "it selects edges, which can only be followed, not used as a boundary, an \
                 avoid or a check surface"
                    .to_owned(),
            ));
        }
    }
    let mut segments = Vec::new();
    let mut triangles = None;
    let mut any = false;
    for id in &operation.selections {
        let set = find(*id)?;
        let SelectionTarget::MeshEdges(edges) = &set.target else {
            triangles = Some(set);
            continue;
        };
        any = true;
        if operation.strategy != openchuck_path::edge_following::NAME {
            return Err(fail(
                set,
                format!(
                    "it selects edges, and the strategy '{}' does not use selected edges; only \
                     edge following does",
                    operation.strategy
                ),
            ));
        }
        if set.model != part.id {
            return Err(fail(
                set,
                "it was made on a different model from the setup's part".to_owned(),
            ));
        }
        if set.model_hash != hash {
            return Err(fail(
                set,
                format!(
                    "it was made on a different version of '{}' (the file has changed); select \
                     again on the current model",
                    part.label()
                ),
            ));
        }
        if edges.is_empty() {
            return Err(fail(set, "it selects no edges".to_owned()));
        }
        if set.stock_to_leave.is_some() {
            return Err(fail(
                set,
                "a stock to leave per selection set is not used on followed edges (OP-11); \
                 clear it on the set and set the operation's depth"
                    .to_owned(),
            ));
        }
        for edge in edges {
            let ends = mesh
                .triangles
                .get(edge.triangle as usize)
                .map(|t| edge.corners().map(|c| t.get(c).copied()))
                .and_then(|[a, b]| {
                    Some([
                        mesh.vertices.get(a? as usize)?,
                        mesh.vertices.get(b? as usize)?,
                    ])
                });
            let Some([a, b]) = ends else {
                return Err(fail(
                    set,
                    format!(
                        "triangle {} does not exist; the model has {} triangles",
                        edge.triangle,
                        mesh.triangles.len()
                    ),
                ));
            };
            segments.push([*a, *b]);
        }
    }
    if !any {
        return Ok(None);
    }
    if let Some(set) = triangles {
        return Err(fail(
            set,
            "it selects triangles, and edge following follows the operation's selected edges; \
             take the triangle set out"
                .to_owned(),
        ));
    }
    // Chosen edges are followed as they are: a boundary, avoid or check set
    // would not be honoured, so it is refused rather than dropped (OP-02).
    if let Some(id) = operation
        .boundary
        .iter()
        .chain(&operation.avoid)
        .chain(&operation.check)
        .next()
    {
        return Err(fail(
            find(*id)?,
            "edge following on chosen edges cannot yet honour a boundary, avoid or check set; \
             take it out of the operation"
                .to_owned(),
        ));
    }
    Ok(Some(segments))
}

#[derive(Clone, Copy)]
enum Join {
    Add,
    Remove,
    Replace,
}

/// Joins `other` to the target of `set` as `join` says; both must be of one
/// kind, and the result must select something.
fn combine(set: &mut SelectionSet, other: SelectionTarget, join: Join) -> Result<(), AppError> {
    fn apply<T: Ord>(current: &mut BTreeSet<T>, other: BTreeSet<T>, join: Join) {
        match join {
            Join::Add => current.extend(other),
            Join::Remove => current.retain(|t| !other.contains(t)),
            Join::Replace => *current = other,
        }
    }
    let kind = set.target.kind_name();
    match (&mut set.target, other) {
        (SelectionTarget::Triangles(a), SelectionTarget::Triangles(b)) => apply(a, b, join),
        (SelectionTarget::MeshEdges(a), SelectionTarget::MeshEdges(b)) => apply(a, b, join),
        (SelectionTarget::Faces(a), SelectionTarget::Faces(b)) => apply(a, b, join),
        (SelectionTarget::Edges(a), SelectionTarget::Edges(b)) => apply(a, b, join),
        (_, other) => {
            return Err(selection_error(
                &set.name,
                format!(
                    "it selects {kind}, and the edit gives {}; a set holds one kind",
                    other.kind_name()
                ),
            ));
        }
    }
    if set.target.is_empty() {
        return Err(selection_error(
            &set.name,
            format!("the edit would leave the set with no {kind}; remove the set instead"),
        ));
    }
    Ok(())
}

/// Checks that `target` can be stored on the model read as `mesh`, with its
/// placed B-rep `brep` for a STEP model: of a kind the model is selected by,
/// not empty, and naming only what the model has.
pub(crate) fn check_target(
    name: &str,
    mesh: &TriangleMesh,
    brep: Option<&openchuck_types::brep::BrepModel>,
    target: &SelectionTarget,
) -> Result<(), AppError> {
    let kind = target.kind_name();
    if target.is_empty() {
        return Err(selection_error(name, format!("it selects no {kind}")));
    }
    match (target, brep) {
        (SelectionTarget::Triangles(t), None) => check_triangles(name, mesh, t),
        (SelectionTarget::MeshEdges(edges), None) => {
            let count = mesh.triangles.len();
            match edges.iter().map(|e| e.triangle).max() {
                Some(last) if last as usize >= count => Err(selection_error(
                    name,
                    format!("triangle {last} does not exist; the model has {count} triangles"),
                )),
                _ => Ok(()),
            }
        }
        (SelectionTarget::Faces(faces), Some(brep)) => {
            match faces
                .iter()
                .find(|f| !brep.faces.iter().any(|g| g.id == **f))
            {
                Some(f) => Err(selection_error(
                    name,
                    format!("face {} is not in the model", f.0),
                )),
                None => Ok(()),
            }
        }
        (SelectionTarget::Edges(edges), Some(brep)) => {
            match edges
                .iter()
                .find(|e| !brep.edges.iter().any(|g| g.id == **e))
            {
                Some(e) => Err(selection_error(
                    name,
                    format!("edge {} is not in the model", e.0),
                )),
                None => Ok(()),
            }
        }
        (SelectionTarget::Triangles(_) | SelectionTarget::MeshEdges(_), Some(_)) => {
            Err(selection_error(
                name,
                "a STEP model is selected by face and edge, not by triangle, because its \
                 triangles change with the tessellation tolerance",
            ))
        }
        (SelectionTarget::Faces(_) | SelectionTarget::Edges(_), None) => Err(selection_error(
            name,
            "a mesh model has no faces or B-rep edges to select; select triangles or edges",
        )),
    }
}

/// An operation's selections resolved to surfaces, ready to give a strategy
/// as a [`RegionConstraint`] (OP-02).
pub(crate) struct ResolvedRegion {
    region: Option<Box<dyn SurfaceSource>>,
    boundary: Option<Window>,
    avoid: Option<Box<dyn SurfaceSource>>,
    check: Option<Box<dyn SurfaceSource>>,
    /// The avoid and check sets that only warn, each with its name (decision
    /// record 0229 item 1).
    watched: Vec<(String, Protection, Box<dyn SurfaceSource>)>,
    /// A warning for each set that limits nothing (set to Ignore).
    pub(crate) warnings: Vec<PathWarning>,
    /// The selected triangles that have a stock to leave of their own,
    /// grouped by amount; where sets overlap the largest amount applies
    /// (OP-11, decision record 0034 item 4).
    leaves: Vec<(Box<dyn SurfaceSource>, f64)>,
    /// The rest of the part, which takes the operation's stock to leave.
    /// `None` when no set has an amount of its own.
    rest: Option<Box<dyn SurfaceSource>>,
}

impl ResolvedRegion {
    /// The zones for a strategy, the rest taking `operation_leave`. Empty
    /// when no selected set has a stock to leave of its own.
    pub(crate) fn zones(&self, operation_leave: f64) -> Vec<LeaveZone<'_>> {
        if self.leaves.is_empty() {
            return Vec::new();
        }
        let mut zones: Vec<LeaveZone<'_>> = self
            .leaves
            .iter()
            .map(|(surface, leave)| LeaveZone {
                surface: surface.as_ref(),
                leave: *leave,
            })
            .collect();
        if let Some(rest) = &self.rest {
            zones.push(LeaveZone {
                surface: rest.as_ref(),
                leave: operation_leave,
            });
        }
        zones
    }

    /// The watched surfaces, for [`constraint`](Self::constraint).
    pub(crate) fn watch(&self) -> Vec<WatchedSurface<'_>> {
        self.watched
            .iter()
            .map(|(name, protection, surface)| WatchedSurface {
                name,
                protection: *protection,
                surface: surface.as_ref(),
            })
            .collect()
    }

    /// The region as the pathing library takes it. The boundary takes no
    /// options here: its limits are swarf's, and swarf on a STEP part reads
    /// its sets itself (decision record 0229 item 2).
    pub(crate) fn constraint<'a>(
        &'a self,
        watch: &'a [WatchedSurface<'a>],
    ) -> RegionConstraint<'a> {
        RegionConstraint {
            region: self.region.as_deref(),
            boundary: self.boundary,
            avoid: self.avoid.as_deref(),
            check: self.check.as_deref(),
            watch,
            ..RegionConstraint::default()
        }
    }
}

/// The faces of a STEP part and the face each triangle of its joined
/// tessellation belongs to (`BrepModel::joined_tessellation`), so a face set
/// resolves to the triangles of its faces (decision 0209 item 3). While the
/// tool is dropped on the tessellation (OI-81), those triangles are exactly
/// the surface the contacts lie on. With exact contacts (decision 0277,
/// OI-833) each set is its exact faces instead (`exact`), dropped on
/// exactly as the part is.
pub(crate) struct FaceTriangles<'a> {
    faces: BTreeSet<FaceId>,
    owner: Vec<FaceId>,
    exact: Option<&'a BrepModel>,
}

impl<'a> FaceTriangles<'a> {
    /// The faces and triangles of the placed B-rep `brep`; its sets as
    /// exact faces when `exact` is set.
    pub(crate) fn of(brep: &'a BrepModel, exact: bool) -> Self {
        Self {
            faces: brep.faces.iter().map(|f| f.id).collect(),
            owner: brep.joined_tessellation().1,
            exact: exact.then_some(brep),
        }
    }

    /// The faces the triangles belong to, in order.
    fn faces_of(&self, triangles: &TriangleSet) -> Vec<FaceId> {
        let set: BTreeSet<FaceId> = triangles
            .iter()
            .filter_map(|&t| self.owner.get(t as usize).copied())
            .collect();
        set.into_iter().collect()
    }

    /// The triangles of `faces`, in order.
    fn triangles(&self, faces: &BTreeSet<FaceId>) -> TriangleSet {
        self.owner
            .iter()
            .zip(0_u32..)
            .filter(|(f, _)| faces.contains(f))
            .map(|(_, t)| t)
            .collect()
    }
}

/// The triangles `target` selects on the part: a triangle set's own on a
/// mesh, or the triangles of a face set's faces on a STEP part (`faces`).
/// `None` for a target of another kind.
fn triangles_of(
    target: &SelectionTarget,
    faces: Option<&FaceTriangles<'_>>,
) -> Option<TriangleSet> {
    match (target, faces) {
        (SelectionTarget::Triangles(triangles), None) => Some(triangles.clone()),
        (SelectionTarget::Faces(chosen), Some(map)) => Some(map.triangles(chosen)),
        _ => None,
    }
}

/// Resolves the selection sets of `operation` against its part: `part` is the
/// model, `hash` the hash of its file as read now and `mesh` its placed mesh,
/// or a STEP part's joined tessellation with `faces` mapping its triangles to
/// their faces. `None` when the operation uses no sets.
pub(crate) fn resolve(
    project: &Project,
    operation: &Operation,
    part: &ModelRef,
    hash: ContentHash,
    mesh: &TriangleMesh,
    faces: Option<&FaceTriangles<'_>>,
) -> Result<Option<ResolvedRegion>, AppError> {
    if operation.selection_refs().next().is_none() {
        return Ok(None);
    }
    let fail = |set: &SelectionSet, reason: String| AppError::Operation {
        name: operation.name.clone(),
        reason: format!("selection set '{}' cannot be used: {reason}", set.name),
    };
    // The union of the sets of one role.
    let union =
        |ids: &mut dyn Iterator<Item = SelectionSetId>| -> Result<Option<TriangleSet>, AppError> {
            let mut all: Option<TriangleSet> = None;
            for id in ids {
                let set = project
                    .selection_sets()
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or_else(|| AppError::UnknownSelection(id.to_string()))?;
                let Some(triangles) = triangles_of(&set.target, faces) else {
                    return Err(fail(
                        set,
                        match faces {
                            None => format!(
                                "it selects {}, which only edge following uses, as the edges it \
                                 follows",
                                set.target.kind_name()
                            ),
                            Some(_) => format!(
                                "it selects {}, and a STEP part is limited by a set of its faces",
                                set.target.kind_name()
                            ),
                        },
                    ));
                };
                if set.model != part.id {
                    return Err(fail(
                        set,
                        "it was made on a different model from the setup's part".to_owned(),
                    ));
                }
                if set.model_hash != hash {
                    return Err(fail(
                        set,
                        format!(
                            "it was made on a different version of '{}' (the file has changed); \
                         select again on the current model",
                            part.label()
                        ),
                    ));
                }
                if let (SelectionTarget::Faces(chosen), Some(map)) = (&set.target, faces)
                    && let Some(face) = chosen.iter().find(|f| !map.faces.contains(f))
                {
                    return Err(fail(set, format!("face {} is not in the model", face.0)));
                }
                if triangles.is_empty() {
                    return Err(fail(
                        set,
                        format!("it selects no {}", set.target.kind_name()),
                    ));
                }
                if let Some(&bad) = triangles.iter().next_back()
                    && bad as usize >= mesh.triangles.len()
                {
                    return Err(fail(
                        set,
                        format!(
                            "triangle {bad} does not exist; the model has {} triangles",
                            mesh.triangles.len()
                        ),
                    ));
                }
                all.get_or_insert_with(BTreeSet::new).extend(triangles);
            }
            Ok(all)
        };
    // The surface of a set: its exact faces with exact contacts, otherwise
    // its triangles.
    let of_triangles = |t: &TriangleSet| -> Result<Box<dyn SurfaceSource>, AppError> {
        match faces.and_then(|f| f.exact.map(|b| (f, b))) {
            Some((map, brep)) => Ok(Box::new(
                BrepSurface::with_faces(brep, &map.faces_of(t))?.with_exact_drops(true),
            )),
            None => Ok(Box::new(MeshSurface::new(&select::submesh(mesh, t)?)?)),
        }
    };
    let surface =
        |triangles: Option<TriangleSet>| -> Result<Option<Box<dyn SurfaceSource>>, AppError> {
            triangles.map(|t| of_triangles(&t)).transpose()
        };
    let region = surface(union(&mut operation.selections.iter().copied())?)?;
    // Each avoid and check set by its choice (decision record 0229 item 1):
    // the sets that cut short are one surface per role, as before; each set
    // that only warns is a watched surface of its own; a set on Ignore is
    // still checked, then left out with a warning.
    let options = &operation.restriction_options;
    let mut watched = Vec::new();
    let mut warnings = Vec::new();
    let mut short = [Vec::new(), Vec::new()];
    for (ids, protection, role, slot) in [
        (&operation.avoid, Protection::Avoid, "avoid", 0),
        (&operation.check, Protection::Check, "check", 1),
    ] {
        for &id in ids {
            let on_break = match protection {
                Protection::Avoid => options.avoid(id).on_break,
                Protection::Check => options.check(id).on_break,
            };
            let triangles = union(&mut std::iter::once(id))?;
            let name = || {
                project
                    .selection_sets()
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default()
            };
            match on_break {
                OnBreak::CutShort => {
                    if let Some(cut_short) = short.get_mut(slot) {
                        cut_short.push(id);
                    }
                }
                OnBreak::Warn => {
                    if let Some(s) = surface(triangles)? {
                        watched.push((name(), protection, s));
                    }
                }
                OnBreak::Ignore => warnings.push(crate::operation::limits_nothing(
                    &format!("{role} set '{}'", name()),
                    "it is set to Ignore: the tool is not kept clear of it",
                    "Set it to Cut short or Warn only",
                )),
            }
        }
    }
    let [short_avoid, short_check] = short;
    let avoid = surface(union(&mut short_avoid.into_iter())?)?;
    let check = surface(union(&mut short_check.into_iter())?)?;
    let boundary = union(&mut operation.boundary.into_iter())?
        .map(|t| Ok::<_, AppError>(Window::of_mesh(&select::submesh(mesh, &t)?)?))
        .transpose()?;
    // Each triangle takes the largest stock to leave of the selected sets
    // that hold it.
    let mut amount: BTreeMap<u32, f64> = BTreeMap::new();
    for id in &operation.selections {
        let Some(set) = project.selection_sets().iter().find(|s| s.id == *id) else {
            continue;
        };
        let (Some(leave), Some(triangles)) = (set.stock_to_leave, triangles_of(&set.target, faces))
        else {
            continue;
        };
        for t in triangles {
            let entry = amount.entry(t).or_insert(leave);
            *entry = entry.max(leave);
        }
    }
    let mut groups: BTreeMap<u64, (f64, TriangleSet)> = BTreeMap::new();
    for (t, leave) in &amount {
        groups
            .entry(leave.to_bits())
            .or_insert_with(|| (*leave, BTreeSet::new()))
            .1
            .insert(*t);
    }
    let leaves = groups
        .into_values()
        .map(|(leave, triangles)| Ok((of_triangles(&triangles)?, leave)))
        .collect::<Result<Vec<_>, AppError>>()?;
    let rest = if leaves.is_empty() {
        None
    } else {
        let others: TriangleSet = (0..mesh.triangles.len() as u32)
            .filter(|t| !amount.contains_key(t))
            .collect();
        if others.is_empty() {
            None
        } else {
            Some(of_triangles(&others)?)
        }
    };
    Ok(Some(ResolvedRegion {
        region,
        boundary,
        avoid,
        check,
        watched,
        warnings,
        leaves,
        rest,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use openchuck_project::MeshEdge;
    use openchuck_testkit::brep::{HoleSpec, drilled_blank};
    use openchuck_types::brep::{EdgeId, FaceId};

    use super::*;

    /// Face and edge sets are checked against the placed B-rep they are made
    /// on, as `save_selection` reads it from `PlacedPart` (slice 11 item 7;
    /// decision record 0066): the kind, emptiness and every ID.
    #[test]
    fn req_prj_06_req_ui_05_face_and_edge_sets_are_checked_against_the_b_rep() {
        let (brep, _) = drilled_blank(
            10.0,
            40.0,
            &[HoleSpec::blind(20.0, [0.0, 0.0, 1.0], 4.0, 3.0)],
        );
        let mesh = brep.joined_tessellation().0;
        let check = |target: SelectionTarget, brep| check_target("Set", &mesh, brep, &target);
        let faces = |ids: &[u32]| SelectionTarget::Faces(ids.iter().map(|i| FaceId(*i)).collect());
        let edges = |ids: &[u32]| SelectionTarget::Edges(ids.iter().map(|i| EdgeId(*i)).collect());
        check(faces(&[0, 2]), Some(&brep)).unwrap();
        check(edges(&[0, 1]), Some(&brep)).unwrap();
        let err = |r: Result<(), AppError>| r.unwrap_err().to_string();
        assert!(err(check(faces(&[9]), Some(&brep))).contains("face 9 is not in the model"));
        assert!(err(check(edges(&[7]), Some(&brep))).contains("edge 7 is not in the model"));
        assert!(err(check(faces(&[]), Some(&brep))).contains("no faces"));
        // Triangles and mesh edges are refused on a STEP model, faces and
        // B-rep edges on a mesh.
        let triangles = SelectionTarget::Triangles([0].into());
        assert!(err(check(triangles.clone(), Some(&brep))).contains("face and edge"));
        let mesh_edges = SelectionTarget::MeshEdges([MeshEdge::new(0, 1).unwrap()].into());
        assert!(err(check(mesh_edges.clone(), Some(&brep))).contains("face and edge"));
        assert!(err(check(faces(&[0]), None)).contains("no faces"));
        check(triangles, None).unwrap();
        check(mesh_edges, None).unwrap();
        let beyond = SelectionTarget::MeshEdges([MeshEdge::new(u32::MAX, 0).unwrap()].into());
        assert!(err(check(beyond, None)).contains("does not exist"));
    }
}
