Owner check for decision 0536 build step 6 (decision 0644): every
refusal, the Facing notes and the setup checks in the one form

1. Open the folder "project" with File > Open project (the tutorial job
   plus the public cylinder.step; seven operations, each made to give
   one kind of message).
2. Operations, Generate. Read Messages and Results.
Expected: every refusal reads "Topic (operation '...'): ...", ending
with one full stop: End facing (2), Stepdown (3), Ramp entry (4),
Links (6, with no "(OP-14)"), Lead entry (7, "88 passes, from pass 1
to pass 88 (1, 2, 3 ... 86, 87, 88), are entered ..."). Face end (1):
"Facing (operation 'Face end'): facing covered the end faces it was
asked to." Show setting opens 2 at Stepover (slice thickness along X),
3 at Stepdown, 4 at Allow single-point plunge, 7 at Entry.
Setup 2, low clearance: "Clearance (setup 'Setup 2, low clearance'):
the safe cylinder (radius 10.000 mm) does not enclose the stock ...".
Old behaviour: "operation '...' cannot be generated: End facing: ...",
"parameter stepdown: ...", "... (OP-14); enlarge ...", "88 passes, the
first pass 1, are entered ..."; setup checks with no topic.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Tick "Step 6 passes" on the tracking issue, or reply there with FAIL and
what you saw.
Full steps and what we saw: CHECK.md in
owner-checks/rc/0536-step6-refusals-in-one-form on the branch
owner-checks/2026.1-alpha.
