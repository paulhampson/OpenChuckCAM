Owner check for issue #512: face-flow finishing (OP-28)
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project (the blisk with Hub
   floor, Floor guide, Blade wall and "1 Floor finish", face-flow finishing).
2. Operations, Front, Fit, Generate (about half a minute).
3. Click "1 Floor finish": look at the passes and read Results and Messages.
4. Simulate.
5. Try your own guide edges, stepovers and floors.

Expected: passes along the blade's foot, stepping across the floor. "face 20
is cut from its guide, edge 65 (49.1 mm long), towards the opposite side in
17 passes up to 0.953 mm apart along the face"; an unreached strip of
208.6 mm2 reported; 2,133 moves; no overcut.
Old behaviour: a new strategy; the build before it refused the project.
Open for you: OI-2004 (unreachable floor) and OI-2053 (guide edges in view).

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
