# 0292: Fine finishing of a carving: where the time goes, and four speed-ups that leave the output unchanged (lane CARVEPERF; refs issue #382)

- **Status:** accepted by the project owner on 30 Oct 2026, relayed by the coordinator session (decision 0049): the owner saw the condensed record in the coordinator session and approved it. Proposed 29 Oct 2026 by lane CARVEPERF.
- **What it amends:** nothing in the documents. It adds rules for skipping work in the mesh drop-cutter (decision record 0012 items 4 and 5), the composite target (decision record 0035 items 1 and 2) and a region's candidate intervals (decision record 0034 item 5). Each rule leaves every answer the same, bit for bit, ties included (decision record 0113). NFR-04, NFR-06 and GC-07 are unchanged, and so are every tolerance, sampling step and toolpath. No project schema change.
- **Date:** 29 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): opens OI-1180 (the owner re-times the carving on the laptop), OI-1181 (pencil clean-up on an STL part runs on one thread) and OI-1182 (a region's passes are seeded at the tolerance: a choice for the owner, not built).

Numbering: the coordinator gave lane CARVEPERF records 0292 to 0296 and OI-1180 to OI-1199. On 29 Oct 2026 the highest record on `main` and on every pushed branch was 0282, and no `OI-118x` or `OI-119x` number was in use.

## Context

Issue #382: "Axial finishing (and potentially other strategies) take a long time to generate for fine stepovers (e.g., 0.03 with a 0.1 V bit) despite multithreading." Requirements: NFR-04 (generation uses the cores), NFR-06 and GC-07 (the same output at any thread count, byte for byte). [Decision 0195](0195-axial-finishing-speed-and-steady-progress.md) already fits the passes of axial finishing one to a thread.

The lane timed two jobs:

- **The owner's carving project**, handed to the lane session by the owner and kept out of the repository at the owner's request (only counts, settings and timings are recorded here). It has a 500,738-triangle STL model and two operations: rotary roughing, then axial finishing with a 60 degree V-bit with a 0.1 mm tip at a 0.03 mm stepover over a selected region of 496,172 triangles. It is on box stock with a held region and a 10 mm tab, and the finishing makes 276,766 moves.
- **The lane's own test piece**, `openchuck_testkit::parts::CarvedRelief::STANDARD`: a 24 mm by 30 mm cylinder with raised strokes 0.8 mm high (diagonals, rings and crosses on 5 mm cells, chosen by a fixed hash) with chamfered walls and sharp edges, over a gentle wave. At 0.1 mm vertex spacing it has 453,908 triangles. `benches/examples/carving_model.rs` writes it as STL (22 MB, not committed) for a project, and `crates/openchuck-path/examples/carving_profile.rs` times a strategy on it through the public API.

Machine: a 4-vCPU cloud container (Intel Xeon at 2.80 GHz, 16 GB), release build, nothing else running. The method: five uninstrumented runs, medians, at 1, 2 and 4 threads. On the owner's project `main` was run five times at 4 threads and once each at 2 and 1 (one run at 1 thread takes about 40 minutes).

### Where the time goes

Multithreading works: on `main`, 4 threads give 3.6 times one thread on both jobs, and the cores stay busy. A run is slow because each pass does a great deal of work:

| Share of the time on `main` | Owner's project (stack samples) | Test piece (instruction counts, 1 thread) |
| --- | --- | --- |
| Dropping onto the held region and the tab | 40% | none (no solids) |
| A region's candidate intervals | 35% | none (no region) |
| The mesh drop-cutter | 24% | 97% |
| Fitting, assembly, entries, everything else | about 1% | 3% |

1. **The solids.** `CompositeSurface::drop_tool` dropped onto every solid for every drop. Wherever the tab is within the tool's reach, its drop samples 72 angles for the reachable arcs, then 16 or more angles on each arc with a golden-section search on each of three segments, even when the part is far above it. A box solid had no reach test at all: every drop built its twelve triangles and tested them.
2. **The candidate intervals** (`DevIndex::intervals`, decision 0034 item 5). Every pass looked at all 496,172 region triangles. For each triangle within reach (1,000 to 13,000 a pass) it ran a 48-step ternary search and two 48-step bisections, although nearly every triangle's interval lay inside one already found.
3. **The drop-cutter** (`cutter::drop_on_mesh`). About half of it was the edge test. An edge is bisected to rounding precision even when its highest point cannot beat the contact already found. The rest was the hierarchy's candidate search, the frame change and sort of the candidates, and an allocation per edge.

Simulation of the operations and assembly are small (assembly under 1 s), and there is no serial section of note.

## Decision

Four speed-ups in `openchuck-path`, none of which changes an answer:

1. **The drop-cutter skips an edge that cannot win** (`cutter.rs`, `edge_can_win`). No point of an edge lies above its higher end, and the profile is never lower than at the edge's nearest approach to the tool axis, since `h(r)` never decreases. So no tip from the edge is above the higher end less `h` there. `better` takes no tip at or below the best less `TIP_TIE`, so an edge whose bound, with 1e-7 mm for rounding, is that low leaves the best as it was and is not searched. Lane STLPENCIL's change (pull request 383, merged first) already reuses each thread's drop lists and sorts the candidates unstably; this skip sits on top of it.
2. **A solid below the part's contact is not dropped onto** (`composite.rs`, `Solid::highest_along`). No tip on a solid lies further along the tool line than the solid's highest point along it: a box's highest corner, or the higher of a cylinder's or frustum's two rims. When that, with 1e-7 mm for rounding, is at or below the model's contact plus `TIP_TIE`, the solid could not replace the contact under the existing tie rule, so it is skipped. A box out of the tool's reach (a lower bound on its distance from the line, taken across the line along each model axis) answers "no contact" at once, which is what its triangle tests give.
3. **A region's intervals look only near the pass, and skip what is already found** (`candidates.rs`). The period in `u` is cut into cells about one triangle's span wide. A triangle is listed in every cell its span, widened by its reach, meets, and in one cell either side. A pass covering less than half the period looks only in its cells, and a strip or a wide triangle is always looked at. The intervals are joined as they are found (`Union`). That gives the same groups, bit for bit, as one sort and sweep, because a group is fixed by the joining rule whatever the order. A triangle whose whole range along the pass already lies in one group is skipped: it adds no point and closes no gap. The triangles nearest the pass are tried first, so the union fills early. The order changes the work, never the answer.
4. **Measurement and checks are kept** (NFR-04, NFR-06, GC-07): the test piece, the `carving_profile` example, the `carving` benchmarks in `benches/`, and golden hashes of four carving toolpaths written from `main` before these changes.

## Results

All runs are uninstrumented release builds on the machine above, with nothing else running. "main" is `e403239`, which includes pull request 383; "branch" is this lane's code merged on it. Every run, at every thread count and on both builds, gave the same toolpath hash (on the owner's project `c4f87ac1d878372b`, 276,766 moves; on the test piece `2c092fcc12f8a995`, 528,303 moves). The golden hashes written from `2440a14`, before pull request 383, still match.

