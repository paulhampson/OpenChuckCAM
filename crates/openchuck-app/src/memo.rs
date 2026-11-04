// SPDX-License-Identifier: AGPL-3.0-or-later
//! Answers about the project that the interface asks every frame, made once
//! per change of the project (issue #470; UI-09, NFR-04, UI-11).
//!
//! The status keys of the operations (decision record 0117) hash every
//! input of every operation, the selection sets included; on a carving
//! whose region selects half a million triangles that is tens of
//! milliseconds a call, and the inspector asked for them twice a frame.
//! Whether the project has unsaved changes compares the whole project with
//! the saved one, and the title and the recovery checkpoints asked for it
//! every frame. Both answers depend on nothing but the project (and, for
//! the second, the project last saved), and the project changes only
//! through [`crate::Application::execute`], `undo` and `redo`, each of
//! which moves the application's revision on. So each answer is kept with
//! the stamp it was made at, and made again only when the stamp moves.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use openchuck_project::id::OperationId;

use crate::cache::StatusKey;

/// The answers an [`crate::Application`] keeps.
#[derive(Debug, Default)]
pub(crate) struct Answers {
    /// The status keys of the operations, at a revision.
    pub(crate) keys: Memo<u64, Vec<(OperationId, StatusKey)>>,
    /// Whether the project differs from the saved one, at a revision and a
    /// count of saves.
    pub(crate) modified: Memo<(u64, u64), bool>,
    /// How many times the project was saved: a save changes the saved
    /// project without changing the revision.
    pub(crate) saves: AtomicU64,
}

/// One answer and the stamp it was made at.
#[derive(Debug)]
pub(crate) struct Memo<S, T> {
    slot: Mutex<Option<(S, Arc<T>)>>,
}

impl<S, T> Default for Memo<S, T> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }
}

