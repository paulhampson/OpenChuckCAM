# Makera Z1 gates 4 and 5: step-by-step test instructions

**Status: draft for the owner's review (lane GATES, 22 Oct 2026).** Written by an agent from the project documents, not from experience of the machine. Nothing in it has been run on a Z1. Where the documents do not say how the Z1 behaves, this document does not guess: it names the question and the open item (OI-910 to OI-919, section 9). Until the owner has answered the questions marked **Owner decision pending**, the steps that depend on them are not to be run.

**Generated G-code is the user's to verify before running on a machine (NFR-12).** OpenChuckCAM does not say that a program is safe or correct for any machine, tool or setup, and nothing in this document says otherwise. Passing gates 1 to 3, a clean simulation or a step of these instructions lowers the risk; it does not remove it. You, the operator, decide whether a program runs, and you can stop at any point.

## 1. What this document is for

Gates 4 and 5 are the two hands-on G-code gates of [verification plan section 6](verification-plan.md): gate 4, the **air cut** (the Z1 runs the program with no tool and no material, attended, at reduced feed), and gate 5, the **first cut** (soft, low-cost material, attended, then compared with the model). They are slice 20 item 11 of the [work breakdown](work-breakdown.md), a manual release-checklist step owned by the owner ([decision 0106](decisions/0106-air-cut-is-a-manual-release-step.md)), and the review checklist's "Gates 1 to 5 have passed for the reference job set with results stored, and the G93 result is recorded".

This document turns them into steps that someone who is not a machinist can follow, each with what to do, what to expect, what to record and when to stop. It builds on, and does not replace, two documents the owner has already reviewed (14 Oct 2026, OI-133):

- the [safe-practice checklist](z1-safe-practice-checklist.md): complete its section 1 at the start of every session, section 2 before gate 4, section 3 before gate 5 and section 4 after every run;
- the [air-cut procedure](z1-air-cut-procedure.md): its steps 0 to 7 are the run order of gate 4 (section 4.4 below). Two statements in its step 3 do not match the software (OI-910, section 4.5).

The programs are the reference job set of [reference-jobs.md](reference-jobs.md) on the Makera Z1 post.

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
- The project folders for the reference jobs, or their golden programs in `testdata/golden/reference/` (which one: OI-913, section 4.1 step P2).
- The setup sheet (HTML or PDF) written with each program (decision 0189), printed or on screen beside the machine.
- The Makera Z1 with its rotary module and chuck, and its controller software. Know where its emergency stop, feed hold and feed override are before you start.
- A stopwatch (a phone is fine), a camera (a phone is fine), masking tape and a marker.
- For gate 5: the material (OI-915), the tool, a caliper or micrometer, eye and ear protection, and a way to manage dust and chips for the material.
- The results forms of section 8, printed or copied into a file.

## 4. Gate 4: the air cut

Gate 4 passes when, for each program run, the controller accepts every line, raises no alarm, the motion matches what the simulator showed, and the timed move agrees with the predicted time within the band set at the first run ([verification plan section 6](verification-plan.md)). It also records the `G93` result on the installed firmware.

### 4.1 Preparation

Do these at the computer and at the machine before anything moves. Each step has a pass condition. **If a step does not pass, stop and do not run any program** (section 7).

