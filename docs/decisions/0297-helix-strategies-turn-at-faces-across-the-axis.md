# 0297: Helix strategies turn once at each face across the rotary axis (OP-03, OP-04, OP-05; issue #384; lane HELICALFACES)

- **Status:** accepted (29 Oct 2026), by the project owner, given to the Merge Manager session and relayed by it (decision 0049). The owner chose option 1 of 4 ("always on, no new setting") on 29 Oct 2026, accepted both of the lane's refinements in item 3 and approved re-blessing the changed golden programs, each relayed by the coordinator session.
- **Date:** 29 Oct 2026.
- **Issue:** #384, "Faces still have excessive material left on helical operations".
- **Open items:** new OI-1200 (the simultaneous spiral refuses parts with sharp shoulders) and OI-1201 (face areas with nothing under the tool). No project schema change.

## Context

Helical finishing, continuous rotary roughing and the spiral pattern of simultaneous finishing all move along the rotary axis by one pitch (at most the stepover) each turn. A face perpendicular to the axis is reached only by the side of the tool, and only where a turn passes the X at which the side touches the face. Everywhere else the nearest turn is up to one pitch short, so each such face kept a helical step of material up to one pitch thick along X, varying round the circumference. This is a defect, not geometry: the straight side of a flat end mill reaches the whole face.

Earlier work did not cover it. Decision 0161 item 6 found the same step beside a held region and added, for roughing only, one turn without advancing in X at a held end. Decisions 0212 and 0252 cover the part's ends and outside the stock (extend past the stock, facing the unheld end). Decisions 0241 and 0242 cover operation boundaries and ramps. None covers faces inside the part's length.

