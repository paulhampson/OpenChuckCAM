Owner check for issue #644 (fixed in #655): on ring and spiral passes, Extend past the
stock follows the pass. Re-issued 10 Nov 2026 after the rc1 FAIL.
Build: the release candidate (ea1f3f96), or commit 1f2c0be6 or later.

1. Open the folder "project" with File > Open project. Setup 1 has Face
   free ends on (the default), Setup 2 has it off.
2. Operations, Generate. Zoom on the far end (X 61 to 67) of each.
Pass criterion: the spiral carries on turning past X 61, or rings are
added past X 61 (not a line along X); a "Past the ends" note gives what
was added.
What stays at the tip (expected, by design): the run-out keeps the
height of the pass's last point. With Face free ends off a core about
5.3 mm across stays from about X 60.2 to the stock end. With it on, the
stock past X 64.2 is cut free ("Part cut free" warnings to acknowledge)
and a small blunt nub stays at the finial's point. The tip is not cut to
a point in 0.55.0 (ticket #779).
Old behaviour: a straight line along X at one angle to X 66.888.

Nothing in this check shows that a program is safe to run. Do not run this
project's G-code on a machine. Generated G-code must be verified by you
before it is run on a machine.

Reply on the issue with: PASS, or FAIL and what you saw.
Full steps and what we saw: CHECK.md in this folder on the branch
owner-checks/2026.1-alpha.
