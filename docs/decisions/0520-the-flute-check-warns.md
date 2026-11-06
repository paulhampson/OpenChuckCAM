# 0520: The flute check of 0509 warns instead of refusing, in every case, and names the operation and its tool; a refusal from generation names its operation (TL-04, NFR-07, NFR-12; issue #547; lane FLUTE-TABS)

- **Status:** proposed by lane FLUTE-TABS on 6 Nov 2026, on the project owner's decision of 6 Nov 2026, relayed by the coordinator (decision 0049): "the flute-depth check is too conservative; until detailed collision detection exists it is downgraded from a refusal to a warning, all cases" (the owner chose "all cases become warnings" over keeping the exact box and round cases as refusals). Only the project owner accepts a record.
- **Date:** 6 Nov 2026.
- **Number:** 0520 is provisional; the Merge Manager assigns the number at release.
- **Issue:** [#547](https://github.com/paulhampson/OpenChuckCAM/issues/547) (sev1). Related: [#530](https://github.com/paulhampson/OpenChuckCAM/issues/530), [#540](https://github.com/paulhampson/OpenChuckCAM/issues/540).
- **Amends:** [0509](0509-flute-length-against-the-deepest-cut.md) item 3 ("Refused, not warned") and the refusals its items 1 and 2 and its Verification section describe. The measure of 0509 (items 1 and 2) is unchanged.
- **Open items:** OI-2043 new: replace the warning by real collision detection of the shank and holder against the stock (COL-05). OI-2025 kept open, its effect now a warning. OI-153 unchanged.
- **Library:** breaking. `flutes::check` returns `Option<PathWarning>` and `flutes::check_path` returns `Result<Option<PathWarning>>` (an error only when cancelled); new `flutes::WARNING_LEAD`. Workspace version 0.44.0 to 0.45.0 (decision 0048).

## Context

Decision 0509 refuses every strategy that cuts all the stock above it at once when the stock stands above the tool's tip, along the tool axis, further than the flutes reach. The stock earlier operations left has no exact answer, so it is held to a conservative bound: the farthest material from the axis anywhere, less the tip's height, plus 1 mm (OI-2025).

The owner's carving project (private; numbers only) could not be generated (issue #547). An axial finish with an 8 mm flute V-bit after roughing was refused at "up to 15.412 mm (a bound) at X -1.587, A 269.8", and the message did not name the operation.

- **Why it was refused.** The setup grips 5 mm of 40 x 40 mm box stock (SET-09). The gripped length is never cut, so the farthest material stays the box's corner, about 28.3 mm from the axis, whatever roughing removed. The place named is only the path's lowest tip, where that bound is largest: the finish's edge, a tool radius past the part's end. It is not a place where the stock was measured.
- **Public reproduction.** The CrankShaft model in 35 x 35 mm box stock with a 4 mm held start and the default tabs, roughed leaving 0.3 mm and finished with 8 mm of flute: refused at 27.900 mm.
- **Cost.** On the owner's project the refusal came after about 300 s of generation (the LARGEPATH lane's measurement).

The owner's decision: the check is too conservative to block generation. Until there is detailed collision detection, it warns.

## Decision

