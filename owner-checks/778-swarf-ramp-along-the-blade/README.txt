Owner check for issue #778 (fixed in #781): a swarf ramp entry goes along
the blade, not round the part

Use a build from main at c7623522 or later (the release candidate
ea1f3f96 does not have this fix).
1. Open the folder "project" with File > Open project (the public blisk
   example, swarf on the blade wall, face 19, 3 mm flat end mill).
2. Operations, Generate.
Expected: Blade swarf, ramp generates; its entry (172 moves) runs back
and forth along the blade, reversing A 3 times (a Ramp entry note says
so), never round the part. Simulate: the far-end tips of the other blades
are not cut (deepest overcut 0.0558 mm, no problems). The swarf pass's
own Overcut warnings (0.0158 mm, the face's twist) are as before.
Blade swarf, lead: refused, as before (the known lead case, #572).
Old behaviour: the entry stayed at X 59.804 and turned A about 4.2 turns
round the part, cutting the other blades' tips (simulated 1.700 mm deep).

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on issue #778 with: PASS, or FAIL and what you saw, and your own
project's numbers.
Full steps and what we saw: CHECK.md in owner-checks/778-swarf-ramp-along-the-blade
on the branch owner-checks/2026.1-alpha.
