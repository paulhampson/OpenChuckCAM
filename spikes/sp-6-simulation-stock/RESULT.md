# SP-6 result: simulation stock representation

Throwaway spike code (work breakdown, Spikes; spike rules). The decision record is `docs/decisions/0022-sp-6-simulation-stock.md` on branch `docs/decision-0022-sp-6-simulation-stock` (open items OI-99 to OI-102); this file is the write-up and [`out/report.md`](out/report.md) holds the full tables.

## Question

Rotary grid, dexel or voxel: which gives the best accuracy, speed and memory on a rotary part (SIM-03, SIM-05 to SIM-07)? Also: how would a collision query run on each (COL-05, P2), and does a V-bit and a custom-profile tool sweep correctly (SP-13)?

Pass criteria: (1) removed volume on cylinder, cone and sphere within the resolution error; (2) time and memory within budget at default resolution; (3) same results at any thread count; (4) ray and region queries supported.

## Method

A separate Cargo project, no third-party dependencies (`cargo test --release`, then `cargo run --release > out/report.md`). Three implementations behind one `StockModel` trait (`stock.rs`), all in the part frame (X along the A axis; the part turns about +X by A; the tool is vertical).

| Module | What it does |
| --- | --- |
| `profile.rs`, `shapes.rs`, `par.rs` | Copied unchanged from SP-13 (tool profile, standard shapes, deterministic parallel map) |
| `toolray.rs` | The one query all three share: where a straight line is inside the revolved tool. Convex profiles: golden-section peak of a concave function plus bisection (exact). Others: marching at R/8 plus bisection |
| `rotary.rs` | Rotary grid: material radius per (x, part angle); a cut finds where the ray from the axis enters the tool |
| `dexel.rs` | Dexels along part-frame Z in an (x, y) grid, intervals per column; a cut subtracts the tool span from each column (tool axis tilts as A turns) |
| `voxel.rs` | One bit per cell, whole words per axial slab; a cut clears the cells whose centres lie in the tool span of each column |
| `parts.rs` | Analytic cylinder, cone (16.7 degrees) and sphere zone; ball-nose turning poses that cut them out; a quadrature oracle for a straight pass |
| `tests/sp6.rs` | 5 tests named after the requirements |

Test setup: stock Ø50 x 40 mm. Parts are turned with a Ø4 ball (flute 30 mm), centre following the offset curve at 0.25 mm stations, one full turn of A in 1 degree steps at each (52,200 poses; 23,400 for the sphere). The expected removed volume is the analytic volume of the part's zone; the surface is also probed by 4,380 radial rays from outside per part. The error bound is the lateral area times (half a cell plus 0.006 mm of ball scallop). Reference for straight passes: a plan-view quadrature at 0.01 mm that uses only the profile's `h(r)`.

## Numbers (release, 4 threads in the container)

**1. Turned parts** (full table in the report). Volume error is within the bound everywhere (at 0.1 mm: +8 to +19 mm3 against bounds of 113 to 253 mm3).

| Cell | Worst radial error: rotary / dexel / voxel (mm) | Time, cone, 4 threads (s) | Memory (MB) |
| --- | --- | --- | --- |
| 0.2 | 0.035 / 0.127 / 0.159 | 6.3 / 3.1 / 3.7 | 1.3 / 2.9 / 1.6 |
| 0.1 | 0.023 / 0.071 / 0.084 | 25 / 13.0 / 13.6 | 5.0 / 11.8 / 12.5 |
| 0.05 | 0.017 / 0.036 / 0.043 | 99 / 58 / 61 | 20 / 48 / 100 |

Radial error stays within one cell at every setting. Against slice 7's "no worse than 0.02 mm at the finest setting", only the rotary grid meets it at 0.05 mm (0.017); dexel and voxel give 0.036 to 0.043 there, which is below one cell but above 0.02 mm, so that limit needs a finer cell for them or a restated limit (OI-100).

**2. Straight pass by tool type** (Ø6 tools, tip 3 mm into the cylinder, 0.1 mm cell):

| Tool | Reference mm3 | Rotary error | Dexel error | Voxel error |
| --- | --- | --- | --- | --- |
| flat | 436.3 | **+27.8 (6 %)** | +0.0 | +0.6 |
| ball | 330.8 | 0.0 | +0.2 | +0.6 |
| V-bit 90 | 200.5 | 0.0 | 0.0 | -3.7 |
| custom (ball as arc + side) | 232.6 | +2.6 | +0.2 | +1.5 |

**3. Reference job** (cone, 52,200 poses, 0.1 mm), seconds at 1 / 2 / 4 threads: rotary 83 / 49 / 24; dexel 46 / 24 / 13; voxel 51 / 26 / 14. State hashes identical at 1, 2 and 4 threads for all three, in the job and in the test.

**4. Memory of untouched stock** at 0.2 / 0.1 / 0.05 mm: rotary 1.3 / 5 / 20 MB; dexel 2 / 8 / 32 MB (grows with cuts); voxel 1.6 / 12.5 / 100 MB (cubic).

