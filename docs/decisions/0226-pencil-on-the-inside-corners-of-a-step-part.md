# 0226: Pencil clean-up on the inside corners of a STEP part, from its exact edges (lane STEPALL-P; issue #257)

- **Status:** proposed by lane STEPALL-P (23 Oct 2026), awaiting the project owner. It builds the pencil part of [decision 0209](0209-step-geometry-for-every-strategy.md) item 2 (T, B-rep topology), step 3 of the order the owner chose (option A), and records the choices the build made that 0209 does not.
- **Date:** 23 Oct 2026.
- **Requirements:** OP-09, OP-23, IMP-02, NFR-06, GC-07, NFR-07, NFR-12.
- **Open items** ([docs/open-items.md](../open-items.md)): settles part (b) of OI-234 (part (a) stays open, so the row stays open). Opens OI-950 and OI-951. Depends on OI-81 (the exact drop, lane STEPALL-X) without closing it.

Numbering: the coordinator gave lane STEPALL-P record 0226 and OI-950 to OI-959. On 23 Oct 2026 the highest record on `main` was 0219 and on the unmerged branches 0232 (0227 is lane STEPALL-E's, 0230 to 0232 other lanes'), with 0226 unused anywhere; the highest `OI-` number in use was OI-982, with none from OI-950 to OI-959 used on any branch. No project schema number: nothing stored changes.

## Context

Pencil clean-up (OP-09; decision 0044 item 5) follows each concave crease of the part with one pass. It found the creases in the part's triangles (`pencil::concave_creases`): a mesh edge whose two triangles turn by at least the crease angle (30 degrees by default) and whose far corner lies outside the other's plane. On a STEP part those triangles are the import's tessellation (0.01 mm, 0.1 rad), so a crease could be a join between two facets of one curved face, and a real corner was judged by its facets, not its faces. Decision 0209 item 2 says that on a STEP part the concave B-rep edges are the real creases, and that a test needs a corpus part with one (OI-234 (b)).

Lane BUG-RESULTS (decision 0162 item 6, issue 217) made pencil check each move between two rests against a drop along it, so it rides over a blade tip rather than cutting through it. That stays as it is, with its tests.

Records read: 0044, 0053, 0056, 0057, 0066, 0162, 0209, 0210, 0211 (draft, lane STEPALL-M, unmerged), 0227 (lane STEPALL-E, unmerged). Open items read: OI-81, OI-229, OI-234, OI-830, OI-833.

## Decision

