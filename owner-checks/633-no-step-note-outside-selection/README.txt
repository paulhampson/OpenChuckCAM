Owner check for issue #633: no "does not turn at a step" warning at an X
outside the selection and the stock

1. Open the folder "project" with File > Open project (the three
   owner-check projects #633 names, as three setups of one project on
   the public baluster).
2. Operations, Generate. Open Messages.
Expected: no line "Held region (operation '...'): the tool does not turn
at a step across the rotary axis ... at X ...". The only Held region
warning left is on Whole part: "nothing within one tool radius of the
held region at the start of the stock is cut" (by design). Whole part
also keeps its Tab clearance and Tab length warnings (not part of this
issue). The G-code is the same as before the fix, byte for byte.
Old behaviour: the "does not turn at a step" warning on Tenon side at
X -15.001, on Vase only and Whole part at X -16.001, and on Finish vase
at X -3.411.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on issue #633 with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in
owner-checks/633-no-step-note-outside-selection on the branch
owner-checks/2026.1-alpha.