impl<S: PartialEq + Copy, T> Memo<S, T> {
    /// The answer at `stamp`: the kept one when it was made at `stamp`,
    /// otherwise `make()`, kept for the next call. A poisoned lock (a panic
    /// while making an answer elsewhere) is cleared, never trusted: the
    /// answer is made again.
    pub(crate) fn get(&self, stamp: S, make: impl FnOnce() -> T) -> Arc<T> {
        let mut slot = self.slot.lock().unwrap_or_else(|poisoned| {
            let mut slot = poisoned.into_inner();
            *slot = None;
            slot
        });
        if let Some((kept, answer)) = slot.as_ref()
            && *kept == stamp
        {
            return Arc::clone(answer);
        }
        let answer = Arc::new(make());
        *slot = Some((stamp, Arc::clone(&answer)));
        answer
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use openchuck_project::{Item, ParameterValue};

    use super::*;
    use crate::Command;
    use crate::cache::{status_keys, status_keys_made};
    use crate::jobs::JobLog;
    use crate::test_project::TestProject;

    fn fresh(app: &crate::Application) -> Vec<(OperationId, StatusKey)> {
        status_keys(app.project())
    }

    #[test]
    fn req_nfr_04_a_memo_makes_its_answer_once_per_stamp() {
        let memo: Memo<u64, u32> = Memo::default();
        let mut made = 0;
        for _ in 0..3 {
            assert_eq!(
                *memo.get(1, || {
                    made += 1;
                    7
                }),
                7
            );
        }
        assert_eq!(made, 1);
        assert_eq!(*memo.get(2, || 8), 8);
        assert_eq!(*memo.get(2, || 9), 8);
        assert_eq!(*memo.get(1, || 10), 10, "an old stamp is made again");
    }

    #[test]
    fn req_nfr_04_a_poisoned_memo_makes_its_answer_again() {
        let memo: Memo<u64, u32> = Memo::default();
        assert_eq!(*memo.get(1, || 7), 7);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = memo.slot.lock().unwrap();
            panic!("poison the lock");
        }));
        assert_eq!(*memo.get(1, || 8), 8);
    }

    /// Issue #470: with an operation selected the inspector asked for the
    /// status keys twice a frame, and the tree's overview again for each
    /// job event. Asked many times with no change, they are made once.
    #[test]
    fn req_ui_09_req_nfr_04_status_keys_are_made_once_per_project_change() {
        let mut p = TestProject::new("memo-keys-once");
        let first = p.add_operation("Roughing", "rotary_roughing");
        let second = p.add_operation("Finishing", "helical_finishing");
        let before = status_keys_made();
        for _ in 0..5 {
            for id in [first, second] {
                let _ = p.app.operation_path_is_current(id);
                let _ = p.app.operation_warning_settings(id);
            }
            let _ = p.app.overview(None, &JobLog::default());
        }
        assert_eq!(status_keys_made() - before, 1, "made once for five frames");
        p.app
            .execute(Command::SetParameter {
                operation: second,
                name: "stepover".into(),
                value: Some(ParameterValue::Number(0.7)),
            })
            .unwrap();
        for _ in 0..5 {
            let _ = p.app.operation_path_is_current(second);
        }
        assert_eq!(status_keys_made() - before, 2, "made again after a change");
    }

    /// The kept keys are always the keys of the project as it is: after every
    /// kind of change, an undo and a redo, a command that fails, and a
    /// recovered project (UI-11: the tree's states must not be stale).
    #[test]
    fn req_ui_11_kept_status_keys_equal_fresh_keys_after_every_change() {
        let mut p = TestProject::new("memo-keys-fresh");
        let first = p.add_operation("Roughing", "rotary_roughing");
        let check = |app: &crate::Application, step: &str| {
            assert_eq!(*app.status_keys(), fresh(app), "{step}");
        };
        check(&p.app, "start");
        let second = p.add_operation("Finishing", "helical_finishing");
        check(&p.app, "added");
        p.app
            .execute(Command::SetParameter {
                operation: first,
                name: "stepover".into(),
                value: Some(ParameterValue::Number(1.1)),
            })
            .unwrap();
        check(&p.app, "parameter");
        let mut off = p.app.project().operation(second).unwrap().clone();
        off.enabled = false;
        p.app
            .execute(Command::Replace(Item::Operation(off)))
            .unwrap();
        check(&p.app, "disabled");
        assert!(
            !p.app.operation_path_is_current(second),
            "an operation that is off has no current path"
        );
        assert!(p.app.operation_warning_settings(second).is_empty());
        assert!(p.app.undo().unwrap());
        check(&p.app, "undo");
        assert!(p.app.undo().unwrap());
        check(&p.app, "second undo");
        assert!(p.app.redo().unwrap());
        check(&p.app, "redo");
        assert!(
            p.app
                .execute(Command::SetParameter {
                    operation: OperationId::new(),
                    name: "stepover".into(),
                    value: Some(ParameterValue::Number(1.0)),
                })
                .is_err()
        );
        check(&p.app, "failed command");
        let recovered = crate::Application::from_checkpoint(
            &p.dir,
            p.app.project().clone(),
            crate::Threads::Exactly(1),
        )
        .unwrap();
        check(&recovered, "recovered");
    }

    /// Whether the project has unsaved changes is asked every frame (the
    /// window title and the recovery checkpoints): kept per change and per
    /// save, and always the answer of comparing with the saved project
    /// (PRJ-08: the recovery and the close guard depend on it).
    #[test]
    fn req_nfr_04_req_prj_08_unsaved_changes_are_compared_once_per_change_and_save() {
        let mut p = TestProject::new("memo-modified");
        p.app.save().unwrap();
        let compare = |app: &crate::Application| app.saved_for_tests() != *app.project();
        let check = |app: &crate::Application, step: &str| {
            assert_eq!(app.is_modified(), compare(app), "{step}");
        };
        check(&p.app, "saved");
        let before = crate::modified_compared();
        for _ in 0..5 {
            let _ = p.app.is_modified();
        }
        assert!(crate::modified_compared() - before <= 1, "compared once");
        let id = p.add_operation("Roughing", "rotary_roughing");
        check(&p.app, "added");
        assert!(p.app.is_modified());
        p.app.save().unwrap();
        check(&p.app, "saved again");
        assert!(!p.app.is_modified());
        p.app
            .execute(Command::SetParameter {
                operation: id,
                name: "stepover".into(),
                value: Some(ParameterValue::Number(1.1)),
            })
            .unwrap();
        check(&p.app, "changed");
        assert!(p.app.undo().unwrap());
        check(&p.app, "undone to the saved state");
        assert!(!p.app.is_modified());
        assert!(p.app.redo().unwrap());
        check(&p.app, "redone");
        let recovered = crate::Application::from_checkpoint(
            &p.dir,
            p.app.project().clone(),
            crate::Threads::Exactly(1),
        )
        .unwrap();
        assert!(recovered.is_modified(), "a recovered project is unsaved");
    }
}
