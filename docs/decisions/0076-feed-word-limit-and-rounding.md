# 0076: The feed word within the machine's limit where A counts in it, and small feeds written to three significant figures (OP-13, GC-04, GC-09, GC-10)

- **Status:** accepted by the project owner (17 Oct 2026), in the lane F session, on the terms relayed by the orchestrator session at the owner's instruction (decision [0049](0049-relayed-acceptance-of-records.md)): the GC-04 reading below, items 1 to 3 and 5 as built, and item 4 at three significant figures, not four. Lane F, built on `claude/lane-f-feed-limit-and-rounding`. It changes no requirement and does not edit the architecture; it adds a machine profile field and takes project schema 14 (provisional, renumbered at release) and machine profile schema 3. The owner's answers are under *Choices put to the owner*.
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): new OI-252 to OI-254. It fixes the two defects lane Z records in OI-236 (decision 0056, merged in pull request 79), which this change closes.

Numbering: the orchestrator assigned records 0076 to 0080 and OI-252 to OI-256 to this lane (17 Oct 2026). This record takes 0076 and OI-252 to OI-254; 0077 to 0080, OI-255 and OI-256 are unused. Project schema 12 is lane O's and 13 is held for lane S, so this record claims **14**, as the orchestrator directed (17 Oct 2026). On this branch the chain runs through two no-change steps, 11 to 12 and 12 to 13, held for those lanes; at release the lane renumbers its step to the next free schema on `main`, so the chain on `main` stays gapless (the provisional rule for all lanes). The number is the one constant `file::SCHEMA`, and the tests derive from it, so renumbering is one line and the step's name.

## GC-04 and the machine profile (the owner's answer)

GC-04 lists "feed mode for rotary moves" among the post options and says "these machine-specific conventions live in the post-processor definition, not in the engine". This record keeps the conversion there: the post's `feed_rule` still turns each move's time into the F word. What it adds to the machine profile is only how the machine's feed limit counts A (item 1), read as part of SET-01's "maximum feed rates", because path assembly must respect that limit before any post is chosen and the IR must stay the same for every post (GC-10, decision 0007 item 3).

**The owner accepted this reading (17 Oct 2026).** Nothing in the requirements changes, and OI-254 tracks whether a mismatch between the profile and the post should be reported. The other reading, keeping all of it in the post definition, would have needed alternative 2 below: path assembly given the post's feed rule, so the IR differs between the Z1 and LinuxCNC programs. That would have changed the architecture (one neutral IR for every post, section 5) and GC-10's premise that the verification post writes the same toolpath; it was not taken.

Requirements and architecture are not edited by this record.

## Context

Lane Z's tests of every strategy on STEP parts (decision 0056, proposed, on lane Z's branch) found two failures of the G-code gates (verification plan section 6). Neither is STEP-specific: the same shapes as meshes fail the same way on `main` (`feed_limits.rs` in `openchuck-app`).

**(a) Gate 1: the Makera Z1's F word over its linear limit.** Helical finishing of the corpus box wrote `F1416` to `F1428` (as a mesh here, `F1432.7`), rotary roughing and rest machining of the filleted box `F1360.8`, and simultaneous finishing of the revolved spline `F1200.4` to `F1205.9`, against the Z1's 1200 mm/min. The Z1 firmware times a move with X, Y or Z and A as `max(L, dA * P / 360) / F`, with `P = 2 pi r + 30` mm and `r` the end point's distance from the axis ([decision 0007](0007-sp-12-z1-conventions.md), measured by the SP-12 air cut). Path assembly (`openchuck-path::assemble`, OP-13) made no move shorter than each axis allows and kept the straight XYZ line within the linear limit (or Z's, decision 0050 item 6), but did not count A in the line. Where the ball rolls round a corner, A turns at its limit (3600 deg/min) while the tip hardly moves: the move takes `dA / 3600` minutes and the Z1's F is `3600 * P / 360 = 10 P`, above 1200 once `r` passes 14.3 mm. Lowering the programmed feed does not help, since these moves already run at the machine's limits.

