Owner check for issue #639 (fixed in #663): a ramp entry follows the pass
Build: commit fa1c2de1 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project (the tutorial job,
   four operations, 3 degree ramps).
2. Operations, Generate.
Expected: 1 Rough rings, ramp: generated, two grouped notes (49 passes
back and forth; 26 single points plunged). 2 Rough spiral, ramp: refused,
"... 17.6 turns, more than the 10 allowed ...". 3 Rough spiral, steeper:
generated, ramp up to 3.69 degrees near the axis. 4 Finish rings, ramp:
generated, "pass 4 starts on the rotary axis ... descends there while the
part turns 3.64 turns". Each ramp comes down over its pass, around the
ring at its X; nothing runs along X towards the chuck.
3. On 1, untick Allow single-point plunge: refused, naming it.
Old behaviour: ramps ran along X and were refused at the held region.
Plunge to remove face nubs changes no public program (see CHECK.md).

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
