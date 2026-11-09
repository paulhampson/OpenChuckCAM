// SPDX-License-Identifier: AGPL-3.0-or-later
//! The texts the shell's status line held, which was never drawn (OI-2098),
//! placed by the author's test of decision record 0536 section 2 (section 8,
//! the owner's answer of 8 Nov 2026):
//!
//! - work that is lost unless the project is saved is a **warning** in
//!   Messages, with "Save" (and "Undo" where undoing also clears it), until
//!   the cause goes;
//! - a pick dropped because its geometry changed is an **error** on what it
//!   was for, with "Reselect" or "Edit workholding", until a pick is made;
//! - what the user just did ("Saved …") is a finished entry in the Jobs
//!   tab, not a message (the shell writes it to the job log).

use openchuck_app::Application;
use openchuck_app::lead_in::Context;
use openchuck_app::overview::{ItemRef, Message, MessageAction, SetupPart, Severity};
use openchuck_app::project::Project;
use openchuck_app::project::id::{SelectionSetId, SetupId};

/// What a pick that was dropped was for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DroppedPick {
    /// The working selection, not yet a set.
    Selection,
    /// The selection set being changed.
    Set(SelectionSetId),
    /// The workholding of a setup (a held region or a tab).
    Workholding(SetupId),
}

/// The warning while the project holds changes recovered from a checkpoint
/// and not yet saved: they are lost at close unless saved.
pub(super) fn recovered(app: &Application) -> Option<Message> {
    app.is_recovered_unsaved().then(|| {
        Message::about(
            Severity::Warning,
            "Recovered checkpoint",
            &Context::default(),
            "the project holds changes recovered from a checkpoint, and they are lost when it \
             closes unless it is saved. Save the project to keep them",
            None,
            Some(MessageAction::Save),
        )
    })
}

/// The warning while tool numbers the export changed are not saved: the
/// program just written uses the new numbers, but the project would reopen
/// with the old ones. `changed` is whether the export changed them in this
/// session.
pub(super) fn tool_numbers(app: &Application, changed: bool) -> Option<Message> {
    (changed && app.tool_numbers_unsaved()).then(|| {
        Message::about(
            Severity::Warning,
            "Tool numbers",
            &Context::default(),
            "the tool numbers were changed for the program, but the project keeps them only once \
             it is saved: it would reopen with the old numbers, and the next program would not \
             match the tool table. Save the project to keep them, or undo the change",
            None,
            Some(MessageAction::Save),
        )
        .with_views(vec![MessageAction::Undo])
    })
}

/// The error for a pick that was dropped because the geometry it was made
/// on changed before it could be added.
pub(super) fn dropped_pick(project: &Project, dropped: DroppedPick) -> Message {
    let text = "the geometry changed before the pick was added, so nothing was picked. \
                Select it again";
    let (source, action) = match dropped {
        DroppedPick::Selection => (None, None),
        DroppedPick::Set(id) => (
            project
                .selection_sets()
                .iter()
                .find(|s| s.id == id)
                .map(|s| (ItemRef::SelectionSet(id), s.name.clone())),
            Some(MessageAction::Reselect(id)),
        ),
        DroppedPick::Workholding(id) => (
            project.setup(id).map(|s| {
                (
                    ItemRef::SetupPart(id, SetupPart::Workholding),
                    s.name.clone(),
                )
            }),
            Some(MessageAction::EditWorkholding(id)),
        ),
    };
    let context = match dropped {
        DroppedPick::Workholding(id) => project
            .setup(id)
            .map_or_else(Context::default, |s| Context::setup(s.name.clone())),
        _ => Context::default(),
    };
    Message::about(Severity::Error, "Pick", &context, text, source, action)
}

/// `text` with its first letter in lower case, for the text after a
/// lead-in, unless it starts a name in capitals ("STEP").
pub(super) fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_uppercase() && !second.is_uppercase() => first
            .to_lowercase()
            .chain(text[first.len_utf8()..].chars())
            .collect(),
        _ => text.to_owned(),
    }
}
