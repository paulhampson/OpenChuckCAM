Owner check for decision 0536 build step 3: every message has its lead-in,
and Show setting goes to the field
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project.
2. Operations, Generate.
3. In Messages, every warning should read "<Topic> (operation '<name>'...):
   <what happened>. <what to change>." with no requirement numbers.
4. Click each Show setting: the item opens and the named field is marked
   (Flute length of the short-flute tool; Face free ends, a tick box, on
   "Free ends"; Stepdown).
Today: only "Flute length (operation 'Short flutes', tool ...)" has that
form; the others start "Operation 'Free ends': ...".

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
