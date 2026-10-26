# Makera Z1 gates 4 and 5: step-by-step test instructions

**Status: draft; owner results submitted 25 Oct 2026, audited below (lane GATES).** Section 10 records passes, failures, waivers and missing evidence separately. Local preparation is complete for Merge Manager intake; the existing PR stays draft under decision 0055. Decisions 0221 and 0232 are accepted; decision 0222 is accepted on current main. None needs reacceptance.

**Generated G-code is the user's to verify before running on a machine (NFR-12).** OpenChuckCAM does not say that a program is safe or correct for any machine, tool or setup, and nothing in this document says otherwise. Passing gates 1 to 3, a clean simulation or a step of these instructions lowers the risk; it does not remove it. You, the operator, decide whether a program runs, and you can stop at any point.

## 1. What this document is for

Gates 4 and 5 are the two hands-on G-code gates of [verification plan section 6](verification-plan.md): gate 4, the **air cut** (the Z1 runs the program with no tool and no material, attended, at reduced feed), and gate 5, the **first cut** (soft, low-cost material, attended, then compared with the model). They are slice 20 item 11 of the [work breakdown](work-breakdown.md), a manual release-checklist step owned by the owner ([decision 0106](decisions/0106-air-cut-is-a-manual-release-step.md)), and the review checklist’s "Gates 1 to 4 have passed for the reference job set and gates 1 to 5 for the first-cut project, with results stored, and the G93 result is recorded" (accepted decision 0221).

This document turns them into steps that someone who is not a machinist can follow, each with what to do, what to expect, what to record and when to stop. It builds on, and does not replace, two documents the owner has already reviewed (14 Oct 2026, OI-133):

- the [safe-practice checklist](z1-safe-practice-checklist.md): complete its section 1 at the start of every session, section 2 before gate 4, section 3 before gate 5 and section 4 after every run;
- the [air-cut procedure](z1-air-cut-procedure.md): its steps 0 to 7 are the run order of gate 4 (section 4.4 below). Its step 3 is corrected to time a whole operation (OI-910, section 4.5).

Gate 4 runs the reference job set of [reference-jobs.md](reference-jobs.md) on the Makera Z1 post, and the first-cut project's program. Gate 5 cuts the first-cut project of section 5.2 (OI-914), not the reference job set (decision 0221).

**Step IDs.** Every step has an ID (P1 to P10, R1 to R12, S1 to S8, G4-0 to G4-8 and B1 to B14 for gate 4; F1 to F6, G0, W1 to W5, T0 to T5, V1 to V6, M1 to M6, C1 to C7, I1 to I5 for gate 5). Results are recorded against these IDs on the forms of section 8 and, once the owner has submitted them, folded into section 10 step by step, so each instruction can be read beside what happened when it was run.

| Rule ([verification plan section 6](verification-plan.md)) | Applies to |
| --- | --- |
| Attended for the whole run, emergency stop within reach | Gates 4 and 5 |
| No tool, no material, reduced feed | Gate 4 |
| Soft, low-cost material (wax or foam) | Gate 5 |
| Work offsets and stock position checked by hand | Gates 4 and 5 |
| Any unexpected motion stops the trial and is logged as a defect with the G-code attached | Gates 4 and 5 |
| A program moves to the next gate only when it has passed the one before; a change to a post or to the kinematics sends the affected programs back to gate 1 | Gates 4 and 5 |

## 2. Words used here

| Word | Meaning |
| --- | --- |
| A axis | The rotary axis. It turns the part about the machine's X axis. A+ turns the top of the chuck toward the front of the machine (-Y), confirmed on the machine (OI-03). A is continuous: it can turn many times. |
| Work zero, G54 | The point the program's coordinates are measured from. For the Z1 post it must be **Y0 Z0 on the A axis centreline** and X0 at the part's model origin (decision 0007, OI-40). |
| Machine coordinates, G53 | Coordinates measured from the machine's own home, not from the work zero. The program's last moves use them (`G53 G0 Z-3`). |
| Axis height | Where the A axis centreline is in machine Z. The profile holds an unconfirmed value (machine Z -55); it must be measured (OI-60). A wrong value moves every Z the software checks. |
| Safe Z | The height above the axis that rapid moves travel at. The Z1 profile's default is 50 mm above the axis. |
| Rapid (`G0`) | A move at the machine's fastest rate, not cutting. |
| Fed move (`G1`) | A cutting move at the feed in its `F` word. |
| Feed override | A control on the machine that scales the programmed feed, for example to 10 %. |
| Feed hold | Pauses motion; the program can continue afterwards. |
| Emergency stop | Stops everything at once. Use it whenever you are unsure. |
| DRO | The machine's position readout. |
| Unwind | The last A line of a long program, `G92.4 A0 S0`: the controller drops whole turns from its A reading **without moving** (decision 0007). |
| Held region | The length of stock the chuck grips (SET-09). The software keeps every cutting move clear of it (decision 0161). |
| Tab | A short length of stock left uncut to keep the part joined to the stock (OP-21). |
| Deviation | The simulation's comparison of the cut stock with the part: **material left** (positive) and **overcut** (negative, the tool went into the part). |
| Programmed time | The sum of the move times at their programmed feeds, with no acceleration, tool changes or fixturing (decision 0028 item 3). Each operation's comment line gives it as "about N s". |

## 3. What you need

- The release build of OpenChuckCAM (or the build named for the trial) on the computer you use to make the programs.
- A source checkout at the trial's commit, to write the reference job projects (P2), and the first-cut project (5.2).
- The setup sheet (HTML or PDF) written with each program (decision 0189), printed or on screen beside the machine.
- The Makera Z1 with its rotary module and chuck, and its controller software. Know where its emergency stop, feed hold and feed override are before you start.
- A stopwatch (a phone is fine), a camera (a phone is fine), masking tape and a marker.
- For gate 5: tooling board stock 35 x 35 x 100 mm, the tailstock, the 3.175 mm tools agreed with the owner (5.3), a caliper or micrometer, eye and ear protection, and dust extraction or a mask for tooling board dust.
- The results forms of section 8, printed or copied into a file.

## 4. Gate 4: the air cut

Gate 4 passes when, for each program run, the controller accepts every line, raises no alarm, the motion matches what the simulator showed, and the timed move agrees with the predicted time within the band set at the first run ([verification plan section 6](verification-plan.md)). It also records the `G93` result on the installed firmware.

### 4.1 Preparation

Do these at the computer and at the machine before anything moves. Each step has a pass condition. **If a step does not pass, stop and do not run any program** (section 7).

