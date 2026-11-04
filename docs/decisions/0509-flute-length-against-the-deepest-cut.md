# 0509: Every strategy that cuts all the stock above it at once is refused when the stock stands deeper above the tool's tip than its flutes, measured from the stock itself, so box stock is measured at its corner (TL-04, SET-03, OP-12, NFR-07, NFR-12; issue #530; lane FLUTE-BOX)

- **Status:** accepted by the project owner on 4 Nov 2026, relayed by the coordinator (decision 0049), with refusal as the behaviour (the owner's answer, relayed on 3 Nov 2026). Proposed by lane FLUTE-BOX on 3 Nov 2026.
- **Date:** 3 Nov 2026.
- **Number:** proposed and accepted as 0508 (provisional); renumbered 0509 on 4 Nov 2026 at release (assigned by the Merge Manager; 0508 went to lane LARGEPATH, #471). Earlier comments on pull request #534 call it 0508.
- **Issue:** [#530](https://github.com/paulhampson/OpenChuckCAM/issues/530) (sev1).
- **Builds on:** [0033](0033-slice-14-rotary-roughing.md) item 8 (OI-153), [0344](0344-finishing-stepdown-and-free-ends.md) item 7, [0345](0345-finishing-layers.md) item 5, [0504](0504-end-facing-depth-within-the-flutes.md) items 1, 3 and 5, and [0161](0161-tools-keep-clear-of-the-gripped-length.md) item 3 ("said and refused, never silent").
- **Open items:** OI-153 narrowed: roughing's Stepdown is now held to the flutes, while the shank, the holder and the links stay with COL-05. OI-2025 new: the stock earlier operations left has no exact answer and is held to a conservative bound. No project schema change. The library change is additive: a new public module `flutes`, and defaulted trait methods `StockModel::describe`, `StockModel::top_over_disc` and `Strategy::cuts_in_layers`.

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
   - **Where it reads.** At every cut point, and between points so that no step is more than 0.5 degree of A or 0.5 mm of travel. Between two points A turns the way assembly turns it, by `rotary::nearest_turn`: a pass given as 350 then 10 degrees turns 20 degrees, and an exact half turn goes the way assembly takes it (90 then -90 turns up through 270).
   - **How it reads.** For the setup's own box or round stock, exactly: `StockModel::top_over_disc` gives the highest material over the disc in closed form, so nothing can lie between readings. Any other stock model, including the stock earlier operations left, is never read by sampling: it is held to the conservative bound below, plus 1 mm because a farthest radius may be measured at cell centres, and the message says it is a bound. A stock with no farthest radius is refused (its size is not known). The bound can refuse finishing after roughing that the real stock would allow (OI-2025).
   - **What that covers.** Box stock is measured where a corner is uppermost over the axis. Stock off the axis (bounding box plus offset) and stock turned about X are measured as they lie. The stock earlier operations left is measured rather than the blank, so a roughed part is not refused.
   - **The quick bound.** No material is further from the axis than `max_radius`, so at a tip `h` above the axis the stock stands at most `max_radius - h` above it. The exact answer is asked for only where that bound is past the flutes. A tip below the axis adds its depth (the pin sides of the crankshaft).
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
- `req_tl_04_req_nfr_07_a_narrow_strip_of_stock_between_readings_is_not_missed`: a box strip 0.1 mm wide, 0.95 mm across the tool axis and 15 mm above the tip, which the sampled read of the first version missed entirely (it failed on that code, accepting any flute), is measured exactly (15.0 to 15.08 mm) and refused with 14.9 mm of flute (CodeRabbit on #534).
- `req_tl_04_a_pass_across_the_0_360_seam_turns_the_short_way`: A given as 340 to 20 through 0 reads as 340 to 380 does (CodeRabbit on #534). On the code before the fix it read the box's corner (19.749 mm at A 135) for a pass that turns 40 degrees about a face (14.201 mm).
- `req_tl_04_an_exact_half_turn_goes_the_way_assembly_turns_it`: an exact half turn (0 then -180, 0 then 180, and 90 then -90) reads the half assembly cuts (`nearest_turn`: 90 -> 270), on box stock off the axis where the two halves differ by more than 1 mm. The shortest-step formula of the first fix read the other half, so it could read the shallower half and accept (CodeRabbit on #534, second review); the test fails on that code.
- `req_tl_04_req_nfr_07_a_narrow_round_of_stock_between_readings_is_not_missed`: the same for round stock (a rod of radius 0.06 mm): 15.06 mm, exact.
- `req_tl_04_req_nfr_07_stock_with_no_exact_answer_is_held_to_a_conservative_bound`: a stock model without `top_over_disc` (a round 0.5 mm over the bar) is held to 5.5 - 5 + 1 = 1.5 mm, refused at 1.4 mm of flute although it stands 0.5 mm, with words that say it is a bound; with no farthest radius either it is refused whatever the flutes.
- One refusal (0.05 mm short) and one acceptance (0.05 mm over) per strategy on the bar in 35 x 35 box stock. Finishing layers on hold only the Stepdown.

**Cuts below the middle of the stock** (the owner's question, relayed). This was the key failure in the owner's case. Where the tip is below the rotary axis, the stock above it is the stock's farthest point from the axis plus the depth below the axis.

- **Measure.** `flutes::deepest` takes the tip's height above the axis, along the tool axis, as a signed number. It comes from `RotaryMapping::to_axis` and is negative below the axis.
- **Quick bound.** The bound is `max_radius - height`. Below the axis it grows by the depth, so such a place is skipped less, never more. It also keeps 1 mm inside the flutes, because a stock model's farthest radius may be read at cell centres (the simulation's columns).
- **Exact read.** It starts above the tip by `max_radius + |height| + 1` and reads down along the tool axis to the first material. The depth is therefore from the tip up to the stock's top on the far side, through the axis. It reads over a disc the width of the tool's cutting part, or of its shank where that is wider.

The tests are in `tests/flutes.rs` and `tests/swarf.rs`:

- `req_tl_04_a_cut_below_the_midpoint_adds_its_depth_below_the_axis_in_both_senses_of_a`: tips 5, 10 and 15 mm below the axis, on a 35 mm round and a 35 x 35 box, in both senses of A. The depth is 17.5 or 24.749 plus the depth below; refused 0.01 mm short, accepted 0.01 mm over.
- `req_tl_04_the_quick_bound_never_skips_a_tip_below_the_axis`: 0.5 to 12.5 mm below, refused 0.02 mm short.
- An off-axis pin (radius 3, its centre 13 mm from the axis, so its tip goes 10 mm below the axis) in 35 x 35 box stock. Each strategy is refused with 24 mm of flute, naming a depth past the farthest corner (24.749 mm), which only a tip below the axis has, and accepted with 36 mm. The named depths:

  | Strategy | Depth named, mm |
  | --- | --- |
  | simultaneous | 32.163 |
  | axial | 29.458 |
  | helical | 30.508 |
  | indexed | 27.500 |
  | rest | 31.489 |

- `req_op_07_req_tl_04_swarf_from_below_the_axis_counts_the_far_half_of_the_stock`: the tip end reversed (A 180), the tip 18 mm below the axis in 22 mm round stock: 40.000 mm, refused at 39.9 and accepted at 40.1.
- `req_tl_04_req_nfr_07_pencil_finishing_on_the_crankshaft_is_refused_at_the_depth_it_measures`: pencil's creases on the CrankShaft model do not go below the axis (13.474 mm deepest). Its boundary is tested either side by 0.05 mm.
- `req_tl_04_req_nfr_07_the_crankshaft_in_35_by_35_box_stock_is_refused_with_25_mm_of_flute`: the CrankShaft model with simultaneous finishing.
  - Centred 35 x 35 box: refused at 25.096 mm and accepted with 28.5 mm.
  - The same box 2.5 mm off the axis in Z (farthest corner 26.575 mm): refused at 27.231 mm.

Depths approaching 30 mm, as the owner saw, need the tip about 5.25 mm below the axis with a corner of a centred 35 x 35 box uppermost (24.749 + 5.25). Box stock off the axis, as bounding-box stock can be, gives the same. The off-axis-pin tests above, at 29.5 to 32.2 mm, show such depths refused.

The owner's case was reproduced on the public `testdata/stl/crankshaft.stl` with synthetic 35 x 35 box stock and a 3.175 mm flat tool with 25 mm of flute:

- before the change, simultaneous finishing was accepted with no warning;
- the stock stood 25.096 mm above the tip at X 39.416, A 212.5;
- the quick bound was 27.251 mm, because the tip goes 2.502 mm below the axis on the pins.

The owner's project is private and not used: numbers only.
