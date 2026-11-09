Owner check for issue #641: End facing enters each band from air and
checks its engagement
DRAFT: to run on the release candidate, once the fix is merged.

1. Open the folder "project" with File > Open project.
2. Operations, "1 Face end": Stepover is the slice along X, Stepdown the
   band along the tool.
3. Generate: each band comes down in air beside the stock and only cuts
   across; nothing feeds along X into the end face.
4. Post and export: written, not refused.
5. Stepdown 30: refused, naming Stepdown.
6. "2 Face end, part only": a note "Stock above the area (...)"; side
   entries at the plunge feed.
Old behaviour: refused at posting ("an actual End cut exceeds the selected
maximum axial stepdown"); with smaller values each band fed sideways
along X into the end face at the cutting feed.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in owner-checks/rc/ on the branch
owner-checks/2026.1-alpha.
