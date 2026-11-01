# 0631: Very large toolpaths: the interface asked for the operations' keys every frame; they are now kept per change of the project (lane LARGEPATH; issue #470)

- **Status:** proposed 1 Nov 2026 by lane LARGEPATH. The plan (the measurements and pull request 1, items 1 to 3 below) was approved by the coordinator the same day; only the owner accepts this record (decision 0049).
- **Date:** 1 Nov 2026.
- **Lane:** LARGEPATH, `claude/lane-largepath`.
- **Issue:** #470 (the application is slow with very large toolpaths; the owner's carving with axial finishing at a 0.03 mm stepover).
- **Requirements:** UI-09 and NFR-04 (long work never blocks the interface), UI-11 (the tree's states), PRJ-08 (the unsaved-changes answer the recovery checkpoints and the close guard use), GC-07 and NFR-06 (output unchanged). NFR-12: generated G-code remains the user's to verify before running it on a machine.
- **Amends:** nothing in the documents. Decision record 0117 (the status keys) and 0040 item 8 (the 17 ms frame budget) are unchanged; this record makes the first cheaper to ask for, to keep the second.
- **Numbers:** takes 0631 and OI-1981 to OI-1984, the next free numbers across `main` and every unmerged branch on 2 Nov 2026 (the highest were 0630 and OI-1980), by the owner's rule of 1 Nov 2026 that numbers are as monotonic as possible. No project schema number; no library version change (only `openchuck-app` and `openchuck-ui` change, no public API).

## Context

The owner: "The slow down is during general use after generation of the axial finishing operation. It's not necessarily tied to a particular view or action."

Lanes CARVEPERF ([0292](0292-carving-finishing-speed.md)), SIMPERF ([0322](0322-simulate-playback-frame-cost.md)), SIMSPEED ([0332](0332-simulator-skips-sure-misses.md), [0333](0333-playback-information-with-generation.md)) made generation, the simulation and playback faster. This lane measured every remaining stage end to end on the owner's project (kept outside the repository, at the owner's request; only counts, sizes and times are recorded here): a 500,738-triangle STL model, three operations (rotary roughing, 15,174 moves; axial finishing with a 60 degree V-bit at a 0.03 mm stepover over a region of 496,183 triangles, 276,766 moves; helical finishing in rings over a region of 3,055 triangles, 856 moves), 4.89 MB of G-code in 292,566 lines, and 59,041 notes (7.4 MB; 58,000 of them per-move feed-limit notes, decision 0430).

Machine: a 4-vCPU cloud container (Intel Xeon), 16 GB, no graphics adapter; release builds; nothing else running. The interface was measured with the whole shell drawn headless, at 1366 by 768, frame by frame on the owner's project (a probe, not committed); the application's calls were timed directly through its public API.

## Findings

| Stage | Time on the owner's project | Thread |
| --- | --- | --- |
| Open the project | 0.28 s | interface |
| Generate (roughing 12 s, axial finishing 128 s, helical finishing 38 s) | 166 s, peak 537 MB | job |
| Generate again with nothing changed (the posting runs again) | 6.9 s, peak 737 MB | job |
| Simulate | 25 s | job |
| Write the output (prepare and write, from the export dialog) | 0.28 s | interface |
| Save (unchanged / to an empty folder) | 0.09 s / 0.07 s | interface |

The interface, frame by frame (median, p95):

| State | Before generating | After generating |
| --- | --- | --- |
| Nothing selected | 20 ms, 24 ms | 13 ms, 28 ms |
| An operation selected (any of the three) | 72 ms, 92 ms | 117 ms, 175 ms |
| The Results window open | | 90 ms, 125 ms |
| While generating (4,253 frames) | | mean 32 ms, worst 175 ms |

With an operation selected the interface drew about 8 frames a second, everywhere, which is the owner's report. The calls behind it, timed alone:

1. **The status keys, twice a frame.** The inspector asks `operation_path_is_current` and `operation_warning_settings` for the selected operation every frame (`apps/openchuck-ui/src/shell/inspector.rs`). Each made the status keys of every operation (`cache::status_keys`, decision 0117): two keys per operation, each hashing the operation's inputs, including its selection sets written out as text (`selection_inputs_hash`), about six times per call for the 496,183-triangle region. Each then copied the generation record, which holds every path warning of the run. 49 to 63 ms per call after generating.
2. **The unsaved-changes answer, twice a frame.** `is_modified` compares the whole project, selections included, with the saved one; the window title and the recovery checkpoints ask for it every frame. 5 to 6 ms per call.
3. **The Results window** groups all 59,041 notes every frame (`run_panel.rs`), and the export dialog checks the keys of the run's project and the current one every frame (52 ms) while it is open.
4. The tree's overview is built again for each job event, with the same keys, which is the cost while generating.

The viewport's drawing on a graphics adapter could not be measured here (OI-987, OI-1314 stay open; OI-1981).

## Decision

Items 1 to 3 are built (pull request 1). Each leaves every answer as it was; only how often it is made changes.

