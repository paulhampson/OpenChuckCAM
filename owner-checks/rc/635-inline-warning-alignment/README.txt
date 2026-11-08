Owner check for issue #635: the warning line under a setting uses the
Inspector's width
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project (the #442 project:
   the blisk, Blade wall set, Blade swarf at its defaults).
2. Operations, Generate, click "1 Blade swarf".
3. In Messages, Show setting on the swarf warning.
4. Read the line under the marked setting; drag the Inspector wider and
   narrower.
Expected: "The last path was made with a warning about this setting (see
Messages): ..." directly under the setting, across the Inspector's whole
width, wrapping there, at every width.
Old behaviour: the line is out of line with the Inspector's columns.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
