# 0544: A lead on a pass that turns A stays in the plane of the cut, and the move to an entry's start is checked against the stock (OP-11, OP-13, OP-14, OP-15, SET-09, NFR-07, NFR-12, GC-07, NFR-06; issues #640 and #642; lane RAMPENTRY)

- **Status:** accepted by the project owner on 8 Nov 2026, relayed by the coordinator (decision 0049), with the roughing clause of item 6 as he stated it (reworded below). Proposed the same day by lane RAMPENTRY from issue #640 (a defect, sev3: Plunge or Helix is the workaround). It applies the principle of [decision 0543](0543-a-ramp-entry-follows-the-toolpath.md) (an entry or exit follows the toolpath) to the lead, as the owner decided for #639's D8. The owner's answers (relayed): every recommendation L1 to L5 as a package; item 6 (issue #642) is part of the same decision.
- **What it would amend:**
  - accepted [decision 0026](0026-entry-moves.md) items 4 and 5 for the lead on passes that start or end by turning A: the arcs lie in the plane of the cut at the pass's X (L1), A moves through them (L5), and only the move that meets the surface carries a contact point (L3). Line-pass leads are unchanged (L2).
  - accepted [decision 0204](0204-new-operation-strategy-list-and-inspector-heading.md) item 4 for the lead: a pass that only turns A gets a lead, and a single-point pass keeps its plunge with a path note (L4).
  - accepted [decisions 0496](0496-low-links-between-finishing-passes.md) and [0521](0521-low-links-for-helical-rest-and-simultaneous-finishing.md) ("entries and leaves are not changed") and [0033](0033-slice-14-rotary-roughing.md) item 6 (roughing's local links): the move from the end of a link to an entry's start is now checked as the link is (item 6).
- **Decisions it uses:** [0390](0390-helical-finishing-in-rings.md), [0420](0420-rings-for-rest-roughing-and-round-facing.md) and [0497](0497-rings-for-the-simultaneous-spiral.md) (rings); [0161](0161-tools-keep-clear-of-the-gripped-length.md); [0241](0241-continuous-rotary-operation-boundaries.md); [0536](0536-message-levels-operation-reference-and-setting-links.md) (message form, proposed: guidance for new wording).
- **Date:** 8 Nov 2026.
- **Open items:** none new.

Numbering: 0544 is final, after 0543 (confirmed by the Merge Manager on 9 Nov 2026). No project schema number.

## Context

The lead entry is a quarter circle meant to be tangent to the cut (0026 items 1 and 4). It takes its direction from the pass's first move that changes machine X or Y (`direction_at_start`; `direction_at_end` for the lead-out), the rule decision 0543 replaces for the ramp. On passes that turn A (rings, the spiral) the tool hardly moves sideways in the machine; the part turns under it. So the arcs are drawn in the machine X-Y plane, tangent to the X axis, while the cut runs around the part: the lead-in comes in along the axis from beyond the start of the cut, and the lead-out carries on along the axis, past the end of the part, or into the part where it rises. The arcs are not tangent to the cut, so 0026 item 4's premise for the cutting feed and a contact point does not hold. A pass that only turns A has no sideways move: its lead-in silently becomes a plunge and it gets no lead-out.

### What was measured (issue #640's probe, re-run by this lane)

The issue's throwaway probe was re-run in release mode on `main` `1f3675d` on 8 Nov 2026 and not committed. Its output matches the issue's line for line. Synthetic fixtures (cylinder radius 15 mm, length 40 mm, 6 mm ball; cone 8 to 12 mm, 6 mm flat; stock radius 16 mm; lead radius 0, so the tool radius, 3 mm), not the owner's projects and not machine results:

| Strategy, passes | Lead-in | Lead-out | Result |
| --- | --- | --- | --- |
| Helical finishing, Spiral | X -3 to 0, across 0 to 3 mm, A fixed at 0, while the cut turns A 50 degrees over 20 moves | X 40 to 43 mm, beyond the part's end | generated |
| Helical finishing, Rings | the same | the same | generated |
| Continuous roughing, Spiral and Rings | — | — | refused: "pass 3: the lead-out needs an arc of radius 3.000 mm, and the tool would cut 0.317 mm into the finished surface at position 12 of its path" |

On the cone the lead-out continues along +X, where the part rises. On finishing the arcs reach 3 mm beyond each end of the part; a held region there would be reached (not measured: the probe's keep-out began 1 mm beyond the arc).

## Context, issue #642

Read on `main` `1f3675d` and reproduced by this lane (`crates/openchuck-path/tests/entry_link_stock.rs`, synthetic fixtures): a link ends above the pass's first cut point at a height checked against the stock under the tool's footprint along the link. The lead-in (`Entry::lead_in`) and the ramp (`Entry::ramp`) then move sideways at that height to their own start, which is not over the first cut point, and nothing checks that move. Under a low link (0496, 0521) or roughing's local link that height is only the approach distance above the stock along the link. With a block of stock 19.5 mm from the axis beside the first cut point (outside the one-tool-radius footprint of the link's check), two line passes, the second linked low: the lead-in's sideways rapid at 17.0 mm enters the block at X 19.16 mm, and a 10 degree ramp's at X 18.81 mm; the plunge on the same stock stays clear. In roughing a lead arc's side is chosen only against the finished surface, so it can swing through uncut stock at full depth (issue #642 item 3; not yet reproduced).

## Decision

On a pass that starts or ends by turning A, the lead-in and lead-out are tangent to the cut, as on a line pass, and stay in the plane of the cut at the pass's X:

1. **Geometry (L1 a).** The lead-in is a quarter circle of the lead radius in the plane that holds the tool axis and the cut's direction around the part, at the first cut point's machine X and Y: the tool comes down beside the start of the cut (by the stock-clear approach and a plunge, as today), then follows the arc, moving A and Z together, and meets the cut tangentially at the first cut point. In the unrolled frame (the distance around the part at the tip's radius, and the height), the arc starts one lead radius before the first point and one lead radius above it. The lead-out leaves the last point the same way, rising, turning the Rotation way.
2. **Which passes (L2 a).** Passes that start or end by turning A: rings, the spiral, partial rings, and simultaneous finishing's rings and spiral, told apart by the pass shape of decision 0543 section 2. Line passes keep today's horizontal arcs, byte-identical.
3. **Feed and contact (L3 a).** The cutting feed, with a contact point only on the move that meets the surface, timed by the shared rules for simultaneous moves (OP-13); the moves above the surface carry none.
4. **Passes with no direction (L4 a).** A pass that only turns A gets the lead of item 1. A single-point pass stays a plunge (0204 item 4) and gets no lead-out, and a path note (information, decision 0536) names the passes.
5. **A at the start (L5 a).** The link arrives at the A where the arc starts, and A turns the Rotation way through the arc and the ring, never reversing (OP-15). The extra A counts against the A range (0241); where it does not fit, generation is refused naming the A range (Setup step, Machine).
6. **The move to an entry's start (issue #642).** For every entry style, every move at the link height from where the link ends to the entry's start is checked against the stock along its whole line under the tool's footprint, as the link itself is (0496 item 3); the pass's own stock is used where it carries one (roughing), else the setup's. Where it is not clear, the tool rises first to the height the check gives (never above the retract height) and an information note counts such entries; where even the retract height is not clear, the operation is refused naming the pass and the entry (NFR-07, decision 0536 form). **Lead arcs in roughing (a pass that carries its stock), as the owner stated it:**
   - on a **full ring** or a **spiral**, the lead arc of item 1 is allowed: it lies over cut the pass makes anyway, as the ring ramp does (decision 0543 section 3);
   - on a **partial ring** (a cut that turns A only part of a turn), every position of the arc above the point where it meets the cut must lie above the stock the pass carries (probed along the tool axis under the tool's footprint, from the retract height); otherwise the operation is refused: "Lead entry (operation '<name>'): pass N: the lead-in on an arc of radius R mm would pass through stock the pass has not cleared, beside a cut that turns A only part of a turn. Set Entry to Plunge, Ramp or Helix.";
   - on a **line pass**, the lead keeps today's arcs, and the move to its start is checked as this item says. With decision 0543 the ramp starts over its own path, so the ramp's part of #642 goes away; the check still covers the rapid to the ramp's start.
7. **A single-point pass with a lead.** It keeps 0204 item 4's plunge (L4), but only when the tool can plunge (`Tool::can_plunge`); otherwise the operation is refused naming the tool. This closes for the lead the gap decision 0543 D7 closes for the ramp (today the plunge is made whether or not the tool can plunge).

**Why it is safe.** On a full ring, the stretch before the first point and after the last is the ring's own path, so the lead lies over cut the ring makes anyway, raised along the tool axis: the dominance argument of decision 0543 section 3. On a spiral and a partial ring it lies just off the cut at the same X; the gouge drop on every arc position, the keep-out check and the collision oracle stay, and a lead that would cut the finished surface or reach a held region is refused in the form of decision 0536, naming the Lead radius and the entry styles that remain.

## Owner decisions (answered 8 Nov 2026, relayed by the coordinator)

All recommendations accepted as a package. Items 6 and 7 follow from the same answers and from decision 0543 D7; the coordinator treats them as part of this decision. Item 6's behaviour was confirmed by the owner on 8 Nov 2026 (relayed): rise first to the checked height, with an information note counting such entries, and refuse by name only when even the retract height is not clear.

| # | Question | Options | Recommendation |
| --- | --- | --- | --- |
| L1 | Lead geometry on passes that turn A | (a) an arc in the plane of the cut, at the pass's X, moving A and Z together; (b) an arc in the part's surface in the unrolled frame (X and the distance around), moving X and A together at constant height; (c) leave the arcs and refuse a lead on passes that turn A | **(a).** It never leaves the ring's X, so it cannot reach the held region, the next ring or a part that rises along X. (b) moves sideways in X, the direction that fails today. |
| L2 | Which passes | (a) passes that start or end by turning A; line passes byte-identical; (b) every pass, with a choice of horizontal or vertical lead | **(a).** On line passes the arc is already tangent to the cut. |
| L3 | Feed and contact on an L1 (a) arc | (a) the cutting feed, contact only on the move that meets the surface, OP-13 timing; (b) the plunge feed with no contact point, as a ramp | **(a).** It is a finishing lead; the tool reaches the surface only at the end of the arc. |
| L4 | A pass that only turns A, or a single-point pass | (a) the L1 lead for a pass that only turns A; a single-point pass stays a plunge with a path note; (b) as today: plunge, silently | **(a).** The silent plunge is part of this defect. |
| L5 | A at the start of a lead | (a) the link arrives at the arc's start A; A turns the Rotation way, never reversing; the extra A counts against the A range, with a refusal naming it | **(a)**, the same as the ring ramp of 0543. |

## Alternatives considered

- **L1 (b), an arc in the surface.** The closest analogue of today's line-pass arc, but it moves along X into the held region or a rising part, which is the failure measured above.
- **L1 (c), refuse.** Keeps a refusal where a lead can be placed safely, on the strategies (finishing in rings) where a lead is most useful.

## Consequences

- **Code.** `openchuck-path` `assemble/lead.rs` (new: the lead on passes that turn A), `assemble/entry.rs` (dispatch; the checked move to an entry's start, item 6), `assemble.rs` (the link and `fit_range` with the arc's A). No API change beyond decision 0543's.
- **Output.** Saved operations with Lead entry on passes that turn A give a different program at the next version; golden files are regenerated with a changelog entry and a release-notes line. Line-pass leads are byte-identical.
- **Documents (agreed with the owner's acceptance; edited with the code pull request, architecture section 5.2 after it):** verification plan row OP-11 (the lead on a pass that turns A); UI specification Entry row ("With `lead`, a matching arc also leaves the cut", gains "on a ring or spiral the arcs turn with the part, in the plane of the cut"); user guide Entry row. No change to `docs/requirements.md` or `docs/architecture.md`.
- **Not proved.** The geometry checks say nothing about chip load or tool suitability; no machine trial is claimed. Generated G-code remains the user's to verify before running it on a machine (NFR-12).

## Verification (planned; filled in when built)

Tests written first, named by requirement, in `crates/openchuck-path/tests/entry.rs` (items 1 to 5 and 7 as built: `tests/lead_follows.rs`, below):

- On rings and the spiral, the lead-in and lead-out stay at the pass's X and are tangent to the cut where they meet it (`req_op_11_*`).
- A turns only the Rotation way (`req_op_15_*`); the A range refusal is named (`req_nfr_07_*`).
- A pass that only turns A gets a lead, not a silent plunge; a single-point pass is plunged with a note.
- Feed, contact and duration follow L3 (`req_op_13_*`).
- A held region at the starting end, and the cone that rises along X, are no longer refused (`req_set_09_*`).
- Line-pass leads byte-identical to `main`; output the same at 1, 2 and N threads (GC-07, NFR-06).
- Public stand-ins: the blisk STEP part and the blisk start example with Lead entry on helical finishing in Rings.
- Issue #642 (`entry_link_stock.rs`, written first and failing on `main`): the rapid to a lead-in's start and to a ramp's start never sweeps the tool through the stock under a low link; the plunge control stays clear; where only the retract height is clear the entry rises and the note counts it; where nothing is clear the refusal names the pass; in roughing a lead arc never passes through uncut stock.
- A single-point pass under Lead with a tool that cannot plunge is refused naming the tool.

### Items 1 to 5 (lane RAMPLEAD, issue #640)

Built on `claude/lane-rampentry-lead-wip` (stacked on lane RAMPENTRY's branch, after its merge of `main` at library 0.52.0), in `crates/openchuck-path/src/assemble/lead.rs`, a child module of `entry.rs` that shares its surface check and arc spacing; dispatched from `Entry::enter`, `Entry::exit` (which now also takes the pass's stock view and rapids) and `Entry::start`. As built:

- **Geometry (item 1).** In the unrolled frame (the distance round the part at the end point's tip radius, and the height) the arc has the lead radius and turns, on the side away from the part, from straight down to the cut's direction at the first point (lead-in), or from the cut's direction at the last point to straight up (lead-out). On a level cut that is the quarter circle of item 1, one lead radius back (or on) and one up. On a cut that falls or rises round the part it is shorter or longer than a quarter: a quarter circle tangent to a falling cut would start by moving back round the part, turning A against the pass (found by the lobed-part test below and fixed before it was pushed). The tool holds the end point's machine X and Y throughout; A is `a0 + sign * s / rho`.
- **Which passes (item 2).** Each end of a pass is told apart on its own by the pass shape of decision 0543 section 2 (`pass_shape::start`, and the same read backwards for the end): a full ring, a spiral or a partial ring gets this lead; a line or a single point keeps the line-pass code path unchanged. An end whose tip is within the path tolerance of the rotary axis (an end face's ring) has no plane of the cut: a pass that starts there is entered by a plunge and gets no lead-out, and one that ends there gets no lead-out; each is named in a "Lead entry" note in its own words ("starts on the rotary axis, where a lead arc has no plane to follow", "ends on the rotary axis"), not as a single point (decision 0543 point 2, the owner's answer of 8 Nov 2026, relayed).
- **Feed and contact (item 3).** `LeadIn` and `LeadOut` at the cutting feed; no contact point on any arc move except the lead-in's last, which meets the surface with the first cut point's contact. Timing is the shared rule for simultaneous moves (OP-13).
- **No direction (item 4).** A pass that only turns A is a ring and gets the lead. A single-point pass keeps the plunge; one information note with the new topic `notice::topic::LEAD_ENTRY` ("Lead entry") names every such pass. Item 7 is checked in `openchuck-app` by lane RAMPENTRY for Ramp and Lead alike.
- **A (item 5).** `Entry::start` gives the arc's start, so the link arrives at its A; A turns the pass's way through the arc and the pass. Every arc A is checked against a limited A range and refused naming A range.
- **Roughing (item 6's lead clause, reading (a)).** In a pass that carries its stock, an arc beside a full ring or a spiral is allowed; beside a partial ring every arc position must lie above the pass's stock, probed under the tool's footprint from the retract height, else the operation is refused naming Entry.
- **Held regions.** The arc never leaves the pass's X, so it reaches a held region only where the pass does, which generation already refuses; the turning lead's refusal has no held-region form.

**Tests** (`crates/openchuck-path/tests/lead_follows.rs`, a module of the integration binary; uninstrumented times from `cargo nextest run`, all under 4 s). The first eight were written before the code: seven failed on RAMPENTRY's `36b0327` without it; the feed test passed there too, as the old arcs already run at the cutting feed.

- `req_op_11_req_op_15_a_lead_on_rings_and_the_spiral_stays_at_the_pass_x_and_meets_the_cut` (helical finishing, Rings and Spiral, cylinder; X and Y held, A one way, tangent within one segment's angle, start one radius up);
- `req_op_11_a_pass_that_only_turns_a_gets_a_lead_not_a_silent_plunge` (the rings stub on a round part);
- `req_op_11_req_op_13_a_turning_lead_is_fed_at_the_cutting_feed_with_a_contact_only_where_it_meets_the_surface`;
- `req_op_11_req_set_09_a_turning_lead_never_reaches_a_held_region_at_the_starting_end` (held region from 0.5 mm beyond the part's end);
- `req_op_11_req_op_14_continuous_roughing_of_a_rising_part_with_a_lead_is_no_longer_refused` (the issue's cone, Rings and Spiral);
- `req_op_11_req_nfr_07_a_turning_lead_outside_the_a_range_is_refused_naming_it`;
- `req_op_11_req_nfr_06_turning_leads_are_the_same_at_one_two_and_many_threads` (1, 2 and 8 threads);
- `req_op_11_a_single_point_pass_under_lead_is_plunged_with_a_note_naming_it`;
- `req_op_11_req_op_15_a_partial_ring_gets_the_lead_in_the_plane_of_the_cut_at_both_ends`;
- `req_op_11_req_op_14_in_roughing_a_lead_beside_a_partial_ring_through_uncut_stock_is_refused_naming_entry` (and a whole ring in the same stock is allowed);
- `req_op_11_req_nfr_07_a_turning_lead_that_would_cut_the_finished_surface_is_refused_naming_lead_radius` (a twelve-lobed part, ring starting in a valley; the same ring starting on a crest gets its lead with A one way);
- `req_op_11_a_turning_pass_with_its_tip_on_the_rotary_axis_is_plunged_with_a_note_and_no_lead_out`;
- `req_op_11_a_turning_pass_that_ends_on_the_rotary_axis_has_no_lead_out_and_a_note` (a pass that turns half a turn at 5 mm, then comes in to the axis);
- `req_op_11_req_op_15_helical_finishing_of_the_blisk_in_rings_gives_each_end_the_lead_its_shape_asks_for` (`testdata/stl/blisk.stl`, the public stand-in: helical finishing in Rings there is one pass that starts and ends with a short line along X, so both its leads are line leads; the planned blisk stand-in does not exercise the new arc).

One existing test changed: `entry::req_op_11_req_op_14_req_op_15_clear_pass_transitions_without_a_reset_preserve_motion` assumed that a lead-out on its spiral passes ends at the clearance height; only that precondition was relaxed for Lead (the lead-out now rises one lead radius). The whole `openchuck-path` suite passed (846 of 846, 20 min 30 s uninstrumented) at the commit before the last change; after it, the 78 tests of the lead, entry, ramp, #642 and ring files passed. Line coverage of `lead.rs` on its own tests is 97.3% (`cargo llvm-cov`); the lines not reached are three defensive returns for an empty pass and the rise to the arc's start when a low link arrives below it.

**Line-pass leads, byte-identical.** A throwaway probe (not committed) dumped the full segments and notices of 16 line-pass programs with Lead entry (axial finishing, indexed finishing, rotary roughing indexed and along the length; a 15 mm cylinder with a 6 mm ball and the 8 to 12 mm cone with a 6 mm flat; lead radius 0 and 2 mm): 6,364,071 bytes, identical before and after the change (md5 `ad8ed4bdd985127008b5c954c6968422`).

**Issue #640's probe, re-run on the change** (release, not committed; synthetic fixtures; lead radius the tool radius, 3 mm):

| Strategy, passes | Lead-in | Lead-out | Result |
| --- | --- | --- | --- |
| Helical finishing, Spiral and Rings (with and without the held region) | X 0 held, A -11.2 to 0 degrees, then the cut | X 40 held, A turning 11.2 degrees on | generated (was: arcs along X from -3 and to 43 mm) |
| Continuous roughing on the cone, Rings | X held at the ring's X, A turning about 12 to 14 degrees into the ring | the same, out | generated (was refused: the lead-out cut 0.317 mm into the part) |
| Continuous roughing on the cone, Spiral | the same | the same | generated (was refused); the spiral's four single-point passes at the cone's top are plunged and named in one note |

**Not verified.** No machine trial; the geometry checks say nothing about chip load or tool suitability. Generated G-code remains the user's to verify before running it on a machine (NFR-12).

## Open items

None new.
