# 0222: Short moves: a minimum block time, reported first, then met by slowing down (lane SHORTMOVE; SP-14)

- **Status:** accepted (23 Oct 2026) by the project owner, given in this lane's own session (decision 0049). Proposed 22 Oct 2026. GATES #284 implemented Report and the profile/post fields at schemas 19/5; SHORTMOVE 0245 completes managed reporting and selective Slow down at schemas 20/6. The original investigation below predates the hardware trial; see the current follow-through note.
  - **Item 2 decided:** the owner chose option A (Report first, then Slow down; Merge deferred), relayed by the coordinator on 22 Oct 2026.
  - **Item 3 decided:** the owner chose option A, a window of N blocks held in the machine profile beside the minimum block time (N = 1 is the strict per-block case; the Z1 uses 32), relayed by the coordinator on 22 Oct 2026. OI-931 is closed.
  - **Item 7's air cut decided:** the owner chose to add it to the gate 4 procedure now, in lane GATES' draft pull request #284, and the Z1 profile ships with the check **off** until it has been run (OI-930 stays open until then).
  - **5 ms is not a Z1 value.** It is a placeholder threshold, used to describe the measurements and as a test value. No Z1 value is set until the air cut measures one (see *The 5 ms figure*).
- **What it amends:** GC-17 and the profile/post fields are now reflected in the requirements, architecture and verification plan by the GATES follow-through (25 Oct 2026). With the record accepted, the requirement (GC-17), the profile and post fields and the verification rows below are agreed. They are written into `docs/requirements.md`, `docs/architecture.md` and `docs/verification-plan.md` in the build lane's pull request, which also claims the project schema number (OI-935).
- **Date:** 22 Oct 2026.
- **Spike branch:** `spike/sp-14-short-moves` (throwaway, never merged, but kept: the gate 4 procedure fetches the air-cut program `spikes/sp14-short-moves/air-cut/block_rate_z1.nc` from it). The write-up is `spikes/sp14-short-moves/RESULT.md` and the raw tables are in `spikes/sp14-short-moves/results/`.
- **Open items:** OI-930 to OI-937 are new (OI-937 added after the published sources were read). OI-68 (arcs are not replayed) is touched but unchanged. OI-932 (the merge tolerance) stays open as deferred, with Merge.
- **Number note:** the lane was given 0222 and 0223. This investigation needs one record, so 0223 stays free.

## Current follow-through (25 Oct 2026)

