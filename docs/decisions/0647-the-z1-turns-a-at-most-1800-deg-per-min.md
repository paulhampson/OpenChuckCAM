# 0647: The Makera Z1 turns A at most 1800 deg/min (SET-01, GC-04, GC-06, GC-09, GC-12, SIM-08, UI-13, NFR-12; issue #793; lane ALIMIT)

- **Status:** Accepted by the project owner on 14 Nov 2026, relayed by the coordinator (decision 0049). Proposed by lane ALIMIT on 14 Nov 2026; the fix itself is the owner's decision of the same day, relayed: fix before the first release, with a plain limit in the profile. The number is provisional: the Merge Manager assigns the final number at release.
- **Date:** 14 Nov 2026.
- **Found by:** the owner's gate 4 run of 0.55.0-rc2 on firmware 1.1.3: every program that turns A while cutting took longer than its estimate (the sphere 127 s against 81 s). Lane FWDIFF compared the firmware with the profile and found the cap; the owner confirmed it on the machine: `M203.1` in the console prints `A: 30`.
- **What it amends:**
  - [Decision 0013](0013-z1-profile-consensus.md) items 1, 3 and 7 (accepted), for A only: the Z1's A limit is 1800 deg/min for feeds and rapids, not 3600. X, Y and Z are unchanged. The value stays *confirmed*.
  - [Decision 0007](0007-sp-12-z1-conventions.md)'s limits row and the A limit that [decision 0076](0076-feed-word-limit-and-rounding.md) applies: both now read 1800 for the built-in Z1. The feed rule and the code that applies it are unchanged.
