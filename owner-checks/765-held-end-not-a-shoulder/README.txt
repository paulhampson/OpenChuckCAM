Owner check for issue #765: the held bar's own end is no longer named as
a shoulder

1. Open the folder "project" with File > Open project (the tutorial
   project, unchanged).
2. Operations, Generate. Read Messages for Finish.
Expected: no "Held region (operation 'Finish'): the tool does not turn
at a step across the rotary axis ... at X 8.589 ...". Still there, as
designed: "Held region (operation 'Finish'): nothing within one tool
radius of the held region at the start of the stock is cut ...", the
Free ends warning at X 61, and the setup's "the grip ... covers 7.000 mm
of the part". The badge still reads Needs attention. The G-code is the
same as before the fix, byte for byte.
Old behaviour: the X 8.589 warning as well.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on issue #765 with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/765-held-end-not-a-shoulder
on the branch owner-checks/2026.1-alpha.
