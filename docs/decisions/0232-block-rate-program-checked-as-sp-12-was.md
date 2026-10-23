# 0232: The Z1 block-rate air-cut program is checked as the SP-12 program was (verification plan policy 6; lane GATES)

- **Status:** proposed (23 Oct 2026). The owner chose option A on OI-925, relayed by the coordinator on 23 Oct 2026 (decision 0049), and asked for a record of the exception. Only the owner accepts it, directly in the lane GATES session.
- **What it amends:** [verification plan](../verification-plan.md) section 1, policy 6 ("Nothing reaches a real machine unverified"), whose one exception is the SP-12 conventions program. The edit is proposed below and is made only once this record is accepted (CLAUDE.md). No requirement and no architecture change.
- **Date:** 23 Oct 2026.
- **Open items:** OI-925 closes when this record is accepted. OI-930 (the Z1's measured minimum block time) is lane SHORTMOVE's and stays open until the air cut has run.

Numbering: 0231 is the highest record on `main` and every remote branch on 23 Oct 2026, so this takes 0232.

## Context

The owner added lane SHORTMOVE's block-rate air cut ([decision 0222](https://github.com/paulhampson/OpenChuckCAM/pull/315) item 7, proposed) to gate 4 ([instructions](../z1-gates-4-5-test-instructions.md) section 4.8). Its program, `spikes/sp14-short-moves/air-cut/block_rate_z1.nc` on `spike/sp-14-short-moves` (69d532f), is written by a Python script, not by the application's post, so gates 1 to 3 have not checked it. Policy 6 lets no G-code reach the Z1 before it has passed the simulator gate, with one exception: the SP-12 conventions program, "hand-written, checked by hand and in the LinuxCNC simulator first, and then run only as an attended air cut with no tool, no material and reduced feed". The program is 1,170,259 bytes, over the 1 MiB a `testdata/` file may hold (decision 0004), so it is not committed.

Requirements: GC-09 (the Z1 post), GC-10 (the verification post), NFR-12 (the user verifies the program). Records read: 0007 (SP-12 and the Z1 conventions), 0004 (test-data size), 0222 (the air cut).

## Decision

1. **The same exception as SP-12, for this program only** (OI-925, the owner's option A). The block-rate program may run on the Z1, as an attended air cut under the gate 4 rules, once it has been:
   - **read by hand**: the owner reads its header, its footer and one section of each part, against the checklist of instructions section 4.8 (steps K1 to K4);
   - **run through gate 3**: a LinuxCNC copy, the program with its Z1 footer (`G92.4 A0 S0`, `G53 G0 Z-3`, `G28`, `M2`) replaced by the LinuxCNC post's (`M5`, `M30`), is replayed by `cargo xtask replay` with its path, and passes as any golden program does (every move reached, no fault, the end where the replay says).
2. **How the program is fetched.** From a checkout: `git fetch origin spike/sp-14-short-moves`, then `git show 69d532f:spikes/sp14-short-moves/air-cut/block_rate_z1.nc > block_rate_z1.nc`. The commit is named, so the file run is the file checked. A change to the program sends it back to step 1.
3. **No other program gains the exception.** A later test program that is not written by the post needs its own record.

## Alternatives considered

- **A committed generator with gate 1 in a test** (OI-925 option B): repeatable, but more work in another lane before a two-minute test.
- **Run it under the gate 4 rules only, as an accepted risk** (option C): leaves policy 6 broken without a check.

## Consequences

The document edit below is proposed with this record and made only once the owner accepts it:

- `docs/verification-plan.md` section 1, policy 6: after "The one exception is the SP-12 conventions program: …", add "The Z1 block-rate air-cut program of decision 0222 is treated the same way ([decision 0232](decisions/0232-block-rate-program-checked-as-sp-12-was.md)): read by hand, its LinuxCNC copy replayed by gate 3, then run only as an attended air cut under the gate 4 rules."
- `docs/open-items.md`: OI-925 closed.

NFR-12 still applies: the user verifies every program before it runs on a machine.
