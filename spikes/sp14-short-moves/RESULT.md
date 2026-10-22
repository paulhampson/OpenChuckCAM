# SP-14 result: short moves and the controller's block rate

Throwaway spike (AGPL-3.0-or-later). Never merged. The decision record is [0222](../../docs/decisions/0222-short-moves-minimum-block-time.md) on the branch `claude/docs-decision-0222-short-moves`.

## Question

When consecutive moves are very short, can the controller keep its motion buffer full, or does the machine stall? The owner agreed the model on 22 Oct 2026: a **minimum block time** in milliseconds, held in the machine profile, that a post can override. Each move's time comes from the post's feed rule (decision 0007). The modes are Off, Report, Merge, and Merge then slow down. The owner will confirm the build order after seeing measurements. This spike provides those measurements.

## Method

- **Measured from the programs as written.** `src/lib.rs` reads each program with `openchuck-gcode` (`read`, `replay`). It times every `G1` block by the feed rule of its post: `makera` for `.nc` files, `linuxcnc` for `.ngc` files. These are the rules the post uses for its "about N s" comment and that gate 2 checks. Rapids are timed at the Z1's axis maxima (decision 0013).
- **Statistics, per file.** For each file, the harness counts:
  - the feed blocks under 1, 2, 5 and 10 ms;
  - the longest run of consecutive feed blocks under each limit, and the effective feed in that run if each block lasted the limit;
  - the seconds a slow-down to each limit would add;
  - the percentiles of block time;
  - two planner-window figures, using the Z1's 32-block queue (below). The first is the shortest mean block time over any 32 consecutive feed blocks. The second is the lowest ratio, over all windows, of the stopping-distance speed limit `sqrt(2 a L)` to the programmed speed, with a = 150 mm/s² and L the window's path length.
- **Merge experiment.** `merge` greedily joins consecutive `G1` blocks into one joint-space line, with three limits:
  - the two-way sampled distance between the old and new tool-tip paths, in the part frame, stays within a tolerance (0.002, 0.005 or 0.01 mm);
  - the programmed feed varies by at most 1.25 times within a merged block;
  - a merged block holds at most 64 blocks.

  The merged block keeps the summed time. The "<10 ms" variants merge only blocks shorter than 10 ms; "all" merges any block.
