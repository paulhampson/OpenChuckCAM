Owner check for issues #639, #640 and #642: a ramp follows the pass, and a
lead on a ring stays in the plane of the cut
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project.
2. Operations, Generate.
3. Look at the start of each operation's path at the head end (X 0).
Expected: all three generated. "Rough, ramp" and "Rings, ramp": the ramp
lies over the pass (on rings, down around the first ring at its X).
"Rings, lead": the arcs turn with the part at the ring's X. Nothing runs
along X towards the chuck; no message about the held region.
Old behaviour: all three refused, e.g. "operation 'Rings, ramp' cannot be
generated: pass 1: the ramp needs a run of 134.860 mm ... would run over
the held region at the start of the stock at X -5.070 mm ...".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
