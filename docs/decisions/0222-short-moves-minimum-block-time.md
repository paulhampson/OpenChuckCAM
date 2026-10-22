# 0222: Short moves: a minimum block time, reported first, then met by slowing down (lane SHORTMOVE; SP-14)

- **Status:** proposed (22 Oct 2026). This is an investigation for the owner to agree or refine. Nothing is built. Only the owner accepts a record (decision 0049).
- **What it amends:** nothing yet. The requirement, the profile and post fields, and the verification rows below are proposals. `docs/requirements.md`, `docs/architecture.md` and `docs/verification-plan.md` are not edited (CLAUDE.md).
- **Date:** 22 Oct 2026.
- **Spike branch:** `spike/sp-14-short-moves` (throwaway, never merged). The write-up is `spikes/sp14-short-moves/RESULT.md` and the raw tables are in `spikes/sp14-short-moves/results/`.
- **Open items:** OI-930 to OI-936 are new. OI-68 (arcs are not replayed) is touched but unchanged.
- **Number note:** the lane was given 0222 and 0223. This investigation needs one record, so 0223 stays free.

## Context

The owner raised this outside GitHub. On some CNC controllers, if consecutive moves are too short, the controller cannot keep its motion buffer full, and the machine stalls: it moves jerkily or pauses.

The owner agreed this model on 22 Oct 2026:

- **The property.** The property is a minimum block time in milliseconds. 0 means off. Off is the default, so existing G-code does not change (GC-07, NFR-06).
- **Where it lives.** It is held in the machine profile, because it belongs to the controller, and a post can override it.
- **How a move is timed.** A move's time comes from the post's feed rule, the same rule as the "about N s" comment (decision 0007).
- **The modes:**
  - **Off.**
  - **Report:** a setup sheet line and a warning, with no G-code change.
  - **Merge:** combine short moves within a user tolerance, verified by the independent checker.
  - **Merge then slow down:** cap the feed on the blocks that are still short.
- **Build order.** The owner provisionally agreed Report and Merge first, with slow-down as the fallback, and will confirm the order after seeing the measurements.

Requirements touched:

- GC-04 (post options);
- GC-06 and GC-12 (comments, setup sheet);
- GC-07 and NFR-06 (determinism);
- GC-08 (G-code checks);
- OP-13 (feed mode);
- SET-01 (machine profile);
- NFR-05 (the tolerance budget);
- NFR-12 (the user verifies the program).

Architecture sections touched: 5.1, 5.3 and 6 (the post stage), and section 4 (library family rules). Verification plan sections touched: 4.1 (acceptance limits) and 6 (gates 2 and 4).

## What the spike measured

The full tables are in RESULT.md. Here is the summary.

### Every move is already timed by the post

Each post chooses F so that a block takes the IR segment's duration under its own rule (decision 0007). The block times of the Z1 and LinuxCNC files are therefore the same, within the rounding of F, which is under 0.3 %. Fed blocks match the IR's fed segments one to one. The check can run on the IR's durations in the post's neutral stage, without reading the text back.

### Reference jobs

The reference jobs have essentially no short blocks:

- All of `testdata/golden/reference/`, and the `first_cut` job on `claude/docs-gates-4-5-test-instructions` (#284), have no block under 5 ms except one 2.28 ms block in `first_cut`.
- The axial reference jobs have 4 to 32 blocks of 9.49 ms.
- No file has a zero-length block.

### The owner's blisk

The owner's blisk does have short blocks:

| Operation | Blocks | < 1 / 2 / 5 / 10 ms | Longest run < 5 / < 10 ms | Shortest block | Worst mean over 32 blocks | Time a slow-down to 5 / 10 ms adds |
| --- | --- | --- | --- | --- | --- | --- |
| Rotary roughing (3 projects) | 16k to 69k | 0 / 0 / 0 / 0 | 0 / 0 | 36 ms | 232 ms | 0 |
| Helical finishing | 57,895 | 12 / 109 / 382 / 1,848 | 3 / 4 | 0.86 ms | 21.2 ms | 0.8 s / 5.5 s of 3,211 s |
| Axial finishing (rest, tapered ball) | 150,523 | 99 / 374 / 15,139 / 31,014 | 6 / 10 | 0.47 ms | 12.2 ms | 15.7 s / 147 s of 21,715 s |
| Pencil finishing | 3,565 | 19 / 85 / 801 / 1,382 | 8 / 11 | 0.34 ms | 5.6 ms | 1.4 s / 7.1 s of 97 s |

### Causes

- **Path fitting by bisection.** Almost every short block comes from path fitting at high curvature, at the blade edges: steps are halved down to about 0.06 mm, or to an eighth of the A step, to hold the 0.004 mm chord limit (decision 0018 item 12).
- **Tip-speed compensation.** It raises F on convex regions: the short blocks run at 730 to 1,190 mm/min against a contact feed of about 500.
- **What does not cause them:** links, plunges, ramps and leads (none under 10 ms in any project), A stepping (`max_angle_step` gives long blocks), and output rounding (no zero-length blocks).

### Merging inside the budget hardly helps

- **Inside the budget.** The budget leaves about 0.001 mm for a merge (verification plan 4.1: 0.005 mm for fitting, of which the fit uses 0.004 mm). A joint-space merge at 0.002 to 0.005 mm removes 6 to 20 % of the short blocks. The short blocks follow the shape, so they cannot be merged away.
- **At 0.01 mm**, the whole budget, a merge removes 73 % on axial finishing but only 19 % on pencil.
- **Merging every block** at 0.005 mm cuts the block count by 47 to 98 %. A helix at constant radius is a straight line in X and A. That is a different benefit: less line traffic.

### Controller facts

These were read from source, not measured on the machine:

- **Makera Z1 firmware** (Smoothieware fork, MakeraZ1Firmware 1.1.2 and 1.1.3):
  - **Planner and motion settings.**
    - The planner queue holds 32 blocks.
    - The junction deviation is 0.01, the acceleration is 150 mm/s², and `minimum_planner_speed` is 0.
    - The firmware has no G61 or G64 and no look-ahead control word.
    - G2 and G3 do not move A (`Robot.cpp:1957`, a TODO).
  - **How lines arrive.** The ESP32 board sends lines over a 230400-baud serial link, and the firmware executes one line per main-loop pass. That puts the floor at about 1.5 ms per line. The real rate is not known: the ESP32 side is closed source.
  - **Stopping distance.** With a full queue, the speed limit that comes from stopping within 32 blocks (`sqrt(2 a L)`) stays above the programmed speed in every 32-block window of every file measured. The lowest ratio is 1.72, in pencil. So **the only way these programs can stall the Z1 is the line supply.**
  - **Makera guidance.** Makera's documentation, forum and Fusion post say nothing about short segments or stutter.
- **LinuxCNC 2.9:**
  - G61, which our verification post writes, stops at every vertex that is not collinear.
  - A segment's speed is capped at `length / (1.02 x servo period)`. The servo period is 1 ms, so LinuxCNC needs blocks of about 1 ms or more.
  - `G64 P Q` blends and merges collinear moves, for XYZ-only moves.

## Decision proposed (for the owner to agree or refine)

### 1. Requirement (proposed wording, ID GC-17, P1, Include)

> GC-17: A machine profile can set a minimum block time in milliseconds, which a post can override; 0, the default, turns the check off and leaves the G-code unchanged. When it is set, each fed move's time is worked out with the post's feed rule, and the user chooses, per machine profile or post: **Report** (the setup sheet and a warning state how many moves are under the limit, the longest run of them, the effective feed in that run and the time a slow-down would add, and the G-code is unchanged), or **Slow down** (the feed of each move under the limit is lowered so that the move lasts the limit; the toolpath geometry is unchanged). A later mode, **Merge**, may join consecutive moves within a stated tolerance before slowing down; merged moves are checked against the model by the independent checker.

GC-17 is the next free GC number on `main` and on every unmerged branch, checked on 22 Oct 2026. The owner may prefer to extend GC-04 instead.

### 2. Build order (this changes the provisional order)

**Report first, then Slow down. Merge is deferred**, to be built only for block-count reduction, with its own tolerance. The reasons are in the numbers:

- **Slow down** costs at most 1.4 % of an operation's time at 5 ms (7 % at 10 ms, pencil) and adds no geometric error. It needs no change to the tolerance budget. It is a deterministic pure function of the durations.
- **Merge** within the existing budget removes only 6 to 20 % of the short blocks. A useful merge tolerance (0.01 mm) takes the whole NFR-05 budget. It needs the independent checker and its own gate-2 handling, so it costs more and does less.
- **Report** costs almost nothing and tells the owner, on real jobs, whether the other modes are ever needed.

### 3. A window over the controller's queue

The check is **a block time averaged over a window of N blocks**, not a test of each block alone. N is the controller's queue depth: 32 for the Z1. With N = 1 it is the strict per-block form.

A queue absorbs short runs. The longest run measured is 11 blocks, but the worst 32-block mean is 5.6 ms. A strict per-block limit of 5 ms would flag 15,139 blocks on the axial file, where no 32-block window averages under 12 ms. Slow down then lowers the feed only where a window's mean is under the limit, lengthening the shortest blocks in that window first.

This refines the owner's model, so it is put to the owner as a question (OI-931).

### 4. Fields

- **Machine profile** (`openchuck-types` `MachineProfile`, SET-01):
  - `min_block_ms` (number, default 0);
  - `block_window` (whole number, default 1);
  - `short_block_mode` (`off`, `report` or `slow_down`; default `off`; `merge` reserved);
  - provenance per field, as for the other Z1 values (decision 0013).

  The Z1 values stay unset (0) until gate 4 measures them (OI-930).
- **Post definition** (`openchuck-post` `PostOptions`, GC-04): optional overrides of the same three fields. They follow the precedence of the post's `max_feed`.
- **Format change.** `machine.toml` is part of the project folder, so this changes the project file format. It needs a project schema number, to be claimed in the work breakdown's table when the build is planned (OI-935; not claimed here).

### 5. Where the check sits (architecture 5.3, section 6)

- **In the post's neutral stage.** The check and the slow-down are pure functions over the IR's segment durations, in the neutral machine program stage of `openchuck-post` (`convert.rs`). That is where the feed rule is already applied (decision 0019 item 5). They need only `openchuck-types`, so the library family rules (architecture section 4) hold.
- **A slowed segment** keeps its pose and kind, and gets a longer duration. The feed rule then writes a lower F.
- **The report** goes back with the program (`PostOutput`). The application shows it as a warning, using the grouped notes of decision 0219, and writes it on the setup sheet (GC-12).
- **Gate 2** (the replay comparison) compares the G-code with the durations after the slow-down, so the time check keeps its ±1 % meaning.
- **Merge, if built,** is a toolpath-to-toolpath transform before the post. Its output goes through the independent gouge checker (OP-12) like any toolpath.

### 6. Determinism, tolerance and the setup sheet

- **Off is byte-identical** to today, so no golden changes until a profile sets a limit.
- **Report** changes no G-code.
- **Slow down** depends only on the ordered durations, and is sequential, so it gives the same bytes at any thread count (GC-07, NFR-06).
- **No tolerance is used** by Report or Slow down. Merge would take its tolerance from the NFR-05 budget, and the verification plan's 4.1 row would say how (OI-932).
- **The setup sheet**, per operation, gives: the limit and its source (profile or post), the mode, the count under the limit, the longest run, the worst window mean, the effective feed in the worst run, and the seconds added. The "about N s" comment then includes the added time.

### 7. Verification (proposed rows; the verification plan is not edited)

- **Tests**, written first from the requirement:
  - `req_gc_17_off_leaves_output_unchanged`: the reference goldens, byte-identical with 0.
  - `req_gc_17_report_counts_runs_and_windows`: a hand-made IR with known short runs.
  - `req_gc_17_slow_down_meets_the_limit`: every window's mean is at least the limit after the slow-down, the geometry is unchanged, and the time added equals the report's figure.
  - `req_gc_17_deterministic_at_any_thread_count`.
  - `req_gc_17_post_overrides_profile`.
  - A gate-2 test in which a slowed program replays within 1 % of the adjusted durations.
- **Golden.** The blisk pencil finish, or a small cut of it in `testdata/`, posted with the limit set to 5 ms. The owner's projects are not used directly (their README).
- **Hardware check for the gate 4 procedure** (`docs/z1-gates-4-5-test-instructions.md` on #284; not edited here). This is an attended air cut, with no tool and no material, under the gate 4 rules:
  - **Program.** A program with six sections, each 100 mm of X travel at F1000, made of equal collinear blocks of 20, 10, 5, 3, 2 and 1 ms. The block lengths are 0.333, 0.167, 0.083, 0.050, 0.033 and 0.017 mm, and the 1 ms section has 6,000 lines.
  - **Variant.** The same blocks with a 0.2 degree A step in each, which exercises the firmware's A speed handling.
  - **Measurement.** Stopwatch each section, as SP-12's T3 tests did. The nominal time is 6 s per section. The block time at which the measured time first exceeds the nominal by more than 5 % is the Z1's effective minimum block time, together with the firmware and the ESP32 version.
  - **Listening.** Also listen for stutter in each section.
  - **Use of the result.** That figure sets the Z1 profile's `min_block_ms`, marked confirmed (OI-930). The test runs about 2 minutes.

### 8. What this does not do

- **No arc fitting.** The Z1 cannot move A in an arc, gate 2 does not replay arcs (OI-68), and decision 0016 rules out G2/G3 with A. G18 arcs on axial passes are a possible later step (OI-936).
- **No change to the LinuxCNC verification post's G61** (decision 0020): blending belongs to a production LinuxCNC post (OI-934).
- **No change to path fitting.** A minimum step length in the strategies would change toolpaths, so it would be a strategy decision in its own right.

## Alternatives considered

- **Merge first, slow down as the fallback** (the provisional order). The measurements show merge removes few short blocks within the budget. Kept as the later Merge mode.
- **A per-block check only (N = 1).** It is simple, but it over-reports by an order of magnitude on a controller with a 32-block queue. It is kept as the N = 1 case of the window.
- **Checking the G-code text after rendering.** It gives the same numbers, because time is the same under both posts. It is later in the pipeline and cannot feed the slow-down back. Rejected.
- **A block rate in lines per second instead of milliseconds.** It is equivalent. The owner chose milliseconds.
- **Controller words.** None exist on the Z1. LinuxCNC's G64 is out of scope for the verification post.

## Consequences

- **If accepted:** the requirement, verification plan and architecture edits above are proposed to the owner and made in the build lane's pull request. The work breakdown gains a task: Report and Slow down in the post stage, the profile and post fields with a schema claim, the setup sheet lines, and the tests.
- **The gate 4 procedure** gains the block-rate check above. Until it runs, the Z1 profile ships with the check off.
- **New open items:**
  - OI-930: the Z1's effective minimum block time is not measured.
  - OI-931: per-block or window check.
  - OI-932: the merge tolerance budget.
  - OI-933: Z1 lines over 63 characters are cut short.
  - OI-934: G61 in the LinuxCNC post.
  - OI-935: the schema claim.
  - OI-936: arc fitting on A-free passes.