- **Builds on:** [0076](0076-feed-word-limit-and-rounding.md) (the F word counts A's travel; the profile's A limit sets a move's time), [0317](0317-z1-measured-axis-height-and-safe-z.md) item 3 (a project's old Z1 copy is named, never changed), [0109](0109-z1-copy-without-an-id-migrates-by-name.md) (a copy with no ID is the Z1 by its name).
- **Open items:** OI-2143 (the owner's acceptance of this record, given on 14 Nov 2026, and the re-verification runs below), new; OI-2145 (the user guide's scallop example states the tutorial's programmed times, which change, and its figures need a re-capture), new. OI-267 was closed on `main` by the owner's air cut of 14 Nov 2026, which also bears this record out (below).

Requirements and architecture are not edited by this record. No project or machine schema changes, and no public API of the library family changes (`openchuck-path` changes behaviour only, item 2).

## Context

The built-in Z1 profile held A to 3600 deg/min (10 RPM): Makera's website and `delta_max_rate` in the firmware's default configuration (decision 0013). The firmware does not run A at that rate on the Z1 with its 4th-axis module. When it registers the motors (`src/modules/robot/Robot.cpp:295-298`, in firmware 1.1.2 and 1.1.3), it caps the configured rate of the A actuator at 1800 when bit 0 of the factory setting `FuncSetting` is set:

```cpp
if((THEKERNEL->factory_set->FuncSetting & (1<<0)) && (a == 3))
{
    uint16_t s = THEKERNEL->config->value(motor_checksums[a][4])->by_default(3000.0F)->as_number();
    if(s > 1800) s=1800;
    actuators[a]->set_max_rate( s/60.0F); // it is in mm/min and converted to mm/sec
```

`M203.1` prints the rates the controller holds, per second. On the owner's Z1 (firmware 1.1.3) it prints `A: 30`: 30 deg/s, 1800 deg/min. The owner's air cut of the same day (OI-267, relayed) agrees: `G1 A360 F4000` took about 12 s, a full turn at 30 deg/s.

What the profile's A limit does in the software:

- **Each move's time.** Path assembly times a move for the slowest of its axes, and on the Z1 for A's travel counted in the F word (`openchuck-path::assemble`, decision 0076). A move whose contact feed would turn A faster than the limit is slowed to it. With 3600 the software asked A for up to twice what it can do.
- **The F word.** The post writes F from each move's time (decision 0007 item 3, GC-04). With 3600, a move that A's limit should set got an F that asks the firmware for 3600 deg/min. The firmware then turns A at 1800 and the move takes up to twice its planned time. Nothing moves to a different place: the firmware only slows down.
- **The estimates.** The setup sheet (GC-12), the simulation's time (SIM-08) and the playback all add the planned move times (GC-06), so they were short. The owner measured 127 s for the sphere against 81 s planned. A model with a 30 deg/s cap fits the measured times within −2 % to +0.5 %.
- **The short-block report (GC-17).** It reads the planned move times, so it treated the A-limited moves as shorter than they are.

![The Z1's F word on a move whose time A's limit sets, against the distance r of its end point from the axis. With the old profile, 3600 deg/min, the F word asked A for twice its rate, and A's limit set the time below r 14.3 mm. With the controller's 1800 deg/min, F is half that, and A's limit sets the time wherever r is below 33.4 mm, the point where F reaches the 1200 mm/min X and Y limit.](0647-the-z1-turns-a-at-most-1800-deg-per-min.svg)

## Decision

1. **The built-in Z1's A limit is 1800 deg/min** (owner, 14 Nov 2026, relayed). `crates/openchuck-project/machines/makera-z1.toml` sets `rotary` and `rapid_rotary` to 1800. The value stays *confirmed*: its source cites `Robot.cpp:295-298` and the owner's `M203.1` reading `A: 30` on firmware 1.1.3, and says how to check a machine: a Z1 that prints `A: 60` can raise both A limits to 3600 in a copy of the profile. It is a plain limit, not a new profile field for the factory setting: the software needs the rate the controller holds, which `M203.1` shows, not the reason for it.
2. **The timing follows the profile, with one correction.** Path assembly already times every move from the profile's limits (decision 0076), so the F words, the estimates (GC-06, GC-12, SIM-08, setup sheet) and the GC-17 report follow the new value. Positions, A words and rapids do not change: a G0 has no F word, and the A limit changes only how long a move takes.

   The new limit exposed one defect (found by this lane, not in the brief). A fed move held to A's own limit was timed for the exact turn, but the post works the F word out from A written to three decimals, which can turn up to 0.001 degree further. With 3600 no move reached A's limit, so it never showed; at 1800, edge following on a STEP part wrote `F1802.2` on LinuxCNC, a gate 1 failure (GC-09). A fed move that turns A is now timed for the turn plus 0.001 degree, the same allowance decision 0076 item 2 already gives A in the Makera term and the line gives its positions (`openchuck-path::assemble`). A move A's limit sets takes up to 0.001 degree's time longer (0.02 % of a 5 degree step); nothing else changes. The owner accepted this code change, with the two test changes below, on 14 Nov 2026 (relayed by the coordinator, decision 0049); the record as a whole was accepted the same day. Test: `assemble::tests::req_gc_09_req_op_13_a_turn_at_a_s_limit_stays_within_it_as_written`.
3. **A project's old Z1 copy warns.** A project's machine copy is the project's own (SET-01), so it is not changed. When the copy is the Z1 (by its library ID, or by its name when it has none, decision 0109), still holds 3600 on either A limit, and its feed limits are still the built-in profile's (marked *confirmed*: the machine editor marks a value the user changes unconfirmed, UI-13), the setup checks and Messages say, under the topic Machine profile:

   "This project's Makera Z1 copy has the old A limit, 3600 deg/min. The Z1's controller turns A at most 1800 deg/min (30 deg/s), so programs from this copy ask for feeds the machine does not reach and their time estimates are short. Choose the Makera Z1 again in the setup's machine section to update the copy. To check your own Z1, send M203.1 in its console: it prints A's limit in deg/s."

   This follows decision 0317 item 3. A copy whose feed limits the user changed is theirs and is not flagged, so a Z1 that prints `A: 60` and keeps 3600 in its own copy does not warn.
4. **Sample projects and golden programs.** The fifteen sample machine copies (`testdata/projects/*/machine.toml` and `testdata/examples/*/*/machine.toml`) take item 1, so they neither warn nor differ from the built-in profile. The golden programs are blessed again: the reference jobs, the first cut and the blisk's helical programs for both posts (item 1), and the grouping programs and one line of the carving golden's `seconds` column on the example machine (item 2's allowance). Only F words and the estimates in operation comments change: a diff check strips them from the old and new files and finds the rest identical line for line (see *Verification*). drilling, swarf, face_flow, cone_axial, cone_axial_lead, cone_indexed and the blisk's axial program do not change.

## Options not chosen

- **A profile field for the factory setting** (an "A axis homing enabled" flag that picks 1800 or 3600). The flag is the firmware's reason, not the machine's rate; `M203.1` shows the rate directly. A field would need a schema change and would still have to be set from that reading.
- **Changing the project copies when a project is opened.** The copy is the project's source of truth (SET-01, decision 0109 option C); decision 0317 item 3 already set the pattern of naming it instead.
- **Keeping 3600 and leaving the slow moves to the firmware.** The programs would run as before, but their F words and every estimate would be wrong, and the GC-17 report would understate the short blocks.

## Consequences

- Every Z1 program that turns A while cutting near the axis states a longer time. The setup sheet's programmed time (Z1 post, the committed sample projects):

  | Job | At 3600 deg/min | At 1800 deg/min |
  | --- | --- | --- |
  | sphere | 1:21 | 2:12 |
  | groove | 2:21 | 3:07 |
  | cone_held_tabs | 2:32 | 4:31 |
  | cone | 1:46 | 3:09 |
  | lobed | 1:39 | 2:24 |
  | cone_axial | 7:01 | 7:07 |
  | cone_axial_lead | 7:18 | 7:24 |
  | cone_indexed | 1:02 | 1:06 |
  | first_cut | 38:37 | 1:07:52 |

  The programmed time includes the rapids and links, timed at the profile's rapid limits; the axial and indexed jobs change only there (their A turns between passes are rapids), and their G-code does not change. The firmware's own rule applied to the written programs (see *Verification*) gives the same time for the rc2 programs and the new ones, so the machine should run them exactly as before: only what the program says about its speed changes.
- On the sphere, the cone, the groove and cone_held_tabs, every finishing move away from the part's ends is now held at A's limit, below the programmed contact feed of 480 mm/min (on the sphere down to about 156 mm/min, as the run's feed-limit note says), so the chip load is lower there than the cutting data ask for. It was so on the machine before too, since the firmware already capped A; now the program says so. The golden audit's contact-feed check (decision 0020 item 14) checks each such move against the rotary limit, and its feed statistics come from lobed alone; the audit handles a job with every move at the limit instead of failing on an empty list. The schema 11 Z1 fixture's migration test now checks that the copy keeps its own 3600 deg/min (SET-01) and that the rest of its limits are the built-in Z1's. Both test changes were accepted by the owner with item 2's code change.
- The LinuxCNC programs change too: they take their times from the same IR (GC-10), so a move that A's limit slows has a lower F there as well.
- OI-267 (an A-only move whose F is above 1200 mm/min) is closed: the firmware neither refuses nor clamps such an F word, only A's rate limit applies. With A at 1800 such an F needs r above 33.4 mm.
- `openchuck-project` gains `machines::has_old_z1_a_limit`, an addition (no version bump needed, decision 0048); `openchuck-app` re-exports it and adds the warning.
- The user guide's scallop example quotes the tutorial project's programmed times in two screenshots and a table; they change (Rough 13:25 to 25:25, Finish 14:57 to 24:54) and are left to a re-capture after this change (OI-2145).
- Documents updated in this change: decisions 0007, 0013, 0076 and 0222 (notes), the UI specification (the Setup dock), the gate 4 and 5 instructions (P3), the Z1 safe-practice checklist, the user guide (machine profiles, confirming machine values) and the release notes.

## Re-verification strategy

The rc2 gate 4 run (firmware 1.1.3) stands for everything that does not depend on a move's speed, because only F words change. The diff check proves that: with the F words stripped, every new program is byte-identical to the rc2 one.

**Carried over from rc2 gate 4:** positions, A words and directions, collisions with the stock and the chuck, tool changes, the start and end states, and the unwinds. SP-12's conventions (A direction, offsets, the wrap) need no new run.

**Done again:**

1. **Gates 1 to 3 in CI and the full test suite** on the new build (automatic): gate 1 now holds every A-only F word and every A rate to 1800, and the golden audit checks the new programs against the 1800 limit.
2. **The LinuxCNC replay (gate 3)** of the changed programs, since their F words change.
3. **A spot check on the Z1**, at 100 % override, no tool, no material: the sphere (new programmed time 2:12; the firmware rule gives 126 s of fed moves, and the owner measured 127 s on rc2), the groove (3:07; 182 s fed) and cone_held_tabs (4:31; 265 s fed), which are the reference programs that turn A longest. Each measured time must match its new estimate within 10 %. As a control, cone_axial (7:07, 6 s more than before, all in its A rapids) or drilling must still take its rc2 time, since its G-code does not change.
4. **The first-cut air cut** (C11) has not run yet, so it runs once, on the new build only.
5. **The gate 5 first cut** runs on the new build only.

**What the spot checks cannot show.** They time whole programs, so a move that is too slow can hide behind one that is too fast; the per-move model is checked in CI (the test in item 1 of *Verification*), not on the machine. With no tool and no material they show nothing about cutting forces or surface finish at the new feeds, which are lower where A's limit binds, so the chip load drops there. They run on one Z1 on firmware 1.1.3; a Z1 with a different factory setting may hold A at another rate (check with `M203.1`). Gate 4's measured block-time limit (7 ms) is unchanged; the A-limited moves now state their real, longer time, so a program's short-block report can only fall. As before, every generated program is the user's to check before it runs (NFR-12).

## Verification

Run by lane ALIMIT on 14 Nov 2026 in a Linux cloud container (4 cores, rustc 1.97), on branch `claude/lane-alimit-793` from `main` at `bc1df28` (the 0.55.0-rc2 commit). Raw evidence is in [`docs/archive/0647-z1-a-limit/`](../archive/0647-z1-a-limit/README.md).

**Tests written from the requirements before the change.** The profile, timing and rounding tests were seen to fail first; the two warning tests were written with `has_old_z1_a_limit` and its message and passed at once:

- `openchuck-project` `machines::tests::req_set_01_req_gc_09_the_z1_turns_a_at_most_1800_deg_per_min`: the built-in Z1's two A limits are 1800 and their source cites `Robot.cpp:295-298`, `M203.1`, `A: 30` and this record.
- `openchuck-project` `machines::tests::req_set_01_req_ui_13_a_z1_copy_with_the_old_a_limit_is_recognised`: by ID and by name, either A limit, not another machine, not a renamed copy, not a limit the user edited.
- `openchuck-app` `overview::req_set_01_req_ui_13_req_gc_09_an_old_z1_a_limit_warns_and_the_new_z1_does_not`: the check and the message, their words, and the copy kept unchanged.
- `openchuck-app` `feed_limits::req_gc_04_req_gc_09_req_gc_06_req_gc_12_z1_moves_turn_a_at_most_1800_deg_per_min_and_are_timed_so`: helical finishing of a square box on the Z1. In the IR no move turns A faster than 1800 deg/min, and moves whose contact feed asked for more are held at it. In the written program every helical step's F asks A for at most 1800 (to the written decimals), and the firmware's rule gives the IR's fed time within 0.5 %. With 3600 it failed on the IR (`0.3125 deg in 0.0062 s`).
- `openchuck-path` `assemble::tests::req_gc_09_req_op_13_a_turn_at_a_s_limit_stays_within_it_as_written`: a turn written 0.401 degree for 0.4002 turned stays within 1800 under both feed rules (1803.6 before item 2's correction).

**Changed tests:** `assemble::tests::req_op_13_a_controller_that_counts_a_in_the_feed_word_keeps_it_within_the_limit` (the expected time includes the written-A allowance); `golden_audit::req_gc_09_req_gc_10_req_nfr_08_reference_job_golden_files_pass_the_audit` (a job with every move at the rotary limit); `setup::req_prj_04_req_op_13_an_older_z1_copy_migrates_to_count_a_in_its_feed_word` (the copy keeps its own A limit). The owner accepted all three with item 2 (relayed, 14 Nov 2026).

**Results:**

- Full suite, `cargo nextest run --workspace --locked --no-fail-fast`: 3412 tests, 3410 passed, 2 failed; the two were the audit and the migration test above, fixed afterwards and passing when run again on the final code. No new test is long: the new `feed_limits` test takes 0.4 s, the others under 0.1 s.
- STEP kernel tests (OpenCASCADE 7.8.1 built locally): `cargo nextest run -p openchuck-app --features kernel-tests --test step_pipeline`: 11 of 11 pass. The drilling, swarf and face_flow golden programs do not change.
- Diff check, `python3 docs/archive/0647-z1-a-limit/fdiff.py bc1df28`: in every changed G-code file only F words and the estimate comments change. Fed blocks whose F changes: sphere 720 of 722, cone 1080 of 1082, groove 1098 of 1100, lobed 1154 of 1802, cone_held_tabs 1585 of 1587, first_cut 14,639 of 15,450 (both posts each).
- Firmware time, `python3 docs/archive/0647-z1-a-limit/fwtime.py 1800 bc1df28:<file> <file>`: the rc2 Z1 programs and the new ones take the same fed time under the firmware's rule with A at 1800, within 0.02 % (sphere 126.3 s, groove 182.2 s, cone_held_tabs 265.1 and 265.2 s, first_cut 3163.0 and 3163.6 s).
- `cargo fmt --all`, `cargo clippy --workspace --all-targets`: clean. `cargo xtask records`, `deps`, `licences`, `trace`: ok. `cargo semver-checks --package openchuck-path --baseline-rev bc1df28`: no update required.
- Coverage of the changed lines (`cargo llvm-cov nextest --profile coverage`, the crates' own tests): every new line of `machines.rs`, `assemble.rs` and `overview.rs` is run.

**Not verified here:** nothing ran on a Z1 or on LinuxCNC; the re-verification runs above do that (OI-2143). The firmware source was read at `main` of `MakeraInc/MakeraZ1Firmware`; that 1.1.2 and 1.1.3 hold the same lines is lane FWDIFF's reading. What sets bit 0 of `FuncSetting` on a given machine was not checked: the owner's `M203.1` reading is the evidence for the owner's Z1. The user guide's scallop example is left to OI-2145.
