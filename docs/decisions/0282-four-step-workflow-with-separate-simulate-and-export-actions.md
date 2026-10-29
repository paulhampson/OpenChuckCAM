# 0282: Four-step workflow with separate Simulate and Post and export actions (lane FOURSTEP)

- **Status:** accepted (30 Oct 2026) by the project owner, who read the condensed record (items 1 to 9, the UI-10 wording, the Simulate button placement and enablement, Alt 5, the "G-code written / not written" status item, health as the worst of four, the 0114 accounting change) in the coordinator session and answered "Approve". The acceptance was relayed to this lane by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7), as decision 0049 allows. Proposed on 29 Oct 2026. The owner approved the direction on 24 Oct 2026 in the OUTPUT-1 chat: "Have two buttons - one for triggering simulation and the other for triggering post and export. I approve going to 4 step flow." On 29 Oct 2026 the owner chose option A for each of the six questions in the coordinator session (OI-1140 to OI-1145), relayed to this lane by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7, decision 0049), and agreed the UI-10 wording of item 1 as proposed. Those answers are recorded in items 1 to 6.
- **Date:** 29 Oct 2026.
- **Lane/issue:** FOURSTEP, [#322](https://github.com/paulhampson/OpenChuckCAM/issues/322) (follow-up of the OUTPUT-1 assessment, #306).
- **Requirements:** UI-10 (amended, item 1), UI-11, UI-12, UI-14, UI-15, UI-16, GC-15, GC-16, NFR-12, NFR-15; GC-07 and NFR-06 (output bytes unchanged).
- **Builds on:** [OUTPUT-1 assessment](../ui-design/output-workflow-assessment.md); [0240](0240-export-write-results-and-replacement-confirmation.md) (accepted: preflight, replacement confirmation, write results and Messages history); [0267](0267-output-file-names-sheet-hashes-and-staged-writes.md) (accepted: file names, sheet hashes, staged writes); [0238](0238-simulate-display-and-operation-navigation.md) (accepted: Simulate display, out-of-date banner, guarded row navigation, SIMUI boundary); [0233](0233-unconfirmed-edits-before-leaving-a-form.md) (accepted: pending-edit guard); [0189](0189-setup-sheet-written-with-the-g-code.md), [0172](0172-stale-state-and-removed-paths.md), [0173](0173-status-bar-and-required-values.md), [0192](0192-simulate-result-out-of-date.md), [0148](0148-menus-and-view-directions.md) (post blocker), [0117](0117-slice-22-shell-frame.md) (per-user settings), [0114](0114-slice-22-conformance-report.md) (conformance accounting).
- **Amends:** requirement UI-10 (item 1; wording agreed by the owner on 29 Oct 2026 and edited in this change); UI specification sections 2, 3, 4, 5, 10, 11.1, 11.5, 11.6, 16 and 20; verification plan rows UI-10 and UI-14; decision 0114's C2 accounting (five steps become four). No architecture text changes. No project schema change. No library-family crate changes (`openchuck-app` and `openchuck-ui` only), so no library version bump.
- **Open items:** OI-1140 (action placement and styling), OI-1141 (when each action is enabled), OI-1142 (Alt 5), OI-1143 (the current-export indication), OI-1144 (overall health), OI-1145 (requirement and specification wording): each closed by the owner's option A on 29 Oct 2026 (relayed). Native Windows and macOS display checks of the step bar and footer stay with OI-580. No new open item.

## Context

The fifth step, Post and export, shows the same tree, inspector and viewport as the other steps. What only it has is: a place in the step bar (with a green ring when G-code has been written from the project as it is now), its health mark, Alt 5, and a footer button "Post and export…" that opens the export dialog. Everything that writes files is already in the dialog. The OUTPUT-1 assessment recommended replacing the step with an action, and the owner chose four steps with two separate buttons on Simulate.

### Where the fifth step appears today (main `03c8d18`)

| Place | What it does |
| --- | --- |
| `crates/openchuck-app/src/overview.rs`: `Step::Export`, `Step::ALL` (5), `Step::name`, `Step::next` | The step itself; Simulate's "next" is Export |
| `overview.rs` `steps` (the `state(Step::Export, post_blocker, any_enabled && exported_all, export_health)` row) | Availability is the post blocker (0148); `complete` is "G-code written from every enabled operation as it is now"; health is Attention with the post blocker's reason, else Good |
| `overview.rs` `Overview::primary` | Simulate: "Continue to post and export" once simulated; Export: "Post and export…" (`PrimaryKind::OpenExport`), disabled by an enabled item's blocking error or the post blocker |
| `overview.rs` `Overview::health` | Worst of the five |
| `overview.rs` `RunRecord::exported`, `Application::record_export` (`lib.rs`, called only when a write is complete) | The record behind the green ring |
| `apps/openchuck-ui/src/shell.rs` `STEP_KEYS` (Num1 to Num5), `Shell::shortcuts`, `Shell::primary` | Alt 1 to Alt 5; the footer primary calls `open_export` |
| `apps/openchuck-ui/src/shell/frame.rs`: `KEY_MAP` ("Alt 1 to Alt 5"), View menu (one item per step), step bar (`step_tab` for each of `Step::ALL`, green ring), `health_badge`/`worst`, `step_purpose`, `step_note`, `view_step`, `empty_state` (lists the steps) | The step bar, its ring and health, the View menu, the empty state's list and the keyboard help |
| `apps/openchuck-ui/src/viewport.rs` `ViewStep::Export` | The view's step; nothing differs from the other steps |
| `apps/openchuck-ui/src/export_panel.rs` (comment at `finish_write`) | Says the write completes "the Post and export step" |
| Tests: `crates/openchuck-app/tests/overview.rs` (availability, reasons, primary, `complete`, health), `apps/openchuck-ui/src/shell/smoke_tests.rs` (Alt 5 to `Step::Export`), `frame_tests.rs` ("Continue to post and export", the step's availability, the View menu), `import_layout_tests.rs`, `toolbar.rs`, `viewport.rs` | Five-step expectations |
| Docs: requirement UI-10; specification sections 1 (board list), 2 (principle 2), 3 (workflow bar), 4, 5, 11.1, 11.6 ("Opened from step 5"), 16 (Alt 1 to Alt 5), 20 (boards); verification plan UI-10 and UI-14 ("the five steps"); decision 0114 C2 | Five-step wording |
| Boards: `Main`, `Main1366`, `Model`, `Setup`, `Simulate`, `Start`, `Workholding`, `OpError`, `OpMissing` (step bar); `Simulate` (footer); `Export` ("step 5") | Five steps drawn |

The current step is not kept in `settings.toml` (`UserSettings` in `crates/openchuck-app/src/settings.rs` has no step); the shell opens a project on Model or Operations. Nothing per user refers to the fifth step.

## Decision

Items 1 to 6 are the owner's answers (option A each, 29 Oct 2026, relayed). The recommendations and alternatives are kept as they were put.

1. **Four steps; Post and export is an action (OI-1145: A, wording agreed).**
   - The steps are Model, Setup, Operations and Simulate. `Step::Export` and `ViewStep::Export` are removed; Simulate has no next step.
   - Post and export is reached from: a button on Simulate's footer (item 2), File > Post and export… (unchanged), Ctrl E or Cmd E (unchanged), and Alt 5 if the owner keeps it (item 4).
   - The dialog is unchanged: post choice and read-only post values, grouping, output folder, file names (0267), tool numbers (GC-14), checks before writing (GC-15), the NC file list with names, line counts and programmed time, the existing first-lines preview, the setup sheet in HTML and PDF from the same run (0189), the NFR-12 notice and GC-16 and skip-stock acknowledgements, preflight and replacement (0240), write results and Messages history (0240). OUTPUT3's full preview (#312) is not part of this lane.
   - **UI-10 wording** (agreed by the owner on 29 Oct 2026, edited in requirements.md): "Organise the work as ordered steps (model, setup, operations, simulate) that the user can move between at any time, with post and export as an action of its own, on the simulate step and in the menu. A step or action whose inputs do not exist yet is unavailable and says why."
   - **Specification wording** is in the appendix (sections 2, 3, 4, 5, 10, 11.1, 11.5, 11.6 and 16), with the verification plan rows, edited in this change as agreed with answers 1 to 5 applied.

2. **Placement and styling (OI-1140: A).** On Simulate the footer's right end holds two buttons in fixed places, in workflow order: the simulation action, then "Post and export…" right-most. Neither ever changes into the other, and neither moves. Exactly one is filled (primary), by the section 5 rule:
   - while the simulation is not current (section 5 rules 1 to 3: "Generate stale (n)", "Run simulation", "Generate and simulate op N", or with stock simulation skipped "Generate toolpaths"), the simulation action is filled and "Post and export…" is a plain (secondary) button to its right;
   - once the simulation is current, "Post and export…" is filled and the simulation action is plain.
   - Section 5's "right-most" rule gets one stated exception for Simulate: the two actions keep their places and the filled one is the next required step. Every other screen keeps one filled button, right-most.
   - Recommended over: (B) keep the filled button right-most by swapping the two buttons' places when the next step changes (a button that moves under the pointer and in the tab order); (C) the simulation action always filled (two different "next steps", against principle 6).

3. **When each action is enabled (OI-1141: A). No accepted check changes.**
   - **Simulation action:** exactly today's Simulate primary (section 5; 0168, 0172, 0192): disabled by an enabled item's blocking error, naming it and highlighting its message; disabled while a job runs ("Wait for the running job, or cancel it"). When the simulation is current it reads "Run simulation", disabled, with "The simulation matches the project" (while the stock simulation is skipped: "Generate toolpaths", disabled, with "The toolpaths match the project"); with every operation turned off it is disabled with "Turn on an operation to simulate", and the out-of-date banner keeps no run button (0238). The viewport's "Simulation out of date" banner and its "Run simulation again" (0238, SIMUI) stay as they are.
   - **Footer "Post and export…":** exactly today's fifth-step footer button: disabled by the post blocker of 0148/0173 item 6 (with its reason: "Fill in the missing values first" and so on) or an enabled item's blocking error (naming it); disabled while a job runs, as every footer action is. It is not disabled because the simulation is stale or missing: the dialog keeps GC-15 (stale or failed enabled toolpaths block writing; operations not simulated and unconfirmed machine values warn; skipped stock needs its acknowledgement), the current-file checks and the 0240 revalidation. Two buttons do not allow writing from a stale run.
   - **File > Post and export…, Ctrl E (and Alt 5):** unchanged: the post blocker only (0148).
   - The pending-edit guard (0233) runs before either button acts, as before (`Shell::defer`).
   - Recommended over: disabling "Post and export…" while the simulation is stale (stricter than the File menu and Ctrl E, so the same command would be enabled in one place and not another, and it changes 0148 by implication).

4. **Alt 5 (OI-1142: A).** Keep Alt 5 as a compatibility shortcut that does exactly what Ctrl E does (same blocker, same guard). Help > Keyboard shortcuts and section 16 list "Alt 1 to Alt 4: Workflow steps" and "Alt 5: Post and export (as Ctrl E)". The View menu lists the four steps; it does not list Post and export (File does). Alternative: drop Alt 5 (one less rule to learn, but a user of the fifth step finds the key does nothing).

5. **The current-export indication (OI-1143: A).** The green ring of step 5 goes, so the same fact needs a home: a status item in the step bar, right after step 4 and before the workflow-bar actions, visible on every step, not a button and not in the tab order as a control (it is exposed to screen readers as a label with its words; its tooltip opens on hover):
   - **"G-code written"** with the green ring the step used (a ring round a tick), when the last complete write was made from every enabled operation as it is now (the same test as today's ring: `exported_all`);
   - **"G-code not written"** with a hollow grey circle otherwise; its tooltip says either "No G-code has been written in this session" or "The project has changed since G-code was last written", so a past write does not read as current.
   - The tooltip says where past results are: "Past write results are in Messages". Messages keeps its history (0240) and is not changed.
   - It never says "safe", "verified" or "ready" (NFR-12); "written" is a fact about files.
   - The bar is narrower by one step, so this fits at 1366 × 768 (to be checked, item 8).
   - Alternatives: (B) only on Simulate's footer, beside the two buttons (visible on one step only, so less than the ring gave); (C) only in the health badge tooltip (hidden). Partial writes leave it "not written" (unchanged: only a complete write records an export).

6. **Overall health (OI-1144: A).** The badge shows the worst of the four steps, with the same words and shapes (0173). The fifth step's health was Attention only with a post blocker, and every post blocker is already counted by another step: no model (Model, Attention), no setup or machine (Setup), a missing value (Setup or Operations, Error), no operation (Operations, Attention), no tool (an enabled operation then misses its tool: Operations, Error; with every operation off, Operations is Attention). So the worst of four equals the worst of five for every project; a test proves it on the existing cases. Whether G-code was written does not change health, as today (0173: not exporting is not a problem). Alternative: make "not written" Attention (it would leave the bar amber after every edit, a new check by implication; not recommended).

7. **Per-user state (UI-15).** No step, ring or health is kept in `settings.toml`, so there is no migration and no new setting. Panel sizes, the export acknowledgements and the file name templates are unchanged. No project schema change.

8. **Tests and evidence (written first, from the requirements; each named by requirement).**
   - `openchuck-app` (`tests/overview.rs`, `src/overview.rs`): four steps in order and Simulate has no next; availability and reasons unchanged for the four; Post and export action availability and reasons (the post blocker, a blocking error), equal to the old fifth step's for the existing fixtures; current-export state true only after a complete write and false after an edit, a partial write or a refused write; health is the worst of four and equals the old worst of five on every existing fixture; Simulate's two actions (which one is filled in each case, labels, disabled reasons) including skip-stock and run-holds-gone cases.
   - `openchuck-ui` (shell rigs): two independently reachable buttons on Simulate in fixed places and tab order, one filled; Alt 1 to Alt 4 open the steps, Alt 5 (if kept) opens the dialog with the same guard and blocker as Ctrl E, and does nothing behind a modal; File > Post and export and Ctrl E unchanged; keyboard-only path: Tab to "Post and export…", Enter opens the dialog, and closing it (its own Cancel) gives the keyboard back to "Post and export…" (the dialog's Esc and footer are OUTPUT-1's and unchanged); an enabled footer action shows its tooltip on keyboard focus, and a disabled filled action's reason is in the footer status (egui gives no keyboard focus to a disabled button, so section 5's "the footer status says the same" is the keyboard route); the pending-edit guard runs before each action (Keep, Discard, Cancel); a running job disables both; the current-export item reads "not written", "written" after a complete write, "not written" after an edit, and has its text and shape; the View menu shows four steps; the empty state lists four.
   - Layout: the step bar with the indication and the Simulate footer with both buttons at 1366 × 768 and 1920 × 1080, at 100% and 120% (the X8 scales), nothing clipped or overlapping; the existing whole-shell smoke and dock layout tests.
   - Byte preservation: the existing export and setup-sheet tests run unchanged (names and bytes with the default templates, 0267 names and hashes, 0240 results); a test writes the reference project through the dialog opened from the Simulate button and from Ctrl E and compares the files byte for byte.
   - Boards: the step bar redrawn with four steps and the indication on the boards that show it, the Simulate footer with both buttons; images re-rendered at 100% (section 20).
   - Traceability (`cargo xtask trace`), decision 0114 C2 note, work breakdown, both changelogs (`openchuck-app`, `openchuck-ui`), open items and the decision index.

9. **Boundaries.** SIMUI (0238) keeps Playback, the results inspector, the out-of-date banner and row navigation; this lane changes only `Shell::shortcuts` (the step keys), `Shell::primary`, the footer's action buttons, the step bar's steps and the new indication, the View menu, the keyboard help and the empty state's list. OUTPUT-1 (#307, #311) and OUTPUT2 behaviour stays. OUTPUT3 (#312) is not absorbed. GATES' export gates are untouched. The command line is unchanged.

## Alternatives considered

- **Keep five steps.** Less change, but keeps a destination that repeats the other steps (assessment).
- **One button on Simulate that changes from Simulate to Post and export.** Rejected by the owner.
- **Move the dialog's controls into Simulate.** Crowds Simulate's results and the shared layout (assessment).

## How it is built

- `openchuck-app` (`overview.rs`): `Step` has four values; `Overview::export_action` (the old fifth-step button's rules), `Overview::simulation_action` (Simulate's own action, disabled with "The simulation matches the project" or "The toolpaths match the project" once current), `Overview::primary(Step::Simulate)` (the filled one), `Overview::g_code_written` and `Overview::wrote_g_code_before` (the step bar's status, from the existing export record). `PrimaryKind::Continue(Step::Export)` and the fifth `StepState` are gone; `Overview::health` is the worst of four.
- `openchuck-ui` (`shell.rs`, `shell/frame.rs`, `viewport.rs`): the step keys are Alt 1 to Alt 4 and Alt 5 calls `open_export` (the same guard, blocker and modal check as Ctrl E); `Shell::simulation` and `Shell::post_and_export` act through the pending-edit guard (`defer`) and the running-job check; the Simulate footer puts each button in its own place measured from the right edge, made in reading order; the step bar draws the G-code status after step 4; the keyboard help, the empty state, the step notes and `ViewStep` lose the fifth step.
- The export dialog, its checks, preflight, writes, names and bytes are unchanged; the command line is unchanged.

## Conformance accounting (decision 0114)

Decision 0114 counted section 4 (C2) as five steps. With this record C2 is four steps, the G-code status and Simulate's two actions, all built and tested here. The boards that drew the fifth step (images 00, 01, 02, 03, 04, 14, 15, 16) are re-rendered with the four steps (OI-342, lane by lane); images 03b, 03c, 05, 13 and 17 do not draw the step bar and are unchanged.

## Verification

As item 8. The results are in the lane's pull request: tests run and timed uninstrumented (`cargo test`; `cargo nextest` is not installed in the lane's container), coverage checked locally per crate (decision 0102).

## Appendix: proposed specification and verification-plan wording (for the owner's agreement, OI-1145)

**Section 2, principle 2:** "The four steps (Model, Setup, Operations, Simulate) follow requirements section 2 and architecture section 6, left to right. Post and export is an action at the end of Simulate."

**Section 3, workflow bar row:** "The four steps (section 4) and the G-code status on the left; …" (rest unchanged).

**Section 4:** remove row 5 of the table. Simulate's primary column: "Section 5 (two actions)". After the table, add:

> **Post and export** is not a step. It opens the export dialog (11.6) from Simulate's footer, from File > Post and export… and with Ctrl E [and Alt 5]. It is available when the project has a model, a setup, a machine, a tool and an operation, and no required value is missing (generated or not: the dialog makes its own files; decision 0148 item 1, amended by decision 0173 item 6); otherwise it gives the first missing item: "Import a model first", "Add a setup first", "Choose a machine first", "Add a tool first", "Add an operation first" or "Fill in the missing values first".

In "The step indicator": remove the "Post and export" complete bullet; "A badge … shows the worst of the four"; "Alt 1 to Alt 4 open the steps [; Alt 5 opens Post and export, as Ctrl E does]". Add:

> **The G-code status.** After step 4 the bar says whether G-code has been written from every enabled operation as it is now: "G-code written" with a green ring round a tick, or "G-code not written" with a hollow grey circle. Its tooltip says why ("No G-code has been written in this session" or "The project has changed since G-code was last written") and that past write results are in Messages. It does not change the health, and it is not a button.

**Section 5, add after "One per screen":** "On Simulate the footer has two actions in fixed places: the simulation action (rules 1 to 3) and, right-most, "Post and export…". Exactly one is filled: the simulation action while the simulation is not current, otherwise "Post and export…". The plain one stays enabled when its own rules allow." Rule 4 becomes: "Otherwise it continues to the next step; on Simulate, it is "Post and export…"."

**Section 10:** "the step's actions, with the primary right-most (section 5; Simulate keeps its two actions in fixed places)".

**Section 11.1:** "Steps 2 to 4 are dimmed with their reasons"; "the four steps, one line each".

**Section 11.5, Footer:** "Footer: the simulation action and "Post and export…" (section 5)."

**Section 11.6, first line:** "Board 5. Opened from "Post and export…" on Simulate, from File > Post and export…, or with Ctrl E [or Alt 5]." The rest of 11.6 is unchanged, except "Full batch success marks export complete" becomes "Full batch success shows "G-code written" in the step bar (section 4)".

**Section 16:** "Alt 1 to Alt 4 | Workflow steps" [and "Alt 5 | Post and export (as Ctrl E)"].

**Verification plan:** UI-10: "View-model test: each of the four steps' availability and reason, and the Post and export action's, for an empty project, a model only, a setup, and a generated operation; the demo script walks the steps and opens the export dialog from Simulate". UI-14: "… a keyboard-only demo script through the four steps and Post and export, with tooltips opened by focus".
