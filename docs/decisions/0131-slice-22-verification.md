# 0131: Slice 22 verification: smoke test, display checks, screenshots and the frame benchmark (lane X8)

- **Status:** proposed (20 Oct 2026). Lane X8, slice 22 step 3 ([decision 0114](0114-slice-22-conformance-report.md) section E, accepted). Only the project owner accepts it. Item 5 settles a rule the coordinator asked this lane to settle (OI-514); the owner may change it.
- **What it changes:** no requirement, no architecture text and no project format. Item 5 amends the "no model" wording of [decision 0148](0148-menus-and-view-directions.md) item 2.
- **Date:** 20 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md). OI-515 to OI-522 are new and closed in this change. OI-523 to OI-529 are new and open. OI-514 (closed) is edited. OI-360, OI-442 and OI-493 are updated and stay open.

Numbering: 0130 and 0133 are on `main`, 0131 and 0132 were assigned to this lane and are free on `main` and on every unmerged branch (checked 20 Oct 2026); this record takes 0131. The highest open item on `main` and every fetched branch was OI-514. The coordinator's numbers OI-349, OI-359, OI-505 and OI-507 to OI-509 never reached the register, so they are new items here from OI-515 and carry the old number in their text (the coordinator agreed, 20 Oct 2026). OI-385 to OI-389 are not used.

## Context

Decision 0114 gives lane X8 the last step of slice 22: a smoke test that opens every step, panel and dialog, the screenshots of UI-16, the *(display)* checks of the report, the rerun of the 1M-triangle frame benchmark (NFR-03), and the small findings carried from other lanes. Its board column in section E is "None": X8 compares with the boards as the other lanes re-rendered them, so step 1 (re-rendering) has nothing to render for this lane.

## Decision

1. **The smoke test** is `apps/openchuck-ui/src/shell/smoke_tests.rs`. It draws the whole shell at 1366 by 768 and 100% (UI-16) and opens: the five steps (by their keys too), every inspector (model, operation, the five setup sections including the workholding section), picking on the part, each dock tab, the nine modal dialogs (keys, About, settings, error, unsaved changes, offer to create, the two folder dialogs, rename), the import window, the export dialog, the tool library window, the new-operation dialog, and the results and playback after a run. A step or window that panics when drawn, or does not open, fails it. It runs with the unit tests of `openchuck-ui` (about 10 seconds).
2. **The display run.** The lane built the release application and ran it under Xvfb at 1366 by 768 with Mesa's `llvmpipe` software renderer (Vulkan), at the default 100% scale, with no window manager, on the cloud container (4 vCPU Intel Xeon at 2.1 GHz, 15 GB). It stepped through the Model, Setup, Operations, Simulate and Export steps, the workholding section, picking, the export dialog and the tool library window and took the screenshots in [docs/ui-design/screenshots-1366x768/](../ui-design/screenshots-1366x768/). They are the reference screens of UI-16 for this build, from a software renderer: fonts and layout are real, the colours and the line widths are the renderer's. What they show:
   - **Fits:** every step draws without overlapping panels, and the footer, the workflow bar, the toolbar strip, the tools box (placed under the strip, issue 159) and the Legend and cube overlays do not overlap each other.
   - **Findings** (open items, not fixed here, because each is a layout change in a file other lanes are changing): a Messages row's source link over its wrapped sentence (OI-524); floating windows placed from the top-left corner, taller than the window, with Esc not closing the tool library window (OI-525, which may be the absence of a window manager); unrounded numbers in the default-tab line (OI-526); the "Showing unapplied workholding" chip with nothing unapplied and a crowded front-pane corner (OI-527); the Simulate toolbar choices cut off at 1366 (OI-528); the tree legend wrapping "Warning" away from its mark (OI-529).
   - **Not checkable here:** contrast ratios, tooltips on keyboard focus, AccessKit names, and Windows or macOS behaviour (OI-493, OI-442, OI-440, OI-523, OI-430 stay with the owner).
3. **The frame benchmark (NFR-03).** `openchuck-ui --bench` on the 1,002,000-triangle model of `bench_model`, shaded with edges, 10 seconds of the scripted path, same container, `llvmpipe` (a CPU renderer, 762 by 404 pixel view): 86 frames, mean 117.2 ms, p95 164.0 ms, p99 183.0 ms, which fails the 33 ms limit as expected in software. This is not the NFR-03 result: that is the run on the reference hardware (verification plan 4.2; OI-264, OI-266), which is the owner's. The benchmark still runs and draws.
4. **Carried findings fixed, each test first:**
   - OI-515 (task OI-349): "Write G-code" has the primary styling only when it can be pressed.
   - OI-516 (task OI-359): the dock's limit is half the window, not what the first frame leaves; a size the window clamped is not saved as the user's; a stale panel asks again for the saved size. The Windows check is the owner's (OI-523).
   - OI-517 (task OI-505): the "+N labels hidden" note of the Setup panes is a Fixed request of the label layout, so no label is placed under it.
   - OI-518 to OI-520 (task OI-507 to OI-509): the `xtask records` row cells keep an empty last cell; the decision index checks a row's link number against its file; the golden-file tests use one predicate, `golden::blessing`, true only for `OPENCHUCK_BLESS=1`.
   - OI-521 and OI-522 (CodeRabbit on pull request 164): switching to a filter that cannot hold the selection clears it when the warning is off; a pick is ignored while the reset question is open.
5. **One rule for the view directions (OI-514).** The toolbar buttons and the View menu's "View direction" are on when the shell is not on Setup's fixed panes and **the open project holds a model**; otherwise they are off with the reason "Import a model first". The shell passes the project fact to the view once a frame (`Viewport::set_project_has_models`), and both places call `directions_unavailable` with it. The alternative, the scene's model-backed meshes, would turn the toolbar off for a project whose scene is briefly empty (a job building) while the menu stays on; the project is the stable fact. An import preview alone leaves both off, as OI-514 asked.

## Alternatives considered

- Reading the scene for the rule (the first fix of OI-514): rejected, item 5.
- Fixing the layout findings of item 2 in this lane: rejected, they are in files other lanes still change (the dock, the Setup panes, the toolbar) and each needs a design choice; the lane records and screenshots them.
- Faking a renderer to take screenshots in the test suite: rejected, the screens must come from the real application.

## Consequences

- `Viewport` gains `set_project_has_models` and `project_has_models`; `RunPanel` gains `is_adding`; `golden` gains `blessing` and `is_bless_value`. No library crate changes its public API outside the testkit.
- The screenshots are 1.8 MB of PNG in `docs/ui-design/screenshots-1366x768/`; a later lane replaces them when the layout changes.
- The work breakdown's X8 row and slice 22 status record the lane; X8 does not close slice 22 (OI-334, OI-342, OI-360 and the findings above are open).
