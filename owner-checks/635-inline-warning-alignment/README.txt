Owner check for issue #635 (fixed in #648): the warning line under a
setting uses the Inspector's width
Build: commit 645256c3 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project (the #442 project:
   the blisk, Blade wall set, Blade swarf at its defaults).
2. Operations, Generate, click "1 Blade swarf".
3. In Messages, Show setting on "Overcut (operation 'Blade swarf'): Swarf
   on face 19 cuts into the finished surface by 0.0158 mm ..." (Tolerance).
4. Read the line under the marked setting; drag the Inspector wider and
   narrower.
Expected: the warning under the marked setting is one block across the
Inspector's whole width, the warning shape just before its first line,
every line starting at the same place, wrapping at the edge at every
width: "The last path was made with a warning about this setting (see
Messages): change it to accept or remove the warning."
Old behaviour: the shape alone after an indent, the text starting on the
next line at the left edge.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
