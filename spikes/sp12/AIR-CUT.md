# SP-12 air cut: Makera Z1 rotary conventions and G93

This is the hands-on part of spike SP-12, under the bring-up exception in verification policy 6: a hand-written program, checked by hand and in the LinuxCNC simulator, then run **only as an attended air cut with no tool, no material and reduced feed** (gate 4 rules). Generated or hand-written G-code is yours to verify before it runs on your machine (NFR-12); read `z1_conventions_test.cnc` line by line before you run it.

## Before you start

- [ ] The Z1 is on firmware 1.1.2 (the source this spike read). Note the version you have: 1.1.2
- [ ] The 4th-axis module is fitted and enabled the way Makera's instructions say, with the jaws empty.
- [ ] **No tool in the spindle.** The program has no tool change and never starts the spindle.
- [ ] No material in the chuck, nothing loose on the bed, the enclosure closed, your hand near the stop button for the whole run.
- [ ] Set G54 so that **Y0 Z0 is on the A axis centreline** (Makera's 4th-axis origin), and X0 at least 10 mm to the right of the jaws. The Z1's feed rule assumes Y0 Z0 on the axis, and the expected times below depend on it.
- [ ] With the origin set, jog (or MDI `G0`) to X60 Y0 Z60 and check that nothing is in the way from X0 to X60 at Z30 and Z60. If Z60 is not clear, skip step 4 (delete its lines, or stop at its pause and skip ahead).
- [ ] Put a strip of tape on one jaw so you can see which way the chuck turns, and set A to point it straight up at A0.
- [ ] Have a stopwatch (a phone is fine). Feed override at 100 %.

The program pauses with `M600` before each step; resume from Makera Studio (or the controller) when you are ready. The feeds are low (600 mm/min or less on cutting moves). The only rapid moves are the retract to machine Z-3 at the start and end, and the final `G28`.

## Steps and what to record

| Step | What happens | Expected (from Makera's profiles and firmware source) | What you saw |
| --- | --- | --- | --- |
| 0 | `G92.4 A0 S0`, then A0, X0 Y0, Z30 | No surprise moves; DRO shows X0 Y0 Z30 A0 | As expected. DRO matches. |
| 1 | `G1 A90` | DRO A90. The mark on top moves **toward the front (toward you, -Y)** and ends facing you. Seen from the right-hand end of the axis, the chuck turns counter-clockwise. | Toward the front |
| 1 | `G1 A0` | Back to A0 | As expected, mark on top |
| 2 | `G1 A370 F1200` | One turn plus 10 degrees, the same way as step 1; DRO A370 | As expected |
| 2 | `G92.4 A0 S0` | **No motion**; DRO changes to A10 | DRO: A10 |
| 2 | `G1 A0` | Turns **10 degrees back**, not 370 | Degrees turned: |
| 2 | `G1 A-370 F1200`, `G92.4 A0 S0` | Turns the other way; no motion at G92.4; DRO A-10 | DRO: A-10.001, mark matches |
| 2 | `G1 A0` | Turns 10 degrees forward | As expected |
| T3a | `G1 X60 F300` | **12.0 s** | 12 s |
| T3b | `G1 A360 F600` (A only, radius 30) | **21.8 s** on the Z1 rule; 36.0 s if it behaved like LinuxCNC | 21.6 s |
| T3c | `G1 X20 A540 F600` (X and A, A larger) | **10.9 s** on the Z1 rule; 2.0 s if it behaved like LinuxCNC | 10.9 s |
| T3d | `G1 X60 A570 F300` (X and A, X larger) | **12.0 s** on either rule | 12 s |
| 3 | `G92.4 A0 S0` | No motion; DRO A210 | DRO: A210 |
| T4 | `G1 A570 F600` at Z60 (A only, radius 60) | **36.0 s** on either rule (the firmware stops scaling past a 360 mm "perimeter") | 36 s |
| T5 | `G93`, then `G1 X10 F6` | **100 s (a crawl) if G93 is ignored**, which the firmware source says it is; 10.0 s if G93 works. If it is still crawling after 20 s, feed hold: the answer is clear. | 100 s, run to the end (no feed hold) |
| End | `G94`, X0, unwind, retract, `G28`, `M2` | A unwinds to A0 by at most one turn; clearance move; program ends | As expected |

Result: no alarm, warning or message was shown, the controller accepted every line, and nothing moved in a way the table does not predict (run on firmware 1.1.2, 14 Oct 2026).

Also note: any alarm, warning or message the controller showed, and which line it was on; whether the controller accepted every line; anything that moved in a way the table does not predict.

## What happens with the result

Send the filled-in table (a photo is fine). Each convention in `RESULT.md` then moves from *derived* to *confirmed* or *refuted*, the G93 answer is recorded, and the timing results decide the Z1 feed rule for the post (decision record 0007, open items OI-03, OI-04 and OI-37). Times within about 10 % of the Z1 column confirm the firmware rule; acceleration adds a little to the short moves.
