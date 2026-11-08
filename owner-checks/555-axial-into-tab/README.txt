Owner check for issue #555: axial finishing runs past the selection onto the tab
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project.
2. Click Operations, then Generate (a few seconds).
3. Click Front above the view and zoom in on the left end of the part (X 0),
   where the tenon meets the 11 mm tab.

Expected: every pass of "1 Tenon side" ends on the tenon's side at X 0; none
goes over the rim, down the end face or along the tab.
Old behaviour: passes rolled over the rim and ran down onto the tab, up to
X -0.916, at the tab's radius (5.5 mm).

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
