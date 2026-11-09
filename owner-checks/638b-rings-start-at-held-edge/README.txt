Owner check for decision 0550 (issues #638 and #640): rings start on a
whole ring at a held region's edge

1. Open the folder "project" with File > Open project (the tutorial job,
   the chuck holding 8 mm, its Finish in Rings twice).
2. Operations, Generate.
Expected: 1 Finish rings, sharp and 2 Finish rings, blended both
generate and start on a whole ring at X 9.088, the held region's edge:
the first cutting move turns A. Nothing runs along X before the first
ring.
Old behaviour: Sharp started with a 0.27 mm cut along X at A 0 from
X 9.137 before its first ring; Blended started on a piece of a blend.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on issue #638 with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/638b-rings-start-at-held-edge
on the branch owner-checks/2026.1-alpha.
