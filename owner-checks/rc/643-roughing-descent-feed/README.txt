Owner check for issue #643: a steep drop inside a roughing pass goes down
no faster than the plunge feed
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project (the public test
   blisk, the tutorial's Rough operation).
2. Operations, Generate, Post and export with the LinuxCNC post.
3. python3 gcode_descent.py <file>.ngc 300 (on the branch).
Expected: 0 moves faster down the axis than 300 mm/min.
Old behaviour: 193 such moves, the fastest 601.6 mm/min.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