1. **The creases of a STEP part are its inside corners.** `BrepEdges::concave` keeps only the edges between two faces that turn inward, judged on the exact faces. `PencilFinishing::on_brep` follows them, at the operation's crease angle, chained as edge following chains its edges (joined end to end where exactly two meet, in a deterministic order). Boundaries, seams, edges of three or more faces and outside corners are never creases. On a mesh part nothing changes: `PencilFinishing::default()` reads the triangles as before.
2. **How an edge's side is told.** At up to 32 places along the edge, the places where `BrepEdges` already compares the faces' normals, the closest point of each face is found. Where the faces turn by more than 0.5 degrees (`SIDE_DEGREES`; below that they are tangent and have no side), the direction into each face is found: square to the edge in the face's tangent plane, the one of the two ways whose step of 0.05 mm, taken in the face's (u, v) from its derivatives, stays inside the face by its trim test. The edge is an inside corner there when the way into each face points out of the other face, along its outward normal; an outside corner when both point in; neither when they disagree. No extra closest-point search is made, so telling the side costs little beside finding the edges (item 6).
3. **An edge that is not an inside corner along its whole length is not followed.** An edge that is an inside corner in some places and an outside corner in others, or whose side cannot be told anywhere, is left out. The operation's notes say how many such edges turn by the crease angle, so the user knows a corner may be missing (NFR-12). Splitting such an edge where it changes is OI-950. None of the blisk's edges is one.
4. **Points and drops.** The points along a crease are the edge's polyline: the kernel's, at the model's tolerance, or the exact curve once lane STEPALL-E (decision 0227) lands, since both come through `BrepEdges`. The tool is still placed by dropping it on the tessellation (OI-81, lane STEPALL-X), and every move is still checked against a drop along it (decision 0162 item 6). The operation's note about the tessellation on a STEP part reads: "Pencil clean-up follows the STEP part's inside corners on its exact edges, and drops the tool on the part's tessellation, made at a deflection of 0.01 mm, not on its exact faces: dropping on the exact faces is not built yet." The pass note gains "along the part's edges". The import notice says that pencil clean-up finds its inside corners on the exact edges and drops the tool on the tessellation.
5. **Face sets as on an STL (OP-23).** Pencil clean-up takes no region on any part. On a STEP part a face set, boundary, avoid or check set is refused by the one check of decision 0210 item 5, before Generate and at generation, as it is now; the strategy refuses a region it is handed.
6. **Cost.** `BrepEdges` now examines its edges in parallel, each alone, and keeps them in model order (NFR-06); the edges it keeps and their order are unchanged, so edge following gets the same chains, faster. On the blisk, finding the edges takes 3.4 s and finding the inside corners 3.2 s (4 cores, release), where pencil's own pass takes 1.2 s. The application builds the corners each time a pencil operation on a STEP part is generated; keeping them per part is OI-951.
7. **The corpus part (OI-234 (b)).** A shouldered shaft, radius 8 to 12 mm along, a sharp shoulder out to radius 12, radius 12 to 24 mm, a shoulder in to radius 10, a quarter-round fillet of radius 2 tangent to it and to a bar of radius 8 from 26 to 40 mm. It has one inside corner (the shoulder at 12 mm, 90 degrees), four outside corners and two tangent edges. It exists twice: `openchuck_testkit::brep::shouldered_shaft`, built from closed forms with its fillet in a chosen number of facets, for the tests with no kernel; and `testdata/step/valid/shouldered_shaft.step`, written by OpenCASCADE 7.8.1 through `testdata/step/generate/make_fixtures.cpp`, for the kernel tests.
8. **Library version.** `PencilFinishing` was a unit struct, so it becomes a struct with a private field, no longer `Copy` or `Eq`: a breaking change. The workspace takes 0.19.0 (0.18.0 is lane STEPALL-E's), with a changelog entry in each crate (decision 0048). This follows `EdgeFollowing::with_source`, which takes its edges the same way; a second strategy type for STEP parts would duplicate the strategy.

## Measurements

`crates/openchuck-app/examples/pencil_creases.rs` (kernel; the command is in its header), on `blisk.step` from lane STEPALL-M's branch (SHA-256 `0e472ff5374c6bc40825a4f7e8885d69f6ac99e53fbb6dbc412104fadb55b646`, the owner's file), placed on the rotary axis from X 10 as lane STEPALL-M places it. Intel Xeon at 2.80 GHz, 4 cores, release build, cloud container, 23 Oct 2026.

**Creases.** Length within 0.05 mm of the other form's creases in the last column.

| Crease angle | Form | Creases | Length mm | Within 0.05 mm of the other form, mm |
| --- | --- | --- | --- | --- |
| 30, 20 and 10 | tessellation | 25 | 616.8 | 616.8 |
| 30, 20 and 10 | B-rep | 25 | 616.8 | 616.8 |
| 5 | tessellation | 71 | 709.3 | 616.8 |
| 5 | B-rep | 25 | 616.8 | 616.8 |
| 2 | tessellation | 1294 | 2696.2 | 616.8 |
| 2 | B-rep | 25 | 616.8 | 616.8 |

At the default 30 degrees, and down to 10, both forms find the same 25 corners of the blisk: its corners are sharp and its curved faces are finely tessellated. Below 10 degrees the tessellation adds false creases between facets (92 mm more at 5 degrees, 2,079 mm more at 2), and the B-rep adds none. No corner is missed by either form.