| Step | Do | Pass when | Record |
| --- | --- | --- | --- |
| P1 Software build | Open OpenChuckCAM, choose **Help > About OpenChuckCAM** and press **Copy build information**. Paste it into the results form (section 8.1). From a terminal, `openchuck-cli --version` gives the same text. | The version is the one named for the trial, the commit is shown, and **Uncommitted changes: no** (NFR-16). The About window also shows the notice that generated G-code must be verified by the user. | The pasted build information |
| P2 Project and programs | **Owner decision pending (OI-913).** The reference jobs exist today as test code and as golden programs, not as project folders you can open in the application. Until OI-913 is answered, use the golden Z1 programs `testdata/golden/reference/*_makera-z1.nc` from the commit in P1 and note that they were written by the test suite, not by the build in P1. | You have the `.nc` files of the jobs you will run, from the commit in P1. | File names and the commit |
| P3 Machine profile | In the application, open the project's setup and its machine values (or `openchuck-cli machine <project>`). Each value is marked confirmed, vendor-supplied or unconfirmed (UI-13). | The profile is **Makera Z1**. Travel 200 x 200 x 100 mm, feeds 1200 mm/min X and Y, 600 Z, 3600 deg/min A, A continuous and right-handed about +X. The axis height is the measured value from P8, not the placeholder (OI-60). | Each value, and which are unconfirmed |
| P4 Post | Check the post chosen for export. | **Makera Z1** (files end `.nc`), not the LinuxCNC verification post (`.ngc`). Feed mode: units per minute; the Z1 post never writes `G93`. | Post name |
| P5 Firmware | On the Z1 controller, find the firmware version. | Noted. If it is not 1.1.2, the SP-12 conventions (decision 0007) must be rechecked first: step 1 of section 4.4. | Firmware version |
| P6 Tool and stock | Take any tool out of the spindle. Empty the chuck, or close the jaws on nothing. Clear the bed. | **No tool, no material.** Nothing loose on the bed or in the enclosure. | Photo of the empty spindle and chuck |
| P7 Spindle and tool change | **Owner decisions pending (OI-911, OI-912).** Every generated program has `M5`, `T1 M6` and `S... M3` near its start. With no tool fitted, decide before running how the spindle start and the tool change are handled. | The owner's decisions on OI-911 and OI-912 are recorded and followed. | The decisions |
| P8 Axis height | **Owner decision pending (OI-917).** Measure the machine Z of the A axis centreline (OI-60). OI-60 suggests touching the tool tip to the chuck centre and reading the machine (G53) Z; that needs a tool, so it is done before P6. If it differs from machine Z -55, correct the project's machine profile and regenerate (P2). | A measured value is in the profile. | Measured axis height (mm), how it was measured |
| P9 Work zero | **Owner decision pending (OI-917).** Set G54 with Y0 Z0 on the A axis centreline and X0 at the part's model origin, at least 10 mm clear of the jaws (OI-40). Note where A is zero. | The DRO in G54 reads Y0 Z0 at the axis centre and X0 where the setup sheet's "Work offset" line says. | G54 X, Y, Z in machine coordinates; how X0 was found |
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

The setup sheet does not list the held regions or the tabs (OI-918). For gate 5 read them in the application (section 5.2).

### 4.3 Safeguards for every run

- Feed override at **10 %** for the first run of each program (air-cut procedure).
- Your hand near the feed hold and the emergency stop for at least the first 20 lines, and in reach for the rest.
- **Owner decision pending (OI-919):** whether the Z1 controller can step one line at a time (single block) for the first moves, or only feed hold at 10 % is used.
- The enclosure closed. Watch through it; do not reach in while anything can move.
- A strip of tape on one jaw so you can see A turn.
- Film each run if you can (the first 20 lines and the end block at least).

### 4.4 Run order

Follow the [air-cut procedure](z1-air-cut-procedure.md) steps 0 to 7 in order, with these additions:

