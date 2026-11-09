Owner check for decision 0536 build step 4: setup, model and export
messages in one form, Show setting for setups and tools, plain help
Build: commit 0d640692 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project.
2. Messages: "Clearance (setup 'Setup 1'): the safe cylinder (radius
   30.000 mm) is inside the minimum clearance ... before running."
   Show setting opens the Setup step at the clearance field, marked.
3. Generate, File > Post and export...: the same text, then "A possible
   collision: acknowledge it before the files are written". Every line:
   topic, lower case after it, full stop at the end.
4. Command line: no help page names a requirement, decision or "gate";
   regenerate --out without --acknowledge-collision-warnings writes
   nothing and still prints "Generated G-code must be verified ...".
Before step 4: "the safe cylinder ..." with no topic; 20 help lines with
numbers; no notice on that refusal.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
