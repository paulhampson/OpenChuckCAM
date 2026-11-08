Owner check for issue #442: swarf runs past the end of the blade
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project (the blisk starting
   project with Blade wall, face 19, and "1 Blade swarf", 3 mm flat).
2. Operations > Generate; click "1 Blade swarf" and read its note.
3. Simulate; read the overcut.
4. Pattern and tolerance: clear "Keep off the end faces"; Generate stale;
   read the warnings; simulate again. Then tick it again.

Expected: step 2 "The pass stops short of face 5 ... about 0.471 mm ... left
uncut there. The pass stops short of face 15 ... about 0.833 mm ...".
Step 3 deepest overcut about 0.056 mm. Step 4 warnings "Swarf on face 19 cuts
0.283 mm into face 5, which meets it at an end of the pass ..." and an
overcut of 0.300 mm at X 20.
Old behaviour: the path of step 4, with no warning about faces 5, 15 or 1.
Also say whether OI-2079's strip matters to you (see CHECK.md).

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