**Owner's project, axial finishing** (the time of that operation alone, from `operation_times`; five runs on the branch at every thread count and on `main` at 4 threads; one run on `main` at 1 and 2 threads, as each takes 20 to 40 minutes):

| Threads | main | branch | Faster |
| --- | --- | --- | --- |
| 1 | 2,330 s (one run) | 372 s (369 to 380) | 6.3 times |
| 2 | 1,270 s (one run) | 203 s (197 to 209) | 6.3 times |
| 4 | 633 s (596 to 668) | 101 s (99 to 106) | 6.2 times |

CPU time (user and system) per run: about 2,360 s on `main` and 390 s on the branch. The scaling from 1 to 4 threads is 3.7 times on both. Peak memory of the whole run (roughing included): 474 MB on both.

**Test piece, axial finishing** (`carving_profile`, five runs each, medians, range in brackets):

| Threads | main | branch | Faster |
| --- | --- | --- | --- |
| 1 | 67.5 s (67.1 to 68.9) | 52.3 s (51.5 to 52.6) | 1.29 times |
| 2 | 35.1 s (34.5 to 35.3) | 27.7 s (27.3 to 28.2) | 1.27 times |
| 4 | 18.7 s (18.4 to 19.1) | 14.6 s (14.4 to 14.7) | 1.29 times |

2,093,176 drops each run; peak memory 440 to 470 MB on both. Pull request 383 alone took this piece from 22.4 s to 19.9 s at 4 threads (one run each, before and after it).

**Benchmarks** (`cargo bench -p openchuck-benches --bench carving`, criterion medians):

| Benchmark | main | branch |
| --- | --- | --- |
| 4,000 V-bit drops along a pass on the test piece | 107.5 ms | 86.1 ms |
| The same with a held box and a tab | 610.5 ms | 129.7 ms |
| Axial finishing of a 300k-triangle region, 21 passes | 1.463 s | 0.415 s |

**Other strategies** (test piece, 0.1 mm stepover, 4 threads, one run each, the same hash): helical finishing 7.3 s to 3.8 s (on `2440a14`); pencil clean-up 217 s to 175 s.

What is left on the owner's project: 95% of the drops (15.4 million of 16.3 million) are a region's seeds every 0.01 mm (OI-1182).

## Not done: changes that would alter the output

These are for the owner, and none is built:

