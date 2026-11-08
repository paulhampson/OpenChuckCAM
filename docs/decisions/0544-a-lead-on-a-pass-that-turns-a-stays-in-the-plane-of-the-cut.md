# 0544: A lead on a pass that turns A stays in the plane of the cut (OP-11, OP-13, OP-14, OP-15, SET-09, NFR-07, NFR-12, GC-07, NFR-06; issue #640; lane RAMPENTRY)

- **Status:** proposed (8 Nov 2026) by lane RAMPENTRY from issue #640 (a defect, sev3: Plunge or Helix is the workaround). It applies the principle of [decision 0543](0543-a-ramp-entry-follows-the-toolpath.md) (an entry or exit follows the toolpath) to the lead, as the owner decided for #639's D8. The options L1 to L5 are put to the owner by the coordinator (decision 0049); none is answered yet, and nothing is built before its answer.
- **What it would amend:**
  - accepted [decision 0026](0026-entry-moves.md) items 4 and 5 for the lead on passes that start or end by turning A: the arcs lie in the plane of the cut at the pass's X (L1), A moves through them (L5), and only the move that meets the surface carries a contact point (L3). Line-pass leads are unchanged (L2).
  - accepted [decision 0204](0204-new-operation-strategy-list-and-inspector-heading.md) item 4 for the lead: a pass that only turns A gets a lead, and a single-point pass keeps its plunge with a path note (L4).
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

## Decision (proposed)

On a pass that starts or ends by turning A, the lead-in and lead-out are tangent to the cut, as on a line pass, and stay in the plane of the cut at the pass's X:

1. **Geometry (L1 a).** The lead-in is a quarter circle of the lead radius in the plane that holds the tool axis and the cut's direction around the part, at the first cut point's machine X and Y: the tool comes down beside the start of the cut (by the stock-clear approach and a plunge, as today), then follows the arc, moving A and Z together, and meets the cut tangentially at the first cut point. In the unrolled frame (the distance around the part at the tip's radius, and the height), the arc starts one lead radius before the first point and one lead radius above it. The lead-out leaves the last point the same way, rising, turning the Rotation way.
2. **Which passes (L2 a).** Passes that start or end by turning A: rings, the spiral, partial rings, and simultaneous finishing's rings and spiral, told apart by the pass shape of decision 0543 section 2. Line passes keep today's horizontal arcs, byte-identical.
3. **Feed and contact (L3 a).** The cutting feed, with a contact point only on the move that meets the surface, timed by the shared rules for simultaneous moves (OP-13); the moves above the surface carry none.
4. **Passes with no direction (L4 a).** A pass that only turns A gets the lead of item 1. A single-point pass stays a plunge (0204 item 4) and gets no lead-out, and a path note (information, decision 0536) names the passes.
5. **A at the start (L5 a).** The link arrives at the A where the arc starts, and A turns the Rotation way through the arc and the ring, never reversing (OP-15). The extra A counts against the A range (0241); where it does not fit, generation is refused naming the A range (Setup step, Machine).

**Why it is safe.** On a full ring, the stretch before the first point and after the last is the ring's own path, so the lead lies over cut the ring makes anyway, raised along the tool axis: the dominance argument of decision 0543 section 3. On a spiral and a partial ring it lies just off the cut at the same X; the gouge drop on every arc position, the keep-out check and the collision oracle stay, and a lead that would cut the finished surface or reach a held region is refused in the form of decision 0536, naming the Lead radius and the entry styles that remain.

## Owner decisions needed (the coordinator puts them; one answer each)

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

- **Code.** `openchuck-path` `assemble/entry.rs` (`lead_in`, `exit`, using the pass shape and the `A0 - Δ` link of decision 0543), `assemble.rs` (the link and `fit_range` with the arc's A). No API change beyond decision 0543's.
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

## Open items

None new.
