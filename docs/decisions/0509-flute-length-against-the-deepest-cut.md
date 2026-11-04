# 0509: Every strategy that cuts all the stock above it at once is refused when the stock stands deeper above the tool's tip than its flutes, measured from the stock itself, so box stock is measured at its corner (TL-04, SET-03, OP-12, NFR-07, NFR-12; issue #530; lane FLUTE-BOX)

- **Status:** accepted by the project owner on 4 Nov 2026, relayed by the coordinator (decision 0049), with refusal as the behaviour (the owner's answer, relayed on 3 Nov 2026). Proposed by lane FLUTE-BOX on 3 Nov 2026.
- **Date:** 3 Nov 2026.
- **Number:** proposed and accepted as 0508 (provisional); renumbered 0509 on 4 Nov 2026 at release (assigned by the Merge Manager; 0508 went to lane LARGEPATH, #471). Earlier comments on pull request #534 call it 0508.
- **Issue:** [#530](https://github.com/paulhampson/OpenChuckCAM/issues/530) (sev1).
- **Builds on:** [0033](0033-slice-14-rotary-roughing.md) item 8 (OI-153), [0344](0344-finishing-stepdown-and-free-ends.md) item 7, [0345](0345-finishing-layers.md) item 5, [0504](0504-end-facing-depth-within-the-flutes.md) items 1, 3 and 5, and [0161](0161-tools-keep-clear-of-the-gripped-length.md) item 3 ("said and refused, never silent").
- **Open items:** OI-153 narrowed: roughing's Stepdown is now held to the flutes, while the shank, the holder and the links stay with COL-05. OI-2025 new: the stock earlier operations left has no exact answer and is held to a conservative bound; the owner accepted the bound (4 Nov 2026, relayed) and the more accurate answer is a Phase 2 enhancement, issue [#540](https://github.com/paulhampson/OpenChuckCAM/issues/540). No project schema change. The library change is additive: a new public module `flutes`, and defaulted trait methods `StockModel::describe`, `StockModel::top_over_disc` and `Strategy::cuts_in_layers`.

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
   - **Where it reads.** At every point, and between points so that no step is more than 0.5 degree of A or 0.5 mm of travel, and over each step between two places read, not at the places only (the owner's Codex review of #534: a thin slab can come within the tool's radius only half way along a step). In the machine's frame the tool axis is fixed and the stock turns with A, so over a step the tip is within `s / 2` across the tool axis and `h / 2` along it of the step's middle (`s` and `h` the step's travel across and along the axis), and no material moves more than `t = R * angle / 2` (`R` the stock's farthest radius, the angle the step's turn; none for stock that is the same at every A, `StockModel::turns_unchanged`, as round stock on the axis is, so a turn flush with the flutes on round stock, such as a free end of 20 mm round stock faced with 10 mm of flute, is read exactly and accepted). The stock's exact top over a disc wider by `s / 2 + t` at the middle, plus `h / 2 + t`, therefore bounds the whole step; a straight cut at one height with A still under a flat top is bounded exactly, with no halving. Where that bound is past the flutes the step is halved and its middle read exactly, until the bound is within the flutes or the step's reach `s / 2 + h / 2 + t` is under 0.000001 mm, where the bound is taken as the depth and the path refused. Each step has a budget of 256 readings (CodeRabbit on #534, review of 6580f9b: stock flush with the flutes would otherwise be read about a million times a step); past it the bound is kept, as a bound, and the path refused, never accepted unread. That refuses only stock within about 0.002 mm of the flutes over a turning step of stock that is not the same at every A. The depth a refusal names is the deepest reading, or the bound where it says it is one. A stock with no exact answer needs no more: its bound (below) is already highest at the lower tip of a step and does not turn with A. A stock with an exact answer but no farthest radius (the `StockModel` contract allows it; the owner's Codex review of 6580f9b) is bounded the same way over a step at one A, which needs no radius; a step that turns A on such stock cannot be bounded and is refused as stock of unknown size. A tool position, angle or stock answer that is not a finite number is refused, never read as no stock (NFR-07). Cancel is polled at each move (UI-09, architecture section 5.1): `flutes::check_path` takes the job's context, and a move's work is bounded by its readings and their budget. In production (item 2) the points are the emitted program's, so A turns exactly as written. The passes-level form (`flutes::check`, `flutes::deepest`), which a strategy may call before assembly, predicts the turn by `rotary::nearest_turn` (a pass given as 350 then 10 degrees turns 20 degrees) and, at an exact half turn, where the way assembly takes depends on the whole turns it carries and on its fit to the A range, reads both ways round and takes the deeper.
   - **How it reads.** For the setup's own box or round stock, exactly: `StockModel::top_over_disc` gives the highest material over the disc in closed form, so nothing can lie between readings. Any other stock model, including the stock earlier operations left, is never read by sampling: it is held to the conservative bound below, plus 1 mm because a farthest radius may be measured at cell centres, and the message says it is a bound. A stock with no farthest radius is refused (its size is not known). The bound can refuse finishing after roughing that the real stock would allow (OI-2025).
   - **What that covers.** Box stock is measured where a corner is uppermost over the axis. Stock off the axis (bounding box plus offset) and stock turned about X are measured as they lie. The stock earlier operations left is measured rather than the blank, so a roughed part is not refused.
   - **The quick bound.** No material is further from the axis than `max_radius`, so at a tip `h` above the axis the stock stands at most `max_radius - h` above it. The exact answer is asked for only where that bound is past the flutes. A tip below the axis adds its depth (the pin sides of the crankshaft).
2. **Applied to every strategy by default.**
   - **The production check reads the emitted program.** The shared generation (`generate_with`) runs the check (`flutes::check_path`) after assembly, on the path as it will be posted, for every strategy whose `Strategy::cuts_in_layers` is false. It reads every fed move from the pose before it: the cuts, and also the leads in and out, the plunges and the fed links, which can go deeper than the passes. Rapids are not read: a rapid is not a cut, and the fed move after it is read from where the rapid left the tool. The angles are the ones assembly emitted (unwrapped, carried across passes and fitted to the A range), so the arc read is the arc the tool turns through, not a prediction of it (CodeRabbit on #534, third review). That is the default, so a new strategy is checked unless it says otherwise.
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
4. **Free ends (0344 item 7) use the same measure for their refusal.** They read it at every half degree over the tool's width, and over each step between, as item 1 reads a move; where the depth is a bound (stock earlier operations left), the refusal says "up to" and that it is a bound (CodeRabbit on #534). Before, they read every 5 degrees at three points along X, so a box corner could fall between readings. The layer count of a free-end turn keeps its earlier reading, so a path that was accepted is unchanged.
5. **Unchanged.**
   - End facing keeps its support reading (0504 item 1), which already measures the box at each view's A.
   - The real stick-out and the holder are still not modelled (0504 item 5, OI-1782). The limit is the flute length.
6. **Not covered by this check.**
   - The stock earlier operations left has no exact answer: it is held to the conservative bound of item 1, which can refuse a finish the real stock would allow (OI-2025). The owner accepted the bound (4 Nov 2026, relayed); the more accurate answer, with an investigation of its performance, is a Phase 2 enhancement, issue [#540](https://github.com/paulhampson/OpenChuckCAM/issues/540).
   - The shank above the flutes, the stick-out and the holder meeting the stock or the part stay with COL-05 (OI-153, OI-1782). This check compares the stock's depth with the flute length only.
   - Strategies that return `cuts_in_layers` true (item 2) are held per layer by their own Stepdown checks, not by this one.

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
- `req_tl_04_req_nfr_07_the_generation_checks_the_turn_assembly_writes`: through the shared generation, a first pass given as 350 then 170 (cut down through 260) and a later pass given as 10 then 190 after one ending at 360 (cut up from 370 to 550), each on box stock whose cut half stands 30.355 mm above the tip and the other 21.926 mm: 25 mm of flute is refused and 31 mm accepted. It failed on the code that checked the passes before assembly, which accepted 25 mm. It also holds the owner's Codex review's case: a first pass given as 270 then 90 on box stock Y -10 to 25 and Z -25 to 10, which assembly unwraps to -90 then -270 (30.355 mm at -225), is refused with 25 mm of flute and accepted with 31 mm (the code before accepted it, reading 270 to 450 at 21.926 mm), and the same half turn after a pass that wraps.
- `req_tl_04_an_exact_half_turn_reads_both_ways_round`: at an exact half turn both ways round are read (CodeRabbit on #534, second and third reviews). A first pass given as 350 then 170 is cut by assembly down through 260 (it unwraps from 0), and a later pass given as 10 then 190 after one ending at 360 is cut up through 100 (the carried turn). In each case, with box stock off the axis, the half assembly cuts is 8.4 mm deeper than the half the raw step's nearest turn would read. The test fails on the code that read one way only, which would have accepted.
- `req_tl_04_req_nfr_07_stock_between_two_readings_of_a_move_is_not_missed` (the owner's Codex review of #534): a 3 mm flat tool with 1 mm of flute moves 0.5 mm along X at A 0; a slab 0.1 mm long in X, 1.49 to 1.50 mm across the tool axis and 15 mm tall is out of the tool's radius at both ends of the move and inside it half way. Each end alone is accepted, the move is refused naming at least 15 mm, and with 15.1 mm of flute it is accepted. It failed on the code that read the places only (it accepted the move).
- `req_tl_04_req_nfr_07_stock_flush_with_the_flutes_is_settled_in_bounded_work` (CodeRabbit on #534, review of 6580f9b): a straight cut at one height under a flat top exactly 10 mm above the tip, with 10 mm of flute, is accepted with no halving; a turn under round stock 12.5 mm above the tip is accepted with 12.5 mm of flute (round stock is the same at every A); the same stock as a model that does not say so is refused as a bound with 12.5 mm of flute and accepted with 12.51 mm; each within a second. On 6580f9b the straight case took 3 s of halving.
- `req_tl_04_req_nfr_07_stock_of_unknown_reach_between_readings_is_not_missed` (the owner's Codex review of 6580f9b): Codex's slab, on a stock model with an exact top and no farthest radius, is refused naming at least 15 mm; a turn on such stock is refused as stock of unknown size. On 6580f9b the slab was accepted.
- `req_tl_04_req_nfr_07_a_cut_exactly_at_the_flutes_is_read_in_bounded_work` (the same review): Codex's cut, 100 mm along round stock exactly 10 mm above the tip, is accepted with 10 mm of flute in under a second and refused at 10 mm with 9.99 mm. On 6580f9b it took about 157 million readings.
- `req_ui_09_req_tl_04_a_cancelled_flute_check_stops` (the same review): a cancelled job stops the check with the cancel error.
- `req_nfr_07_req_tl_04_a_place_or_answer_that_is_not_a_number_is_refused` (the coordinator's adversarial review of the check): a tool position or angle that is not a finite number is refused as the path's, and a stock answer that is not a number as stock of unknown size. Before, a position that was not a number read as no stock and was accepted.
- `req_tl_04_a_move_of_no_length_is_read_where_it_stands`, `req_tl_04_req_nfr_07_a_move_of_many_turns_is_read_all_round` (10 and 30 turns at one place, past the 10,000 readings a move is split into) and `req_tl_04_req_nfr_07_a_shank_wider_than_the_flutes_is_read_over_its_width` (the same review): each passed on the code before it and holds the behaviour.
- `req_nfr_07_req_tl_04_a_free_end_flush_with_the_flutes_on_round_stock_is_faced`: a free end of round stock 10.3 mm in radius is faced with exactly 10.3 mm of flute. Before `turns_unchanged`, the budget refused it as a bound, and so refused three application tests whose 20 mm round stock is faced with 10 mm of flute.
- `req_nfr_07_req_nfr_12_a_free_end_refused_on_a_bound_says_it_is_a_bound` (in `tests/helical_faces.rs`; CodeRabbit on #534): a free end on stock with no exact answer is refused naming "up to 11.300 mm" and saying it is a bound; 6580f9b named the bound as the stock's depth.
- `req_tl_04_req_nfr_07_fed_moves_deeper_than_the_cut_are_refused_and_rapids_are_not_read`: a written program on round stock of radius 17.5 whose cuts stand 10 mm deep: a plunge, a lead in, a lead out, a fed link or a cut 2.5 mm deeper is refused with 10 mm of flute naming 12.500 mm, while the same move as a rapid is accepted; the deeper lead is refused at 12 mm and accepted at 13 mm.
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

No new false refusal of a roughed part, in `crates/openchuck-app/tests/flute_check.rs`, on the public CrankShaft model in the setup's own 35 x 35 mm analytic box stock with a 3.175 mm flat tool of 25 mm flute:

- `req_tl_04_req_nfr_07_finishing_the_crankshaft_blank_with_25_mm_of_flute_is_refused`: simultaneous finishing alone is refused through the application, naming the flute and the box.
- `req_tl_04_req_set_03_roughing_then_finishing_the_crankshaft_with_25_mm_of_flute_is_not_refused`: rotary roughing (4 mm Stepdown, 0.3 mm left) and then the same finishing are accepted.

The golden G-code files are unchanged by the move to the emitted program.

The owner's project is private and not used: numbers only.