The owner submitted the air-cut results in [#284 comment 6399134358](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6399134358). Both parts passed 20, 10 and 7 ms and stuttered at 5 ms and below, on Z1 firmware 1.1.2 via WiFi upload. The controller is identified as ESP32 without its firmware version. **7 ms is the shortest tested smooth setting**, not an exact capacity boundary. OI-930 is closed by this evidence; earlier statements below that the value is unknown describe the pre-trial investigation.

The owner instructed this session to execute Report followed by Slow down, with Merge still deferred. [0245](0245-short-block-report-and-slowdown.md) records the configurable implementation: new Z1 profiles Report at 7 ms/32, old project copies retain Off, optional post overrides, geometry-preserving window slowdown, adjusted replay timing and same-run setup sheets. Schema claim OI-935 is closed. Hardware mitigation validation remains open as OI-992; a software window target alone is not a smoothness guarantee. GATES #284 is now merged; its complete-window IR reporting API, schema guards, hardware evidence and waivers are preserved. The managed reporting path additionally examines actual emitted durations and bounded short runs as recorded in 0245.

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
  - **Stopping distance.** With a full queue, the speed limit that comes from stopping within 32 blocks (`sqrt(2 a L)`) stays above the programmed speed in every 32-block window of every file measured. The lowest ratio is 1.72, in pencil. This stopping-distance comparison assumes junction blending. **Correction, 25 Oct 2026:** the later [first-cut tail investigation](0245-short-block-report-and-slowdown.md#first-cut-far-end-tab-underflow-like-motion-at-100-override-oi-993-owner-report-25-oct-2026) identifies repeated A-only blocks as an exception in the published firmware: they bypass junction blending and can stop at vertices even with a full queue. A block-rate check cannot diagnose all stutter.
  - **Makera guidance.** Makera's documentation, forum and Fusion post say nothing about short segments or stutter.
- **LinuxCNC 2.9:**
  - G61, which our verification post writes, stops at every vertex that is not collinear.
  - A segment's speed is capped at `length / (1.02 x servo period)`. The servo period is 1 ms, so LinuxCNC needs blocks of about 1 ms or more.
  - `G64 P Q` blends and merges collinear moves, for XYZ-only moves.

## Decision (accepted 23 Oct 2026)

### 1. Requirement (agreed wording, ID GC-17, P1, Include)

> GC-17: A machine profile can set a minimum block time in milliseconds, which a post can override; 0, the default, turns the check off and leaves the G-code unchanged. When it is set, each fed move's time is worked out with the post's feed rule, and the user chooses, per machine profile or post: **Report** (the setup sheet and a warning state how many moves are under the limit, the longest run of them, the effective feed in that run and the time a slow-down would add, and the G-code is unchanged), or **Slow down** (the feed of each move under the limit is lowered so that the move lasts the limit; the toolpath geometry is unchanged). A later mode, **Merge**, may join consecutive moves within a stated tolerance before slowing down; merged moves are checked against the model by the independent checker.

GC-17 is the next free GC number on `main` and on every unmerged branch, checked on 22 Oct 2026. The owner may prefer to extend GC-04 instead.

### 2. Build order (decided: the owner's option A, relayed 22 Oct 2026)

**Report first, then Slow down. Merge is deferred**, to be built only if wanted later for block-count reduction, with its own tolerance taken from the budget (OI-932). This changes the owner's provisional order (Report and Merge first). The reasons are in the numbers:

- **Slow down** costs at most 1.4 % of an operation's time at 5 ms (7 % at 10 ms, pencil) and adds no geometric error. It needs no change to the tolerance budget. It is a deterministic pure function of the durations.
- **Merge** within the existing budget removes only 6 to 20 % of the short blocks. A useful merge tolerance (0.01 mm) takes the whole NFR-05 budget. It needs the independent checker and its own gate-2 handling, so it costs more and does less.
- **Report** costs almost nothing and tells the owner, on real jobs, whether the other modes are ever needed.

### 3. A window over the controller's queue (decided: the owner's option A, relayed 22 Oct 2026)

The check is **a block time averaged over a window of N blocks**, not a test of each block alone. N is the controller's queue depth: 32 for the Z1. With N = 1 it is the strict per-block form.

A queue absorbs short runs. The longest run measured is 11 blocks, but the worst 32-block mean is 5.6 ms. A strict per-block limit of 5 ms would flag 15,139 blocks on the axial file, where no 32-block window averages under 12 ms. Slow down then lowers the feed only where a window's mean is under the limit, lengthening the shortest blocks in that window first.

This refines the owner's model. The owner agreed it (option A), closing OI-931.

On the Z1 there are two buffers in series, so two window sizes matter (see *The 5 ms figure*). The 224-line receive buffer absorbs a slow line supply for up to about 256 lines. The 32-block planner queue absorbs slow parsing and planning for up to 32 blocks. The proposal uses the smaller of the two, 32, which is the stricter check.

### 4. Fields

- **Machine profile** (`openchuck-types` `MachineProfile`, SET-01):
  - `min_block_ms` (number, default 0);
  - `block_window` (whole number, default 1);
  - `short_block_mode` (`off`, `report` or `slow_down`; default `off`; `merge` reserved);
  - provenance per field, as for the other Z1 values (decision 0013).

  - The Z1 profile sets `block_window` 32 (fact: the firmware's planner queue, `Conveyor.cpp:75`) and keeps `min_block_ms` 0, so the check is off, until the gate 4 air cut measures a value (OI-930).
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

### 7. Verification (agreed rows; written into the verification plan by the build lane)

- **Tests**, written first from the requirement:
  - `req_gc_17_off_leaves_output_unchanged`: the reference goldens, byte-identical with 0.
  - `req_gc_17_report_counts_runs_and_windows`: a hand-made IR with known short runs.
  - `req_gc_17_slow_down_meets_the_limit`: every window's mean is at least the limit after the slow-down, the geometry is unchanged, and the time added equals the report's figure.
  - `req_gc_17_deterministic_at_any_thread_count`.
  - `req_gc_17_post_overrides_profile`.
  - A gate-2 test in which a slowed program replays within 1 % of the adjusted durations.
- **Golden.** The blisk pencil finish, or a small cut of it in `testdata/`, posted with the limit set to 5 ms. Here 5 ms is a test value chosen because it slows some blocks, not a default. The owner's projects are not used directly (their README).
- **Hardware check for the gate 4 procedure: the Z1 block-rate air cut.** The owner decided to add it to the gate 4 procedure now, in lane GATES' draft pull request #284 (`docs/z1-gates-4-5-test-instructions.md` on that branch; this record does not edit it). The specification below is complete enough to copy.
  - **Program.**
    - `spikes/sp14-short-moves/air-cut/block_rate_z1.nc` on `spike/sp-14-short-moves`, written by `gen_block_rate.py` beside it. Copy it into the gates lane's test files, or write it again from this specification.
    - It has 27,966 lines; the longest is 50 characters, under the 63-character limit of OI-933.
    - Header `G90 G21 G17`, `G54`, then `G0 X0 Y0 Z50 A0`.
    - Footer `G0 Z50`, `G92.4 A0 S0`, `G53 G0 Z-3`, `G28`, `M2`, as the Z1 post writes it (decision 0007).
    - No tool change and no spindle.
  - **Sections.**
    - Fourteen sections. Each is 100 mm of X at F1000, with a nominal time of 6.0 s.
    - Each is made of equal collinear `G1` blocks of one block time: 20, 10, 7, 5, 3, 2 and 1 ms. The block lengths are 0.333, 0.167, 0.117, 0.083, 0.050, 0.033 and 0.017 mm, so a section has 300, 600, 857, 1,200, 2,000, 3,000 or 6,000 lines.
    - The 7 ms section was added after the published Smoothieboard line rates (*The 5 ms figure*), which allow a floor between 5 and 10 ms.
    - The sections are separated by a Z marker rather than a `G4` dwell, because Makera's and Smoothie's pages disagree on the units of `G4 P` (OI-937).
    - Every section is longer than the 224-line receive buffer plus the 32-block queue (about 256 lines). A section at its predicted time therefore shows that the supply keeps up over time, not that the buffer covered it.
    - Sections alternate direction (X 0 to 100, then 100 to 0) to stay inside 100 mm of X travel.
    - Each section starts after a marker: `G1 Z55 F1000`, then `G1 Z50 F1000`, a visible 5 mm bob of 0.6 s. Start the stopwatch as Z comes back down to 50, and stop it as the next bob starts.
    - Each `G1` line carries X, Y, Z, A and F, written with three decimals as the post writes them (about 43 characters). The link limit is in bytes, so short test lines would flatter the supply rate.
  - **Parts A and B.**
    - **Part A** (sections 1 to 7): X only.
    - **Part B** (sections 8 to 14): the same, with A advancing in every block.
    - **The A step in part B.** Each block moves A by 60 % of its X length's worth under the Z1 rule: 0.628 degrees per mm of X at Z50, where P = 2π·50 + 30 mm. So X still sets each block's time, and A stays far below its limit.
    - **Why not the 0.2 degree step first proposed.** It cannot hold the short block times. The Z1's A axis is limited to 3600 degrees per minute (decision 0013), so a 0.2 degree step needs at least 3.3 ms. The rule's A term (0.19 mm at r = 50) would also outweigh the X length below 10 ms.
    - Part B exercises the firmware's mixed X and A path (`Robot.cpp:1640-1704`).
  - **Predicted times.** The prediction is inference from the firmware source: the planner plans the newest block to end at rest, and the acceleration is 150 mm/s². The 32-block queue then allows at most `sqrt(2 x 150 x 32 x L)` mm/s:

    | Block time | Stopping limit, mm/min | Predicted section time |
    | --- | --- | --- |
    | 20, 10, 7, 5, 3, 2 ms | 3,394, 2,400, 2,008, 1,697, 1,315, 1,073 | 6.0 s |
    | 1 ms | 759 | 7.9 s |

    Starting and stopping add about 0.1 s to each section.
  - **Setup and safety.** Gate 4 rules apply: no tool, no material, attended, emergency stop in reach. Use the gate 4 work offset (G54, Y0 Z0 on the A axis) with X0 placed so that X 0 to 100 at Z50 is clear travel.
  - **What to record.**
    - The firmware version and the controller (ESP32) version.
    - How the file reached the machine (WiFi upload, USB), and that it was played from the machine.
    - For each of the 14 sections: the stopwatch time, and whether there was audible stutter or a visible pause (yes or no).
    - A phone video with sound is a good second record.
  - **Pass and fail, per section.**
    - A section **passes** when its time is within 5 % of its predicted time (6.0 s, or 7.9 s for 1 ms) and it has no audible stutter or pause.
    - If the 20 ms or 10 ms section fails, stop the test and log a defect: nothing in the analysis predicts it.
  - **Result.**
    - The Z1's measured minimum block time is the shortest block time at which both parts pass and every longer block time passes. Record it in the Z1 profile's `min_block_ms`, marked confirmed with the firmware and controller versions (OI-930).
    - If every section passes, record 1 ms, because the planner-depth limit already covers shorter blocks.
    - A 1 ms section near 7.9 s confirms the stopping-distance prediction. One near 6.0 s means the firmware plans differently from its source (issue #2 on the firmware repository). Either way, record it in the decision that sets the value.
  - **Duration.** About 2 minutes of motion, 92 s fed.

### 8. What this does not do

- **No arc fitting.** The Z1 cannot move A in an arc, gate 2 does not replay arcs (OI-68), and decision 0016 rules out G2/G3 with A. G18 arcs on axial passes are a possible later step (OI-936).
- **No change to the LinuxCNC verification post's G61** (decision 0020): blending belongs to a production LinuxCNC post (OI-934).
- **No change to path fitting.** A minimum step length in the strategies would change toolpaths, so it would be a strategy decision in its own right.

## The 5 ms figure: evidence for the limit value

The owner asked whether 5 ms, and 1 ms or 10 ms, is a reasonable default or limit.

**Short answer.**
- **5 ms is not a measured Z1 value.** It is a placeholder: one of the thresholds (1, 2, 5 and 10 ms) chosen to describe the measurements, and the test value of the slow-down golden. No source gives the Z1's block rate, and no source names 5 ms. The check is off on the Z1 until the air cut measures a value (the owner's decision).
- **The evidence puts the Z1's floor somewhere between about 1 ms and 6 ms per line.** That is inference, set out below.
  - The firmware source alone suggests 1.2 to 2.2 ms.
  - Published Smoothieboard rates, read once the owner added network access, span 160 to over 1,000 lines a second from the board's own storage or serial (1 to 6 ms), and as few as 60 lines a second over USB in 2016.
  - The Z1's supply side is closed source, and no published rate is for a Carvera or a Z1.
- **Decided.** The default is **off**, and the Z1 profile's value comes from the air cut in item 7 (OI-930), marked confirmed. Until then the Z1 profile ships with the check off (the owner's decision, relayed 22 Oct 2026).
- **Not adopted, and no longer supported:** a provisional 3 ms. It sits above the floors worked out from the firmware source, but one published Smoothieboard ran at 160 lines a second, about 6.25 ms a line. So 3 ms could be too short, and the published range backs the owner's decision to keep the check off until it is measured. 5 ms with the window would slow nothing measured; 10 ms would slow pencil.

Each point below is marked **fact** (read in a source), **inference** (worked out from facts) or **not found**.

### The Makera Z1 controller (MakeraInc/MakeraZ1Firmware, HEAD 8ae81f3, 1.1.3; the files cited are unchanged since b3a2e26, 1.1.2)

- **Fact: lines arrive in packets of at most 504 bytes, over a serial link at 230400 baud.**
  - The motion processor (LPC1768) talks to the controller board (ESP32) on UART2 at 230400 baud with DMA (`src/libs/Kernel.cpp:147`, pins P2_8 and P2_9; `src/modules/communication/SerialConsole_DMA.cpp`).
  - A program played from the machine is sent from the ESP32 in `PTYPE_PLAY_DATA` packets. Each packet has at most 513 data bytes (`SerialConsole.cpp:136`), of which 504 are G-code text (`SerialConsole.cpp:225-262`).
- **Fact: the firmware asks for the next packet only when the previous one has arrived.**
  - It requests more lines when its buffer holds fewer than 180, and only once the previous request has been answered (a new request also goes out after 5 s with no answer).
  - The buffer holds 224 lines of 64 bytes (`Player.cpp:67, 75, 783-805`).
  - It executes one line per pass of the main loop (`Player.cpp:766-781`).
- **Inference: the link alone allows at most about 1.2 ms per line.**
  - At 230400 baud and 10 bits a byte, the link carries 23,040 bytes a second.
  - A packet of 504 bytes takes about 22 ms and holds about 14 lines of the post's 35-character lines (about 18 at 28 characters).
  - So the link alone allows at most about 650 to 800 lines a second, roughly 1.2 to 1.5 ms per line. Each round trip's ESP32 time (reading the stored file, framing) adds to that.
  - If the ESP32 takes 10 ms per packet, the rate falls to about 450 to 550 lines a second, roughly 1.8 to 2.2 ms per line.
  - The ESP32 firmware is closed source (MakeraZ1Firmware issue #1), so its time per packet is **not found**.
- **Fact: the planner plans every move to end at a stop.**
  - The planner queue holds 32 blocks (`Conveyor.cpp:75`).
  - With `minimum_planner_speed` 0 (`Planner.cpp:47`), the newest block in the queue is always planned to end at rest (`Planner.cpp:281, 325`).
  - **Inference:** the machine keeps its speed only while new blocks arrive before it must start braking for the newest one. When lines arrive more slowly than the blocks run, the queue empties and the machine slows towards a stop at the end of each queued block. That is the stutter.
  - The stepper waits up to 100 ms for a full queue before it starts (`queue_delay_time_ms`, `Conveyor.cpp:76, 185-189`). That affects the start of a program, not the run.
- **Inference: stopping distance is not the limit on these programs.** The acceleration is 150 mm/s² (`configZ1.default:445`). With a full queue, the speed allowed by stopping within the queued length stays at least 1.72 times the programmed speed in every 32-block window measured (SP-14). So on these programs the risk is the supply and the parse rate, not the queue depth.
- **Inference: the queue depth sets a floor of its own.** Blocks of time t at speed v fill the 32-block queue with 32·v·t of path. Stopping within that allows `v <= sqrt(2 a 32 v t)`, that is `t >= v / (2 a 32)`. That is 1.74 ms per block at 1,000 mm/min and 2.08 ms at the Z1's 1,200 mm/min. Below it the machine slows even when lines arrive in time, so the 32-block window check with a limit of about 2.1 ms or more covers this too. The air cut's 1 ms section tests it (predicted 7.9 s against 6.0 s nominal).
- **Not found: the parse and plan time per line on the LPC1768.** Neither the firmware nor anything published gives it.
  - As a scale only, Grbl's 2012 planner rewrite reports 1.9 ms per arc segment on a 16 MHz AVR (Grbl commit log, mirrored at gitea.psi.ch/motion/ecmc_plugin_grbl). The LPC1768 runs at 100 MHz, so this is an order of magnitude, not a figure for the Z1.
- **Not found: anything from Makera.** No block-rate, stutter or segment guidance was found in Makera's sources:
  - Makera's own Fusion post (`Carvera.cps` in CarveraProfiles at de08a65) sets `minimumChordLength` 0.25 mm and tolerance 0.002 mm, and has no minimum segment time. **Inference:** at 1,000 mm/min, a 0.25 mm chord is a 15 ms block.
  - Makera's web documentation (wiki.makera.com) and forum (community.makera.com) were not reachable from this session (see below).
- **Fact: a firmware issue doubts the source.** Issue #2 on MakeraZ1Firmware says the source may not match the shipped binary. Every Z1 figure above is therefore medium confidence until the air cut.

### Other controllers, for comparison

- **Marlin 2.1.2.5 — fact.** Its rule is the closest published equivalent of the owner's model.
  - `DEFAULT_MINSEGMENTTIME 20000` µs, the "minimum time that a segment needs to take as the buffer gets emptied" (`Configuration_adv.h:1141`).
  - It applies only while the queue is less than half full (`SLOWDOWN`, `SLOWDOWN_DIVISOR 2`). There it adds time in proportion to how empty the queue is (`src/module/planner.cpp:2376-2391`).
  - **Inference:** Marlin's large value (20 ms) is safe because it applies only when the queue is draining. That supports item 3's window over the queue rather than a per-block test.
- **LinuxCNC 2.9 — fact.**
  - The servo period is 1 ms (`docs/src/config/ini-config.adoc:599`).
  - The speed of a segment is capped at its length over 1.02 servo periods (`src/emc/tp/tp.c:1708-1709`, `tp_types.h:23-26`), so blocks shorter than about 1.02 ms run slower than programmed.
  - The documentation's rule is `min_length = v x period`, and its Naive CAM tolerance advice is about half of that (`ini-config.adoc:798-808`).
  - So 1 ms is LinuxCNC's hard floor, not a margin.
- **Grbl — fact.** The 2012 development notes say short, fast segments can run faster than new blocks are added, so "the planner buffer will starve" with "hiccup-like jerky motion". They also say the segment length was tied to an arc tolerance for that reason. This states the mechanism; it gives no number for the Z1.

### What 1, 5 and 10 ms would do on the jobs measured (32-block window)

| Limit | Reference jobs | Blisk helical / rest (axial) / pencil | Fit with the evidence |
| --- | --- | --- | --- |
| 1 ms | Nothing slowed | Nothing slowed | LinuxCNC's hard floor. Below the Z1's estimated link floor of about 1.2 ms per line, so too low for the Z1. |
| 3 ms | Nothing slowed | Nothing slowed (worst mean 5.6 ms) | Above the 1.2 to 2.2 ms floors from the firmware source, but below the 6.25 ms of a published Smoothieboard at 160 lines a second. Not safe as a provisional value. |
| 5 ms | Nothing slowed | Nothing slowed by the window. Per block it would slow 382 / 15,139 / 801 blocks. | Inside the published range. Nothing measured needs it. |
| 10 ms | Nothing slowed | Pencil slowed (its worst mean is 5.6 ms) | Higher than any evidence supports. It costs up to 7 % on pencil per block, less with the window. |

### Published sources (read 22 Oct 2026, after the owner added network access)

The quotes are exact. The downloads stay outside the repository.

- **Makera wiki** (wiki.makera.com; all 100 English pages searched, including the Z1 manual, quick start and 4-axis pages, last updated 16 Oct 2026):
  - **Fact.** https://wiki.makera.com/en/supported-codes (updated 4 Feb 2026, written for the Carvera): "The Carvera firmware is based on Smoothieware". It lists G2/G3 and "G4 Dwell P<seconds>".
  - **Fact.** The Carvera software manual: "The current task cannot be stopped immediately when the task is paused or held. It has to wait for the buffer zone finish." It gives no buffer size.
  - **Not found.** Any line-length or comment limit, block rate, look-ahead, segment or tolerance advice, or stutter troubleshooting. Nothing on streaming against playing from the machine.
- **Smoothieware** (smoothieware.org; the pages show no dates):
  - **Fact.** https://smoothieware.org/configuration-options:
    - `planner_queue_size`: "32 is optimal for most machines… Must be at least 8 for proper operation".
    - `minimum_planner_speed`: "A value of 0.0 (default) allows the planner to decelerate to a complete stop at corners when needed".
    - `junction_deviation`: "0.01 - Maximum precision for critical work".

    These agree with the Z1 source: 32 blocks, 0 and 0.01.
  - **Fact.** https://smoothieware.org/supported-g-codes: "G4 → Dwell S or P , In grbl mode P is float seconds". This disagrees with Makera's "P<seconds>" (OI-937). No post writes `G4` today.
  - **Not found.** A block rate, a line-length limit, a stutter page.
- **Smoothie users' measurements** (forum.makerforums.info; Smoothieboard LPC1769, not the Z1):
  - **Fact.** Topic 76554:
    - wolfmanjm, 11 Jun 2016: "if you have lots of very small segments, then yes USB is too slow to keep the buffer full… This can even happen when printing from SD card with very small segments. Buffering will not help if the movements complete faster than the planner can be filled."
    - spe3Dy, 13 Jun 2016: over USB "up to 60 lines per second, movement was smooth. At 70… hickups"; from SD "400 lines per second totally smooth".
    - spe3Dy, 24 Jun 2016: "Smoothie onboard SD starts choking at slightly over 900 lines per second."
  - **Fact.** Topic 76277, 30 Apr 2017:
    - mayhem2408: "about 160 lines of gcode per second processed from the card… over serial… still got 160 lines per second".
    - Arthur Wolf (Smoothieware): "Smoothie is known to go over 1k lines/seconds".
  - **Fact.** Topic 8795, 10 Dec 2016:
    - Todd Fleming (LaserWeb): his CAM limits segments to ".5 mm default" to avoid stalls, which he found in recorded waveforms as "spots where both axis go quiet".
    - Arthur Wolf: "It's going to be different based on the machine's config".
  - **Inference.** These rates are 1 to 17 ms a line, depending on board, firmware build and transport. The Z1's transport is its own serial link from the controller board, which none of them measured. This is why the value must be measured on the Z1.
- **LinuxCNC 2.9** (linuxcnc.org, pages updated 17 Oct 2026):
  - **Fact.** `gcode/overview`: "a maximum (currently 256) to the number of characters allowed on a line".
  - **Fact.** `config/ini-config`: "min_length ~= v_req * t_c… any segments shorter than min_length will slow the path down", and "very tiny segments will slow things down significantly".
  - **Fact.** `gcode/g-code`: G61 "Moves will slow or stop as needed to reach every programmed point", and G64 P with the Naive CAM Detector Q, as quoted above.
- **Line length (OI-933).** No Makera or Smoothie page gives a line-length limit, so the 63 characters found in the Z1 source is neither confirmed nor contradicted. LinuxCNC allows 256.

### What still could not be reached

- community.makera.com: the proxy gateway answers 502 to the connection (an upstream failure, or still outside the policy).
- smoothieware.github.io: the policy denies it (403).
- forum.makerforums.info rate-limited the reader after seven topics. Topics 8508, 82833 and 16383 were seen only as search snippets and are not cited.

### What the owner will measure instead (decided)

The air cut in item 7, added to the gate 4 procedure in #284, measures the Z1's minimum block time directly:
- its sections cover the whole range above, at 1, 2, 3, 5, 7, 10 and 20 ms;
- each section is longer than both buffers;
- it takes about 2 minutes.

Its result sets the Z1 profile's value (OI-930); its pass rule is in item 7.

## Alternatives considered

- **Merge first, slow down as the fallback** (the provisional order). The measurements show merge removes few short blocks within the budget. Kept as the later Merge mode.
- **A per-block check only (N = 1).** It is simple, but it over-reports by an order of magnitude on a controller with a 32-block queue. It is kept as the N = 1 case of the window.
- **Checking the G-code text after rendering.** It gives the same numbers, because time is the same under both posts. It is later in the pipeline and cannot feed the slow-down back. Rejected.
- **A block rate in lines per second instead of milliseconds.** It is equivalent. The owner chose milliseconds.
- **Controller words.** None exist on the Z1. LinuxCNC's G64 is out of scope for the verification post.

## Consequences

- **Accepted:** the requirement, verification plan and architecture edits above are made in the build lane's pull request. The work breakdown gains a task: Report and Slow down in the post stage, the profile and post fields with a schema claim, the setup sheet lines, and the tests.
- **The gate 4 procedure** gains the block-rate check above. Until it runs, the Z1 profile ships with the check off.
- **New open items:**
  - OI-930: the Z1's effective minimum block time is not measured.
  - OI-931: per-block or window check.
  - OI-932: the merge tolerance budget.
  - OI-933: Z1 lines over 63 characters are cut short.
  - OI-934: G61 in the LinuxCNC post.
  - OI-935: the schema claim.
  - OI-936: arc fitting on A-free passes.
  - OI-937: the units of `G4 P` on the Z1 (Makera and Smoothie disagree).

## GATES follow-through, 25 Oct 2026

The owner’s [PR #284 result](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6399134358) records 7, 10 and 20 ms passing in both X-only and simultaneous X/A sections; 5, 3, 2 and 1 ms stuttered in both parts (eight failures). The measured minimum passing tested value is 7 ms. Firmware 1.1.2, ESP32 (firmware version not supplied), WiFi upload, build 0.20.0 at dc610e26dd260d69b5fc859270e7bf5fc913a3bb. This is not a measured limit for other programs, transports or controllers. The owner explicitly retained original K4 as a release waiver in the takeover session; a subsequent local LinuxCNC replay passed. See the full [results audit](../archive/0244-z1-gates-4-5/z1-gates-4-5-results-and-review-record.md#10-results).

The new built-in Z1 profile uses `min_block_ms = 7.0`, `block_window = 32`, `short_block_mode = "report"`. Existing project copies retain their own profile and default Off when the fields are absent; no migration enables the check. Machine schema 5 and project schema 19 were claimed before use. Library version 0.22.0 accounts for the public profile/post API additions. Report does not modify G-code or replay tolerances. Report itself changes no G-code; the software-version bump changes the generator version comment. Released main integration separately adopts accepted decision 0241’s rotary golden changes (results 10.9). Setup sheets and warning text may gain Report information.

OI-930 and OI-935 are closed by the measurement/profile implementation and the schema claim, with their rows moved to the Closed table. OI-932 to OI-934, OI-936 and OI-937 remain open; OI-931 was already closed by the owner’s accepted averaging rule.
