# 0016: SP-7 result, 2D offsets and booleans on an unrolled cylinder

- **Status:** accepted by the project owner (12 Oct 2026), including the proposals that needed agreement (items 2 to 7) and the edit to `architecture.md` section 7 in item 6. The spike met its three pass criteria for `cavalier_contours` used as items 1 to 5 say. OI-68 to OI-70 stay open.
- **Date:** 12 Oct 2026.
- **Spike branch:** `spike/sp-7-2d-offsets` (throwaway, never merged). The code is in `spikes/sp-7-2d-offsets/`, a separate Cargo project with an empty `[workspace]`, never a workspace member; the write-up is `spikes/sp-7-2d-offsets/RESULT.md` and the full report `spikes/sp-7-2d-offsets/out/report.md`, both on that branch.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-68 to OI-70 are new.

Numbering: the unmerged SP-4 work claims record 0015 and OI-64 to OI-67, so this record takes 0016 and OI-68 to OI-70.

## Context

SP-7 (work breakdown, Spikes, 2 days) asks: do `cavalier_contours` and `i_overlay` give robust offsets and booleans for unrolled regions, with arcs surviving back to A angles and G2 or G3? Pass criteria: no failures on a corpus of awkward shapes; arcs kept or approximated within tolerance; deterministic. Fallback: a `clipper2` C++ wrapper, or polylines only. It runs before slice 5 and feeds slices 12 (OP-04) and 16 (OP-08).

Requirements: OP-03, OP-04, OP-08 (wrap and unroll family), NFR-05 (double precision), NFR-06 and GC-07 (determinism), NFR-07 (invalid input gives an error), NFR-14 (dependency records). Architecture section 5.2 (wrap and unroll family) and the crate selection row for 2D offsets, which says "Confirm `cavalier_contours` arc support before relying on G2/G3 output". Verification plan 4.1 (0.01 mm toolpath deviation, 0.005 mm of it for tessellation).

## Method

A separate project with `cavalier_contours =0.9.0` and `i_overlay =9.0.1`. An independent oracle (exact distance to a boundary of lines and arcs, written separately from both crates) judges every pass: result boundary points must lie at the offset distance from the original boundary, and random points must be inside the result exactly when the exact signed distance says. A corpus of 23 hand-made cases (holes touching, near-touching, slivers, spikes, redundant vertices, a region 2000 mm along s, thin rings) plus 120 seeded random cases, swept to the end inward and 10 passes outward; booleans of regions against moved and rotated copies and adjacent passes; regions that wrap, tiled three times; conversion to (X, A) at R 12.5, 25 and 50 mm; scale timings. 17 tests; checks shown able to fail. Details and numbers: `RESULT.md` and `out/report.md` on the spike branch.

## Results

- Cavalier, inward from the original region: 0 failed passes in 3,100 random-corpus passes and in every non-degenerate hand case; outward 0 of 600; boundary on the exact curve to the oracle's 1e-5 mm resolution; arcs kept.
- Cavalier loop booleans: 416 operations, 0 wrong over 207,890 judged points. `i_overlay` region booleans: 336 operations, 0 wrong over 167,912 points.
- Deterministic: identical between runs, at 1, 2 and 4 threads and across processes.
- Four failure modes found, all silent: successive offsetting (item 1), touching loops (item 2), `i_overlay` zero-area contours (item 4), an extreme jagged loop (item 5). Also `i_overlay` offset is very slow with finely flattened arcs (item 3).
- Arcs cannot become G2 or G3 in a plane containing A (item 6).

## Decision (accepted)

