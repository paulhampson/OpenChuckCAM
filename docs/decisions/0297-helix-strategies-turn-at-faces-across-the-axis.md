# 0297: Helix strategies turn once at each face across the rotary axis (OP-03, OP-04, OP-05; issue #384; lane HELICALFACES)

- **Status:** proposed. The owner chose option 1 of 4 ("always on, no new setting") on 29 Oct 2026, relayed by the coordinator session (decision 0049); the record stays proposed until the owner accepts it.
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
     - The stock field is too coarse (cells of about a third of the tool's radius) to hold the thin step a helix leaves on a face. So at a turn the tool cuts wherever the layer is below the face's top, even where the field shows nothing left.
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
  - The owner's project numbers are in the pull request.
- **Cost.** One extra turn per face per pass, and per layer for roughing. Finding the faces drops the tool a few thousand times per operation.
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
- **Fixture.** `testdata/stl/crankshaft.stl` (100,284 bytes, 2,004 triangles) is the owner's CrankShaft as placed in its project: centred on the rotary axis, X 0 to 40, tessellated by the STEP import and written as binary STL. The owner agreed to add the model to the repository as a test piece.
