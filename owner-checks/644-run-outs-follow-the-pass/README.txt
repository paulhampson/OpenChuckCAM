Owner check for issue #644 (fixed in #655): on ring and spiral passes, Extend past the
stock follows the pass
Build: commit 1f2c0be6 or later (Help > About OpenChuckCAM), or main.

1. Open the folder "project" with File > Open project.
2. Operations, Generate. Zoom on the far end (X 61 to 67) of each.
Expected: "Spiral, extend": the helix carries on past X 61; "Rings,
extend": rings added past X 61; a note gives what was added. Simulated,
the overhang is cleared all round.
Old behaviour: a straight line along X at one angle to X 66.888, then a
90-degree turn into the pass.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
