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
use openchuck_path::{LeaveZone, MeshSurface, RegionConstraint, Window};
use openchuck_project::id::{ModelId, OperationId, SelectionSetId};
use openchuck_project::{
    ContentHash, Item, ItemId, ModelRef, Operation, Placement, Project, SelectionSet,
    SelectionTarget,
};
use openchuck_types::geometry::Vec3;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
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

/// A change to a selection set's triangles or name (UI-05).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SelectionEdit {
    /// Adds triangles.
    Add(TriangleSet),
    /// Takes triangles out.
    Remove(TriangleSet),
    /// Replaces the selection.
    Replace(TriangleSet),
    /// Renames the set.
    Rename(String),
    /// Sets the stock to leave on the set, in millimetres, or clears it so the
    /// operation's value applies (OP-11).
    StockToLeave(Option<f64>),
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

    /// The role's name for a panel.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Machine => "Machine",
            Self::Boundary => "Boundary",
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

    /// Saves `triangles` of the mesh model `model` as a new named set at the
    /// end of the list (UI-05, PRJ-06). Fails if `triangles` is empty, names a
    /// triangle the model does not have, or the model file has changed since
    /// it was added to the project. Undoable.
    pub fn save_selection(
        &mut self,
        name: &str,
        model: ModelId,
        triangles: TriangleSet,
    ) -> Result<SelectionSetId, AppError> {
        let reference = self.model_ref(model)?.clone();
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &NoProgress);
        let (mesh, hash) =
            crate::model::placed_mesh_hashed(&self.document.location, &reference, &context)?;
        check_triangles(name, &mesh, &triangles)?;
        if hash != reference.hash {
            return Err(selection_error(
                name,
                format!(
                    "'{}' has changed since it was added to the project; accept the change \
                     before selecting on it (PRJ-02)",
                    reference.path.as_str()
                ),
            ));
        }
        let id = SelectionSetId::new();
        let set = SelectionSet {
            id,
            order: Default::default(),
            name: name.to_owned(),
            model,
            model_hash: hash,
            target: SelectionTarget::Triangles(triangles),
            descriptors: Vec::new(),
            stock_to_leave: None,
        };
        self.execute(Command::Add {
            item: Item::SelectionSet(set),
            placement: Placement::End,
        })?;
        Ok(id)
    }

    /// Changes selection set `set` (UI-05) and returns the operations that
    /// use it, which now need regenerating. The set's model hash is kept as it
    /// was: a set made on one version of the model is not made current by
    /// editing it. Undoable.
    pub fn edit_selection(
        &mut self,
        set: SelectionSetId,
        edit: SelectionEdit,
    ) -> Result<Vec<openchuck_project::id::OperationId>, AppError> {
        let mut edited = self
            .document
            .project
            .selection_sets()
            .iter()
            .find(|s| s.id == set)
            .cloned()
            .ok_or_else(|| AppError::UnknownSelection(set.to_string()))?;
        if let SelectionEdit::Rename(name) = edit {
            edited.name = name;
        } else if let SelectionEdit::StockToLeave(leave) = edit {
            if leave.is_some_and(|v| !v.is_finite() || v < 0.0) {
                return Err(selection_error(
                    &edited.name,
                    "the stock to leave must be zero or more millimetres",
                ));
            }
            edited.stock_to_leave = leave;
        } else {
            let SelectionTarget::Triangles(current) = &mut edited.target else {
                return Err(selection_error(
                    &edited.name,
                    "only triangle selections on meshes can be edited so far",
                ));
            };
            match edit {
                SelectionEdit::Add(more) => current.extend(more),
                SelectionEdit::Remove(less) => current.retain(|t| !less.contains(t)),
                SelectionEdit::Replace(all) => *current = all,
                SelectionEdit::Rename(_) | SelectionEdit::StockToLeave(_) => {}
            }
            if current.is_empty() {
                return Err(selection_error(
                    &edited.name,
                    "the edit would leave the set with no triangles; remove the set instead",
                ));
            }
            let reference = self.model_ref(edited.model)?;
            let cancel = CancelToken::new();
            let context = JobContext::new(&cancel, &NoProgress);
            let (mesh, _) =
                crate::model::placed_mesh_hashed(&self.document.location, reference, &context)?;
            check_triangles(&edited.name, &mesh, current)?;
        }
        self.execute(Command::Replace(Item::SelectionSet(edited)))?;
        Ok(self.document.project.operations_using(set))
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
        if !self
            .document
            .project
            .selection_sets()
            .iter()
            .any(|s| s.id == set)
        {
            return Err(AppError::UnknownSelection(set.to_string()));
        }
        let mut edited = self
            .document
            .project
            .operation(operation)
            .cloned()
            .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
        let list = match role {
            SelectionRole::Machine => &mut edited.selections,
            SelectionRole::Avoid => &mut edited.avoid,
            SelectionRole::Check => &mut edited.check,
            SelectionRole::Boundary => {
                match (included, edited.boundary == Some(set)) {
                    (true, _) => edited.boundary = Some(set),
                    (false, true) => edited.boundary = None,
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
        self.replace_operation(edited)
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
    /// holds, it selects triangles of a mesh, it selects something, and the
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
                    (SelectionTarget::Faces(_) | SelectionTarget::Edges(_), Some(_)) => Some(
                        "it selects faces or edges, which need a STEP model (slice 11)".to_owned(),
                    ),
                    (SelectionTarget::Triangles(t), Some(_)) if t.is_empty() => {
                        Some("it selects no triangles".to_owned())
                    }
                    (_, Some(m)) if m.hash != set.model_hash => Some(format!(
                        "it was made on a different version of '{}'; select again on the current model",
                        m.path.as_str()
                    )),
                    (_, Some(m)) => changed
                        .iter()
                        .find(|w| w.starts_with(&format!("'{}' ", m.path.as_str())))
                        .map(|w| format!("{w}, so the selection may not match it")),
                };
                let count = match &set.target {
                    SelectionTarget::Triangles(t) => t.len(),
                    SelectionTarget::Faces(f) => f.len(),
                    SelectionTarget::Edges(e) => e.len(),
                };
                let kind = match &set.target {
                    SelectionTarget::Triangles(_) => "triangles",
                    SelectionTarget::Faces(_) => "faces",
                    SelectionTarget::Edges(_) => "edges",
                };
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
                    model_path: model.map(|m| m.path.as_str().to_owned()),
                    kind,
                    count,
                    uses,
                    problem,
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

/// An operation's selections resolved to surfaces, ready to give a strategy
/// as a [`RegionConstraint`] (OP-02).
pub(crate) struct ResolvedRegion {
    region: Option<MeshSurface>,
    boundary: Option<Window>,
    avoid: Option<MeshSurface>,
    check: Option<MeshSurface>,
    /// The selected triangles that have a stock to leave of their own,
    /// grouped by amount; where sets overlap the largest amount applies
    /// (OP-11, decision record 0034 item 4).
    leaves: Vec<(MeshSurface, f64)>,
    /// The rest of the part, which takes the operation's stock to leave.
    /// `None` when no set has an amount of its own.
    rest: Option<MeshSurface>,
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
                surface,
                leave: *leave,
            })
            .collect();
        if let Some(rest) = &self.rest {
            zones.push(LeaveZone {
                surface: rest,
                leave: operation_leave,
            });
        }
        zones
    }

    pub(crate) fn constraint(&self) -> RegionConstraint<'_> {
        RegionConstraint {
            region: self.region.as_ref().map(|s| s as _),
            boundary: self.boundary,
            avoid: self.avoid.as_ref().map(|s| s as _),
            check: self.check.as_ref().map(|s| s as _),
        }
    }
}

