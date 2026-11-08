Owner check for issue #567 (decisions 0538 and 0540): a possible collision
must be acknowledged before the G-code is written
Build: commit 3b99ea23e or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project.
2. Operations, Generate.
3. Hover the mark of "1 Whole part": it starts "Needs acknowledging: Tab
   length (operation 'Whole part', tool '6 mm ball (check 567)')". In
   Messages it comes first among the warnings.
4. File > Post and export..., Makera Z1, type a folder, scroll to Checks
   before writing: the tab-length line is listed as a possible collision,
   with the box "I have read the possible collisions listed for this
   result and will check them before running the program".
5. Without that tick, Write G-code is greyed: "Tick the box to say you have
   read the possible collisions of these files." Tick it: the file is
   written.
Old behaviour: a plain warning, no box; the file was written at once.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
