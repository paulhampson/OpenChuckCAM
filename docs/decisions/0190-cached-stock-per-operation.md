# 0190: The pipeline cache keeps the stock each operation leaves (lane BUG-REGEN; issue 239)

- **Status:** proposed (21 Oct 2026). Only the owner accepts this record, directly in the lane's session (decision 0049). Item 3 and the status text of item 5 are the owner's answers to OI-626, relayed by the coordinator on 21 Oct 2026.
- **What it amends:** [decision 0030](0030-slice-9-pipeline-cache.md) item 5 ("Simulation is not cached") and its consequence on OI-132. Items 1 to 4 and 6 of 0030 are unchanged: the chained key is still the only cache key, and it now also keys the stock. Architecture section 6 already says "Generate and simulate write to the cache", so the architecture is not changed. No requirement changes.
- **Date:** 21 Oct 2026.
- **Open items:** OI-626 (the number of stocks kept) is new and closed by the owner's answer (item 3). OI-627 (the deviation is compared again when nothing changed) is new. OI-132 is progressed.

## Context

Trial feedback F-73 (issue 239): with a roughing followed by a 4-axis finishing operation, changing the finishing seemed to regenerate the roughing too. Requirements: NFR-04 (generation and simulation do not block, and use the cores), SIM-06 (an operation is generated on the stock the earlier ones left), GC-07 and NFR-06 (deterministic output), UI-11 (item states). Records read: 0025 (simulation in the pipeline), 0030 (the pipeline cache), 0117 (the item states), 0172 (what is a cache input).

The lane reproduced it with the owner's `blisk_1_overcut_mismatch` project (roughing and helical finishing, release build, 4 threads). The cache keys were right: the roughing's key does not change when the finishing changes, and the run reported the roughing as reused, with a byte-identical path. But the roughing still cost most of its time. Decision 0030 item 5 left the stock simulation out of the cache, so the pipeline ran the roughing's whole toolpath through the dexel simulator again on every run, to give the finishing its stock (SIM-06). A second run with nothing changed took 10.6 s against 32.5 s for the first, all of it the roughing's simulation. A change to the finishing took 13.9 s. During that replay the project tree showed the roughing as "Running: generating", which is what the owner saw.

## Decision

