Owner check for issue #637 (fixed in #652): unreachable areas are hatched
on the surface
Build: commit 01aa5e32 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project (the #512 project).
2. Operations, Generate, click "1 Floor finish" (Show menu: Unreachable
   areas on). Follow the link of "Unreached areas (operation 'Floor
   finish'): ..." in Messages.
3. Look close up, turn the part, zoom far out and back.
Expected: dark diagonal lines with a white edge, about 1 mm apart, painted
on the surface (they move with the part); the strip one closed area, no
ticks or holes; far away a flat dark tint, no shimmer; the legend swatch
shows two lines.
Old behaviour: dots fixed on the screen while the part turned.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
