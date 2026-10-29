# 0292: Fine finishing of a carving: where the time goes, and four speed-ups that leave the output unchanged (lane CARVEPERF; refs issue #382)

- **Status:** proposed 29 Oct 2026 by lane CARVEPERF. Guidance only until the owner accepts it (decision 0049).
- **What it amends:** nothing in the documents. It adds rules for skipping work in the mesh drop-cutter (decision record 0012 items 4 and 5), the composite target (decision record 0035 items 1 and 2) and a region's candidate intervals (decision record 0034 item 5). Each rule leaves every answer the same, bit for bit, ties included (decision record 0113). NFR-04, NFR-06 and GC-07 are unchanged, and so are every tolerance, sampling step and toolpath. No project schema change.
- **Date:** 29 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): opens OI-1180 (the owner re-times the carving on the laptop), OI-1181 (pencil clean-up on an STL part runs on one thread) and OI-1182 (a region's passes are seeded at the tolerance: a choice for the owner, not built).

Numbering: the coordinator gave lane CARVEPERF records 0292 to 0296 and OI-1180 to OI-1199. On 29 Oct 2026 the highest record on `main` and on every pushed branch was 0282, and no `OI-118x` or `OI-119x` number was in use.

## Context

Issue #382: "Axial finishing (and potentially other strategies) take a long time to generate for fine stepovers (e.g., 0.03 with a 0.1 V bit) despite multithreading." Requirements: NFR-04 (generation uses the cores), NFR-06 and GC-07 (the same output at any thread count, byte for byte). [Decision 0195](0195-axial-finishing-speed-and-steady-progress.md) already fits the passes of axial finishing one to a thread.

The lane timed two jobs:

- **The owner's carving project**, handed to the lane session by the owner and kept out of the repository at the owner's request (only counts, settings and timings are recorded here). It has a 500,738-triangle STL model and two operations: rotary roughing, then axial finishing with a 60 degree V-bit with a 0.1 mm tip at a 0.03 mm stepover over a selected region of 496,172 triangles. It is on box stock with a held region and a 10 mm tab, and the finishing makes 276,766 moves.
- **The lane's own test piece**, `openchuck_testkit::parts::CarvedRelief::STANDARD`: a 24 mm by 30 mm cylinder with raised strokes 0.8 mm high (diagonals, rings and crosses on 5 mm cells, chosen by a fixed hash) with chamfered walls and sharp edges, over a gentle wave. At 0.1 mm vertex spacing it has 453,908 triangles. `benches/examples/carving_model.rs` writes it as STL (22 MB, not committed) for a project, and `crates/openchuck-path/examples/carving_profile.rs` times a strategy on it through the public API.

Machine: a 4-vCPU cloud container (Intel Xeon at 2.80 GHz, 16 GB), release build, nothing else running. The method: five uninstrumented runs, medians, at 1, 2 and 4 threads. On the owner's project `main` was run five times at 4 threads and once each at 2 and 1 (one run at 1 thread takes about 45 minutes).

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

1. **The drop-cutter skips an edge that cannot win** (`cutter.rs`, `edge_can_win`). No point of an edge lies above its higher end, and the profile is never lower than at the edge's nearest approach to the tool axis, since `h(r)` never decreases. So no tip from the edge is above the higher end less `h` there. `better` takes no tip at or below the best less `TIP_TIE`, so an edge whose bound, with 1e-7 mm for rounding, is that low leaves the best as it was and is not searched. The edge test reuses one buffer per drop, and the candidates are sorted unstably (their keys are distinct, so the order is the same).
2. **A solid below the part's contact is not dropped onto** (`composite.rs`, `Solid::highest_along`). No tip on a solid lies further along the tool line than the solid's highest point along it: a box's highest corner, or the higher of a cylinder's or frustum's two rims. When that, with 1e-7 mm for rounding, is at or below the model's contact plus `TIP_TIE`, the solid could not replace the contact under the existing tie rule, so it is skipped. A box out of the tool's reach (a lower bound on its distance from the line, taken across the line along each model axis) answers "no contact" at once, which is what its triangle tests give.
3. **A region's intervals look only near the pass, and skip what is already found** (`candidates.rs`). The period in `u` is cut into cells about one triangle's span wide. A triangle is listed in every cell its span, widened by its reach, meets, and in one cell either side. A pass covering less than half the period looks only in its cells, and a strip or a wide triangle is always looked at. The intervals are joined as they are found (`Union`). That gives the same groups, bit for bit, as one sort and sweep, because a group is fixed by the joining rule whatever the order. A triangle whose whole range along the pass already lies in one group is skipped: it adds no point and closes no gap. The triangles nearest the pass are tried first, so the union fills early. The order changes the work, never the answer.
4. **Measurement and checks are kept** (NFR-04, NFR-06, GC-07): the test piece, the `carving_profile` example, the `carving` benchmarks in `benches/`, and golden hashes of four carving toolpaths written from `main` before these changes.

## Results

To be filled with the medians of the five-run measurement.

## Not done: changes that would alter the output

These are for the owner, and none is built:

- **A region's passes are seeded at the tolerance** (OI-1182). With a selected region, `fit_region` drops the tool every `tolerance` (0.01 mm by default) along the intervals that reach the region, then thins the points. On the owner's project that is most of the drops left after this change. Seeding more coarsely and refining as the whole-part path does would cut the drops by several times, but it changes the points of the path, so it is the owner's choice. The lane does not recommend it without a measurement of what it would change on the owner's part.
- **Pencil clean-up on an STL part runs on one thread** (OI-1181). On the test piece at a 0.1 mm stepover it spent 249 s in drops in a 252 s run at 4 threads. These changes take it from 252 s to 192 s with the same path. Spreading its work over the threads belongs to lane STLPENCIL, which is editing that code.

## Alternatives considered

- **A tighter bound per triangle, or a tighter hierarchy test.** Either would change the order in which triangles are visited, and with it which of two contacts within a tie is kept (decision 0113). The skips above change no order.
- **Caching drops between neighbouring passes.** The passes are fitted one to a thread and alone (decision 0195), so a cache would be shared state or a change of the order of work. It is not needed for these gains.
- **Coarser seeds, fewer passes, a looser tolerance.** They would change the output; see "Not done".

## Consequences

- `openchuck-path` (internal only, so no public API change and no version bump): `cutter.rs`, `surface.rs` (`triangles_for_drop` returns the hierarchy's order; the drop sorts its candidates), `composite.rs`, `solid.rs` and `candidates.rs`; the new example `carving_profile`; a changelog line. `openchuck-testkit`: `parts::CarvedRelief`. `benches`: the example `carving_model` and the `carving` benchmarks. No B-rep code (`brep.rs`, `brep/`) and no pencil code is touched. The composite skip also applies to a STEP part with workholding, unchanged in output.
- `testdata/golden/carving/` holds the four golden hashes (Linux x86-64; OI-319).

## Tests

- `req_gc_07_skipped_edges_leave_every_drop_the_same` and `req_gc_07_an_edge_ruled_out_never_changes_the_best` (`cutter.rs`): every drop with the skip equals every drop with all edges tested, bit for bit, for six profiles (V-bit, tapered ball, ball, flat, bull-nose and a concave custom profile) on the test piece; and an edge the rule rules out never changes the best.
- `req_gc_07_a_box_out_of_reach_gives_the_same_answer_as_its_faces` and `req_gc_07_no_contact_with_a_solid_is_above_its_highest_point` (`solid.rs`).
- `req_gc_07_a_solid_below_the_part_is_skipped_with_the_same_answer` (`tests/solids.rs`): the composite's drops equal dropping onto the model and every solid, bit for bit, along passes over the test piece with a held box, a tab, a collar above it and a band at its own radius.
- `req_gc_07_candidate_intervals_are_the_same_as_a_sweep_of_every_triangle` and `req_gc_07_a_running_union_is_the_same_as_one_sweep` (`candidates.rs`): against the old algorithm, kept in the test, on a dense region of the test piece and on scattered triangles across the seam, for axial passes, slanted lines and helices.
- `req_gc_07_req_nfr_06_*` (`tests/carving.rs`): axial finishing of the whole piece, of a region and next to a held box and a tab, and helical finishing of a region, the same at 1, 2 and N threads and the same as the golden hash written from `main`.
- Each identity test was checked against a deliberately wrong rule, and it failed.
