# 0234: Replay output correction for combined linear and rotary rounding (OI-984; issue #324)

- **Status:** proposed implementation record, 24 Oct 2026 (Europe/London). The correction policy is already accepted in decision 0211: preserve the existing tip-error limit and correct the output, checking controller support before any precision change. The owner subsequently instructed "Complete all steps and also include changes for #324 in this work", authorising this correction in STEPALL-M / PR #321. The implementation below is reviewable; this record has not been accepted.
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
2. Investigate and correct output precision/rounding, with evidence of controller support before any precision change. The implementation retains the existing precision, schema and post defaults.
3. Write a requirement-based regression for combined XYZ/A rounding at nonzero radius before the correction, reproduce its failure, then verify the corrected output through production application and CLI replay.
4. Keep byte-identical output at 1, 2 and N threads for the same project, post and software version (GC-07, NFR-06).
5. Recheck form-a replay in STEPALL-M after the product correction. Keep the failure visible until it passes the unchanged gate.

## Implementation

Round the commanded A angle to the post's existing rotary precision first. Use that written angle to transform the original IR model tip into machine XYZ, then round XYZ to the existing linear precision. This changes the positioning values, not the decimal counts or the controller dialect. At the reported pose, Y changes from `-0.016` to `-0.017`; A remains `120.938`.

The inverse rotation at the written angle is an isometry. If each XYZ coordinate rounds by at most half a linear unit, its model-tip error is at most `sqrt(3) / 2 * 10^-position_decimals`, plus the existing numerical allowance. There is no extra radius-dependent error from A rounding because XYZ was calculated at that same angle. A itself remains within its existing half-unit angular limit. This proves the rounding contribution at commanded endpoints; it does not certify machining accuracy between poses or replace path verification.

- `openchuck-post` applies this order in the neutral machine-program conversion. Feed calculation continues to use the positions actually written, so the correction also enters the existing feed rules.
- `openchuck-gcode` uses the written-angle transform only when identifying moves that collapse onto the preceding rounded pose. Its replayed tip is still compared with the original IR tip. `Tolerances`, tip/angular limits and feed-time checks are unchanged.
- No new decimal precision or command needs controller support: both posts keep their existing precision and dialect. No schema, dependency or public API changes are required by the replay correction.

## Verification

The application regression was written first and failed before the correction: combined XYZ/A rounding exceeded the existing tip limit. It covers both X/Y rotary axes, both A senses, a nonzero frame origin, a duplicate rapid pose and a following fed move, through the production Makera Z1 and LinuxCNC posts and independent reader/replay comparison.

The G-code crate also reads hand-written original and corrected programs for the reported pose: the original `Y-0.016` remains rejected for tip error, and corrected `Y-0.017` passes. The post crate checks the converted endpoint by a direct trigonometric inverse against the linear half-unit norm. The lobed reference program changes at fractional angles (for example, Y 0.624 becomes 0.623 at A 30.057, with feed recalculated from the written motion). Changed reference files are regenerated through the production posts, reviewed and checked by the independent golden audit; unchanged post-only goldens remain unchanged.

On the saved form-a quick-stepover project that reproduced the defect, the production CLI reports 6 toolpath groups, 1 file and 66,076 moves; replay matches at a printed maximum tip error of 0.0008 mm and A error of 0.0005 degrees, with cut time 7597.8 s against IR 7597.9 s. The STL remains outside the repository. The separate full-stepover M4 now completes with exit 0: a/b/c library paths and production-posted bytes agree at 1/2/4 threads; STEP and STL application output matches the library and passes replay at each count. Refused simultaneous finishing is explicitly disabled for the application jobs. Source and full output are in [the M4 evidence](0211-full-stepover-m4-evidence.txt); these are correctness checks, separate from five-run performance measurements.

The corrected lobed LinuxCNC reference also passes gate 3 in LinuxCNC 2.9.10: all 1,806 moves reached, no fault, 105.6 s of motion. The simulator/replay comparison's largest sampled difference is 0.0552 against its existing 0.1 threshold; that gate-3 metric is separate from gate 2's model-tip rounding limit. The geometry measurements were paused during the real-time replay. The exact command, image digest and report hash are in [the evidence file](0234-replay-output-evidence.txt).

## Scope and release

The owner's instruction to include #324 in this work supersedes the former pending-lane restriction for this correction. It does not authorise STEPALL-T/X implementation or a tessellation default. Keep OI-839 in STEPALL-X and keep sampling, accuracy allocation and performance verification separate.

PR #321 stays draft. Decision 0211 is accepted, but that acceptance and the instruction to implement this correction do not accept this implementation record, release the PR to the Merge Manager, mark it ready, merge it or enable auto-merge. Present the complete final record before requesting its acceptance, after the other decisions and verification results.

## Consequences

- OI-984 and issue #324 track the implemented output correction and its verification in this PR.
- Programs with fractional rotary angles can change by one or more linear output units to preserve their requested model tips. Existing commands and decimal precision remain unchanged; regenerated programs remain subject to the normal verification gates.
- Other STEPALL-M verification limits remain visible in decision 0211. Correcting replay does not establish the physical sampling bound, the 0.005 mm tessellation allocation or a successful seven-operation blisk job.
