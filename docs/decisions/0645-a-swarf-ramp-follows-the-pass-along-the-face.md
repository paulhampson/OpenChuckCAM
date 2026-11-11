# 0645: A swarf ramp follows the pass along the face, never round the part (OP-07, OP-11, OP-12, NFR-07, NFR-12; issue #778; lane RAMPENTRY)

- **Status:** accepted by the project owner on 11 Nov 2026, relayed by the coordinator (decision 0049). Proposed by lane RAMPENTRY on 10 Nov 2026, after the owner chose option (a) that day (relayed).
- **Date:** 10 Nov 2026.
- **Found by:** the owner (issue #778, sev2, on 0.55.0 `ea1f3f96`): with Entry Ramp, swarf on the blisk gouged the far-end tips of the other blades.
- **What it amends:** [0543](0543-a-ramp-entry-follows-the-toolpath.md) D2 (a spiral is ramped round the rotary axis at its start X), for strategies whose passes turn A without cutting all round the part: swarf.
- **Builds on:** 0543 section 3 (the dominance argument: a ramp position that is a pose of the pass raised along the tool axis cuts no deeper than the pass), D3 (back and forth over the pass's own path, A reversing during the entry only) and point 4 (the minimum stretch and the legs at least the tool radius); [0111](0111-swarf-through-the-application.md) and [0532](0532-swarf-keeps-off-the-faces-at-its-ends.md) (swarf on a STEP part, its face set and its neighbours); [0048](0048-pre-1-0-api-changes.md).
- **Open items:** OI-2141 (the owner's acceptance of this record), closed on 11 Nov 2026 with the acceptance.

## Context

A swarf pass lays the side of the tool along each ruling of a blade wall and moves along the blade. On a twisted blade, A turns as the tip moves along X, so the entry reads the pass's start as a spiral (`pass_shape::start`). 0543 D2 ramps a spiral round the rotary axis at its start X, before its first point: the right entry for a helical pass, which cuts all round the part there.

A swarf pass does not cut all round the part. On the public blisk (`testdata/step/valid/blisk.step`, placed as the `blisk-start` example; the blade wall, face 19; the 3 mm flat end mill), the ramp turned A round the axis at the pass's start, by the far end of the blades, and swept the tool through the other blades' tips: the independent bounded checker (decision 0255) measured a cut of 0.946 mm at X 60.6 on `main` `ccc9a878`.

The spiral ramp's own proof is a drop of the tool on the operation's surface (0543: "this stretch is not the pass's own path, so the gouge check on every position stays the proof"). For swarf that surface is only the face set (`BrepSurface::with_faces`, decision 0111), so the check never saw the other blades.

![The part unrolled, X across and A up. Before: the swarf pass runs along the blade, and the ramp turns A at the pass's start X through the other blades' tips. Decision 0645: the ramp goes back and forth over the pass's own path, A reversing, and descends along the tool axis to the pass's first point.](0645-a-swarf-ramp-follows-the-pass-along-the-face.svg)

## Decision (accepted on 11 Nov 2026, relayed; the owner chose (a) on 10 Nov 2026)

1. **A strategy says whether its ramps may go round the part.** `Strategy::ramps_round_the_part` (true by default) says whether a ramp may turn A round the part before a pass that starts as a ring or a spiral (0543 D2), on a stretch round the axis the pass does not cut. Swarf returns false.
2. **Then the ramp follows the pass.** For such a strategy, a pass that starts as a spiral or a ring is ramped back and forth over its own path up to its first rapid, as a partial ring is (0543 D3): A reverses during the entry only, and the reversals are counted in the existing note. Every ramp position is a pose of the pass raised along the tool axis, so by 0543 section 3 it cuts no deeper than the pass, against the whole part and not only the face set. The minimum stretch and the leg length of 0543 point 4 apply.
3. **Unchanged:** every other strategy, every other entry style, and swarf with any other entry. On the blisk, Entry Lead on swarf is refused by name ("the lead-in on an arc of radius 1.500 mm would cut … into the finished surface"), not changed here (the #572 family, which checks against the face set only).
4. **NFR-12.** Generated G-code remains the user's to verify before running it on a machine; this record does not say a swarf entry is safe beyond what the checks measure.
5. **Library (0048).** A provided trait method with a default: an addition, not a breaking change, so no minor bump. The changelog says what changes.

## Options not chosen

- **(b) Refuse Ramp entry on swarf** in the 0536 form, naming Entry. Smaller, but it removes the entry the owner expects to use.
- **(c) Give the entry checks the whole part on swarf** (a second surface in `StrategyInput`). It would also check leads (#572), but it is a larger change than a freeze exception allows, and the spiral round the part would still not be the entry the owner expects.

## Consequences

- **Code:** `crates/openchuck-path/src/strategy.rs` (the trait method, passed to assembly), `assemble.rs`, `assemble/entry.rs` (the ramp of a spiral or ring pass follows it when the strategy says so), `swarf.rs` (returns false).
- **Output:** swarf with Entry Ramp changes: the ramp goes along the blade instead of round the part. No golden program uses swarf with ramp entry.
- **Documents:** the `openchuck-path` changelog. The user guide's Entry section already says a ramp follows the pass; it gains nothing.

## Verification

On branch `claude/lane-rampentry-778-swarf-ramp`, from `main` `ccc9a878`. Each test was written first and seen failing on `main`:

- `crates/openchuck-app/tests/step_strategies.rs` (kernel feature): `req_op_11_req_op_07_a_swarf_ramp_on_the_blisk_cuts_no_deeper_than_its_pass`. Swarf with Entry Ramp on the public blisk's face 19, its neighbours given as the application gives them; the bounded checker against every face of the blisk at the poses and the quarter, half and three-quarter points of each machine move. On `main` the entry cut 0.946 mm at X 60.6; now 172 entry moves, deepest 0.0268 mm, the pass's own cutting moves 0.0265 mm (the blade's twist, which swarf already warns of).
- `crates/openchuck-path/tests/ramp_follows.rs`: `req_op_11_req_op_07_a_swarf_like_pass_ramps_along_its_own_path_never_round_the_part`. A synthetic pass that turns A while moving along X: with the default the ramp turns A round the part (to -143 degrees on `main`); with the method off, every entry pose lies on the pass's own X and A, the descent is within the ramp angle, and the A reversals are noted.

Not verified: no machine or air cut. The owner's own project for #778 is not in the repository; the owner re-runs it himself and reports numbers only.
