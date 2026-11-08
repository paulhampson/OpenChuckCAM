Owner check for decision 0536 build step 4: setup and export messages in
one form, and plain command-line help
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project.
2. Setup, Setup 1: the clearance message should start "Clearance (setup
   'Setup 1'): ...", use the Inspector's labels and link to the field.
3. Operations, Generate, File > Post and export...: the same text is a
   possible collision to tick.
4. Command line: no help line has a requirement number (SET-01, GC-14,
   SIM-03 ...), a decision number or "gate 3".
Today: "the safe cylinder (radius 30.000 mm) is inside the minimum
clearance ..."; 20 help lines carry numbers.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
