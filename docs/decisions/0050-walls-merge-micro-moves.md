# 0050: Walls in the target: path assembly merges the micro-moves path fitting leaves at a wall (OP-13, OP-14; closes OI-209)

- **Status:** accepted by the project owner (16 Oct 2026), relayed by the orchestrator session at the owner's instruction (decision 0049), as recommended: items 5 and 6 kept beside the merge, a wall move timed by its end contacts (OI-211), the tolerance-based thresholds with the 0.75 cascade ratio (OI-213) and the three-decimal assumption (OI-212) kept. Built on `claude/lane-y-wall-micro-moves` (lane Y). It builds the owner's decision on OI-209 (16 Oct 2026, recorded in [decision 0045](0045-slice-21c-wiring.md), accepted, on lane W's branch): fix in path assembly by merging the micro-moves at a wall into one move, not in the post and not by deferring. It adds nothing to the requirements or the architecture.
- **Date:** 16 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): new OI-211 to OI-213; OI-209 is closed by this record's acceptance (it is registered on lane W's branch, pull request 61; its register row is set to closed once that is on `main`).

Numbering: the orchestrator assigned record 0050 and OI-211 to OI-215 to lane Y on 16 Oct 2026. This record takes OI-211 to OI-213; OI-214 and OI-215 are unused. Lane Y takes no project schema number (it changes no file format).

## Context

A wall is a step in the target that the tool climbs or drops: a tab narrower than the part's end face (the owner's default tab, whose small end sits on the part), a held region wider than the part, or a step in the part itself. At a wall the tool's drop height jumps: a ball of radius R rides at one height until its axis is R from the wall's plane and at the other past it. The wrap-family path fitter ([decision 0018](0018-slice-5-pathing-core.md) item 12, `wrap_fit`) bisects a move until the straight machine move is within the chord limit of the dropped tool; across a jump that never holds, so it halves down to its floor (the seed step over 2^10, 0.005 degrees at the default 5 degree step). A pass reaches the wall through a cascade of halving moves, 2.5, 1.25, 0.6 degrees and on down to 0.005, and crosses it in one move almost wholly along the tool axis.

Lane W's first trial of the owner's default layout on the cone failed gates 1 and 2 (verification plan section 6) on that cascade, and this lane reproduced the failure on `main` with a mesh that has the same walls:

- **Gate 2.** The shared services smooth the contact path over five points ([decision 0015](0015-sp-4-ray-cast-sweep.md), `CONTACT_SMOOTHING`), so the contact's jump across the wall, a few millimetres, is shared with the micro-moves beside it. Each then moves the tip a few thousandths of a millimetre for a fifth of the jump in contact, the post writes `F0.4` to `F1.1`, and the written feed's rounding to one decimal is 2.6 to 7 % of the move's time.
- **Gate 1.** The wall move changes X by a fraction of a micron. The assembler then took the linear limit for its straight line and Z's limit only for Z's own travel, with no allowance for rounding, while the post writes X unchanged and the checker holds the F word to Z's limit: `F600.1` against 600. Before the fix the same job also wrote `F688.8` on a move beside the wall.

## Decision

1. **Where.** In the shared services (`openchuck-path::assemble`, a private `walls` module), after A is placed and before the contact is smoothed, for every pass of every strategy. The strategies, the path fitter and the post are unchanged. No public item changes (no version bump; the changelog records the behaviour).
2. **What a wall move and a micro-move are.** The tool tip's motion over the part in a step, split into the part across the tool axis and the part along it. A step that moves the tip across the part by less than the toolpath tolerance and along the tool axis by more is a **wall move**; one that moves it by less than the tolerance both ways is a **micro-move**. The tolerance is the operation's toolpath tolerance (0.01 mm by default, NFR-05).
3. **What is merged.** Walking away from each wall move on both sides, the steps of the bisection cascade: every micro-move, and every step at most 0.75 times the step beyond it (bisection halves), up to the first step that is neither. The merged steps together are never longer than the step path fitting started from, so the largest angle step still holds. A pass with no wall move is unchanged, which keeps every existing golden file byte for byte.
4. **How they are merged.** The points of each run are thinned to the fewest that keep every dropped point within the path-fitting chord limit (0.4 times the tolerance, decision 0018 item 12, verification plan 4.1) of the straight machine move that replaces it; the distance is the tool tip's, in the model frame, at the nearest parameter of the move in the machine's axes. On a flat or gently curved surface that is one move to the foot or from the top of the wall. The wall move itself is kept, so the path stays at the wall's foot until it climbs (or on top until it drops): a merge never cuts the corner.
5. **The contact smoothing stops at a wall move.** The points before a wall move and those after it are smoothed separately, so the wall move carries the whole jump and the moves beside it keep their own contact. Without this, a short move the chord keeps beside a wall (on a curved surface) still crawls.
6. **A move that changes X or Y, but by less than a written unit (0.001 mm), is timed against the lower of the linear and Z limits**, with the rounding allowance the line already has ([decision 0034](0034-slice-12-baseline-finishing.md), as built): such a move may be written with no X or Y, and the checker then holds its F word to Z's limit, or with a unit's change, and then to the linear limit. A move with no X or Y keeps Z's limit, as before.

*Choices put to the owner, each kept as recommended on acceptance:*