**The cut.** Pencil clean-up with the owner's 2 mm flat end, crease angle 30, largest step 0.5, on a 42 mm cylinder of stock with nothing cut (smallest material to clean 0, so every crease is followed). Judged by the testkit's independent checker (its outline only) at every fed pose and at the quarter, middle and three-quarter points of every fed move on the machine's straight move, against 5.3 million points on the exact faces 0.05 mm apart, inside their trims and along their trim loops.

| Form | Passes | Fed moves | Stations | Deepest into the exact faces mm | Stations over 0.01 mm | Time s |
| --- | --- | --- | --- | --- | --- | --- |
| tessellation | 31 | 1,869 | 7,476 | 0.0340 | 647 | 1.2 |
| B-rep | 32 | 1,753 | 7,012 | 0.0328 | 643 | 1.1 |

Both forms place the tool by the same drops on the tessellation, so the depth into the exact faces is the drop's (lane STEPALL-M measured 0.035 mm for pencil on form (b) and 0.003 mm on its finer form (c)); it is lane STEPALL-X's to remove (OI-81). The deepest point is the same place in both forms. What the B-rep form changes on the blisk is where the passes go at a crease angle under 10 degrees, not how deep they cut.

## Alternatives considered

- **Tell the side by searching the closest point of a probe on each side.** Built first. It gave the same answers but took 31 s on the blisk (four more closest-point searches at each place; a B-spline face's search is the cost). The step in (u, v) with the trim test needs none.
- **Concavity from the face orientation along the edge** (which face has the edge on its left in its loop). The model carries no edge use or loop direction (`BrepEdge` has its polyline and faces only), so this would need a change to the import.
- **A crease source port on `SurfaceSource`, answered by `BrepSurface`.** It would put the edges in `brep.rs` (lane STEPALL-X's file) and change a trait every surface implements; the strategy taking the model, as edge following does, keeps the change in `pencil.rs` and `brep_edges.rs`.
- **Splitting an edge that turns both ways at the place it changes.** Deferred (OI-950): no part in the corpus or the trials has one, and the notes say when one is left out.

## Verification

Library tests with no kernel, `crates/openchuck-path/tests/brep_pencil.rs`, on `shouldered_shaft` (all nine together 2.7 s uninstrumented, release build):

- `req_op_09_brep_the_only_crease_of_the_shouldered_shaft_is_its_sharp_inside_corner`: at 1, 20, 30 and 89 degrees only the shoulder; none above 90; one closed chain on the circle; edge following still keeps the outside corners.
- `req_op_09_brep_no_false_creases_between_the_facets_of_a_fillet`: with the fillet in four facets (22.5 degrees apart) the tessellation has creases on the fillet at 20 degrees; the B-rep form has one and cuts only at the shoulder.
- `req_op_09_req_nfr_12_brep_pencil_rests_in_the_corner_within_tolerance_without_gouging`: every tip within 0.02 mm of the closed form (on the bar, a tool radius from the shoulder; slice 15's criterion), and the ball's centre at least its radius less the tolerance from every exact face.
- `req_op_09_req_nfr_12_brep_an_edge_whose_side_cannot_be_told_is_left_out_and_said`: a damaged model's edge, listed between two faces it does not bound, is not followed, and the notes say so (item 3).
- `req_op_09_brep_a_part_with_no_inside_corner_says_so`, `req_op_09_brep_pencil_cuts_only_where_material_is_left`, `req_op_09_brep_pencil_takes_no_region`, `req_nfr_06_req_op_09_brep_pencil_is_identical_at_1_2_and_n_threads`, and `req_op_09_without_brep_edges_pencil_reads_the_tessellation_as_before` (the mesh form unchanged).
- In `brep_edges.rs`, `req_op_09_an_edge_that_turns_both_ways_or_cannot_be_told_is_no_inside_corner`.

With the inside and outside corners swapped, the five tests that depend on which edges are inside corners fail; the other three (the message on a part with no inside corner, the refused region, determinism) pass either way. The mesh tests of pencil (`openchuck-path` `tests/rest.rs`; `openchuck-sim` `tests/rest.rs`, including `req_op_09_req_nfr_12_pencil_never_cuts_through_a_tooth_between_two_rests` of issue 217) and edge following's B-rep tests pass unchanged.

Application, no kernel: `operation.rs` `req_op_09_req_op_23_pencil_on_a_step_part_follows_its_inside_corner_not_the_facets` (the STEP part cuts only at the shoulder and says so in its notes; the same part read as a mesh follows the fillet's facets); `openchuck-import` `req_op_23_req_imp_02_req_nfr_12_the_notice_says_which_strategies_use_the_tessellation` gains pencil.

Kernel (`--features kernel-tests`, `tests/step_strategies.rs`, the CI job "STEP kernel"): `req_op_09_req_imp_02_pencil_on_a_step_part_follows_its_inside_corner_without_gouging` (gates 1 and 2 on both posts, deepest 0.0000 mm into the exact faces by the independent checker, the same G-code at 1, 2 and N threads; one inside corner on the B-rep where the import's tessellation has 32 creases at 2 degrees; every cut on the bar a tool radius from it; 6 s in a debug build). The whole kernel set of the CI job passes in the lane's container (OpenCASCADE built with the repository's tooling).

Coverage, on the crate's own tests that reach the changed files (`cargo llvm-cov nextest`): `brep_edges.rs` 92.7 % of lines and `pencil.rs` 84.9 % before the test of the left-out note was added. The new lines not reached are the defensive branches of NFR-07 in `into_face` and `between` (a face whose parametrisation is singular, or whose closest point is not found) and two that `openchuck-sim`'s pencil tests reach. Of `pencil.rs`'s lines, the rest are earlier code reached by `openchuck-sim`'s tests. In `req_imp_02_req_op_03_req_op_09_roughing_then_rest_and_pencil_on_a_step_part_do_not_gouge`, pencil on the filleted box now must say it has no inside corner (all its edges are outside corners), where before it could either cut or say so.

OP-23 for pencil: face sets as on an STL (item 5) and the notes are covered; the depth into the exact faces on the blisk (0.033 mm) is over the tolerance and stays with the exact drop (OI-81).

## Consequences

- **Code:** `openchuck-path` (`brep_edges.rs`: `BrepEdges::concave`, `not_inside_throughout`, `SIDE_DEGREES`, edges examined in parallel; `pencil.rs`: `PencilFinishing::on_brep`; `strategy.rs`: the registry's `PencilFinishing::default()`), `openchuck-app` (`operation.rs`: pencil on a STEP part, its note; the example `pencil_creases`), `openchuck-import` (`step_tessellation_notice`), `openchuck-testkit` (`brep::shouldered_shaft`), `testdata/step/generate/make_fixtures.cpp` and `testdata/step/valid/shouldered_shaft.step`. Version 0.19.0.
- **Documents:** this record and the decision index; `docs/open-items.md` (OI-234 part (b) settled, OI-950 and OI-951 added); `docs/work-breakdown.md` (lane STEPALL's row: STEPALL-P built); `testdata/README.md` (the new fixture). `docs/requirements.md` and `docs/verification-plan.md` are unchanged: OP-09's and OP-23's rows already ask for this.
- **Merging with lane STEPALL-E:** both change `BrepEdges`' `EdgeData` and its build; whichever merges second keeps E's exact points and this record's `turn` field and parallel examination.
- **Open items:**
  - OI-950: an edge that is an inside corner along only part of its length, or whose side cannot be told, is not followed (the notes say so); split it where it changes.
  - OI-951: the inside corners of a STEP part are worked out again each time a pencil operation is generated (3.2 s on the blisk, beside 1.2 s for the pass); keep them per part in the application.
