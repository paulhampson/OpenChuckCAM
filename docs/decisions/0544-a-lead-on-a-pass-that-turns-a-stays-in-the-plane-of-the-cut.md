# 0544: A lead on a pass that turns A stays in the plane of the cut, and the move to an entry's start is checked against the stock (OP-11, OP-13, OP-14, OP-15, SET-09, NFR-07, NFR-12, GC-07, NFR-06; issues #640 and #642; lane RAMPENTRY)

- **Status:** proposed (8 Nov 2026) by lane RAMPENTRY from issue #640 (a defect, sev3: Plunge or Helix is the workaround). It applies the principle of [decision 0543](0543-a-ramp-entry-follows-the-toolpath.md) (an entry or exit follows the toolpath) to the lead, as the owner decided for #639's D8. The owner's answers, relayed by the coordinator on 8 Nov 2026 (decision 0049): every recommendation L1 to L5 accepted as a package. Item 6 (issue #642, the move to an entry's start) was added to this record by the coordinator as part of the same decision, accepted by default unless the owner says otherwise. The record itself is accepted only when the owner has read it in full.
- **What it would amend:**
  - accepted [decision 0026](0026-entry-moves.md) items 4 and 5 for the lead on passes that start or end by turning A: the arcs lie in the plane of the cut at the pass's X (L1), A moves through them (L5), and only the move that meets the surface carries a contact point (L3). Line-pass leads are unchanged (L2).
  - accepted [decision 0204](0204-new-operation-strategy-list-and-inspector-heading.md) item 4 for the lead: a pass that only turns A gets a lead, and a single-point pass keeps its plunge with a path note (L4).
  - accepted [decisions 0496](0496-low-links-between-finishing-passes.md) and [0521](0521-low-links-for-helical-rest-and-simultaneous-finishing.md) ("entries and leaves are not changed") and [0033](0033-slice-14-rotary-roughing.md) item 6 (roughing's local links): the move from the end of a link to an entry's start is now checked as the link is (item 6).
- **Decisions it uses:** [0390](0390-helical-finishing-in-rings.md), [0420](0420-rings-for-rest-roughing-and-round-facing.md) and [0497](0497-rings-for-the-simultaneous-spiral.md) (rings); [0161](0161-tools-keep-clear-of-the-gripped-length.md); [0241](0241-continuous-rotary-operation-boundaries.md); [0536](0536-message-levels-operation-reference-and-setting-links.md) (message form, proposed: guidance for new wording).
- **Date:** 8 Nov 2026.
- **Open items:** none new.

Numbering: provisional, 0544 after 0543 (see that record). The Merge Manager assigns the final numbers at release. No project schema number.

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

## Decision (proposed)

On a pass that starts or ends by turning A, the lead-in and lead-out are tangent to the cut, as on a line pass, and stay in the plane of the cut at the pass's X:

1. **Geometry (L1 a).** The lead-in is a quarter circle of the lead radius in the plane that holds the tool axis and the cut's direction around the part, at the first cut point's machine X and Y: the tool comes down beside the start of the cut (by the stock-clear approach and a plunge, as today), then follows the arc, moving A and Z together, and meets the cut tangentially at the first cut point. In the unrolled frame (the distance around the part at the tip's radius, and the height), the arc starts one lead radius before the first point and one lead radius above it. The lead-out leaves the last point the same way, rising, turning the Rotation way.
2. **Which passes (L2 a).** Passes that start or end by turning A: rings, the spiral, partial rings, and simultaneous finishing's rings and spiral, told apart by the pass shape of decision 0543 section 2. Line passes keep today's horizontal arcs, byte-identical.
3. **Feed and contact (L3 a).** The cutting feed, with a contact point only on the move that meets the surface, timed by the shared rules for simultaneous moves (OP-13); the moves above the surface carry none.
4. **Passes with no direction (L4 a).** A pass that only turns A gets the lead of item 1. A single-point pass stays a plunge (0204 item 4) and gets no lead-out, and a path note (information, decision 0536) names the passes.
5. **A at the start (L5 a).** The link arrives at the A where the arc starts, and A turns the Rotation way through the arc and the ring, never reversing (OP-15). The extra A counts against the A range (0241); where it does not fit, generation is refused naming the A range (Setup step, Machine).
6. **The move to an entry's start (issue #642).** For every entry style, every move at the link height from where the link ends to the entry's start is checked against the stock along its whole line under the tool's footprint, as the link itself is (0496 item 3); the pass's own stock is used where it carries one (roughing), else the setup's. Where it is not clear, the tool rises first to the height the check gives (never above the retract height) and an information note counts such entries; where even the retract height is not clear, the operation is refused naming the pass and the entry (NFR-07, decision 0536 form). In a pass that carries its stock (roughing), a lead arc is placed only where the stock along it is already cleared (the arc's positions lie above the stock the pass carries, by the approach distance at the arc's start and at no depth below the cleared stock along it); otherwise the other side is tried, and then the operation is refused naming Entry ("set Entry to Plunge, Ramp or Helix"). With decision 0543 the ramp starts over its own path, so the ramp's part of #642 goes away; the check still covers the rapid to the ramp's start.
7. **A single-point pass with a lead.** It keeps 0204 item 4's plunge (L4), but only when the tool can plunge (`Tool::can_plunge`); otherwise the operation is refused naming the tool. This closes for the lead the gap decision 0543 D7 closes for the ramp (today the plunge is made whether or not the tool can plunge).

**Why it is safe.** On a full ring, the stretch before the first point and after the last is the ring's own path, so the lead lies over cut the ring makes anyway, raised along the tool axis: the dominance argument of decision 0543 section 3. On a spiral and a partial ring it lies just off the cut at the same X; the gouge drop on every arc position, the keep-out check and the collision oracle stay, and a lead that would cut the finished surface or reach a held region is refused in the form of decision 0536, naming the Lead radius and the entry styles that remain.

## Owner decisions (answered 8 Nov 2026, relayed by the coordinator)

All recommendations accepted as a package. Items 6 and 7 follow from the same answers and from decision 0543 D7; the coordinator treats them as part of this decision.

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
- **Documents (proposed, not edited until the owner agrees):** verification plan row OP-11 (the lead on a pass that turns A); UI specification Entry row ("With `lead`, a matching arc also leaves the cut", gains "on a ring or spiral the arcs turn with the part, in the plane of the cut"); user guide Entry row. No change to `docs/requirements.md` or `docs/architecture.md`.
- **Not proved.** The geometry checks say nothing about chip load or tool suitability; no machine trial is claimed. Generated G-code remains the user's to verify before running it on a machine (NFR-12).

## Verification (planned; filled in when built)

Tests written first, named by requirement, in `crates/openchuck-path/tests/entry.rs`:

- On rings and the spiral, the lead-in and lead-out stay at the pass's X and are tangent to the cut where they meet it (`req_op_11_*`).
- A turns only the Rotation way (`req_op_15_*`); the A range refusal is named (`req_nfr_07_*`).
- A pass that only turns A gets a lead, not a silent plunge; a single-point pass is plunged with a note.
- Feed, contact and duration follow L3 (`req_op_13_*`).
- A held region at the starting end, and the cone that rises along X, are no longer refused (`req_set_09_*`).
- Line-pass leads byte-identical to `main`; output the same at 1, 2 and N threads (GC-07, NFR-06).
- Public stand-ins: the blisk STEP part and the blisk start example with Lead entry on helical finishing in Rings.
- Issue #642 (`entry_link_stock.rs`, written first and failing on `main`): the rapid to a lead-in's start and to a ramp's start never sweeps the tool through the stock under a low link; the plunge control stays clear; where only the retract height is clear the entry rises and the note counts it; where nothing is clear the refusal names the pass; in roughing a lead arc never passes through uncut stock.
- A single-point pass under Lead with a tool that cannot plunge is refused naming the tool.

## Open items

None new.