- **A region's passes are seeded at the tolerance** (OI-1182). With a selected region, `fit_region` drops the tool every `tolerance` (0.01 mm by default) along the intervals that reach the region, then thins the points (decision 0034 item 5; the fine seeding finds features narrower than the old 0.1 mm spacing, OI-138). On the owner's project that is 15.4 million of the 16.3 million drops. A prototype that seeds at 0.1 mm (not kept) took the operation from 101 s to 50 s at 4 threads (2.8 million drops), but the path changed: 384,389 moves instead of 276,766, because the thinning that fine seeds allow no longer happens and the bisection adds points instead. The owner chose on 30 Oct 2026, relayed by the coordinator, to offer it as a per-operation setting whose default, 0.01 mm, keeps today's output; a later lane builds it (OI-1182).
- **Pencil clean-up on an STL part runs on one thread on this piece** (OI-1181). On `main` with pull request 383, at a 0.1 mm stepover, it takes 176 s at 1 thread and 175 s at 4 (branch; 217 s on `main` at 4). The places along each crease are found with `parallel::map_in_order`, whose fixed chunks are 64 points, and the creases one after another: the test piece's creases are short, so each is one chunk on one thread. The blisk stand-in of pull request 383 has long creases and scales. This is the per-crease placement of OI-1160 (lane STEPALL-Q), not this lane's.

## Simulation display (scope added 29 Oct 2026)

The owner added the display of the simulation to the lane: with these toolpaths the tool seems not to move, and the cut part takes long to load. The lane measured it on the owner's project (`crates/openchuck-app/examples/playback_times.rs`) and reported the profile and options to the coordinator separately. None of it is built in this record. In short, at 4 threads:
- Preparing the stock playback simulates the whole toolpath again (131 s) and keeps 10 checkpoints of 7 MB each.
- Each cut-part frame then replays up to about 36,000 moves from the nearest checkpoint, maps the comparison's colours onto 1.3 million triangles on one thread and builds a picker. That takes 2 to 23 s, and the tool waits at the cut part's cursor meanwhile (issue 333).

## Alternatives considered

- **A tighter bound per triangle, or a tighter hierarchy test.** Either would change the order in which triangles are visited, and with it which of two contacts within a tie is kept (decision 0113). The skips above change no order.
- **Caching drops between neighbouring passes.** The passes are fitted one to a thread and alone (decision 0195), so a cache would be shared state or a change of the order of work. It is not needed for these gains.
- **Coarser seeds, fewer passes, a looser tolerance.** They would change the output; see "Not done".

## Consequences

- `openchuck-path` (internal only, so no public API change and no version bump): `cutter.rs` (the edge skip, on top of pull request 383), `composite.rs`, `solid.rs` and `candidates.rs`; the new example `carving_profile`; a changelog line. `openchuck-testkit`: `parts::CarvedRelief`. `benches`: the example `carving_model` and the `carving` benchmarks. No B-rep code (`brep.rs`, `brep/`) and no pencil code is touched. The composite skip also applies to a STEP part with workholding, unchanged in output.
- `testdata/golden/carving/` holds the four golden hashes (Linux x86-64; OI-319).

## Tests

- `req_gc_07_skipped_edges_leave_every_drop_the_same` and `req_gc_07_an_edge_ruled_out_never_changes_the_best` (`cutter.rs`): every drop with the skip equals every drop with all edges tested, bit for bit, for six profiles (V-bit, tapered ball, ball, flat, bull-nose and a concave custom profile) on the test piece; and an edge the rule rules out never changes the best.
- `req_gc_07_a_box_out_of_reach_gives_the_same_answer_as_its_faces` and `req_gc_07_no_contact_with_a_solid_is_above_its_highest_point` (`solid.rs`).
- `req_gc_07_a_solid_below_the_part_is_skipped_with_the_same_answer` (`tests/solids.rs`): the composite's drops equal dropping onto the model and every solid, bit for bit, along passes over the test piece with a held box, a tab, a collar above it and a band at its own radius.
- `req_gc_07_candidate_intervals_are_the_same_as_a_sweep_of_every_triangle` and `req_gc_07_a_running_union_is_the_same_as_one_sweep` (`candidates.rs`): against the old algorithm, kept in the test, on a dense region of the test piece and on scattered triangles across the seam, for axial passes, slanted lines and helices.
- `req_gc_07_req_nfr_06_*` (`tests/carving.rs`): axial finishing of the whole piece, of a region and next to a held box and a tab, and helical finishing of a region, the same at 1, 2 and N threads on every machine. Their golden hashes, written from `main` before these changes, are compared only with `OPENCHUCK_CARVING_GOLDEN=1` on the machine they were written on: a CI runner gave another region path from the same code, because glibc chooses `asin`, `atan2` and `hypot` by the processor (OI-319). On the lane's machine they matched after every merge.
- Each identity test was checked against a deliberately wrong rule, and it failed.