1. **The status keys are kept per revision** (`memo.rs`, `Application::status_keys`). The project changes only through `execute`, `undo` and `redo`, each of which moves the application's revision on, and the status keys read nothing but the project (they take the model files at their recorded hashes). So the keys are kept with the revision they were made at and made again only when it moves. The inspector's two questions and the overview's build use them. The pipeline's own keys (which hash the model files as read) are not changed.
2. **The generation record is read in place**, under its lock, by `operation_path_is_current` and `operation_warning_settings`, instead of being copied with every warning it holds.
3. **The unsaved-changes answer is kept per revision and per save.** A save changes the saved project without changing the revision, so the saves are counted too. A project recovered from a checkpoint is unsaved, as before, without a comparison.

Not built in pull request 1, planned with the coordinator:

4. **The Results window's lines and the simulate results panel's notes**, kept per run and revision (pull request 2, after lanes RESULTS-COPY, POLISH and RESULTS-DOCK, which change `run_panel.rs`; OI-1984).
5. **The export dialog's checks kept per revision, and writing the output as a job** with progress and cancel (UI-09) (pull request 2, after lane POLISH, which changes `export_panel.rs`; OI-1984).
6. **Only with the owner's go-ahead:** reusing the posted output when nothing changed (OI-1983); the 38 s helical finishing on a 3,055-triangle region (OI-1982); a cheaper hash of a selection (not needed with item 1, and it would change every key).

## Results

The same machine and project, pull request 1 against `main` `bf6bca2`:

Frames of the whole shell drawn headless (median, p95; one run of each):

| State | `main` | Pull request 1 |
| --- | --- | --- |
| Before generating, nothing selected | 20 ms, 24 ms | 0.4 ms, 0.5 ms |
| Before generating, an operation selected | 72 ms, 92 ms | 1.0 ms, 1.6 ms |
| After generating, nothing selected | 13 ms, 28 ms | 0.4 ms, 0.6 ms |
| After generating, an operation selected (each of the three) | 117 ms, 140 to 175 ms | 0.8 to 0.9 ms, 1.8 to 2.3 ms |
| After generating, the Simulate step | 15 ms, 17 ms | 3.8 ms, 5.8 ms |
| While generating | mean 32 ms, worst 175 ms | mean 1.0 ms, worst 107 ms |
| The Results window open (item 4, not built) | 90 ms, 125 ms | 82 ms, 133 ms |

`operation_path_is_current` and `operation_warning_settings` went from 49 to 63 ms a call to a lookup in the kept keys; `is_modified` from 5 to 6 ms a call to nothing between changes. The Results window is pull request 2 (OI-1984).

The G-code is byte-identical: SHA-256 `fb7f13fa07a4a812...` (first 8 bytes) on the owner's project before and after, and the reference jobs and golden files are unchanged (`openchuck-app`: 688 tests pass).

## Verification

- `req_ui_09_req_nfr_04_status_keys_are_made_once_per_project_change` (`crates/openchuck-app/src/memo.rs`): five frames' worth of the inspector's questions and the overview make the keys once, by a count of `status_keys` calls on the test's thread; a parameter change makes them once more. It failed before the change (25 makes for five frames).
- `req_ui_11_kept_status_keys_equal_fresh_keys_after_every_change`: after an addition, a parameter change, turning an operation off, two undos, a redo, a command that fails and a project recovered from a checkpoint, the kept keys equal keys made from the project.
- `req_nfr_04_req_prj_08_unsaved_changes_are_compared_once_per_change_and_save`: the answer equals a comparison with the saved project after changes, saves, an undo back to the saved state, a redo and a recovery, and five calls compare once. It failed before the change (five comparisons).
- `req_nfr_04_a_memo_makes_its_answer_once_per_stamp`, `req_nfr_04_a_poisoned_memo_makes_its_answer_again`.
- The existing tests of the item states, the inspector's warnings and the close guard pass unchanged; the reference jobs (`req_gc_07_reference_jobs_are_identical_at_1_and_n_threads`, `req_gc_09_req_gc_10_req_nfr_08_reference_jobs_pass_gates_1_and_2_and_match_their_golden_files`) pass.
- Coverage on the crate's own unit tests (`cargo llvm-cov nextest -p openchuck-app --lib`): `memo.rs` 100% of lines, and every changed line of `lib.rs`, `overview.rs` and `cache.rs` is run.
- The frame times above are from a headless probe of the whole shell on the owner's project, not committed (it needs the owner's project); no test asserts a time.

## Consequences and limits

- `crates/openchuck-app`: new private module `memo.rs`; `lib.rs` (`Application::status_keys`, `is_modified`, `save`); `overview.rs` (`operation_path_is_current`, `operation_warning_settings`, the overview's build); `cache.rs` (a test-only count). No public API change.
- Anything that changes the project must still go through `execute`, `undo` or `redo` (or open a new `Application`); a new way of changing the document without moving the revision would leave the kept answers stale. `req_ui_11_kept_status_keys_equal_fresh_keys_after_every_change` covers the ways there are today.
- The headless frame times include egui's layout and the shell's own work, not the GPU's drawing or upload; the owner's laptop is the measure for those (OI-1981).

## Open items

- OI-1981: frame times on the owner's laptop with this change, in general use after generating (drawing and upload included).
- OI-1982: helical finishing on a small region of a large model takes 38 s for 856 moves on the owner's project: profile it.
- OI-1983: generating again with nothing changed posts the whole program again (6.9 s on the owner's project); reuse it when the keys, the post and the grouping are unchanged, if the owner wants it.
- OI-1984: pull request 2: the Results window, the simulate results panel and the export dialog keep their answers per run and revision, and the output is written as a job.