- **Items 5 and 6 go beyond the merge the owner chose.** On the two test parts the merge alone happens to pass both gates; item 5 is needed where the chord keeps a short move beside a wall, and item 6 wherever a wall move's time is set by the axes, as unit tests show (each fails without its item). Dropping either leaves that failure for a later wall.
- **The thresholds** (item 2 and the 0.75 ratio of item 3) use the toolpath tolerance, not a fixed length. A steep surface sampled more finely than the tolerance counts as a wall; the merge then only drops points within the chord, so accuracy is kept, and the smoothing stops there (OI-213).
- **A wall move is timed by its end contacts**, as every move is: the contact jumps from the foot of the wall to its corner (or from the corner to the surface below), which is longer than the tip's travel, so the tool climbs or drops at 0.17 to 0.55 of the programmed feed on the test parts. It is slower than needed, never faster. Timing it by the tip's travel instead is OI-211.

## Measurements

The cone of the reference jobs (diameter 20 to 10 over 30 mm), 3 mm ball, stepover 2 mm, 5 degree step, tolerance 0.01 mm, Makera Z1 profile.

| Case | Before | After |
| --- | --- | --- |
| Owner's default layout at the head (lane W's branch, held region, frustum tab narrowing to 10 mm on the 20 mm end) | Gate 1 `F600.1`; gate 2: a move 7.00 % off the IR's time | – (re-run on the default tabs once pull request 61 is on `main`) |
| The same layout as a mesh (`walls.rs` in `openchuck-app`, part `head`), Z1 post | Gate 1 `F688.8` and `F600.1`; gate 2 7.02 % | Both gates clean, both posts |
| A narrow tab and a wide held region at the tail, as a mesh (part `tail`) | Gate 2 2.6 % | Both gates clean, both posts |
| The default-like layout on `CompositeSurface` (`walls.rs` in `openchuck-path`), three walls | 16 micro-moves | None; one move up or down each wall; slowest move 0.17 of the feed (a wall move, OI-211) |

The independent checker (verification plan 4.1: no overcut beyond the tolerance) samples the exact surface of revolution and each wall's face at four poses along every cutting move within 2 mm of a wall: no gouge beyond 0.01 mm. Paths and G-code are identical at 1, 2 and N threads. All existing golden files are unchanged.

## Tests

- `openchuck-path` unit tests: the cascade before a climb merges into one move (`req_op_13_micro_moves_before_a_climb_merge_into_one_move`); a pass without a wall is unchanged; a point the chord cannot drop is kept (`req_op_12_...`); smoothing stops at a wall move; a move that hardly moves X is timed against Z, and against the linear limit where Z is faster (fails without item 6: `F2000.436` against 2000).
- `openchuck-path/tests/walls.rs`: helical finishing on the composite with walls has no micro-move and one move per wall, and no move crawls (`req_op_13_req_op_14_...`, fails before: 16 micro-moves); the independent checker finds no gouge at the walls (`req_op_12_...`); identical at 1, 2 and N threads (`req_nfr_06_req_gc_07_...`).
- `openchuck-app/tests/walls.rs`: the `head` and `tail` parts pass gates 1 and 2 with both posts (`req_op_13_req_op_14_req_gc_09_...`, fails before as in the table); identical G-code at 1, 2 and N threads (`req_gc_07_req_nfr_06_...`).

The gouge and determinism tests pass before and after: they guard that the merge keeps those properties.

## Alternatives considered

- **Stop bisecting below a useful step** (in the fitter): the owner chose assembly. It would leave the strategies' points as they are now for every other strategy, and each fitter would need it (OI-213 notes the saving it would bring).
- **Clamp the written feed to the limit in the post**: the owner rejected it; it hides a timing error rather than removing it, and gate 2 would still fail on the crawling moves.
- **Merge every run of micro-moves, not only at walls**: would change the sphere's golden file (moves near its pole are short) and could merge steps a strategy wanted for its largest angle step.
- **Remove the contact smoothing**: decision 0015 needs it on meshes; stopping it at a wall move is enough.

## Consequences

- `openchuck-path`: a private `assemble::walls` module; `assemble` calls it. No public item changes; the changelog says so.
- `docs/work-breakdown.md`: the lane Y row, the numbers paragraph and the slice 21 status. `docs/open-items.md`: OI-211 to OI-213. `docs/verification-plan.md`: the OP-13 and OP-14 rows name the wall tests. Decision index.
- Once pull request 61 is on `main`: the default tabs are proved on the composite through the application (`Application::start_default_tabs`), OI-209's row is closed in the register, and the slice 21 status drops "default tabs depend on OI-209".

## Open items

- **OI-211:** a wall move is timed by its end contacts, so the tool climbs or drops at 0.17 to 0.55 of the programmed feed. Decide whether to time it by the tip's travel (the cutter rubs the wall's face while it climbs, and is in air while it drops).
- **OI-212:** the written unit (0.001 mm) and the rounding allowance assume three decimals, as decision 0034 did. A post that writes fewer decimals needs both from the post's decimal setting, which the path library does not see.
- **OI-213:** path fitting still bisects to its floor at a wall and assembly drops the points; the fitter could stop where the drop jumps and save the drops. A steep surface sampled more finely than the tolerance counts as a wall (item 2): its smoothing stops there and its points are thinned within the chord. Deferred: no failure is known.