**(b) Gate 2: a move's time more than 1 % off the IR's on LinuxCNC.** Lane Z found 1.04 % on simultaneous finishing of the revolved spline and 3.95 % on roughing and rest machining of the filleted box. A move that is almost all A has a small F on LinuxCNC (the XYZ length's speed: `F4.2`, `F7.8`), and the posts wrote F to one decimal (`decimals.f = 1`), so the rounding alone can be more than 1 % of the move's time (0.05 in 4.2 is 1.2 %). The post already works F out from the written positions ([decision 0034](0034-slice-12-baseline-finishing.md), as built), so the F word's own rounding is what is left. Decision 0050 met the same rounding at walls (`F0.4` to `F1.1`) and removed the micro-moves there; these moves are not micro-moves.

The gate 2 time tolerance (1 %, `Tolerances::time_relative`) is a verification-plan matter and is not changed.

## Decision

1. **The machine profile says how the controller counts A in its feed word.** `MaxFeeds` gains `rotary_in_feed` (`RotaryInFeed`): `separate` (the default and every profile so far: the F word is the XYZ line's speed, and A has only its own limit) or `makera` (the Z1: the F word is the speed of `max(L, dA * (2 pi max(r, 1) + 30) / 360)`). It is a fact about the machine's controller, beside the limits it qualifies (SET-01); the post's feed rule still turns each move's time into the F word (decision 0007 item 3, GC-04), so the IR stays the same for every post (GC-10). The built-in Z1 profile sets `makera`; its provenance cites the air cut.
2. **Path assembly times each fed move for it.** On a `makera` machine, a fed move with any XYZ motion counts for its line limit the larger of the line (with the written-position allowance it already had) and `(dA + 0.001) * (2 pi max(r + 0.001, 1) + 30) / 360` mm, where 0.001 degree and 0.001 mm allow for A and the two coordinates across the axis written to three decimals, and `r` is the end tip's distance from the axis. The line limit is chosen as before (linear if X or Y moves, Z's otherwise, the lower of the two below a written unit; decision 0050 item 6). A move with no XYZ motion is held to A's own limit, as before: the Z1 counts at most `dA` for it (its 360 mm cap), which A's limit already bounds. Rapids are unchanged (they have no F word). Where this lengthens a cutting move, the existing note says the machine's limits slow the move.
3. **Older files migrate.** Project schema 14 (provisional; see *Numbering*) and machine profile schema 3 add the optional field. A machine copy (or library file) whose `id` is the built-in Z1's gains `rotary_in_feed = "makera"`, so a Z1 project saved before this change is timed for the Z1's feed word when opened; any other profile keeps `separate`, which is how it was timed before. A Z1 copy saved before schema 6 has no `id` and keeps `separate`: choosing the Z1 again from the library sets it (OI-253).
4. **Feeds are written to at least three significant figures.** The template engine gains a `feed` filter (`feed_str` in Rust): fixed decimals as `fixed` writes them, at least `decimals.f` places, and more where the value would otherwise keep fewer than three significant figures (at most nine places). Both shipped posts write the F word with it. Half a unit of the third significant figure is at most 0.5 % of the value, so the written F's rounding alone stays within half of gate 2's 1 %. Feeds of 100 and more are written exactly as before (`F1200.0`, `F480.0`); `F4.2` becomes `F4.24`. The filter multiplies by powers of ten built by repeated multiplication, so the text is the same on every platform (NFR-06, decision 0019 item 2). A custom post that keeps `fixed` keeps its one-decimal F.
5. **Library family 0.9.0.** `MaxFeeds` gains a public field, a breaking change, so the workspace moves from 0.8.0 to 0.9.0 with changelog entries (decision 0048).

*Choices put to the owner, with the owner's answers (17 Oct 2026):*

- **Where the rule lives (item 1):** the GC-04 reading at the top of this record. *Accepted.*
- **Three significant figures, not four (item 4).** Four (0.05 %) would rewrite about 3,900 F words below 100 in four LinuxCNC golden files (`cone`, `groove`, `sphere`, `cone_held_tabs`). Three leave every existing golden file unchanged and keep the rounding within half the tolerance. *Accepted: three.*
- **The Z1's F with more decimals.** The Z1 programs only write F below 100 on slow moves (the Z1's rule has no near-zero feeds, decision 0007), but a Z1 program may now hold `F42.5` or `F7.25`. That the firmware reads F with two or three decimals is unconfirmed until gate 4 (OI-252). *OI-252 stays open until gate 4.*

## Measurements

Meshes of the corpus shapes (`feed_limits.rs` in `openchuck-app`): the box (20 by 30, 40 long, on the axis), the filleted box (the same section with its long edges rounded to 3 mm, 12 long) and the revolved spline (radius 8, 14, 10, 13 and 9 over 40 mm, a Catmull-Rom profile), each with a 3 mm ball in oak on the Makera Z1 profile at the oak feed (480 mm/min), posted with both shipped posts.