1. **The stock an operation leaves is cached with its path.** When a run simulates an operation, the cache keeps a copy of its setup's simulation just after it (the dexel stock and the per-operation figures) beside the operation's path, under the operation's key. The key chains every operation before it and covers the setup, the machine and the simulation settings (decision 0030 item 1), so it fixes that stock too. A later run that reuses the operation takes its stock from the cache and does not simulate it again. Simulation starts again from the first operation whose stock is not cached. A stock is kept only beside a cached path, and only when the run needed it: a later operation in the same setup is generated on it, or a simulation was asked for. A run that needed neither keeps none, and the next run that needs it simulates it once and keeps it.
2. **Copied only when needed.** The run shares the cached state with the cache and copies it only when it has to simulate on from it (copy on write), so a chain of reused operations copies nothing.
3. **One stock per operation, the latest (OI-626: the owner chose option B).** Whatever the number of results kept per operation (`Application::set_cache_versions`, default 3), the cache keeps the stock of only the result stored last, and keeping a new stock for an operation drops the one it had. Memory is therefore bounded to one stock per operation whose stock is needed: a blisk stock at the default 0.1 mm cell is about 21 MB. An undo, or a parameter toggled back, reuses the cached paths, but the stock they leave is worked out again. The options not chosen were A (a stock with each cached result, up to 3 per operation, so an undo stays instant) and C (nothing cached, which would not fix issue 239).
4. **Output is unchanged.** A stock taken from the cache is the state the same inputs give (NFR-06), and the notes of rapid moves that cut material come from it in the same order. The report's names are taken from the project as it is now, because names are not cache inputs (issue 209, decision 0172). The report's memory figure counts the vectors' capacities, which differ between a stock that was cut and a copy of it, so the stock in the report is shrunk to its contents (`DexelStock::shrink_to_fit`, new in `openchuck-sim`). The figure is then the same either way, and the report holds less memory.
5. **Only work shows as running, and a replay says so (the owner's addition to item 3).** An operation shows as running in the tree only while it is made or simulated, not while the run passes it by because it was taken from the cache. While an operation whose path came from the cache is run through the simulation again (its stock was not cached, or was dropped by item 3), the tree says "Running: re-running the simulation". The status bar adds "Re-running the simulation for '<operation>'" after the job and its progress, for example "Simulating… 1 of 2: Re-running the simulation for 'Roughing'"; while an operation is made, it adds "Generating '<operation>'". The application gives the text (`Application::pipeline_activity`, from `PipelineActivity`) apart from the progress counts, so the single progress figure of issue 201 (lane BUG-PROG) can replace the counts without touching it. Wording follows decision 0114. A failure is still attributed to the operation the run had reached (decision 0117).
6. **`Regeneration::simulated`** lists the operations run through the stock simulation in that run, beside `generated` and `reused`, so the UI and the tests can see what was done.

## Alternatives considered

- **Cache the setup's simulation checkpoints for playback as well (OI-132):** not needed for issue 239, and playback keeps its own checkpoints (decision 0040 item 11).
- **Separate stock cache keyed per setup:** rejected; the operation's chained key already fixes the stock, so a second key would only have to be kept in step with the first.
- **A key that leaves out a later operation's parameters on purpose:** not needed. The owner saw the roughing regenerate after a parameter change on the finishing. A parameter edit replaces only its own operation (`Command::SetParameter`), and a sweep over every parameter of every built-in strategy, set on the later operation, leaves the earlier operation's keys unchanged. What the owner saw was the replay of item 1.
- **Report the memory figure from the lengths, not the capacities:** rejected; the figure says what the stock holds, and shrinking it makes that true and the same either way.

## Consequences

- Measured on the owner's blisk project (release build, 4 threads), before and after: a run with nothing changed went from 10.6 s to 0.06 s; after a change to the finishing, from 13.9 s to 4.2 s (the finishing's own generation); with Simulate, from 14.0 s to 1.2 s and from 17.0 s to 6.6 s. A first run is unchanged (32 to 39 s).
- A Simulate run with nothing changed still compares the final stock with the part again (about 1 s of the 1.2 s on the blisk project); caching the comparison is OI-627.
- Memory: at most one cached stock per operation (item 3) beside the paths decision 0030 keeps.
- Changelog: `openchuck-app` (`Regeneration::simulated`, `Application::pipeline_activity`, `overview::PipelineActivity`) and `openchuck-sim` (`DexelStock::shrink_to_fit`) gain public items. Neither is in the library family that `cargo-semver-checks` checks, and nothing is removed, so the workspace version is not bumped (decision 0048).

## Tests

In `crates/openchuck-app/tests/operation.rs` (ring then rest machining, which is generated on the ring's stock):

- `req_nfr_04_req_sim_06_editing_a_later_operation_reuses_the_earlier_one_and_its_stock`: changing the second operation reuses the first and does not simulate it again.
- `req_nfr_04_req_sim_06_editing_an_earlier_operation_makes_and_simulates_the_later_one_again`: a change upstream still makes and simulates the downstream operation.
- `req_nfr_04_a_cached_operation_whose_stock_was_not_kept_is_simulated_once_then_reused`.
- `req_gc_07_req_nfr_06_reused_paths_and_stock_give_byte_identical_output`: the G-code, notes, toolpath and simulation report are the same as with the cache off.
- `req_gc_07_a_rename_after_reuse_names_the_operations_as_they_are_now`.

In `crates/openchuck-app/src` (unit tests): `cache::tests::req_nfr_04_no_parameter_of_a_later_operation_reaches_an_earlier_operations_key` (the parameter sweep), and `pipeline::tests::req_nfr_04_req_ui_09_a_replayed_simulation_says_so_and_one_stock_is_kept_per_operation` (item 3's bound, and the replay text read from the record while the simulation reports progress).

Also `req_ui_11_req_nfr_04_an_operation_taken_from_the_cache_is_never_shown_running` in `tests/overview.rs`, and `req_nfr_06_a_shrunk_stock_holds_the_same_material_and_memory_as_a_copy` in `crates/openchuck-sim/tests/simulation.rs`.