| Step | Do | Pass when | Record |
| --- | --- | --- | --- |
| P1 Software build | Open OpenChuckCAM, choose **Help > About OpenChuckCAM** and press **Copy build information**. Paste it into the results form (section 8.1). From a terminal, `openchuck-cli --version` gives the same text. | The version is the one named for the trial, the commit is shown, and **Uncommitted changes: no** (NFR-16). The About window also shows the notice that generated G-code must be verified by the user. | The pasted build information |
| P2 Project and programs | From a checkout at the commit in P1, write the reference job projects to an empty folder: `cargo run -p openchuck-cli --example sample_projects -- <folder>` (OI-913, OI-920: the project files are committed in `testdata/projects/`, and the generator writes each generated part beside them). Open each folder in the build of P1 and write its G-code and setup sheet with the Makera Z1 post. Compare each program with its golden file in `testdata/golden/reference/`: only the version and operation comment lines may differ. `drilling` and `swarf` are committed whole, with their STEP files (OI-922); opening them needs the build's STEP kernel. | You have the `.nc` file and setup sheet of each job you will run, from the build or commit in P1. | File names, the build or commit, the comparison |
| P3 Machine profile | In the application, open the project's setup and its machine values (or `openchuck-cli machine <project>`). Each value is marked confirmed, vendor-supplied or unconfirmed (UI-13). | The profile is **Makera Z1**. Travel 200 x 200 x 100 mm, feeds 1200 mm/min X and Y, 600 Z, 3600 deg/min A, A continuous and right-handed about +X. The axis height is the measured value from P8, not the placeholder (OI-60). | Each value, and which are unconfirmed |
| P4 Post | Check the post chosen for export. | **Makera Z1** (files end `.nc`), not the LinuxCNC verification post (`.ngc`). Feed mode: units per minute; the Z1 post never writes `G93`. | Post name |
| P5 Firmware | On the Z1 controller, find the firmware version. | Noted. If it is not 1.1.2, the SP-12 conventions (decision 0007) must be rechecked first: step 1 of section 4.4. | Firmware version |
| P6 Tool and stock | Take any tool out of the spindle. Empty the chuck, or close the jaws on nothing. Clear the bed. | **No tool, no material.** Nothing loose on the bed or in the enclosure. | Photo of the empty spindle and chuck |
| P7 Spindle and tool change | Every generated program has `M5`, `T1 M6` and `S... M3` near its start. Run them **as generated**: the spindle runs with no tool fitted (OI-911, the owner confirms the Z1 spins an empty collet safely) and `T1 M6` runs with no tool, watched, with a hand on the stop (OI-912). Do not edit the program. | The controller accepts `T1 M6` and `S... M3`; what it does on `M6` matches what the owner expects. | What the controller did on `M6`; the spindle ran: yes / no |
| P8 Axis height | Measure the machine Z of the A axis centreline (OI-60). Use **Makera's own work-zero routine** for Y and Z on the axis (OI-917), done before P6 if it needs a tool or probe fitted; the machine (G53) Z of the resulting G54 Z0 is the axis height. If it differs from machine Z -55, correct the project's machine profile and regenerate (P2). | A measured value is in the profile. | Measured axis height (mm), how it was measured |
| P9 Work zero | Set G54 with **Makera's own routine** (OI-917): Y0 Z0 on the A axis centreline, X0 at the part's model origin, at least 10 mm clear of the jaws (OI-40), and A0. This document does not copy the routine's steps: follow Makera's documentation for the installed firmware. | The DRO in G54 reads Y0 Z0 at the axis centre and X0 where the setup sheet's "Work offset" line says. | G54 X, Y, Z in machine coordinates; which face X0 was taken from; how A0 was set |
| P10 Clearances | With nothing fitted, jog Z to the program's safe Z (Z50 in G54 for the reference jobs) and jog X over the program's whole X range (read it from the program: the smallest and largest X words). Turn A by hand or by jog a full turn. | The spindle nose clears the chuck, jaws and rotary module everywhere along that X range, and safe Z is below the top of Z travel (with the profile's figures, safe Z 50 is machine Z -5, 5 mm below the top). | Lowest clearance seen |

### 4.2 Read the program before you run it

Open the `.nc` file in a text editor. You are checking that the program says what you expect. You do not need to understand every number; you do need to understand every **kind** of line. Here is the start and end of `sphere_makera-z1.nc` with what each line means:

```text
(sphere - Setup 1)                      the project and setup the program is for
(OpenChuckCAM <version>)                the software that wrote it; matches P1
G90 G21 G17                             absolute coordinates, millimetres, XY plane
G54                                     use work zero G54 (P9)
(operation Finish, tool T1 3 mm ball, about 76 s)
                                        operation, tool number and name, programmed time
M5                                      spindle stop (before the tool change)
T1 M6                                   tool change to tool 1 (P7, OI-912)
S12000 M3                               spindle on, clockwise, 12000 rpm (P7, OI-911)
G0 X-10.000 Y0.000 Z50.000 A0.000       rapid to the start, at safe Z 50 above the axis
Z12.000                                 rapid down to 12 mm above the axis (still G0)
G1 Z4.177 F150.0                        fed plunge at 150 mm/min (the plunge feed)
X-9.972 Z4.227 A5.000 F565.6            fed moves: X, Z and A together, new F each time
...
G0 Z50.000                              rapid back up to safe Z
G92.4 A0 S0                             the unwind: A reading reset, no motion
M5                                      spindle stop
G53 G0 Z-3                              rapid to machine Z -3 (near the top of travel)
G28                                     return to home (not yet seen on the machine: OI-41)
M2                                      end of program
```

(In the golden files the version and the operation comment read `<version>` and `(operation <id>)`; the programs the application writes give the real values.)

Check each item and tick it on the form (section 8.1):

| # | Look for | Expect | Stop if |
| --- | --- | --- | --- |
| R1 | The first two comment lines | The project and setup you meant; the version from P1 | A different project, setup or version |
| R2 | `G90 G21 G17` then `G54` | Exactly these, before any motion | Missing, or `G20` (inches) or another work offset |
| R3 | No `G93` anywhere | None: the Z1 ignores it (decision 0007) | Any `G93` |
| R4 | Each `(operation ...)` comment | The operations you expect, in order, each with its tool and "about N s" | An operation you did not expect, or one missing |
| R5 | Tool changes `M5` then `T<n> M6` | One before each change of tool; the number matches the setup sheet's Tools table (GC-14) | A tool number you did not confirm, or `M6` without `M5` before it |
| R6 | Spindle `S<rpm> M3` | After each tool change; the speed matches the setup sheet's **Spindle rpm**, and its **Speed from** column says where it came from: "operation" (set on the operation) or "tool data: <material>" (the tool's cutting data for the stock material) (decision 0196, OI-755). Within the Z1's spindle range (the manufacturer's figure) | A speed of 0 or none, a speed outside the spindle's range, or a source you did not expect |
| R7 | The first rapid `G0 X.. Y.. Z.. A..` | At safe Z (Z50 for the reference jobs), Y0 | A first rapid below safe Z |
| R8 | Every other `G0` | Either at safe Z, or the rapid down from safe Z to just above the stock (for the sphere, Z12: the stock radius is 11 mm) | A rapid at a Z inside the stock radius (setup sheet's Stock line, diameter / 2) |
| R9 | The first `G1` after each rapid down | A plunge with the plunge feed (150 for the reference jobs) | A plunge with a large F |
| R10 | `F` words | Present on the first `G1` and whenever the feed changes. On the Z1 the F of a move that turns A is **not** the cutting feed: the post chooses it so the Z1's feed rule gives the right speed at the contact (decision 0007), so it varies line to line (565.6, 568.6, ...). The reference jobs' largest F is 1190.6 | An F above 1200 (the X and Y limit) on a move with X, Y or Z: see OI-267 for A-only moves |
| R11 | A words | A grows steadily through a helical pass (the groove reaches A5400, 15 turns) and the program ends with `G92.4 A0 S0` after a retract to safe Z | A jump of many turns in one line, or an unwind before the tool is at safe Z |
| R12 | The end block | `M5`, `G53 G0 Z-3`, `G28`, `M2`, in that order | Anything else, or a missing `M5` |

Then read the **setup sheet** for the same run (decision 0189 writes it with the G-code, so they describe the same run):

| # | Look for | Expect |
| --- | --- | --- |
| S1 | Traceability block | Generated time (UTC), project last saved, **Software version** matching P1, the project fingerprint (decision 0207). Copy the fingerprint to the form. |
| S2 | Job: machine | Makera Z1 |
| S3 | Setup: work offset | "G54, work zero at model X ... mm on the rotary axis": this is the X0 for P9 |
| S4 | Setup: stock and material | The stock shape, size and X range; the material name |
| S5 | Tools | Each tool's number, type, diameter, flute and overall length |
| S6 | Operations | Spindle rpm and Speed from (R6); Time per operation matches the "about N s" comments (R4) |
| S7 | Notes | The statement that generated G-code must be verified by the user before it is run |
| S8 | Setup: Held at the start, Held at the end, Tabs, and the table "Tabs: <setup>" | The held regions (length, X range, section or holding diameter) and each tab (X from and to, shape, diameter, layout, placed by hand or automatically) as you expect (OI-918) |

### 4.3 Safeguards for every run

**Reported controller discrepancy (25 Oct 2026):** the owner says the minimum available override was 50%, and used 50% then 100%. The accepted checklist and OI-919 choice below still require 10%. The submitted outcome does not authorize a standing change to that policy. If the controller cannot meet the instruction, stop for the owner’s disposition (OI-471); the actual trial settings remain in section 10.

- Feed override at **10 %** for the first run of each program (air-cut procedure).
- Your hand near the feed hold and the emergency stop for at least the first 20 lines, and in reach for the rest.
- No single block (OI-919, the owner's answer): the first moves run at 10 % with your hand on feed hold.
- The enclosure closed. Watch through it; do not reach in while anything can move.
- A strip of tape on one jaw so you can see A turn.
- Film each run if you can (the first 20 lines and the end block at least).

### 4.4 Run order

Follow the [air-cut procedure](z1-air-cut-procedure.md) steps 0 to 7 in order, with these additions (step G4-n here is the procedure's step n):

| Step | Program | What to watch | Pass when |
| --- | --- | --- | --- |
| G4-0 | Read `sphere_makera-z1.nc` (section 4.2) | | Every R and S item ticked |
| G4-1 | SP-12 conventions program (`spikes/sp12/AIR-CUT.md` on `spike/sp-12-z1-conventions`), only if the firmware is not 1.1.2 | Direction, wrap, feed rule as decision 0007 records | The results match decision 0007 |
| G4-2 | `sphere_makera-z1.nc` at 10 % | The first rapid goes to safe Z before any other motion. X, Z and A move together during the pass. The tape turns the way the A words say (A+ turns the top toward the front). The end block: up to Z near the top, then `G28` (watch where it goes and write it down: not yet seen, OI-41) | No alarm, no line refused, the motion looks like the simulation's playback of the same program, the machine ends where the program says |
| G4-3 | Timed operation on the sphere (section 4.5) | Stopwatch from the start of the plunge to the end of the last fed move | Measured time within the band agreed at this first run of predicted time = "about N s" x 100 / override |
| G4-4 | `cone_makera-z1.nc` at 10 % | Z changes steadily with X | As G4-2 |
| G4-5 | `groove_makera-z1.nc` at 10 % | About 15 turns of A, then the unwind. **At `G92.4 A0 S0` A must not move**; the DRO's A reading changes to the remainder. Note the DRO before and after | As G4-2, and A does not move at the unwind |
| G4-6 | `G93` recheck (SP-12 test T5) only if the firmware is not 1.1.2 | | The result is recorded (verification plan section 6, feed mode) |
| G4-6b | OI-267: an A-only fed move with an F above 1200 | The test program is not written yet; the owner asked for the test on an air cut before the first release (17 Oct 2026). Run it only once it is written and read like any other program | The controller's behaviour (clamps, refuses or runs it) is recorded in OI-267 |
| G4-7 | Raise the override (25 %, 50 %, 100 %) on the sphere only | Only if steps 2 to 5 had no surprise and you agree | As G4-2 |
| G4-8 | The block-rate air cut (section 4.8; decision 0222, accepted; OI-930) | Fourteen sections of 100 mm of X, each started by a 5 mm Z bob | Each section as in 4.8; **run only after K1 to K4 have passed** (decision 0232) |

The other reference programs (`lobed`, `cone_axial`, `cone_indexed`, `cone_axial_lead`, `cone_held_tabs`, `drilling`, `swarf`) and the first-cut project's program (5.2) are run the same way as G4-2 once G4-2 to G4-5 have passed. `lobed` is the one program that moves X, Y, Z and A together; `drilling` turns A to -30 and has no unwind; `swarf` turns to three A positions.

### 4.5 The timed operation (OI-910)

The verification plan asks for a simultaneous XYZA move whose measured time is compared with the predicted time under the Z1's feed rule. No single move in the reference programs is long enough to time by hand: computed from the golden programs with decision 0007's rule, the longest move that turns A while X, Y or Z also move takes about 0.13 s at 100 % feed (`groove`), 1.3 s at 10 %. The program header states no predicted total, and `openchuck-cli gcode-sim` gives no time per Z1 move.

So, as the owner decided (OI-910, option A), **time a whole operation**, which is a long run of such moves:

1. Find the operation's comment line, `(operation <name>, tool T<n> <tool>, about N s)`. N is the programmed time of its fed moves at 100 % feed.
2. Predicted time = N x 100 / override. At 10 % that is ten times N: about 760 s (12.5 minutes) for the sphere's N of about 76.
3. Start the stopwatch when the plunge (the first `G1` after the rapid down) starts, and stop it when the last fed move ends (the `G0` up to safe Z starts).
4. Ratio = measured / predicted. At this first run, agree the band with the owner (the SP-12 run confirmed the rule within about 10 %), write it on the form, and use it for every later run.

Rapids are not in N, and the feed override does not scale them, so they are not timed.

### 4.6 Gate 4 pass and fail

- **Pass** for a program: every preparation step passed; every R and S item ticked; no alarm; no line refused; the motion matched the simulation; A did not move at the unwind; the end position matched the program; the timed operation was within the agreed band (G4-3).
- **Gate 4 passes** when every program of the reference job set chosen for the release, and the first-cut project's program, has passed, the `G93` result on the installed firmware is recorded, and the block-rate air cut (4.8) has been run and recorded.
- **Fail**: any unexpected motion, alarm, refused line, or a step that does not pass. Follow section 7. Do not go on to gate 5 with that program. A change to the profile, the post or the kinematics sends the affected programs back to gate 1.

### 4.7 What to keep from gate 4

- The filled results form (section 8.1), one row per run.
- The build information (P1), the firmware version and the axis height measurement.
- The exact `.nc` files run and their setup sheets.
- Photos: the empty spindle and chuck (P6), the DRO at work zero (P9), the DRO before and after each unwind.
- Videos of the runs, if taken.
- Any defect raised (section 7), by issue number.

Attach them to one GitHub issue for the gate 4 result (section 7.2), so the verification report (slice 20 item 13) can cite it.

### 4.8 The block-rate air cut (decision 0222, OI-930)

The owner added this measurement to gate 4 (22 Oct 2026, relayed by the coordinator). It is lane SHORTMOVE's [decision 0222](https://github.com/paulhampson/OpenChuckCAM/pull/315) item 7, which is **proposed**: the specification below is copied from it, not invented here. It measures how short a block the Z1 can run at full speed. Its result sets the Z1 profile's `min_block_ms`, with a block window of 32 blocks; **the Z1 profile ships with the minimum-block-time check off until this has been run** (OI-930).

**Check it first, as SP-12 was (OI-925, the owner's option A; [decision 0232](decisions/0232-block-rate-program-checked-as-sp-12-was.md), accepted).** The program is written by a script on the spike branch, not by the application's post, so gates 1 to 3 have not checked it. Verification plan policy 6 excepts only the SP-12 conventions program; decision 0232 extends that exception to this program, on the checks K1 to K4 below. Do not run it on the Z1 until K1 to K4 have passed.

| Step | Do | Pass when |
| --- | --- | --- |
| K1 Fetch | From a checkout: `git fetch origin spike/sp-14-short-moves`, then `git show 69d532f:spikes/sp14-short-moves/air-cut/block_rate_z1.nc > block_rate_z1.nc`. | The file has 27,966 lines (`wc -l`) and its first line is `(SP-14 Z1 block-rate air cut - decision 0222)` |
| K2 Read the header and footer | Lines 1 to 5 and the last 5 lines. | Header: the two comment lines, `G90 G21 G17`, `G54`, `G0 X0.000 Y0.000 Z50.000 A0.000`. Footer: `G0 Z50.000`, `G92.4 A0 S0`, `G53 G0 Z-3`, `G28`, `M2`. No `T`, `M6`, `S`, `M3`, `G93` or `G4` anywhere (`grep -nE 'T[0-9]\|M6\|S[0-9]\|M3\|G93\|G4' block_rate_z1.nc` finds only the `S0` of `G92.4 A0 S0`) |
| K3 Read one section of each part | Section 1 (part A, from the line `(section 1: ...`) and section 8 (part B, `(section 8: ...`): its marker and its first and last ten lines. | Each section starts `G1 Z55.000 F1000.0`, `G1 Z50.000 F1000.0`; every other line is `G1` with X, Y0.000, Z50.000, A and F1000.0. X steps by the block length (0.333 mm in section 1) and stays within 0 to 100; Y stays 0 and Z 50. In part A A stays at its value; in section 8 A grows by about 0.209 degrees a block (0.628 per mm) |
| K4 Gate 3 on a LinuxCNC copy | Make the copy without the Z1 footer: `sed -e '/^G92.4 A0 S0$/,$d' block_rate_z1.nc > block_rate_linuxcnc.ngc` and `printf 'M5\nM30\n' >> block_rate_linuxcnc.ngc`. Build the LinuxCNC image once (`docker build -t openchuck-linuxcnc docker/linuxcnc`), then `cargo xtask replay --docker openchuck-linuxcnc block_rate_linuxcnc.ngc` (about 2 minutes, in real time; see `tools/linuxcnc/README.md`). | The replay reports every move reached and no fault, and the program ends where the replay says, as for the golden programs |

**The program.**
- `spikes/sp14-short-moves/air-cut/block_rate_z1.nc` on the branch `spike/sp-14-short-moves` (commit `69d532f`; a spike branch is never merged), written by `python3 -I gen_block_rate.py > block_rate_z1.nc` beside it. It is 1,170,259 bytes, over the 1 MiB a file of `testdata/` may hold (decision 0004), so it is not copied here.
- 27,966 lines; the longest is 50 characters, under the firmware's 63-character limit (OI-933).
- Header `G90 G21 G17`, `G54`, then `G0 X0 Y0 Z50 A0`. Footer `G0 Z50`, `G92.4 A0 S0`, `G53 G0 Z-3`, `G28`, `M2`, as the Z1 post writes it (decision 0007). No tool change and no spindle.

**The sections.**
- Fourteen sections, each 100 mm of X at F1000 (6.0 s nominal), made of equal collinear `G1` blocks of one block time: 20, 10, 7, 5, 3, 2 and 1 ms (blocks of 0.333, 0.167, 0.117, 0.083, 0.050, 0.033 and 0.017 mm; 300 to 6,000 lines a section).
- **Part A**, sections 1 to 7: X only. **Part B**, sections 8 to 14: the same, with A advancing 0.628 degrees per mm of X in every block (at Z50), so X still sets each block's time.
- Every section is longer than the 224-line receive buffer plus the 32-block queue. Sections alternate direction (X 0 to 100, then back) to stay within 100 mm of X.
- Each section starts after a marker: `G1 Z55 F1000`, then `G1 Z50 F1000`, a visible 5 mm bob of 0.6 s. There is no `G4` dwell (OI-937).

**Predicted section times** (inference from the firmware source, decision 0222): 6.0 s for 20, 10, 7, 5, 3 and 2 ms; **7.9 s for 1 ms** (the 32-block queue limits the speed of very short blocks). Starting and stopping add about 0.1 s.

**Setup and safety.** Gate 4 rules: no tool, no material, attended, emergency stop in reach. The gate 4 work offset (G54, Y0 Z0 on the A axis), with X0 placed so that X 0 to 100 at Z50 is clear travel. About 2 minutes of motion (92 s fed).

| Step | Do | Pass when |
| --- | --- | --- |
| B0 | Record the firmware version, the controller (ESP32) version, and how the file reached the machine (WiFi upload, USB); play it from the machine. Start a phone video with sound. | Recorded |
| B1 to B14 | For each section: start the stopwatch as Z comes back down to 50 after the bob, stop it as the next bob starts. Listen for stutter, watch for a pause. | Within 5 % of the predicted time (6.0 s, or 7.9 s for the 1 ms sections B7 and B14) and no audible stutter or pause |
| B15 | If B1 or B2 (20 ms) or B8 or B9 (10 ms) fails | **Stop the test and log a defect** (section 7): nothing in the analysis predicts it |
| B16 | Work out the result | The shortest block time at which **both parts pass** and every longer block time passes. If every section passes, the result is 1 ms. A 1 ms section near 7.9 s confirms the prediction; one near 6.0 s means the firmware plans differently from its source. Record the result for the Z1 profile's `min_block_ms` (OI-930) |

Section numbers: Part A B1 = 20 ms, B2 = 10, B3 = 7, B4 = 5, B5 = 3, B6 = 2, B7 = 1; Part B B8 = 20 ms, B9 = 10, B10 = 7, B11 = 5, B12 = 3, B13 = 2, B14 = 1.

## 5. Gate 5: the first cut

Gate 5 cuts soft, low-cost material, attended, and compares the result with the model. It passes when the part matches the model within the first-cut limit of [verification plan 4.1](verification-plan.md), **±0.1 mm**, and the findings are recorded. The owner chose (22 Oct 2026) to skip the 3-axis repeatability part and keep ±0.1 mm, and to cut a new first-cut project in tooling board rather than the reference job set. Both change slice 20 item 11, so they are in [decision 0221](decisions/0221-gate-5-first-cut-project-and-limit.md) (accepted by the owner, 22 Oct 2026); the verification plan, the work breakdown and the safe-practice checklist are edited to match.

### 5.1 Before gate 5

| Step | Do | Pass when |
| --- | --- | --- |
| F1 | Gate 4 for this program | The first-cut project's own Z1 program (5.2) has passed gate 4 (air cut, no tool, no material, every step of section 4), at the same commit, with the same profile and post. Any change since: back to gate 1. |
| F2 | Safe-practice checklist section 3 | Every box ticked. Its first-cut limit line reads ±0.1 mm (decision 0221). |
| F3 | Tool change test with a real tool (OI-41) | The tool change (`M5`, `T<n> M6`) has been run attended with a real tool and its length measurement watched. Gate 4 ran `T1 M6` with no tool (OI-912), which shows the controller accepts the line but not what it does with a tool fitted, so this test is still needed. |
| F4 | First-cut limit | ±0.1 mm on every measurement of 5.7 (OI-916, decision 0221). |
| F5 | The first-cut project | The project of 5.2 exists, its geometry has been confirmed with the owner (5.2, item G0), and it passes gates 1 to 3 like any other program. |
| F6 | The tools confirmed with the owner | **Stop here and confirm the tools with the owner** once the project is generated (5.3, T0). Do not go on until they have agreed them. |

### 5.2 The first-cut project

The owner's specification (22 Oct 2026, OI-914):

| Item | Value |
| --- | --- |
| Stock | Box, 35 x 35 x 100 mm, tooling board |
| Where the job sits | At the **end** of the stock (the tailstock end), not over the whole length |
| Part | 25 mm long |
| Tab | 5 mm long, joining the part to the rest of the stock |
| Tailstock tab | Very thin: 2 mm long, 10 mm diameter, at the tailstock end, to give the tailstock clearance |
| Workholding | Chuck at one end, **tailstock** at the other |
| Tools | Only the Z1 library's 3.175 mm single-flute or ball-nose tools (5.3) |
| Material | "Plastic" (OI-915, 5.3) |

**G0. The geometry, as the owner answered it (22 Oct 2026, relayed by the coordinator) and as this lane read it.** The project is committed in `testdata/projects/first_cut/`, built by `apps/openchuck-cli/tests/e2e.rs` (`build_first_cut`), and written out with its part by `cargo run -p openchuck-cli --example sample_projects -- <folder> first_cut`. Its programs are `testdata/golden/first_cut/`, checked by gates 1 and 2 in the end-to-end suite and replayed by gate 3 (`cargo xtask replay`).

| From X 0 (the head end, the chuck) | What | Source |
| --- | --- | --- |
| X 0 | Work zero X0: the head end of the stock, on the axis | Owner |
| X 0 to 10 | Gripped by the chuck jaws (at most 10 mm) | Owner |
| X 10 to 68 | Free stock, **left uncut** | Owner ("do not use the whole length"); X positions the coordinator's proposed layout |
| X 0 to 68 | **One held region at the start, 68 mm, box section** (no holding diameter, decision 0160): the 10 mm in the jaws plus the 58 mm left uncut, so the setup sheet reads "Held at the start: 68 mm" though the chuck grips 10 mm. Nothing else in the software keeps a tool off a length of stock (OP-22 is not built, OI-923). Accepted by the owner (22 Oct 2026) | This lane's reading, accepted |
| X 68 to 73 | Tab, cylinder 10 mm diameter, 5 mm long, on the axis | Owner |
| X 73 to 98 | Part: a cone 25 mm long, **20 mm diameter at X 73 (the head side), 10 mm at X 98 (the tailstock side)**, so its small end meets the 10 mm tailstock piece flush and the tailstock side has no shoulder | Owner: a cone 25 mm long, 20 mm largest; the small diameter and the direction are this lane's reading |
| X 98 to 100 | Tailstock piece, 10 mm diameter, 2 mm long: **a held region at the end** (the owner's answer to OI-924, option A), so every cutting move keeps clear of it and of the tailstock beyond it (decision 0161): the fed moves stay between X 70.1 and 95.9, and the cone's last 2 mm (X 95.9 to 98) are left unfinished. As a tab (rejected), the program cut past the stock's end: roughing 544 fed moves to X 101.02, the finishing ball's edge to about X 101.6 | Owner (a 10 mm, 2 mm piece; OI-924 A) |
| X 100 | End of the stock; the tailstock centre | Owner |

Stock box 35 x 35 x 100 mm, X 0 to 100, Y and Z -17.5 to 17.5; material "Plastic"; roughing (rotary roughing) with tool 104, finishing (helical, 5 degree steps) with tool 204, both with the Plastic presets (OI-921). The program numbers them **T1 (104, roughing) and T2 (204, finishing)**, as the owner decided (22 Oct 2026): the Z1 profile carries no tool set, so fit 104 as T1 and 204 as T2.

**Still for the owner to confirm:** the cone's direction, its 10 mm small end on the tailstock side (in the draft pull request's review). The 68 mm held region, the tailstock piece as a held region and the tool numbers are confirmed (22 Oct 2026).

Read them on the setup sheet (its Setup section's "Held at the start", "Held at the end" and "Tabs" lines and the "Tabs: <setup>" table, OI-918) and check them against the application's **Workholding and tabs** section (specification section 11.7). Write them on the form.

| Step | Check | Pass when |
| --- | --- | --- |
| W1 | Held region at the start: 68 mm, box section | The chuck grips 10 mm or less of it (measure the jaw depth); the rest, X 10 to 68, is the free stock left uncut (G0). |
| W2 | The grip warning | No amber warning under **Grip length**. If there is one, it names how much of the part the held region covers (OI-554): that length of the part will **not** be machined. With the job at the far end of a 100 mm stock there should be none. |
| W3 | Tab and the tailstock piece | The 10 mm tab at X 68 to 73; "Held at the end" reads "2 mm of the stock, X 98 to 100 mm, at diameter 10 mm" (OI-924). A tab with no room shows a warning (decision 0202). |
| W4 | Setup checks and messages | No error. Read every warning and write it down. "Safe Z cannot be checked against the Z travel" means the axis height is still the placeholder (OI-60): go back to gate 4 P8. |
| W5 | Stock and part | Box 35 x 35 x 100 mm from X 0; the setup sheet's Part offset reads "part moved 73 mm", so the cone spans X 73 to 98. |

### 5.3 Tools, material and speeds

The Z1 tool library (built-in defaults, `crates/openchuck-project/defaults/z1/`) holds five 3.175 mm tools. Every one has cutting data for **"Plastic"** and **"Softwood"** (checked on `main`, 22 Oct 2026; 32 of the library's 54 tools have Plastic data; material names match ignoring case, decision 0197):

| No. | Tool | Type, flutes | Flute length | "Plastic" preset: rpm, mm per tooth (feed), plunge, stepover, stepdown |
| --- | --- | --- | --- | --- |
| 101 | Z1 O-flute metal 3.175x12 | Flat end, 1 flute | 12 mm | 10000, 0.1 (1000 mm/min), 300 mm/min, 2.2225 mm, 1.0 mm |
| 104 | Z1 O-flute 3.175x25 | Flat end, 1 flute | 25 mm | 10000, 0.1 (1000 mm/min), 300 mm/min, 2.2225 mm, 1.0 mm |
| 105 | Z1 O-flute 3.175x42 | Flat end, 1 flute | 42 mm | 10000, 0.1 (1000 mm/min), 300 mm/min, 2.2225 mm, 1.0 mm |
| 201 | Z1 ball metal 3.175x10 | Ball end, 2 flutes | 10 mm | 12000, 0.0208 (500 mm/min), 200 mm/min, 0.4536 mm, 1.0 mm |
| 204 | Z1 ball 3.175x22 | Ball end, 2 flutes | 22 mm | 12000, 0.0208 (500 mm/min), 200 mm/min, 0.4536 mm, 1.0 mm |

The library has no single-flute ball-nose tool at 3.175 mm: the ball-nose tools are two-flute.

**The owner's choice (OI-921, 22 Oct 2026): rough with 104 (Z1 O-flute 3.175x25, single flute) and finish with 204 (Z1 ball 3.175x22, two flutes), both with the "Plastic" presets.** Write on the form that the finishing tool is two-flute. The "Softwood" presets of these tools differ from "Plastic" only in the stepdown (2.0 mm).

| Step | Check | Pass when |
| --- | --- | --- |
| T0 | **Confirm the tools with the owner** (the owner's instruction). When the project is generated, show the owner the setup sheet's Tools and Operations tables (number, name, type, diameter, flute length, spindle rpm, Speed from) | Roughing uses 104 and finishing 204, as the owner chose (OI-921), and the owner has agreed the generated project. Record who agreed and when. |
| T1 | Each operation's tool | One of the five tools above, and the tool number matches the program's `T<n>` (GC-14). |
| T2 | Material | The setup's material is **Plastic**, so the operations take the Plastic presets: the setup sheet's **Speed from** reads "tool data: Plastic" (decision 0196, OI-755). If an operation sets its own speed, Speed from reads "operation": check the value with the owner. |
| T3 | A tool with no Plastic data | None of the five lacks it. If a tool without Plastic data is ever used: copy its Softwood data to a new Plastic entry in the tool library; if it has no Softwood data either, **ask the owner before** using settings from a similar tool (OI-915). |
| T4 | Feeds | The program's F words are within the profile's limits (R10); the plunge F matches the preset. |
| T5 | Stick-out | The tool sticks out of the holder at least as far as the deepest cut plus a margin, and the flute length covers the depth from the stock's surface to the part (the box's corners are 24.7 mm from the axis). The Z1 measures tool length itself (decision 0007: no `G43`); follow the machine's own procedure. |

### 5.4 Simulate before cutting

| Step | Do | Pass when |
| --- | --- | --- |
| V1 | Run **Simulate** in the application (or `openchuck-cli simulate <project>`), at the default cell (0.1 mm) or finer | It completes. No banner saying the result is out of date (decision 0192). |
| V2 | Overcut | The results inspector shows no overcut beyond the tolerance, and the command line reports no "OVERCUT" and no rapid move that removes material. |
| V3 | Material left | Write down the largest material left and where it is (the probe and "Go to largest left" show it). Material left on a held region or tab face is not counted; a cut **into** one is (decision 0194). |
| V4 | Held region and tabs | They stay whole in the simulated stock (SET-09, OP-21); the 2 mm tailstock tab is not cut through. |
| V5 | Playback | Play the program through. No fed move goes below X 70.1 or above X 95.9: the tool stays clear of the held regions at both ends (decision 0161, OI-924). |
| V6 | Keep | A screenshot of the coloured part, the histogram and the worst values, for the comparison in 5.7. |

The simulation is a check on the program, not a promise about the machine (NFR-12): it does not model the bodies of the chuck, the jaws and the tailstock (the held regions stand for them, SET-09), or the machine's own errors.

### 5.5 On the machine: stock and offsets

| Step | Do | Pass when |
| --- | --- | --- |
| M1 | Measure the stock (35 x 35 x 100 mm) | It matches the project's stock within what you consider safe; never larger. |
| M2 | Load the stock in the chuck with the gripped length the project says (W1), and bring up the tailstock; tighten both | It does not move when you try to turn or pull it by hand. Turn A by jog a full turn at low feed: it turns true by eye and nothing touches. **Tailstock clearance (OI-924): jog the tool, in the air, to X 95.9 at the radius of the cone's tail end and up to safe Z, and check that the tailstock's body clears the tool and the holder at both.** The held region at the end keeps every cutting move clear of the tailstock end (SET-09, decision 0161); the tailstock's body itself is checked here, by eye. |
| M3 | Fit the tool (T1, T5) | The tool number in the controller is the program's `T<n>`. |
| M4 | Set the work zero with **Makera's own routine** (OI-917): Y and Z on the A axis, X0, and A0 | G54 Y0 Z0 on the axis centreline, X0 where the setup sheet's work offset line says, A0 as the routine sets it. Record on the form: which face X0 was taken from, how A0 was set, and G54 X, Y, Z in machine coordinates. The machine Z of G54 is the axis height (OI-60): compare it with the profile. |
| M5 | Check by hand that the first rapid position is clear: jog to the program's first `G0` position in G54 (X, Y0, safe Z, A0) at low feed | The tool is above the stock at the X the program starts at, and clears the jaws and the tailstock body. |
| M6 | Protection | Eye and ear protection on; loose clothing, hair and jewellery secured; dust extraction or a mask for tooling board dust. |

### 5.6 The first cut

- **C1.** Reread the program (section 4.2) one more time. It must be the file you air-cut for F1, regenerated only if you changed the tool, speeds or stock, in which case read it again from the start and air-cut it again.
- **C2.** Feed override at 10 %. Hand near feed hold and emergency stop.
- **C3.** Start. Watch the tool change and the spindle start.
- **C4.** Watch the first plunge: the tool comes down at the plunge feed to the first cut, no faster.
- **C5.** Raise the override slowly, only while the cut sounds and looks steady.
- **C6.** Stop at any unexpected sound, vibration, smell, smoke, or motion (section 7): for tooling board, tearing, the stock moving, or the tailstock tab breaking.
- **C7.** At the end, wait until everything has stopped before opening the enclosure.

### 5.7 Inspect against the model and the simulation

| Step | Do | Pass when |
| --- | --- | --- |
| I1 | Look at the part before you remove it | Both tabs whole, no marks on the gripped length (decision 0161), no gouges. Photograph it from four sides (A 0, 90, 180, 270). |
| I2 | Measure the part's diameters (or widths) at stations along X: at least 5 over its 25 mm, away from the tabs | Each is within ±0.1 mm of the model at that X. |
| I3 | Length and positions | The part's length (25 mm) and the tabs' positions and diameters within ±0.1 mm. |
| I4 | Compare with the simulation (V3, V6) | Where the simulation showed material left, the part has it too, about as much; where it showed none, there is none. A difference larger than 0.1 mm is a finding. |
| I5 | Surface | Write down anything that looks wrong: steps, chatter, a seam where A wraps. |

### 5.8 Gate 5 pass and fail

- **Pass**: every check in 5.1 to 5.5 passed; the cut ran with no unexpected event; I1 to I4 within ±0.1 mm; the findings recorded.
- **Fail**: any step above, or any measurement outside the limit. Follow section 7. A machine-side cause (stock slipped, wrong offset) is recorded and the cut repeated; a cause in the program is a defect.

### 5.9 What to keep from gate 5

- The filled results form (section 8.2).
- The gate 4 issue number for the first-cut program.
- The record of G0 (the geometry agreed) and T0 (the tools agreed).
- The program, setup sheet and simulation screenshots (V6).
- Photos of the setup in the machine (stock in the chuck, tailstock, tool), and the finished part from four sides.
- The measurements and the instrument used.

## 6. One-page checklists

### 6.1 Gate 4 (air cut)

- [ ] Safe-practice checklist sections 1 and 2 complete.
- [ ] Owner's answers followed: OI-910 (time a whole operation), OI-911 (spindle runs, no tool), OI-912 (`T1 M6` as generated, watched), OI-913 (project folders, or golden programs until they exist), OI-917 (Makera's work-zero routine), OI-919 (no single block).
- [ ] P1 build information pasted; uncommitted changes: no.
- [ ] P2 programs from that commit.
- [ ] P3 profile is Makera Z1; unconfirmed values noted.
- [ ] P4 Makera Z1 post, `.nc`.
- [ ] P5 firmware noted (1.1.2, or SP-12 rechecked).
- [ ] P6 no tool, no material; photo.
- [ ] P7 program run as generated: spindle on with no tool, `T1 M6` watched.
- [ ] P8 axis height measured (Makera's routine, G53 Z of G54 Z0) and in the profile.
- [ ] P9 G54 by Makera's routine: Y0 Z0 on the axis, X0 as the setup sheet says; X0 face and A0 recorded.
- [ ] P10 safe Z and X range clear by jogging.
- [ ] Program read: R1 to R12 and S1 to S8 ticked.
- [ ] Override 10 %, hand on feed hold and the stop (no single block), enclosure closed, tape on the jaw.
- [ ] Sphere, cone, groove, the other reference programs and the first-cut program run: no alarm, no refused line, motion as simulated.
- [ ] Groove unwind: A did not move.
- [ ] Timed sphere operation: measured against "about N s" x 100 / override; band agreed and recorded.
- [ ] `G93` result recorded; OI-267 test run.
- [ ] Block-rate program checked (K1 to K4: fetched from 69d532f, header, footer and sections 1 and 8 read, gate 3 on its LinuxCNC copy), then the air cut (4.8) run: 14 sections timed, the result recorded for `min_block_ms` (OI-930).
- [ ] Results form filled; issue raised with the attachments.

### 6.2 Gate 5 (first cut)

- [ ] Gate 4 passed for the first-cut program, same commit, profile and post.
- [ ] Safe-practice checklist section 3 complete.
- [ ] First-cut project's geometry agreed with the owner (G0, OI-914); first-cut limit ±0.1 mm (decision 0221).
- [ ] Tools agreed with the owner (T0): rough with 104 (single flute), finish with 204 (two flutes) (OI-921); material Plastic, Speed from "tool data: Plastic".
- [ ] Tool change tested attended with a real tool (OI-41).
- [ ] Workholding: grip length matches the chuck; no grip warning; the 5 mm tab and the 2 mm, 10 mm tailstock tab as agreed.
- [ ] Stick-out, spindle speed and feeds checked against the Plastic presets.
- [ ] Simulated: up to date; no overcut; no rapid removes material; held regions and tabs whole; screenshots kept.
- [ ] Stock 35 x 35 x 100 mm measured, loaded to the grip length, tailstock up, turns true.
- [ ] G54 by Makera's routine; X0 face and A0 recorded; first rapid position jogged and clear of jaws and tailstock.
- [ ] Protection on.
- [ ] Program reread; override 10 %; hand on the stop.
- [ ] Part inspected and measured; compared with the simulation.
- [ ] Results form filled; issue raised with the attachments.

## 7. When something surprises you

### 7.1 Stop, then record

1. **Stop.** Feed hold if the machine is moving slowly and nothing is at risk; emergency stop otherwise. When unsure, emergency stop.
2. Do not restart or "just try again". Do not change the program, the offsets or the profile yet.
3. Write down: the line number the controller shows (or the last line you saw), the DRO position, any alarm text, and what you saw and heard.
4. Photograph the DRO, the controller screen and the machine.
5. Keep the exact `.nc` file and its setup sheet.
6. Raise an issue (7.2). The trial does not go on with that program until the issue is understood.

### 7.2 Issue template

Copy this into a new GitHub issue and fill it in. Attach the program, the setup sheet and the photos.

```text
Title: Gate <4 or 5>: <program> - <one line on what happened>

Gate: 4 (air cut) / 5 (first cut)
Date and operator:
Build information (Help > About > Copy build information):

Firmware version:
Machine profile: Makera Z1; axis height used (mm):
Post: Makera Z1
Program file (attached):            Setup sheet (attached):
Project fingerprint (setup sheet):
Feed override at the time (%):
Line number (controller) and the line's text:
DRO position (X Y Z A) in G54:
Alarm or message text:

What I expected:
What happened:
What I did (feed hold / emergency stop / other):
Damage or contact (none / describe):
Photos and video (attached):

Severity (verification report): 1 crash or contact / 2 wrong motion / 3 refused line or alarm / 4 other
```

For a pass, the same issue (or one per gate) holds the results form and the attachments, with the title "Gate <4 or 5> result: <programs>".

## 8. Results forms

### 8.1 Gate 4 results

```text
Gate 4 (air cut)                          Date:            Operator:
Build information:
Firmware:                                 Axis height measured (mm):
G54 in machine coordinates X / Y / Z:
Programs from: project folders (build in P1) / golden files (commit)
What the controller did on T1 M6 with no tool:
X0 taken from:                            A0 set by:
```

| Program | Read (R1-R12, S1-S8) | Override % | Alarms or refused lines (line) | Motion as simulated | Unwind: DRO A before / after, moved? | End position as program | Timed: predicted s x 100/override, measured s, ratio | Pass / fail | Issue |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sphere | | | | | | | | | |
| cone | | | | | | | | | |
| groove | | | | | | | | | |
| lobed | | | | | | | | | |
| cone_axial | | | | | | | | | |
| cone_indexed | | | | | | | | | |
| cone_axial_lead | | | | | | | | | |
| cone_held_tabs | | | | | | | | | |
| drilling | | | | | | | | | |
| swarf | | | | | | | | | |

| Check | Result |
| --- | --- |
| `G93` on this firmware (only if not 1.1.2) | |
| OI-267: A-only move with F above 1200 | |
| `G28` at the end: where the machine went | |
| Agreed band for the timed operation | |

Block-rate air cut (4.8): firmware ________ ; controller (ESP32) ________ ; file sent by ________ ; video: yes / no

Checks before running (decision 0232): K1 fetched from 69d532f, 27,966 lines: yes / no ; K2 header and footer as listed: yes / no ; K3 sections 1 and 8 as listed: yes / no ; K4 gate 3 on the LinuxCNC copy: moves reached ______ , faults ______ , pass / fail

| Section | Part | Block time | Predicted s | Measured s | Within 5 %? | Stutter or pause? | Pass / fail |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | A | 20 ms | 6.0 | | | | |
| B2 | A | 10 ms | 6.0 | | | | |
| B3 | A | 7 ms | 6.0 | | | | |
| B4 | A | 5 ms | 6.0 | | | | |
| B5 | A | 3 ms | 6.0 | | | | |
| B6 | A | 2 ms | 6.0 | | | | |
| B7 | A | 1 ms | 7.9 | | | | |
| B8 | B | 20 ms | 6.0 | | | | |
| B9 | B | 10 ms | 6.0 | | | | |
| B10 | B | 7 ms | 6.0 | | | | |
| B11 | B | 5 ms | 6.0 | | | | |
| B12 | B | 3 ms | 6.0 | | | | |
| B13 | B | 2 ms | 6.0 | | | | |
| B14 | B | 1 ms | 7.9 | | | | |

Result: the shortest block time at which both parts pass ________ ms (for `min_block_ms`, OI-930)

### 8.2 Gate 5 results

```text
Gate 5 (first cut)                        Date:            Operator:
Build information:
Gate 4 issue for this program:
Program:                                  Setup sheet fingerprint:
Material: tooling board, setup material Plastic
Stock measured (35 x 35 x 100 mm):
Geometry agreed with the owner (G0), by and date:
Tools agreed with the owner (T0), by and date:
Grip length at the chuck:                 Tabs (5 mm; 2 mm x 10 mm tailstock tab):
X0 taken from:                            A0 set by:
Tools: roughing 104 (single flute), finishing 204 (TWO flutes); stick-out of each:
Spindle rpm and Speed from:               Cutting feed / plunge feed:
First-cut limit: ±0.1 mm (decision 0221)
Simulation: cell (mm), worst material left (mm, where), overcut:
```

| Station X (mm) | Model diameter (mm) | Measured diameter (mm) | Difference (mm) | Simulation's material left there (mm) | Within limit? |
| --- | --- | --- | --- | --- | --- |
| | | | | | |
| | | | | | |
| | | | | | |
| | | | | | |
| | | | | | |

| Check | Result |
| --- | --- |
| Tabs whole | |
| Gripped length unmarked | |
| Length and tab positions within the limit | |
| Surface notes | |
| Pass / fail, issue | |

## 9. Owner decisions and open items

The owner answered the questions of OI-910 to OI-919 on 22 Oct 2026 (relayed by the coordinator). Each is a row in [open-items.md](open-items.md).

| OI | Question | Owner's answer | Where it is used | Still open |
| --- | --- | --- | --- | --- |
| OI-910 | How the timed XYZA move is measured | A: time a whole operation against "about N s" x 100 / override; band agreed at the first run; the air-cut procedure's step 3 corrected | 4.5, G4-3 | No |
| OI-911 | Whether the spindle runs in the air cut | A: it runs as generated, with no tool; the program is tested exactly as generated | P7 | No |
| OI-912 | How `T1 M6` is handled with no tool | B: run as generated and watch, ready on the stop; the tool change with a real tool is still tested before gate 5 (OI-41) | P7, F3 | No (OI-41 stays open) |
| OI-913 | Where the reference jobs come from | A: committed as project folders (built in lane GATES); OI-920 A: without their generated meshes, which a generator example writes | P2 | No (OI-922: `drilling` and `swarf`) |
| OI-914 | Which job gate 5 cuts | C: a new first-cut project to the owner's specification and geometry (5.2), built in lane GATES | 5.2, F5 | No (OI-923, OI-924) |
| OI-915 | Material and cutting data for gate 5 | Tooling board with the "Plastic" presets; Softwood copied where Plastic is missing; ask the owner before using a similar tool's data | 5.3 | No |
| OI-916 | The 3-axis test part | B: skip it; the first-cut limit stays ±0.1 mm | F4, 5.7 | No ([decision 0221](decisions/0221-gate-5-first-cut-project-and-limit.md) accepted) |
| OI-917 | How the axis height and work zero are found | Makera's own routine for Y and Z on the axis, X0 and A0; the form records the X0 face and A0 | P8, P9, M4 | No |
| OI-920 | The meshes are too large to commit | A: commit the project files, generate the parts (`sample_projects` example) | P2 | No |
| OI-921 | The first-cut project's tools | A: rough with 104, finish with 204, Plastic presets | 5.3 | No |
| OI-923 | Free stock to leave uncut | Accepted: the 68 mm held region at the start (G0) | 5.2 | Yes, for OP-22 volumes |
| OI-924 | The tailstock | A: the 2 mm, 10 mm piece is a held region at the end; the tailstock body's clearance is checked by the operator (V5, M2, M5) | 5.2, V5, M2, M5 | No |
| OI-925 | How the block-rate air cut program is checked before it runs, since it is written by a script and has not passed gates 1 to 3 (verification plan policy 6 excepts only the SP-12 program) | A: as SP-12 was, read by hand and replayed by gate 3 as a LinuxCNC copy ([decision 0232](decisions/0232-block-rate-program-checked-as-sp-12-was.md), accepted) | 4.8 K1 to K4, G4-8 | No (accepted); execution gap recorded with OI-930 in section 10 |
| OI-918 | The setup sheet does not show held regions or tabs | Show them, in plain wording (owner, 22 Oct 2026); built in lane GATES: the Setup section's held-region and tab lines and a tab table per setup, HTML and PDF | S8, 5.2 | No |
| OI-919 | Single block on the Z1 | B: no single block; 10 % override with a hand on feed hold | 4.3 | No |

### 9.1 Register audit after the submitted results

| Item | State retained | Evidence and next action |
| --- | --- | --- |
| OI-267 | Open | Explicitly not checked; run the A-only F>1200 trial before first release or obtain its individual owner disposition. |
| OI-913 | Closed | Reference project folders are built; waived physical trials do not undo that delivery. |
| OI-914 | Closed | First-cut project built; PH agreed G0 on 24 Oct 2026. |
| OI-915 | Closed | Tooling board/Plastic choice is accepted and was used. |
| OI-916 | Closed | Accepted 0221 removes the 3-axis repeatability part; no reacceptance needed. |
| OI-917 | Closed | Procedure choice is settled; missing coordinates remain with OI-60, not a reopened procedure decision. |
| OI-918 | Closed | Held-region/tab setup-sheet implementation remains delivered. |
| OI-919 | Closed | Original single-block choice remains settled; 50% minimum override differs from its 10% instruction, awaiting release disposition under OI-471. |
| OI-920 | Closed | Generated model delivery policy remains implemented. |
| OI-921 | Closed | Tools 104/204 accepted and recorded in the trial. |
| OI-922 | Closed | STEP project folders and kernel tests delivered; machine trials waived, not passed. |
| OI-923 | Open | OP-22 volumes remain unbuilt. Successful first cut does not meet that acceptance condition. Grip actually reported 5 mm; modeled held start remains 68 mm. |
| OI-924 | Closed | Accepted held-end model remains; tailstock-body clearance evidence is not supplied for M2/M5. |
| OI-925 | Closed | Acceptance of 0232 settled the verification route. K4's execution deviation is tracked with OI-930; the accepted decision is not reopened. |
| OI-930 | Closed 25 Oct 2026 | Measured 7 ms implemented through the approved profile/post fields in Report mode, window 32; requirement tests pass. Eight failed sections remain failures. K4 retained as the owner’s trial-specific release waiver; subsequent replay passed. |
| OI-931 | Closed | Accepted averaging rule: N blocks, Z1 N=32; do not replace with per-block limiting. |
| OI-932 | Open, deferred | Merge tolerance was not measured or resolved; no Merge implementation. |
| OI-933 | Open | No over-63-character trial or output-length enforcement supplied. |
| OI-934 | Open, out of scope | Production LinuxCNC blending is not this verification post or trial. |
| OI-935 | Closed 25 Oct 2026 | Project schema 19 and standalone machine schema 5 claimed before use after checking main and remote allocations. Old-copy/schema-guard tests pass; released main integration must preserve OPSPEED’s actual predecessor migrations. |
| OI-936 | Open, deferred | No arc-fitting/replay evidence supplied. |
| OI-937 | Open | Program has no G4; dwell units remain untested. |
| OI-991 | Closed 25 Oct 2026 | Owner-authorized bounded replay: 13/13 passed, all original lines/moves preserved, actual/source boundaries and all 12 transitions verified; driver/wrapper limits and CI sharding support it. Complete record 0244 remains proposed. |

OI-991 closes after the owner-authorized bounded first-cut replay passes (10.5 and 10.8); it was allocated after checking main and every remote unmerged branch (highest OI-990). OI-930, OI-935 and OI-991 close in this follow-through, with their rows moved to the Closed table. Existing closed items retain their accepted resolutions. OI-41, OI-60 and OI-471 retain the other missing machine and measurement evidence. A passing automated check does not close a waived physical check.

## 10. Results

Evidence: [owner comment of 25 Oct 2026](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6399134358), comment ID 6399134358, reproduced verbatim in 10.3. The owner reviewed as PH on 25 Oct 2026. Gate 5 was run on 24 Oct 2026 by PH. Gate 4's date and operator fields are blank; do not infer them from gate 5. The tested commit is from main’s history, not this lane’s old head; missing program files and setup fingerprints prevent a claim that the exact committed golden programs were run. Both forms name OpenChuckCAM 0.20.0, clean commit `dc610e26dd260d69b5fc859270e7bf5fc913a3bb`, built 2026-10-24T20:29:47Z, rustc 1.97.0 (2d8144b78 2026-07-07), x86_64-pc-windows-msvc. Z1 firmware: 1.1.2. The block-rate controller is identified only as ESP32, without its firmware version; WiFi upload; video exists but was not shared.

Classification follows the pass conditions above and verification plan section 6: **passed** means the submitted evidence supports that check; **failed** means an observed pass-condition violation; **waived** means explicitly omitted by the owner (not a pass); **not run** means no completed verification of that step is established by the submission, including blank or insufficient fields; it does not assert that an unreported physical action never occurred. **Not applicable** means a stated condition does not apply. A partial observation is retained in Notes, never used to infer the rest of the check.

### 10.1 Gate 4

Sphere and cone are reported passed at 50%, then 100%, with no alarms, motion and end positions as simulated, and timing matching predictions. No numerical reference-program predicted time, measured time, ratio, timing-band width or unwind DRO values were supplied. Eight other reference programs were explicitly waived for time. The first-cut air cut is not separately recorded. Gate 4 as a whole is **not recorded as passed**. The sphere/cone motion observations passed at their actual settings, while the full G4-2/G4-4/G4-7 instructions are not established because the required lower overrides were unavailable; both facts are retained.

Decision 0221 changes gate 5's part and tolerance, but explicitly retains the reference set at gate 4; it does not authorize omitting eight air cuts. Decision 0106 makes the trial a manual owner release step and permits release with unconfirmed Z1 values; it does not turn omitted checks into passes. The owner’s explicit reference-program waivers are retained as release exceptions in the release report. They do not amend the accepted records or establish a pass. Decision 0232 expressly requires K4 first; the submitted K4 waiver departs from it. The owner explicitly chose “Retain K4 as a release waiver” on 25 Oct 2026 in this takeover session. K4 is an approved release exception for this trial only; it remains waived, not passed. Decision 0232 and policy 6 still govern subsequent trials. The observed minimum passing block time is **7 ms**, limited to this program, machine, firmware and transport. No limit between the tested values, for other firmware or for other controllers is inferred.

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| P1 | passed | Clean build and full provenance supplied. | OI-471 |
| P2 | not run | No separate completed-check evidence supplied. | OI-471 |
| P3 | not run | No separate completed-check evidence supplied. | OI-471 |
| P4 | not run | No separate completed-check evidence supplied. | OI-471 |
| P5 | passed | Firmware 1.1.2. | OI-471 |
| P6 | not run | No separate completed-check evidence supplied. | OI-471 |
| P7 | not run | T1 M6: “requests tool change”; separate spindle observation absent. | OI-471 |
| P8 | not run | Axis height −48.6 mm after probing; profile correction/regeneration not evidenced. Safe Z50 would be machine Z+1.4 at this height, so clearance requires reconciliation. | OI-471; OI-60 |
| P9 | not run | X0: “Setup on in control software”; A0: “default position via makera setup.” G54 machine X/Y/Z blank. | OI-471 |
| P10 | not run | No separate completed-check evidence supplied. | OI-471 |
| R1 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R2 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R3 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R4 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R5 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R6 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R7 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R8 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R9 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R10 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R11 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R12 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S1 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S2 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S3 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S4 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S5 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S6 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S7 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S8 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| G4-0 | passed | Sphere read: “No issues”. | OI-471 |
| G4-1 | not applicable | Firmware is 1.1.2; no new SP-12 run required. | OI-471 |
| G4-2 | not run | Partial evidence: sphere motion passed at 50%, then 100%, with no alarms and expected end position; G28 went to clearance. The required 10% run was not completed (reported minimum 50%). | OI-471: override disposition |
| G4-3 | passed | Sphere: “Matched predicted”; agreed band “yes”, numerical band and times absent. | OI-471 |
| G4-4 | not run | Partial evidence: cone motion passed at 50%, then 100%, with no alarms and expected end position. The required 10% run was not completed (reported minimum 50%). | OI-471: override disposition |
| G4-5 | waived | Groove not run; no groove unwind DRO evidence. | OI-471 |
| G4-6 | not applicable | G93 recheck conditional on firmware other than 1.1.2. | OI-471 |
| G4-6b | not run | Owner: “Not checked”. OI-267 stays open. | OI-267 |
| G4-7 | not run | Partial evidence: sphere and cone raised from 50% to 100%; controller minimum override reported 50%. The full instructed 25%/50%/100% sequence was not completed. | OI-471: override disposition |
| G4-8 | failed | Six sections passed; eight visibly stuttered and failed. K4 was waived. See individual section results. | OI-930 |
| K1 | passed | Owner confirms pinned 69d532f and 27,966 lines. | OI-930 |
| K2 | passed | Header/footer checks: yes. | OI-930 |
| K3 | passed | Sections 1 and 8 checked: yes. | OI-930 |
| K4 | waived | Owner: “pass - gate 3 waived”; moves reached and faults N/A. Classified waived, not passed, under decision 0232. | OI-930 |
| B0 | not run | Firmware 1.1.2, ESP32, WiFi upload; video yes, not shared. ESP32 firmware version and explicit playback-from-machine confirmation absent. | OI-471: complete controller/transport provenance |

| Program | Result | Evidence | Follow-up |
| --- | --- | --- | --- |
| groove | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| lobed | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_axial | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_indexed | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_axial_lead | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_held_tabs | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| drilling | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| swarf | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| first_cut air cut | not run | Gate 4 issue field blank; no separate air-cut result supplied. | OI-471: obtain evidence or run |

| Section | Part | Block time | Predicted s | Measured s | Within 5 %? | Stutter or pause? | Pass / fail |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | A | 20 ms | 6.0 | 5.9 | yes | no | pass |
| B2 | A | 10 ms | 6.0 | 6.1 | yes | no | pass |
| B3 | A | 7 ms | 6.0 | 6.1 | yes | no | pass |
| B4 | A | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B5 | A | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B6 | A | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B7 | A | 1 ms | 7.9 | N/A | N/A | yes | fail |
| B8 | B | 20 ms | 6.0 | 6.2 | yes | no | pass |
| B9 | B | 10 ms | 6.0 | 5.8 | yes | no | pass |
| B10 | B | 7 ms | 6.0 | 5.9 | yes | no | pass |
| B11 | B | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B12 | B | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B13 | B | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B14 | B | 1 ms | 7.9 | N/A | N/A | yes | fail |

Result: the shortest block time at which both parts pass ___7___ ms (for `min_block_ms`, OI-930)

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| B15 | not applicable | No 20 or 10 ms section failed; stop condition not triggered. | OI-930 |
| B16 | passed | Minimum passing value in both parts and all longer sections: 7 ms. | OI-930: measured value implemented as 7 ms/window 32/Report |

All eight failed sections (B4–B7, B11–B14) are linked to OI-930. Their measured times remain N/A. The failure at shorter times is evidence for the measured threshold, not evidence that all block-rate sections passed. B15 is not applicable: none of the 20 or 10 ms sections failed. B16 (record the minimum) passed: 7 ms.

### 10.2 Gate 5

The owner reports the tooling-board first cut **passed** at decision 0221’s ±0.1 mm limit: tabs whole, gripped length unmarked, length and tab positions within limit, surface as simulated. “All measurements of resulting machined stock were as expected.” About 0.8 mm remained adjacent to tabs as simulated, with no overcut reported. This is the submitted first-cut outcome, not a claim that every procedural field or five-station measurement table was completed. No station diameters, instrument, numerical errors, setup fingerprint, G54 machine coordinates, photographs or screenshots were supplied.

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| F1 | not run | First-cut gate 4 issue and result absent. | OI-471 |
| F2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| F3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-41 |
| F4 | passed | ±0.1 mm, decision 0221. | OI-471 |
| F5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| F6 | passed | Tools agreed by PH, 2026-10-24. | OI-471 |
| G0 | passed | Geometry agreed by PH, 2026-10-24. | OI-471 |
| W1 | passed | Grip 5 mm, within the agreed maximum 10 mm; 68 mm held region remains the agreed free-stock workaround, not a measured jaw depth. | OI-471 |
| W2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| W3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| W4 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| W5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| T0 | passed | Tools agreed by PH, 2026-10-24. | OI-471 |
| T1 | passed | 104 single flute; 204 two flutes. | OI-471 |
| T2 | passed | Plastic and tool-data speed source reported; one 10000 rpm entry supplied. Individual operation speeds were not recorded. | OI-471 |
| T3 | not applicable | Both selected tools have Plastic presets. | OI-471 |
| T4 | not run | Cutting/plunge feed 1000/300 supplied; individual F-word limit check not reported. | OI-471 |
| T5 | not run | Stick-out “as with Makera standard collet spacer”; numerical depth/margin not supplied. | OI-471 |
| V1 | not run | Simulation reported, cell “default”; completion/current-result check not separately evidenced. | OI-471 |
| V2 | not run | No overcut reported; separate rapid-removal check not supplied. | OI-471 |
| V3 | passed | ~0.8 mm adjacent to tabs. | OI-471 |
| V4 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| V5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| V6 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M1 | passed | Stock measured 35x35x100 mm. | OI-471 |
| M2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M4 | not run | X0 chuck jaw, A0 default; G54 machine coordinates absent. | OI-60 |
| M5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M6 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C1 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-41 |
| C4 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C6 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C7 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| I1 | passed | Tabs whole, gripped length unmarked; four-side photos not supplied. | OI-471 |
| I2 | not run | General measurement statement only; at least five station diameters and errors not supplied. | OI-471 |
| I3 | not run | Partially evidenced: length and tab positions were reported within ±0.1 mm; individual dimensions and tab diameters were not supplied, so the full step is not established. | OI-471 |
| I4 | passed | Surface agrees with simulation; ~0.8 mm adjacent to tabs, no overcut reported. | OI-471 |
| I5 | passed | “as simulation”. | OI-471 |

### 10.3 Verbatim owner submission

The following is source evidence, including the owner’s “all steps passed” shorthand. That shorthand is not the audited classification; sections 10.1 and 10.2 govern this report. Blank fields and N/A are unchanged; trailing whitespace is normalised.

<details>
<summary>Owner comment 6399134358, 25 Oct 2026</summary>

````text
Here are the results:


### 8.1 Gate 4 results

```text
Gate 4 (air cut)                          Date:            Operator:
Build information:

OpenChuckCAM 0.20.0
Commit: dc610e26dd260d69b5fc859270e7bf5fc913a3bb
Uncommitted changes: no
Built: 2026-10-24T20:29:47Z
Rust: rustc 1.97.0 (2d8144b78 2026-07-07)
Target: x86_64-pc-windows-msvc
Crates:
  openchuck-app 0.20.0
  openchuck-buildinfo 0.20.0
  openchuck-cli 0.20.0
  openchuck-gcode 0.20.0
  openchuck-geom 0.20.0
  openchuck-import 0.20.0
  openchuck-path 0.20.0
  openchuck-post 0.20.0
  openchuck-project 0.20.0
  openchuck-sim 0.20.0
  openchuck-types 0.20.0
  openchuck-ui 0.20.0
  openchuck-viewport 0.20.0

Firmware: 1.1.2                                Axis height measured (mm): -48.6 (after probing routine)
G54 in machine coordinates X / Y / Z:
Programs from: project folders (build in P1) / golden files (commit)
What the controller did on T1 M6 with no tool: requests tool change
X0 taken from:  Setup on in control software                          A0 set by: default position via makera setup.
```

| Program | Read (R1-R12, S1-S8) | Override % | Alarms or refused lines (line) | Motion as simulated | Unwind: DRO A before / after, moved? | End position as program | Timed: predicted s x 100/override, measured s, ratio | Pass / fail | Issue |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sphere | No issues | 50 then 100% | None | Yes | As expected | Yes | Matched predicted | Pass | N/A |
| cone | No issues | 50 then 100%  | None | Yes | As expected | Yes | Matched predicted | Pass | N/A |
| groove | | | | | | | | | |
| lobed | | | | | | | | | |
| cone_axial | | | | | | | | | |
| cone_indexed | | | | | | | | | |
| cone_axial_lead | | | | | | | | | |
| cone_held_tabs | | | | | | | | | |
| drilling | | | | | | | | | |
| swarf | | | | | | | | | |

Deviation: Only cone and sphere were run due to time. Others were waived.
Deviation: Minimum override is 50%. Sphere was run first at 50% then 100%, same with cone.

| Check | Result |
| --- | --- |
| `G93` on this firmware (only if not 1.1.2) | Running 1.1.2 |
| OI-267: A-only move with F above 1200 | Not checked |
| `G28` at the end: where the machine went | To clearance |
| Agreed band for the timed operation | yes |

Block-rate air cut (4.8): firmware 1.1.2 ; controller (ESP32) __ESP32__ ; file sent by __Wifi Upload____ ; video: yes - not shared

Checks before running (decision 0232): K1 fetched from 69d532f, 27,966 lines: yes ; K2 header and footer as listed: yes ; K3 sections 1 and 8 as listed: yes ; K4 gate 3 on the LinuxCNC copy: moves reached __N/A__ , faults __N/A___ , pass - gate 3 waived.

| Section | Part | Block time | Predicted s | Measured s | Within 5 %? | Stutter or pause? | Pass / fail |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | A | 20 ms | 6.0 | 5.9 | yes | no | pass |
| B2 | A | 10 ms | 6.0 | 6.1 | yes | no | pass |
| B3 | A | 7 ms | 6.0 | 6.1 | yes | no | pass |
| B4 | A | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B5 | A | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B6 | A | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B7 | A | 1 ms | 7.9 | N/A | N/A | yes | fail |
| B8 | B | 20 ms | 6.0 | 6.2 | yes | no | pass |
| B9 | B | 10 ms | 6.0 | 5.8 | yes | no | pass |
| B10 | B | 7 ms | 6.0 | 5.9 | yes | no | pass |
| B11 | B | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B12 | B | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B13 | B | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B14 | B | 1 ms | 7.9 | N/A | N/A | yes | fail |

Result: the shortest block time at which both parts pass ___7___ ms (for `min_block_ms`, OI-930)

### 8.2 Gate 5 results

```text
Gate 5 (first cut)                        Date:   2026-10-24         Operator: PH
Build information:

OpenChuckCAM 0.20.0
Commit: dc610e26dd260d69b5fc859270e7bf5fc913a3bb
Uncommitted changes: no
Built: 2026-10-24T20:29:47Z
Rust: rustc 1.97.0 (2d8144b78 2026-07-07)
Target: x86_64-pc-windows-msvc
Crates:
  openchuck-app 0.20.0
  openchuck-buildinfo 0.20.0
  openchuck-cli 0.20.0
  openchuck-gcode 0.20.0
  openchuck-geom 0.20.0
  openchuck-import 0.20.0
  openchuck-path 0.20.0
  openchuck-post 0.20.0
  openchuck-project 0.20.0
  openchuck-sim 0.20.0
  openchuck-types 0.20.0
  openchuck-ui 0.20.0
  openchuck-viewport 0.20.0

Gate 4 issue for this program:
Program:  first_cut                          Setup sheet fingerprint:
Material: tooling board, setup material Plastic
Stock measured (35 x 35 x 100 mm): 35x35x100
Geometry agreed with the owner (G0), by and date: PH / 2026-10-24
Tools agreed with the owner (T0), by and date: PH / 2026-10-24
Grip length at the chuck: 5mm                Tabs (5 mm; 2 mm x 10 mm tailstock tab):
X0 taken from: Chuck jaw                           A0 set by: default
Tools: roughing 104 (single flute), finishing 204 (TWO flutes); stick-out of each: as with Makera standard collet spacer
Spindle rpm and Speed from: 10000 As tool library        Cutting feed / plunge feed: 1000 / 300
First-cut limit: ±0.1 mm (decision 0221)
Simulation: cell (mm), worst material left (mm, where), overcut: cell: default, ~0.8mm adjacent to tabs, no overcut
```

All measurements of resulting machined stock were as expected.

| Check | Result |
| --- | --- |
| Tabs whole | yes |
| Gripped length unmarked | yes |
| Length and tab positions within the limit | yes |
| Surface notes | as simulation |
| Pass / fail, issue | pass |

## 10. Results

Empty until the owner submits the results of gates 4 and 5. They are then added here, one row per step ID, by the lane that folds them in, with the issue that holds the evidence; a step that did not pass names its defect issue. A surprise that changes an instruction is fixed in the step above in the same change, and the row says so.

### 10.1 Gate 4

As above - all steps passed

### 10.2 Gate 5

As above - all steps passed

## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | PH |
| Date | 2026-10-25 |
| Machine and firmware | As above |
| Changes required | No |
| Reviewed: yes / no | Yes |
````

</details>

### 10.4 Initial takeover verification (before implementation, 25 Oct 2026)

This is historical preparation evidence; post-implementation results are recorded in 10.6. These checks concern the old lane head `9573685001117c4e71a236c555f34138fb21cbad` plus uncommitted preparation. Main `dc610e26dd260d69b5fc859270e7bf5fc913a3bb` was inspected, not merged. They do not verify omitted physical trials.

| Check | Outcome | Time |
| --- | --- | --- |
| `cargo test -p openchuck-app --test output_options --locked` | 43 passed | 19.41 s tests; 75.582 s command/build |
| `cargo test -p openchuck-post -p openchuck-project --tests --locked -- --skip req_set_01_req_gc_09_z1_records_the_measured_block_time_and_queue_window` | Existing baseline passed; new red test explicitly excluded only for this baseline comparison | 7.108 s |
| `cargo test -p openchuck-project --test z1_block_time --locked` | Expected TDD failure before implementation: absent min_block_ms (`None` versus `Some(7.0)`); not ready | <0.01 s test; 55.380 s command/build |
| `cargo test -p xtask replay --locked` | Six passed, including first-cut replay selection | 2.469 s |
| `cargo test -p openchuck-cli --test cli --locked` | 27 passed | 0.19 s tests; 11.919 s command/build |
| `cargo xtask all` | Passed: dependency rules, licences, records, testdata, conflicts, publishing checks and traceability (99/99 P1) | 107.106 s |
| `cargo clippy -p openchuck-project -p openchuck-post -p openchuck-app -p openchuck-cli --all-targets --locked -- -D warnings` | Passed | 19.468 s |
| `cargo fmt --all -- --check`; `git diff --check` | Passed | 1.471 s formatting |
| `cargo audit`; `cargo deny check` | Commands unavailable: cargo-audit and cargo-deny not installed | 0.039 / 0.044 s |
| Changed-crate coverage | Not run: cargo-llvm-cov and nextest not installed; no product implementation added yet, only the requirement test | — |
| Full e2e/first-cut determinism, full application/UI suites and STEP kernel checks | Not rerun yet; required before final intake | — |
| Native Windows/macOS and Z1 trials | No access in this Linux session; original Windows/Z1 evidence is the owner submission | — |

No test observed so far exceeded ten minutes. No instrumented artifacts were created. Current main has the clipboard-win 5.4.3 dependency fix; this old lane still has 5.4.1. Preserve the main fix during the Merge Manager’s released main update. Do not push a speculative dependency update merely to begin takeover.

Local K4 attempt uses the exact Z1 file at `69d532f` with only the footer replacement required by 0232, and image `ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10` at digest `sha256:22d8b9f0acb8a0fd7a02bf47be7fd9e864e671bd510ca27ab35ff214d1489546`. Initial `cargo xtask replay` failed twice with no report (48.226 s command). Temporary copied files were mode 600/700 and unreadable by the image’s unprivileged user. Changing only those temporary copies to 644/755 let LinuxCNC complete. The comparison then failed at line 2215, target `[63.25, 0, 50, 0]`, with a 46 ms sampling gap. LinuxCNC reported no errors and final position approximately `[0, 0, 50, 439.331]`. This is not a K4 pass. First report SHA-256: `2737b10a7e85f3f46b3ff278f275e27e8680824e8cb9375a6c170d64aa0407de`. Raw report is temporary local evidence (8.8 MB), not committed as test data.

The one subsequent repeat, with the build workload stopped, **passed the local comparison**: all 7,559 moves recognised by `gcode-sim` reached; largest difference 0.0379 mm at tolerance 0.05 mm; 68.8 s of reported motion; no fault. Docker wall time: 153.711 s; comparison: 0.487 s. Maximum sampling gap: 25.8 ms; final position approximately `[0, 0, 50, 439.331]`, G92 offset zero. Report SHA-256: `48fdb6ec67c3fd74d8166bed41d3e4dc73875a1edca9e8af732359be8ba0f6c8`.

Reproduction after the temporary-file permissions adjustment: `docker run --rm -v /tmp/openchuck-replay-1237-block_rate_linuxcnc.ngc:/work -w /work -e CHECK_PROGRAM=/work/program.ngc -e CHECK_REPORT=/work/report.json ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 linuxcnc checks.ini`, then `target/debug/openchuck-cli gcode-sim /tmp/block_rate_linuxcnc.ngc /tmp/openchuck-replay-1237-block_rate_linuxcnc.ngc/report.json --tolerance 0.05`. The work directory contains unmodified repository checks.ini, sim.tbl and sim_driver.py, plus the pinned program with the prescribed footer substitution. The exact digest above pins the image. No tolerance was increased to obtain a pass.

This subsequent replay supplies current simulator evidence. It does not show that K4 preceded the owner’s machine run, does not change the historical K4 row from waived to passed, and does not pass any omitted physical reference program. The owner retained the sequence deviation as a release waiver on 25 Oct 2026. The approved 7 ms profile/report implementation and schema allocation are completed locally. Post-implementation verification is recorded below; the subsequent bounded first-cut evidence is recorded in 10.8; this initial-stage account does not claim a full uninterrupted replay.

### 10.5 First-cut replay duration flag (owner-authorized bounded replay)

The stored first-cut LinuxCNC golden has **1,727.333 s of nominal fed motion**, derived from its modal G0/G1 positions and F words under the LinuxCNC feed rule. This is an analysis estimate, not a replay result, and excludes rapids, acceleration and tool changes. Both the original wrapper and driver imposed 900 s, so an unchanged full replay could not finish. AGENTS.md requires an owner disposition for an uninstrumented test over ten minutes.

The owner directly instructed “Apply your suggested approach to allow support of the test” on 25 Oct 2026, authorizing bounded chunks preserving every original move with explicit modal-state and continuity checks. The complete detailed record [0244](decisions/0244-bounded-first-cut-linuxcnc-replay.md) is proposed; acceptance of its full text is not inferred from that earlier instruction. No reacceptance of 0221, 0222 or 0232 is requested.

Implementation targets at most 300 s estimated simulator time per fixture, including startup, with a hard limit of at most 600 s in both driver and wrapper. It expands before CI sharding. Each fixture initializes simulated joints at the original boundary coordinates, restores G modes, F, tool and spindle state, and checks actual start/end coordinates and modes at simulator-only M0 stops followed by `M66 E0 L0` queue barriers. The barriers prevent interpreter read-ahead from changing F/modes before the paused snapshot. Original contiguous source lines are retained exactly once across all chunks; production G-code and comparison tolerances are unchanged. Unsupported modes/syntax and atomic moves that cannot fit fail closed. Separate fixtures add stops and fresh interpreter initialization, so their results cannot establish uninterrupted full-program dynamics or the original pre-cut gate ordering.

OI-991 is closed by the thirteen passing bounded fixtures and verified source/boundary continuity in 10.8. OI-471 retains the physical-trial and missing-measurement gaps. The K4 release waiver and the supported Gate 5 physical outcome are unchanged.

### 10.6 Implementation and local verification, 25 Oct 2026

The requirement-first Z1 profile test failed before the fields existed (`None` versus `Some(7.0)`). Report/setup-sheet and longest-run-feed checks also exposed failures before their implementations were corrected. Accepted 0222 now has an executable **Report** implementation: the new built-in Z1 uses **7 ms / 32 blocks / Report**, and optional post fields override the profile independently. A complete sliding window is required; rapids and operation boundaries reset it. Reports identify affected blocks, the longest run, its effective contact feed, the worst window mean and estimated added time. An incomplete window is identified explicitly. The estimate does not change path durations or G-code. Slow down and Merge remain unbuilt.

Older project machine copies and other profiles default to **0 ms / 1 / Off**; no migration substitutes the new built-in profile. Tests cover old standalone profiles/project copies, schema guards, disabled-profile warnings, both posts' byte-identical Off/Report output, deterministic reports, overrides, invalid input, rapid boundaries, and application warnings plus HTML/PDF setup sheets. CLI inspection shows the actual setting. The public API additions bump the workspace/library version to **0.21.0**. The intentional G-code text change is the generator version comment; motion/feed words remain unchanged. Existing golden tests normalise that comment and passed at 1, 2 and N threads.

Project schema **19** and standalone machine schema **5** were claimed before use after checking main and all remote branches. This old branch uses reserved predecessor steps for 18/4. **Before release, the Merge Manager's main integration must replace those placeholders with OPSPEED's actual migrations and preserve its operation-material and spindle-limit fields.** They are not a substitute for integrating main. Current main's clipboard-win 5.4.3 and tokio 1.53.3 dependency fixes are retained locally; no unrelated dependency upgrade was made.

| Local check | Result | Uninstrumented time / scope |
| --- | --- | --- |
| Product tests: types, project, post, application and CLI, including e2e golden regeneration and first-cut determinism | All **775 currently selected tests** have passing evidence across the baseline and final affected-test reruns; no unresolved product-test failure | Final affected/remaining batch: 325/325, 488.246 s; core batch: 234/234, 2.506 s; subsequent focused additions passed. Longest individual test: first-cut determinism, **369.528 s**. |
| UI-26 numeric-entry tests | 18/18 passed | 0.75 s |
| xtask replay-selection tests | 6/6 passed | 0.01 s final execution (build excluded) |
| Clippy, all targets, warnings denied | Passed for types, project, post, app, CLI, UI and xtask | 9.023 s final command (earlier all-target run 32.60 s) |
| `cargo xtask all` | Passed: dependencies, licences, records, testdata, conflicts, publishing checks and traceability; **100/100 P1** requirements have tests | 22.051 s final command |
| `cargo audit` | Passed; 1,352 advisories / 449 dependencies, no vulnerability reported after the tokio fix | 5.448 s command |
| `cargo deny check` | Passed advisories, bans, licences and sources; existing duplicate-version/unmatched-exception warnings retained | 2.371 s command |
| Formatting and whitespace | `cargo fmt --all -- --check` and `git diff --check` passed | 1.465 s formatting; whitespace clean |
| Fuzz manifest/lockfile consistency | Locked offline metadata passed; not a fuzz campaign | Scope only |

Tests were run uninstrumented before coverage. Every newly observed test over about a minute is listed in both the `coverage` exclusions and `long` selections of `.config/nextest.toml`, with measured times. None of the executed uninstrumented product tests exceeded ten minutes. The first-cut duration flag now has an owner-authorized bounded implementation (10.5); the subsequent execution evidence is recorded separately.

Changed-crate **own-test** line coverage is a lower bound, not the nightly whole-suite result:

| Crate | Covered / executable lines | Line coverage |
| --- | --- | --- |
| openchuck-types | 2,043 / 2,621 | 77.95% |
| openchuck-project | 3,276 / 3,608 | 90.80% |
| openchuck-post | 652 / 740 | 88.11% |
| openchuck-app | 15,495 / 16,786 | 92.31% |
| openchuck-cli | 929 / 1,400 | 66.36% (27 CLI command tests; e2e coverage not included in this lower bound) |

These totals **do not meet the 95% development target**; it is not a PR merge gate (0102). No source was excluded to improve them. The new `block_time.rs` module is **121/123 lines, 98.37%** on its own crate's tests. Its unexecuted lines are a loop closing brace and the defensive zero-duration effective-feed fallback; normal posted moving blocks require a positive duration. Other new own-crate gaps are the types' field-name/provenance plumbing (covered by consuming application/CLI tests), a project migration closing brace, UI/profile label branches and setup-sheet fallback branches. These are recorded rather than filled with implementation-mirroring tests. Application coverage ran all 463 selected own tests (708.717 s instrumented suite time; 10 long tests skipped by the coverage profile). CLI e2e tests passed uninstrumented. Instrumented artifacts were cleaned with `cargo llvm-cov clean --workspace` and the separate instrumented target directory removed; JSON summaries were retained outside it.

The K4 replay outcome and hashes are in 10.4. Reference-program gate-3 replays passed separately on the idle Docker host at the repository's unchanged **0.1 mm** comparison tolerance; their final outcomes are recorded in 10.7. They cannot pass any omitted physical air cut. The K4 repeat used 0.05 mm, stricter than that repository default.

Facilities unavailable here: native Windows/macOS execution, direct Z1 access, and an installed OpenCASCADE 7.8.1 kernel. The existing lane's earlier ten passing STEP-pipeline kernel tests remain historical evidence; this prepared code has not rerun them. Current integrated-head kernel CI and platform checks remain required. The owner's Windows/Z1 0.20.0 results are preserved independently from this Linux 0.21.0 verification. Generated G-code remains the user's responsibility to verify (NFR-12).

### 10.7 Local reference-program LinuxCNC replay results before bounded first-cut support

All **11 selected programs** (ten reference jobs plus the small-part fixture) passed on the managed Docker host using the image/digest in 10.4 and unchanged repository tolerance **0.1 mm** (or degrees for A); final positions retain the checker's separate 0.001 tolerance. This is the default `xtask` program selection with only `first_cut_linuxcnc.ngc` withheld under OI-991. At this earlier verification stage the full first-cut gate-3 replay remained **not run**; the subsequent owner-authorized bounded implementation and evidence are recorded below. These passes do not supply hardware evidence for the eight waived air cuts.

The temporary reproduction harness copied unmodified `checks.ini`, `sim.tbl` and `sim_driver.py`, and the exact committed program into each `/tmp/gates-gate3/<filename>/` directory, with permissions readable by the image's unprivileged user. It ran the same `docker ... linuxcnc checks.ini` command and then `target/debug/openchuck-cli gcode-sim <program> <report> --tolerance 0.1`. Heavy build/coverage workloads were stopped for the final replays. Reports/logs are temporary local artifacts; the following summary and report hashes preserve the evidence identity in the repository.

| LinuxCNC program | Result | Recognised moves reached | Largest difference | Reported motion s | Docker wall s | Report SHA-256 |
| --- | --- | --- | --- | --- | --- | --- |
| post_linuxcnc_small_part.ngc | passed; no fault | 30 | 0.1000 | 29.6 | 44.215 | `a85d9a4f3dedbece16d749fbac7b339110366b7a840009668f078e5dd333aa15` |
| cone_axial_lead_linuxcnc.ngc | passed; no fault | 1345 | 0.0536 | 286.4 | 317.552 | `ab886c0e0af81be494c3d62bbad02d1c75227758f426be1ef6ffd5f7f67ee8a9` |
| cone_axial_linuxcnc.ngc | passed; no fault | 609 | 0.0999 | 271.2 | 288.515 | `2a12342e5b4ec0768cfbe880f0b715eb86cf0dc83bce07914272e512bcfa3c01` |
| cone_held_tabs_linuxcnc.ngc | passed; no fault | 1591 | 0.1000 | 171.1 | 196.048 | `256e5f473510031222ce86169b2db38009f9c121e9fcfccd1a5fcb46c43a09cf` |
| cone_indexed_linuxcnc.ngc | passed; no fault | 77 | 0.0067 | 36.2 | 57.916 | `1b2288559573a0d91b1f5e350c53fd8153be8baa1606ad366c8698c443259760` |
| cone_linuxcnc.ngc | passed; no fault | 1086 | 0.0150 | 117.7 | 141.676 | `b03943da8a666267b2e7ab7992a3c54a5fab0c599997c8d95133e33b072effa5` |
| drilling_linuxcnc.ngc | passed; no fault | 15 | 0.0010 | 23.3 | 43.096 | `2a69ef2432baa21e3d1f6c8f6686de64f468a21555b6c7ddd7cd553f1c0e1258` |
| groove_linuxcnc.ngc | passed; no fault | 1104 | 0.0150 | 152.9 | 176.608 | `d33f237a7b2f78e8b7dcb00ac00e9ac17e7d8b9bb43ac3a22e85f34187fe40b0` |
| lobed_linuxcnc.ngc | passed; no fault | 1806 | 0.0920 | 105.6 | 131.348 | `6908a091013f9f6d498039e54e7852a319bed1637531955437ea64829bdb2902` |
| sphere_linuxcnc.ngc | passed; no fault | 726 | 0.0150 | 87.8 | 109.496 | `cedd62cfe3c198337df4aa0e42d5ea2f5cf2625e0aaf92b43b35c60e162ff7a4` |
| swarf_linuxcnc.ngc | passed; no fault | 54 | 0.0041 | 105.6 | 126.108 | `1b713a8e0cfa6bff6d17761f5ad621501ea0fb5882ec183ced26138caaf7ae69` |

An earlier cone-axial attempt failed comparison at line 314, expected `[0, 0, 45, 180]`, with no LinuxCNC errors, maximum sampling gap 97.8 ms and wall time 295.818 s. One attempt was interrupted when the host was busy with coverage; it supplied no result. The idle-host repeat in the table passed. The earlier failure is retained; neither a tolerance change nor a product fix was used to obtain the repeat pass. An initial temporary wrapper also mistakenly selected a Z1 `.ngc` file; LinuxCNC rejected its Z1-specific `G92.4`. Selection was corrected to the repository's `linuxcnc` filename rule before these final reference results. It is not a LinuxCNC-post product failure.

### 10.8 Owner-authorized bounded first-cut follow-through

The owner instructed “Apply your suggested approach to allow support of the test” after the explanation of the first-cut gate-3 duration gap. This authorizes the bounded approach. Complete record [0244](decisions/0244-bounded-first-cut-linuxcnc-replay.md) remains proposed; no acceptance of its later full text is inferred. Decisions 0221, 0222 and 0232 remain accepted.

The first-cut source expands to **13 deterministic simulator fixtures**. Their contiguous source ranges reconstruct every original line exactly once, in order. Both driver and wrapper enforce a limit of at most 600 s per unit, or a smaller requested timeout; the planner targets 300 s including startup. Each fixture restores and checks its original boundary state. The independent `gcode-sim --source-lines FIRST:LAST` path replays the original unsplit prefix, checks both actual boundary states and compares the original moves in that range. It therefore checks the fixture against the original program. Production G-code and comparison tolerances are unchanged. The public verification API additions use the already-unreleased library version 0.21.0; ordinary report JSON remains compatible through default-empty boundary data. No machine/project schema is added.

**Observed initial tooling failure:** part 1 passed in 237.846 s. Parts 2–5 each failed twice at their starting state check, with the following values. Part 6 was interrupted while the issue was investigated; it supplies no result. Parts 7–13 were not run in that attempt. The observations and first reports are retained under the initial-attempt evidence; none is retroactively passed.

| Part | Expected boundary F | Reported F | Attempt 1 wall s | Attempt 2 wall s |
| --- | --- | --- | --- | --- |
| 2 | 16.7 | 17.3 | 20.234 | 20.229 |
| 3 | 18.1 | 299.9 | 19.218 | 18.621 |
| 4 | 19.7 | 20.7 | 19.019 | 18.422 |
| 5 | 21.7 | 21.6 | 19.022 | 19.625 |

The reported F matched the next source line, with about a 1 ms sampling gap. The conclusion is that interpreter-reported settings look ahead across M0; this is a checkpoint assumption in the test tooling, not a new physical machine result. Each checkpoint now has a LinuxCNC-only `M66 E0 L0` queue barrier after M0, preventing that read-ahead before the paused state is checked. A focused real-simulator probe then checked F60 before an original line changed it to F120 and passed the independent original-source comparison. No feed, coordinate or comparison tolerance was relaxed. A separate negative probe deliberately supplied a wrong expected tool and stopped before original motion.

Local regressions passed: 19 gcode tests, 28 CLI command tests, 97 xtask tests, and the Python boundary-helper tests. Uninstrumented own-test execution was under a second per suite (build time excluded); no new Rust/nextest case exceeded a minute. The external simulator units are timed separately and distributed by the gate-3 harness rather than nextest. The legacy whole-program small-part replay passed in 52.168 s; a short fixture exercised the complete bounded harness in 23.435 s. Audit and deny passed in 5.798 s and 1.862 s. Final document-freeze checks passed: `cargo xtask all` 6.815 s (records, testdata and 100/100 P1 traceability included), all-target Clippy for eight relevant crates with warnings denied 9.054 s, formatting 1.505 s, whitespace clean. The final own-test executions passed: gcode 19 (including 0.25 s reader properties), CLI command tests 28 in 0.17 s, xtask 97 in 0.21 s.

Own-test line-coverage lower bounds for this follow-through are gcode 861/1,116 (77.15%), CLI command tests 946/1,406 (67.28%), and xtask 2,528/3,364 (75.15%); these whole-crate totals remain below the 95% development target. The new independent range module covers 163/171 lines (95.32%); the planner covers 315/322 lines (97.83%). Remaining new gaps are defensive parse/impossible-state paths and the external execution paths exercised by uninstrumented LinuxCNC runs. Focused Python unit/short-runtime coverage is 36/37 executable lines (97.30%) for the boundary helper and 98/106 (92.45%) for the driver. These standard-library trace counts include the unexecuted synthetic module-prologue line 0; it is not source code. Other driver gaps are defensive timeout/homing/extra-checkpoint paths and legacy-control paths exercised uninstrumented. LLVM-instrumented targets were cleaned after reporting; trace evidence is retained outside the repository. Instrumented runtime timings are not used as performance evidence.

The synchronized bounded run passed all thirteen fixtures on their first attempts. All 16,853 original lines reconstruct byte for byte, every adjacent expected boundary agrees, and all twelve actual boundary transitions match. The independent comparisons reached all 16,827 recognised original moves with no faults. Maximum per-unit wall time: 270.312 s; sum of unit wall times: 2,441.792 s. The sequential command batch took 2,448.126 s (40 min 48.126 s), including repeated harness setup. No execution unit exceeded ten minutes. OI-991 is closed. This software evidence does not close the physical reference-program waivers, the override discrepancy, OI-267 or the unrecorded Gate 5 measurements. Native Windows/macOS, installed OpenCASCADE kernel execution and direct Z1 access remain unavailable locally.



The bounded evidence was also assembled in original-source order and compared once against the complete original program: all 16,827 recognised moves reached, largest difference 0.1000, no fault (3.126 s comparison command). This assembled comparison is subsequent bounded evidence, not an uninterrupted simulator execution. Actual transition checks retain the original coordinates/feed/tool/spindle state across all twelve junctions.

Software-fixture SHA-256: `d42a2e229b631794fb37b25086e464164b8c4dd364cc2111172a9e78b287bd7e`. This identifies the stored program verified here; it does not fill the owner's blank physical program/setup fingerprints. The stored fixture includes S10000 roughing and S12000 finishing. The owner's single 10000 rpm entry does not identify every physical operation's speed; per-operation physical settings remain unrecorded under OI-471.

| Part | Original source lines | Recognised moves | Largest difference | Wall s | Report SHA-256 |
| --- | --- | --- | --- | --- | --- |
| 1 | 1–872 | 862 | 0.0175 | 236.797 | `be80eb1876ef7772af791dddf3d82727045134e4475f65de93c4abeedec71aa4` |
| 2 | 873–2020 | 1139 | 0.1000 | 224.992 | `d0d621aef9f5e28de212c091f1906d5cb4760c01f25ee72e92b7fae5756b7d10` |
| 3 | 2021–3497 | 1477 | 0.1000 | 209.736 | `3ed65b52a6815824eeadd2872221217d952cff79b04efc6a0ed7d15e7551b389` |
| 4 | 3498–5309 | 1812 | 0.0442 | 192.267 | `f9301c609ca59aa058850e1392009286024e0f40f9ff730cc441cfc9a26c8905` |
| 5 | 5310–7183 | 1874 | 0.0150 | 191.052 | `a2e2ce32e8e07aecc795c453b6965535831b68b8ea86277cb4aa24e9263edfd3` |
| 6 | 7184–9052 | 1869 | 0.1000 | 190.289 | `feee2b44465f756a526b76acc9fc84cfb1f9a01c547ab61711834e04340be654` |
| 7 | 9053–10893 | 1841 | 0.0150 | 191.661 | `b7ab8b6ce79528329f2495765af9206b7568f1f9f19285ed79431a5e7e303cc6` |
| 8 | 10894–12598 | 1705 | 0.0150 | 196.482 | `7a7d4d4a09042c1374cab0d638bf5245ea128fe07fd676b6d014764d5733399e` |
| 9 | 12599–12738 | 140 | 0.0150 | 46.411 | `c698c619fac565a83a0bd8f32d28a2ebecbf59b456d7cce9641394d1ee4aef06` |
| 10 | 12739–12909 | 167 | 0.0150 | 270.312 | `67c70babea208fd7fc2e8d6bd5bf39cb9f773ca66effc684b48dbffc17e67f3f` |
| 11 | 12910–14643 | 1733 | 0.1000 | 197.087 | `9c959febe80d02023497437d82a1a7f81802b6d5a707d57c3e9af7135ba2ab09` |
| 12 | 14644–16552 | 1909 | 0.1000 | 189.055 | `f21335a0161f7436be42c98a732425a525f50fc8782d726d805d27dcfa7e1955` |
| 13 | 16553–16853 | 299 | 0.0150 | 105.651 | `209cab39f8e421a92dc141f681e1f63bfa36fd7486fef2cffb3040c99bfab63a` |

Reproduction on an otherwise idle host: `cargo xtask replay --docker ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 --keep-reports target/replay --timeout 600 testdata/golden/first_cut/first_cut_linuxcnc.ngc`. The local run used each execution share exactly once (`2/13`, `3/13`, `1/13`, then `4/13` through `13/13`) to check the previously failing ranges first. All share assignments use the same deterministic expansion; CI distributes the full reference/first-cut set over its existing three shares. Raw reports, expected-state/source manifests, simulator INIs, per-attempt timings and the assembled evidence remain temporary local artifacts; the summaries/hashes above preserve their identity in the repository. The Docker image/digest is the one recorded in 10.4.

## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | PH |
| Date | 2026-10-25 |
| Machine and firmware | Makera Z1, 1.1.2 |
| Changes required | Owner: No. Evidence audit: unresolved exceptions above. |
| Reviewed: yes / no | Yes (owner submission); local selected checks and bounded first-cut replay passed; physical evidence and unavailable-platform gaps remain |