| Job | Before | After |
| --- | --- | --- |
| Box, helical finishing (10 degree step) | Gate 1 on the Z1: `F1432.7` against 1200 | Both gates clean, both posts |
| Filleted box, helical finishing (10 degree step) | Gate 1 on the Z1: `F1360.8` (lane Z: up to `F1273.9` on the STEP part) | Both gates clean, both posts |
| Filleted box, rotary roughing (0.5 mm to leave) then rest machining | Gate 1 on the Z1: `F1360.8`; on LinuxCNC F down to `F4.6` (rounding up to 1.09 %, within 1 % on this mesh by chance) | Both gates clean, both posts |
| Revolved spline, simultaneous finishing (10 degree step) | Clean (the mesh's widest radius is below 14.3 mm, so the defect needs the B-rep's own profile) | Clean |
| Near-A moves (`round_trip.rs`, X 0.05 mm and A 10 degrees in 0.72 s, LinuxCNC F 4.15) | Gate 2: `F4.2`, 1.2 % off | `F4.15`, within 0.5 % |

**Golden files: none changes.** Every reference job (`reference_jobs.rs`: groove, cone, sphere, lobed, cone axial, indexed, axial with lead, cone with held regions and tabs) and both post golden files (`post_linuxcnc_small_part.ngc`, `post_makera_z1_small_part.ngc`) match byte for byte on both posts. For item 2, the reference parts reach at most 12.5 mm from the axis, below the 14.3 mm where the Z1's counted A can pass 1200 at A's limit, and their G-code is unchanged. For item 4, every F word in the golden files is 10 or more (the LinuxCNC files have about 3,900 between 10 and 100, all keeping three significant figures at one decimal). The setup sheet and grouping golden files hold no F words. The project files the tests write now say `schema = 14`, and a Z1 copy's `machine.toml` gains the line `rotary_in_feed = "makera"`; no golden project folder is stored.

Output is identical at 1, 2 and N threads on both posts for the box and the spline jobs; the bound is a function of each move alone, so it adds no order dependence.

## Tests

- `openchuck-path` unit test `req_op_13_a_controller_that_counts_a_in_the_feed_word_keeps_it_within_the_limit`: 10 degrees of A with 0.01 mm of X, 18 mm from the axis: counted separately the Z1's F would be over 1400; counted as the Z1 counts, it is within 1200; an A-only move keeps A's own limit.
- `openchuck-post` `req_gc_04_req_op_13_a_feed_word_keeps_three_significant_figures`: the filter's text for feeds from 1e-12 to 1200.
- `openchuck-app/tests/round_trip.rs` `req_op_13_req_gc_10_a_small_feed_is_written_to_keep_its_moves_time_within_gate_2`: the near-A moves pass gate 2 on both posts, and the same template with `fixed` (one decimal) fails it.
- `openchuck-app/tests/feed_limits.rs`: the three jobs pass gates 1 and 2 on both posts and every F is within 1200 (`req_op_13_req_gc_09_req_gc_10_...`, fails before as in the table); identical G-code at 1, 2 and N threads on both posts (`req_gc_07_req_nfr_06_...`).
- `openchuck-project/tests/setup.rs` `req_prj_04_req_op_13_an_older_z1_copy_migrates_to_count_a_in_its_feed_word`: the schema 11 fixture (`tests/fixtures/schema-11-z1`) gains `makera`; the same copy with another `id` keeps `separate`.
- `openchuck-app/tests/step_strategies.rs` (needs the kernel, `--features kernel-tests`, run by CI's STEP kernel job): the cases lane Z left out for OI-236 are back, at the oak feed on the exact STEP parts: helical finishing of the box and the filleted box, simultaneous finishing of the revolved spline, and roughing then rest machining of the filleted box, each through gates 1 and 2 on both posts.

## Alternatives considered

1. **The post clamps the F word to the limit.** The move then takes longer than the IR says, which gate 2 finds and which hides a timing error; decision 0050 rejected the same for walls.
2. **`StrategyInput` carries the post's feed rule.** Path assembly would then time for whichever post is chosen. The IR would differ between the Z1 and LinuxCNC programs, so the verification post would no longer write the same toolpath (GC-10), the pipeline cache would key on the post, and decision 0007 item 3 (one IR, the post converts) would change. This is the change lane Z first proposed ("StrategyInput does not carry the controller's feed rule").
3. **Bound A's Makera travel on every machine.** No new field, but Makera's 30 mm would be in the engine for every machine, and a machine whose controller counts A separately would be slowed for nothing.
4. **Path timing allows for the F word's rounding (defect b).** The F word depends on the post's rule, which path assembly does not know (alternative 2), and slowing a move does not make its F larger on LinuxCNC (`F = L / t` falls as `t` grows).
5. **Loosen gate 2's time tolerance.** Not this lane's to change, and not needed: three significant figures keep the rounding within half of it.