| Step | Program | What to watch | Pass when |
| --- | --- | --- | --- |
| 0 | Read `sphere_makera-z1.nc` (section 4.2) | | Every R and S item ticked |
| 1 | SP-12 conventions program (`spikes/sp12/AIR-CUT.md` on `spike/sp-12-z1-conventions`), only if the firmware is not 1.1.2 | Direction, wrap, feed rule as decision 0007 records | The results match decision 0007 |
| 2 | `sphere_makera-z1.nc` at 10 % | The first rapid goes to safe Z before any other motion. X, Z and A move together during the pass. The tape turns the way the A words say (A+ turns the top toward the front). The end block: up to Z near the top, then `G28` (watch where it goes and write it down: not yet seen, OI-41) | No alarm, no line refused, the motion looks like the simulation's playback of the same program, the machine ends where the program says |
| 3 | Timed move | **Owner decision pending (OI-910).** No single move in the reference programs is long enough to time by hand (section 4.5) | As decided on OI-910 |
| 4 | `cone_makera-z1.nc` at 10 % | Z changes steadily with X | As step 2 |
| 5 | `groove_makera-z1.nc` at 10 % | About 15 turns of A, then the unwind. **At `G92.4 A0 S0` A must not move**; the DRO's A reading changes to the remainder. Note the DRO before and after | As step 2, and A does not move at the unwind |
| 6 | `G93` recheck (SP-12 test T5) only if the firmware is not 1.1.2 | | The result is recorded (verification plan section 6, feed mode) |
| 6b | OI-267: an A-only fed move with an F above 1200 | **Owner decision pending (OI-910 covers the timing; the program for this test is not written).** The owner asked for it on a later air cut before the first release (17 Oct 2026) | The controller's behaviour (clamps, refuses or runs it) is recorded in OI-267 |
| 7 | Raise the override (25 %, 50 %, 100 %) on the sphere only | Only if steps 2 to 5 had no surprise and you agree | As step 2 |

The other reference programs (`lobed`, `cone_axial`, `cone_indexed`, `cone_axial_lead`, `cone_held_tabs`, `drilling`, `swarf`) are run the same way as step 2 once steps 2 to 5 have passed. `lobed` is the one program that moves X, Y, Z and A together; `drilling` turns A to -30 and has no unwind; `swarf` turns to three A positions.

### 4.5 The timed move (OI-910)

The verification plan asks for one simultaneous XYZA move whose measured time is compared with the predicted time under the Z1's feed rule. Two things stop the air-cut procedure's step 3 being followed as written:

- **The moves are too short to time by hand.** The reference programs are made of many short moves. Computed from the golden programs with decision 0007's rule, the longest move that turns A while X, Y or Z also move takes about 0.13 s at 100 % feed (`groove`), 1.3 s at 10 %; a stopwatch cannot time that to the 10 % the SP-12 run reached.
- **The predicted time is not where step 3 says.** The program header does not state a predicted total; each operation's comment does ("about N s", the programmed time of its fed moves). `openchuck-cli gcode-sim` compares a LinuxCNC simulator report with a LinuxCNC program and gives no time per Z1 move.

Until OI-910 is answered, do not count step 3 as passed. The recommended method (for the owner to decide) is to time a whole operation: start the stopwatch when the first fed move of the operation starts (the plunge) and stop it when the last fed move ends (the rapid up to safe Z starts), and compare it with "about N s" multiplied by 100 divided by the override (for 10 %, ten times). For the sphere that is about 76 s at 100 %, about 12.5 minutes at 10 %. Agree the band at the first run (verification plan section 6).

### 4.6 Gate 4 pass and fail

- **Pass** for a program: every preparation step passed; every R and S item ticked; no alarm; no line refused; the motion matched the simulation; A did not move at the unwind; the end position matched the program; the timed operation was within the agreed band (step 3, once OI-910 is decided).
- **Gate 4 passes** when every program of the reference job set chosen for the release has passed, and the `G93` result on the installed firmware is recorded.
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

Gate 5 cuts soft, low-cost material (wax or foam), attended, and compares the result with the model. It passes when the part matches the model within the limit in [verification plan 4.1](verification-plan.md) (start at ±0.1 mm; set the limit from a 3-axis test part first) and the findings are recorded.

### 5.1 Before gate 5

