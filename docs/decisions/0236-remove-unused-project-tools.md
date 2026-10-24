# 0236: Remove unused project tools (lane TOOLLIB2; issue 313)

- **Status:** accepted by the project owner on 24 Oct 2026, directly in the TOOLLIB2 session, after reviewing the complete unnumbered record: "I accept the decision record". The changed-candidate behavior was accepted separately before that review.
- **Date:** 24 Oct 2026.
- **Requirements:** TL-01, TL-07, TL-11, GC-14, UI-08, UI-14, UI-16, GC-07 and NFR-06.
- **Amends:** UI specification section 12.1 and the corresponding verification-plan entries.
- **Dependencies:** TOOLLIB #266, OPSPEED #318 and BUG-EDIT #323, all verified merged into `main`.
- **Numbering:** the owner instructed this lane to choose a number and let the Merge Manager resolve collisions. The initial remote-branch audit found 0234 as the highest claimed file number; the Merge Manager reserved 0235 for BUG-SELSET2, so this record takes 0236. The released main update reconfirmed merged overlay record 0235 and held STEPALL-M replay record 0234 / OI-984; this record retains 0236 and its accepted content. No project schema change or new open-item number.
- **Implementation:** implemented for Merge Manager review on `codex/lane-toollib2-remove-unused-tools`; local verification below.

## Context

A project can retain tool snapshots after its operations stop using them.
Issue 313 requests a manual action to review and remove these unused project
copies. The library is separate from the project (TL-01). Project snapshots
preserve their own tool data and numbers (TL-07, TL-11). Cleanup must preserve
operation references, surviving tool numbers and generated output, and support
normal project undo and redo.

## Decision

1. **Manual action.** Add "Remove unused tools…" beside the tool library
   window's "This project" list. It examines the whole project, independently
   of search and display filters. Without an open project, it is unavailable.
2. **Definition of unused.** A project tool is unused only when no operation
   anywhere in the project references its ID. Disabled operations count as
   users. Setup, visibility, generation status and displayed workflow step do
   not change this rule.
3. **Review list.** Show each unused tool's name, type, relevant dimensions
   and project tool number, in project tool-number order. Each row has a
   checkbox, initially unchecked. Explain: "Only project copies are removed.
   The tool library is unchanged. Tools used by disabled operations are
   retained." If there are no candidates, show "No unused project tools."
   with a Close action.
4. **Explicit confirmation.** After selecting tools, the user chooses
   "Remove selected…". A confirmation names the selected tools and numbers,
   states that removal affects project copies only, and offers Remove and
   Cancel. Cancelling either stage changes nothing. An empty selection cannot
   start removal.
5. **Current-state validation.** Before removal, validate every selected
   tool against the current project. It must still exist, match the snapshot
   reviewed by the user—including its number—and remain unused. If any
   selected tool changed, disappeared or became used, remove nothing. Explain
   that the project changed, refresh the candidate list, clear the selection
   and require a fresh selection and confirmation. Newly unused tools are
   never silently added to the selection.
6. **One undoable project edit.** Remove the validated selection through one
   coherent project command using existing command and batch mechanisms.
   Mark the project modified. Undo restores exact snapshots, IDs, controller
   mappings and ordering; redo removes the same tools while preserving
   reference validity. Empty, cancelled and refused actions create no history
   entry.
7. **Tool numbers and history.** Surviving tools keep their numbers. Removed
   numbers become available under existing allocation rules, including machine
   tool-set reservations. Normal history ordering applies: a later command
   that reuses a number must be undone before the removal is undone. Cleanup
   introduces no separate number allocator.
8. **Output and staleness.** Removing unused tools does not mark unrelated
   toolpaths or simulation stale. At the same software version and with the
   same output settings, generated G-code remains byte-identical.
9. **Pending edits and drafts.** Opening cleanup and confirming removal use
   BUG-EDIT's shared pending-edit guard. Keep validates and confirms pending
   fields before the action is re-evaluated; Discard restores confirmed
   values; Cancel returns to editing. Existing library Save and workholding
   Apply boundaries remain explicit. Cleanup never saves or discards an
   enclosing draft implicitly.
10. **Layer ownership.** Reference discovery, candidate comparison and
    removal validation belong in the application/project layers. The UI
    presents results, collects selections and requests commands. Reuse
    existing project reference checks rather than duplicating them in the UI.
11. **Persistence and scope.** Save the resulting project using the existing
    format. No new project field or schema version is required. Shared and
    personal library tools, files and machine tool sets remain unchanged.
    Automatic cleanup and export warnings are outside this decision.
12. **Keyboard and layout.** The action, checkboxes and confirmation controls
    are keyboard accessible. The candidate list scrolls while explanation and
    action controls remain reachable at supported window sizes and scales.
    Escape cancels the active cleanup stage without removing tools and
    respects existing text-edit Escape behavior.

## Alternatives considered

- Skip changed candidates and remove the rest: rejected by the owner's choice
  because the result could differ from the selection reviewed.
- Automatically remove tools after deleting operations or during export:
  outside the agreed manual scope.
- Ignore disabled operations: rejected because they retain valid references.
- Delete library entries too: rejected; cleanup concerns project snapshots.
- Renumber surviving tools: rejected; numbers and output must be preserved.
- One history entry per removed tool: rejected; cleanup is one user action.

