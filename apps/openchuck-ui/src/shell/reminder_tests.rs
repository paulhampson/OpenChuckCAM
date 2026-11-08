// SPDX-License-Identifier: AGPL-3.0-or-later
//! The texts of the shell's status line, which was never drawn (OI-2098),
//! each at the level the author's test gives it (decision record 0536
//! section 8, with the owner's answer of 8 Nov 2026).
#![allow(clippy::unwrap_used, clippy::panic)]

use openchuck_app::overview::{ItemRef, Message, MessageAction, Severity};
use openchuck_app::project::Project;
use openchuck_app::tools::{ToolNumbering, tool_numbering_command};
use openchuck_app::{Application, JobOutcome, Threads};

use super::frame_tests::{Rig, temp};
use super::reminders::DroppedPick;
use super::{Dialog, Next, Shell};

/// The messages whose text starts with `topic`.
fn about<'a>(rig: &'a Rig, topic: &str) -> Vec<&'a Message> {
    rig.shell
        .overview
        .messages
        .iter()
        .filter(|m| m.text.starts_with(topic))
        .collect()
}

/// How many errors and warnings the Messages label counts.
fn counted(rig: &Rig) -> usize {
    rig.shell.overview.count(Severity::Error) + rig.shell.overview.count(Severity::Warning)
}

/// The labels of the Jobs tab's entries for what the user did, newest first.
fn actions(rig: &Rig) -> Vec<String> {
    rig.shell
        .jobs
        .finished()
        .filter(|e| e.outcome == Some(JobOutcome::Action))
        .map(|e| e.label.clone())
        .collect()
}

#[test]
fn req_ui_09_req_ui_12_what_the_user_did_is_in_the_jobs_tab_and_not_in_messages() {
    let mut rig = Rig::cube("reminder-jobs", None);
    let opened = actions(&rig);
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].starts_with("Opened "), "{opened:?}");
    let before = counted(&rig);
    assert!(rig.shell.save());
    rig.settle();
    let done = actions(&rig);
    assert!(done[0].starts_with("Saved "), "{done:?}");
    // Never in Messages, at any level, and never counted.
    for word in ["Opened", "Saved"] {
        assert!(
            !rig.shell
                .overview
                .messages
                .iter()
                .any(|m| m.text.contains(word)),
            "{word}"
        );
    }
    assert_eq!(counted(&rig), before);
    // The Jobs tab draws the entry with how long ago it was.
    rig.shell.dock_tab = super::DockTab::Jobs;
    rig.frame(Vec::new());
    assert!(rig.shows(&done[0]));
    assert!(rig.shows("just now"));
}

#[test]
fn req_ui_12_req_prj_08_a_recovered_checkpoint_warns_with_save_until_saved() {
    let root = temp("reminder-recovered");
    let mut rig = Rig::new(Shell::with_settings(false, None, None), vec![root.clone()]);
    let app = Application::from_checkpoint(
        &root.join("project"),
        Project::new("Recovered"),
        Threads::Exactly(1),
    )
    .unwrap();
    rig.shell.recovery_project = Some((app, Vec::new(), true));
    let ctx = rig.ctx.clone();
    rig.shell.request(Next::Recovery, &ctx);
    rig.settle();
    let warning = about(&rig, "Recovered checkpoint");
    assert_eq!(warning.len(), 1, "{:?}", rig.shell.overview.messages);
    assert_eq!(warning[0].severity, Severity::Warning);
    assert_eq!(warning[0].action, Some(MessageAction::Save));
    assert!(warning[0].text.contains("Save the project to keep them"));
    // Its "Save" saves, and the warning goes.
    rig.shell.message_action(MessageAction::Save);
    rig.settle();
    assert!(about(&rig, "Recovered checkpoint").is_empty());
    assert!(!rig.shell.app.as_ref().unwrap().is_modified());
}

#[test]
fn req_ui_12_req_gc_14_changed_tool_numbers_warn_with_save_and_undo_until_either() {
    let mut rig = Rig::cube("reminder-tools", None);
    let renumber = |rig: &mut Rig| {
        let app = rig.shell.app.as_mut().unwrap();
        let command = tool_numbering_command(
            app.project(),
            &ToolNumbering::InOrderOfUse { first: 7 },
            None,
        )
        .unwrap()
        .unwrap();
        app.execute(command).unwrap();
        // As the export dialog reports it.
        rig.shell.tool_numbers_changed = true;
        rig.shell.overview_dirty = true;
        rig.settle();
    };
    renumber(&mut rig);
    let warning = about(&rig, "Tool numbers");
    assert_eq!(warning.len(), 1, "{:?}", rig.shell.overview.messages);
    assert_eq!(warning[0].severity, Severity::Warning);
    assert_eq!(warning[0].action, Some(MessageAction::Save));
    assert_eq!(warning[0].views, vec![MessageAction::Undo]);
    // Undo clears it.
    rig.shell.message_action(MessageAction::Undo);
    rig.settle();
    assert!(about(&rig, "Tool numbers").is_empty());
    // So does saving.
    renumber(&mut rig);
    assert_eq!(about(&rig, "Tool numbers").len(), 1);
    rig.shell.message_action(MessageAction::Save);
    rig.settle();
    assert!(about(&rig, "Tool numbers").is_empty());
}

