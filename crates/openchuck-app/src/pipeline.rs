// SPDX-License-Identifier: AGPL-3.0-or-later
//! The operation pipeline (architecture section 6). Slice 0 runs it over an
//! empty project; the stages fill in from slice 1 onwards.

use std::path::Path;

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
}

/// Regenerates every operation of the project at `_location`.
pub(crate) fn regenerate(
    _location: &Path,
    context: &JobContext<'_>,
) -> Result<Regeneration, AppError> {
    context.check_cancelled()?;
    context.report("regenerate", 0, Some(0));
    let regeneration = Regeneration::default();
    regeneration.toolpath.validate()?;
    context.report("regenerate", 0, Some(0));
    Ok(regeneration)
}
