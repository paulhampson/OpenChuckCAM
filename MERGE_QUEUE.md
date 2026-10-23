# OpenChuckCAM merge queue

Updated: 23 October 2026 (Europe/London).

Only lanes handed to the merge coordinator by the owner or task coordinator enter this queue. Only one open PR may be ready for review at a time. The lane owns its branch, code, register reconciliation and review corrections; the merge coordinator owns release order and merging.

## Planned queue

| Position | Lane | PR | Branch | Status |
| --- | --- | --- | --- | --- |
| 1 | SWARF3 | [#320](https://github.com/paulhampson/OpenChuckCAM/pull/320) | `claude/lane-swarf3` | Ready; lane verifying local main merge; single release push pending |
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

## Current release blocker

At head `96258e867712ac9ee47000904715a6d956e30727`, GitHub reports the PR unmergeable and CodeRabbit flags merge conflicts. No pull-request Actions runs exist for this head: GitHub suppresses these workflows while conflicts remain. The lane must perform the already authorised main update, resolve conflicts and register unions, verify, and push once before full CI can run. The dedicated lane has acknowledged release and is verifying its local merge; the new remote head remains pending. Do not wait for CI before this release update, or rerun old draft checks as a substitute.

## SWARF3 release progress

The lane's [latest report](https://github.com/paulhampson/OpenChuckCAM/pull/320#issuecomment-6376159612) confirms its execution environment recovered. Locally, main `a2605495d4e05d323d4cc3f85a4f4d6be118072f` is merged, both changelog conflicts are resolved with entries preserved, the decision index is ordered and duplicate register IDs were not found. Formatting, `cargo xtask records` and `cargo xtask all` pass, including workspace-built traceability: 99/99 P1 requirements have tests. These are lane-reported local results, not CI results for a pushed release head.

Focused uninstrumented tests and changed-crate clippy are still running before the single release push. Remote head remains `96258e867712ac9ee47000904715a6d956e30727`, unmergeable with main; no PR Actions runs exist for it. CodeRabbit reports success with no actionable findings on that old head and no inline review threads; review and CI must be checked again after the release push. No correction push is requested while lane verification proceeds.

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
