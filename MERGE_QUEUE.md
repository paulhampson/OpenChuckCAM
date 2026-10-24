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
| 1 | BUG-EDIT | [#323](https://github.com/paulhampson/OpenChuckCAM/pull/323) | `codex/lane-bug-edit-pending-edits` | Released; f6aeb9d corrections pushed; current-head CI/review pending |
| 2 | STEPALL-M | [#321](https://github.com/paulhampson/OpenChuckCAM/pull/321) | `claude/lane-stepall-m-measure` | BLOCKED / IN PROGRESS — draft; not authorised for release |

## SWARF3 / #320 — MERGED

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

## SWARF3 completion — 24 October 2026

Merged head `87b557ed3b1bcfd252733c389d499ab103f393a2` using an exact-head SHA guard. Confirmed merge commit: `e590f4de67bbde573edf30982982d6d0c5d92707`. Latest main was verified as ancestor (behind_by 0) before merge. No branch-protection bypass or auto-merge used.

[CI run 40039352507](https://github.com/paulhampson/OpenChuckCAM/actions/runs/40039352507) and end-to-end run 40039352328 completed SUCCESS on the merged head. All required CI jobs including macOS tests and CI required checks succeeded; fuzz skipped by path filter. CodeRabbit status SUCCESS; lockfile-only final update skipped by configured review filters, source reviewed on parent; its sole minor suggestion withdrawn and thread resolved. No unresolved review findings. Local records/traceability passed, 99/99 P1 requirements. Owner acceptance of 0228/0229 preserved; OI-970 stays open.

The single batched correction updated only transitive Tokio version/checksum to 1.53.3 for RUSTSEC-2026-0386 after both previous CI and review finished. No new record accepted.

Next queued #321 remains explicitly blocked/draft. Newly discovered #323 is queued after it with its separate owner-decision approval hold; neither is released.

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

## BUG-EDIT / #323 — intake history (superseded by release below)

- Automatically queued under new-owner-draft intake policy: created 2026-10-23T23:59:07Z by paulhampson (ID 528002).
- Branch `codex/lane-bug-edit-pending-edits`; intake head `4b6ffa93feb40a60aa179b14da2f9ddc0d8a66bc`. Owner/draft state verified.
- Resolves pending form edits before navigation, selection, generation, dialogs, Save and close; fixes #304. Lane owns branch changes.
- Decision 0233 remains PROPOSED. OI-983 authorises interaction/implementation, not acceptance. Owner must see and approve the complete record before ready transition. No release or record acceptance inferred.
- Lane reports all 422 UI tests covered by uninstrumented runs, 37 BUG-EDIT requirement tests; changed-crate clippy, formatting and repository checks including 99/99 P1 traceability passed.
- UI coverage 85.93%, below report-only development target under 0102. New uncovered ranges and verification gaps documented in 0233; OS-native windows/close, GPU and Windows/macOS interactions not locally verified. Full CI/review remain pending release.
- Intake acknowledgement posted on PR; lane to monitor owner comments and await release before main update. Queue position is not release permission.

## Periodic-check continuity

Owner instruction on 24 October 2026: periodic checks must remain set. Automation `Coordinate OpenChuckCAM merge queue` verified enabled and reaffirmed with an hourly RRULE, Europe/London timezone, no end date. Keep enabled after merges and when queue is empty; notify only for meaningful changes, failures, completion or required owner action.

## Latest draft check — 24 October 2026

No new intake beyond already queued #323; bounded owner-draft search returned one existing entry. No open PR is ready. #321 remains at `9e981c2149ed9f8637b7bf06f9edecb7fd2f3dd5`, draft with its explicit replay/decision hold. #323 remains at `4b6ffa93feb40a60aa179b14da2f9ddc0d8a66bc`, draft pending full-record owner approval of 0233, with main conflicts to resolve only after release. No owner approval or new lane handoff appears in PR comments. CodeRabbit skips both drafts; no inline review threads exist.

Draft CI runs 40042502111 (#321) and 40042638209 (#323) completed failure. Heavy jobs and end-to-end were skipped as expected for draft; required-check aggregators intentionally fail on drafts. Separately, both fast-check jobs genuinely fail cargo audit on Tokio 1.53.2 / RUSTSEC-2026-0386 (confirmed in logs), while their other fast checks pass. Main now contains the 1.53.3 patch merged via #320. Reconcile it during each authorised main update and verify audit, bundled with the permitted push; no standalone lockfile push, ready transition or gate waiver is requested now.

Full decision 0233 was fetched and read at its exact current head; it remains proposed. The owner still must be shown the complete record before acceptance is requested. No acceptance is inferred from OI-983, queue intake or this inspection.

## Owner priority and BUG-EDIT release — 24 October 2026

Owner instructed "#323 is good jump in front": #323 is now first, ahead of blocked #321. At new head `0c28d6d4db8532d06d20d74c24c753332dfb6d56`, PR body and decision 0233 Status record the owner's explicit "Accept decision record 0233" after reviewing the complete record directly in the lane session, with acceptance/doc updates pushed on owner instruction. This supersedes the earlier proposed-status hold; no other decision acceptance is inferred.

No other PR was ready. Coordinator marked #323 ready FIRST, then posted release instructions for lane main update, conflict/register reconciliation, required checks including records/traceability/audit, and one push with exact-head report. Main includes SWARF3 and Tokio 1.53.3. Wait for full CI and CodeRabbit on that updated head before batching any necessary corrections. Lane retains branch/code ownership; coordinator handles verified-green merge. #321 retains its explicit draft/blocker hold. Hourly monitoring remains enabled.

## BUG-EDIT release progress — 24 October 2026

Release head `cb35309c27e2386ddec8bea0ee21938491c8a751` is pushed, merging main `e590f4de67bbde573edf30982982d6d0c5d92707` after the ready transition. No manual merge conflicts; lane verified register unions contain no duplicate rows, accepted 0228/0229/0233 and open-item changes preserved, SWARF3 Geometry and Tokio 1.53.3 retained.

[Lane verification](https://github.com/paulhampson/OpenChuckCAM/pull/323#issuecomment-6377740077): 423/423 UI tests pass uninstrumented (171.074 seconds; slowest 31.214 seconds), UI clippy/formatting/whitespace and repository checks pass, full traceability 99/99 P1, cargo audit passes. Fresh instrumented UI suite 423/423 passes, 85.94% lines (23,653/27,522); below report-only development target, documented verification gaps retained and artifacts cleaned.

[Current-head CI 40046712026](https://github.com/paulhampson/OpenChuckCAM/actions/runs/40046712026) remains in progress: fast checks, Linux/Windows build-tests, all UI smoke and standalone-library jobs succeed; macOS build/test still runs. STEP kernel, fuzz and Gate 3 skip by filter. End-to-end run 40046711977 succeeds.

CodeRabbit status SUCCESS for current head; incremental review reports no new actionable comments. Two earlier unresolved threads still require lane assessment: Major possible Keep stall for hidden pending fields (discussion_r4365353688), Minor tab-count validation (discussion_r4365353698). The possible indefinite modal stall is a potential genuine blocker regardless of the severity threshold; do not merge until verified or resolved. Lane acknowledges both and waits for BOTH full CI and review before a bundled required correction push. No early correction push requested. Minor/nonblocking changes follow next-PR policy unless riding an already required push.

#323 is the sole ready PR. #321 remains unchanged and draft on its explicit hold. No new eligible owner drafts found (bounded search below 100 results). Hourly monitoring continues.

## BUG-EDIT correction stage — 24 October 2026

CI run 40046712026 and offline end-to-end run 40046711977 are now completed SUCCESS for `cb35309c27e2386ddec8bea0ee21938491c8a751`. CodeRabbit status SUCCESS; both prior review threads remain unresolved, with no lane response yet. Full CI and review have both reported, so coordinator posted the bundled-correction instruction.

Lane must verify the hidden-field Keep-stall finding against accepted 0233, reproduce with a requirement-based regression, and fix if genuinely blocking without silently discarding pending values or changing accepted interaction semantics. If invalid, give concrete evidence and resolve appropriately. Assess tab-count Minor too; no independent minor-only push, but it may ride an already required correction push. Report exact head, checks and thread dispositions after ONE bundled push. Merge stays blocked on disposition/correction of the possible indefinite modal stall despite green CI.

No new eligible drafts; #321 unchanged and explicitly held. Hourly checks continue.

## BUG-EDIT bundled corrections pushed — 24 October 2026

New head `f6aeb9d322a8065a5fbee046130e36ef00e0bec9`; [lane report](https://github.com/paulhampson/OpenChuckCAM/pull/323#issuecomment-6381146437). Lane reproduced the hidden-field Keep stall (red in 2.206 seconds), fixed recovery while preserving drafts and accepted Keep/Discard/Cancel semantics, and added stable semantic field identities. Tab-count range regression also failed first (1.031 seconds) and its fix rides this required push. Both original review threads are now resolved with evidence.

Lane reports 425/425 UI tests passing uninstrumented (131.501 seconds), 39 BUG-EDIT tests, placement-pick regression 20 consecutive passes; clippy/formatting/repository checks, traceability 99/99 P1 and audit pass. Instrumented suite 425/425; UI coverage 86.44% (report-only), shell guard 95.86%; artifacts cleaned and documentation updated without accepting any new record.

Current-head CI 40071939548 and end-to-end 40071939553 are IN PROGRESS; CodeRabbit is PENDING. Do not merge based on parent-head green checks. Hourly monitor verified enabled; last recorded run 24 October 2026 at 06:14 UTC (07:14 Europe/London). Keep checks enabled after completion.
