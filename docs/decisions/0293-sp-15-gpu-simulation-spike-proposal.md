# 0293: SP-15 proposal, GPU acceleration of the simulation stock

- **Status:** accepted by the project owner on 29 Oct 2026, in this session ("accept 0293"; decision 0049), and confirmed again in the Merge Manager session ("accept 0293 now"), relayed under decision 0049. Accepting it approves running spike SP-15 as described. It amends nothing yet: no requirement, architecture rule or accepted record changes unless the spike passes and a later record says so.
- **Date:** 29 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): opens OI-1221 to OI-1223.
- **Spike:** SP-15 in the work breakdown spike table. Its code goes on `spike/sp-15-gpu-simulation` (never merged); its write-up is `spikes/gpu-simulation/RESULT.md` on that branch. Neither exists yet.

Numbering: on 29 Oct 2026 the highest record on `main` and on every pushed branch was 0292 (`claude/lane-carveperf`) and the highest `OI-` number was OI-1220 (`claude/lane-docsuser`); this record takes 0293 and OI-1221 to OI-1223. Lane-owned ranges from the coordinator were not checked, so renumber if they clash.

## Context

Requirements: NFR-04 (multiple CPU cores, never block the UI), SIM-03, SIM-04, SIM-06, SIM-07 (a finer cell costs speed and memory), NFR-05 (double precision), NFR-06 and GC-07 (identical results at any thread count), NFR-02 (CI runners often have no GPU). Architecture sections 5.5, 8 and 12; accepted records [0022](0022-sp-6-simulation-stock.md) (dexels) and [0025](0025-slice-7-simulation.md) (the simulator and its budget).

Today only `openchuck-viewport` uses the GPU, and it only draws. Simulation and toolpath generation run on the caller's `rayon` pool. The question was whether the simulation could use the GPU. This record gives the measurements that justify a spike and the spike's design. It builds nothing.

## Profile of the current simulator

Method: the `req_sim_03_req_nfr_04_reference_job_meets_the_budget` job (record 0025, item 4: a 16.7 degree cone turned from Ø50 x 40 stock with a Ø4 ball, 260,995 sampled poses, 0.1 mm cell, so 400 x 500 = 200,000 columns). Release build (`opt-level` and debug info as the workspace sets them) on a 4-vCPU Intel Xeon at 2.1 GHz with no GPU. Callgrind on the first 300 moves gives the function split; temporary atomic counters (reverted, not committed) on the first 2,000 and 20,000 moves give the counts. The profiling harness was a throwaway test and is not kept.

**Time.** 75.6 s on one thread and 25.1 s on four (3.0 times, 75% efficiency); the stock hash is the same at both, and the budget (30 s and 120 s) holds. Record 0025 recorded 20.7 s and 70.4 s on its machine.

**Where the time goes** (instructions, first 300 moves; inclusive of callees):

| Part | Share |
| --- | --- |
| `cut_spans`, the exact tool span per column line, with its callees | about 77% |
| of which `hypot` (libm), called by the `f` closure through `height_at(x.hypot(y))` | 26% |
| of which `cut_spans` itself | 29% |
| of which `ToolSolid::height_at` | 17% |
| of which `root` (bisection) | 7% |
| The column loop: row and pose rejection, the line set-up, bounds | about 10% |
| `subtract`, `clone_from`, allocation and `memmove` | about 5% |

**Work per pose** (first 99,995 poses): 1,123 column lines reach `cut_spans` (112.3 million calls); 29% of them (32.2 million) return a span, and in every one of those the column changes. About 274 ns a call at one thread. Extrapolated to the whole job that is about 293 million column cuts, which agrees with 75.6 s (about 258 ns). Of the visits of a row to a pose, 10% pass the x slab; the row and pose test is not a cost.

**Two cheap CPU findings.** Replacing `x.hypot(y)` with `(x * x + y * y).sqrt()` in `span.rs` cut the 2,000-move run from 4.89 s to 3.46 s (29%). It changes the stock hash, so every golden that depends on the simulated stock needs regenerating; it is not part of this spike and was reverted. The 71% of calls that return no span suggests a tighter footprint test per pose could skip many of them; the cost of those empty calls was not measured separately.

