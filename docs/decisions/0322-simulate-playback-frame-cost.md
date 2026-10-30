# 0322: Simulate playback: what a cut-part frame costs, changes that show the same thing sooner, and a memory setting for playback checkpoints (lane SIMPERF; refs issue #382)

- **Status:** proposed 30 Oct 2026 by lane SIMPERF. Guidance only until the owner accepts it (decision 0049). Item 5 builds the owner's decision D1, relayed by the coordinator on 30 Oct 2026 (option C with a user setting); its cap at a share of the computer's memory waits on the owner (OI-1315). Owner decisions D2 to D5 are open (OI-174, OI-1311 to OI-1313).
- **What it amends:** [decision 0040](0040-slice-9-display-and-playback.md) item 11 (the stock at any move, from checkpoints): a stock kept from the last request, the checkpoint after an operation used for its end, and checkpoints spaced by a memory budget the user sets instead of about eight per operation (item 5). The UI specification, section 13 (per-user settings), gains the setting. It changes no displayed value, no stock, no picking and no playback behaviour. SIM-04, SIM-05, SIM-11, NFR-04 and NFR-06 are unchanged. No project schema change; no library-family API change.
- **Date:** 30 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): opens and closes OI-1310 (D1, decided by the owner); opens OI-1311 to OI-1315 (the owner's decisions D3 to D5, the measurement on the owner's project and laptop, and the memory cap); progresses OI-174 (D2).

Numbering: the coordinator gave lane SIMPERF records 0322 to 0326 and OI-1310 to OI-1329. On 30 Oct 2026 the highest record on `main` and on every pushed branch was 0312, and no `OI-131x` or `OI-132x` number was in use.

## Context

Issue #382: with a fine-stepover finishing (a V-bit with a 0.1 mm tip at a 0.03 mm stepover) the Simulate step takes minutes before the cut part appears, and during playback the tool seems not to move. Lane CARVEPERF ([decision 0292](0292-carving-finishing-speed.md)) profiled the owner's project and split the display side into this lane.

Since issue #333 ([decision 0040](0040-slice-9-display-and-playback.md), "Issue 333 correction") the tool is shown at the cursor of the cut part on screen until the next cut part is ready, so the time of one cut-part frame is how long the tool stands still.

### Measurement

`crates/openchuck-app/examples/playback_times.rs` times, through the public API: generate and simulate, prepare playback (the second simulation, with checkpoints), the surface job at seven jumps along the run (and, alone, the mesh and the picker on the stock it shows), and forward play as the viewport does it: a new surface is asked for at the clock's cursor when the last one is finished, eight frames from 30% of the program time at 10x, 100x, 1000x and "whole program in 60 s". Given an STL it builds the project around it in a temporary folder; given a project folder it opens it. It prints counts, sizes and times only.

Test piece: lane CARVEPERF's carving (`CarvedRelief::STANDARD` at 0.1 mm, written by its `carving_model` example, not committed), cylinder stock 28 mm by 34 mm, rotary roughing with a 3 mm ball, then axial finishing with a 60 degree V-bit with a 0.1 mm tip at a 0.03 mm stepover: 539,133 moves in 2 groups, 17,929 s of machining, 10 checkpoints, a stock copy of 3.8 MB, cut-part meshes of 0.76 to 1.07 million triangles. Machine: a 4-vCPU cloud container, release build, 4 threads, nothing else running, two runs of each build. There is no graphics adapter, so the upload and the drawing of a frame are not measured.

### Where a frame's time goes (`main`)

| Part of a cut-part frame | Test piece | Owner's project (CARVEPERF) |
| --- | --- | --- |
| Stock replayed from the nearest checkpoint (up to 1/8 of an operation) | 0 to 5 s | 0.01 to 18.5 s |
| Colours: the final comparison's nearest sample for each triangle, one thread | about 2 to 2.5 s | about 2 to 4 s |
| Picker (`MeshPicker::new`, a hierarchy over every triangle) | about 1 to 2 s | not split out |
| Stock mesh | 0.05 s | 0.05 to 0.1 s |

