Owner check for issue #604 (fixed in #675): the default tab and "Add
default tabs" follow a stock change at once
Build: commit 07ec6990 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project.
2. Setup, Setup 1, Inspector > Workholding: "No room for a default tab at
   the head: the held region reaches the part." Add default tabs greyed.
3. Stock: Length 78 (Enter), Start X -13 (Enter).
Expected, straight away: "Head zone X -5.000 to 0.000 mm (5.000 mm)",
"Default tab, not added · Head · Frustum · X -5.000 to 0.000 · Ø 14.000
to Ø 7.000 mm", no "No room ... at the head", Add default tabs enabled
(it adds the head tab only: the part comes to a point at the tail).
4. Stock back to 63 from -1: "No room ..." and the button greyed again.
Old behaviour: the "No room" text and the greyed button stayed until the
project was saved and opened again.
Not yet clicked by anyone: checked from the code and its test only.

Nothing in this check shows that a program is safe to run. Generated
G-code must be verified by you before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
