# 0233: Confirm uncommitted edits before leaving a form (lane BUG-EDIT; issue 304)

- **Status:** accepted by the project owner (24 Oct 2026, directly in the BUG-EDIT lane session, after reviewing the complete record). The owner previously chose "Auto-confirm between fields; ask when leaving the form" on 23 Oct 2026, relayed by the coordinator under decision 0049 (OI-983).
- **Date:** 23 Oct 2026.
- **Implementation status:** implemented. The full 425-test UI suite passes locally, including 39 BUG-EDIT requirement tests. Coverage and uncovered lines are recorded below.
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

## Decision

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

The shared field registry uses ordered widget IDs and records each field's
form and containing sections. A collapsed section reopens when Keep or a
restored focus needs its field. Preflight checks syntax and field rules, then
applies interacting setup or tool edits to private candidate copies using the
application's existing editors, in confirmation order. A refusal applies no
pending field. Only after preflight succeeds do the ordinary field handlers
execute their commands; the guard refreshes the overview before dispatching
the requested action.

Before any confirmation, a preparation frame reopens all pending sections and
refreshes the fields' checks. If a pending field is still not drawn, the guard
returns to the warning with its name and the pending values preserved. Cancel
or Esc returns to the form so the field can be shown again; Discard remains an
explicit choice. There is no timeout that discards input. Fields use the form
and semantic row key as their identity (plain dialog fields use their label),
so conditional controls and asynchronous inspection details cannot strand a
draft under an old layout-generated widget ID.

Operation and setup forms retain every edit returned in a frame. A focus-loss
confirmation followed by a shape, material or other control change therefore
keeps both edits and their existing commands. Drop-downs participate in the
same form's field-to-field confirmation, alongside text boxes and choices.

Deferred scene picks retain model and geometry identity and the model file's
hash. A changed file cancels the old pick. A placement change waits for the
scene rebuild, then resolves current mesh indices, edge indices and surface
normals. Held-region and tab picks resolve against the current workholding
draft as well as stored workholding; field confirmation still does not Apply
that draft.

## Verification and risks

Requirement-first shell tests type stepover without Enter and attempt
Continue, switch step, select another operation, open a dialog, generate,
save and close. They exercise Keep, Discard and Cancel, the saved project,
the run's input snapshot, path and simulation, command undo/redo, the stale
state, invalid arithmetic and multi-field edits. Enter is the comparator.
Shared widget tests cover normal field-to-field confirmation and the
separate Save/Apply boundary of enclosing drafts.

The continuation compiled the checkpoint before changing product code. Its
13 tests passed after correcting two fixture assumptions: the setup test
must actually leave the Setup step, and floating dialogs need entry lookup
independent of their background colour. Additional requirement regressions
then failed before the corresponding fixes: partial tool-field application
(0.738 s), lost inline refusal on Rename (0.401 s), a pick stamped with a
replacement model's hash (1.597 s), disabled library Save while a field was
pending (0.781 s), a lost setup shape choice (0.616 s), a preset drop-down
that did not confirm the active field (1.511 s), and a draft held-region pick
that could not resume (0.939 s), and a reused Rename widget restoring the previous item's name on Discard (1.445 s). Project-folder Browse (0.409 s), model replacement (0.589 s) and the full model report (0.367 s) also bypassed confirmation and failed their regressions before being routed through the shared guard. These are uninstrumented times.

The review regressions reproduced an indefinite "Confirming changes…" modal
after a custom post folder was hidden by choosing a built-in post (2.206 s),
and Keep accepting a tab count of zero and leaving the form (1.031 s), before
their fixes. The recovery test also exposed an output-folder widget ID that
changed when the custom post field was shown again. It now checks that a
hidden field blocks every pending commit, Cancel and Esc preserve both drafts,
Keep can resume after the field is restored, and Discard restores the hidden
buffer when it is drawn again. Count tests cover zero, 65, invalid arithmetic,
a fractional count, and the accepted boundaries 1 and 64; Keep changes only
the workholding draft and never applies it.

