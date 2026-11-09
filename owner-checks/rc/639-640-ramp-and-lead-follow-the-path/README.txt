Owner check for issues #639 and #640: a ramp follows the pass, and a lead
on a ring stays in the plane of the cut
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project (the tutorial job,
   five operations, 3 degree ramps).
2. Operations, Generate.
Expected: 1 Rough rings, ramp: generated (single points plunged, as Allow
single-point plunge asks). 2 Rough spiral, ramp: refused, "... 17.6
turns, more than the 10 allowed ...". 3 Rough spiral, steeper: generated,
the ramp near the axis steeper. 4 Finish rings, ramp: generated, the tool
descends on the axis while the part turns. 5 Finish rings, lead:
generated, the lead turns with the part at the ring's X.
Each ramp comes down over the pass, around the ring at its X; nothing runs
along X towards the chuck.
Old behaviour: the new settings unknown; ramps and leads along X refused
at the held region ("... over the held region ... at X 7.604 mm ...").

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
