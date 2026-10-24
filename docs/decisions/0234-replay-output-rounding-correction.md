# 0234: Replay output correction for combined linear and rotary rounding (OI-984; issue #324)

- **Status:** proposed implementation record, 24 Oct 2026 (Europe/London). The correction policy is already accepted in decision 0211: preserve the existing tip-error limit and correct the output, checking controller support before any precision change. No implementation technique, precision change or new default is selected by this record. Implementation and lane assignment remain pending; this record starts no additional lane.
- **Date:** 24 Oct 2026 (Europe/London).
- **Requirements:** GC-09, GC-10, GC-07, NFR-06, NFR-12; verification plan 4.1 and replay gate 2.
- **Builds on:** [accepted decision 0211](0211-step-accuracy-measured.md), its form-a replay diagnostic and the owner's individual choice to preserve the limit and correct the output.
- **Open item:** [OI-984](../open-items.md), open; [GitHub issue #324](https://github.com/paulhampson/OpenChuckCAM/issues/324).

## Number allocation

On 24 Oct 2026 the owner instructed this lane: "Change of plan - The coordinator cannot give numbers. You need to select them yourself." This supersedes the earlier instruction to wait for centrally assigned numbers for this follow-up. Lane scheduling and release remain separate.

The lane fetched all 26 remote branch heads and checked `main`, every unmerged remote branch and the local branches without merging main. At the audit:

- `origin/main` was `e590f4de67bbde573edf30982982d6d0c5d92707`: highest decision file 0231 and highest OI reference OI-982.
- `claude/docs-gates-4-5-test-instructions` held decision 0232.
- `codex/lane-bug-edit-pending-edits` at `cb35309c27e2386ddec8bea0ee21938491c8a751` held decision 0233 and OI-983, the highest numbers in use across the checked branches. Those claims are untouched.
- No decision 0234 or OI-984 was found in the checked branch files or documented reservations. This lane claims 0234 and OI-984; issue #324 publishes the claim. No schema number is needed for tracking the defect. Recheck all branches before any later schema claim or implementation decision.

## Reproduced problem

The STEPALL-M measurement on source `827fc45dd8736265d8504ab71bc8c5057ce9c664` returns exit status 1 on form-a replay: **0.000912 mm tip error exceeds the unchanged 0.000867 mm limit**. The production CLI and two/four-thread application diagnostics reproduce it. The 1/2/4-thread programs are byte-identical, SHA-256 `9c4d20200e5a70076e1e16c010fc170ae255bcb57f5c104c04d8940b773fc268`.

At pencil's repositioning pose (zero-based group 2, pose 2295; G-code line 31,243), work-frame XYZ is `(21.558251369535647, -0.016498447475797207, 45.000000000000014)` and A is `120.93751838129586` degrees. Posted XYZ is `(21.558, -0.016, 45.000)`, A `120.938`. Each coordinate rounds within half a unit at three decimals, but combined XYZ/A rounding at approximately 45 mm rotary radius gives model-tip error `0.0009120345106203881` mm. The matching modal G0 line is `X21.558 Y-0.016`, following `G0 Z45.000`.

The rounding-only diagnostic uses shared kinematics. It characterizes the failure but is not independent certification of the post. See [0211's full diagnostic and fixture provenance](0211-step-accuracy-measured.md) and [the commands, raw output and diagnostic source](0211-step-accuracy-evidence.txt). The original trial STL remains outside the repository; no new fixture permission is inferred.

## Accepted policy and required outcome

1. Preserve the existing gate-2 tip-error and angular limits. Do not relax the comparison budget to make the case pass.
2. Investigate and correct output precision/rounding, with evidence of controller support before any precision change. The implementation technique remains to be determined; tracking does not authorise a particular precision, schema or post-default change.
3. Write a requirement-based regression for combined XYZ/A rounding at nonzero radius before the correction, reproduce its failure, then verify the corrected output through production application and CLI replay.
4. Keep byte-identical output at 1, 2 and N threads for the same project, post and software version (GC-07, NFR-06).
5. Recheck form-a replay in STEPALL-M after the product correction. Keep the failure visible until it passes the unchanged gate.

## Scope and release

This follow-up tracks the replay correction separately from STEPALL-M's measurement implementation. A product correction needs an authorised implementation lane; this record does not assign one or start work in post/checker product code. Keep OI-839 in STEPALL-X and keep the sampling bound, accuracy allocation, five-run timings and full-stepover determinism as separate outstanding criteria.

PR #321 stays draft. Decision 0211 is accepted, but neither this tracking claim nor that acceptance releases the PR to the Merge Manager, marks it ready, merges it, or enables auto-merge. The implementation record remains proposed until an implementation is reviewable and accepted where required.

## Consequences

- Track the defect as OI-984 and issue #324; cross-link it from decision 0211, the decision index and work breakdown.
- Replace statements that numbering must wait for the coordinator with the owner's new allocation instruction. Coordinator lane scheduling and release remain separate.
- No product code, tolerance, schema, fixture, dependency or default changes in this tracking update.
