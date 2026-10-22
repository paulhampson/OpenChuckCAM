# Makera Z1 gates 4 and 5: step-by-step test instructions

**Status: draft for the owner's review (lane GATES, 22 Oct 2026), with the owner's answers to OI-910 to OI-919 (22 Oct 2026, relayed by the coordinator).** Written by an agent from the project documents, not from experience of the machine. Nothing in it has been run on a Z1. Where the documents do not say how the Z1 behaves, this document does not guess: it names the question and the open item (section 9). Two things are still to settle before gate 5: the first-cut project's exact geometry (OI-914, 5.2 item G0) and [decision 0221](decisions/0221-gate-5-first-cut-project-and-limit.md) (accepted). The document stays a draft until the owner's gate 4 and 5 results are folded into section 10.

**Generated G-code is the user's to verify before running on a machine (NFR-12).** OpenChuckCAM does not say that a program is safe or correct for any machine, tool or setup, and nothing in this document says otherwise. Passing gates 1 to 3, a clean simulation or a step of these instructions lowers the risk; it does not remove it. You, the operator, decide whether a program runs, and you can stop at any point.

## 1. What this document is for

Gates 4 and 5 are the two hands-on G-code gates of [verification plan section 6](verification-plan.md): gate 4, the **air cut** (the Z1 runs the program with no tool and no material, attended, at reduced feed), and gate 5, the **first cut** (soft, low-cost material, attended, then compared with the model). They are slice 20 item 11 of the [work breakdown](work-breakdown.md), a manual release-checklist step owned by the owner ([decision 0106](decisions/0106-air-cut-is-a-manual-release-step.md)), and the review checklist's "Gates 1 to 5 have passed for the reference job set with results stored, and the G93 result is recorded".

This document turns them into steps that someone who is not a machinist can follow, each with what to do, what to expect, what to record and when to stop. It builds on, and does not replace, two documents the owner has already reviewed (14 Oct 2026, OI-133):

- the [safe-practice checklist](z1-safe-practice-checklist.md): complete its section 1 at the start of every session, section 2 before gate 4, section 3 before gate 5 and section 4 after every run;
- the [air-cut procedure](z1-air-cut-procedure.md): its steps 0 to 7 are the run order of gate 4 (section 4.4 below). Its step 3 is corrected to time a whole operation (OI-910, section 4.5).

Gate 4 runs the reference job set of [reference-jobs.md](reference-jobs.md) on the Makera Z1 post, and the first-cut project's program. Gate 5 cuts the first-cut project of section 5.2 (OI-914), not the reference job set (decision 0221).

**Step IDs.** Every step has an ID (P1 to P10, R1 to R12, S1 to S8, G4-0 to G4-7 for gate 4; F1 to F6, G0, W1 to W5, T0 to T5, V1 to V6, M1 to M6, C1 to C7, I1 to I5 for gate 5). Results are recorded against these IDs on the forms of section 8 and, once the owner has submitted them, folded into section 10 step by step, so each instruction can be read beside what happened when it was run.

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
- **Gate 4 passes** when every program of the reference job set chosen for the release, and the first-cut project's program, has passed, and the `G93` result on the installed firmware is recorded.
- **Fail**: any unexpected motion, alarm, refused line, or a step that does not pass. Follow section 7. Do not go on to gate 5 with that program. A change to the profile, the post or the kinematics sends the affected programs back to gate 1.

### 4.7 What to keep from gate 4

- The filled results form (section 8.1), one row per run.
- The build information (P1), the firmware version and the axis height measurement.
- The exact `.nc` files run and their setup sheets.
- Photos: the empty spindle and chuck (P6), the DRO at work zero (P9), the DRO before and after each unwind.
- Videos of the runs, if taken.
- Any defect raised (section 7), by issue number.

Attach them to one GitHub issue for the gate 4 result (section 7.2), so the verification report (slice 20 item 13) can cite it.

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
| OI-918 | The setup sheet does not show held regions or tabs | Show them, in plain wording (owner, 22 Oct 2026); built in lane GATES: the Setup section's held-region and tab lines and a tab table per setup, HTML and PDF | S8, 5.2 | No |
| OI-919 | Single block on the Z1 | B: no single block; 10 % override with a hand on feed hold | 4.3 | No |

## 10. Results

Empty until the owner submits the results of gates 4 and 5. They are then added here, one row per step ID, by the lane that folds them in, with the issue that holds the evidence; a step that did not pass names its defect issue. A surprise that changes an instruction is fixed in the step above in the same change, and the row says so.

### 10.1 Gate 4

| Run (date, firmware, build) | Program | Step ID | Result (pass / fail / not run) | Notes and measurements | Issue |
| --- | --- | --- | --- | --- | --- |
| | | | | | |

### 10.2 Gate 5

| Run (date, firmware, build) | Program | Step ID | Result (pass / fail / not run) | Notes and measurements | Issue |
| --- | --- | --- | --- | --- | --- |
| | | | | | |

## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | |
| Date | |
| Machine and firmware | |
| Changes required | |
| Reviewed: yes / no | |
