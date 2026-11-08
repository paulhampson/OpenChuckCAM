# 0543: Smooth steps between rings, and a survey of the other sharp transitions (OP-03, OP-04, OP-05, OP-09, OP-11, OP-12, OP-14, OP-15, OP-24, GC-07, NFR-06, NFR-07, NFR-12; proposed lane RINGBLEND)

- **Status:** proposed, 8 Nov 2026, by an investigation session at the owner's request ("strategies that operate as rings have sharp transitions which can create abrupt tool load with effectively a lateral plunge; investigate options and propose smooth transitions; cover all transitions and links"). Nothing is built. The owner's choices D1 to D5 (below) are open; OI-2111 tracks them.
- **Date:** 8 Nov 2026.
- **Would amend, for operations that choose the blended step only:** [0390](0390-helical-finishing-in-rings.md) item 1 (every ring starts and ends at the Start angle, so all steps lie on one line), [0420](0420-rings-for-rest-roughing-and-round-facing.md) item 1 (the step at constant A) and [0497](0497-rings-for-the-simultaneous-spiral.md) item 3 (the step holds A, the owner's D3). With the step left sharp, all three are unchanged.
- **Builds on:** [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (a ring at each face across the axis), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (a whole turn at a held region's end), [0506](0506-combine-pure-a-runs.md) (pure-A runs written as one block; the Z1 stops at the end of every pure-A block, OI-993), [0521](0521-low-links-for-helical-rest-and-simultaneous-finishing.md) item 6 (the pattern for a new operation's default), [0026](0026-entry-moves.md) (entries), [0344](0344-finishing-stepdown-and-free-ends.md) and [0345](0345-finishing-layers.md) (engagement not promised; OI-1373).
- **Open items:** new OI-2111 (this proposal's decisions) and OI-2112 to OI-2117 (the other transitions found, section "Survey"). OI-1373 (engagement limits) and OI-151 (roughing entries default to plunge) stay open and are related, not changed. No project schema change would be needed (a free parameter, as 0390's).

## Context

### What a ring's step does today

Every strategy that offers **Passes: Rings** cuts one whole turn of A at constant X, then steps along X at constant A to the next ring, with no retract:

| Strategy | Where the step is made | Step |
| --- | --- | --- |
| Helical finishing (0390) | `helical::ring_passes` (`crates/openchuck-path/src/helical.rs`) | an axial fit (`Fit` with `total_angle: 0`) from `x_k` to `x_k+1`, joined into one cut by `helical::join` |
| Rest machining (0420 item 2) | the same `helical::ring_passes`, through rest's `wanted` filter | as helical finishing |
| Continuous rotary roughing, and round Facing (0420 items 3, 4) | `Job::continuous_rings` (`roughing.rs`): `Piece::Ring` then `Piece::Step { a: a + 360 }` | X moves with A held, at the layer's depth, through `Job::layers` |
| Simultaneous finishing, spiral pattern (0497) | `simultaneous::ring_passes`: a step `Track` with `th0 == th1` and `held_plan` | X moves with A held; split where the lean exceeds lead plus tilt |

In the unrolled plane of the cut (X along the part, arc length round it) the step is a 90-degree corner at each end, made at full depth.

- **Roughing and round Facing (the load case).** On the ring the tool's side meets a band one stepover wide (radial engagement `a_e = s`). On the step the tool moves along X into the wall the next ring has not yet cut: everything ahead of it is uncut to the layer depth, so the front half of the cutter is engaged, `a_e` is the full diameter, a slot. The engagement arc jumps from about 78 degrees (40 % stepover) to 180 degrees at the corner and back at the next one. Measured with a small independent model (section "Verification"): width of cut on the ring 1.26 mm, on the step 2.45 mm median and 2.95 mm peak, against a 3.175 mm cutter; the step is 1.27 mm of slot in every ring of every layer.
- **Finishing, rest machining and the simultaneous rings.** The material is the stock to leave and the scallop, so the load is smaller, but the corner is the same 90 degrees in material: the step takes the whole width of the remaining band at once, and the direction change marks the surface. In the simultaneous rings the step also leans the tool by the lead it holds (0497 item 3, by design).
- **What the machine does at the corner.** Nothing in the assembly slows the feed at a corner: every in-pass move is `Cutting` at the cutting feed (`assemble.rs`), with no engagement feed and no arc output. The LinuxCNC post writes G61 (exact stop). On the Makera Z1, a pure-A block has no direction vector in the firmware planner, so every such block, and every combined ring since 0506, is planned to stop at its end (OI-993, read from the firmware source). So at each ring's end the tool stops at depth and starts again into a slot.

The spiral does not have this problem: X advances by one stepover per turn, so `a_e` stays at the stepover everywhere after the first turn. A blended step borrows that property for a short arc.

## Options for the step

| | Option | Engagement at the step | Corner in the cut | What else changes |
| --- | --- | --- | --- | --- |
| A | **Whole ring, then a blend.** After each whole ring, the tool turns on by a blend angle φ while X moves to the next ring (a short helix piece). The next ring starts φ further round and is again a whole turn. | Bounded near the stepover: peak 1.65 mm at φ = 30°, 1.52 mm at 45°, 1.43 mm at 90°, against 2.95 mm today and 1.31 mm on the spiral | atan(s / rφ): 13.6° at φ = 30°, 9.2° at 45°, 4.6° at 90° (s = 1.27 mm, r = 10 mm), against 90° | Every ring stays a whole turn at one X, so faces (0297), held ends (0161) and round Facing's ends (0420 item 3) are unchanged. A builds up 360° + φ a ring, and the steps no longer lie on one line (they move on by φ a ring). The cut is longer by about φ/360 of a ring (12.5 % at 45°). |
| B | **Blend in place of the ring's end** (0390's option 3, not chosen then). The blend takes the last φ of each ring, so every step lies in one window after the Start angle. | Same as A (the model gives the same peaks) | Same as A | Not longer, and A builds up exactly one turn a ring, but no ring is a whole turn at one X. A ring that must be whole (the ends, a held region's end, a face's ring) needs a special case that moves the window on by φ. 0390 rejected this shape because "each ring is not quite flat at its end". |
| C | **Sharp step at a lower feed.** Keep the step; feed it at the plunge feed, or a stated fraction. | Still a full slot, at a lower chip load | Still 90° | Simple and byte-identical elsewhere, but the step still slots and the Z1 still stops at the corner. A mitigation, not a fix. |
| D | **A fillet inside the step only.** Round the two corners within the step's own length (radius up to s/2). | Close to a slot: the model's φ = 15° case (an arc of about 2.6 mm) still peaks at 1.9 mm | Smooth | The step is too short for a fillet to change the engagement much. |
| E | **Lift for the step.** Retract, move, re-enter. | Replaces the slot with an entry into material (a plunge by default, OI-151) | – | Worse: a vertical plunge every ring, and slower. 0390's option 4, not chosen. |
| F | **Keep the step and warn.** | Unchanged | Unchanged | Documents the load; does not remove it. |

## Proposal

1. **The setting (D1, D3).** Each strategy that offers **Passes: Rings** gains **Steps between rings** (`ring_step`), shown only with Rings, in the "Pattern and tolerance" section beside **Passes**:
   - `sharp`: the step of 0390, 0420 and 0497, made by exactly today's code, so its output and every golden program are byte-identical;
   - `blended`: option A.

   An operation saved without the parameter keeps `sharp`. A new operation is written with `blended` (D3), the way 0521 item 6 writes `link_height = low`, so no saved project and no golden file changes.
2. **The shape (option A, D1).** After ring k (a whole turn at `x_k` from `a_k` to `a_k + 360·sign`), the blend runs from (`x_k`, `a_k + 360·sign`) to (`x_k+1`, `a_k + (360 + φ)·sign`), X linear in A: a helix piece of lead `s_k / φ`. Ring k+1 starts where the blend ends. A turns the Rotation way throughout (OP-15), never reverses, and builds up 360° + φ a ring. The last ring has no blend. The blend is cut on the surface as the ring is: the same fit, stock to leave, zones, region, leave zones, tabs and protections, gouge test (OP-12) and splits, and the shared services link any pieces (OP-14).
3. **The blend angle (D2).** A parameter **Blend over** (`ring_blend_angle`), degrees, default 45, greater than 0 and at most 180, refused by name otherwise (NFR-07). The note gives the steepest angle the blend makes with the ring, from the stepover and the smallest cut radius (for example "steps between rings blended over 45°, at most 9° off the ring"). The alternative in D2 derives φ from a fixed largest angle off the ring instead.
4. **Per strategy.**
   - **Helical finishing and rest machining:** `helical::ring_passes` fits the blend as a `Fit` with `length: s_k`, `start: a_k + 360·sign` and `total_angle: φ`, the helix fit that already exists. Rest machining filters the blend through `wanted` as it filters the step; a blend with nothing to cut is linked as the step is. Rest's wall rings, which may be closer than the stepover, get the same φ over a shorter gap.
   - **Continuous roughing and round Facing:** a third piece, `Piece::Blend { x0, x1, a0, nodes }`, with X and A both linear, `nodes` at least `ceil(φ / max_angle_step)` and no fewer than the step's. The node estimate that refuses too fine a stepover counts the blends. `Job::layers` and the spiral's arithmetic are unchanged.
   - **Simultaneous finishing:** the blend is a short spiral-pattern `Track` from the drive angle the ring ended at to φ further round, driven as the spiral is (lead the way it travels, A smoothed within the tilt band, `axis_plan_from` starting at the ring's end), instead of `held_plan`. It is split where the section changes zone, as the step is. The face rings of 0410 and 0297 stay whole.
   - **Adaptive spacing** (0239, helical rings): added rings are separate passes as today and are not blended.
5. **What stays.** Ring placement (`helical::ring_places`), whole rings at each end, at each face and at each held region's end; entries, leaves and links, including low links (0496, 0521); 0506's combining, under which a whole ring is still one block (a blend is an XA move, so it ends the run). The Z1's stop at the end of a pure-A block (OI-993) remains, but it happens at ring engagement, not at the start of a slot.
6. **NFR-12.** The wording says the blend keeps the load nearer the stepover; it does not say the program is safe or checked. Generated G-code remains the user's to verify before running on a machine.
7. **Library.** One added parameter in five schemas and two constants; a new operation's output changes (item 1), so the library family takes a minor bump and changelog entries, as 0521 item 5 did (decision 0048).

### Tests, written first from this record

In `crates/openchuck-path/tests/spiral_rings.rs` and `helical_rings.rs` (one integration binary per crate, decision 0516), each timed uninstrumented:

- `req_op_04_req_op_11_blended_rings_are_whole_turns_joined_by_helix_pieces`: on a cylinder at three stepovers, rotations and blend angles, every ring is a whole turn at one X, every blend moves X by the gap while A turns φ the Rotation way, and the whole part is one cut where nothing splits it.
- `req_op_03_blended_ring_steps_cut_no_wider_than_near_the_stepover`: continuous roughing and round Facing, graded by an independent width-of-cut check added to `openchuck-testkit` (the tool's footprint swept over a dexel stock, as the archived model does). The step's width of cut today is the cutter's diameter; blended, it stays under a bound the lane sets from the model (for example 1.3 s at 45°).
- `req_op_12_blended_rings_leave_no_more_than_sharp`: gouge and coverage with the test kit's independent checks on a cylinder, a cone and the CrankShaft faces.
- `req_op_15_blended_rings_turn_one_way`: no reversal; A builds up 360° + φ a ring.
- `req_op_05_blended_simultaneous_rings_follow_the_spiral_drive` and `req_op_09_blended_rest_rings_cut_only_where_material_is_left`.
- `req_gc_07_sharp_steps_are_unchanged` (the golden programs byte-identical) and `req_gc_07_req_nfr_06_blended_rings_identical_at_1_2_and_n_threads`.
- `req_nfr_07_a_bad_blend_angle_is_refused_by_name` (0, negative, above 180, not finite).
- In `crates/openchuck-app/tests/operation.rs`: `req_op_11_a_new_ring_operation_blends_its_steps` and `req_gc_07_a_saved_ring_operation_keeps_sharp_steps`.

Simulation of a roughing and a finishing operation both ways, as 0521's Verification did, gives the time and the material left.

## Survey of the other transitions and links

The owner asked for every transition and link to be checked for the same kind of load. Two read-only audits of `crates/openchuck-path/src/` (the shared assembly, and each strategy) found the following. Items marked *read* were found by reading the code and are not reproduced; the follow-up lane writes a failing test first.

**The general pattern holds.** Separate passes are joined by a leave, a link and an entry (`assemble.rs`), so a step-over *between passes* is never a feed move through material: axial and indexed finishing step in air past the ends, Flat and Box Facing rows are separate passes, low links (0496, 0521) end with a vertical descent, and pass ordering (0520) and 0506 add no corners. The load problems are inside one joined pass, or at the first pass of a layer.

| # | Where | What happens | Severity | Follow-up |
| --- | --- | --- | --- | --- |
| 1 | Ring steps, all five ring strategies | A 90° corner and a slot at depth (above) | High in roughing, medium in finishing | This proposal |
| 2 | Facing, End shape (`facing/end.rs`, the four-point pass) | Each pass runs along X from air into the end face at full band depth (up to the flute length), turns 90° in the material and runs across. Decision 0252 asks for "validated engagement"; `plan_end` checks only that the axial stepdown is no more than the diameter. | High | OI-2115 |
| 3 | First pass of each roughing layer: line 0 of continuous roughing along the length, the held-end turn of the spiral or rings (0161), line 0 of indexed roughing, round Facing's end rings with the Part extent | A full-width slot at the layer's depth, for a whole line or turn per layer. Inherent to a first cut, but nothing bounds or warns of it. | High at held ends and along the length | OI-2114 (with OI-1373 and OI-151) |
| 4 | Entry direction (`assemble/entry.rs`, `direction_at_start` and `direction_at_end`) | The direction is taken in the fixed (along, across) frame, so a pure-A pass has none: ramp and lead fall back to a plunge, helix uses +X, the back-and-forth ramp refuses. In a ring chain the first direction found is the X step a whole turn later, so a ramp runs along X behind the start and a lead arc ends tangent to X, then the cut turns 90° into A at depth. Decision 0026 item 5 says the lead "follows the pass's A as the cut does"; as built it is at the fixed A of the pass's first point. *read* | Medium | OI-2112 |
| 5 | Lead-in and ramp starts under low links (`entry.rs`, lead and ramp) | The lead-in rapids sideways at the link height to the arc's start (up to r√2 away) with no stock check there; the ramp's start check compares against `here.height - top`, which is near zero when the link is low, so it cannot refuse, and the line between the head and the ramp's start is not sampled. Either can be a rapid into stock that is higher beside the pass. *read* | High if it happens; needs lead or ramp entry with low links or in roughing | OI-2113 |
| 6 | Lead arcs in roughing (`entry.rs`) | The arc's side is chosen against the finished surface only, not the pass stock, so in roughing the arc can swing through uncut stock at full depth and the cutting feed. 0026 offers the lead "for finishing" but nothing stops roughing from choosing it. *read* | Medium | OI-2113 |
| 7 | Drops off a wall or a raised floor (`assemble/walls.rs`, roughing's `tip = max(floor, level)`) | The near-radial step down is a `Cutting` move at the cutting feed, not the plunge feed. *read* | Medium in roughing | OI-2116 |
| 8 | Run-outs past the stock on ring and helical passes (`extend.rs`, 0212) | A run-out goes along X at the end point's A, perpendicular to a ring or helix, side-milling any stock between the stock end and the part, then turns 90° into the ring at depth. | Low to medium (finishing allowance) | OI-2117 |
| 9 | Leaves (`assemble.rs`) | Except the lead, a pass ends with a straight lift along the tool axis; the tool stops at depth first. | Low | none |
| 10 | Free-end layers (0344) and finishing layers (0345) | Each layer's first turn or pass is up to a full-width groove at the finishing feed; the steps are in air. Already recorded: 0344 "Full-width engagement", 0345 "no engagement is promised", OI-1373. | Medium | OI-1373 |
| 11 | Simultaneous spiral and rings splits (`simultaneous.rs`) | Where `held_plan` or `axis_plan_from` cannot hold the angle, the pieces are linked in air. | None (good) | none |
| 12 | Roughing spiral's knots at a face turn (0297) | The helix meets the face's turn with a kink of atan(pitch / 2πr): about 3° at r = 10 mm, 13° near the axis; engagement falls and rises gradually. | Low | none |
| 13 | Pencil creases (`pencil.rs`) | Creases are chained at any vertex with two crease edges, with no limit on the turn, so an inside corner can be a 90° turn where most material is left. | Medium-low | none yet; noted here for the owner |
| 14 | Rest machining depth | Rest has no layers, so a groove narrower than the earlier tool, still full of stock, is taken at full depth. | Medium (depth, not a transition) | OI-1373 |

Not examined in depth: swarf (one flank pass a face, OI-241 already records its missing tangent lead), face-flow finishing (separate passes, reversed for a meander, so its steps are in air), drilling.

## Owner's decisions

- **D1, the step's shape.** A (whole ring, then a blend; recommended), B (blend in place of the ring's end), C (sharp step at a lower feed), or F (warn only). A keeps every whole-ring guarantee the accepted records rely on; B is shorter but needs special cases and was declined once.
- **D2, the blend's length.** A blend angle with a default of 45° (recommended: the user sees it in A, and the note reports the steepest angle off the ring), or a fixed largest angle off the ring from which φ is derived per ring.
- **D3, defaults.** Saved operations keep `sharp` (byte-identical, as every ring record kept its golden files) and new operations start `blended` (recommended), or `sharp` for new operations as well.
- **D4, scope and order.** One lane for all five strategies (recommended: one shared helper, one parameter), or roughing and round Facing first, where the load is, then finishing, rest and simultaneous.
- **D5, the survey.** Which of OI-2112 to OI-2117 to queue, and in what order. Recommended: OI-2113 first (a possible rapid into stock; write the failing test), then OI-2115 (End Facing), then OI-2112, OI-2114, OI-2116 and OI-2117.

## Documents (when accepted)

- No change to the requirements or the architecture: a setting of strategies that already have rings (OP-03, OP-04, OP-05, OP-09, OP-24), using the shared services (OP-11, OP-14).
- UI specification, section 8: **Steps between rings** and **Blend over** beside **Passes**.
- Verification plan, "Rings instead of a spiral": the blended cases.
- Work breakdown: the RINGBLEND lane row; the changelogs of `openchuck-path` and `openchuck-app`.

## Verification

What was done, 8 Nov 2026, on `main` `1f3675d`; no code was changed.

- **Code read.** The ring steps in `helical.rs` (`ring_passes`, `join`), `roughing.rs` (`continuous_rings`, `layers`), `rest.rs` and `simultaneous.rs` (`ring_passes`, `held_plan`), the assembly (`assemble.rs`, `assemble/entry.rs`) and the strategies listed in the survey. The survey's entries come from two read-only audits of the same tree; items 2 and 4 were checked against the code by this session (`facing/end.rs` builds the four-point air, in, across, out pass; `entry.rs` takes the direction in the along-across frame), the others are marked *read* where they are not reproduced.
- **Engagement model.** [`docs/archive/0543-smooth-ring-steps/`](../archive/0543-smooth-ring-steps/) holds the script and its output. It is a 2D model of one roughing layer, unrolled (X along the part, arc length round it at a 10 mm cut radius), with a 3.175 mm flat end mill at a 1.27 mm (40 %) stepover over nine rings, a 0.02 mm grid, the width of cut averaged over 0.5 mm of path, and the first turn left out (a slot on every path). It is independent of the product code and measures the shapes, not the product's output. Results (width of cut, mm):

  | Path | Corner off the ring | Ring, median | Step or blend, median and peak |
  | --- | --- | --- | --- |
  | Spiral | 0° | 1.24 | – |
  | Rings, sharp step (today) | 90° | 1.26 | 2.45, 2.95 |
  | Option A, φ = 30° | 13.6° | 1.26 | 0.97, 1.65 |
  | Option A, φ = 45° | 9.2° | 1.26 | 0.86, 1.52 |
  | Option A, φ = 90° | 4.6° | 1.26 | 0.75, 1.43 |
  | Option B, φ = 45° | 9.2° | 1.26 | 1.27, 1.52 |

- **Not verified.** Nothing was run on the product or on a machine. The finishing and simultaneous load figures are not measured, the Z1's stop at a pure-A block's end is from 0506's reading of the firmware source (OI-993), and survey items marked *read* need their failing test first. Generated G-code remains the user's to verify before running on a machine (NFR-12).
