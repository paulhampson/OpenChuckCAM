Owner check for issue #622: a note about the whole path no longer reads
"move 0:"
Build: commit 1f3675d52 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project (the tutorial job).
2. Operations, Generate. Read the Results tab and the Messages list.
Expected: no line contains "move 0". Notes such as "Shoulders (operation
'Rough'): the part has a step across the rotary axis ... at X 49.889 ..."
and "Free ends (operation 'Finish'): the end face at X 61.000 has nothing
beyond it ..." name no move. Finish's two warnings ("Held region
(operation 'Finish'): ...": the shoulder at X 8.589, and the held region)
are said once each, as warnings. (Builds before 645256c3 print these
without the topic in front; either form passes.)
Old behaviour: "operation 'Rough': move 0: the part has a step ...", and
the same "move 0:" on the three Finish lines.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
