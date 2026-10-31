// SPDX-License-Identifier: AGPL-3.0-or-later
//! Why an operation's back-and-forth ramp entry cannot be used, when that is
//! known before it is generated (#291, lane RAMPERRORS; decisions 0242 and
//! 0248; NFR-07, decision 0114). One check, read by the operation
//! inspector's Entry, links and retract section (under the Back-and-forth
//! ramp field), by the operations overview (the operation in error, and a
//! line in Messages) and by generation, as the STEP set refusals are
//! (decision 0209 item 10).

use openchuck_path::params::names;
use openchuck_project::{Operation, ParameterValue, Project};

/// What the operation's Entry settings offer instead.
const ENTRY_FIX: &str = "in the operation's Entry, links and retract settings, turn off \
     Back-and-forth ramp or set Entry to Plunge, Lead or Helix";

/// True when `operation` asks for the back-and-forth ramp: Entry is Ramp and
/// Back-and-forth ramp is on (a retained flag under another entry does
/// nothing and is labelled inactive in the form).
pub(crate) fn wanted(operation: &Operation) -> bool {
    matches!(
        operation.parameters.get(names::RAMP_BACK_AND_FORTH),
        Some(ParameterValue::Flag(true))
    ) && matches!(
        operation.parameters.get(names::ENTRY),
        Some(ParameterValue::Text(style)) if style == "ramp"
    )
}

/// Why `operation`'s back-and-forth ramp cannot be used, in plain words with
/// where to change it; `None` when it is not wanted or nothing is known yet
/// against it. Generation checks the rest (the moves themselves) and says
/// so with its own message.
pub(crate) fn refusal(project: &Project, operation: &Operation) -> Option<String> {
    if !wanted(operation) {
        return None;
    }
    // The clearance check works on the part's triangles, which for a STEP
    // part are made at a nominal deflection with no proved largest error
    // (decision 0242 item 5): it is refused, not weakened.
    if crate::operation::on_step_part(project, operation) {
        return Some(format!(
            "The back-and-forth ramp is not available on a STEP part yet: its clearance cannot \
             be checked against the part's exact faces. To generate this operation, {ENTRY_FIX}."
        ));
    }
    let snapshot = operation
        .tool
        .and_then(|id| project.tools().iter().find(|t| t.id == id))?;
    let cutting = 2.0 * snapshot.tool.profile().ok()?.cutting_radius();
    let shank = snapshot.tool.shank_diameter.0;
    (shank > cutting).then(|| {
        format!(
            "The back-and-forth ramp cannot check its clearance with the tool '{}': its shank \
             ({shank:.3} mm) is wider than its cutting diameter ({cutting:.3} mm). Choose a tool \
             whose shank is no wider than its cutter, or {ENTRY_FIX}.",
            snapshot.name
        )
    })
}