- **Per move kind.** `src/bin/pstat.rs` opens each project with `openchuck-app` at `main` 3335db8. It regenerates the project once per post, writing one file per operation, and counts the IR segments by `MoveKind` under each limit, so that each short block's cause is known.
- **Inputs.**
  - The reference jobs: `testdata/golden/reference/` on `main`, plus the projects on `claude/docs-gates-4-5-test-instructions` (#284) written out by its `sample_projects` example. That includes `first_cut`, whose goldens are `testdata/golden/first_cut/`.
  - The blisk goldens in `testdata/golden/blisk/`.
  - The owner's four blisk projects on `claude/upbeat-davinci-jk46yd` (`docs/trial-feedback-projects/`). These were re-saved to schema 17, given a spindle speed of 12000 rpm (the posts refuse an operation without one, decision 0196), and in `blisk_1_overcut_mismatch` the roughing entry was changed from ramp to plunge, because the ramp is refused at the held region (issue 199). None of these edits changes any block time of the finishing operations.
- **Outputs.** The raw tables are `results/projects.txt` (projects) and `results/goldens.txt` (golden files). `scripts/short_context.py` prints a short block with its neighbours.
- **Controller facts.** These were read from source, not measured:
  - the Makera Z1 firmware `MakeraInc/MakeraZ1Firmware` at HEAD 8ae81f3 (1.1.3; the cited files are unchanged since b3a2e26, 1.1.2);
  - Makera's Fusion post `Carvera.cps` (CarveraProfiles de08a65);
  - LinuxCNC 2.9 sources and documentation sources (linuxcnc.org was unreachable).

Run it with:

```text
CARGO_TARGET_DIR=../../target/sp14 cargo build --release
../../target/sp14/release/gstat <file.nc|file.ngc>...
SP14_OUT=<dir> ../../target/sp14/release/pstat <project folder>...
```

## Numbers

### Block time is the same under both posts

Each post chooses F so that every block takes the IR's duration under its own rule. The block-time tables of the Z1 and LinuxCNC files are therefore identical, apart from the rounding of F, which changes times by under 0.3 %. Fed blocks match IR fed segments one to one: on the rest-machining file, 148,837 cutting, 586 linking and 1,100 plunge segments make 150,523 fed blocks. The check can therefore run on the IR's durations in the post's neutral stage. It does not need to read the text back.

### Reference jobs (all posts)

| Job | Fed blocks | < 5 ms | < 10 ms | Shortest block, ms | 32-block mean, ms (worst window) |
| --- | --- | --- | --- | --- | --- |
| cone, cone_held_tabs, sphere, lobed, groove | 722 to 1,802 | 0 | 0 | 39.5 to 83.3 | 42 to 112 |
| cone_axial, cone_axial_lead, cone_indexed | 64 to 1,216 | 0 | 4 to 32 | 9.49 | 72 to 460 |
| drilling, swarf | 6, 36 | 0 | 0 | 500, 1,250 | n/a |
| first_cut rough (rotary roughing) | 12,296 | 0 | 0 | 20.8 | 78 |
| first_cut finish (helical) | 4,104 | 1 | 1 | 2.28 | 83 |

There are no zero-length blocks in any file. The only reference block under 5 ms is a single 2.28 ms block in `first_cut`.

### Owner's blisk (Z1 post; LinuxCNC is the same)

| Operation (strategy) | Fed blocks | < 1 ms | < 2 ms | < 5 ms | < 10 ms | Longest run < 5 ms / < 10 ms | Shortest, ms | 1st pct, ms | 32-block mean, ms | Added by a slow-down to 5 / 10 ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Roughing (rotary roughing), 3 projects | 16,466 to 69,125 | 0 | 0 | 0 | 0 | 0 / 0 | 36.0 | 84 to 100 | 232 to 241 | 0 / 0 |
| Helical Finish (glitchy) | 57,895 | 12 | 109 | 382 | 1,848 | 3 / 4 | 0.86 | 6.98 | 21.2 | 0.8 s / 5.5 s of 3,211 s |
| Helical Finishing (overcut) | 16,146 | 0 | 142 | 586 | 980 | 3 / 4 | 1.08 | 2.18 | 33.3 | 1.2 s / 5.6 s of 1,044 s |
| Rest machining (axial finishing, tapered ball r0.25) | 150,523 | 99 | 374 | 15,139 | 31,014 | 6 / 10 | 0.47 | 3.25 | 12.2 | 15.7 s / 147 s of 21,715 s |
| Operation 3 (pencil finishing, 2 mm ball) | 3,565 | 19 | 85 | 801 | 1,382 | 8 / 11 | 0.34 | 1.65 | 5.6 | 1.4 s / 7.1 s of 97 s |

- **Runs are short.** The longest run of consecutive blocks under 10 ms is 11 blocks, in pencil. That is a third of the Z1's 32-block queue (Makera Z1 firmware facts below). Under 5 ms the longest run is 8.
- **The stopping distance never binds.** Smoothie, with `minimum_planner_speed` 0, keeps the speed at which the machine could stop within the queued blocks. Over every 32-block window of every file, that limit stays above the programmed speed: the lowest ratio is 1.72, in pencil. The rest-machining file's lowest is 3.55 and the reference jobs' lowest is 6.9. With a full queue and an acceleration of 150 mm/s², the Z1's planner depth does not slow any of these programs.
- **The worst sustained block rate** is a mean of 5.6 ms per block over 32 blocks (pencil), and 12.2 ms on the rest-machining file. A stall from block supply happens only if the Z1 cannot accept a line every 5.6 ms (about 180 lines a second). Nobody has measured that figure (open item OI-930).

### Merge experiment

Effect of the merge, in counts of blocks under 5 ms or under 10 ms:

| Operation | Before: < 5 / < 10 ms | 0.002 mm, < 10 ms only | 0.005 mm, < 10 ms only | 0.01 mm, < 10 ms only | 0.005 mm, all blocks: block count, < 5 ms |
| --- | --- | --- | --- | --- | --- |
| Rest machining (axial) | 15,139 / 31,014 | 14,275 / 29,419 | 13,129 / 25,594 | 4,051 / 16,970 | 150,523 → 80,143; 12,617 |
| Helical Finish (glitchy) | 382 / 1,848 | 303 / 1,757 | 303 / 1,593 | 303 / 1,510 | 57,895 → 30,627; 22 |
| Helical Finishing (overcut) | 586 / 980 | n/a | 341 / 721 | 341 / 721 | 16,146 → 6,127; 113 |
| Pencil | 801 / 1,382 | n/a | 701 / 1,213 | 648 / 1,153 | 3,565 → 2,798; 637 |
| first_cut finish (helical) | 1 / 1 | 1 / 1 | 1 / 1 | 1 / 1 | 4,104 → 82; 1 |
| first_cut rough | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 | 12,296 → 1,234; 0 |

- **Within the tolerance budget, merging hardly helps.** Merging only the short blocks at 0.002 or 0.005 mm removes 6 to 13 % of them on the rest-machining file and 13 to 20 % on pencil. The budget leaves about 0.001 mm for a merge (below). Only 0.01 mm, the whole budget, removes most short blocks on rest machining (73 %), and even that removes just 19 % on pencil.
- **Merging long blocks is a different benefit.** Merging every block at 0.005 mm halves the rest-machining program and cuts the blisk helical program by 47 %. It cuts the first_cut helical finish by 98 %, from 4,104 to 82 blocks, because a helix at constant radius is a straight line in X and A. That reduces line traffic and file size, but on pencil and rest machining the short blocks mostly stay.

## Causes

- **Path fitting by bisection at high curvature: nearly all short blocks.**
  - In axial finishing, the steps halve from 0.25 to 0.125 to 0.0625 mm, an XZ length of 0.003 to 0.080 mm (median 0.064 mm). The surface curves sharply at the blade edges, where the 0.004 mm chord limit (0.4 times the tolerance, decision 0018 item 12) requires them.
  - In pencil, A steps of 1.406 degrees are split to 0.176 degrees (one eighth).
  - In the blisk helical passes, the short blocks have a median length of 0.018 mm and a median A step of 0.016 degrees.
  - These blocks follow the shape, so a collinear merge within a small tolerance cannot remove them.
- **Tip-speed compensation shortens them further.** On convex regions the tool tip travels farther than the contact point, so the post raises F. On the rest-machining file the contact feed is about 500 mm/min, but its short blocks run at 730 to 1,170 mm/min, so the same length takes less time. Pencil's short blocks run at about 1,190 mm/min, which is at the Z1's 1,200 mm/min limit.
- **Not A-axis stepping.** The `max_angle_step` of 5 degrees gives long blocks: the shortest helical block on a smooth part is 83 ms.
- **Not links, ramps, leads or plunges.** No `Linking`, `Plunge`, `LeadIn` or `LeadOut` segment is under 10 ms in any project. The cone_axial blocks at 9.49 ms are cutting moves.
- **Not simultaneous 4-axis passes as such.** The lobed simultaneous finish has none under 39 ms. The short blocks come from the surface, not the axis count.
- **Not output rounding.** There are no zero-length blocks. The shortest blocks are a few thousandths of a millimetre, or A-only steps of 0.016 degrees, both above the 0.001 resolution of three decimals. Rounding can shift such a block's time by a few per cent, which the F word absorbs.

## Controller facts (from source; not measured)

### Makera Z1 firmware, MakeraZ1Firmware

- **Planner queue.** It holds **32 blocks** (`planner_queue_size`, default 32, `src/modules/robot/Conveyor.cpp:75`; the Z1 config does not set it). The queue waits up to 100 ms to fill before it starts (`queue_delay_time_ms`, line 76).
- **Speed and acceleration settings** (`src/configZ1.default`):
  - `junction_deviation 0.01` (line 35);
  - `acceleration 150` mm/s² (line 445) and `delta_acceleration 360` deg/s² (line 446);
  - `minimum_planner_speed` 0 (`Planner.cpp:47`).
- **The firmware splits lines by length, not time.** A line is split into 5 mm XYZ pieces (`mm_per_line_segment` 5.0, `Robot.cpp:207, 1821`), with A spread over the pieces. A-only moves are not split.
- **Arcs cannot carry A.** Arcs are segmented at 0.002 mm (`Robot.cpp:209`), but "TODO we need to handle the ABC axis here" (`Robot.cpp:1957`): the firmware does not move A inside an arc. G2/G3 is usable only on moves with no A, and only in the G17, G18 or G19 plane.
- **No blending control.** There is no G61, G61.1 or G64 handler, and no other code that controls look-ahead.
- **How a program reaches the planner.** The motion processor does not read the file itself. The ESP32 board sends lines over a **230400-baud** serial link (`Kernel.cpp:147`; `Player.cpp:358-396, 785-805`). The firmware keeps a 224-line buffer that refills below 180 lines (`Player.cpp:67, 786`), and executes one line per pass of the main loop.
  - At about 30 bytes a line plus framing, the link alone allows at most about 600 to 700 lines a second, so the floor is about 1.5 ms per line.
  - The real rate depends on the ESP32 side, which is closed source, and nobody has measured it.
- **Lines over 63 characters are silently cut short** (`Player.h:24, 53-57`: `char data[64]`). Our longest Z1 line is 41 characters, in `lobed`. This is a separate hazard (OI-933). One issue on the firmware repository says the source may not match the shipped binary (medium confidence).
- **Makera publishes no guidance on stutter or segments.** Nothing was found in the wiki, the forum or the GitHub issues. Makera's own Fusion post, `Carvera.cps`, writes arcs (tolerance 0.002 mm, minimum chord 0.25 mm), uses `FEED_DPM` for multi-axis moves, and has no smoothing and no minimum-segment option.

### LinuxCNC 2.9

- **G61, which our verification post writes, is exact-path mode.** Moves slow or stop as needed to reach every programmed point, and the motion stops at every vertex that is not exactly collinear (`g-code.adoc:1704-1711`; `tp.c:899-904`).
- **G64 P- Q- blends.** It blends within P, and the Naive CAM Detector merges collinear moves within Q, for XYZ-only moves (`user-concepts.adoc:94-96`). It does not apply to moves with A.
- **Look-ahead.** The trajectory queue holds 2000 entries and the blend optimisation looks 50 segments ahead.
- **One block per servo period at least.** The servo period is 1 ms. A segment's speed is capped at `length / (1.02 x period)` (`tp.c:1708-1709`, `TP_MIN_SEGMENT_CYCLES`). LinuxCNC therefore needs blocks of at least about 1.02 ms to hold the programmed feed. In our files, 19 to 99 blocks per blisk operation are under 1 ms.

## Options

Each option's error, cost and fit:

1. **Report.** It adds no G-code change, no error and no cost to the toolpath. It counts the blocks under the limit, the longest run, the effective feed in that run and the worst 32-block mean. The figures come from the IR's durations in the neutral stage.
2. **Drop zero-length moves.** It adds no error. It would remove no block from any file measured here, so it is not worth a mode of its own. It is worth keeping as an invariant check: a zero-length fed block is a post bug.
3. **Collinear merge in joint space with a part-frame tolerance.**
   - **Error:** up to its tolerance, added to the 0.01 mm budget: 0.005 mm for tessellation and 0.005 mm for fitting, of which the fit already uses 0.004 mm (verification plan 4.1).
   - **Effect:** at the 0.001 mm the budget leaves, it removes few short blocks. At 0.01 mm it removes 73 % on the rest-machining file but exceeds the budget unless the user raises the tolerance.
   - **Cost:** it needs the independent checker to re-verify the merged path against the model.
   - **Determinism:** deterministic if greedy and sequential.
   - **Better use:** merging long, near-linear blocks (helical at constant radius: 98 % fewer blocks), which cuts line traffic.
4. **Arc fitting (G2/G3).** On the Z1 it applies only to moves with no A, because the firmware does not move A in an arc. That means axial passes (XZ, G18) and some links.
   - **Effect:** it would replace the bisection steps on the rest-machining file by arcs.
   - **Cost:** the gate-2 replay does not replay arcs yet (OI-68), and decision 0016 rules out G2/G3 with A. Both make it a larger piece of work.
   - **Error:** the arc fit's tolerance, from the same budget.
5. **Slow down (feed cap), so each block lasts at least the minimum.**
   - **Error:** none.
   - **Cost in time:** at 5 ms it adds 0.8 to 15.7 s per blisk operation, 0.03 to 1.4 % of its time. At 10 ms it adds 0.2 to 7 %. Pencil is the worst, at 7.1 s of 97 s.
   - **Cost at the cutter:** a lower feed in the capped blocks only, which are the sharp-curvature spots, where the feed was raised by tip compensation in the first place.
   - **Determinism:** a pure function of the durations.
   - **Gate 2:** its replay compares against the adjusted durations.
6. **Controller-side words.**
   - The Z1 has none: there is no G64 and no look-ahead word in the firmware.
   - For LinuxCNC, `G64 P<tol> Q<tol>` instead of `G61` would make a production LinuxCNC post blend. The verification post writes `G61` on purpose, so that gate 3 compares exact positions (decision 0020). A blending option belongs to a production LinuxCNC post, which is out of scope (OI-934).

## Conclusion

1. **None of the shipped reference jobs needs anything.** The one exception is a single 2.28 ms block in `first_cut`.
2. **Real parts with fine features do have short blocks.** On the blisk's rest machining, 10 % of blocks are under 5 ms, and on pencil 22 %.
3. **But they come in short runs.** The longest is 11 blocks, a third of the Z1's 32-block queue. The worst mean over 32 blocks is 5.6 ms. Whether the Z1 stalls on them depends on one unmeasured number: how many lines a second its ESP32 link and main loop deliver. A short attended air cut can measure it (decision 0222).
4. **The cheapest fallback is the slow-down.** It costs at most 1.4 % of an operation's time at 5 ms (7 % at 10 ms) and adds no error. A merge within the existing tolerance budget removes 6 to 20 % of the short blocks. A merge is worth having for line traffic, not as the fix for short blocks.
5. **Recommended order: Report first, then Slow down.** The slow-down is the fallback that does the work. Merge should be built third, if at all, as a separate block-count reduction with its own tolerance from the budget. This reverses the owner's provisional order of Merge before Slow down.
6. **The check should allow a window.** It should average the block time over the controller's queue depth, not test each block alone. A per-block limit of 5 ms would flag 15,139 blocks on the rest-machining file, and no 32-block window there averages below 12 ms.

## Air-cut program for the Z1 block rate (added 22 Oct 2026; decision 0222 item 7)

The owner decided to add this test to the gate 4 procedure (#284). `air-cut/gen_block_rate.py` writes `air-cut/block_rate_z1.nc`: 26,246 lines, the longest 50 characters.

- **Part A:** six sections of 100 mm of X at F1000, made of 20, 10, 5, 3, 2 and 1 ms blocks (300 to 6,000 lines each).
- **Part B:** the same six sections, with A advancing in every block.
- **The A step in part B.** Each block's A step is 60 % of its X length's worth under the Z1 rule (0.628 degrees per mm of X at Z50), so X still sets the block's time. The owner's first idea was a 0.2 degree step, but that cannot hold the short block times. The Z1's A axis is limited to 3600 degrees per minute, so a 0.2 degree step needs at least 3.3 ms. The rule's A term would also dominate: 0.19 mm at r = 50.
- **Check.** `gstat` on the file confirms the block times: 79 s of fed motion, 72 s of sections plus 12 markers.
- **Stopping-distance prediction.** Smoothie plans the newest block to end at rest. A 32-block queue at acceleration 150 mm/s² therefore allows at most `sqrt(2 a 32 L)`, where L is the block length:

  | Block | Length, mm | Stopping limit, mm/min | Predicted section time |
  | --- | --- | --- | --- |
  | 20 ms | 0.333 | 3,394 | 6.0 s |
  | 10 ms | 0.167 | 2,400 | 6.0 s |
  | 5 ms | 0.083 | 1,697 | 6.0 s |
  | 3 ms | 0.050 | 1,315 | 6.0 s |
  | 2 ms | 0.033 | 1,073 | 6.0 s |
  | 1 ms | 0.017 | 759 | 7.9 s |

  This is inference from the firmware source, not a measurement.
  - It means the 1 ms section runs at about 7.9 s even if lines arrive fast enough.
  - The same arithmetic gives a planner-depth floor of `t >= v / (2 a 32)` for any program: 1.74 ms per block at 1,000 mm/min, and 2.08 ms at the Z1's 1,200 mm/min.
