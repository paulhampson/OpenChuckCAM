Owner check for issue #611 and decision 0539: a refusal and a warning in the
workholding section each keep their own level; a tab with no room is stored
Build: commit 3b99ea23e or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project.
2. Setup step, Setup 1, Inspector > Workholding.
3. Add tab: untick Place automatically, Cylinder, Start X 30, Length 5,
   Diameter 10. Apply.
   Expected: refused. Tab 2 row red, Error mark ("is not wholly in the head
   or the tail zone"). Tab 1 row amber, Warning mark, NOT red ("Tab 1 is
   5.000 mm long along X, shorter than the tool ...").
4. Tab 2 > ... > Remove. Apply: stored.
5. Grip length 13, Enter, Apply.
   Expected: stored, not refused. Tab 1 amber Warning: "Tab 1 is placed
   automatically at the head of the part, but there is no room there ...".
Old behaviour: in 3 both rows red; in 5 Apply refused.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
