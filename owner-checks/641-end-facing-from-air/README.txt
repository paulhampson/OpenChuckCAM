Owner check for issue #641 (fixed in #656): End facing enters each band
from air and checks its engagement
Build: commit db6bc84b or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project.
2. Operations, "1 Face end": Stepover is the slice along X, Stepdown the
   band along the tool.
3. Generate: each band comes down in air beside the stock and only cuts
   across; nothing feeds sideways into the end face.
4. "2 Face end, part only": note "Stock above the area (...): ... 2 passes
   from the stock's top down to the area are added ..."; its side entry
   inside the stock's width is at the plunge feed (F300).
5. Post and export: written, not refused.
6. Stepdown 30 on Face end: refused, naming Stepdown.
Old behaviour: refused at posting ("an actual End cut exceeds the selected
maximum axial stepdown"); with smaller values each band fed sideways into
the end face at the cutting feed.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
