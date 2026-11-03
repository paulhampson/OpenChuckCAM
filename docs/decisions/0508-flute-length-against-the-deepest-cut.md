# 0508: Every strategy that cuts all the stock above it at once is refused when the stock stands deeper above the tool's tip than its flutes, measured from the stock itself, so box stock is measured at its corner (TL-04, SET-03, OP-12, NFR-07, NFR-12; issue #530; lane FLUTE-BOX)

- **Status:** proposed by lane FLUTE-BOX on 3 Nov 2026. Refusing rather than warning is the project owner's answer, relayed by the coordinator (decision 0049). The record as a whole awaits the owner's acceptance.
- **Date:** 3 Nov 2026.
- **Number:** provisional. The Merge Manager assigns the number at release.
- **Issue:** [#530](https://github.com/paulhampson/OpenChuckCAM/issues/530) (sev1).
- **Builds on:** [0033](0033-slice-14-rotary-roughing.md) item 8 (OI-153), [0344](0344-finishing-stepdown-and-free-ends.md) item 7, [0345](0345-finishing-layers.md) item 5, [0504](0504-end-facing-depth-within-the-flutes.md) items 1, 3 and 5, and [0161](0161-tools-keep-clear-of-the-gripped-length.md) item 3 ("said and refused, never silent").
- **Open items:** OI-153 narrowed: roughing's Stepdown is now held to the flutes, while the shank, the holder and the links stay with COL-05. No new item. No project schema change. The library change is additive: a new public module `flutes`, and defaulted trait methods `StockModel::describe` and `Strategy::cuts_in_layers`.

## Context

The owner ran simultaneous finishing on the CrankShaft model in 35 x 35 mm box stock with a 25 mm flute tool. The operation was accepted, although in places the cut was nearly 30 mm deep.

The cause was not the inscribed radius. The farthest distance of the stock from the rotary axis (`StockModel::max_radius`, `Stock::radius`) was already the corner's half diagonal, 24.749 mm for 35 x 35. The gap was that no strategy which cuts all the stock above a pass at once compared that depth with the flutes. The checks that existed held only a layer's Stepdown to the flutes:

- finishing layers (0345);
- Facing (0252, 0504);
- the cutting data in the tool library.

The free ends (0344) and End facing (0504) also read the stock's depth.

The strategies with no check at all were:

- simultaneous finishing;
- axial, helical and indexed finishing with Finishing layers off (the default);
- rest machining;
- pencil finishing;
- swarf;
- edge following, where the depth was measured from the part's surface only;
- radial drilling.

Rotary roughing did not refuse a Stepdown longer than the flutes (OI-153).

## Decision

1. **One measure (`openchuck_path::flutes::deepest`).**
   - **What it measures.** How far the stock stands above the tool's tip along the tool axis, over a disc the width of the tool's body: the cutting radius, or the shank's where the shank is wider.
   - **Where it reads.** At every cut point, and between points so that no step is more than 0.5 degree of A or 0.5 mm of travel. It reads the stock model itself with `ray_entry`, on two rings of the disc, then refines about the highest reading.
   - **What that covers.** Box stock is measured where a corner is uppermost over the axis. Stock off the axis (bounding box plus offset) and stock turned about X are measured as they lie. The stock earlier operations left is measured rather than the blank, so a roughed part is not refused.
   - **The quick bound.** No material is further from the axis than `max_radius`, so at a tip `h` above the axis the stock stands at most `max_radius - h` above it. A place is read only where that bound is past the flutes. A tip below the axis adds its depth (the pin sides of the crankshaft).
2. **Applied to every strategy by default.**
   - The shared generation (`generate_with`) runs the check on the passes as they will be cut, after the keep-out trimming, for every strategy whose `Strategy::cuts_in_layers` is false. That is the default, so a new strategy is checked unless it says otherwise.
   - The strategies that hold each layer to the flutes themselves return true:
     - rotary roughing: its layers start at `max_radius`, and its Stepdown is now refused past the flutes (OI-153);
     - Facing (0252, 0504);
     - axial, helical and indexed finishing with Finishing layers on (0345).
3. **Refused, not warned (the owner's answer, relayed).** As 0344 item 7, 0345 item 5 and 0504 item 3 do. The message is in plain words and gives:
   - the depth, with its place (X and A);
   - the flute length;
   - the stock (a box gives its size, its Y and Z range and its farthest corner from the axis; a cylinder gives its diameter; stock left by earlier operations says so);
   - the remedies: longer flutes, roughing first, and Finishing layers where the strategy has them.

   Where Finishing layers exist the message names that parameter, so the host points at the switch. Nothing in it says the program is safe (NFR-12).
4. **Free ends (0344 item 7) use the same measure for their refusal.** They read it at every half degree over the tool's width; before, they read every 5 degrees at three points along X, so a box corner could fall between readings. The layer count of a free-end turn keeps its earlier reading, so a path that was accepted is unchanged.
5. **Unchanged.**
   - End facing keeps its support reading (0504 item 1), which already measures the box at each view's A.
   - The real stick-out and the holder are still not modelled (0504 item 5, OI-1782). The limit is the flute length.

## Verification

Tests in `crates/openchuck-path/tests/flutes.rs`. The analytic cases are a round bar of radius 5 and a tip on it over a turn of A:

- `req_tl_04_req_set_03_a_box_corner_above_the_axis_is_the_deepest_cut`: 35 x 35 box, depth 19.749 mm (24.749 - 5), at a corner angle.
- `req_tl_04_req_nfr_07_a_box_corner_needs_flutes_past_the_half_diagonal`: refused at 12.6 mm (past the inscribed depth) and at 19.739 mm; accepted at 19.759 mm.
- `req_tl_04_req_set_03_a_rectangular_box_is_measured_at_its_corner`: 30 x 50 box, at hypot(15, 25) and its angle.
- `req_tl_04_req_set_03_stock_off_the_axis_is_measured_at_its_farthest_corner`: Y -10 to 25.
- `req_tl_04_a_tip_below_the_axis_adds_its_depth`: a tip 2.5 mm below the axis, 27.249 mm.
- `req_tl_04_stock_turned_about_the_axis_is_measured_as_it_lies`: a square turned 30 degrees.
- `req_tl_04_round_stock_is_measured_at_its_radius`: 12.5 mm, refused at 12.49 and accepted at 12.51.
- `req_tl_04_stock_left_by_earlier_operations_is_measured_not_the_blank`.
- `req_nfr_07_req_nfr_12_the_refusal_gives_the_numbers_in_plain_words`.
- One refusal (0.05 mm short) and one acceptance (0.05 mm over) per strategy on the bar in 35 x 35 box stock. Finishing layers on hold only the Stepdown.

The owner's case was reproduced on the public `testdata/stl/crankshaft.stl` with synthetic 35 x 35 box stock and a 3.175 mm flat tool with 25 mm of flute:

- before the change, simultaneous finishing was accepted with no warning;
- the stock stood 25.096 mm above the tip at X 39.416, A 212.5;
- the quick bound was 27.251 mm, because the tip goes 2.502 mm below the axis on the pins.

The owner's project is private and not used: numbers only.