## Findings

- **The rotary grid is wrong for walled cuts.** The ray from the axis that leaves the tool through a vertical wall takes the material beyond the wall with it, so the stock is star-shaped by construction. A flat-bottom slot overcuts by 4 to 6 % at every cell size (+27.8 mm3 at 0.1, +25.0 at 0.05, +16.7 at 0.2, +26.5 at 0.4). It is the most accurate model on turned and wrapped surfaces (0.011 to 0.04 mm radial error) and the smallest, but roughing, pockets, rest machining and any islands (OP-03, OP-09, SIM-06) need walls. The test `req_sim_03_straight_pass_groove_volume` records this as an overcut, so it cannot be forgotten.
- **Dexel and voxel agree with the oracle on every tool**, including the V-bit and the custom profile (SP-13: the profile drives the sweep; criterion 2 holds here as well). The voxel's V-bit error (-3.7 mm3, 1.8 %) is the cell-centre quantisation of a thin sliver; it shrinks with the cell (+0.6 on the flat).
- **Dexel is the fastest and, past 0.1 mm, much smaller than voxel**, and its radial error is half that of the voxel's at the same cell. Voxel memory is cubic (100 MB at 0.05 mm for a Ø50 x 40 part; 800 MB at 0.025). Dexel memory depends on how cut the stock is.
- **Per-pose cost is dominated by the line-tool solver, not the storage.** The golden-section search costs about 130 evaluations of `h(r)` per column for a convex tool, 70 times a flat tool's cost (Table 2: 95 us per pose for a flat, 6 to 7 ms for a ball or a V-bit at Ø6, 3 mm deep, 1 thread). A cheap bracketing of the peak by a few samples before the search is the obvious saving; not built (OI-101). The three models share the solver, so the ranking does not depend on it.
- **A too-short flute is not a stock-model fault.** The first run used a 12 mm flute against 25 mm radius stock; dexel and voxel left material above the flute (the shank is not modelled) while the rotary grid, which removes everything outside the entry, did not. Strategies and collision checks (COL-05) own the shank and holder.
- **Poses must be dense.** The swept volume between two poses is not built; at 1 degree of A the ball's scallop is under 0.006 mm, at 2 degrees it would be 0.03 mm. Slice 7 must sample moves no coarser than the resolution (OI-102).
- **Non-convex custom profiles** are marched at R/8 and can miss a span shorter than that step; convex ones are exact. Slice 7 should refine this or refuse thin non-convex features (OI-102).

## Collision query sketch (COL-05, P2)

Asking "is there material at the tool or holder pose?" is the same query as a cut, without the subtraction: the holder is a second revolved profile.

- **Dexel**: for each column in the tool's part-frame bounding box, `cut_spans` against the column's intervals; any overlap is a collision, and the overlap length and column position give depth and place. Cost is that of one cut pose.
- **Voxel**: the same span test, then test the bits in the span; the word-wise test is a mask and compare.
- **Rotary grid**: the span test on each ray in the pose's angular window; a collision when the tool entry is nearer than the stored radius. Fast, but it inherits the star-shape error: after a slot it holds too little material beyond the slot wall, so it can miss a collision with that wall.

## Decision (recommendation)

1. **Dexels in the part frame as the `StockModel` implementation for slice 7**, along part Z, with the tool span solved exactly per column. It meets all four criteria, handles walls, undercuts and islands, is the fastest, has modest memory, and answers ray queries (first interval) and region queries (interval overlap) directly.
2. **Voxel is the fallback** behind the same interface (same accuracy class, simpler to reason about, cubic memory). **The rotary grid is not recommended as the main stock**; it remains a possible fast path for the wrapped-surface family where walls do not occur, to be reconsidered only if dexel speed fails on real jobs.
3. **Default cell 0.1 mm** for slice 7, adjustable (SIM-07), with 0.05 mm as the proposed finest setting; to be confirmed by the owner (OI-100).
4. **Budget proposed for SP-13 criterion 3 (OI-56)**: the reference job (52,200 poses, Ø4 ball, 0.1 mm cell) finishes in no more than 30 s on 4 threads, 120 s on one thread, and 256 MB at the default cell. Dexel measures 13 s, 46 s and 12 MB, so SP-13 criterion 3 is met against it; the tool profile adds no extra cost beyond `h(r)` in the line solver. Per-pose cost is in Table 2.

## Limits of this spike

- Linux x86-64 only, one machine; bit identity across platforms is not tested (the same open question as OI-64, OI-75).
- Vertical tool only (the tool's tilt comes only from A); lead and tilt angles (OP-05) are supported by the line solver but not exercised.
- The tool above its flute length is not modelled.
- No brute-force high-resolution comparison on a job (slice 7, item 7); the oracles are analytic.
- Dexel direction is fixed in the part frame, so accuracy is anisotropic (walls parallel to Z are sampled at the cell size across x and y); not measured separately here.
