# OpenChuckCAM merge queue

Updated: 24 October 2026 (Europe/London).

## Working practice — owner instruction, 23 October 2026

Effective intake cutoff: 2026-10-23T22:43:41Z (23:43:41 Europe/London). This owner instruction supersedes the previous requirement for an explicit lane assignment before intake.

- Lanes complete their tasks and become ready to work through the merge process before opening a draft PR.
- A newly opened draft PR in `paulhampson/OpenChuckCAM` authored by repository owner `paulhampson` (GitHub ID 528002) is the trigger for automatic queue intake. Add each once, in creation order, behind existing entries. Existing unassigned PRs predating the cutoff are not automatically assigned by this change.
- After merging the active PR, address the next queued item. Keep explicit holds and incomplete-work blockers in force until resolved; draft creation does not approve a decision record.
- Only one open PR may be ready for review at a time. Before release, verify task completion, applicable owner-approved decision records, and the absence of another ready PR.
- Release by marking ready first, then posting a PR comment instructing the lane to update from latest main, resolve conflicts and register unions, run `cargo xtask records` and required checks, and push once with its exact head. This starts the main-update, CodeRabbit and heavy-CI cycle under decision 0055.
- Lanes monitor PR comments for messages from the owner. The coordinator posts release and correction instructions through the owner's connected GitHub account. All branch/code changes remain the lane's work.
- Wait for both CI and CodeRabbit before requesting one bundled correction push. Fix findings higher than major and genuinely blocking failures; minor/optional fixes and nonblocking docs/open items ride the next PR.
- Only the owner approves decision records. Route new decisions individually through the coordinator, show the complete record before requesting acceptance, and give a recommendation and risks.
- Merge only when fully ready and green, using an exact-head SHA guard; do not bypass branch protection or enable auto-merge. Record the merge SHA/date, then address the next item.
- Event-driven GitHub intake is unavailable in the current toolset. The existing hourly automation checks for new owner-authored draft PRs and active-PR progress; remain quiet when unchanged or non-actionable. Keep it enabled after a merge for future intake. Do not archive this chat.

The lane owns its branch, code, register reconciliation and review corrections; the merge coordinator owns intake, release order and merging.

## Planned queue