**Measured before the change** (3.175 mm tools and stepover 2.2225 mm, the owner's cutting data; tolerance 0.01 mm). The measure takes points on each face and, for each one, its distance to the nearest tool surface over every fed pose. It uses the testkit's tool outline and none of the strategies' code.

- A stepped shaft (radius 6, then 10, then 7; faces at X 10 and X 25).
  - Helical finishing, flat end, no stock to leave: worst 2.07 mm, mean 0.80 mm, 95 % of the face over 0.02 mm.
  - Helical finishing, ball end: worst 2.38 mm.
  - Continuous roughing, stock to leave 0.3: worst 2.35 mm, mean 1.05 mm.
  - Axial finishing: worst 0.31 mm, mean 0.08 mm, the flank scallop between its stations.
- The owner's CrankShaft project (rotary roughing with 0.3 mm stock to leave, then simultaneous finishing). Its four faces across the axis inside its length (X 10, 15, 25 and 30) kept means of 0.58 to 1.21 mm and worsts of 2.12 to 2.33 mm. 94 to 98 % of each face was over 0.05 mm.

## Decision

1. **One whole turn at each face** (option 1, the owner's choice, always on). Helical finishing, continuous rotary roughing (its own helix, not Facing's Round form) and the simultaneous spiral add one whole turn of A at constant X at each face across the rotary axis that they meet. The turn is at the X where the side of the tool (grown by the stock to leave) touches the face, a tenth of the tolerance clear of it on the low side. It is the construction decision 0161 item 6 uses beside a held region. The indexed and axial forms are unchanged, and so is Facing (decision 0252).
2. **Finding the faces** (`openchuck-path::faces`). The strategy drops its own tool, grown by its stock to leave, along lines of X at angles all round the axis (spaced no more than the tool's radius at the part's largest radius, 36 to 720 lines, X samples no more than half the tool's radius apart). Where the side of the tool meets a face the drop's height jumps. A jump between neighbours is narrowed by bisection to 1e-6 mm; one still larger than the tolerance is a face. The samples of one face at different angles are one turn, taken furthest onto the low side.
   - A slope, however steep, rises by less than the tolerance over 1e-6 mm (within 0.006 degrees of square to the axis at the default tolerance) and is left to the helix.
   - A tool whose side is not straight up (a tapered or V tool) meets no jump and gets no turns.
   - A turn inside a held region's keep-out interval is not made.
   - The turn drops the tool at every point, so it never cuts the part. A face that covers only part of the circumference (a crank web) is cut where it is.
3. **Each strategy places the turn its own way.** Both are refinements the lane made to option 1 in building it: the proposal had one continuous cut everywhere.
   - **Helical finishing and the simultaneous spiral:** the helix or spiral is unchanged, the same seeds, fit and (for the spiral) smoothed A as before. Each face's turn is its own pass after it, fitted by the shared fitter with the tool across the rotary axis, and the shared services link the turns, so each turn adds a link in and a link out. The face's normal lies along the axis, which no A can follow, so the spiral's tilt from the normal means nothing at a turn.
     - Why not one cut: splicing the turn into the helix put the join a tenth of the tolerance from the face's jump, so the fitter crossed the jump at the join. That left single micro-moves the shared wall merge (decision 0050) does not absorb: 2 on the walls test part (`tests/walls.rs`), which allows none.
     - For the spiral, splitting it at the turns also moved where its A was smoothed. On the owner's CrankShaft the strategy then refused a move it had accepted before (0.0114 mm against 0.01 mm, at X 27.485, not at a turn).
   - **Continuous roughing:** the turn is in its helix, as at a held end (decision 0161 item 6), and is cut in every layer below the top of its face, a stepdown at a time. It is one cut per layer, as proposed.
     - The stock field is too coarse (cells of about a third of the tool's radius) to hold the thin step a helix leaves on a face. So at a turn the tool cuts where the face is (within one line of A of where the finder met it) wherever the layer is below the face's top there, even where the field shows nothing left. It does not cut a turn in the air round the parts of the circumference where the face is not.
     - Each turn starts on a node of the helix, so the helix meets it at a point of the path and not between two (where the straight move would cut the corner by up to 0.03 mm). The helix between turns is stretched by less than one node to suit.
4. **Said, not hidden.** An operation that turned at faces carries a note naming how many and their X, for example: "the part has 4 faces across the rotary axis: the tool turns once round the axis without moving along it at X 8.412, 16.589, 23.411 and 31.589, where its side touches each, so the face keeps only the stock to leave."
5. **Unchanged where there is no such face.** A part with no face across the axis gives no turn, and its path is made by exactly the code it was before. The golden programs and reference jobs are byte-identical. The result does not depend on the thread count (GC-07, NFR-06).

Options not chosen: (2) a setting, on for new operations and off for saved ones; (3) the same setting off by default; (4) no path change, only a note naming the faces and pointing to axial finishing.

## Consequences

- **Saved projects change.** A project whose part has faces across the rotary axis gets different paths and G-code from these three strategies when it is regenerated. The owner's CrankShaft is one: each operation gains four turns. A project whose part has no such face is unchanged.
- **Measured after the change.** The same independent measure, with each face point also classed by whether a vertical tool can reach it at all. That class is a test of its own: rays across a tool section 0.1 mm larger at the point, along its radius, against the mesh.
  - Stepped shaft: every face point is left at the stock to leave within the tolerance, by helical finishing (flat end, 0 and 0.3 mm), by a ball end beyond its own corner radius, and by roughing (0.3 mm). No point is cut deeper than the tolerance.
  - CrankShaft, on the committed tessellation (`testdata/stl/crankshaft.stl`), points the tool reaches: helical finishing worst 0.0096 to 0.0100 mm, the simultaneous spiral worst 0.0098 to 0.0100 mm, roughing worst 0.3098 to 0.3099 mm for 0.3 mm. None is cut deeper than 0.004 mm.
  - About 6 % of each CrankShaft face (2,190 to 2,920 points of 34,000 to 39,000) is covered by the crank's plate when seen along its radius, or too narrow for the tool beside it. No vertical tool reaches it in 4-axis.
  - About 2.5 % of each face (940 to 970 points) lies beside an off-axis journal with nothing under the tool for a drop to stand on (OI-1201).
  - **The owner's CrankShaft project** (STEP tessellation; its own roughing at 0.3 mm stock to leave and 1 mm stepdown, then simultaneous finishing; points every 0.1 mm, 195,000 to 225,000 per face). Same measure and classes, run on `main` and on this change, faces X 10, 15, 25 and 30, after both operations:

    | | Before | After |
    | --- | --- | --- |
    | Mean left | 1.08, 0.59, 1.25, 1.20 mm | 0.018, 0.018, 0.021, 0.018 mm |
    | Median left | — | 0.001 mm |
    | Share over 0.05 mm | 97.3 to 98.8 % | 5.9 to 6.7 % |
    | Share over 1 mm | 11 to 65 % | 0.0 % |
    | Worst where a vertical tool reaches | 1.72 to 2.27 mm | 0.0100 to 0.0180 mm |

    - After the change, 54 points of about 790,000 the tool reaches are over 0.01 mm, by at most 0.008 mm.
    - What stays over 0.05 mm is out of reach: covered from every radial view (about 5 %) or with nothing under the tool (about 2.6 %, OI-1201).
    - After roughing alone the worst where the tool reaches is 0.3100 mm for 0.3 mm.
    - The end faces carry the tabs and are Facing's (decision 0252), but the turns beside the tabs help them too: their means go from 1.23 and 0.59 mm to 0.30 and 0.24 mm.
    - Each operation turns at 7 faces: the four webs, the part's two ends beside the tabs, and one tab face that stands proud of the part's end.
- **Cost.** One extra turn per face per pass, and per layer for roughing; the finishing turns add a link in and out each. Finding the faces drops the tool a few thousand times per operation.
  - On the owner's CrankShaft (7 faces, 18 roughing layers), the posted Z1 program goes from 29,893 to 40,664 moves and from 3,150.8 s to 4,221.7 s of cutting (+34 %).
  - In the path's own times, roughing's cutting goes from 1,836 to 2,551 s and finishing's whole operation from 300 to 491 s.
  - Regeneration on four threads goes from 92 s to 118 s.
  - The gate 5 first-cut project (one face, at its tab) goes from 28.8 minutes to 30.8 minutes (1,849.5 s, LinuxCNC).
- **Library.** No public API changes. `Strategy::passes_with_notes` is overridden for helical finishing and rotary roughing so the note can be given. The behaviour change is recorded in the `openchuck-path` changelog.
- **Paths without faces.** The helix of helical finishing and the spiral of simultaneous finishing are made by the same code as before, so the moves before the turns are unchanged even on a part with faces. Only roughing's helix changes there, by the turns and the stretch between them.
- **Tests,** in `crates/openchuck-path/tests/helical_faces.rs`. Each failed before the change, except the no-face and determinism guards.
  - `req_op_04_helical_finishing_cleans_the_faces_across_the_axis`
  - `req_op_04_a_ball_end_leaves_only_its_corner_on_a_face`
  - `req_op_03_continuous_roughing_leaves_the_stock_to_leave_on_the_faces`
  - `req_op_04_helical_finishing_cleans_the_crankshaft_faces`
  - `req_op_03_continuous_roughing_cleans_the_crankshaft_faces`
  - `req_op_05_the_simultaneous_spiral_cleans_the_crankshaft_faces`
  - `req_op_04_one_turn_at_each_face_where_the_side_touches_it`
  - `req_op_04_a_part_with_no_face_across_the_axis_gets_no_turn`
  - `req_nfr_06_req_gc_07_face_turns_are_identical_at_1_2_and_n_threads`
  - Unit tests of the bisection, the clustering and the note in `faces.rs`.
- **Golden files.** Two committed jobs have faces across the axis, and their golden programs are re-blessed.
  - The blisk's helical programs (`testdata/golden/blisk/helical_2_*`, `helical_5_*`) are the old helix byte for byte plus 313 to 348 lines of turns.
  - The gate 5 first-cut programs (`testdata/golden/first_cut/*`) gain a turn at X 71.411 in both operations, at the part's end beside its tab.
    - Its finishing is the old helix plus the turn.
    - Its roughing differs from its first move: X moves by up to 0.001 mm where the helix is stretched to put each turn on a node, and each layer gains its turn.
    - LinuxCNC goes from 16,851 to 18,106 lines. The replay still matches the toolpath (tip 0.0007 mm, A 0.0005 degrees). The gate 3 chunks are cut from the program at run time.
    - **The program the owner physically ran on 24 Oct 2026 (gate 5, decisions 0221 and 0244) is no longer what this project generates.** It stays in the repository's history: `testdata/golden/first_cut/` at commit `0620cd0` (lane GATES, #284), unchanged up to `main` at `c8f1de1`. Decision 0244's recorded numbers are kept as they were and point here.
  - Every other reference job and golden file is byte-identical.
- **A move of A alone.** Roughing's lowest layer keeps the tool's tip at the rotary axis, so a turn there moves A alone. Decision 0076 item 2 holds such a move to A's own limit; on LinuxCNC its F word is in degrees per minute (`F2802.5` on the filleted box). `crates/openchuck-app/tests/feed_limits.rs` now checks F words on lines with no X, Y or Z against A's limit, and the rest against the linear limit as before.
- **Fixture.** `testdata/stl/crankshaft.stl` (100,284 bytes, 2,004 triangles) is the owner's CrankShaft as placed in its project: centred on the rotary axis, X 0 to 40, tessellated by the STEP import and written as binary STL. The owner agreed to add the model to the repository as a test piece.