A frame with nothing to replay took 3.5 s. The last frame of the run replayed from the last checkpoint inside the operation, although a copy is kept after it (7.9 to 9.2 s). Before any of this, generating and simulating took 71 to 75 s and preparing playback 52 to 56 s (owner's project: 250 s and 131 s).

## Decision

1. **Colours are shared over the job's threads** (`SurfaceSamples::bands`). Each triangle's value depends on that triangle alone, so the triangles are split into fixed chunks of 4,096 and coloured in parallel, and the chunks are joined in order: the values are the same at any thread count (NFR-06). The cancel flag is looked at before each chunk, as before. About 2.2 s down to 0.85 s on the test piece.
2. **The stock of the last request is kept as a checkpoint** (`StockPlayback`; `Simulator::stock_at_from` in `openchuck-sim`). After each request the stock after its whole moves (before the swept part of the current move) is kept, one copy for the whole playback. The next request replays from it when it is nearer than every checkpoint before the point asked for; otherwise it is ignored. Replaying from any checkpoint gives the same stock as running up to the point (decision 0025 item 4), so forward play replays only the moves since the last frame and shows the same stock. The copy is counted: the checkpoint limit already counts one copy more than a setup keeps (`groups + 2 + moves / every`), and the kept stock is one copy of one setup's stock.
3. **The end of an operation is its checkpoint.** A request for the stock after the last move of a group uses the checkpoint after the group, and a request that lands on a checkpoint (or on the kept stock) is a copy of it, with nothing replayed.
5. **Checkpoints are spaced by a memory budget the user sets (the owner's decision D1).** "Memory for playback checkpoints (MB)" in Settings, an application preference kept in the user's settings (`SimulateSettings::playback_memory_mb`), never in the project (no schema change): 1,536 MB by default (the old limit), 256 to 8,192 MB in steps of 256; a value out of range or between steps is brought to the nearest step, and a settings file written before it existed reads the default. "Prepare playback" counts the copies that must be held: the start and after each operation in every setup, and two for the replay (the stock kept from the last request and the one being replayed). If they do not fit, it is refused with a message naming the setting and the simulation cell, as before (NFR-07). The rest is shared by checkpoints inside the operations, spaced evenly over all the moves to fill it (`checkpoint_spacing`) and never closer than 1,000 moves. The simulator counts each copy at its own size, which grows as the part is cut, and does not keep one that would pass the budget (`Simulator::with_checkpoint_budget`): fewer checkpoints, the same stock. A change applies the next time the cut part is prepared. On the owner's project (CARVEPERF's numbers: 290,548 moves, a copy of 7 MB) the default gives about 225 copies, about 1,300 moves apart, instead of 10; a jump then replays at most about 0.7 s there.
6. **Not done: the picker built beside the colours.** Running the picker and the colours at once (`rayon::join`) was tried and made frames slower (about 5 s instead of 2 s: each waited on work the other's thread held), so they run one after the other as before.

## Results

Two runs of each build; `main` against this branch:

| | `main` | This branch |
| --- | --- | --- |
| Frame with nothing to replay | 3.5 to 3.6 s | 1.6 to 2.0 s |
| Forward play at 10x (after the first frame) | 2.9 to 8.2 s | 2.0 to 2.5 s |
| Forward play at 100x | 2.6 to 8.7 s | 2.2 to 3.2 s |
| Forward play at 1000x, at the end of the run | 8.0 to 8.5 s | 1.9 to 2.3 s |
| "Whole program in 60 s" (about 300x; 50,000 to 80,000 moves a frame) | 2.9 to 8.8 s | 2.2 to 8.1 s |
| Jumps along the run | 3.5 to 9.2 s | 1.6 to 7.8 s |
| Last frame of the run | 7.9 to 9.2 s | 2.8 to 3.0 s |
| Peak memory | 1.47 GB | 1.56 GB (run-to-run variation of the same order; the kept copy is 3.8 MB) |

The first frame of forward play and a jump still replay from the fixed checkpoints (D1). At the highest speeds a frame replays as many moves as the clock passed during the last one, about 13,000 moves a second here and, from CARVEPERF's numbers, several times slower on the owner's project (D3).

## For the owner (not built)

- **D1, checkpoint spacing (OI-1310, closed).** Put to the owner on 30 Oct 2026 with the options 8 per operation, 512 MB (the lane's recommendation) or up to the 1.5 GiB limit. The owner chose the 1.5 GiB limit as the default, with a user setting to raise or lower it: item 5.
- **The budget on a computer with less memory (OI-1315).** The budget is a ceiling, not an allocation, but a budget above the computer's free memory makes the operating system page to disk or stop the program, which the program cannot turn into a clear error. Proposed to the owner: also cap the budget used at a quarter of the computer's physical memory, saying so in Settings when the cap applies (a few lines of platform code, no new dependency), or no cap, with Settings advising a value below the free memory.
- **D5, the picker built after the cut part is shown (OI-1311).** Building the picker only on the first hover would put 1 to 2 s on the interface thread, over the 17 ms limit of decision 0040 item 8, so it would have to be a background job started when the surface is shown; until it finishes, pointing at the stock shows no value. That is a display change. It would take a frame with nothing to replay from about 1.8 s to about 0.8 s.
- **D3, playback sync at high speed (OI-1312).** Issue #333 holds the tool at the cut part shown. At the speeds where a frame replays tens of thousands of moves, the tool still moves in jumps of several seconds.
- **D2, checkpoints kept by the pipeline's own simulation (OI-174).** Would remove "prepare playback" (52 to 56 s here, 131 s on the owner's project) at the cost of the memory of the checkpoints for every simulated run (decision 0025 item 7, decision 0040's alternative "checkpoints in the pipeline's simulation").
- **D4, the pipeline's simulation and comparison (OI-1313).** About 140 s of the 250 s on the owner's project; a separate measurement.
- **The owner's project and laptop (OI-1314).** The numbers above are from a container without a graphics adapter; the frame's upload and drawing, and the times on the owner's laptop, are not measured.

## Tests

- `req_sim_04_req_nfr_07_checkpoints_are_spaced_to_fill_the_memory_budget` (`stockplay.rs`): on the owner's project's numbers, about 1,300 moves apart at 1,536 MB and about 4,000 at 512 MB, the 1,000-move floor at 8,192 MB, never further apart with more memory, the copies within the budget, none inside when there is no room. `req_nfr_04_the_checkpoint_limit_counts_every_setup_together`: the refusal counts every setup and names the setting and the cell.
- `req_nfr_07_req_sim_04_checkpoints_never_pass_their_memory_budget` (`openchuck-sim/tests/simulation.rs`): with a budget for about a third of the checkpoints, the copies held never pass it at their real sizes, the start is kept, and the stock at every point is the same; a budget that holds them all keeps them all.
- `req_nfr_07_req_sim_04_playback_never_holds_more_than_its_memory` (`openchuck-app/tests/playback.rs`): a playback with room for one checkpoint more than it must keep holds fewer than the default, never more memory, and shows the same stock; one with too little is refused naming the setting.
- `req_ui_15_req_nfr_07_the_playback_checkpoint_memory_is_personal_and_bounded` (`openchuck-app/tests/settings.rs`) and `req_ui_15_req_nfr_07_settings_sets_the_memory_for_playback_checkpoints` (`openchuck-ui`): the default, an older settings file, the steps and range, the Settings dialog, playback given the value, and a restart.

- `req_sim_05_req_nfr_06_stock_colours_are_the_same_at_any_thread_count` (`stock_surface.rs`): 20,000 triangles over 3,601 samples with a tie and triangles out of reach give, at 1, 2 and 4 threads, the bands of the one-triangle-at-a-time rule. Fails when the chunks are joined out of order.
- `req_sim_04_req_nfr_04_jumping_from_a_kept_stock_matches_running_up_to_it` (`openchuck-sim/tests/simulation.rs`): from a kept stock, across the end of a group and within one, the stock equals running up to the point; a kept stock after the point is not used; at a kept point, and at the end of each group, the stock is the copy, with nothing replayed (a cancelled job still gets it).
- `req_sim_04_req_nfr_06_the_stock_at_a_move_is_the_same_whatever_was_shown_before` (`openchuck-app/tests/playback.rs`): playing forward, stepping back, jumping and playing on, every stock equals the one replayed from the checkpoints alone. Fails when the kept stock is labelled with the wrong move.
- The existing playback, stock surface and picking tests are unchanged and pass.

## Consequences

- `openchuck-sim`: `Simulator::stock_at_from`, `with_checkpoint_budget` and `checkpoint_bytes` (new, public; `stock_at` calls `stock_at_from`). `openchuck-sim` is not in the library family; no version bump.
- `openchuck-app`: `stock_surface.rs` (parallel colours), `stockplay.rs` (the kept stock and the budget; `MAX_CHECKPOINT_BYTES` becomes `DEFAULT_PLAYBACK_MEMORY`, and `Application::start_stock_playback` takes the memory), `settings.rs` (`playback_memory_mb`, `PLAYBACK_MEMORY_MB`), the example `playback_times` (an optional memory argument).
- `openchuck-ui`: the Settings dialog's new line, and the playback panel passes the setting on. No change to the Simulate footer, the step bar, the export dialog, generation, or what is displayed.
- UI specification section 13: the setting.
