# 0520: The flute check bounds the stock earlier operations left from the material within the tool's reach, and a generation refusal names its operation (TL-04, NFR-07, NFR-12; issue #547; lane FLUTE-TABS)

- **Status:** proposed by lane FLUTE-TABS on 6 Nov 2026. Only the project owner accepts a record (decision 0049).
- **Date:** 6 Nov 2026.
- **Number:** 0520 is provisional; the Merge Manager assigns the number at release.
- **Issue:** [#547](https://github.com/paulhampson/OpenChuckCAM/issues/547) (sev1).
- **Amends:** [0509](0509-flute-length-against-the-deepest-cut.md) item 1 ("How it reads" and "The quick bound") and item 6 (first bullet).
- **Open items:** OI-2025 narrowed, not closed: the stock earlier operations left is still held to a bound, now from the material within the tool's reach; the exact answer stays the Phase 2 enhancement of issue [#540](https://github.com/paulhampson/OpenChuckCAM/issues/540). No new open item. No project schema change. The library change is additive: a new public type `ReachProfile` and a defaulted trait method `StockModel::reach_profile`.

## Context

The owner's carving project (private; numbers only) was refused by the flute check of decision 0509 for an axial finish with an 8 mm flute V-bit after roughing. The refusal named 15.412 mm "at X -1.587, A 269.8", a place beside a retaining tab that the finish's selection set does not include, and did not name the operation.

The cause is the bound of 0509 item 1. The stock earlier operations left (the simulation's cells) has no exact answer over the tool, so every reading is held to the farthest material from the axis anywhere in the stock, less the tip's height, plus 1 mm. A setup with a held region (SET-09) keeps the blank there, never cut, so that farthest material is the blank's corner whatever roughing removed. In the owner's project the held region is the corner of 40 x 40 mm box stock, 28.3 mm from the axis, at X -10 to -5; the finish never comes near it.

- The place named is not where the stock stands: it is the tip with the lowest height on the path (the bound is highest there). X -1.587 is the finish's own edge, a tool radius past the part's end, so the path does go there, but it does not reach the stock that set the bound.
- The same refusal was reproduced on the public CrankShaft model: 35 x 35 mm box stock from X -12 to 46, a 4 mm held start, the default tabs, rotary roughing leaving 0.3 mm, then axial, helical or simultaneous finishing with an 8 mm flute: each was refused at 27.900 or 28.667 mm.
- The library's refusal is a `PathError`, which does not know which operation it refuses, and the application passed it on unchanged.

## Decision

1. **The bound comes from the material within the tool's reach.** A stock model with no exact answer may give a reach profile (`StockModel::reach_profile`): how far its material reaches from the rotary axis in each slice along X and each 1 degree sector about the axis. The simulation's stock gives one, a slice a cell: each column's spans are boxes, each recorded at its farthest corner in every sector it spans, so the profile is never below the material. The application's stock left by earlier operations passes it on.
   - **What it bounds.** In a 4-axis machine the tool axis is square to the rotary axis, so material over the tool's disc lies within the tool's body radius of the tip along X. For one reading, or one step between two readings, the bound is the highest the profile's material in that X range (the tips' X, widened by the body radius) can stand along the tool axis, for every direction the axis takes over the step's turn, less the lower tip's height, plus the 1 mm margin of 0509. A sector whose material is square to the tool or behind the axis counts at most at the axis's height.
   - **Taken with the old bound.** The lower of this bound and the farthest-radius bound of 0509 is used; both are bounds, so the lower is one. The message still says "may stand up to" and that it is a bound.
   - **Where it does not apply.** A tool axis not square to the rotary axis (none in the workspace), or a stock model with no reach profile, keeps the farthest-radius bound. The quick bound that skips readings is unchanged.
   - **What it does not change.** A path that reaches tall stock is still refused: the profile holds every material cell, so stock within the tool's reach, such as the held region or a tab within a tool radius, still sets the bound there. The exact answers on the setup's own box and round stock are unchanged.
2. **The selection set itself is not read.** The check reads the emitted path (0509 item 2), so stock the path does not reach is already not read once the bound is local; the faces selected are not needed, and leads, plunges and fed links stay checked.
3. **A refusal from generation names its operation.** The application wraps a pathing library's error from generating an operation as "operation '<name>' cannot be generated: ...". A refused parameter keeps its plain words without the schema's parameter name (specification section 15); the words already say "turn on Finishing layers". A cancel stays a cancel (UI-09).

## Consequences

- The owner's project generates (the finish's bound is TBD mm against 8 mm of flute); the CrankShaft case above generates.
- A finish after roughing can still be refused where the profile's cells, their corners or the 1 mm margin stand past the flutes though the real stock does not (OI-2025, #540).
- Cost: the profile is built once per check, from every column of the simulation's stock, and each reading that the quick bound does not skip reads at most 360 sectors of a few slices.

## Verification

Tests in `crates/openchuck-path/tests/flute_reach.rs`, on a stock of cells with no exact answer: a bar of radius 5 roughed to 5.5, and a held region of 35 x 35 box stock at X -3 to -1.

- `req_tl_04_req_nfr_12_stock_out_of_the_tools_reach_does_not_refuse_a_finish`: a full turn along the bar from X 4.175 is accepted with 2.5 mm of flute. It failed on the code before (the held region's corner set the bound).
- `req_tl_04_req_nfr_12_stock_in_the_tools_reach_is_still_held_to_the_bound`: the same finish from X 0 is refused; a turn at X 0.5 names at least 24.749 - 5 + 1 mm.
- `req_tl_04_req_nfr_12_stock_beside_the_tool_counts_when_the_turn_brings_it_over`: a rib 10 to 12 mm from the axis beside the tool does not refuse a cut at one A, and does refuse a turn that brings it over the tip. The cut at one A failed on the code before.
- `req_tl_04_req_nfr_12_a_reach_profile_never_reads_below_its_material`: 200 random boxes, 20 points each, random directions and spreads: the profile is never below a point's height.
- `req_nfr_07_a_reach_profile_of_no_size_is_none`: sizes that are not finite and positive give no profile; a box that is not a number fills its slice at infinite reach.

Tests in `crates/openchuck-app/tests/flute_check.rs`, on the public CrankShaft model with the held start and default tabs above:

- `req_tl_04_req_nfr_12_a_finish_beside_a_held_region_is_not_refused_for_stock_out_of_reach`: roughing (0.3 mm left) then axial finishing with 8 mm of flute is accepted. It failed on the code before, at 27.900 mm.
- `req_tl_04_req_nfr_12_a_finish_under_stock_deeper_than_the_flutes_is_still_refused_by_name`: roughing leaving 9 mm, then the same finish, is refused, and the message starts "operation 'Op 2' cannot be generated".

The 0509 tests are unchanged and pass.
