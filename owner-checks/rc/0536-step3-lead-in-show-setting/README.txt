Owner check for decision 0536 build step 3: every message has its lead-in,
and Show setting goes to the field
Build: commit 645256c3 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project.
2. Operations, Generate.
3. In Messages, every warning should read "<Topic> (operation '<name>'...):
   <what happened>. <what to change>." with no requirement numbers.
4. Click each Show setting: the operation opens and the named field is
   marked: Finishing layers on "Short flutes" (Flute length warning), Face
   free ends on "Free ends" (Part cut free warning). Held region warnings
   have no setting.
Before step 3: only "Flute length (operation 'Short flutes', tool ...)"
had that form; the others started "Operation 'Free ends': ...".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
