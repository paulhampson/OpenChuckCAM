Owner check for issue #637: the mark on areas a strategy could not reach
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project (the #512 project).
2. Operations, Generate, click "1 Floor finish".
3. Look at the unreached strip on the hub floor next to the far blade;
   turn the part and zoom.
Expected: the strip is hatched: black diagonal lines in a white casing,
1 mm apart, painted on the surface (they move with the part), fading to a
flat tint far away; one closed area along the strip, no single ticks.
Old behaviour: dots that stay still on the screen while the part turns.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
