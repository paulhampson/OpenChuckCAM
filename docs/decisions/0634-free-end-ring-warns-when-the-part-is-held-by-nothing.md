# 0634: A free-end ring that would part the part from the chuck is warned about (SET-09, NFR-12; issue #477; lane LARGEPATH)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049), unchanged, after reading it in full. Proposed the same day by lane LARGEPATH; the owner chose option A on issue #477 (warn, and still generate) on 1 Nov 2026, relayed the same way.
- **Date:** 2 Nov 2026.
- **Lane:** LARGEPATH, pull request 3.
- **Issue:** #477 ("SAFETY: the free-end ring can part the PART from the chuck").
- **Requirements:** SET-09 (keep clear of what holds the stock), NFR-12 (generated G-code remains the user's to verify before running it on a machine), OP-04, OP-09.
- **Amends:** [decision 0344](0344-finishing-stepdown-and-free-ends.md) item 8 (the cut-free warning) and its "Risks, and what limits them" (a loose piece). 0344 is not edited; this record adds to it.
- **Numbers:** 0634, the next free number on 2 Nov 2026 (0631 is this lane's pull request 1; 0632 and 0633 are held by lanes PATHBUGS and SIMVIS), by the owner's rule that numbers are as monotonic as possible; renumbered at release if needed. No project schema number; no library version change (an internal change to `openchuck-path`: a new warning, no API change).
- **Open items:** OI-1393 stays open (a test that what is past a ring is joined to a held region, or a limit to the depth); this record narrows it.

## Context

Decision 0344 faces a free end (an end of the part with nothing of the target past it) with a ring cut in Stepdown layers down to the rotary axis, and warns when the stock past the ring is not gripped by a held region past it (item 8). Its hazard analysis considered only that stock coming loose.

Lane LARGEPATH, investigating issue #471, found the other side: when a held region past the ring grips the stock (the chuck), and nothing holds the part on its own side (its other end is the stock's end, with no held region there), the last layer parts the **part** from the stock the chuck holds, and no warning was given. The test `req_set_09_stock_past_a_free_end_that_a_held_region_grips_gets_no_cut_free_warning` asserted that silence. Decision 0344's own measurement setup (a chuck holding 10 mm, raw stock between it and the part's free end) is this case. Not seen on a machine; the owner's carving project is not affected (its chuck end has a tab, so it is not a free end).

## Decision

1. **The part's side of each ring is checked.** After the free ends are faced, each ring down to the axis is checked on the part's side: the part stays held only if a held region (a keep-out interval of the setup) lies on that side with no other ring down to the axis between them. Rings at both free ends therefore leave the part held by nothing, and both are warned about. An end beside a tab is never a free end (0344 item 4), so tabs need no separate test.
2. **The warning (the owner's option A),** in the style of the cut-free warning of 0344 item 8, on the Face free ends setting, not requiring acknowledgement; the program is still generated:
   "the end face at X {face} is faced down to the rotary axis: the tool cuts a ring {width} mm wide from X {from} to {to} down to the axis, and nothing in this setup holds the part on the other side of the ring (no held region at its other end, and no tab joining it to one), so the part can be cut free there and thrown while the rotary axis turns. Before running this program, check the stock and the workholding on the machine"
3. **Unchanged:** the path (no move changes; the G-code of every project is the same), 0344 item 8's warning about the stock past the ring, and the refusal of a face deeper than the flutes.

## Alternatives put to the owner

- B: refuse the operation in that case, as with the flute check (OI-1396).
- C: turn only as deep as the stock past the face is already clear (0344's OI-1390 option 1), which never parts anything.

The owner chose A.

## Verification

- `req_set_09_a_free_end_ring_that_would_part_the_part_from_the_chuck_is_warned` (`crates/openchuck-path/tests/helical_faces.rs`): the layout of the old silent test (part X 0 to 30, stock from X -5, a held region past the free end, the far end the stock's end), for helical, axial and rest finishing: still 6 layers of 2 mm, the new warning, and not 0344's warning about the stock past the ring. It failed before the change.
- `req_set_09_a_free_end_ring_with_the_part_held_past_it_gets_no_cut_free_warning`: a second held region on the part's far end: faced, no cut-free warning. It replaces the old silent test, keeping its intent where it is true.
- `req_set_09_rings_at_both_free_ends_leave_the_part_held_by_nothing_and_are_warned`: stock and a held region past both ends: both rings are warned about. It failed with the "no other ring between" rule removed.
- The other free-end tests (`req_op_04_req_op_09_a_free_end_is_faced_in_layers_down_to_the_axis`, the thread-identity test, the flute refusal, the tab's outer end) pass unchanged.

## Consequences and limits

- `crates/openchuck-path/src/finishing.rs` (`free_ends`): the rings are collected and checked after the loop. Messages and the operation's notes show the warning; the setup sheet carries no cut warnings (as 0344 item 8) and export is not blocked.
- The test is of held regions, not of whether the part is joined to one through other stock or a tailstock the setup does not describe; a tailstock is not modelled, so a part held only by one is warned about (a false warning, on the safe side).
