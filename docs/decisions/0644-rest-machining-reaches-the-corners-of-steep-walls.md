# 0644: Rest machining reaches the inside corners of steep walls, and says what its tool cannot reach (OP-09, OP-20, SET-09, GC-07, NFR-06, NFR-07, NFR-12; issue #508; lane RESTCORNERS)

- **Status:** proposed by lane RESTCORNERS on 2 Nov 2026. The approach (D1, option 1: rings at walls from the remaining material, with the unreachable report and the wording of the note) is the owner's choice, relayed by the coordinator (decision 0049); that is not acceptance of this record.
- **Date:** 2 Nov 2026.
- **Issue:** #508, material left after rest machining in sharp inside corners that a flat end mill should reach.
- **Builds on and amends:** [0044](0044-slice-15-rest-and-pencil.md) (rest machining; item 3, the reach of a wanted point, is amended by item 4 below), [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (a turn at each face square to the axis), [0420](0420-rings-for-rest-roughing-and-round-facing.md) (rings, cut only where material is left), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (the keep-out beside a held region), [0368](0368-simultaneous-finishing-of-hidden-surfaces.md) (the report of what a tool cannot reach, OP-20). Related: decision 0633 on lane SIMVIS (material enclosed for every tool; accepted, not yet on `main`) reports material no tool of the machine reaches; this record reports what *this operation's* tool does not.
- **Numbers:** 0644 and OI-2003, the next free across `main` and every unmerged branch on 2 Nov 2026, confirmed by the coordinator; renumbered at merge if `main` has moved. No project schema number. Library 0.42.0 (assigned by the coordinator; applied at the release push).

## Context

The owner's Baluster project (kept outside the repository; only numbers here) roughs with a 3.175 mm flat end, finishes with simultaneous finishing and a 3.175 mm ball, then runs rest machining with the same flat end in rings (stepover 2.2225 mm from the tool's cutting data, smallest material to clean 0.05 mm). After simulating (cell 0.1 mm), grouped by X:

| Place (owner's part, scale 1.15) | Left before rest | Left after rest, before this record |
| --- | --- | --- |
| Inside corner of the collar and the bead (X 14 to 15; the bead's foot is a wall 79 degrees from the axis) | 0.54 mm | 0.54 mm |
| Walls of the cove (X 21 to 22) | 0.27 and 0.22 mm | 0.27 and 0.22 mm |
| Lip of the cove into the vase (X 28) | 0.27 mm | 0.26 mm |
| Shoulder square to the axis (X 11.5) | 0.42 mm | 0.02 mm |

The repository's Baluster example (`testdata/examples/baluster`, unscaled) with a rest operation of the same settings gives the same picture: 0.59 mm at the collar and bead corner, 0.26 mm on the cove walls.

Three causes, found on both:

1. **Rings at the stepover only.** A tool across the rotary axis meets a wall with its side, at one place along X for each point of the wall. Rest machining's rings are evenly spaced at the stepover, plus one at each face square to the axis (0297), whose search passes over "a slope, however steep". So the corner at a steep wall's foot is cleaned only if a ring happens to lie where the side touches it, and between rings a flat end leaves about the spacing times the wall's slope. Rest with a 0.2 mm stepover took the corner from 0.54 to 0.15 mm: the corner is reachable, the passes were not placed for it.
2. **The reach of a wanted point.** A point of a ring is cut when material lies within half a stepover and a sample spacing of the tool's contact point (0044 item 3). A flat end on a floor touches it anywhere under its flat bottom, so the contact can be a whole tool's width from the material the tool would cut, and the rings nearest a wall's foot were mostly not cut (37 of 181 moves on the test's corner ring).
3. **No report.** Rest machining never filled `StrategyOutput::unreached`; its note said "N passes clean them".

## Decision

1. **Rings at walls (the owner's D1, option 1, relayed).** For each sample of the remaining-material map whose normal leans along X by more than about a degree, the tool's profile (grown by the stock to leave) gives the point of its edge whose normal matches the sample's (`RevolvedProfile::point_for_normal`), and so the X of the tool's axis at which that edge touches the sample: the cutting radius from it for a flat end, less for a ball or a corner radius, plus a tenth of the toolpath tolerance clear of the wall (as 0297's turns). A ring there, or up to `min_remaining / slope` further from the wall (slope: the share of the normal along X), leaves no more than about `min_remaining` on the wall; the range is never narrower than the larger of the tolerance and 0.01 mm, nor wider than half the stepover. The fewest rings that put one in every sample's range are found by stabbing the ranges in order of their far end, which puts a ring exactly at each wall's foot. The rings are added to the rings at the stepover and the faces' turns (in the spiral form, as whole turns, as the faces' turns are), and cut, as every rest pass is, only where material is left (0420 item 2). A ring within the smallest spacing of a face's turn is not added again.
2. **A sample stands for its patch of wall.** The map samples the part about `min(stepover, tool radius) / 2` apart, so a wall's lowest sample can lie that far above its foot. Each wall sample asks for places of the tool across the patch of wall within one sample spacing of it (no more than 64 per sample), so the foot is reached.
3. **A ring at the edge of a held region's keep-out.** Beside a held region the rings at the stepover stop short of the keep-out (0161). A sample on a surface round the axis within reach of the tool's flat of a keep-out edge asks for a ring on that edge, the nearest the tool may come, as at a wall's foot.
4. **The reach of a wanted point includes the flat (amends 0044 item 3).** A point of a pass is cut when material lies within half a stepover, one sample spacing **and the width of the tool's flat bottom** (twice its radius; 0 for a ball) of its contact.
5. **What the tool cannot reach is reported (OP-20's report, NFR-12).** The samples with material left are taken in cells 0.25 mm along X and round the axis, one kind of place to a cell (a surface round the axis, a wall facing either way, the rest), and one sample of each cell answers for all of them: the report is of areas, and a drop per sample would cost as many drops as the map has samples (1.7 million on a part tessellated finely, which took a rest test from 17 s to 69 s). That sample is tried at the place that should reach it: a wall sample at its ring's X, a sample round the axis at nine places under the tool's flat. The tool is dropped there on the whole part (held regions and tabs included), and if it stays further from the sample than `min_remaining` plus the tolerance at every place tried, the sample is not reached, because:
   - **Too narrow:** the part holds the tool up, but the sample is seen from outside along its radius (a groove narrower than the tool, or a round bottom a flat end cannot follow);
   - **Blocked:** the sample faces the rotary axis, or something of the part is between it and the outside;
   - **Too near a held region** (`UnreachedReason::NearHeldRegion`, new): every place that would reach it is inside a keep-out interval. Samples under the grip itself (inside the keep-out shrunk by the tool's own largest radius and the held clearance, ends included, so the held region's own faces are left out too) are not reported: the grip holds them by design, and the simulation does not count them (decision 0194). The held clearance, 0.5 mm, moves from `openchuck-app` to `openchuck_path::HELD_CLEARANCE` so both use one value.
   The samples are grouped by reason and by nearness along X (four sample spacings, at least 0.5 mm) into areas with their X range, A range (all round when no gap between their angles is wider than 90 degrees) and area, largest first, in `StrategyOutput::unreached`, and one warning lists the largest five with where, how large and why, in simultaneous finishing's words (one shared list), and ends: "Check them in the simulation; a smaller tool or another operation may reach them. Generated G-code must be verified by the user before it is run on a machine."
6. **Wording (NFR-12).** The note says "N passes cut where it is left", not "N passes clean them", and a note counts the rings added at walls, where they are and how close: "rest machining added N rings between X a and b, as close as d mm apart, where its tool's side meets a wall with material left, so that a steep wall keeps about t mm or less between them".
7. **Limits (NFR-07).** At most 10,000 rings at walls; past it the rest are not added and a warning says so, naming `min_remaining`. The work is bounded by the map's samples (at most 2 million, 0044 item 1) times 64 places and nine drops.
8. **A tool the profile cannot model.** For a profile that is not convex no edge point is known: no rings are added at walls, nothing is reported as unreachable, and a note says so.
9. **Determinism (GC-07, NFR-06).** The samples come in the map's fixed order, the rings are sorted, and the drops run through `parallel::map_in_order`, so the path and the report are the same at any thread count.

## What it costs

The rings at walls are as many as the walls with material need at the user's threshold. On the repository's Baluster with its rest operation: at `min_remaining` 0.05 mm, 218 rings and 36,533 moves (3,390 before); at the default 0.15 mm, 20 rings and 5,148 moves.

## Alternatives considered

- **Adaptive spacing for rest machining (D1 option 2; decision 0239):** not chosen. Its measure is not rest-aware: it would refine wherever the rest passes do not cut, which is most of the part, and it needs a project schema change for its switch.
- **Only a warning recommending a smaller stepover (option 3):** not chosen; it does not reach the corners.
- **A finer remaining-material map:** would place the foot exactly but costs every rest operation and every pencil operation; the patch of item 2 does it from the samples there are.

## Not covered

- **The tip of a dome that runs out to the axis at a free end of the part** (the Baluster's finial: 0.2 mm left on surfaces facing along X within about 1.3 mm of the axis): the places that reach it lie beyond the part's end, where the free ends' layers go (decision 0344); neither rings nor the report go there. OI-2003.
- **"Contact feed 0.0 mm/min" warnings** on the helical family's moves near the axis (269 on the repository's Baluster with rest, 72 of them in helical finishing): not this issue; the coordinator files it separately.

## Verification

Tests, each written before the code and watched to fail (four of the first eight failed on the code before this record; the held-region strip test was added with item 3), run uninstrumented with `cargo nextest run`:

| Check | Test | Result |
| --- | --- | --- |
| A steep wall's inside corner (80 degrees), after a 3.175 mm ball, cleaned by a 3.175 mm flat end with the owner's stepover and threshold, in rings and in a spiral; nothing reported | `crates/openchuck-sim/tests/rest_corners.rs`: `req_op_09_rest_reaches_the_inside_corner_of_a_steep_wall_in_rings`, `..._in_a_spiral` | 0.642 mm to 0.094 mm (threshold 0.05 plus one 0.1 mm cell), no overcut, no rapid cut |
| The near-vertical walls where a bead runs into a cove (ball passes 0.45 mm apart, as the owner's) | `req_op_09_rest_cleans_the_walls_of_a_cove_with_a_flat_end` | 0.267 mm to 0.107 mm; the cove's round bottom, which a flat end stands 0.45 mm above, is reported too narrow, and only there |
| A V groove narrower than the tool is reported | `req_op_09_rest_reports_a_groove_too_narrow_for_its_tool` | Too narrow, all round, within X 11.4 to 12.6, in the warning with the NFR-12 sentence |
| A corner the held region keeps the tool from is reported | `req_op_09_req_set_09_rest_reports_a_corner_the_held_region_keeps_the_tool_from` | Too near a held region, up to the shoulder at X 12 |
| Beside a held region the tool cuts up to its keep-out, and the strip it cannot reach is reported; the grip is not | `req_op_09_req_set_09_rest_cuts_up_to_a_held_region_and_reports_the_strip_it_cannot` | 0.046 mm left from X 3.5 on; one area X 3 to 3.5 all round |
| No wall, no wall ring | `req_op_09_rest_adds_no_wall_ring_where_no_wall_holds_material` | No note, nothing reported |
| The note does not claim the material is cleaned | `req_op_09_rest_does_not_claim_its_passes_clean_the_material` | "cut where it is left" |
| Identical at 1, 2 and N threads | `req_nfr_06_req_op_09_rest_wall_rings_are_identical_at_1_2_and_n_threads` | Rings, spiral and the report |
| Areas: all round, across zero, split by reason and distance | `crates/openchuck-path/src/rest/walls.rs` unit tests | |

Timed one at a time (`cargo nextest run --test-threads 1`, debug build): the new tests take 1 to 6 s, their determinism test 29 s. The existing rest tests stay near their times on `main` (the rings determinism test 9 s against 3 s, rings against the spiral 18 s against 17 s, the simulated rest determinism 19 s against 18 s), after the per-cell reach check of item 5; before it they took 128, 69 and 72 s. No test takes a minute, so none is listed in `.config/nextest.toml`. The decision 0420 test `req_op_09_req_gc_07_rest_spiral_is_unchanged_when_rings_are_not_chosen` now allows the spiral's whole turns at walls (item 1), within a tool radius and a sample's patch of the flank, with the note that counts them.

Measured through the whole pipeline (release build, `openchuck-cli simulate` and `cargo run --release -p openchuck-app --example rest_corners`, cell 0.1 mm), most material left by place:

| Place | Repository Baluster with rest: before | after | Owner's project: before | after |
| --- | --- | --- | --- | --- |
| Collar and bead corner | 0.59 mm | 0.09 mm | 0.54 mm | 0.08 mm |
| Cove walls | 0.26 mm | 0.10 mm | 0.27 mm | 0.10 mm |
| Cove lip into the vase | 0.26 mm | 0.09 mm | 0.26 mm | 0.09 mm |
| Shoulder beside the grip | 0.92 mm, not said | 0.92 mm, reported too near a held region | (no such shoulder) | |

No new overcut on either. The figures are refreshed in the pull request with the final build.

## Consequences

- **Library (0.42.0, decision 0048):** `openchuck-path` gains `rest::Sample`, `RemainingMap::samples_with_normals`, `HELD_CLEARANCE` and the enum variant `UnreachedReason::NearHeldRegion` (breaking for an exhaustive `match` outside the crate). Rest machining's output changes wherever a wall holds material, a held region is near, or the tool has a flat bottom (item 4), so the workspace minor version is bumped (0239 item 13). Spiral and Rings, the parameters and the project format are unchanged; no golden program uses rest machining.
- **Application:** `openchuck_app::HELD_CLEARANCE` is the library's value.
- **Documents:** the verification plan's OP-09 row, the work breakdown (lane RESTCORNERS), the open-items register (OI-2003), the decision index, the changelogs of `openchuck-path` and `openchuck-app`.
- **Open item:** OI-2003, the tip of a dome at a free end (Not covered).