| Step | Do | Pass when |
| --- | --- | --- |
| F1 | Gate 4 | The program you will cut passed gate 4, at the same commit, with the same profile and post. Any change since: back to gate 1. |
| F2 | Safe-practice checklist section 3 | Every box ticked. |
| F3 | Tool change test (OI-41) | The tool change (`M5`, `T<n> M6`) has been tested attended with a tool, or you change the tool by hand and have read the program's tool change lines. |
| F4 | 3-axis test part | **Owner decision pending (OI-916).** No document says what the 3-axis test part is or how its result sets the first-cut limit. Until then the limit is ±0.1 mm. |
| F5 | Choice of job | **Owner decision pending (OI-914).** Most reference jobs have stock only 2 mm longer than the part and no held region, so the stock cannot be gripped without the tool reaching the jaws. `cone_held_tabs` is the reference job that models the grip (held 4 mm and 2 mm) and the tabs. |
| F6 | Material and cutting data | **Owner decision pending (OI-915).** The reference jobs use oak cutting data (12000 rpm, 0.02 mm per tooth, plunge 150 mm/min). Which wax or foam, and which speeds and feeds, is not in the documents. |

### 5.2 The project: workholding and tabs

In the application, select the setup and open **Workholding and tabs** (specification section 11.7). The setup sheet does not show these (OI-918), so read them here and write them on the form.

| Step | Check | Pass when |
| --- | --- | --- |
| W1 | Held region at the start and at the end: grip length, and holding diameter if one is given | The grip length is the length the chuck will really grip (measure the jaw depth). With no holding diameter the held region is the stock's own section (decision 0160). |
| W2 | The grip warning | No amber warning under **Grip length**. If there is one, it names how much of the part the held region covers (OI-554): that length of the part will **not** be machined. Go on only if you meant that. |
| W3 | Tabs | Each tab's position, length and diameter (a frustum has two). Head and tail tabs made by the buttons are placed from the part and the stock and recalculated while ticked (decision 0202). A tab with no room shows a warning. |
| W4 | Setup checks and messages | No error. Read every warning and write it down. "Safe Z cannot be checked against the Z travel" means the axis height is still the placeholder (OI-60): go back to gate 4 P8. |
| W5 | Stock | The stock in the project is no smaller than the real stock you will load, and its X range puts the gripped length where the chuck is. |

### 5.3 Tool and speeds

| Step | Check | Pass when |
| --- | --- | --- |
| T1 | The tool in the project (setup sheet Tools table) | It is the tool you will fit: type, diameter, flute length. The tool number matches the program's `T<n>`. |
| T2 | Stick-out | The tool sticks out of the holder at least as far as the deepest cut plus a margin, and no further than needed. The Z1 measures tool length itself (decision 0007: no `G43`); follow the machine's own procedure. |
| T3 | Spindle speed | The setup sheet's Spindle rpm and **Speed from** (R6): the value is what you chose for the material (OI-915), not the oak value carried over by accident. If you change it, regenerate and reread the program. |
| T4 | Feeds | The operation's cutting feed and plunge feed are what you chose for the material, and the program's F words are within the profile's limits (R10). |

### 5.4 Simulate before cutting

| Step | Do | Pass when |
| --- | --- | --- |
| V1 | Run **Simulate** in the application (or `openchuck-cli simulate <project>`), at the default cell (0.1 mm) or finer | It completes. No banner saying the result is out of date (decision 0192). |
| V2 | Overcut | The results inspector shows no overcut beyond the tolerance, and the command line reports no "OVERCUT" and no rapid move that removes material. |
| V3 | Material left | Write down the largest material left and where it is (the probe and "Go to largest left" show it). Material left on a held region or tab face is not counted; a cut **into** one is (decision 0194). |
| V4 | Held regions and tabs | They stay whole in the simulated stock (SET-09, OP-21). |
| V5 | Playback | Play the program through. The tool stays clear of the chuck end of the stock (the gripped length): no fed move runs along it (decision 0161). |
| V6 | Keep | A screenshot of the coloured part, the histogram and the worst values, for the comparison in 5.7. |

The simulation is a check on the program, not a promise about the machine (NFR-12): it does not model the chuck, jaws or the machine's own errors.

### 5.5 On the machine: stock and offsets

