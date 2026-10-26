# BUG-SIMEND verification

Started from main `ba10400fff3bb7ad32526a7e540919c810e85434`, library 0.23.0.
Rebased before final validation onto current main
`8f788bf199ef580d62c4c27b023a46e2eb5581ec` (upstream #356, library 0.24.0).
This lane changes no library code or version relative to that main.
Refs #334 and #330. No library-family, project-schema or export changes.

## Cause found before the fix

The initial end-state checks passed because they inspected the updated CPU stock,
not the viewport snapshot already handed to rendering. Those checks did not
reproduce the reported display failure and do not establish a fix by #341.

`Shell::draw` calls `layout` (including `Viewport::show`) before `run_frame`.
The viewport's paint callback captures the current layer Arcs during layout.
`PlaybackPanel::update_stock` then receives and publishes the final stock surface,
but returns false when no further job or playback is running. `run_frame` therefore
does not request the next frame needed to capture that surface for rendering.
The last callback retains the previous stock until another interaction draws it.

The requirement regressions now follow that actual draw order and stop when
playback stops requesting frames. Jump, step and play-to-end each fail on current
main: the CPU surface is final but the surface handed to drawing is not. Cases
cover colours off/on, initial preparation and an earlier surface job pending.
The failure occurs in UI display publication, not simulation.

The same drawing-order tests fail 3/3 on pre-#341 main
`ab863f6531d1e8fab8c2562e88d92b9277937c3f` (10.646 / 10.922 / 11.120 seconds).
After the owner identified dragging the playhead as the route, an actual pointer-drag
test also failed there (12.960 seconds), at the final drawing assertion rather than
the final-cursor or CPU-surface assertions.
The only adaptation is the old `shown: usize` versus new `shown: Cursor` assertion.
#341 changes partial-move cutting and synchronized tool/surface publication; it
does not arrange a frame after final publication.

For #330, both injected checkpoint-memory and cut-surface failures fail the new
control-explanation assertion before any product change. Checkpoint exhaustion
is injected at the job boundary; the UI test does not allocate over 1.5 GiB.
Existing application tests independently cover the actual checkpoint bound.

## Fix

Completed checkpoint/surface jobs return a one-frame repaint request, including
failure, so layout can display the new stock or error after publication. Subsequent
idle frames stop normally; no retry or continuous repaint loop is introduced.
The control is disabled when stock preparation has failed, with the original error
beside it and in its disabled tooltip. The remembered colour preference, retained
measurements, stock-triangle identities, mapping, picking, staleness and export
rules are preserved. No target-model fallback is proposed or implemented.

## Validation

Final-base checks: `cargo fmt --all`, `cargo clippy --workspace --all-targets`,
`cargo nextest run -p openchuck-ui --locked --test-threads 4` (**491/491 passed,
285.645 seconds**), and `cargo xtask deps`, `licences`, `records`, `trace` all pass.
Traceability reports **100/100 P1** requirements with tests.

New requirement tests (uninstrumented final-base full-suite timings):

| Test in `playback_panel::tests` | Seconds |
| --- | ---: |
| `req_sim_03_req_sim_04_final_cut_part_is_published_after_jump_to_end` | 43.286 |
| `req_sim_03_req_sim_04_final_cut_part_is_published_after_play_to_end` | 43.115 |
| `req_sim_03_req_sim_04_final_cut_part_is_published_after_step_to_end` | 43.306 |
| `req_sim_03_req_sim_04_req_ui_02_final_cut_part_is_drawn_after_dragging_playhead_to_end` | 45.124 |
| `req_sim_05_req_ui_11_req_nfr_12_req_gc_15_checkpoint_memory_failure_explains_colours` | 3.235 |
| `req_sim_05_req_ui_11_req_nfr_12_req_gc_15_failed_surface_explains_colours` | 7.648 |

The surface-failure regression extends the existing failed-job test, with a shared
helper for the checkpoint failure. Both cover current/stale states, retained errors,
disabled control and visible reason, stopped progress, no automatic retry, retained
run identity, unchanged current export readiness and blocked stale export. The
existing pipeline-failure/stale-export test is retained and its name now also cites
NFR-12/GC-15. Existing successful current/stale colouring, opacity, stock picking,
pixel/occlusion, output-preservation and synchronized-tool tests pass in the full run.

No new test exceeds a minute uninstrumented. The full dock matrix took 80.138 seconds
and is already listed in both nextest profiles. The new end-state tests allow coverage
instrumentation overhead in their deadline without adding work. Their drawing-order
assertion checks the surface handed to the viewport callback, not physical monitor
pixels; existing software-adapter tests cover rasterization separately.

Own-test coverage command:
`cargo llvm-cov nextest --profile coverage -p openchuck-ui --locked --no-report --test-threads 4`:
**489/489 passed in 736.498 seconds**, two existing long tests excluded by the
coverage profile (both passed uninstrumented). The new end-state cases took
83.918–95.239 seconds instrumented; these are not their test-cost measurements.

Reports use the prescribed `/(tests|examples|benches)/` filename filter. Summing
UI source-file line totals from the JSON gives **26,141 / 29,789 = 87.75%**, below
the existing report-only 95% development target. The changed `playback_panel.rs`
file is **1,920 / 2,009 = 95.57%**. The full dependency-graph report total is not
presented as UI coverage.

Every added production line is covered: checkpoint publication's repaint request
executed 24 times, surface publication's 54 times, and the new unavailable-colour
explanation four times. The original Playback error label also executed four times
in the paired current/stale failure cases. New uncovered lines are only the test
helper's `Shape::Vec` recursion (`playback_panel.rs` lines 2254–2257): these egui
fixtures emit flat shapes, so that nested-group traversal is not exercised. No
extra exclusions were added. Changed lines and report counts were read before
`cargo llvm-cov clean --workspace`; instrumentation is cleaned afterwards.

Normal build artifacts were cleaned after checks to leave room for the instrumented
build; no source files were removed.

Nothing here verifies G-code for machining. Owner/reference-hardware display
verification remains OI-987; OI-988 remains closed. The owner retests #334/#330 and
closes them; the PR remains draft for the Merge Manager, under decision 0055.
