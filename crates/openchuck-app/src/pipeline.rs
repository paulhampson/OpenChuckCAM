// SPDX-License-Identifier: AGPL-3.0-or-later
//! The operation pipeline (architecture section 6). Slice 5 generates each
//! operation's toolpath in machining order (OP-01); simulation and posting
//! fill in from slices 6 and 7.

use std::path::Path;

use openchuck_path::Registry;
use openchuck_project::Project;
use openchuck_types::ir::Toolpath;
use openchuck_types::job::JobContext;

use crate::AppError;

/// The result of regenerating a project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Regeneration {
    /// The toolpath of every operation, in machining order.
    pub toolpath: Toolpath,
    /// Generated G-code files as (file name, contents), in output order.
    pub gcode: Vec<(String, String)>,
    /// Notes about the toolpaths, each naming its operation, in order.
    pub notes: Vec<String>,
    /// What the G-code checks found for each file in `gcode` (GC-08, GC-11).
    pub reports: Vec<crate::posting::GcodeReport>,
}

/// Regenerates every operation of `project` (in the folder `location`) in
/// machining order. The first operation that cannot be generated stops the
/// run with its error: nothing is left out silently (verification policy 8).
pub(crate) fn regenerate(
    location: &Path,
    project: &Project,
    registry: &Registry,
    post: Option<&crate::posting::PostSelection>,
    context: &JobContext<'_>,
) -> Result<Regeneration, AppError> {
    context.check_cancelled()?;
    let total = project.operations().len() as u64;
    context.report("regenerate", 0, Some(total));
    let mut regeneration = Regeneration::default();
    for (i, operation) in project.operations().iter().enumerate() {
        let path = crate::operation::generate_operation(
            location,
            project,
            operation.id,
            registry,
            context,
        )?;
        for note in path.notes {
            regeneration
                .notes
                .push(format!("operation '{}': {note}", operation.name));
        }
        regeneration.toolpath.groups.push(path.group);
        context.report("regenerate", i as u64 + 1, Some(total));
    }
    regeneration.toolpath.validate()?;
    if let Some(selection) = post {
        context.check_cancelled()?;
        let (files, reports, notes) =
            crate::posting::post_toolpath(project, &regeneration.toolpath, selection)?;
        regeneration.gcode = files;
        regeneration.reports = reports;
        regeneration.notes.extend(notes);
    }
    Ok(regeneration)
}
