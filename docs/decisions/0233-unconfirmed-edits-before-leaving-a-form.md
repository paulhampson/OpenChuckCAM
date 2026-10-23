# 0233: Confirm uncommitted edits before leaving a form (lane BUG-EDIT; issue 304)

- **Status:** proposed. The owner chose "Auto-confirm between fields; ask when leaving the form" on 23 Oct 2026, relayed by the coordinator under decision 0049 (OI-983). That interaction choice authorises implementation; it is not acceptance of this record.
- **Date:** 23 Oct 2026.
- **Implementation status:** uncompiled continuation checkpoint. The initial Continue regression failed and Enter comparator passed before the implementation; the subsequent prototype and expanded 13-test module have not been compiled or run. Formatting and whitespace checks alone do not establish correctness.
- **Requirements:** UI-07 (validated fields), UI-08 (commands and undo), UI-10 (workflow navigation), UI-11 (staleness and errors), UI-26 (number-box confirmation), NFR-07 (clear refusal), NFR-12 (the user verifies generated output), GC-07 and NFR-06 (output follows committed inputs).
- **Amends:** the focus-loss wording of decisions 0123 item 1 and 0206 item 3, and interface specification sections 8 and 10: ordinary field-to-field confirmation remains; departure from a form first resolves uncommitted input. Existing Save and Apply boundaries of decisions 0127 and 0104 remain.
- **Open items:** OI-983 is the owner's interaction choice, answered as above.

## Context

In issue 304 (trial feedback F-110), the owner typed a stepover, left the
keyboard in its box and chose Continue to simulate. Returning to Operations
showed the typed value, but the operation appeared current. The owner could
not tell which value the path and simulation used. The issue and its comment
were read through the GitHub connector.

The shell draws its footer before its inspector. A departure can therefore
replace the inspector before it processes focus loss. A typed buffer is not
the project: verification must inspect the saved project, the regeneration's
project snapshot, its path and simulation, and its staleness, rather than
infer a project change from the number painted in the inspector.

Initial uninstrumented frame reproduction on main `a260549` (23 Oct 2026):
with committed stepover 0.8, type 1.6 and choose Continue to simulate. The saved
and reopened project still holds 0.8, the regeneration’s snapshot holds 0.8,
and its path and simulation are unchanged. The operation is Up to date and
the step is Simulate, while `FieldState` still holds typed text `1.6`. The
missing-warning regression failed as intended (3.266 s). The Enter comparator
passed (4.887 s): it stores 1.6, marks the old run stale, supports undo and
redo, and generates a different path. This reproduces the misleading retained
buffer, without evidence of a silent project write in this case.

Relevant accepted records: 0172 and 0173 (staleness and required values), 0190
(cached paths and stock), 0191 (the build identity), 0206 (arithmetic), 0123
(field rows), 0127 (library drafts), 0104 (workholding drafts), 0049, 0055,
0102 and 0130. This is an interface change; CAM calculations, cache inputs,
project schema and dependency rules do not change.

## Decision proposed

1. **A form departure is guarded in one shared place.** Navigation, changing
   the selected item or selection, generation, opening another dialog,
   saving or replacing a project, and closing a form, project or application
   first resolve any unconfirmed fields of the form being left. The
   departure must be stopped before focus loss can silently commit a value
   or the next action can read old input.
2. **The owner's interaction choice.** Enter confirms a field, Esc restores
   its last confirmed value, and moving between fields of the same form
   retains ordinary focus-loss confirmation. A changed field does not cause
   a prompt merely because another field in its form gets focus.
3. **One warning.** "Unconfirmed changes" names the changed fields and asks
   whether to keep or discard them, with Keep, Discard and Cancel. Untouched
   fields and inputs that already apply immediately have no pending edit.
   The same guard serves the inspectors, setup, tool library and dialogs,
   including number expressions and text fields.
4. **Keep.** Check all pending entries before applying any. A refusal keeps
   the last confirmed values, shows the existing inline reason and number
   error mark, focuses the first invalid field, and stops departure. Valid
   project changes use existing commands, undo and staleness. Only then is
   the original action re-evaluated against the changed project: an old
   Continue action cannot bypass a newly stale operation.
5. **Discard.** Restore the last confirmed field buffers and resume the
   requested action using the unchanged committed project. No draft number
   or expression reaches the project, path, simulation or G-code.
6. **Cancel.** Stop the requested action, preserve the edits and return
   keyboard focus to the edited field. Cancelling an application close
   also cancels the native close request.
7. **Existing drafts retain their transaction boundary.** Confirming a
   field in a library or workholding draft changes that draft only. The
   existing Save or Apply remains necessary to persist it; the guard must
   never silently save a library tool or apply workholding. Existing
   unsaved-project and changed-library-draft warnings follow after field
   resolution where necessary.

## Alternatives considered

- **Ask on every field-to-field transition.** Rejected by the owner's
  OI-983 answer; it would interrupt ordinary multi-field entry.
- **Commit automatically on every form departure.** Rejected by the
  explicit warning rule in issue 304.
- **Fix only Continue or change the drawing order.** Rejected: either
  leaves other departures dependent on the order in which widgets draw.
- **Write typed values into the project before asking.** Rejected: a draft
  would become a project edit without confirmation or reliable undo and
  staleness.

The pending departure is an owned semantic action (for example Primary, a step,
or an operation ID). Pointer coordinates and raw event snapshots are never
replayed: Keep may change the layout or button eligibility. Early frame input classification
only identifies departure early enough to stop a premature focus-loss commit;
a shared dispatcher retains and later re-evaluates the actual action. Native
close requests are vetoed while the guard is pending.

## Verification and risks

Requirement-first shell tests type stepover without Enter and attempt
Continue, switch step, select another operation, open a dialog, generate,
save and close. They exercise Keep, Discard and Cancel, the saved project,
the run's input snapshot, path and simulation, command undo/redo, the stale
state, invalid arithmetic and multi-field edits. Enter is the comparator.
Shared widget tests cover normal field-to-field confirmation and the
separate Save/Apply boundary of enclosing drafts.

The main risks are identifying the form boundary consistently for floating
windows and keyboard actions, replaying a departure before validation and
commands have completed, losing focus on Cancel, and applying one valid
field before another is refused. Tests must exercise these through real
frames. The guard must re-evaluate the original action, rather than cache
its old eligibility or old primary-button meaning.
