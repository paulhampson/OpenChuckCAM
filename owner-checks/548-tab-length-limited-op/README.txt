Owner check for issue #548: tab length check and an operation limited away from the tab
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project.
2. Click Operations, then Generate (under a minute).
3. Click "1 Vase only", then "2 Whole part"; read each status and Messages.
   (Ignore "the machine's feed limits slow this move" lines.)

Expected: both are generated. "1 Vase only" has no "Tab length" line.
"2 Whole part" has one warning starting "Tab length (operation 'Whole part',
tool '6 mm ball (check 548)'): tab 1 (X -5.000 to 0.000 mm) is 5.000 mm long
along X, shorter than the tool is wide (6.000 mm) ...".
Old behaviour: generation stopped with "operation 'Vase only' cannot be
generated: invalid workholding: tab 1 is 5.000 mm long along X, shorter than
the tool '6 mm ball (check 548)' (6.000 mm); ...".
Known and separate: the setup's own check still lists the tab (OI-2045).

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
