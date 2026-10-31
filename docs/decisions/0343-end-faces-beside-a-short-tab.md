# 0343: Finishing faces the part's ends beside a short tab (OP-04, OP-05, OP-09, OP-21, SET-09; issue #404; lane ENDFACE)

- **Status:** accepted (30 Oct 2026) by the project owner, relayed acceptance (decision 0049) by the coordinator session; proposed the same day by lane ENDFACE. The owner chose option 2 for axial and indexed finishing (OI-1371), relayed by the coordinator session (decision 0049). The owner approved the re-blessed `cone_indexed` golden programs on 30 Oct 2026, relayed the same way. The rest of the record awaits the owner's acceptance.
- **Date:** 30 Oct 2026.
- **Issue:** #404, "Helical finishing (and rest machining) leave about 0.5 mm of material on the far end face of the blisk".
- **Builds on:** [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (one turn at each face across the axis), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (the keep-out beside a held region), [0252](0252-facing-ends-sides-and-extents.md) (Facing).
- **Open items:** OI-1371 (axial and indexed) closed by the owner's option 2. OI-1372 is new: simultaneous finishing refuses the owner's blisk project beside its held start. OI-1370 (an end with nothing beyond it) is decided separately in decision 0344.

## Context

The owner's blisk project (STL, numbers only):
- Box stock X 0 to 60. The part is at X 9.175 to 59.175, with a 5 mm held start and a 4.175 mm tab of diameter 10 at the start. The far end has a tab of diameter 14.5 that is only 0.825 mm long (X 59.175 to 60), and no held end.
- Rotary roughing (flat 3.175 mm, 0.5 mm stock to leave), then helical finishing (flat 3.175 mm, stepover 1), then rest machining.

Material left on the end faces, outside the tabs, from the stock simulation (0.1 mm cell), mean / max:

| After | Start end (X 9.175) | Far end (X 59.175) |
| --- | --- | --- |
| Roughing | 0.49 / 1.01 mm | 0.53 / 0.73 mm |
| Roughing, helical finishing | −0.03 / 0.03 mm | 0.43 / 0.73 mm (96 % over 0.05 mm) |
| All three | −0.03 / 0.03 mm | 0.43 / 0.73 mm |

The −0.03 mm is the simulation's own error at the cell size, seen at both ends.

**The cause.** Helical finishing looked for faces across the axis only over the target's own extent along X: the part, its tabs and its held regions.
- The turn that faces an end face lies one tool radius beyond the face, at X 60.764 for the far end here (59.175 + 1.5875 + a tenth of the tolerance). That is past the end of the 0.825 mm tab, so the turn was never searched for, and it would have been dropped if found.
- Inside the searched range the tool still overlaps the part, so the drop height shows no jump there.
- The start end was faced only because the target there starts at the held region (X 0).
- Roughing faced the far end because its range runs past the stock.
- The same gap affected the simultaneous spiral. Rest machining made no turns at all, since decision 0297 covered only three strategies.
- Axial finishing's passes stop at the target's end, and indexed finishing's do too, so neither reached the far face. Pencil clean-up follows creases only.

## Decision

1. **The face search runs on past each end of the target (helical, simultaneous spiral, rest, axial).**
   - The search keeps its sample spacing and continues for whole samples past each end, covering the tool's radius (grown by the stock to leave), the clearance and one sample more. The samples inside the target's range are the ones taken before, so every face found before is found at the same X.
   - Past a free end the tool has nothing to drop on, so no new face appears there.
   - Roughing keeps its own range, which already runs past the stock.
2. **Rest machining turns at the faces** as helical finishing does (decision 0297), each turn cut only where the remaining-material map says material is left.
3. **Axial finishing turns at an end face whose turn lies past the target's ends** (owner's option 2): one whole turn of A at constant X, the construction of decision 0297. Faces inside the target's length are left to the passes as before, so a part without such an end face is unchanged.
4. **Indexed finishing's passes run on past each end of the target** by the grown tool's radius, less a tenth of the tolerance, so that the first point still stands on the target (owner's option 2).
   - The stretch past each end is fitted on its own and joined to the pass over the target, which is fitted as before, so its points do not move.
   - A stays fixed at each position: no cutting angle and no A move while cutting is added (the owner's choice for indexed finishing in FINSTEP).
   - At a fixed A the tool's round side meets an end face only along the pass line: within the tolerance only for points up to sqrt(2 r tol) off it (0.18 mm for a 3.175 mm tool at 0.01 mm). Between the positions the face keeps what earlier operations left, and the note says so.
5. **Protections hold.**
   - No turn is made, and no pass is cut, inside a held region's keep-out interval (SET-09, decision 0161).
   - A turn's drops land on a tab, never in it, so tabs are never cut (OP-21). Every new test checks the tab surfaces.
6. **Notes** (OP-04 notes, decision 0114 wording):
   - **A turn past an end:** carries decision 0297's note, which names the face's X.
   - **A held region keeps the tool from a face:** "the tool does not turn at a face across the rotary axis at X 31.589: there it would reach over the gripped length of the stock, so that face keeps what earlier operations left".
   - **Indexed:** "the passes run on past both ends of the part by up to the tool's radius, so its side faces each end face where the target stops short of it, along each position only: between the positions the end face keeps what earlier operations left".
   - **Pencil clean-up** (unchanged by design): its note adds "it follows creases only and leaves the part's end faces as earlier operations left them: face them with helical finishing, rest machining or Facing".

## Consequences

**Measured after the change** on the owner's project (same measure; roughing, then each operation):

| Operation | Start end, before → after | Far end, before → after |
| --- | --- | --- |
| Helical finishing | −0.03 / 0.03 mm, unchanged | 0.43 / 0.73 → 0.03 / 0.04 mm |
| Rest machining | 0.36 / 1.01 → −0.03 / 0.03 mm | 0.43 / 0.73 → 0.03 / 0.04 mm |
| Axial finishing | −0.01 / 0.08 mm, unchanged | 0.43 / 0.73 → 0.03 / 0.04 mm |
| Indexed finishing | 0.44 / 0.97 mm, unchanged | 0.52 / 0.73 → 0.44 / 0.72 mm; within 0.02 mm along its four lines |
| Pencil clean-up | 0.49 / 1.01 mm, unchanged | 0.49 / 0.73 mm, unchanged (by design; note) |
| Simultaneous finishing | refused before and after: "would cut 7.5000 mm into the surface" at X 6.588 beside the held start (OI-1372) | |

**Golden files.**
- Helical, axial, rest and simultaneous: every golden program and reference job is byte-identical.
- Indexed: `testdata/golden/reference/cone_indexed_linuxcnc.ngc` and `cone_indexed_makera-z1.nc` change.
  - Each of the four passes keeps its points over the cone and gains about 16 points at each end, where the 3 mm ball rolls over the cone's end edge (X −1.499 to 0 and 30.0 to 31.499).
  - The entries and exits move with the pass ends.
  - A few F words at the joins change, because the contact-feed compensation sees the new neighbours.

**Saved projects** change on regeneration where a finishing operation now reaches an end face (helical, simultaneous, rest, axial), and for every indexed operation (its passes run past the ends).

**Determinism.** The extra samples, turns and stretches depend only on the part, the tool and the parameters, so the result does not depend on the thread count (GC-07, NFR-06). This is tested at 1, 2 and N threads for axial and indexed.

**Library.** No public API change; recorded in the `openchuck-path` changelog. No project schema change.

**Tests** (`crates/openchuck-path/tests/helical_faces.rs` unless noted). Each measures the end faces independently, with the testkit's tool outline over every fed pose, on a cylinder with a 0.8 mm tab of radius 7.25 at each end. Each failed before the change except the determinism guard.
- `req_op_04_req_op_21_helical_finishing_faces_the_ends_beside_a_short_tab` (2.70 mm left before)
- `req_op_05_req_op_21_the_simultaneous_spiral_faces_the_ends_beside_a_short_tab` (2.72 mm before)
- `req_op_09_req_op_21_rest_machining_faces_the_ends_beside_a_short_tab` (2.70 mm before)
- `req_op_04_req_op_21_axial_finishing_faces_the_ends_beside_a_short_tab` (2.74 mm before)
- `req_op_04_req_op_21_indexed_finishing_faces_the_ends_beside_a_short_tab` (2.70 mm before; along the pass lines)
- `req_set_09_req_op_04_a_face_the_held_region_keeps_the_tool_from_is_named`
- `req_nfr_06_req_gc_07_end_faces_beside_a_short_tab_are_identical_at_1_2_and_n_threads`
- `req_op_04_req_op_09_pencil_says_it_leaves_the_end_faces` (`tests/brep_pencil.rs`)
- `faces::tests::req_set_09_the_held_note_says_which_faces_keep_their_material` (unit)

NFR-12: the generated G-code remains the user's to verify before it is run.