1. **Offset the original region by k times the step for pass k, never the previous result.** Successive offsetting failed on the hole grid and the annulus when a pass landed on touching loops, and works only by luck of the step. Direct offsetting passed every step, including exact tangents.
2. **Refuse touching input (NFR-07).** Before offsetting, the path library checks the region: at least two vertices per loop, all values finite, no self-intersection, holes inside the outer boundary, and at least 2e-5 mm between any two loops (twice the library's position epsilon). The error names the loops and the gap. Alternative if the owner prefers to repair: union through `i_overlay`, which handles the same input correctly, at the cost of flattening arcs.
3. **`cavalier_contours` is the offset engine; `i_overlay` is not used for offsets.** `i_overlay`'s outline offset is correct (within 3.4e-3 mm with arcs flattened at 1e-3 mm and joins at 5e-4 mm) but flattening at 1e-4 mm made single offsets take seconds (cost grows faster than the square of the vertex count) and it is about 30 times slower on hole fields. It remains the fallback if a cavalier failure mode cannot be guarded.
4. **`i_overlay` is used for region booleans** (regions with holes, touching input) on arcs flattened at 1e-3 mm, and its results drop contours of less than 1e-6 mm squared (zero-area artefacts when a hole closes). Cavalier's loop booleans keep arcs where a single-loop boolean is enough (window clip, adjacent passes).
5. **Dense or jagged contours are decimated to a tolerance before offsetting, and offset results get a cheap check** (every result vertex within tolerance of the distance, by a spatial index) so a silent failure becomes an error. Cavalier returned 20 loops 35 mm off for a 50,000-vertex loop with 5 um spacing and 0.2 mm noise; the same shape at 50 um spacing was correct. The tolerance and the check belong to slice 5 (OI-69).
6. **Arcs are kept through the 2D stage and cut into chords when converted to (X, A)**, with a sagitta of at most the toolpath chord tolerance (0.005 mm in the spike; OI-08 chooses the factor). A circle in the development is not a circle in (X, A), and LinuxCNC's G2 and G3 lie in the XY, XZ or YZ plane with other axes moving linearly, so they cannot carry the arc. Lines stay single G1 moves exactly. This rests on the LinuxCNC documentation; it was not run in the simulator and not checked against the Z1 (OI-68). Proposed edit to architecture section 7 (crate selection row, 2D offsets): replace "Confirm `cavalier_contours` arc support before relying on G2/G3 output" with "Arc support confirmed (SP-7, decision 0016); arcs are chorded when converted to A angles, G2 and G3 are not used with A". Made in this change.
7. **Regions that wrap the cylinder are offset as three tiles (s - P, s, s + P) and judged in the middle period**, at three times the geometry. How pass loops are cut and emitted at the seam, and the A wrap and unwind, stay with OI-06 (OI-70).
8. **Dependencies (NFR-14).** Both crates are recorded in `docs/dependencies.md` as tried and not yet declared: `cavalier_contours =0.9.0` (survey 82) and `i_overlay =9.0.1` (survey 80), pinned exactly; slice 5 declares them in `openchuck-path` only (architecture: internal to unroll-family strategies). Both MIT or Apache-2.0; the transitive tree is 20 crates, all MIT or Apache-2.0 family (`zerocopy` BSD-2-Clause OR Apache-2.0 OR MIT, `unicode-ident` adds Unicode-3.0 but only for build-time proc macros). Cavalier's repository carries `AGENTS.md` and it has 2 dependent crates; `i_overlay` had five major versions in five months, so its upgrades are deliberate and its types stay behind our interfaces. No fallback needed: the `clipper2` wrapper is not adopted.

## Alternatives considered

- **`i_overlay` for offsets as well, polylines only**: correct, but slower, arc-free (about 50 chords per arc before the toolpath stage needs them) and with the artefact in item 4. Kept as the fallback.
- **`clipper2` C++ wrapper**: not needed; the named fallback is not triggered. It would add a C++ toolchain to `openchuck-path`, against the library-family rule.
- **Offsetting successively (cheaper per pass)**: rejected, item 1; direct offsetting is already about 2 ms for 400 loops.
- **Repairing touching input silently**: rejected as the default; a hole touching the boundary is an input fault the user should see.

## Consequences

- **Slice 5**: `openchuck-path` declares the two crates; implements items 1, 2, 4 and 5; builds the oracle from the spike (kept as test fixtures, per the spike rule) into its tests; tests named after OP-04 and NFR-05 to NFR-07.
- **Slice 12 (OP-04) and slice 16 (OP-08)**: use the wrap approach in item 7 and the chording in item 6.
- **Architecture**: the edit in item 6 is made (section 7, 2D offsets row).
- **Work breakdown**: SP-7 Status updated in this change.
- **Spike fixtures**: the corpus generator and the oracle are worth keeping as slice 5 fixtures.

## Open items

- **OI-68**: verify in the LinuxCNC simulator (and ask whether the Z1 controller differs) that G2 and G3 cannot hold A in the arc plane, and that chorded moves replay inside the toolpath tolerance. Decide in slice 6. Needs the LinuxCNC runner (OI-47).
- **OI-69**: the silent-failure guards: the decimation tolerance for dense contours, the minimum gap for the input check (2e-5 mm in the spike), and the cheap result check. Decide in slice 5.
- **OI-70**: the seam: cutting pass loops at the seam, the three-tile cost, and the A wrap and unwind (with OI-06). Decide in slice 5 with slice 12.
