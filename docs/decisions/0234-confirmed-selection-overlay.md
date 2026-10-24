# 0234: Publish confirmed selection membership before its overlay (lane BUG-SELSET2; issue 298)

- **Status:** Accepted by the project owner on 24 Oct 2026 in the BUG-SELSET2 lane session, after presentation of the complete record: "Accept 0234."
- **Date:** 24 Oct 2026.
- **Requirements:** UI-05; PRJ-06 for stable membership and save/load; NFR-15 for approved colours and framed outlines.
- **Dependencies:** SELSET PRs 282 and 316, STEPALL-F PR 274 and BUG-EDIT PR 323 verified merged into `main` before design, implementation or tests. Work starts at `1a90435654c734c05e4fa3f110c4a7f3f8e5859f`.
- **Number:** 0234 reserved by the coordinator. No schema number and no new open item; BUG-SELSET2 does not use OI-984.
- **Amends:** no accepted interaction or palette decision. Implements decisions 0215 and 0216 and the existing interface specification, sections 7.4, 11.4 and 12.2.

## Context and reproduction

Issue 298 and all comments were read. The owner confirmed that faces were
stored correctly and only their colour was missing, in Operations. The later
trial build was 0.17.0 at `619fbc7`; that build's persistent Windows symptom
is not assumed to remain on current main. Decision 0215 replaced the old
"Add selection to set" button with checked Shift/Ctrl edits.

Current-main reproduction uses the existing small cube fixture, without the
unmerged STEPALL-M fixture. A chosen set holds triangles 0 and 1. A checked
add of triangle 2 completes in the part of a frame after layout, when the
inspector's earlier poll may already have happened. The shell publishes the
overlay first, then polls and stores the checked edit. Storage now holds
0, 1 and 2, while the published overlay still fills only two triangles.
`req_ui_05_checked_add_updates_stored_and_rendered_membership_in_the_same_frame`
failed before product changes with **2 drawn versus 3 stored**, in **0.46 s**
uninstrumented. The final ordering passes it in **0.324 s** in the full UI run.

This proves a late-completion publication mismatch in the shared shell, not
a persistent Windows or native STEP raster failure. Existing revision-based
overlay invalidation already detects a changed set on a later call; adding
another dirty flag is not the source fix.

BUG-EDIT's accepted final record 0233 and integration were read. Its semantic
pick/departure guard and selection-name Save/Clear tickets remain in use.

## Decision

1. Keep keyboard handling in its existing position, including Esc during
   Pick again. Apply finished selection checks and confirmed reset answers,
   then follow the panel's chosen-set change before deriving the shown set, legend and
   overlay. A checked add/remove or save must publish the confirmed state
   through the existing shared overlay path.
2. Keep the approved swatches, old-swatch display mapping, 40% fill and framed
   2-pixel set-colour outline inside the 4-pixel dark frame. Pending blue
   must not be drawn over a directly edited chosen set.
3. Retain application selection commands, stable IDs and model hashes,
   asynchronous checks and queue/cancellation rules, undo/redo, pending-edit
   confirmation and BUG-EDIT's form-departure guard. No CAM, staleness,
   dependency, library API or project-schema change.

## Alternatives and risk

- Another repaint/dirty flag would leave completion after publication and
  rely on another frame to repair the mismatch.
- Restoring the old Add button would contradict accepted decision 0215.
- Inspector or renderer refactoring is unnecessary for this shared ordering
  defect. The principal risk is changing how a completed save or a reset
  answer interacts with selection and confirmation in that frame; the
  existing shell/selection and BUG-EDIT tests are required verification.

## Verification

The regression checks actual stored membership and the viewport's published
overlay, not a repaint call. A second requirement test exercises Operations
first, then Model and Simulate: queued Shift additions and Ctrl removal,
each completed edit's geometry, approved fill and both outline widths, no
working blue, undo/redo, displayed count, save/load ID/hash/membership/colour,
and deselection. The stored historical swatch stays unchanged while its
approved replacement is displayed.

Completed local checks:

- Final uninstrumented UI suite: **427 passed**, **201.209 s**, no slow tests.
  The completion regression took **0.324 s**; the three-view regression took
  **2.542 s**. No new long-test classification is needed.
- Application selection and STEP selection-refusal checks: **41 passed**,
  **286.375 s**, 83 tests filtered out in those two binaries. Three existing
  cases exceeded a minute during concurrent work; isolated serial reruns
  took **20.566 s** (axial face sets), **24.789 s** (helical face sets) and
  **17.647 s** (per-set stock to leave). The existing hidden-field UI case
  that took 60.792 s during concurrent work took **9.935 s** alone. No
  long-test profile change is needed; instrumented timings are not used
  for classification.
- Viewport overlay, outline and hatch checks: **5 passed**, **8.699 s**,
  17 tests outside the filter. These include three offscreen pixel-readback
  tests using **SwiftShader Device (Subzero), Vulkan**, with
  `OPENCHUCK_REQUIRE_ADAPTER=1`; their pixel checks did not skip.
- Formatting, whitespace and UI Clippy (all targets, locked, warnings
  denied) passed. `cargo xtask all` passed deps, licences, records,
  testdata, conflicts, publish and trace; **99/99 P1 requirements have tests**.
  The decision/register check also passed on the final ordering.

Fresh changed-crate coverage:

- `cargo llvm-cov nextest --profile coverage -p openchuck-ui --locked --no-report`:
  **427 passed**, **1739.123 s**. The earlier profiles were cleared before
  this final source version; no profiles from the earlier ordering were mixed in.
- UI line coverage: **86.80% (22,707/26,160)**, below the 95% development
  target and report-only under decision 0102. The prescribed summary command
  and `cargo-llvm-cov 0.9.1` defaults were used; standalone test-only filenames
  are omitted by that tool's default filter, while inline tests still count.
  No UI product source was excluded to raise the result.
- Changed product lines left uncovered in `apps/openchuck-ui/src/shell.rs`:
  **1327 and 1330**, the existing failed-check/reset error-to-dialog routes
  moved with this fix. Their underlying refusal and cancellation behaviours
  are tested, but these shell dialog-conversion routes were not forced.
  LCOV also marks **1332**, the guard's closing brace, uncovered; an absent
  application returns earlier. These lines were read against UI-05 and NFR-07.
- Instrumented artifacts were cleaned with `cargo llvm-cov clean --workspace`
  after the report and changed-line inspection.

The new full-shell cube tests inspect the published overlay rather than
capture a native window's pixels. The separate existing renderer tests read
pixels through the software Vulkan adapter. Native Windows/macOS interaction,
physical-GPU behaviour and selection edits through the native STEP worker
were not exercised locally. The historical persistent trial symptom remains
unverified; issue 298 is referenced, not automatically closed.