/// Resolves the selection sets of `operation` against its part: `part` is the
/// model, `hash` the hash of its file as read now and `mesh` its placed mesh.
/// `None` when the operation uses no sets.
pub(crate) fn resolve(
    project: &Project,
    operation: &Operation,
    part: &ModelRef,
    hash: ContentHash,
    mesh: &TriangleMesh,
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
                let SelectionTarget::Triangles(triangles) = &set.target else {
                    return Err(fail(
                        set,
                        "it selects faces or edges, which need a STEP model (slice 11)".to_owned(),
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
                            part.path.as_str()
                        ),
                    ));
                }
                if triangles.is_empty() {
                    return Err(fail(set, "it selects no triangles".to_owned()));
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
                all.get_or_insert_with(BTreeSet::new)
                    .extend(triangles.iter().copied());
            }
            Ok(all)
        };
    let surface = |triangles: Option<TriangleSet>| -> Result<Option<MeshSurface>, AppError> {
        triangles
            .map(|t| Ok(MeshSurface::new(&select::submesh(mesh, &t)?)?))
            .transpose()
    };
    let region = surface(union(&mut operation.selections.iter().copied())?)?;
    let avoid = surface(union(&mut operation.avoid.iter().copied())?)?;
    let check = surface(union(&mut operation.check.iter().copied())?)?;
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
        let (Some(leave), SelectionTarget::Triangles(triangles)) =
            (set.stock_to_leave, &set.target)
        else {
            continue;
        };
        for &t in triangles {
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
        .map(|(leave, triangles)| {
            Ok((
                MeshSurface::new(&select::submesh(mesh, &triangles)?)?,
                leave,
            ))
        })
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
            Some(MeshSurface::new(&select::submesh(mesh, &others)?)?)
        }
    };
    Ok(Some(ResolvedRegion {
        region,
        boundary,
        avoid,
        check,
        leaves,
        rest,
    }))
}