## Verification

Write requirement-based tests and observe failures before implementation:
candidate information across all setups, enabled/disabled references,
partial selection, confirmation, cancellation, empty selection/no candidates,
edited/renumbered/removed/newly referenced selections with no partial removal,
one history entry and exact undo/redo, freed-number reuse and history,
persistence, unchanged temporary library files/tool sets, modified state
without unrelated staleness, byte-identical G-code, pending-edit
Keep/Discard/Cancel and Save/Apply boundaries, keyboard access and supported
size/scale layout. Run uninstrumented tests with timings, classify long tests,
inspect changed-crate coverage, clean instrumented artifacts and complete
formatting, lint and repository checks. Report incomplete checks and gaps.

## Consequences and coordination

Update UI specification, verification plan, work breakdown, changelog and
applicable registers in the implementation PR. No new dependency or
library-family API change is expected. Shared tool-library/dialog ownership
must be checked before editing; record acceptance alone does not establish
ownership. Number selection is authorised as recorded above.


## Local verification (24 Oct 2026)

- Requirement regressions were written first. The initial cleanup regressions
  failed at the unimplemented query (0.01 s), and the whole-frame regression
  failed because the action was absent (0.18 s). A later dimension regression
  failed because corner radius was omitted (0.655 s), before using the
  editor's existing dimension list for all subtypes.
- Eleven new requirement tests cover discovery across setups and disabled
  references; reviewed-state validation, partial selection and refusal;
  exact single-step undo/redo, number reuse and persistence; unchanged library
  bytes, generated G-code and current cached results; confirmation/cancel,
  empty lists, an unavailable library, pending-edit Keep/Discard/Cancel without
  saving or losing the draft, keyboard operation, and populated/empty layout
  at 1366 × 768 logical pixels at 100%/120% and 1920 × 1080 at 100%, using the
  application fonts and style for the layout checks.
- Uninstrumented: the combined app/UI run passed 910 tests in 824.213 s;
  the final complete UI run passed 432 in 147.230 s. The final four cleanup
  app regressions passed in 1.657 s and seven cleanup UI regressions in
  6.206 s. Every new test took under six seconds in the final runs. No
  uninstrumented test exceeded ten minutes. Nine existing machining
  regressions that exceeded a minute are now excluded from development
  `coverage` and selected by nightly `long`, with their measured times in
  `.config/nextest.toml`; default still runs them. The slowest existing test,
  already classified long, took 165.425 s.
- Both changed crates pass all-target clippy with warnings denied. Formatting,
  whitespace and all repository checks pass: deps, licences, records,
  testdata, conflicts, publish and trace (99/99 P1 requirements have tests).
- Own-crate instrumented runs: app 472 tests pass in 802.660 s; UI 432 pass
  in 452.890 s. These instrumented times do not classify long tests. The app
  run includes the roughing-direction regression whose long classification
  was added during that run; the final development coverage profile excludes
  it. Instrumented workspace artifacts are cleaned after reporting.
- Crate line coverage uses the same LCOV LF/LH aggregation as `xtask coverage`,
  with the prescribed tests/examples/benches directory filter: app **92.40%**
  (15,712/17,004), UI **86.52%** (23,948/27,680). Both are below the 95%
  development target; own-crate tests give a lower bound on nightly coverage.
  No product code was excluded to raise the figures.
- New executable source lines with individual LCOV counters: app **37/37**;
  UI **114/117**. The uncovered new UI lines in `tool_library_window.rs` are
  1544 (no cleanup dialog), 1547 (removal requested before confirmation) and
  1555 (no command for an empty selection). These are defensive action paths:
  the displayed Remove button requires a confirmed, nonempty selection.
  The application empty-selection behavior is tested directly.

Native OS picker/close behavior, GPU drawing and Windows/macOS interaction
were not exercised locally. The layout and keyboard evidence comes from
headless Linux frames; this does not close the tool library's wider native
display/accessibility follow-ups (OI-493 and OI-773).

## Released-main reconciliation (24 Oct 2026)

Merge Manager marked PR 326 ready first and explicitly released its main update.
Merged main `049c70ab1c456ceadad5339b09167e81fafd29ec` (including PR 325),
without product conflicts. Reordered the union decision index: merged overlay
0235 precedes cleanup 0236. Refreshed all remote heads and checked persisted
reservations: TOOLLIB2 retains accepted 0236; STEPALL-M retains proposed replay
0234 / OI-984; no TOOLLIB2 OI or schema allocation. Acceptance and substantive
cleanup behavior are unchanged. Separate other-lane 0237 claims were reported
to Merge Manager for coordination through the PR conversation.

The full uninstrumented app/UI suite passed **915 tests in 616.386 s**, with
no skips or failures. Its slowest test took **107.977 s**. The existing STEP
helical face-set regression took **66.114 s**; its exact qualified test name is
now excluded from development coverage and selected by nightly long, verified
using both profile lists. It remains in default. No test exceeded ten minutes.
Both changed crates pass all-target clippy with warnings denied; formatting,
whitespace, all xtask checks (including records and 99/99 P1 traceability) and
`cargo audit --deny warnings` pass. The original coverage evidence and native
display limitations above remain applicable; no cleanup product code changed
in this reconciliation and coverage was not remeasured.
