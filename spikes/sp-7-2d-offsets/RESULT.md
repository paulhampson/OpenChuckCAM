# SP-7 result: 2D offsets and booleans on an unrolled cylinder

Throwaway spike code (work breakdown, Spikes; spike rules). The decision record is `docs/decisions/0016-sp-7-2d-offsets.md` on branch `docs/decision-0016-sp-7-2d-offsets` (open items OI-68 to OI-70); this file is the write-up and [`out/report.md`](out/report.md) is the full report. Numbers 0015 and OI-64 to OI-67 are already claimed by the unmerged SP-4 work, so this spike takes the next free ones.

## Question

Do `cavalier_contours` and `i_overlay` give robust offsets and booleans for unrolled regions, with arcs surviving back to A angles and G2 or G3?

Pass criteria (work breakdown): (1) no failures on a corpus of awkward shapes; (2) arcs kept or approximated within tolerance; (3) deterministic. Named fallback: a `clipper2` C++ wrapper, or polylines only.

## Method

A separate Cargo project (`cavalier_contours =0.9.0`, `i_overlay =9.0.1`, nothing else). `cargo test --release` runs 17 tests (about 4 minutes); `cargo run --release --bin report > out/report.md` writes the report (about 15 minutes); `./run_determinism.sh` compares digests across processes.

The cylinder is developed to a plane: x along the axis, s = R times the angle. The development is an isometry, so a distance in the plane is a distance on the surface, and a straight segment is a straight (X, A) move. R is 12.5, 25 and 50 mm in the tests.

| Module | What it does |
| --- | --- |
| `geom.rs` | Loops with bulges, regions (outer and holes), constructors, and the **oracle**: exact distance to a boundary of lines and arcs, area, flattening, membership. Written separately from both crates |
| `ops.rs` | The operations under test behind plain data: `cavalier_contours` shape offset and loop booleans (arcs kept); `i_overlay` outline offset and region booleans (arcs flattened first) |
| `corpus.rs` | 23 hand-made cases and a seeded generator (120 random polygons, arc polygons and hole fields) |
| `check.rs` | Offset check, boolean check, hash, input validation |
| `seam.rs` | Regions that go all the way round: the band is tiled three times |
| `dev.rs` | Development to (X, A) and back; arcs cut into chords |
| `tests/sp7.rs` | 17 tests, named after the requirements |

**The oracle.** Every offset pass is judged two ways: each point sampled along the result boundary (chord error 1e-5 mm) must lie at the offset distance from the *original* boundary (exact distance, computed from the input lines and arcs), and random points away from the offset curve must be inside the result exactly when the exact signed distance says they should be. Booleans are judged by the same point test against the inputs. Cavalier results must be within 1e-3 mm of the exact curve, i_overlay results (flattened arcs) within 5e-3 mm, the 0.005 mm tessellation budget of verification plan 4.1. `req_nfr_05_checks_have_teeth` shows that a wrong distance, a missing loop and a 0.01 mm shift all fail.

**Corpus.** Bore; 6 x 6 hole grid with 4 mm gaps (islands touch at a pass); rounded rectangle; slot hole; thin slot (offset vanishes); dumbbell with a 4 mm neck (splits); 5-point and 12-point stars; holes 0.1 to 0.0001 mm from the edge; two holes 0.001 mm apart; 0.02 mm sliver; 1 degree spike; duplicate and collinear vertices, zero-length arcs, bulges of 1e-9 and 1e-12; a region 2000 mm along s (about 12.7 turns); a 0.3 mm hole; disc, annulus, nearly concentric ring; and holes touching the boundary at gaps 0 and 1e-7 (marked degenerate). The random set has 120 cases from seed `0x5eed0007`, and another 40 for outward offsets. Passes: inward by k times the case step until nothing is left (up to 53 passes), outward for 10 passes.

## Results

| Criterion | Result |
| --- | --- |
| 1. No failures on awkward shapes | **Met for `cavalier_contours` used the right way**, with two refusals and one caution (below). Direct inward offsets: 0 failed passes of 3,100 on the random corpus and of every hand case except the degenerate ones, with 0 wrong points; outward offsets 0 failed of 600; 416 booleans 0 wrong over 207,890 judged points; regions that wrap, 0 wrong. `i_overlay` booleans: 336 operations, 0 wrong over 167,912 points, 0 panics. |
| 2. Arcs kept or approximated within tolerance | **Met.** Cavalier keeps arcs as arcs and its boundary points are on the exact offset curve to the oracle's 1e-5 mm resolution (library tolerance 1e-4); one arc segment where a flattened polygon needs about 50 chords. Back to angles, every vertex and chord midpoint is within 0.005 mm at R 12.5, 25 and 50. **G2 and G3 cannot carry the arcs on to the G-code** (item 6 below). |
| 3. Deterministic | **Met.** Bit-identical results between runs, at 1, 2 and 4 threads, and across separate processes (cavalier keeps an `ahash` table with a per-process seed; it is lookup only and does not change the output). The report digest `2bcb25f6d8f9fea3` is the same in separate processes. |

### Findings

