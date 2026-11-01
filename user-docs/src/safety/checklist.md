# Safe-practice checklist

Work through this list before and after every run on a Makera Z1. Stop at the first item you cannot tick.

> [!NOTE]
> This page adapts the project's checklist for its own machine trials, [`docs/z1-safe-practice-checklist.md`](https://github.com/paulhampson/OpenChuckCAM/blob/main/docs/z1-safe-practice-checklist.md), which the project owner reviewed as the operator of a Z1. That document is the authority: where the two differ, follow it, and tell the project. Most items apply to any rotary machine, but the values given are for the Z1.

## 1. Before every session

- [ ] I know which program I am about to run, which version of OpenChuckCAM made it, and which post (Makera Z1, not the LinuxCNC post).
- [ ] I have read the program, at least every line up to the first cut and the end, and nothing in it surprises me.
- [ ] The header comment, the units (mm), the tool numbers and the safe Z match what I expect.
- [ ] The machine profile in the project matches the machine: travel, height of the rotary axis, direction of A. The axis height is the value I measured on this machine.
- [ ] I have noted the firmware version.
- [ ] I will stay beside the machine for the whole run.
- [ ] The emergency stop is within reach of my hand while the program runs.
- [ ] The enclosure is closed, and nothing is on the bed or in the machine that should not be.
- [ ] I know how to stop the machine (the stop button, feed hold) and what it does when I use each.

## 2. Air cut (no tool, no material)

- [ ] No tool is fitted and no material is held.
- [ ] The feed override is at or below 10 % for the first run of each program.
- [ ] G54 is set with Y0 and Z0 on the rotary axis, and X0 where the program expects it, clear of the jaws.
- [ ] I have checked by hand, at reduced feed or by jogging, that the safe Z clears the chuck, the jaws and the rotary axis assembly over the whole length the program uses, and that, with the longest tool the program uses, the spindle stays within the top of its Z travel at the safe Z.
- [ ] I watch the first 20 lines with my hand near the stop.
- [ ] A turns the way the post expects: positive A turns the top of the chuck towards the front.
- [ ] At the unwind at the end of the program (`G92.4 A0 S0`), A does not move.
- [ ] Any unexpected move, alarm or rejected line stops the trial. I write it down, with the program, and do not go on.
- [ ] I record each run (see [Air cut on the Makera Z1](air-cut.md)).

## 3. First cut, only after the air cut has passed

- [ ] The air cut for this program passed, and I kept its record.
- [ ] The material is soft and low-cost (wax, foam or tooling board), held firmly and square in the chuck, and cannot slip or loosen as A turns.
- [ ] The stock is no larger than the stock in the project, and its position along X and on the axis is checked by hand.
- [ ] The tool is the tool in the project, fitted with the right stick-out and tightened. The tool number in the program is the tool in the spindle.
- [ ] I have tested the tool change separately while watching, or I change tools by hand and have checked the program's tool change lines.
- [ ] Spindle speed, plunge and cutting feeds are what I expect and within the machine's limits.
- [ ] The feed override starts low and goes up only while the cut sounds and looks right.
- [ ] Eye and ear protection are on; loose clothing, long hair and jewellery are kept away from the chuck; dust and chips are dealt with as the material needs.
- [ ] I stay at the machine for the whole cut, and stop at any unexpected sound, vibration, move or smell.

## 4. After every run

- [ ] The spindle has stopped and nothing moves before I open the enclosure or touch anything.
- [ ] I have written down the program, the software version, the override, what happened, any alarm, and how the part measures against the model.
- [ ] Anything that surprised me is written down with the program attached, even if the run was otherwise fine.
- [ ] If I change the machine profile or the post, I treat every affected program as new and start again from the air cut.
