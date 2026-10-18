# 0109: A Makera Z1 copy stored without its library ID is recognised by its name when it migrates (SET-01, PRJ-04, OP-13; closes OI-253)

- **Status:** accepted by the project owner (18 Oct 2026, Decision 41, option A), relayed by the orchestrator session at the owner's instruction (decision [0049](0049-relayed-acceptance-of-records.md)). Lane BT, built on `claude/lane-bt-gate2-timing`. It changes no requirement and does not edit the architecture; it amends how [decision 0076](0076-feed-word-limit-and-rounding.md) item 3 migrates older machine copies. No public item changes, so the workspace version is unchanged (decision 0048).
- **Date:** 18 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-253 is closed by this record; none are new.

Numbering: 0107 is lane FZ's, 0108 lane BM's and 0110 is on `main`; 0109 was held for lane BT (decision 0110's numbering note). Lane BT takes no OI number and no project schema number (the file format is unchanged).

## Context

Lane BT was asked to find why helical and axial finishing of the owner's blisk (`testdata/stl/blisk.stl`) failed gate 2 in lane TE's end-to-end run (OI-316, on lane TE's branch). The gate 2 failures came from lane TE's branch base (3cbd4dd), which predates decision 0076: the posts wrote F to one decimal (`F0.5` for `F0.549`, 9.86 % of the move's time). On `main` the same jobs pass gates 1 and 2 on both posts. Lane TE closes OI-316 on its branch with that finding.

Lane TE then reported that its blisk project, made on the old base and regenerated on `main`, still failed gate 1 on the Z1 post: 144 F words above 1200, up to `F1556.5`. The cause is the project's machine copy:

- The project keeps its own copy of the machine profile in `machine.toml` (SET-01). Decision 0076 item 3 migrates an older copy (schema 12 to 13) by giving it `rotary_in_feed = "makera"` only when its `id` is the built-in Z1's. Without the field, `RotaryInFeed` reads as its default, `separate`.
- `Command::SetMachine` stores the profile with no library `id` (`set_machine_from(profile, None)`); only `SetMachineFrom`, which the command-line `machine` command and the interface use, keeps it. Lane TE's project was written through the API with `SetMachine`, so its Z1 copy had no `id`, was not migrated, and path assembly timed A separately: the Z1's F word went over its limit again (the defect decision 0076 fixed).
- Reproduced on `main` (20e5ba6): the same project with the `id` passes (0 findings, line 381 `F1199.7`, cut 490.0 s against the toolpath's 490.0 s); with only the `id` line deleted it gives lane TE's figures exactly (144 findings, line 381 `F1479.0`; LinuxCNC cut 488.7 s against 488.8 s).

OI-253 already recorded this gap for copies saved before schema 6. The population is wider: any Z1 copy saved before schema 13 through `SetMachine` (API or test code) has no `id` either. Gate 1 reports the over-limit F on every post, so such a program is not written silently, but the project cannot be posted cleanly until the Z1 is chosen again.

## Decision

1. **The migration also recognises the Z1 by its name.** `fill_rotary_in_feed` (`openchuck-project`, `machines.rs`) treats a machine copy that has **no** `id` and whose `[machine] name` is the built-in Z1's ("Makera Z1") as the Z1, and gives it `rotary_in_feed = "makera"`. A copy with an `id` is judged by its `id` alone, as before; a copy with no `id` and any other name keeps `separate`. The same function migrates machine profile files in a library (machine profile schema 3), so the rule is the same there.
2. **Why the name is safe.** The error can only go one way: if a user kept the name "Makera Z1" on their own profile for a controller that counts A separately, path assembly now counts A as the Z1 does, which only lengthens moves (a lower F, a slower move than needed). It never makes a move faster than the machine's limit. The opposite error, the one this record removes, writes F words above the limit.
3. **No API change.** The name is a private constant beside `MAKERA_Z1_ID`; the built-in profile's unit test checks they agree.

*The choice put to the owner (OI-253: accept as is, or offer the update when such a project opens), with the alternatives:*

- **A (chosen): widen the migration to the name.** About ten lines, no file format or interface change; existing golden files are unchanged (they are generated from current profiles).
- **B: warn when such a project opens** ("this machine copy predates the Z1's feed rule; choose the Z1 again") and leave the file alone. Honest about the change, but it needs interface wording (UI specification) and a command-line report, and the project still fails gate 1 until the user acts.
- **C: take the limits from the built-in profile when the stored copy looks unmodified.** Rejected: the project's copy is the source of truth (SET-01; decision 0013: a user who changes values makes their own profile), comparing whole profiles breaks when the built-in's provenance text changes, and a project's output would change silently when the built-in changes.

## Consequences

- `openchuck-project`: `fill_rotary_in_feed` and a private `MAKERA_Z1_NAME`.
- A Z1 copy with no `id` that the user renamed is still timed with A separately; it cannot be told from the user's own profile, and choosing the Z1 again sets it (as decision 0076 item 3 already says). This is accepted with option A, so OI-253 closes.
- `docs/open-items.md`: OI-253 closed. `docs/verification-plan.md`: the PRJ-04 and OP-13 rows name the tests.

## Tests

Each fails on `main` before the change and passes after it:

- `openchuck-project/tests/setup.rs` `req_prj_04_req_set_01_a_z1_copy_without_an_id_migrates_by_its_name`: the schema 11 Z1 fixture with its `id` line removed loads with `RotaryInFeed::Makera` (before: `Separate`); the same copy renamed "My mill" keeps `Separate`.
- `openchuck-app/tests/feed_limits.rs` `req_op_13_req_gc_09_req_prj_04_an_older_z1_copy_without_an_id_keeps_the_feed_word_within_the_limit`: the box job of decision 0076, its project saved with the Z1 copy `SetMachine` writes (no `id`), turned back to schema 12 without `rotary_in_feed`, reopened and posted on the Z1: gates 1 and 2 pass and no F word is above 1200 (before: `F1256.1`, `F1432`, `F1432.7` at lines 35 to 37).
- The existing `req_prj_04_req_op_13_an_older_z1_copy_migrates_to_count_a_in_its_feed_word` (copy with the `id`; another `id` keeps `Separate`) is unchanged and passes.