| Position | Lane | PR | Branch | Status |
| --- | --- | --- | --- | --- |
| 1 | SWARF3 | [#320](https://github.com/paulhampson/OpenChuckCAM/pull/320) | `claude/lane-swarf3` | Ready; 87b557e audit fix green; macOS tests still running |
| 2 | STEPALL-M | [#321](https://github.com/paulhampson/OpenChuckCAM/pull/321) | `claude/lane-stepall-m-measure` | BLOCKED / IN PROGRESS — draft; not authorised for release |

## SWARF3 / #320

- Handoff head: `96258e867712ac9ee47000904715a6d956e30727`.
- Owner handoff in the merge-coordinator chat on 23 October 2026 confirms explicit acceptance of decisions 0228 and 0229 after review of their full text. Their Status lines and index entries record acceptance relayed through the coordinator; verified at the handoff head. No other decision acceptance is authorised.
- OI-970 remains open: reliable detection of narrow crossings is deferred. Warn only remains advisory and must always explain its sampling limitation.
- Release under decision 0055: coordinator marks ready first; lane then merges latest main, reconciles register unions, runs `cargo xtask records` and required checks (including outstanding traceability), and pushes once, reporting the exact head.
- Wait for both full CI and CodeRabbit before batching required corrections into one push. Findings higher than major must be fixed; other genuinely blocking failures must be addressed. Minor/optional findings, missing docs and additional open items ride the next PR unless needed to fix a blocker.
- Handoff verification: 25 path, 2 UI and 1 application tests passed; focused coverage reached every new executable path/UI line; changed-crate clippy, formatting, dependency, licence and record checks passed. Traceability was pending at handoff after a local workspace build exceeded environment capacity; see the lane's release progress below. Updated-head full CI and CodeRabbit remain pending.
- The 67.238-second regression remains in default PR tests and is classified in long/coverage profiles.
- Owner explicitly authorises the merge coordinator to merge this PR when fully ready and green; do not enable auto-merge on the lane's behalf.
- Present any new key decision to the owner individually, with a recommendation and risks.

- Released by the merge coordinator on 23 October 2026: marked ready before any lane update; instructions posted in [PR comment](https://github.com/paulhampson/OpenChuckCAM/pull/320#issuecomment-6375867197).
- Hourly follow-up is enabled to inspect lane progress, CI and CodeRabbit, coordinate blockers, and merge only when all gates are met. Notifications are limited to meaningful changes or required action.

## Completion gates

Confirm the released head includes latest main, register and traceability checks pass, required CI is green, CodeRabbit has finished and blocking findings are resolved, and no new unapproved decision is relied on. Merge only the verified head; record its merge SHA and date here before releasing the next assigned PR.

## SWARF3 current release status

Current head: `87b557ed3b1bcfd252733c389d499ab103f393a2`, parent release head `8e25a51e9180128ae0169228e5b6d3cd923b48ca`. Latest main merged: `a2605495d4e05d323d4cc3f85a4f4d6be118072f`. GitHub reports mergeable; #320 remains the sole released PR.

The lane waited for full CI and CodeRabbit on the release head before pushing its [single batched correction](https://github.com/paulhampson/OpenChuckCAM/pull/320#issuecomment-6376729522). Only transitive Tokio's version/checksum in Cargo.lock changed, 1.53.2 to 1.53.3, addressing RUSTSEC-2026-0386. Audit was reproduced failing before and passing after; cargo deny and cargo xtask all pass locally, including 99/99 P1 traceability. No source, schema, direct dependency pin or decision change. OI-970 remains open.

On the release head, 25 path/region and 32 application/UI focused tests passed locally; full platform tests, UI smoke, STEP kernel, standalone libraries, LinuxCNC replay and offline CLI passed in CI. Focused coverage evidence is retained; whole-crate 95% is not claimed. Register unions and changelog conflicts were reconciled with both branches' entries preserved.

Latest-head [CI run 40039352507](https://github.com/paulhampson/OpenChuckCAM/actions/runs/40039352507) remains in progress. Fast checks are now SUCCESS, including audit, records and traceability. All STEP-kernel, UI-smoke, standalone-library, Gate 3 and Linux/Windows build-test legs pass. macOS build/test is still executing Tests; required-check aggregation is not yet complete. End-to-end run 40039352328 passed. Fuzz is skipped by path filter. Do not merge until remaining required CI is green.

CodeRabbit status is SUCCESS for the current head; its new review was skipped because Cargo.lock is excluded by existing filters. Unchanged source was reviewed on the parent. Its sole minor swarf suggestion was withdrawn as outside decision 0229 scope and the thread resolved; no unresolved review threads remain. No further lane push is planned without a genuine blocker.

Intake search found no new owner-authored draft PRs after cutoff (under 100 results). Existing #321 remains on explicit hold.

## STEPALL-M / #321 — BLOCKED / IN PROGRESS

- Queued behind SWARF3; not ready for release. Keep draft. Await completion and a verified handoff from the dedicated STEPALL-M chat; do not automatically release after #320 merges.
- Pushed checkpoint: `9fa660cdfd0af150cc118250994f8c13a582b7e2`. Main merged: `a2605495d4e05d323d4cc3f85a4f4d6be118072f`. GitHub draft state and checkpoint head verified on intake.
- The owner authorised merging main while retaining draft status, then requested transfer to a dedicated chat. The previous agent stopped; the replacement chat continues the pushed checkpoint. This exception is specific to this lane and is not release permission.
- Completed source work (not a validated checkpoint): qualified historical accuracy/performance claims; made M4 mismatches and replay failures return errors; adapted the harness to current pencil dispatch and operation material data.
- Implementation blocker: form-c posting helper remains an unwired, empty-output stub. Two new posting tests failed against that stub; one was cancelled.
- Validation blockers: kernel build stopped for transfer; full harness compilation, refreshed M2/M4 measurements and required checks remain outstanding.
- Decision 0211 remains PROPOSED. Owner choices OI-834, STEPALL-X ordering and OI-835 remain unresolved. Decisions must go individually through the coordinator, and the owner must see each complete record before acceptance is requested. No decision acceptance is authorised by this handoff.
- Historical 1.24× timing includes a failed operation and must not be represented as current successful-job performance. No current successful-job performance is established by this checkpoint.
- No new default or methodology waiver has been approved.
- Release gates specific to this lane: implementation and required verification completed, refreshed evidence and accurate report supplied, pending owner decisions resolved through the coordinator, applicable full records explicitly accepted by the owner, and verified handoff received. Then reassess queue order and the sole ready-for-review slot.

## STEPALL-M checkpoint update — 24 October 2026

Current draft head is `9e981c2149ed9f8637b7bf06f9edecb7fd2f3dd5`; measured code `827fc45dd8736265d8504ab71bc8c5057ce9c664` includes the previously authorised main `a2605495`. The original stub/build blockers above describe the intake checkpoint; the PR now reports production-post integration, full harness/kernel compilation, ten passing native tests, local lint/register/traceability checks (99/99 P1), cargo deny, and focused coverage. Do not treat this as release completion.

Refreshed quick M2/M4 evidence is reported: production-posted bytes/library IR identical for a/b/c at 1/2/4 threads, plus form-b application comparisons. However the complete measurement exits 1: form-a gate 2 tip error 0.000912 mm exceeds the accepted 0.000867 mm limit; production CLI and separate application diagnostics reproduce it. No gate waiver or rounding-method acceptance is authorised. Tracking and the precision/comparison/product question must be resolved through the task coordinator and an authorised lane; merge coordinator makes no allocation or policy decision here.

Decision 0211 remains PROPOSED; OI-834, STEPALL-X order and OI-835 remain unresolved. M1/M3, sampling bounds, unmet 0.005 mm tessellation allocation, five-run stage timings and full-stepover determinism remain unresolved/unwaived. Historical 1.24× remains invalid as current successful-job performance. Keep BLOCKED / IN PROGRESS and draft until verified completion and owner decisions resolve the explicit hold.