The module contains 39 requirement tests. They include real pointer and
keyboard frames, native viewport close events, interacting box bounds, tool
constraints, section collapse and focus recovery, scene rebuilding after a
rotation, edge identity after scene reordering, library Save, and the
workholding Apply boundary. Native picker invocation is replaced by a test probe in the Browse and replacement frame tests, so their ordering and restored inputs are checked without opening an OS window. Native OS close behaviour, GPU drawing and
Windows/macOS interaction remain outside these headless tests.

The main risks are identifying the form boundary consistently for floating
windows and keyboard actions, replaying a departure before validation and
commands have completed, losing focus on Cancel, and applying one valid
field before another is refused. Tests must exercise these through real
frames. The guard must re-evaluate the original action, rather than cache
its old eligibility or old primary-button meaning.

### Local verification and coverage

- Uninstrumented after the review corrections: all 425 UI tests passed in 131.501 s, including 39 BUG-EDIT tests. The slowest test took 19.833 s; the new hidden-field and count tests took 16.879 s and 11.069 s. None needs a long-test profile. The previously intermittent placement-pick regression passed 20 consecutive runs after shared field identities were stabilised.
- Clippy (`openchuck-ui`, all targets, warnings denied), formatting, whitespace and all repository checks passed: deps, licences, records, conflicts, testdata, publish and trace (99/99 P1 requirements have tests).
- Instrumented after the review corrections: all 425 UI tests passed in 449.266 s. Instrumented timings do not classify long tests.
- `cargo llvm-cov report --summary-only --show-missing-lines --ignore-filename-regex '/(tests|examples|benches)/'`: **86.44% UI lines (23,822/27,560)**. UI is report-only under decision 0102; this is below the 95% development target. Shared field rows are 96.68%, the pending-entry registry 98.31%, the shell guard 95.86%, and deferred-pick identity resolution 80.47%. No product code was excluded to raise the figure.

The uncovered new lines were read against the requirements. Remaining gaps
include export tool-number and successful custom-post variations, import-path
dialog variations, cutting-data refusals, workholding check/revert controls,
other shortcut/menu/drop-file entry points, native picker success paths,
STEP face/edge rebinding, and stored end/tab selection variants. The cube's
scene rebuild completed before deferred dispatch, so the explicit wait/repaint
branch was not forced. Defensive missing-context/identity/type checks,
Debug formatting, and ordinary field Enter/Esc condition arms also remain. These are verification limits, not a claim that all
uncovered code is defensive. The generic dialog-owned ticket protocol is
exercised by real New operation Add (Keep/Discard/Cancel) and explicit
workholding Apply, including project state and undo.

New uncovered executable line ranges, relative to the merged PR base `e590f4d`:

| UI source file (under `apps/openchuck-ui/src/`) | New uncovered lines |
| --- | --- |
| `export_panel.rs` | 458-462, 734-736, 738-749 |
| `field_row.rs` | 148-155, 528-529 |
| `form_panel.rs` | 302-306, 309-310, 312-315, 317 |
| `import_panel.rs` | 324, 512, 514, 516-519, 522-527, 529, 531-533, 536-537 |
| `model_panel.rs` | 611 |
| `pending_edits.rs` | 28-30, 43, 166, 168, 405 |
| `results_panel.rs` | 718 |
| `selection_panel.rs` | 1534-1535, 1558-1559 |
| `shell.rs` | 556, 608, 624, 642, 682, 691, 715, 737, 945, 988, 1001, 1006-1011, 1016, 1053, 1086, 1108, 1223, 1438, 1504, 1556-1564, 1566, 1569, 1808-1815, 1836-1841, 1846, 1856-1858, 1861, 1871-1872, 1884-1885, 1961, 1982, 2006, 2109-2110 |
| `shell/frame.rs` | 145, 652, 823, 859, 863, 867 |
| `shell/inspector.rs` | 412, 505, 508-512, 514-516, 634-642, 819, 1080, 1696-1698, 1746-1747 |
| `shell/pending.rs` | 29-31, 51-52, 63, 77 |
| `shell/pending_pick.rs` | 94, 99-102, 108-109, 114-115, 134-136, 138, 166-169, 171-174, 197 |
| `tool_library_window.rs` | 937-949, 1846, 1853, 1897, 1902-1903 |
| `workholding_panel.rs` | 913-914, 916, 1358, 1400, 1409, 1413, 1419 |
