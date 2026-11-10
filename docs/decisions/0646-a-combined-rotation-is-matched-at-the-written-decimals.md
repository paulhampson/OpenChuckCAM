# 0646: A combined rotation is matched at the written decimals in every replay (GC-08, OP-24, NFR-06, NFR-07, NFR-12; issue #641; lane ENDFACE-POST)

- **Status:** proposed by lane ENDFACE-POST on 10 Nov 2026, for the project owner to accept. The fix is an owner-approved exception to the release freeze (a failure in the owner's release testing of 0.55.0-rc1, relayed by the coordinator, decision 0049). The number is provisional: the Merge Manager assigns the final number at release.
- **Date:** 10 Nov 2026.
- **Found by:** the owner, on the 0.55.0-rc1 build (`ea1f3f96`), at step 5 (Post and export) of the check for issue #641 (branch `owner-checks/2026.1-alpha`, `owner-checks/641-end-facing-from-air/CHECK.md`): "cannot post the G-code: Facing written-motion verification: line 2842: rotary counter reset is not at a reached declared boundary or does not match the controller". Investigated by lane ENDFACE-POST.
- **What it amends:** none. It carries out [0506](0506-combine-pure-a-runs.md) ("every pose lies on it exactly: the same written X, Y and Z") in the binary32 replay of [0547](0547-end-facing-enters-from-air-and-checks-its-engagement.md)'s Facing written-motion check, which applied the rule to the float values instead of the written words.
- **Builds on:** [0506](0506-combine-pure-a-runs.md) (one block for consecutive pure-A poses), [0241](0241-continuous-rotary-operation-boundaries.md) and [0247](0247-long-operation-rotary-accumulation.md) (counter resets at declared boundaries), [0547](0547-end-facing-enters-from-air-and-checks-its-engagement.md) (the Facing check of the written words, decimal and binary32).
- **Open items:** OI-2142 (the owner's acceptance of this record), new.

## Context

The check project is the tutorial baluster with two End facing operations before Rough and Finish. Posted as one file (grouping by setup, the default; also by project or by tool), it was refused by both built-in posts: the Makera Z1 with the message above, LinuxCNC with "the G-code ends before the IR does". Each operation in a file of its own posted.

Line 2842 is not in a Facing operation. It is a correct `G92.4 A0 S0` inside Rough, at a boundary Rough declares (0241, 0247). The Facing check (0547) compares the whole file with the IR, not only the Facing operations, and it does so twice: once with the decimal words, once with each X, Y, Z, A and F word as a binary32 controller holds it. Only a file that contains a Facing operation gets this check, which is why Rough on its own posted.

**The cause.** Rough starts with a ring: 72 IR poses that turn A alone from 0 to 360 degrees at X 9.088. The post writes them as one block, `A360.000 F991.2` (0506). The comparison treats a pose as reached part way along such a block when the block's X, Y and Z equal the pose's written X, Y and Z. It compared them exactly. In the binary32 replay the block's X is 9.0880003, not 9.088, so no pose was on its way. Each pose then took a block of its own, the comparison fell 143 poses behind the program, and the next counter reset (line 2842) came before the boundary it belongs to. The decimal replay matched correctly, and the program was correct; the refusal was a false positive.

## Decision

1. **A block's X, Y and Z are read at the written decimals when the comparison decides whether a pose lies on its way** (`openchuck-gcode`, `compare_with_moves`). In the decimal replay this is the same value, so nothing changes there. In the binary32 replay, 9.0880003 reads as 9.088, which is the word the post wrote.
2. **Nothing else is relaxed.** The block must still turn A alone (its X, Y and Z unchanged), be fed in G94, and reach each pose's A strictly on its way and in order. Every reached pose is still compared with the IR within the binary32 replay's A tolerance. The Facing check still requires exact word identity for Facing operations. A block at another written X is still not on the pose's way, so the file is refused as before.
3. **Output unchanged.** Only the check changes; every post writes the same bytes.

## Options not chosen

- **Run the Facing check on the Facing operations only.** The check replays the whole file because the controller's counter and modal state carry from one operation to the next; checking a Facing operation out of its file would not check the program that is run.
- **Compare with a position tolerance.** The Facing check sets the position tolerance to infinity and checks exact word identity itself; a distance would be a looser rule than the written word.

## Consequences

- The #641 check project posts with both built-in posts in every grouping. Face end's words in the per-setup LinuxCNC file are line for line those of its own per-operation file (apart from that file's `M5`, `M30` footer), including the band of the check's step 3 (`G0 X66.588 Y-14.588`, `G1 Z11.001 F300.0`).
- Any file that holds a Facing operation and a combined rotation at an X, Y or Z that binary32 cannot hold exactly (most three-decimal values) posts again. Before, it was refused at posting.

## Verification

### As built (lane ENDFACE-POST, 10 Nov 2026)

From `main` at `ccc9a878`. Tests written before the fix and watched to fail, run uninstrumented with `cargo nextest run`:

- `crates/openchuck-gcode/tests/rotary_runs.rs`: `req_gc_08_float_replay_accepts_a_combined_rotation_at_a_written_x_with_no_exact_float`. Eight 5-degree poses at X 9.088 as one `G1 A40` block, replayed with decimal words and with binary32 words: both match the eight poses to the one block. A block at X 9.089 is still refused. Before the fix the binary32 replay failed with "the G-code ends before the IR does".
- `crates/openchuck-app/src/facing.rs`: `facing::tests::req_op_24_req_gc_08_facing_file_with_a_combined_rotation_posts_through_both_posts`. A Facing group and a 72-pose ring at X 9.088 in one file, through the LinuxCNC and Makera Z1 posts: the ring is one block and the Facing written-motion check passes. Before the fix it failed with "the G-code ends before the IR does: group 1 needs a move to tip (9.088, 7.765, 28.978) A 15.000".
- **The check project.** `openchuck-cli regenerate project --post <post> --tool-numbers keep --grouping <grouping> --out OUT` on the project in `owner-checks/641-end-facing-from-air/project` (branch `owner-checks/2026.1-alpha`), release build, Linux x86_64. On `ccc9a878`, `makera-z1` with grouping setup, project and tool failed with the owner's message (line 2842), and `linuxcnc` with the same groupings failed with "the G-code ends before the IR does"; operation grouping posted. With the fix all eight combinations post, in about 22 s each.

- **Suites.** `cargo nextest run -p openchuck-gcode -p openchuck-app` with the change: 907 tests, all passed (1 skipped), 1,250 s on 4 cores; the two new tests take under 0.1 s each, so neither is long-running. `cargo fmt --all`, `cargo clippy --workspace --all-targets` (no warnings), and `cargo xtask deps`, `licences`, `records`, `trace` and `messages` all pass.
- **Coverage.** `cargo llvm-cov nextest --profile coverage -p openchuck-gcode`: `compare.rs` 94.92 % of lines on the crate's own tests; every changed line runs. The `openchuck-app` change is test code only.
- **API.** No public signature changes; only the comparison's behaviour in a binary32 replay. No version bump is needed (decision 0048).

Not done: nothing was run on a machine. Generated G-code must be verified by the user before it is run on a machine (NFR-12).
