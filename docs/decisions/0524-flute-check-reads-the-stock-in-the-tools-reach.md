# 0524: The flute check bounds the stock earlier operations left from the material within the tool's reach, and reads the stock's farthest radius once a check (TL-04, NFR-07, NFR-12; issue #547; lane FLUTE-TABS)

- **Status:** proposed by lane FLUTE-TABS on 6 Nov 2026. Only the project owner accepts a record (decision 0049).
- **Date:** 6 Nov 2026.
- **Number:** 0524, confirmed free by the Merge Manager on 6 Nov 2026.
- **Issue:** [#547](https://github.com/paulhampson/OpenChuckCAM/issues/547) (sev1), part 2: "only check the stock that the operation can actually touch".
- **Builds on:** [0522](0522-the-flute-check-warns.md), under which the check warns rather than refusing.
- **Amends:** [0509](0509-flute-length-against-the-deepest-cut.md) item 1 ("How it reads" and "The quick bound"), item 4 (free ends) and item 6 (first bullet).
- **Open items:** OI-2025 narrowed, not closed: the stock earlier operations left is still held to a bound, now from the material within the tool's reach. The exact answer stays the Phase 2 enhancement of issue [#540](https://github.com/paulhampson/OpenChuckCAM/issues/540). No new open item. No project schema change.
- **Library:** additive. A new public type `ReachProfile` (exported from `openchuck_path`) and a defaulted trait method `StockModel::reach_profile`; no version bump of its own (decision 0048).

## Context

Under 0509 item 1, the stock earlier operations left (the simulation's cells) has no exact answer over the tool. Every reading is therefore held to the farthest material from the axis anywhere in the stock, less the tip's height, plus 1 mm.

A held region (SET-09) is never cut, so that farthest material stays the blank's corner whatever roughing removed. In the owner's carving project (private; numbers only) it is the 40 x 40 mm box's corner, about 28.3 mm from the axis, at X -10 to -5. The finish never comes near it.

- **Axial Finishing.** It was refused at "up to 15.412 mm (a bound)". Under 0522 it is warned of instead.
- **Rest Machining.** Its free end (0344 item 7, which reads the same measure) was refused at "up to 29.249 mm (a bound)", against 25 mm of flute.
- **Public reproduction.** The CrankShaft model in 35 x 35 mm box stock from X -12 to 46, with a 4 mm held start and the default tabs, roughed leaving 0.3 mm, then finished. Axial, helical and simultaneous finishing were held to 27.900 to 28.667 mm.

The owner asked that only the stock the path can touch be checked: where the path does reach the stock, keep the conservative bound; elsewhere, do not.

The check was also slow. On the owner's project it took 212 s for one operation in a release build. The cause: each reading asked the stock for its farthest radius, which the simulation's stock computes by reading every column.

## Decision

1. **A reach profile.** A stock model with no exact answer may give a reach profile (`StockModel::reach_profile`): how far its material reaches from the rotary axis, in each slice along X and each 1 degree sector about the axis.
   - **From the simulation.** The simulation's stock (`DexelStock`) gives one, a slice a cell. Each column's spans are boxes, each recorded at its farthest corner in every sector it spans, so the profile is never below the material.
   - **Passed on.** The application's stock left by earlier operations passes it on.
2. **The bound from the material in reach.**
   - **Why X bounds it.** In a 4-axis machine the tool axis is square to the rotary axis, so material over the tool's disc lies within the tool's body radius of the tip along X.
   - **The bound.** For one reading, or one step between two readings, it is the highest the profile's material can stand along the tool axis over that X range (the tips' X, widened by the body radius), for every direction the axis takes over the step's turn. From that, subtract the lower tip's height and add 0509's 1 mm margin.
   - **Material off to the side.** Material square to the tool, or behind the axis, counts at most at the axis's height.
   - **Taken with the old bound.** The lower of this bound and 0509's farthest-radius bound is used; both are bounds. The warning still says "may stand up to" and that it is a bound.
3. **Read in bands.** Each slice keeps the highest its material can stand for each 1 degree band of tool direction, worked out once on the first reading. A reading then looks up a few slices and bands instead of 360 sectors. The stock's farthest radius is read once a check, not at every reading.
4. **Where it does not apply.** A stock model with no reach profile keeps the farthest-radius bound, and so does a tool axis not square to the rotary axis (none in the workspace). The exact answers on the setup's own box and round stock are unchanged.
5. **Still conservative (NFR-12).** The profile bounds every material cell from above, so stock within the tool's reach still sets the bound where the path reaches it, and is warned of past the flutes. That includes a held region, a tab, a wall or stock that roughing left. The selection set itself is not read: the check reads the emitted path's fed moves, including the leads, plunges and fed links, so a bound local to the path is already "only the stock the path can touch".
6. **Free ends (0509 item 4).** Their measure reads the same bound, so a free end beside stock that roughing cleared is no longer refused for the uncut held region's corner.

## Consequences

- **The owner's #547 project** (release build, with 0522):
  - The flute check took 212 s for Axial Finishing; it now takes 0.31 s, with no warning.
  - Rest Machining's free end is no longer refused.
  - All three operations are made. The whole run takes 131 s, against 333 s up to the refusal before.
- **The public CrankShaft case above.** With 21 mm of flute it is no longer warned of. With 8 mm it still is, at 18.2 mm, because the crank's webs stand within the tool's reach beside the journals.
- **What is left.** A finish after roughing can still be warned of where the profile's cells, their corners or the 1 mm margin stand past the flutes though the real stock does not (OI-2025, #540).

## Verification

Tests in `crates/openchuck-path/tests/flute_reach.rs`, on a stock of cells with no exact answer. The stock is a bar of radius 5 roughed to 5.5, and a held region of 35 x 35 box stock at X -3 to -1.

- `req_tl_04_req_nfr_12_stock_out_of_the_tools_reach_does_not_refuse_a_finish`: a full turn along the bar from X 4.175 is not warned of with 2.5 mm of flute. On the code before, the held region's corner set the bound and this failed.
- `req_tl_04_req_nfr_12_stock_in_the_tools_reach_is_still_held_to_the_bound`: the same finish from X 0 is warned of, and a turn at X 0.5 names at least 24.749 - 5 + 1 mm.
- `req_tl_04_req_nfr_12_stock_beside_the_tool_counts_when_the_turn_brings_it_over`: a rib 10 to 12 mm from the axis beside the tool is not warned of in a cut at one A, but is when a turn brings it over the tip. The cut at one A failed on the code before.
- `req_tl_04_req_nfr_12_a_reach_profile_never_reads_below_its_material`: 200 random boxes with 20 points each, read in random directions and spreads. The profile is never below a point's height.
- `req_nfr_07_a_reach_profile_of_no_size_is_none`: sizes that are not finite and positive give no profile, and a box that is not a number fills its slice at infinite reach.

Test in `crates/openchuck-app/tests/flute_check.rs`, on the public CrankShaft case above with 21 mm of flute:

- `req_tl_04_req_nfr_12_a_finish_with_flutes_past_the_stock_in_reach_is_not_warned_of`: no flute warning. With 8 mm of flute the existing test of 0522 is still warned of, as a bound.

The tests of 0509 and 0522 are unchanged and pass.