**Not profiled:** `compare` (deviation), `stock_mesh`, `stock_at` replay for playback, and the 0.025 mm cell. Record 0025 item 5 puts time at about the cube of the cell ratio and memory at the square, so the finest setting is about 64 times the time (about 27 minutes on four threads for this job) and 16 times the memory (about 430 MB with checkpoints, over the 256 MB budget).

**Related, unmerged:** lane SIMPERF's proposed [decision 0322](0322-simulate-playback-frame-cost.md) (on `claude/lane-simperf` when this was written) measured the display side of the same simulator on the owner's carving project: stock replay from a checkpoint up to 18.5 s, and the pipeline's own simulation and comparison about 140 s of 250 s (its OI-1313, not split). Those figures are the case for the unprofiled parts in OI-1223, and they are not repeated here.

## Why the GPU is plausible

The work has the shape a GPU wants. Every column is cut alone, by the batch's poses in order (record 0025), with no reads of neighbouring columns: 200,000 independent threads at the default cell and 3.2 million at the finest. The per-column kernel is arithmetic with a short loop (golden section and bisection for convex tools, a march for others) and a short interval list. The CPU result already has a defined order per column, so a GPU kernel that takes one column per thread and loops over the batch's poses keeps that order.

## Why it is not simply an upgrade