1. **Offset the original region by k times the step, never the previous pass.** Offsetting the previous result went wrong on the hole grid (step 0.4: pass 6, a wrong region 6.7 mm out; 395 of 400 passes fail) and on the annulus (steps 0.4 and 0.5: from the pass where the ring closes exactly). It works at steps that miss the exact touch (0.37 on both), and on the grid at 0.5, so it is brittle rather than always wrong. Direct offsetting passed every step, including passes that land exactly on a tangent (grid at 0.4, pass 5, d = 2.0).
2. **Touching loops give a silent wrong answer.** A hole 0 or 1e-7 mm from the boundary makes cavalier's shape offset return wrong regions (41 of 42 passes, up to 41 mm off). A gap of 1e-4 mm works, 1e-5 is the library's position epsilon. The product must refuse such input before offsetting (NFR-07). `validate_region` does it in the spike: a minimum gap between loops of 2e-5 mm, plus self-intersection, hole outside, non-finite values and fewer than 2 vertices. `i_overlay` handles the same touching input correctly, so a union through it (arcs flattened) is an alternative to refusing.
3. **`i_overlay` outline offset is correct but slow when arcs are flattened finely.** With arcs flattened at 1e-4 mm, the rounded rectangle took 129 s for 50 passes (5.2 s for a single offset) and a disc 134 s for 40; at 1e-3 mm the slowest offset is 59 ms (report section 6). The cost grows faster than the square of the vertex count once the offset passes a corner radius (1.8 times the vertices, 4.5 times the time). At 1e-3 mm flattening and 5e-4 mm round joins it stays within 3.4e-3 mm of the exact curve. It stays a fallback: cavalier is about 30 times faster on the hole fields (2.0 ms against 61 ms for 20 x 20 holes) and keeps arcs.
4. **`i_overlay` leaves zero-area contours when a hole closes.** In 1 of 580 outward passes (2 contours) and 1 of 1,381 inward passes (1 contour) the raw result held a zero-area contour; in the outward case a 3-vertex one 6.9 mm from the real boundary. Contours under 1e-6 mm squared must be dropped (`io_offset` does; `io_offset_raw` shows the artefact; `req_op_04_iooverlay_closing_hole_leaves_zero_area_loop`).
5. **Cavalier fails silently on an extreme jagged loop.** 50,000 vertices 5 um apart with each vertex 0.2 mm off a circle: 20 loops returned, 35 mm off, 103 wrong points. The same shape at 5,000 vertices (50 um) is correct. i_overlay is correct on both (and takes 32 s for the larger one). Real contours from a mesh silhouette will not be this bad, but the product needs input decimation to a tolerance and a cheap result check (OI-69).
6. **Arcs do not survive as G2 or G3.** A circle in the development is not a circle in (X, A): the A word is an angle, and in the controllers' arc rules (LinuxCNC takes G2 and G3 in the XY, XZ or YZ plane and moves any other axis linearly, so A would turn linearly along the arc) the arc cannot sit in a plane that holds A. The conversion therefore cuts every arc into chords of at most 0.005 mm sagitta: a bore pass (4 lines and 2 arcs) at R 25 becomes 96 moves and a rounded rectangle pass (4 lines and 4 arcs) 80 (report section 5). Lines stay single G1 moves exactly. Not tried in the simulator (no LinuxCNC in the session) and not checked against the Z1 (OI-68). Arcs are still worth keeping through the 2D stage: exact offsets and booleans, and the chord count is set once, at the end, by the toolpath tolerance.
7. **A region that goes round the cylinder is handled by tiling.** The band is tiled at s - P, s, s + P (P = 2 pi R), offset as one region and judged in the middle period; islands that straddle the seam work (R 12.5 to 50, offsets 0.3 to 2.5, both engines, 0 wrong). It costs three times the geometry. The wrap and unwind of A is not tested; it stays with OI-06 (OI-70).
8. **Speed.** Cavalier, one thread: 20 x 20 holes 2.0 ms; 5,000-vertex loop 32 ms. `i_overlay`, with arcs flattened at 1e-3 mm: 61 ms and 137 ms. The oracle took far longer than either library.
9. **Spike mistakes found on the way** (all fixed, none of them the libraries'): the oracle's arc flattening had a floor on the step that put 1.1e-3 mm of error on a very flat arc and made two random cases look like failures; the first tiling put some islands across the artificial ends of the band (the seam test now runs `validate_region` on its own input).

## Decision

Cavalier for offsets (always from the original region), cavalier loop booleans where arcs matter, i_overlay for region booleans and as the cleaning and fallback engine, both pinned; no fallback needed. See the decision record for the proposals that need agreement and the document changes.

## Limits

- The corpus is generated and hand-made, not taken from real models; none of it came from a mesh silhouette or a STEP face.
- Booleans are tested as pairs (a region and a moved or rotated copy, and adjacent passes), not in long chains.
- cavalier's boolean works on single loops; region booleans with holes in cavalier (subtracting each hole in turn) were not tried.
- The G2/G3 statement rests on the LinuxCNC documentation, not on a run.
- The platforms: one Linux x86-64 container. Both crates are pure Rust, but no run on Windows or macOS; the cross-platform digest is the check to repeat in CI.
- Timings are one thread on this container and were not profiled.
