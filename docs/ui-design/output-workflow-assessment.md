# OUTPUT-1: Post and Export workflow assessment

Issue [#306](https://github.com/paulhampson/OpenChuckCAM/issues/306).
Assessed against main `1a904356` and accepted decisions 0071, 0164, 0172,
0173, 0189, 0192 and 0233. This is an assessment, not a numbered decision
record or an implementation specification.

## Recommendation and owner answer

Replace the fifth view with an export action opening the existing dialog.
The fifth view largely repeats the shared tree, inspector and viewport; its
distinct responsibilities are navigation, export completion and health marks.

The owner approved **four steps** on 24 October 2026: Model, Setup,
Operations and Simulate, with **two separate buttons** on Simulate, one
triggering simulation and one triggering Post and Export. This supersedes
the assessment's initial suggestion of one primary button changing between
those actions. Neither button bypasses its prerequisites.

The separate build requested by #306 is
[#322](https://github.com/paulhampson/OpenChuckCAM/issues/322).
Its detailed implementation scope, button placement/styling, current-export
status and stale-state mapping still need agreement before building. The
workflow rebuild is not part of OUTPUT-1's confirmation and layout fixes.
No numbered record has been allocated or formally accepted for this choice.

## Responsibility mapping

| Responsibility | Current implementation | Recommended home |
| --- | --- | --- |
| Tree, geometry selection, operation forms | Shared across steps; Simulate has results/playback | Existing views, retaining selection/editing rules |
| Post choice, metadata, output grouping, destination | Export dialog | Same dialog |
| Tool-number review and renumbering | Export dialog and application commands | Same dialog and commands |
| Readiness and missing required values | Application overview/export checks, shown in menus, marks and dialog | Same checks, with explicit reasons on both actions |
| NC filenames, line counts and programmed time | Dialog's file list | Same dialog |
| Setup sheets | Both HTML and PDF written with G-code from the same run (0189) | Same behavior; no independent sheet action |
| Preview | First 14 G-code lines in the dialog | Same dialog; full browsing belongs to OUTPUT-3, #312 |
| Export completion, health and navigation | Fifth step's ring/health mark; overall bar; Alt+5 | Explicit current-export status in the remaining UI; exact location in #322 |
| Write history | Folder-level status after successful writes | OUTPUT-1 #307's persistent actual-file result and Messages history |

Retain File > Post and Export and Ctrl/Cmd+E. Keep Alt+1 through Alt+4;
propose Alt+5 as a dialog shortcut for existing users, subject to #322 scope
agreement. A historical successful write must not imply the current project
has been exported after an edit.

## Stale state and costs

Keep GC-15: stale or failed enabled toolpaths block writing; unsimulated
operations follow the existing warnings and explicit skip-stock
acknowledgement route. Preserve the accepted stale-result banners and
guarded semantic departures, including Keep/Discard/Cancel and eligibility
revalidation (0233). Two controls do not authorise exporting a stale run.
An unconditional enabled export action while stale would conflict with the
accepted rules; settle the precise action availability in #322.

UI-10 explicitly names five steps. The separate build must propose its
amendment and update specification sections 4, 5 and 11.6, traceability,
decision 0114's conformance accounting, rendered boards, keyboard help,
step/health/navigation tests and whole-shell smoke/layout regressions.
Inspect saved per-user step/layout state before deciding whether migration
is necessary; no project schema change is justified by this assessment.
Verify keyboard traversal, dialog focus entry/return, disabled explanations
and text/shape status cues (UI-11, UI-14), alongside UI-16 layout checks.

The chief risks are losing export discoverability or the completion mark,
confusing historical results with current readiness, obscuring checks in
scrolling content and bypassing pending-edit resolution. Keeping five steps
reduces migration work but retains a largely redundant destination. Moving
the dialog controls into Simulate would crowd its results and increase
shared-layout overlap.

UI-04/06/07 cover the surrounding selection/tree/forms; UI-10/12/16 cover
workflow, messages and layout. GC-05/12/14/15/16 and NFR-07/12 govern the
retained export responsibilities. Deterministic output remains GC-07/NFR-06.

## Implementation boundary

The owner supplied the shell/dock boundary and OUTPUT-1 agreed on 24 October
2026. SIMUI owns Playback contents/layout, simulation results/stale messages,
viewport display controls and operation navigation. OUTPUT-1 owns its
authorised output-specific panels/controls/commands and dock contents.
Shared dock framing, sizing, scroll infrastructure and tab dispatch stay
structurally stable. Shared shell files receive narrow integration changes
only; identify exact overlapping functions and agree one-lane responsibility
and sequencing before changes. No extra integration or refactor PR.

OUTPUT-1's #311 fix uses the existing reserved-scrollbar helper only within
the export body and keeps its fixed footer, leaving the body available for
OUTPUT-3's preview. OUTPUT-2 retains filename templates, hashes and setup-sheet
expansion. #307's explicit-confirmation policy was approved by the owner on 24 October 2026; the export record was accepted directly after full-text review as 0238 on that date, and is now [0240](../decisions/0240-export-write-results-and-replacement-confirmation.md) after administrative renumbering by Merge Manager; content and acceptance are unchanged.

## Local validation of the #311 layout fix

- On merged main `1a904356`, the requirement-first error and file-list tests
  failed because the floating scrollbar overlapped long error text and the
  setup-sheet filenames (0.120 s and 0.314 s, uninstrumented).
- Both tests pass with reserved scrollbar space. They draw the complete
  dialog at its default 980 × 700 and a constrained 720 × 460 requested
  size, fitted within 1366 × 768 and 1920 × 1080 windows, at 100% and 120%.
  They check glyphs and full control rectangles against the scrollbar, full
  button bounds, fixed action positions before/after scrolling and access to
  the verification notice. The file fixture generates the dialog's chosen
  post/grouping and checks that its plan matches its preview files. Temporary
  projects are cleaned on assertion failure too; no user destinations are
  written by these layout tests.
- The final combined application/UI run passed all 915 tests in 530.411 s,
  including existing whole-shell, Playback dock, pending-edit, readiness/stale
  and export regressions. Additional final feedback/layout/guard tests are
  recorded below with the changed-crate coverage report. The first combined
  build ran out of disk during linking before tests ran; generated artifacts
  were cleaned and validation restarted without incremental/debug data.
- Existing reference-job determinism took 113.680 s (already long); an
  existing workholding direction test took 64.814 s and is now classified by
  exact name in coverage/long profiles. No test exceeded ten minutes.

## Validation of persistent feedback and replacement policy

Requirement-first tests initially failed on lost Messages output (1.186 s),
missing plan/report APIs, and empty preflight acceptance (1.216 s), before
implementation. Controlled temporary destinations exercise single/split NC,
both sheets with a fixed clock and exact unchanged bytes, zero-write
preflight/cancellation, replacement, changed destinations and stale inputs,
partial failure, and remaining files not attempted. UI tests cover retained
actual paths, dismissal/Esc, Copy paths and history; a whole-shell test checks
Keep/Cancel before writing and guarded history reopening after another edit.
A later whole-shell regression failed on Esc consumed by the global
background-job cancel command (5.709 s), and step navigation behind a modal
failed in 2.508 s. The owner assigned OUTPUT-1 the narrow central
`Shell::shortcuts` fix: use egui's existing active-modal signal in the existing
early return. This also protects tool-library confirmations without a new
per-window keyboard handler; no dock/Playback restructuring or extra PR.

Complete/failed/partial results and long replacement lists have whole-modal
bounds, reserved-scrollbar and fixed-action checks at 100%/120%, including
maximised 1920 × 1080 and a constrained window.

Final validation after the central guard fix:

- The focused whole-shell regression passed in 2.307 s; all 434 UI tests then
  passed in 135.314 s. The preceding seven feedback/layout/guard tests passed
  in 6.186 s before the added global-shortcut assertions.
- Final all-target clippy with warnings denied, formatting, whitespace and
  all xtask checks pass: dependencies, licences, records, testdata, conflicts,
  publish and traceability (99/99 P1 requirements).
- Own-application coverage: 481 tests passed in 910.800 s, with two existing
  long tests excluded by the coverage profile. Line coverage is **92.18%
  (15,884/17,232)**; `output_write.rs` is 79.39% (181/228). Both are below
  the 95% development target, report-only under decision 0102. New uncovered
  paths were read: malformed/duplicate internal filenames, metadata and
  directory-creation failures, write-all/cleanup failures, temporary-name
  collision/exhaustion, early stale preflight and a checks refusal. The
  UI-facing retained-report accessor is not reached by own-app tests; it is
  exercised by the UI integration tests. Controlled partial writes and
  failed replacement/temporary cleanup are tested. No product exclusions
  were added. `cargo llvm-cov clean --workspace` completed for this report.
- Own-UI coverage: all 434 tests passed in 448.337 s; line coverage is
  **86.69% (24,072/27,769)**, below the 95% development target. The export panel
  is 91.47% (1,062/1,161); the new feedback module is 99.21% (126/127).
  Read new uncovered paths: a missing pending replacement's defensive exit,
  non-I/O preflight fallback, acknowledgement-store failures, and the
  opposite-kind Esc arm in each generic modal instantiation (both actual Esc
  actions are tested). Native picker and older numbering/error paths also
  remain uncovered. Shared report dispatch/refresh and the central modal
  shortcut guard were covered. No product code was excluded. Both
  `cargo llvm-cov clean --workspace` and final `cargo llvm-cov clean` completed.

These are headless Linux frame checks, not native
Windows/macOS or GPU screenshots; native checks remain in OI-580. No workflow
removal, filename change, setup-sheet expansion or full preview is built.
Record [0240](../decisions/0240-export-write-results-and-replacement-confirmation.md)
was administratively assigned by Merge Manager on 24 October 2026 to resolve
the collision with SIMUI's 0238. The owner's direct acceptance of unchanged
export content under original 0238 is retained; this is not new acceptance.
The metadata-only correction compares the normalized record with the
accepted source at `ff3cf897`: only the heading number and allocation
metadata are normalized; substantive content and Status are unchanged.
No new OI or schema.
