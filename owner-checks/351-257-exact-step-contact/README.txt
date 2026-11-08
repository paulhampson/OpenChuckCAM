Owner check for issues #351 and #257: Exact STEP surface contact, on and off
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project (shouldered_shaft.step;
   "1 Axial, tapered ball" and "2 Pencil, flat").
2. Operations > Generate. On each operation, under Pattern and tolerance,
   "Exact STEP surface contact" is not ticked; read the notes.
3. Tick it on both; Generate stale; read the notes and moves.
4. Untick it on both and generate: as in step 2.

Expected, off: "... drops the tool on the STEP part's tessellation, made at a
deflection of 0.01 mm, not on its exact faces: Exact STEP surface contact is
off for this operation"; pencil "6 passes".
On: "... drops the tool on the STEP part's exact faces, to within 0.0001 mm,
not on its tessellation; ..."; pencil "31 passes"; both paths change.
Old behaviour: no setting; the paths were those of "off".
If it passes, close #351 and then #257.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
