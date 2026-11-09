Owner check for issue #640: a lead on a ring stays in the plane of the cut

1. Open the folder "project" with File > Open project (the tutorial job
   without its grip, plus the public cylinder.step; three operations, all
   with Entry Lead).
2. Operations, Generate.
Expected: 1 Finish rings, lead: generated; the lead-in comes down at X 0
with A and Z moving together, nothing comes in along X; note "Lead entry
... passes 14, 27 start on the rotary axis ... entered by a straight
plunge and has no lead-out". 2 Axial lead, through stock: refused, "...
88 passes, the first pass 1, are entered by a straight plunge into the
stock ... the tool 'O-flute, not centre cutting' cannot cut straight
down ...". 3 Axial lead, through air: generated.
The two "Part cut free" warnings on 1 come from having no grip; they are
expected and not part of the check. The case with the grip waits for pull
request #664.
Old behaviour: 1 came in as a quarter arc in the X-Y plane along X; 2
generated, descending through the stock with a tool that cannot plunge.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/640-lead-follows-the-pass
on the branch owner-checks/2026.1-alpha.