#[test]
fn req_ui_12_a_dropped_pick_is_an_error_on_what_it_was_for_until_a_pick_is_made() {
    let mut rig = Rig::cube("reminder-pick", None);
    let project = rig.shell.app.as_ref().unwrap().project().clone();
    let setup = project.setups()[0].id;
    // On the working selection: an error with nothing to link.
    rig.shell.dropped_pick = Some(DroppedPick::Selection);
    rig.shell.overview_dirty = true;
    rig.settle();
    let error = about(&rig, "Pick");
    assert_eq!(error.len(), 1, "{:?}", rig.shell.overview.messages);
    assert_eq!(error[0].severity, Severity::Error);
    assert!(
        error[0].text.ends_with("Select it again."),
        "{}",
        error[0].text
    );
    // On the workholding: it names the setup and links to its workholding.
    let message = super::reminders::dropped_pick(&project, DroppedPick::Workholding(setup));
    assert_eq!(message.severity, Severity::Error);
    assert_eq!(message.action, Some(MessageAction::EditWorkholding(setup)));
    assert!(
        message
            .text
            .starts_with(&format!("Pick (setup '{}'): ", project.setups()[0].name)),
        "{}",
        message.text
    );
    // On a selection set: it links to the set and offers Reselect.
    if let Some(set) = project.selection_sets().first() {
        let message = super::reminders::dropped_pick(&project, DroppedPick::Set(set.id));
        assert_eq!(message.source, Some(ItemRef::SelectionSet(set.id)));
        assert_eq!(message.action, Some(MessageAction::Reselect(set.id)));
    }
}

#[test]
fn req_ui_12_req_nfr_07_importing_with_no_project_is_an_error_dialog_naming_the_action() {
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.shell
        .dropped_model(temp("reminder-import").join("part.stl"));
    match &rig.shell.dialog {
        Some(Dialog::Error(failure)) => {
            assert_eq!(failure.title(), "Import model failed");
            assert_eq!(
                failure.text,
                "Import model: there is no project open. Open or create a project first."
            );
        }
        other => panic!("expected an error dialog, got {other:?}"),
    }
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Import model failed"), "{:?}", rig.texts());
}

#[test]
fn req_ui_12_req_prj_08_recovery_notices_are_warnings_and_a_failed_write_an_error() {
    let mut rig = Rig::cube("reminder-recovery", None);
    rig.shell.recovery.problem = Some(
        "Another application instance owns this project's checkpoints. Recovery writing is \
         unavailable here."
            .to_owned(),
    );
    rig.shell.recovery.write_failed = Some("the disk is full".to_owned());
    rig.shell.overview_dirty = true;
    rig.settle();
    let notices = about(&rig, "Recovery");
    assert_eq!(notices.len(), 2, "{:?}", rig.shell.overview.messages);
    // Errors first: the checkpoint that could not be written.
    let failed = notices
        .iter()
        .find(|m| m.severity == Severity::Error)
        .unwrap();
    assert_eq!(
        failed.text,
        "Recovery checkpoint: the latest checkpoint could not be written (the disk is full), so \
         the unsaved changes since the last one are not kept if the application closes \
         unexpectedly. Save the project to keep them."
    );
    assert_eq!(failed.action, Some(MessageAction::Save));
    let warning = notices
        .iter()
        .find(|m| m.severity == Severity::Warning)
        .unwrap();
    assert!(
        warning
            .text
            .starts_with("Recovery: another application instance owns"),
        "{}",
        warning.text
    );
    // A later checkpoint written clears the error.
    rig.shell.recovery.write_failed = None;
    rig.shell.overview_dirty = true;
    rig.settle();
    assert!(
        about(&rig, "Recovery")
            .iter()
            .all(|m| m.severity == Severity::Warning)
    );
}

/// The text after a lead-in goes on in lower case, but a name in capitals
/// stays as it is (decision record 0536 section 3).
#[test]
fn req_ui_12_the_text_after_a_lead_in_starts_in_lower_case_unless_a_name() {
    use super::reminders::lower_first;
    assert_eq!(
        lower_first("Your settings could not be read"),
        "your settings could not be read"
    );
    assert_eq!(lower_first("STEP file missing"), "STEP file missing");
    assert_eq!(lower_first("x"), "x");
    assert_eq!(lower_first(""), "");
}
