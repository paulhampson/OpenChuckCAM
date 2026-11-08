Owner check for issue #604: the default tab and "Add default tabs" follow
a stock change at once
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project.
2. Setup, Setup 1, Inspector > Workholding: "No room for a default tab at
   the head ..." and Add default tabs greyed.
3. Stock: Length 78 (Enter), Start X -13 (Enter).
Expected, straight away: "Head zone X -5.000 to 0.000 mm (5.000 mm)", the
line "Default tab, not added · Head · Frustum · X -5.000 to 0.000 ·
Ø 14.000 to Ø 7.000 mm", and Add default tabs enabled.
Old behaviour: the "No room" text and the greyed button stay until the
project is saved and opened again.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
