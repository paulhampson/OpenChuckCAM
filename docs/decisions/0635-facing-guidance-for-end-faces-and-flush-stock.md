# 0635: Helical finishing set up to face the ends: two notes that point at Facing (OP-04, OP-24, NFR-12; issue #471; lane LARGEPATH)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049), unchanged, after reading it in full. Proposed the same day by lane LARGEPATH; the owner agreed both notes and the reworded Facing End refusal on 1 Nov 2026, relayed the same way.
- **Date:** 2 Nov 2026.
- **Lane:** LARGEPATH, pull request 4.
- **Issue:** #471 (helical finishing set up to remove excess material at the ends of the part does not give the expected facing result).
- **Requirements:** OP-04 (helical finishing), OP-24 (Facing), NFR-12 (the notes do not say the G-code is safe; generated G-code remains the user's to verify before running it on a machine).
- **Amends:** nothing in the documents. Adds to [decision 0344](0344-finishing-stepdown-and-free-ends.md) item 9 (the notes of a free end) and [decision 0252](0252-facing-ends-sides-and-extents.md) (End facing's refusal when the stock ends at the part's end).
- **Numbers:** 0635, the next free number on 2 Nov 2026 (0631 and 0634 are this lane's pull requests 1 and 3; 0632 and 0633 are held by lanes PATHBUGS and SIMVIS); renumbered at release if needed. No project schema number; no library version change (internal; no API change).

## Context

The owner's carving project (kept outside the repository) has a helical finishing operation, in rings, over a selection of the part's end faces, added to remove the excess material at the ends. It did not give the facing the owner expected.

The lane's diagnosis (sent to the owner through the coordinator and posted on #471):

- The project was set up correctly for what each operation does; it is a setup expectation gap, not a bug.
- The stock ends exactly at the part's far end, so there is no material past that end for any operation to remove.
- Helical finishing (spiral or rings) is not a facing operation. It cuts with the tool pointing at the rotary axis, so it reaches a face across the axis only with the side of the tool: one turn where the part lies on both sides of the face (decision 0297), or, with Face free ends on, a Stepdown and stock past the face, one ring a tool diameter wide down to the axis (decision 0344), which is a groove, not a facing.
- On the project, the helical operation turned at the chuck-side end face (as roughing already did) and did not turn at the tab's chuck-side face (it would reach the gripped length). The stock simulation shows it changes nothing at either end.
- To skim the far end: set the stock longer than the part at that end by as much as the real blank is longer, and add Facing with Shape End (decision 0252). The owner reports that with a bigger stock and rest machining the far end now faces as expected.

Nothing in the application said any of this. The owner agreed two notes (N1, N2), and N2 includes rewording End facing's refusal when the stock ends at the part's end.

## Decision

1. **N1: a region of faces across the axis.** When helical or axial finishing is limited to a selected region and 99% or more of the region's area lies on faces whose normal is within 10 degrees of the rotary axis, the operation notes:
   "the selected faces lie across the rotary axis. {Helical finishing | Axial finishing} cuts with the tool pointing at the axis, so it reaches such a face only with the side of the tool, in one turn where the part lies on both sides of it. To face the end of the part or the stock, use Facing with Shape End"
   The wording agreed with the owner said "cuts round the part"; that is true of helical finishing but not of axial finishing, which cuts along the part, so "cuts with the tool pointing at the axis" is used for both. Rest machining cannot be limited to a region yet, so it has no N1.
2. **N2: no stock past a free end.** With Face free ends on, when the free-end search finds an end of the part that the stock does not carry on past (within 0.001 mm), helical, axial and rest finishing note:
   "the stock ends at the part's end at X {x}: there is no stock past this end to face. If the real blank is longer, set the stock's length to match it"
   Before, such an end was left without a word.
3. **N2 for Facing End.** End facing refused such a setup with "End facing: the target end plane is outside supplied stock". When the stock ends at or inside the part's end (within the tolerance), the same check now refuses with "End facing: the stock ends at the part's end at X {x}: there is no stock past this end to face. If the real blank is longer, set the stock's length to match it". Other cases keep the old words.
4. **Unchanged:** every path and every G-code file; both notes are information, not warnings, and block nothing.

## Verification

- `req_op_04_a_region_of_faces_across_the_axis_gets_the_facing_note` (`crates/openchuck-path/tests/helical_faces.rs`): a closed bar's end faces as the region give N1 for helical and axial finishing; its round surface as the region gives none. It failed before the change.
- `req_op_04_a_free_end_flush_with_the_stock_says_there_is_nothing_past_it`: the free-end bar whose stock ends at its far end gives N2 for that end, not the other, for helical, axial and rest finishing. It failed before the change.
- `req_op_24_facing_an_end_flush_with_the_stock_says_there_is_nothing_past_it` (`crates/openchuck-path/tests/facing_end_plan.rs`): End facing of a part whose stock ends at its far end refuses with N2's words. It failed before the change (with the old words).

## Consequences and limits

- `crates/openchuck-path`: `finishing.rs` (`free_ends` gives N2; `across_axis_note`), `helical.rs` and `axial.rs` (N1), `facing/end.rs` (the refusal's words).
- N2 is given for any free end flush with the stock while Face free ends is on, including on parts that never meant to face that end; it is one information line, and turning Face free ends off removes it.
- N1 reads the region's triangles once per generation, a cost in proportion to the region.
