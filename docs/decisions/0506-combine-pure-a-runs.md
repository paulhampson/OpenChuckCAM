# 0506: Consecutive pure-A moves written as one block (GC-04, GC-07, GC-08, GC-10, GC-17, NFR-06, NFR-07, NFR-12; issue #503; lane A-MOVES)

- **Status:** accepted by the project owner on 3 Nov 2026, including item 6 (the GC-17 reading) and the regenerated first-cut goldens, relayed by the coordinator (decision 0049). Written as 0648; renumbered 0506 at the release of #517 by the Merge Manager (the next number after main's highest when it merges). Proposed by lane A-MOVES on 2 Nov 2026. Item 1 (build it now, before the air cut confirms the cause; on in both built-in posts; off for a post file without the key) is the owner's choice D1, relayed by the coordinator on 2 Nov 2026.
- **Date:** 2 Nov 2026.
- **Issue:** #503, severity proposal sev3 (Tier 2 bug).
- **Builds on:** [0007](0007-sp-12-z1-conventions.md) (the Z1's feed rule, A-only moves included), [0222](0222-short-moves-minimum-block-time.md) and [0245](0245-short-block-report-and-slowdown.md) (GC-17: Report and Slow down; Merge deferred), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (the end turn at a held region), [0390](0390-helical-finishing-in-rings.md), [0420](0420-rings-for-rest-roughing-and-round-facing.md) and [0497](0497-rings-for-the-simultaneous-spiral.md) (rings), [0241](0241-continuous-rotary-operation-boundaries.md) and [0247](0247-long-operation-rotary-accumulation.md) (counter resets at declared boundaries), the [first-cut tail investigation](../first-cut-tail-tab-investigation.md) (OI-993).
- **Open items:** OI-993 stays open (the owner's air cut, below). OI-2015 (a G93 block split by gate 2) is carried by pull request 519.

## Context

The owner sees jerky rotation on the Makera Z1 when a ring is cut (rotary roughing with Rings). The first-cut investigation (OI-993) had already found the likely cause in the published firmware 1.1.2 source: a move that turns A alone reaches the planner with no direction vector (`Robot.cpp:1724`), so the planner skips junction blending for it (`Planner.cpp:172`) and, with `minimum_planner_speed` 0, plans every such block to stop at its end. This is read from source; the installed binary has not been checked against it.

**Where the small steps come from.** From the toolpath, not the post. The continuous forms place a node every **Largest angle step** (`max_angle_step`, default 5 degrees) round each ring (decisions 0390, 0420, 0497), round the end turn at a held region (0161) and round the turn at a face across the axis (0297). The strategy needs the nodes: the depth and whether to cut are decided at each one. The post writes each IR segment as one block and never splits an A move, so a ring at constant depth becomes 72 blocks of 5 degrees at one machine point, all with the same F.

**Measured** on `main` `71eb139` (release CLI, the public Baluster example with its Rough operation switched to Rings; the golden first-cut program):

| Program | Moving blocks | Pure-A blocks | After combining | File size | Estimated time | Rest-to-rest A prediction |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| Baluster Rough in Rings, Z1 | 9,920 | 6,498 (5, 2.5 and 1.25 degrees) in 152 runs | 392 (3,805 blocks in all) | 130,115 to 70,357 bytes | 756.4 s, unchanged | 1,503 s to 581 s |
| Baluster Rough in Rings, LinuxCNC | 9,910 | 6,498 | 392 (3,804 in all) | 145,701 to 80,388 bytes | 756.4 s, unchanged | |
| Baluster Finish (helical), Z1 | 19,077 | 318 | 34 | 599,694 to 596,570 bytes | 841.0 s, unchanged | 70 s to 26 s |
| Golden first cut, Z1 and LinuxCNC | 18,113 and 18,091 | 2,240 in 75 runs | 76 | see the goldens | unchanged | 528 s to 204 s |

The estimated time is the IR's, under the controller's feed rule; it does not change because every combined run has one F. The last column is a prediction, not a measurement: the A-only time if every block starts and ends at rest with the published A acceleration of 360 degrees/s² (firmware config), against one rest-to-rest move per combined block. Every other golden program has no run of two or more pure-A blocks and is byte-identical.

## Decision

1. **The option (D1, the owner's choice, relayed).** A post option `combine_rotary_runs` (GC-04), on in both built-in posts, Makera Z1 and LinuxCNC, so the LinuxCNC simulator checks the same blocks the Z1 runs (GC-10). A post definition without the key reads it as off and writes every step as before. It is built now, before the air cut confirms the firmware cause, so the owner can post one program both ways and compare them on the machine.
2. **What is combined (exact, no tolerance).** In `openchuck-post` (`to_machine_program`, shared by every post), a fed move joins the block before it when, as written: its X, Y and Z words are identical to that block's (the tool stays at one machine point), A moves the same way round, the move is of the same kind, the block before it is also such a move with nothing written between them, and the written F word is the same per-minute value. A rapid, a move that changes X, Y or Z, a different F, a different kind, a reversal, a tool change, spindle or comment line and a rotary counter reset (`RotaryBoundary`, decision 0247) all end the run. Inverse-time (G93) feeds are never combined (no built-in post writes them; a combined G93 block would need a new F).
3. **Direction and A.** A is written absolute and unwrapped (the Z1's A is continuous; the LinuxCNC check machine's A is not wrapped), so a combined block's end A says how far and which way it turns: a whole turn from A0 ends at A360 (or A-360), two at A720. Counter resets happen only at declared boundaries, which end a run, so no reset falls inside a combined block.
4. **The same motion.** Each step of a run turns A about the axis with the tool at the same machine point, so the run sweeps the same circle whether it is one block or many, at the same rate. The IR is not changed: the simulation (which interpolates in machine coordinates), the gouge, keep-out and held-region checks and the estimated time see exactly what they saw before.
5. **Gate 2 (the independent check, GC-08).** `openchuck-gcode`'s comparison, written separately from the post, accepts an IR pose part way along a G-code block only where the block is a fed pure rotation at exactly the pose's written X, Y and Z and the pose's A lies strictly between where the block was last reached and its end, more than the A tolerance short of it. The pose is then compared and timed as the part of the block that reaches it (1% as before), and observers see that part. An inverse-time (G93) block is never split this way, because its F word is the whole block's time. A block that turns past the IR's last pose, stops short of it, goes the other way round, also moves a linear axis, or misses a pose in order, fails. Everything else in the comparison is unchanged.
6. **GC-17 (accepted by the owner with this record).** GC-17 keeps Merge deferred and says a merge, if added, "must use a stated tolerance and the independent model checker". This change reads as such a merge with a stated tolerance of zero: it joins only moves whose written words describe the same motion, and gate 2 checks every IR pose against the combined block. It is not the deferred Merge mode of decision 0222 (joining short moves within a geometric tolerance, OI-932), which stays deferred, and it is not a `short_block_mode`. GC-17's Report and Slow down count the blocks as written: a combined block is one block, and Slow down, if it lowers a combined block's feed, shares the new time among the block's IR segments in proportion to their own times.
7. **NFR-12.** Nothing in the wording says the combined program is safe or checked; generated G-code remains the user's to verify before running on a machine.

## Alternatives not chosen

- **Fewer nodes round a ring in the toolpath.** The strategy needs a node every step to decide depth and where to cut. A single 360-degree IR segment would also break consumers that draw or test the IR in the model frame (the viewport's lines, path checks).
- **An IR transform before the post** (the place decision 0222 named for a tolerance Merge): the same objection, for no gain, since this combine is exact.
- **In the Z1 template only.** A template sees one step at a time and cannot know what follows.

## Documents

- No change to the requirements or the architecture. Item 6 states how this change reads against GC-17's wording; the owner accepted that reading with this record.
- Verification plan: no row change proposed in this pull request; the GC-04 and GC-17 rows' tests gain the cases below.
- The first-cut goldens (`testdata/golden/first_cut/`) are regenerated: only their pure-A runs change.

## Consequences

- `PostOptions` and `ConvertOptions` gain a public field: a breaking change to `openchuck-post` (decision 0048): the family moves to 0.39.0, with changelog entries in `openchuck-post` and `openchuck-gcode`.
- A user's copy of a built-in post made before this change keeps writing every step until the key is added.
- A user template that writes something per fed block writes it once per combined block.
- A feed hold in the middle of a combined block holds part way round, as it would part way along any long move.

## Verification

- `openchuck-post` `tests/rotary_runs.rs`: `req_gc_04_combine_rotary_runs_writes_an_equal_feed_ring_as_one_block` (both posts: 72 blocks to one, the same estimated time, deterministic), `..._is_on_in_the_built_in_posts_and_off_when_a_post_omits_it`, `..._keeps_a_full_turn_going_the_same_way_round` (A-360, a reversed A direction, two turns to A720), `..._splits_where_anything_else_changes` (feed, Z, kind, reversal), `..._does_not_cross_a_rotary_counter_reset`, `req_gc_17_report_counts_the_blocks_as_written_after_combining`, `req_gc_17_slow_down_spreads_a_combined_blocks_time_over_its_segments`.
- `openchuck-gcode` `tests/rotary_runs.rs`: `req_gc_08_replay_accepts_an_exact_combined_rotation` (poses, travel, time and the observed parts), `req_gc_08_replay_refuses_a_combined_block_that_does_not_pass_every_pose` (beyond, short, the other way round, a linear axis moving, a pose off the point, poses out of order, the wrong rate), `req_gc_08_replay_never_splits_an_inverse_time_block` (found by CodeRabbit on pull request 517).
- The CLI's own gate 2 passes on every Baluster program above with the option on (`replay matches the toolpath`, tip 0.0007 mm, the cut time equal to the toolpath's).
- **The owner's air cut (OI-993).** Pending: whether the combined ring turns smoothly on the Z1. Steps are in the pull request.
