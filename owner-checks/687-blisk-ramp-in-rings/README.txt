Owner check for issue #687: a ramp in Rings on the public blisk generates

1. Open the folder "project" with File > Open project (the public blisk
   example, one rotary roughing operation in Rings, Entry Ramp at 3
   degrees, 1 mm steps).
2. Operations, Generate.
Expected: Rough rings, ramp generates, with three Ramp entry notes
(short stretches ramped back and forth over each pass's own path, passes
following a steep drop, single points plunged). No "please report it as
a defect".
Old behaviour: refused, "pass 438 starts with a stretch of 1.484 mm,
shorter than the 1.500 mm (the tool radius) a ramp needs ...".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on issue #687 with: PASS, or FAIL and what you saw, and the numbers
from re-running your own project.
Full steps and what we saw: CHECK.md in owner-checks/687-blisk-ramp-in-rings
on the branch owner-checks/2026.1-alpha.
