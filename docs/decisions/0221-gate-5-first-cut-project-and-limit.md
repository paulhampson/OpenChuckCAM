# 0221: Gate 5 cuts a first-cut project in tooling board, with no 3-axis test part and a ±0.1 mm limit (lane GATES)

- **Status:** accepted (22 Oct 2026) by the project owner, directly in the lane GATES session ("Accept 0221"). The owner's answers to OI-914, OI-915 and OI-916 were relayed by the coordinator session on 22 Oct 2026 (decision 0049); the coordinator asked for this record because they change slice 20 item 11 and verification plan 4.1. The document edits of *Consequences* are made with the acceptance.
- **What it amends:** [work breakdown](../work-breakdown.md) slice 20 item 11 and its review checklist line "Gates 1 to 5 have passed for the reference job set"; [verification plan](../verification-plan.md) section 4.1, the row "First-cut part (gate 5)", and the note under section 5's assumptions that the limits are revised after "the 3-axis test cut"; the [safe-practice checklist](../z1-safe-practice-checklist.md) section 3 line on the 3-axis test part. No requirement and no architecture change: GC-09, OP-21, SET-09 and NFR-12 are unchanged.
- **Date:** 22 Oct 2026.
- **Open items:** OI-916 closed by the acceptance; OI-914 and OI-915 are answered by items 2 and 3 (both closed: the geometry was given by the owner and the project is built, `testdata/projects/first_cut/`).

Numbering: 0221 was assigned to lane GATES by the coordinator and checked free on `main` and every unmerged branch on 22 Oct 2026.

## Context

Slice 20 item 11 runs gates 4 and 5 on the Z1 "for the reference job set", with "a first cut in wax or foam. Cut a 3-axis test part first to measure the machine's own repeatability, then set the first-cut limit from it (start at ±0.1 mm)". Verification plan 4.1 has the same rule for the first-cut part. Lane GATES wrote the step-by-step instructions ([z1-gates-4-5-test-instructions.md](../z1-gates-4-5-test-instructions.md)) and found three gaps:

- **OI-914.** The reference jobs' stock is 2 mm longer than the part, with no held region, so it cannot be gripped on the Z1 without the tool reaching the jaws; only `cone_held_tabs` models the grip and tabs.
- **OI-915.** No document gives cutting data for wax or foam; the reference jobs carry oak data.
- **OI-916.** No document defines the 3-axis test part, how it is measured or how the limit follows from it.

Requirements: GC-09 (the Z1 post), OP-21 (tabs), SET-09 (workholding), NFR-12 (the user verifies the program). Records read: 0106 (the air cut is a manual release step), 0007 (Z1 conventions), 0160 and 0161 (held regions), 0194 (deviation on held and tab faces), 0196 and 0197 (spindle speed source, material names), 0202 (tabs).

## Decision

1. **No 3-axis test part (OI-916, the owner's option B).** Gate 5 does not cut a 3-axis repeatability part. The first-cut limit is ±0.1 mm.
2. **Gate 5 cuts a first-cut project, not the reference job set (OI-914, the owner's option C).** The owner's specification: box stock 35 x 35 x 100 mm; the job at the end of the stock, not over its whole length; a part 25 mm long with a 5 mm tab; a tailstock at that end, with a very thin tab 2 mm long and 10 mm in diameter to give it clearance; only the Z1 library's 3.175 mm single-flute or ball-nose tools; the tools confirmed with the owner when the project is generated. The project's program passes gates 1 to 4 before it is cut. Gate 4 still runs the reference job set.
3. **The material is tooling board with the "Plastic" presets (OI-915).** Tooling board is the "foam" of verification plan section 6. Where a tool has no "Plastic" data, its "Softwood" data are copied to "Plastic"; where it has neither, the owner is asked before settings from a similar tool are used. The five 3.175 mm Z1 tools all have both (instructions section 5.3).

## Alternatives considered

- **Keep the 3-axis test part** (OI-916 options A and C). It measures what the machine itself holds, which the verification plan wanted; the owner chose to keep the starting limit instead.
- **Cut `cone_held_tabs`** (OI-914 option A), the one reference job that models the grip: its stock is as wide as the part and has no tailstock; the owner preferred a project designed for the first cut.
- **Add held regions to every reference job** (OI-914 option B): regenerates every golden file for a one-off trial.

## Consequences

The document edits below were proposed with this record and are made with its acceptance:

- `docs/verification-plan.md` section 4.1, the "First-cut part (gate 5)" row: "±0.1 mm. The 3-axis test part is not cut (decision 0221); revise the limit after the first cut if it proves too tight or too loose." The note that the limits "are revised after the first benchmarks, the simulation spike and the 3-axis test cut" drops "and the 3-axis test cut".
- `docs/work-breakdown.md` slice 20 item 11: "an air cut at reduced feed for the reference job set and the first-cut project (…), then a first cut of the first-cut project in tooling board ([decision 0221](decisions/0221-gate-5-first-cut-project-and-limit.md)) to a limit of ±0.1 mm"; the review checklist line: "Gates 1 to 4 have passed for the reference job set and gates 1 to 5 for the first-cut project, with results stored, and the G93 result is recorded".
- `docs/z1-safe-practice-checklist.md` section 3: "The first-cut limit is ±0.1 mm (decision 0221)" in place of the 3-axis line, and "tooling board" beside wax or foam.
- `docs/open-items.md`: OI-916 closed.

The first-cut project is built in lane GATES from the owner's geometry (OI-914, closed): `testdata/projects/first_cut/`, its programs in `testdata/golden/first_cut/`. NFR-12 still applies: generated G-code is the user's to verify before running on a machine.