| Step | Do | Pass when |
| --- | --- | --- |
| M1 | Measure the stock (diameter, length) | It matches the project's stock within what you consider safe; never larger than the project's stock. |
| M2 | Load the stock in the chuck with the gripped length the project says (W1); tighten | It does not move when you try to turn or pull it by hand. Turn A by jog a full turn at low feed: it turns true by eye and nothing touches. |
| M3 | Fit the tool (T1, T2) | The tool number in the controller is the program's `T<n>`. |
| M4 | Set and check work zero as in gate 4 P9 (OI-917) | G54 Y0 Z0 on the axis centreline; X0 matches the setup sheet's work offset line, measured from the stock's end face the owner chooses. |
| M5 | Check by hand that the first rapid position is clear: jog to the program's first `G0` position in G54 (X, Y0, safe Z, A0) at low feed | The tool is above the stock at the X the program starts at, and clears the jaws. |
| M6 | Protection | Eye and ear protection on; loose clothing, hair and jewellery secured; dust and chips managed for the material. |

### 5.6 The first cut

1. Reread the program (section 4.2) one more time. It must be the file you air-cut in gate 4, regenerated only if you changed the tool, speeds or stock, in which case read it again from the start.
2. Feed override at 10 %. Hand near feed hold and emergency stop.
3. Start. Watch the tool change and spindle start (OI-41, OI-911).
4. Watch the first plunge: the tool comes down at the plunge feed to the first cut, no faster.
5. Raise the override slowly, only while the cut sounds and looks steady.
6. Stop at any unexpected sound, vibration, smell, smoke, or motion (section 7). For wax: melting or smearing; for foam: tearing or the stock moving.
7. At the end, wait until everything has stopped before opening the enclosure.

### 5.7 Inspect against the model and the simulation

| Step | Do | Pass when |
| --- | --- | --- |
| I1 | Look at the part before you remove it | Tabs whole, no marks on the gripped length (the tool kept clear of it, decision 0161), no gouges. Photograph it from four sides (A 0, 90, 180, 270). |
| I2 | Measure diameters at stations along X (for the cone, at least 5 stations from end to end, away from the tabs) | Each is within the first-cut limit (F4) of the model's diameter at that X. |
| I3 | Length and positions | The part's length and the tabs' positions match the setup within the limit. |
| I4 | Compare with the simulation (V3, V6) | Where the simulation showed material left, the part has it too, about as much; where it showed none, there is none. A difference larger than the limit is a finding. |
| I5 | Surface | Write down anything that looks wrong: steps, chatter, a seam where A wraps. |

### 5.8 Gate 5 pass and fail

- **Pass**: every check in 5.1 to 5.5 passed; the cut ran with no unexpected event; I1 to I4 within the limit; the findings recorded.
- **Fail**: any step above, or any measurement outside the limit. Follow section 7. A machine-side cause (stock slipped, wrong offset) is recorded and the cut repeated; a cause in the program is a defect.

### 5.9 What to keep from gate 5

- The filled results form (section 8.2).
- The gate 4 issue number for the same program.
- The program, setup sheet and simulation screenshots (V6).
- Photos of the setup in the machine (stock in the chuck, tool), and the finished part from four sides.
- The measurements and the instrument used.

## 6. One-page checklists

### 6.1 Gate 4 (air cut)

- [ ] Safe-practice checklist sections 1 and 2 complete.
- [ ] Owner decisions OI-910, OI-911, OI-912, OI-913, OI-917 and OI-919 answered and followed.
- [ ] P1 build information pasted; uncommitted changes: no.
- [ ] P2 programs from that commit.
- [ ] P3 profile is Makera Z1; unconfirmed values noted.
- [ ] P4 Makera Z1 post, `.nc`.
- [ ] P5 firmware noted (1.1.2, or SP-12 rechecked).
- [ ] P6 no tool, no material; photo.
- [ ] P7 spindle and tool change handled as decided.
- [ ] P8 axis height measured and in the profile.
- [ ] P9 G54: Y0 Z0 on the axis, X0 as the setup sheet says.
- [ ] P10 safe Z and X range clear by jogging.
- [ ] Program read: R1 to R12 and S1 to S7 ticked.
- [ ] Override 10 %, hand on the stop, enclosure closed, tape on the jaw.
- [ ] Sphere, cone, groove run: no alarm, no refused line, motion as simulated.
- [ ] Groove unwind: A did not move.
- [ ] Timed operation within the band (OI-910).
- [ ] `G93` result recorded; OI-267 test run.
- [ ] Results form filled; issue raised with the attachments.

