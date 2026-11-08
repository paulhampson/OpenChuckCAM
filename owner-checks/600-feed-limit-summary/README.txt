Owner check for issue #600: one feed-limit line per operation, and no
"0.0 mm/min" at the rotary axis
Build: commit 1f3675d52 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project (the tutorial job).
2. Operations, Generate. Open the Results tab.
Expected: one line per operation: "Feed limit (operation 'Rough'): 9536
moves were limited
by the machine's feed limits ... reduced to 22.0 mm/min ..." and "Feed limit
(operation 'Finish'): 7818 moves ... reduced to 0.7 mm/min ...", and one note
"Rotary axis (operation 'Finish'): moves 19077 to 19148 (72 moves): A
turns with the
contact point at the rotary axis during these moves, ...". No line says
"0.0 mm/min" or "as low as 0 mm/min". No per-move lines.
Optional: in the export preview the last five rings read F551.3, F488.5,
F425.7, F362.8, F362.8; no F0.
Also in the preview: the program's 3rd and 4th lines are the notice
"(This program has not been run on your machine. Check it before you
run it.)" and "(Simulate it and air cut it first. You are responsible
for what the machine does.)".
Old behaviour: 17,426 per-move warnings in the command line, 72 of them
"the contact feed is 0.0 mm/min, not 500.0".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
