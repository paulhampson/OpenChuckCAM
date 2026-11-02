// SPDX-License-Identifier: AGPL-3.0-or-later
//! The selection sets an operation on a STEP part cannot use, judged from the
//! project alone, before anything is generated (OP-02, OP-11, NFR-07;
//! decision 0209 item 10).
//!
//! One check, read in three places, so they never disagree:
//! - the operation inspector's Geometry section shows each refusal under the
//!   role that holds the set ([`Application::set_refusals`]);
//! - the operations overview puts the operation in error with the first
//!   refusal, and Messages gets each one with a Show link;
//! - generation refuses with the first, as the fallback (NFR-07).
//!
//! On a STEP part, roughing, helical, axial, indexed and simultaneous
//! finishing take face sets in every role and a stock to leave per set, as
//! they take triangle sets on a mesh (decision 0209 item 3); swarf takes face
//! sets in every role, radial drilling a face set to choose its holes, and
//! edge following an edge set to follow. Everything else is refused here,
//! worded with the strategy's label and what to do (decision 0209 item 10.3).

use openchuck_path::{Registry, roughing};
use openchuck_project::id::{OperationId, SelectionSetId};
use openchuck_project::{Operation, Project, SelectionTarget};

use crate::Application;
use crate::operation::on_step_part;
use crate::selection::SelectionRole;

/// A selection set an operation cannot use in the role that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetRefusal {
    /// The role that holds the set.
    pub role: SelectionRole,
    /// The set.
    pub set: SelectionSetId,
    /// Why, and what to do, worded for the user (a whole sentence or two).
    pub text: String,
}

impl Application {
    /// The selection sets `operation` cannot use on its setup's STEP part,
    /// each under the role that holds it, in the Geometry section's order
    /// (decision 0209 item 10). Empty for an operation on a mesh part, one
    /// that is not in the project, or one whose sets it can all use.
    pub fn set_refusals(&self, operation: OperationId) -> Vec<SetRefusal> {
        self.project()
            .operation(operation)
            .map(|o| refusals(self.project(), &self.strategies, o))
            .unwrap_or_default()
    }
}

/// The sets `operation` cannot use on its setup's STEP part (decision 0209
/// item 10): see the module documentation. Judged from the project only, so
/// the answer is the same before and at generation.
pub(crate) fn refusals(
    project: &Project,
    registry: &Registry,
    operation: &Operation,
) -> Vec<SetRefusal> {
    if !on_step_part(project, operation) {
        return Vec::new();
    }
    // An unknown strategy is reported where it is looked up.
    let Ok(strategy) = registry.get(&operation.strategy) else {
        return Vec::new();
    };
    let label = strategy.label();
    let name = operation.strategy.as_str();
    let regions = strategy.honours_region();
    let takes_faces =
        regions || name == openchuck_path::drill::NAME || name == openchuck_path::swarf::NAME;
    let follows_edges = name == openchuck_path::edge_following::NAME;
    // Roughing leaves the largest of the sets' amounts wherever it cuts
    // (decision 0058); the others that take one cut each set to its own.
    let takes_set_stock = strategy.honours_leave_zones() || name == roughing::NAME;
    let mut out = Vec::new();
    for role in SelectionRole::ALL {
        let ids: Vec<SelectionSetId> = match role {
            SelectionRole::Machine => operation.selections.clone(),
            SelectionRole::Boundary => operation.boundary.into_iter().collect(),
            SelectionRole::Avoid => operation.avoid.clone(),
            SelectionRole::Check => operation.check.clone(),
        };
        for id in ids {
            // A missing set is reported where it is read.
            let Some(set) = project.selection_sets().iter().find(|s| s.id == id) else {
                continue;
            };
            let named = &set.name;
            let text = match (&set.target, role) {
                (SelectionTarget::Triangles(_) | SelectionTarget::MeshEdges(_), _) => {
                    Some(format!(
                        "The set '{named}' selects {}, but this setup's part is now a STEP file, \
                     which is selected by its faces and edges. Select the faces again on the \
                     STEP part.",
                        if matches!(set.target, SelectionTarget::Triangles(_)) {
                            "triangles"
                        } else {
                            "edges of triangles"
                        }
                    ))
                }
                (_, SelectionRole::Boundary | SelectionRole::Avoid | SelectionRole::Check)
                    if !regions =>
                {
                    Some(format!(
                        "{label} does not use a boundary, avoid or check set. Take the set \
                         '{named}' out of the operation."
                    ))
                }
                (
                    SelectionTarget::Edges(_),
                    SelectionRole::Boundary | SelectionRole::Avoid | SelectionRole::Check,
                ) => Some(format!(
                    "{} takes faces, not edges, on a STEP part. Take the edge set '{named}' out \
                     of {}, and select faces instead.",
                    role.name(),
                    role.name()
                )),
                (SelectionTarget::Edges(_), SelectionRole::Machine) if !follows_edges => {
                    Some(format!(
                        "Only edge following follows chosen edges. Take the edge set '{named}' \
                         out of the operation, or use edge following."
                    ))
                }
                (SelectionTarget::Faces(_), SelectionRole::Machine) if !takes_faces => {
                    Some(format!(
                        "{label} works on the whole part and cannot be limited to chosen faces. \
                         Take the set '{named}' out of the operation."
                    ))
                }
                (_, SelectionRole::Machine) if set.stock_to_leave.is_some() && !takes_set_stock => {
                    Some(format!(
                        "{label} does not leave a stock of its own on a selection set. Clear \
                         the stock to leave on the set '{named}', and set the operation's stock \
                         to leave instead."
                    ))
                }
                _ => None,
            };
            if let Some(text) = text {
                out.push(SetRefusal {
                    role,
                    set: id,
                    text,
                });
            }
        }
    }
    out
}
