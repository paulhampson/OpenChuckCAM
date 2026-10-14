# Safe-practice checklist for first Makera Z1 trials

**Status: reviewed and accepted by the project owner, who operates the Z1 (14 Oct 2026), with no changes.** Written by the agent from the project documents ([verification plan section 6](verification-plan.md), [decision 0007](decisions/0007-sp-12-z1-conventions.md), [reference-jobs.md](reference-jobs.md)), not from experience of the machine. Work breakdown slice 20 item 12 required it to be reviewed by whoever operates the Z1 (OI-133, closed). The operator should still correct anything that proves not to match the machine and record the change in the review table.

**Generated G-code is the user's to verify before running on a machine (NFR-12).** OpenChuckCAM has never run a generated program on a Z1 as of this draft. Passing the project's own gates reduces the risk; it does not remove it.

Use with the [air-cut procedure](z1-air-cut-procedure.md). Tick each box; stop at the first one you cannot tick.

## 1. Before every session

- [ ] I know which program I am about to run, which application version made it and which post (Makera Z1, not the LinuxCNC verification post).
- [ ] I have read the program from the first line to the last, or at least every line up to the first cut and the end of the program, and nothing in it surprises me.
- [ ] The header comment, units (mm), tool number and safe Z match what I expect.
- [ ] The machine profile in the project matches the machine (travel, axis height, A direction). The axis height is a measured value, not the assumed one (OI-60).
- [ ] Firmware version noted. If it differs from 1.1.2, the SP-12 conventions are rechecked before anything else.
- [ ] I am attended, and will stay beside the machine for the whole run.
- [ ] The emergency stop is within reach of my hand while the program runs.
- [ ] The enclosure is closed and nothing is on the bed or in the machine that should not be.
- [ ] I know how to stop (stop button, feed hold) and what the machine does when I use each.

## 2. Gate 4: air cut (no tool, no material)

- [ ] **No tool is fitted and no material is held.** The jaws are empty or closed on nothing.
- [ ] Feed override is at or below 10 % for the first run of each program.
- [ ] G54 is set with Y0 Z0 on the A axis and X0 where the program expects it (OI-40), clear of the jaws.
- [ ] I have checked by hand, at reduced feed or by jogging, that the safe Z clears the chuck, jaws and the A axis assembly over the whole X range the program uses.
- [ ] I watch the first 20 lines of each program with my hand near the stop.
- [ ] The A axis moves the direction the post expects (A+ turns the top of the chuck toward the front, -Y; confirmed 13 Oct 2026).
- [ ] At the `G92.4 A0 S0` unwind at the end of a program, A does **not** move.
- [ ] Any unexpected motion, alarm or rejected line stops the trial. I log it as a defect with the G-code and the application commit attached, and do not continue.
- [ ] I record each run on the air-cut sheet.

## 3. Gate 5: first cut (wax or foam), only after gate 4 has passed

- [ ] The air cut for this program passed, with the sheet stored.
- [ ] The first-cut limit has been set from the machine's measured repeatability on a 3-axis test part (start at ±0.1 mm).
- [ ] The material is soft and low-cost (wax or foam), held firmly and square in the chuck. I have checked it cannot slip or loosen as A turns.
- [ ] The stock is no larger than the stock in the project, and the stock position (X0, centre on the axis) is checked by hand.
- [ ] The tool is the tool in the project (3 mm ball end mill, 8 mm flute length, for the reference jobs), fitted with the correct stick-out and tightened. The tool number in the program is the tool in the spindle.
- [ ] The tool change (`M5`, `T<n> M6`) has been tested separately and attended, or I change the tool by hand and have checked the program's tool change lines (OI-41).
- [ ] Spindle speed, plunge and cutting feeds are what I expect and within the machine's limits (profile: 1200 mm/min X and Y, 600 Z, 3600 deg/min A).
- [ ] Feed override starts low and goes up only while the cut sounds and looks right.
- [ ] Eye and ear protection on; loose clothing, long hair and jewellery secured away from the chuck; dust and chips managed as for the material.
- [ ] I stay at the machine for the whole cut and stop at any unexpected sound, vibration, motion or smell.

## 4. After every run

- [ ] Spindle stopped and everything stationary before I open the enclosure or touch anything.
- [ ] Results recorded: program, commit, override, outcome, alarms, measurements against the model (limits in verification plan 4.1).
- [ ] Anything that surprised me is written down, with the program attached, even if the run was otherwise fine.
- [ ] If I change the profile, the post or the kinematics, I go back to gate 1 for the affected programs and do not reuse an earlier pass.

## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | Paul Hampson (project owner) |
| Date | 14 Oct 2026 |
| Machine and firmware | Makera Z1; firmware not recorded in the review (note it at the first trial) |
| Changes required to this checklist | None |
| Reviewed: yes / no | Yes |
