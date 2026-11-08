Owner check for issue #558: shortening the free-end tab makes the operations stale
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project; Operations > Generate.
A. Setup 1 > Workholding: Tab 2 Length 4, Enter, Apply, and at once (before
   the spinner stops) click "1 Rough". Expected: "1 Rough" becomes Stale.
   Old: it stayed up to date. (On this small project the check may finish
   too fast to catch; then A shows nothing either way.)
B. Workholding again: Tab 2 Length 3, Enter, no Apply; click "1 Rough".
   Expected in Messages: "The workholding of 'Setup 1' has changes that are
   not applied. The view shows them, but the toolpaths, the simulation and the
   G-code use the held regions and tabs stored in the setup: apply or revert
   the changes in its workholding section". Old: no such line. Then Revert.
C. For OI-2060, about your crankshaft: was the free-end tab Automatic, was
   there a "Not applied" chip, did you click away before Apply's spinner
   stopped, which build?

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
