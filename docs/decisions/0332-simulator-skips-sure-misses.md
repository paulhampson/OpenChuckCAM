# 0332: The stock simulator skips the lines that surely miss the tool (lane SIMSPEED; refs issue #382)

- **Status:** accepted (31 Oct 2026) by the project owner, who read the condensed record in the coordinator session and approved it. The acceptance was relayed to lane SIMSPEED by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7), as decision 0049 allows. Proposed the same day by lane SIMSPEED. The plan was sent to the coordinator session before the code was written; the coordinator answered that no owner decision is needed, because the change only skips work that provably does nothing and keeps the stock bit for bit, and set the checks listed under Tests. It builds the owner's decision D4 on lane SIMPERF ([decision 0322](0322-simulate-playback-frame-cost.md), OI-1313, closed).
- **What it amends:** [decision 0025](0025-slice-7-simulation.md) item 2 (the span solver): before the golden-section search or the march below `H`, a line is tested against a bound and, when the bound shows it is outside the tool, gives no span without the search (item 1 below). It changes no span, no stock, no removed volume and no G-code. Decision 0025 item 4 is unchanged: the same columns take the same poses, in the same order, in the same batches. SIM-03, SIM-04, SIM-06, NFR-04 and NFR-06 are unchanged. No project schema change; no public API change (`openchuck-sim` is not in the library family; no version bump).
- **Date:** 31 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): opens OI-1320 (the measurement on the owner's project); progresses OI-174 (D2, put to the owner again with the numbers below) and OI-1222 (decision 0293: this is an exact form of its pose prefilter; replacing `hypot` stays open).

Numbering: the coordinator gave lane SIMSPEED records 0332 to 0336 and OI-1320 to OI-1339. On 31 Oct 2026 no record from 0332 to 0337 and no `OI-132x` or `OI-133x` number was in use on `main` or on any pushed branch (lane ENDFACE has 0343 to 0345, lane STEPALL-X3 0338).

## Context

Issue #382: on the owner's carving project the pipeline's simulation took 96 s (92.6 s of it the V-bit finishing's 287,000 moves), and preparing playback runs the same simulation again with checkpoints (97 s); every cut-part frame replays moves from a checkpoint ([decision 0322](0322-simulate-playback-frame-cost.md)). A faster `Simulator::run_group` shortens all three. The owner asked for the same stock bit for bit at any thread count, because later operations are generated against it (decision 0025 item 7, SIM-06).

## Measurement

`crates/openchuck-app/examples/sim_times.rs` (new) generates a project once, then simulates each group of its toolpath again from the setup's stock at each thread count asked for, on a rayon pool of that size, and prints each group's time, the stock's state hash, and the SHA-256 of each G-code file. Given an STL it builds the carving project of `playback_times` (the builder moved to `examples/support/carving.rs`, shared by both).

Test piece: `CarvedRelief::STANDARD` at 0.1 mm (`openchuck-benches`' `carving_model`, not committed), cylinder stock 28 mm by 34 mm, rotary roughing with a 3 mm ball (12,217 moves, 127,526 poses), then axial finishing with a 60 degree V-bit with a 0.1 mm tip at a 0.03 mm stepover (526,216 moves, 2,789,636 poses); cell 0.1 mm. Machine: a 4-core cloud container (Xeon at 2.1 GHz, one thread per core), release build, nothing else running unless said.

### Where the time went

The finishing's simulation, alone, before the change: 206.8 s at 1 thread, 107.9 s at 2 and 55.3 s at 4 (3.7 times at 4 threads). The simulator already uses the threads well: rows of columns have work in each batch. The stock hash was the same at every count.

A profile (`perf`, 4 threads, the whole example): 89% of the time in the swept-tool cut itself, `span::cut_spans` with `ToolSolid::height_at` and `hypot`; visiting the columns (the row loop) about 3%; allocation and span merging under 0.5%. Counters (temporary, not committed) on the finishing: 690 million calls of `cut_spans`, 685 million (99.3%) giving no span, and 2.8 million changing a column. A column inside a pose's box, holding material in the box's height, is searched; near a fine V-bit most of them lie under the tool's cone, and the solver proved each miss with a golden-section search of about 40 evaluations of `h(r)`.

## Decision

1. **A sure-miss test before the search.** On the part of the line below `H` (decision 0025 item 2), `f(s) = w(s) - h(r(s))` is at most `max(w(a), w(b)) - h(r_min)`: `w` is linear in `s`, so it is largest at an end, and `h` never decreases with `r` (the monotone rule every `RevolvedProfile` passes), so it is smallest at the line's closest approach to the axis, `r_min`. When that bound is below `-1e-9` mm, every value of `f` on the line is negative, so the search would find no span: `cut_spans` returns no span at once. The margin is far above the rounding of the bound and of the search's own evaluations (about 1e-14 mm at the sizes of a tool), so a skipped line is one the search finds empty; any other line is searched exactly as before, by the same arithmetic. The test is in `span.rs` (`surely_misses`), for convex and non-convex profiles alike; a line along the axis, or one whose values are not finite, is never skipped.
2. **Only this.** Batching (32,768 poses), the rows as tasks, the order in which a column takes the poses, the subtraction, and the sum of the removed volume are unchanged. A finer split of the rows was not needed (the threads were already in use) and would change the order of the removed-volume sum.
3. **Not done.** A tighter box per pose, or skipping columns before the line is set up, would save part of the 690 million cheap calls that remain (after this change `cut_spans`, with its clipping and the bound, is about a third of the profile); measured gains would be smaller, and it can follow if a new profile shows it.

## Results

Same container, the finishing group (526,216 moves):

| Threads | Before | After | Gain |
| --- | --- | --- | --- |
| 1 | 206.8 s | 49.5 s | 4.2 times |
| 2 | 107.9 s | 26.7 s | 4.0 times |
| 4 | 55.3 s | 14.4 s | 3.8 times |

Both groups at 4 threads: 63.5 s before, 19.2 s after (roughing 8.3 s to 4.8 s). Stock state hashes after each group are the same before and after the change and at 1, 2 and 4 threads (`7821f596512d5a28` after roughing, `5fbec8b2197d3968` after finishing).

`playback_times` (generate and simulate, then preparing playback, the simulation again with checkpoints at the default 1,536 MB). The "before" runs at 4 and 2 threads are those of `main`'s code with nothing else running; the 1-thread run is from the first series:

| Threads | Generate and simulate, before | after | Prepare playback, before | after |
| --- | --- | --- | --- | --- |
| 1 | 281.6 s | 119.1 s | 226.1 s | 72.9 s |
| 2 | 155.2 s | 68.5 s | 135.0 s | 44.6 s |
| 4 | 81.2 s | 37.4 s | 71.0 s | 27.9 s |

The first series of "before" runs gave 125.0 s at 2 threads and 138.8 s at 4 for preparing playback, so 4 threads looked no faster than 2 (the coordinator asked whether the application's job pool caps it). It does not: the 4-thread run had shared the computer with a `cargo install` started at the same time, and run again with nothing else running it took 71.0 s; the simulation alone scales 3.7 times at 4 threads. No change to the job pool is needed.

Both runs keep 541 checkpoints holding 1,194 MB, with a peak of about 2.2 GB. The G-code of the carving project, whose finishing is generated against the stock the roughing left (carry-over, SIM-06), has the same SHA-256 before and after the change (one file of 11,889,155 bytes with the Makera Z1 post, `c607e4da903bcc21c6d4accfd1ac3669e58830fcfcb1a8df1a839ae583f23bac`).

### The owner's project (OI-1320)

The owner's project is kept outside the repository. Expected from the test piece's gain: the finishing's simulation from 92.6 s to about 25 s at 4 threads, and preparing playback from 97 s to about 25 to 35 s (on the test piece, preparing playback now spends about 9 s outside the simulation, on the checkpoints); not measured. To measure it, through the coordinator: `cargo run --release -p openchuck-app --example sim_times -- PROJECT 4,2,1` on `main` before and after this change (each group's time and stock hash, and the G-code hashes, which must match), and `playback_times -- PROJECT 4` for preparing playback. Only counts, times and hashes are printed.

## Tests

- `req_sim_03_req_nfr_06_skipping_sure_misses_changes_no_span` (`span.rs`): 30,000 lines for each of eight profiles (ball, V-bit 60 degrees with a 0.1 mm tip, V-bit 20 degrees sharp, flat, bull-nose, tapered ball, a concave arc and a reflex joint): at random near the tool, a hair (1e-13 to 1e-3 mm) under the surface, along the axis and tilted 1e-10 to 1e-2 from it, through or next to the axis, and on empty ranges. The spans with the skip equal those of the search alone, bit for bit; some lines of every profile are skipped. With the margin set to -1e-4 mm it fails. 0.23 s uninstrumented.
- `req_nfr_07_req_nfr_06_a_line_that_is_not_finite_is_no_crash_either_way` (`span.rs`): a not-a-number or infinite value in any coordinate, direction or end gives the same spans with and without the skip, and no crash. 0.01 s.
- `req_sim_03_a_line_under_the_tip_surely_misses_and_a_touching_one_does_not` (`span.rs`): a line under a V-bit's tip is a sure miss; one that goes in, or passes under it closer than the margin, is not. 0.01 s.
- `req_sim_03_req_nfr_06_a_carving_finishing_cuts_the_same_stock_as_the_search_alone` (`dexel.rs`): a V-bit finishing (60 degrees, 0.1 mm tip, passes a hundredth of a radian apart round a relief, on a stock roughed by a ball) cuts the same stock (state hash and every column) with the same removed volume, bit for bit, at 1, 2 and 4 threads as the search alone does (a test-only switch on one thread). With the margin set to -1e-4 mm it fails. 4.0 s uninstrumented.
- The existing tests pass unchanged: `req_nfr_06_simulation_is_identical_at_1_2_and_n_threads`, `req_sim_04_req_nfr_04_jumping_from_a_kept_stock_matches_running_up_to_it`, `req_sim_04_req_nfr_07_a_packed_stock_unpacks_to_the_same_stock`, the simulation's accuracy tests against the brute-force run, and in `openchuck-app` `req_gc_07_reference_jobs_are_identical_at_1_and_n_threads`, the golden files, `req_sim_06_the_second_operation_is_generated_against_the_stock_the_first_left`, `req_gc_07_req_nfr_06_carried_stock_gives_the_same_toolpath_at_any_thread_count` and the playback tests.

## Consequences

- `openchuck-sim` `span.rs`: `surely_misses` and `MISS_MARGIN` (private); `cut_spans` calls the solver with the skip on (a test-only switch turns it off on one thread). No public change.
- `openchuck-app` examples: `sim_times` (new); `playback_times` and `sim_times` share the carving project's builder in `examples/support/carving.rs`.
- No change to the user interface, the project file, the G-code or any document other than the registers. The speed-up applies to the pipeline's simulation, to preparing playback and to every cut-part frame's replay.
- D2 (OI-174): put to the owner again through the coordinator with these numbers. The owner chose B with a setting (relayed, 31 Oct 2026): "Generate playback information with toolpath generation", built in a second pull request after this one (decision 0333). OI-174 stays open until it merges.
