Owner check for issue #638: steps between rings can be blended
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project.
2. Operations, Generate.
3. "1 Rings, as saved": Steps between rings reads Sharp; steps along X at
   one angle, as today.
4. "2 Rings, blended": Blended, Blend over 45; each step a short slanted
   piece, the joins spread round the part; the note names the blend.
5. Blend over 0, then 200: each refused, naming Blend over.
6. A new helical finishing operation in Rings starts as Blended.
Old behaviour: no such setting; "Rings, blended" refused ("parameter
ring_blend_angle: this strategy has no parameter of that name").

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