1. **A warning, not a refusal, in every case.** The shared generation runs the check of 0509 item 2 as before, on the emitted program, and keeps the path. Where the stock stands past the flutes it adds one warning to the path (`PathWarning`; decision 0175: shown with the operation's warnings and as one of its notes; not one that needs acknowledging). That applies to exact depths, to bounds, to stock of unknown size and to a path position that is not a finite number.
2. **What the warning says.** It starts "Flute length: " (`flutes::WARNING_LEAD`) and gives:
   - the depth, and whether it is exact ("stands up to") or a bound ("may stand up to", with why it is a bound);
   - the place (X, A);
   - the flute length, and that the shank may meet the stock;
   - the stock;
   - that the path was made and must be checked before it is run;
   - the options: longer flutes, roughing first in the setup, and Finishing layers where the strategy has them.

   The warning carries the Finishing layers parameter where the strategy has it, so the host points at that setting. Nothing in it says the program is safe or verified (NFR-12).
3. **The operation and its tool are named where the warning is shown.** The application shows the flute warning as "Flute length (operation '<name>', tool '<tool name>'): ...":
   - in Messages and Results (source: the operation);
   - in the operation's status;
   - in the export checks, where every warning is already said with its operation's name.

   It is named when shown, not when made, so a renamed operation or tool, which does not make the path stale (issue 209), is named as it is now. The setup sheet shows no path warnings today, so nothing changes there.
4. **A refusal from generation names its operation.** An error from the pathing library while generating an operation becomes "operation '<name>' cannot be generated: <its words>", keeping the parameter it names ("parameter stepdown: ..."). A cancel stays a cancel (UI-09).
5. **Unchanged refusals.** These are other rules, so they stay refusals:
   - a roughing Stepdown past the flutes (OI-153);
   - Finishing layers' Stepdown past the flutes (0345);
   - Facing (0252, 0504);
   - the free-end turn of 0344 item 7, which reads the same measure. Whether it should warn too is a question for the owner.
6. **Not in this record.**
   - Narrowing the bound to the stock within the tool's reach, so the warning is not noise (#547 part 2): a later pull request of this lane. Its prototype cost about 710 s for the check alone on the owner's project in a release build, so it is reworked first.
   - Running a cheap check before the long generation: an option, less urgent now that the check does not block.

## Consequences

- The owner's project generates; the warning says the bound and offers the options.
- A path past the flutes is written as before, with the warning in Messages and in the export checks. The user verifies the program before running it (NFR-12), as for every program.
- The cost of the check is unchanged; it now runs to the end of the path rather than stopping a generation.
- The goldens are unchanged: no golden path is past its flutes.

## Verification

Library tests (`crates/openchuck-path/tests/flutes.rs`): every refusal test of 0509 is now a warning test with the same numbers.

- Past the flutes: the path is made, with one warning that is also a note and needs no acknowledgement, and with the depth, place, flute length and stock named.
- Within the flutes: no warning.
- Among them, the 35 x 35 mm box CrankShaft with 25 mm of flute is still detected and warned (25.096 mm, and 27.231 mm with the box off the axis), never silent.
- The parameter a warning carries is Finishing layers for axial, helical and indexed finishing, and none for the others.
- The roughing Stepdown test stays a refusal.
- The tests 0509 names "..._is_refused..." are renamed "..._is_warned..." (and "the_refusal_gives_the_numbers_in_plain_words" is now "the_warning_gives_the_numbers_in_plain_words"). This includes `drill::req_op_10_req_tl_04_req_nfr_07_a_hole_deeper_than_the_flutes_is_warned` and, in `tests/swarf.rs`, `req_op_07_req_tl_04_req_nfr_07_a_flank_too_short_for_the_stock_above_the_wall_is_warned`. The free-end tests of 0344 and 0509 item 4 keep their refusals.

Application tests (`crates/openchuck-app/tests/flute_check.rs`):

- `req_tl_04_req_nfr_12_finishing_the_crankshaft_blank_with_25_mm_of_flute_is_warned_by_name`: made, with one warning. Messages shows "Flute length (operation 'Op 1', tool '3.175 mm flat'): the stock stands up to ..." with the operation as its source; it says the shank may meet the stock and to check the path before running it, and never "safe".
- `req_tl_04_req_set_03_roughing_then_finishing_the_crankshaft_with_25_mm_of_flute_is_not_refused`: no flute warning.
- `req_tl_04_req_nfr_12_a_finish_past_the_flutes_after_roughing_is_made_with_a_warning`: the #547 reproduction above (held start, default tabs, roughing, then axial finishing with 8 mm of flute) is made. The warning names the operation and the tool, says the depth is a bound and the stock is what earlier operations left, and offers Finishing layers. The export checks say "Operation 'Op 2' was made with a warning: Flute length (operation 'Op 2', tool ...". Before, it was refused.
- `req_nfr_07_a_refusal_from_generation_names_its_operation`: a roughing Stepdown past the flutes is refused as "operation 'Op 1' cannot be generated: parameter stepdown: ...".
