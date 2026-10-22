# 0195: Axial finishing fits its passes in parallel, entry moves stop rescanning the stock, and the progress of a run counts operations (lane BUG-TABDEV; issues 252 and 248)

- **Status:** accepted (22 Oct 2026) by the project owner, in the lane's own session.
- **What it amends:** nothing in the documents. It refines how decisions 0012 and 0015 item 4 (deterministic parallel work: each item computed alone, results in input order) are applied to the passes of axial and indexed finishing, and how the pipeline of [decision 0190](0190-cached-stock-per-operation.md) reports progress. The progress across all stages as one percentage stays with lane BUG-PROG (issue 201). No requirement changes.
- **Date:** 21 Oct 2026.
- **Open items:** OI-741 (the owner re-measures on the reference laptop) is new.

## Context

Trial feedback F-82 (issue 252): axial finishing took about 20 minutes on the owner's `blisk_glitchy_generating` project (Intel Core i5-8265U, 4 cores and 8 threads), with every core busy and the graphics loaded too; roughing and helical finishing were not slow. Trial feedback F-78 (issue 248): while that operation generated, the progress text jumped between "0 of 1", "0 of 10" and "0", in the middle of the run. Requirements: NFR-04 (generation uses the cores and never blocks the interface), UI-09 (progress and cancellation), GC-07 and NFR-06 (identical output at any thread count).

The lane measured first, with a new example, `crates/openchuck-app/examples/operation_times.rs` (each operation through "generate selected", its time, moves, a hash of its toolpath, its progress reports and how often the total shown changed). On a 4-core Xeon at 2.1 GHz (a cloud container, not the owner's laptop), release build, before the change: roughing 4.6 s, helical finishing 29.7 s, axial finishing 449.9 s (153,823 moves), and the total shown changed 2,206 times in the axial run. Sampling the stacks with a debugger showed one of the four workers busy during axial finishing, and two causes:

1. **The passes ran one after another, each on one thread.** The shared parallel map works in fixed chunks of 64 items. An axial pass on this part has about 50 seeds (50 mm at the default 1 mm step) and about 50 intervals to refine, so each pass is one chunk on one thread, and the 1,048 passes (0.12 mm round a radius of 21 mm) ran in turn.
2. **Every entry move read the whole stock several times over.** A plunge or entry into the stock the earlier operations left casts 1 + rings x rays slanted rays at it, and for each ray `DexelStock::first_material` asked for `DexelStock::bounds`, which reads all of the stock's columns (about 250,000 at the 0.1 mm cell). This was about two thirds of the time on one thread.

The progress: the job showed its latest report, and the strategy and the stock simulation report their own stages, each with its own count ("dropping the tool 0 of 50", "fitting the path 0 of 10", the simulated moves), on the same job as the run's count of operations. The text and the tree's progress bar showed whichever came last. The flicker belongs to the axial finishing strategy (the operation the owner named "Rest machining"), not to the rest machining strategy.

The graphics: while a job runs, the run panel asked for a repaint every frame, so the whole window was drawn again at the display's rate for the whole of the generation.

## Decision

1. **Axial and indexed finishing fit their passes in parallel, one pass to a thread** (`AlongX::passes_at`, with a new `parallel::map_each_in_order` in `openchuck-path`). Each pass is fitted alone and the passes are joined in pass order, so the path cannot depend on the thread count (NFR-06); an error is the first in pass order. The fixed chunks of `map_in_order` stay for the many small items they suit.
2. **The stock's extent is kept between rays.** `DexelStock` keeps the material's Z extent once worked out, and drops it whenever the stock is cut, so `bounds` reads the columns once per cut, not once per ray. The value is the same, bit for bit, and two equal stocks still compare equal.
3. **A run's progress counts operations.** The pipeline gives each operation a progress sink of its own (`OperationProgress`) that passes each of the operation's own reports on as the run's count ("2 of 3"), the operation still being worked on, so the text shows the run is alive and its total never changes or goes back while the toolpaths are made. The comparison with the part after them is counted the same way, by setups, in its own stage (`STAGE_SIMULATE`): reading the model and comparing report as the run's count of setups. `OperationProgress` is where lane BUG-PROG adds the operation's own share, from the report it now drops (issue 201). The passes of axial and indexed finishing report one count of passes, with one total.
4. **While a job runs the window is drawn again ten times a second**, not every frame, for its progress text and to see the end of the job. Moving the view or any other input still draws at once.

## Alternatives considered

- **A smaller fixed chunk for every parallel map.** Every strategy's output would stay the same, but the many small items (the seeds of a long helix) would pay for more scheduling; only the passes needed it.
- **Fit the seeds of all passes in one parallel map.** The fitting of a pass reads its own seeds in order; fitting whole passes keeps each as it was.
- **Keep the extent up to date during the cut.** A cut is made in parallel rows; working it out again on the next use is simpler and costs one read of the columns per cut.
- **Show the strategy's own stage text in the footer.** It is what made the text jump; one count across stages is issue 201's work.

## Consequences

- On the owner's project (same container, release build): axial finishing 40.2 s on 4 threads and 145.2 s on 1 thread (3.6 times), from 449.9 s; helical finishing 25.4 s on 4 threads and 84.9 s on 1; roughing about the same. The toolpath hashes of all three operations are the same as before the change and at 1 and 4 threads, so the G-code is unchanged (GC-07, NFR-06). The shown total no longer changes during the run. The stock simulation and the comparison with the part (165 s on this project, at the default cell) were outside what the owner timed and are unchanged.
- The owner's laptop is slower per core than this container, and drawing could not be measured here (no graphics adapter); OI-741 asks the owner to time the project again.
- Changelog: `openchuck-path` gains `parallel::map_each_in_order` (an addition; the library family's version is not bumped, decision 0048). `openchuck-sim`: none public (the cached extent is private).

## Tests

- `req_nfr_04_axial_finishing_fits_its_passes_on_more_than_one_thread`, `req_nfr_06_axial_finishing_is_identical_at_1_2_and_n_threads` and `req_ui_09_axial_finishing_counts_its_passes_with_one_total_that_only_goes_up` (`crates/openchuck-path/tests/axial.rs`); the first and third failed before the change. `req_nfr_06_indexed_finishing_is_identical_at_1_2_and_n_threads` (`crates/openchuck-path/tests/indexed.rs`).
- `req_sim_03_the_stock_extent_follows_each_cut` (`crates/openchuck-sim/tests/simulation.rs`).
- `pipeline::tests::req_ui_09_the_progress_counts_operations_with_one_total_that_only_goes_up` (`crates/openchuck-app/src/pipeline.rs`); it failed before the change.
- `run_panel::tests::req_ui_09_req_nfr_04_a_running_job_redraws_at_a_steady_pace` (`apps/openchuck-ui/src/run_panel.rs`); it failed before the change.
