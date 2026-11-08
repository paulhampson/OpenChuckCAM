Owner check for issue #556: zooming with a large selection set shown
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

The project is a dense copy of the public baluster: 504,000 triangles, and
the set "Everything but the ends" of 468,000 triangles.

1. Open the folder "project" with File > Open project.
2. Click Operations, then Front and Fit.
3. Under Selection sets, click "Everything but the ends" (a pause of up to
   about a second here is expected: OI-2050).
4. Zoom in and out with the wheel and move the pointer over the part.
5. Hide the set and zoom again, to compare.

Expected: as smooth with the set shown as without it.
Old behaviour: each zoom step froze for about a second (873 ms a frame on
your carving). Please also try your carving.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