### 6.2 Gate 5 (first cut)

- [ ] Gate 4 passed for this program, same commit, profile and post.
- [ ] Safe-practice checklist section 3 complete.
- [ ] Owner decisions OI-914, OI-915 and OI-916 answered; first-cut limit set.
- [ ] Tool change tested (OI-41), or done by hand with the program's lines read.
- [ ] Workholding: grip length matches the chuck; no grip warning, or understood; tabs as intended.
- [ ] Tool, stick-out, spindle speed and Speed from, feeds checked.
- [ ] Simulated: up to date; no overcut; no rapid removes material; held regions and tabs whole; screenshots kept.
- [ ] Stock measured, loaded to the grip length, turns true.
- [ ] G54 and X0 checked; first rapid position jogged and clear.
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
G54 in machine coordinates X / Y / Z:     X0 found by:
Decisions followed: OI-910 / 911 / 912 / 913 / 917 / 919
```

| Program | Read (R1-R12, S1-S7) | Override % | Alarms or refused lines (line) | Motion as simulated | Unwind: DRO A before / after, moved? | End position as program | Timed: predicted s x 100/override, measured s, ratio | Pass / fail | Issue |
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
Material:                                 Stock measured (diameter x length):
Grip length (start / end):                Tabs:
Tool (number, type, diameter, stick-out):
Spindle rpm and Speed from:               Cutting feed / plunge feed:
First-cut limit (mm) and where it came from:
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

## 9. Open questions for the owner

Each is an open item in [open-items.md](open-items.md). The ones marked *decision* have been put to the owner through the coordinator, one at a time, with options and a recommendation; this document is updated with the answers.

| OI | Question | Blocks |
| --- | --- | --- |
| OI-910 | How the timed XYZA move is measured, since no single generated move is long enough to time by hand and the predicted time is not where the air-cut procedure's step 3 says (*decision*) | Gate 4 step 3 |
| OI-911 | Whether the spindle runs during the air cut (the programs start it with `S... M3`) (*decision*) | Gate 4 P7 |
| OI-912 | How `T1 M6` is handled with no tool fitted (*decision*) | Gate 4 P7 |
| OI-913 | Where the owner gets the reference jobs to open in the release build, regenerate and read with their setup sheets (*decision*) | Gate 4 P2 |
| OI-914 | Which job or jobs gate 5 cuts, given that only `cone_held_tabs` models the grip (*decision*) | Gate 5 F5 |
| OI-915 | Which wax or foam, and which speeds and feeds, for gate 5 (*decision*) | Gate 5 F6, T3, T4 |
| OI-916 | What the 3-axis test part is, how it is measured and how its result sets the first-cut limit (*decision*) | Gate 5 F4 |
| OI-917 | How the axis height and work zero are found on the Z1: which tool or probe, which face X0 is taken from, and where A is zero (*decision*) | Gate 4 P8, P9; gate 5 M4 |
| OI-918 | The setup sheet does not show the held regions and tabs, so the printed sheet is not enough to set up gate 5 | Gate 5 5.2 (worked around by reading the application) |
| OI-919 | Whether the Z1 controller can run one line at a time (single block) for the first moves (*decision*) | Gate 4 4.3 |

## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | |
| Date | |
| Machine and firmware | |
| Changes required | |
| Reviewed: yes / no | |