1. **Determinism (NFR-06, GC-07).** The stock feeds later operations (SIM-06, the `StockModel` port) and so changes toolpaths and G-code. GPU floating point differs between drivers and vendors, so a GPU stock must never feed generation.
2. **Precision (NFR-05).** WGSL has no portable f64 (wgpu's optional float64 feature is native Vulkan and DX12 only, not Metal). A kernel would be f32, or double-single emulation.
3. **Architecture.** Only `openchuck-ui` and `openchuck-viewport` may use wgpu, and the library family may not. A GPU stock needs a home and an `xtask` allow-list entry, decided after the spike.
4. **Hardware.** The reference machine has integrated graphics (verification plan 4.2); the managed cloud and hosted CI have none. An integrated GPU may not beat four CPU cores by much.
5. **A fair baseline.** The two CPU findings above are cheap. A GPU must beat the CPU after those, not before.

## Proposed spike, SP-15

**Question.** Can a GPU kernel cut the dexel stock fast enough, and exactly enough, on the reference hardware to be worth a place in the product, for display and preview only?

**Scope (kept narrow).** The CPU `DexelStock` stays the authoritative stock for carry-over, deviation figures and everything that reaches G-code. The GPU is considered only as a faster way to produce the stock the user watches (SIM-03, SIM-04 playback) and a faster preview at fine cells (SIM-07). Variants:

- **A (the one to build):** one thread per column in a storage buffer of fixed-capacity interval lists (capacity from `MAX_SPANS`, with an overflow flag that makes the run fall back to the CPU); poses uploaded in batches as a tool-frame matrix and bounds prepared on the CPU in f64 and rounded to f32; the column line set up in the kernel relative to the pose to keep f32 error small.
- **B (measure only):** span solve on the GPU, subtract on the CPU. Expected to lose to read-back cost; one run to confirm.
- **C (note only):** a rendered heightfield or depth-peeled stock for display. It cannot give the exact stock the deviation needs, so only its cost is noted.

**Method.**

1. Baseline first: the budget job on the target machines on four threads, with and without the `hypot` change and with a footprint prefilter (spike-branch copies of `span.rs` and `dexel.rs`, not product changes).
2. Variant A and B in `spikes/gpu-simulation/`, a separate Cargo project with an empty `[workspace]` table, `wgpu` at the workspace pin (30.0.1), and `openchuck-sim` as a path dependency for the CPU reference and the shared fixtures.
3. Same job, same poses, same batch order. Run on lavapipe in the cloud for correctness only, then on the owner's RTX 2070 Super (OI-266) and Intel UHD 620 laptop (OI-263, OI-264), and on a verification plan 4.2 reference machine. Time with timestamp queries, and separately upload, kernel, and read-back.
4. At the 0.1, 0.05 and 0.025 mm cells.
5. The `Spans` of every tool shape of TL-03 (flat, ball, bull nose, tapered ball, V-bit, chamfer, drill and a custom profile), including a non-convex custom profile (the marched path), tilted by A.

**Pass criteria.** All must hold, or the spike fails.

1. Correctness against the CPU stock, per column: every interval endpoint within 0.001 mm (a hundredth of the default cell), and removed volume within 0.01%, for every tool shape above, on the reference job and on the analytic parts of record 0025 (cylinder, cone, sphere). Overflow columns reported, not silently wrong.
2. Speed on a reference-class integrated GPU (UHD 620 or the 4.2 machine): the reference job at 0.1 mm at least 4 times faster than the four-thread CPU baseline from step 1 *after* its cheap fixes, including upload and read-back of the stock mesh. If only the desktop GPU passes, the result is "optional, discrete GPUs only" and goes to the owner as a decision.
3. The finest cell (0.025 mm) runs the reference job within 5 minutes on that GPU, within the adapter's buffer limits (the default `max_storage_buffer_binding_size` is 128 MiB, so the stock may need splitting into bindings), or a limit is stated with the size at which it stops.
4. Behaviour the same on lavapipe, Vulkan, DX12 and Metal where available: no vendor-specific failure; results match across adapters to the same 0.001 mm.
5. The path to the product is stated: which crate holds the GPU stock, how it is kept out of the library family and out of generation (SIM-06 uses only the CPU stock), what the `xtask` allow-list needs, how a headless runner without a GPU is handled (NFR-02), and the added test and dependency cost (NFR-14: a bit-casting crate is already selected).

**If it fails.** Keep the simulator on the CPU. Make the CPU changes below the spike's baseline step as ordinary work: `hypot` removed (regenerate the goldens that depend on it), the pose footprint prefilter, a column-batched layout that vectorises, and a better thread split (the 3.0 times at four threads suggests room). The GPU question closes with a decision record; no GPU dependency enters the simulator.

**Timebox.** 5 working days plus owner hardware time: day 1 baseline and harness, days 2 and 3 variant A, day 4 measurements on the machines, day 5 write-up. A failed spike triggers the fallback, not a longer timebox (work breakdown, spike rules).

**Ownership of results.** Stock and deviation figures shown from a GPU stock are labelled as a preview in the UI until a record says otherwise. Generated G-code is the user's to verify before running (NFR-12); nothing in this spike changes that.

## Alternatives considered

- **GPU toolpath generation.** Not proposed. The strategies that fit (drop-cutter style height queries, scallop fields, batched gouge checks) feed G-code directly and so meet NFR-06 head on, and the remaining work (2D offsets, linking, ordering) is branchy. Revisit only if a profile shows a single hot sample loop, and only after this spike shows what a GPU's f32 gives.
- **CPU only, with SIMD.** Likely the safer first step; the spike's baseline step measures it. Candidate: batch the column lines for one pose in SIMD lanes (`wide` or `std::simd`), which the current scalar `cut_spans` does not use.
- **GPU collision queries (COL-05, P2).** The dexel `touches` test could use the same kernel; deferred until the stock result is known.
- **Accept the current speed.** The default cell meets its budget with margin; the case for a spike is the fine cell, playback at fine cells and slow machines, not the default job.

## Consequences

- Nothing in the product changes with this record.
- `docs/work-breakdown.md`: SP-15 row in the spike table, status "Proposed".
- `docs/open-items.md`: OI-1221 to OI-1223.
- If accepted and run: the spike branch, `RESULT.md`, a result record (next free number), and the architecture and verification plan edits that record agrees.

## Open items

| # | Item | Decide in |
| --- | --- | --- |
| OI-1221 | Run SP-15: build and measure it on the owner's RTX 2070 Super and UHD 620 laptop and a verification plan 4.2 machine, after the owner accepts this record. | SP-15 result record |
| OI-1222 | Whether to remove `hypot` from the span solver and add a pose footprint prefilter, whatever SP-15 finds. The first changes the simulated stock bits and so the goldens that depend on it. | A separate lane and record |
| OI-1223 | Profile the parts this record did not: `compare`, `stock_mesh`, `stock_at` replay for playback, and the 0.025 mm cell. | SP-15 baseline step |
