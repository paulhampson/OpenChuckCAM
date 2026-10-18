# 0108: The deviation samples follow the surface area, not the longest edge

- **Status:** accepted by the project owner (18 Oct 2026), first relayed by the orchestrator session and then confirmed by the owner in lane BM's own session (decision 0049). It amends item 6 of accepted record [0025](0025-slice-7-simulation.md) (the deviation against the target) for thin triangles only. Proposed by lane BM; the code is on branch `claude/lane-bm-sim-memory`.
- **Date:** 18 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md). New: OI-331. Closes OI-317 (lane TE's record of the blisk run).

Numbering: 0108 was reserved for lane BM by the orchestrator (see record 0110). OI-331 is the next free item after lane S19's OI-330.

## Context

Lane TE simulated a helical finishing toolpath on the owner's blisk (`testdata/stl/blisk.stl`: 6,386 triangles, 40 mm diameter, 50 mm long; Makera Z1, 3 mm ball, Ø42 x 52 mm cylinder stock from X = -1, 0.1 mm cell). Memory grew to 12.6 GB and the process was killed (OI-317). This breaks NFR-07: heavy input must give a clear error, never a crash. `openchuck-cli simulate` failed the same way.

The dexel stock is not the cause. Measured through the cut, it holds 17 MB at the end of the run (240,654 intervals, never more than 8 in a column). The cause is the comparison with the target (SIM-05, record 0025 item 6). Each triangle was subdivided n x n, with n the triangle's longest edge in cells, and sampled at the sub-triangle centroids. That gives n² samples whatever the triangle's area. The blisk has many long thin triangles, so it got **74,258,398** samples at 0.1 mm. About 1 million would cover its 10,550 mm² of surface. Each sample was held three times at once: as a point and normal, as a result in a per-chunk vector, and as a result in the flattened list. The comparison alone took 567 s on an uncut stock.

## Decision

1. **A thin triangle is sampled on a lattice.** A triangle whose longest edge is more than 4 times its height gets a square lattice of pitch one cell in its own plane, aligned with its longest edge, plus points along each edge no further apart than the cell. Every point of the triangle is then within one cell of a sample. A point more than cell/√2 from every edge has a lattice point within that distance. A point nearer an edge has an edge point within cell·√3/2. The count follows the area plus the perimeter in cells, not the square of the length.
2. **A well-shaped triangle keeps record 0025's n x n subdivision.** Its samples are unchanged, in number, position and order. The faces of a cube (ratio 2) and the existing simulation tests give the same values as before.
3. **Each sample is held once.** The samples are written straight into the result list, each triangle filling its own share in triangle order. They are then measured in place, in fixed chunks of 256, so the result does not depend on the thread count (NFR-06). The peak is the 64-byte result per sample.
4. **A guard, `MAX_DEVIATION_SAMPLES` (64 million), the same scale as the stock's `MAX_COLUMNS`.** The samples are counted before any is taken. Counting stops just past the limit, so a 20 m triangle at 0.025 mm is refused in microseconds. A surface that needs more is refused with a clear error naming `simulation.cell` and asking for a coarser resolution (NFR-07). The blisk is well inside it at the finest setting: about 16 times its 2.3 million at 0.1 mm.

## How the count is bounded, and what it costs in accuracy

- **Per triangle.** A well-shaped triangle (longest edge at most 4 times its height, so its longest edge squared is at most 8 times its area) has n² samples with n = ceil(longest / cell): at most about 8 A / cell² + 2 L / cell + 1, for area A and longest edge L. A thin triangle has at most about A / cell² lattice points plus one per cell of each row's end and of each edge: about A / cell² + 2 P / cell + 4, for perimeter P. Either way the count follows the area and the perimeter, never the square of the length. Before, a 50 mm sliver of any width had 250,000 samples at 0.1 mm.
- **In total.** At most `MAX_DEVIATION_SAMPLES`, 64 million, checked by counting before any sample is taken. Peak memory is 64 bytes a sample.
- **Accuracy.** Every point of every triangle is still within one cell of a sample, as record 0025 asks. Well-shaped triangles give exactly the samples they gave before, and every existing simulation test value is unchanged. On lane TE's blisk project (OI-317), the report through `openchuck-cli simulate` keeps the same moves, poses, removed volume and most material left (3.0000 mm at the same point). The deepest overcut reads 0.0758 mm against 0.0760 mm under the old sampling, at a different place: a difference of 0.0002 mm, well inside the one-cell tolerance of verification plan section 4.1.

## Measured (release build, 4 threads, 8-vCPU cloud container)

| Case | Before | After |
| --- | --- | --- |
| Lane TE's blisk project (OI-317; 3 mm ball, 8 mm flutes, stepover 2 mm, helical 5°, 5,568 moves), `openchuck-cli simulate`, 0.1 mm | debug: above 11.1 GB at 71 s on `main`; one unguarded run finished in 93.6 s; 74,258,398 samples | release: **9.5 s, 177 MB** peak resident, 2,306,985 samples |
| Blisk, lane BM's own tool (3 mm ball, 12 mm flutes, 3.175 mm shank, stepover 0.3 mm), helical 5°, 37,695 moves, 0.1 mm: `Application::simulate` | killed above 12.6 GB (lane TE); 74,258,398 samples | 40.6 s, 300 MB peak resident for the whole process, 2,306,985 samples, stock 17 MB |
| The same with helical 2°, 52,372 moves, 0.1 mm: `Application::simulate` | killed (lane TE) | 43.7 s, 200 MB peak resident, 2,306,985 samples, stock 17 MB |
| Cylinder of 180 axial strips (Ø20 x 30 mm) at 0.5 mm: comparison alone (`tests/deviation_bounds.rs`) | 1,484,626 samples, 330 MB peak allocated | 66,988 samples, 4.3 MB peak allocated |

## Alternatives considered

- **Longest-edge bisection of thin triangles, then n x n on the pieces.** Tried first. It cut the cylinder case from 1.48 million samples to 352 thousand, but a sliver bisected towards a far apex is cut lengthwise. Its pieces keep getting thinner, so the count stayed about 5 times the lattice's.
- **A lattice for every triangle.** Simpler, but it changes every existing sample and every deviation value in the tests for no gain on well-shaped meshes. Rejected for item 2.
- **Only a cap on the sample count.** This would give a clear error on the owner's own model at the default cell. Rejected: the reference case must simulate.

## Consequences

- Code: `openchuck-sim` (`deviation`: `Plan`, `surface_samples`, `triangle_values` counting by the same plan, `MAX_DEVIATION_SAMPLES` exported). The new constant is an addition, so no version bump is needed (decision 0048).
- Tests: `crates/openchuck-sim/tests/deviation_bounds.rs` measures memory with a counting allocator: `req_sim_05_req_nfr_07_samples_and_memory_follow_the_area_not_the_longest_edge`, `req_sim_05_a_thin_triangle_is_sampled_no_further_apart_than_the_cell`, `req_sim_05_well_shaped_triangles_keep_their_subdivision`, `req_nfr_07_req_sim_07_a_surface_too_fine_to_compare_is_refused_with_a_clear_error` and `req_nfr_06_thin_triangle_samples_are_identical_at_1_2_and_n_threads`.
- Record 0025 item 6 should read "sub-triangle centroids for well-shaped triangles, a lattice and edge points for thin ones (record 0108)" (this accepted record wins over that wording until it is edited). No requirement or architecture text changes.
- OI-331: lane TE's blisk project reports no overcut beyond the cell (0.0758 mm at most). With lane BM's own tool data on the same part (12 mm flutes, 3.175 mm shank, stepover 0.3 mm), the report shows overcut after helical finishing: worst 0.69 mm on 1,992 samples at a 5° step, and 0.18 mm on 117 at 2°. The overcut shrinks with the step, which suggests the path itself (chords between angle steps) rather than the sampling, but this record does not settle it.
