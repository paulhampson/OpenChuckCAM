Owner check for issue #477: a free-end ring that would part the part from the chuck
Build: the nightly of 8 Nov 2026 (76796fb24) or later, or main.

1. Open the folder "project" with File > Open project.
2. Click Operations, then Generate; click "1 Finish with free ends" and read
   Messages.
3. Setup > Stock: Length 80. Workholding: tick Held under "Held at the end",
   Grip length 4, Apply. Then Operations > Generate stale; read Messages.

Expected: after step 2 a warning "the end face at X 0.000 is faced down to the
rotary axis: ... and nothing in this setup holds the part on the other side of
the ring ..., so the part can be cut free there and thrown while the rotary
axis turns. ...". After step 3 that warning is gone.
Old behaviour: no such warning at step 2.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine. (In this layout the part would be cut free.)

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
