# 0237: Strategy dropdown and information panel (lane OPCREATE2; issues 289 and 290)

- **Status:** accepted by the project owner (24 Oct 2026, directly in the OPCREATE2 session, after the complete record was shown). The owner instructed: "I accept 0237". The owner also directly chose always-visible explanations with no tick box and a read-only inspector strategy. Final strategy strings still require separate approval (OI-986).
- **Date:** 24 Oct 2026.
- **Requirements:** OP-16, UI-06, UI-07, UI-14, UI-16, NFR-15; NFR-12 for wording; GC-07 and NFR-06 for unchanged output.
- **Amends:** decision 0204 item 1 and its proposed specification entry for New operation; interface specification sections 6 and 11.4. Decision 0204's read-only inspector and registration-owned descriptions remain.
- **Open items:** OI-985 (closed by acceptance of this record), OI-986 (approval of the complete final strategy explanation text).

Numbering: allocated by the lane on the owner's direct instruction to allocate its own numbers and allow the Merge Manager to report conflicts in PR review. Checked main and all fetched remote branches on 24 Oct 2026: the highest decision was 0236 and highest registered open item OI-984. No schema number is taken.

## Context

Issue 289 requests a dropdown and an aligned information panel because the strategy list is unclear and its description alignment is messy. Issue 290 requests clearer swarf wording. Both issues and their comments have been read.

Accepted decision 0204 specifies one creation dialog, no skip tick and a read-only inspector strategy. Current main implements those rules, rather than the two-step flow and existing explanation preference assumed in the lane brief. The owner has now confirmed that explanations always show, without a checkbox, and that the inspector strategy stays read-only.

OPCREATE PR 271 is merged at adb07bf1a7abc0a209ea82c4656cbe9484eff204. BUG-EDIT PR 323 is merged at 1a90435654c734c05e4fa3f110c4a7f3f8e5859f; both are ancestors of the lane base. Its accepted decision 0233 supplies the pending-edit policy. Fetched branches contain no strategy registration/description edits by FACING, FINSTEP or SWARF4, and no overlapping strategy edits by the other active lanes at this check. Changes here stay within presentation and explanation text; no CAM algorithm or registry reorganisation is proposed.

## Decision

1. **Keep one creation dialog.** Replace the scrolling strategy list with a dropdown showing the chosen strategy's plain name. Keep the existing name, setup, tool, material preset and selection behaviour, including the initial strategy choice and the library handoff. Picking another strategy changes the creation draft only; Add remains the project-edit boundary.
2. **Show one information panel below the dropdown.** The selected strategy's plain name and complete explanation share a fixed left edge and wrap within the available width. The panel always shows; there is no explanation checkbox, hidden mode, new preference or settings migration. Information is text only. Repainting, opening the dropdown or resizing must not reset the selection. If a setup change makes the selected strategy unavailable, retain the choice, show its reason and disable Add as today.
3. **Use registration data and existing eligibility.** Keep the registry's Roughing, Finishing and Specialist groups and order. Read names, descriptions and availability from Application::strategy_choices_for; do not identify STEP-only strategies by name in the UI. Every unavailable strategy remains visible and disabled, with its full reason shown as readable text in the open list, rather than available only on hover. Strategies that ignore selections continue to say so.
4. **Keep the inspector read-only.** Reuse the information-panel presentation under its Strategy heading. Do not add a strategy-change command or convert/reset an existing operation's parameters. The operation's editable name remains separate from its strategy name.
5. **Retain BUG-EDIT's shared policy.** Moving between fields, including the strategy dropdown in the creation form, keeps ordinary field confirmation. Leaving the form, closing it, opening the library, navigation and other dialog actions resolve pending edits through the completed shared guard. Do not add a second Keep/Discard/Cancel mechanism. Cancel restores the edited field's focus under that policy.
6. **Bound the whole dialog.** Keep the dropdown, information and form within the available window. Long information and constrained sizes scroll without covering controls or the footer. Verify the complete dialog at 1366 by 768 and 1920 by 1080 at 100% and 120%, plus supported scale endpoints 80% and 200%. Keyboard focus follows reading order; the dropdown opens, changes selection and closes using the keyboard. Disabled reasons must be readable without a pointer. Use existing theme typography and contrast; explanatory body text is at least 12 pixels at 100%.
7. **Approve wording separately.** Descriptions remain in each strategy's registration. Review every registered strategy, including Facing, against its actual behaviour. Present the complete final proposed text to the owner for explicit approval before replacing strings. Swarf must mention selected STEP wall faces, curvature along the wall, straight lines across its height for side contact, and warnings for some deviations. A ruled non-developable face still has straight rulings. Do not promise one-pass completion, exact contact on every blade or safe G-code.

## Specification amendment

- **Section 6, New operation:** Opens the single creation dialog of section 11.4. Strategy is a dropdown populated from registered strategies, grouped in registry order, with unavailable entries visible and disabled and their reasons readable.
- **Section 11.4, New operation:** The name is followed by the Strategy dropdown and an always-visible, text-only information panel for the selected strategy. The panel uses one left edge for the strategy name and its wrapped explanation. Setup, tool, library access, material preset and selection semantics remain as built. Invalid availability disables Add and shows the reason. Long content scrolls within the dialog without covering its controls or footer. No explanation-skipping option is provided.
- **Section 11.4, operation inspector:** After the operation's name and status, show the read-only Strategy heading, plain strategy name and the same information-panel presentation. Changing an existing operation's strategy is not offered.

## Alternatives considered

- **Keep the full list.** Preserves 0204's presentation but does not address issue 289's clarity and alignment complaint.
- **Two creation steps or a remembered explanation checkbox.** Adds navigation or hidden information; the owner chose always-visible explanations and no checkbox.
- **Editable inspector strategy.** Different strategies have different parameters; a conversion would need a separate reset/migration policy and could discard values. The owner chose read-only.
- **Hide unavailable strategies or maintain UI eligibility rules.** Conceals supported choices or duplicates the registry's rules; contrary to decision 0174 and OP-16.
- **Images or diagrams.** Outside the owner's text-only direction.

## Verification and consequences

Write requirement-based tests and observe failures before implementation. Cover every registered strategy's information, matching selection and explanation, disabled reasons, STEP/STL eligibility, selection persistence, keyboard focus, long text, whole-dialog bounds/alignment/non-overlap, and BUG-EDIT Keep/Discard/Cancel paths when leaving the flow. There is no preference persistence to test because the owner rejected that option; test that explanations always show instead.

Check that selecting strategies in the creation draft and viewing the inspector do not mutate existing project parameters, generation inputs, toolpaths or posted bytes. Existing Add semantics stay covered. Run uninstrumented timings, changed-crate coverage with cleanup, formatting, lint and required xtask checks; report gaps and stop for an owner decision if a test exceeds ten minutes.

Update the specification, verification plan, work breakdown and registers in the implementation PR after acceptance. No project schema, dependency or library API change is expected. Final strings require OI-986 approval independently of this design. Native platform and screen-reader checks not performed locally must be reported as limitations.

## UI checkpoint verification (24 Oct 2026)

The presentation is implemented locally. Final registration strings remain unchanged;
the complete replacement proposal is in [0237-strategy-explanations.txt](0237-strategy-explanations.txt),
presented separately for OI-986 approval. No PR is opened at this checkpoint.

Requirement-first regressions failed before fixes: the closed list showed multiple
explanations; disabled reasons required hover; the old creation window lacked the
bounded layout; keyboard selection/Escape did not restore dropdown focus; valid
name-to-choice transfers were not registered as same-form focus changes. The
fixes use the existing pending-edit registry, including the creation form's Setup,
Tool and Material preset choices, rather than introducing another policy.

- Final uninstrumented UI suite: **436 passed in 138.062 s**. All new tests are
  below the one-minute long-test threshold; no classification change is needed.
- Instrumented UI suite: **435 passed in 501.560 s**, followed by **12 focused
  tests passed in 6.267 s** after the last same-form registration additions, with
  profiles retained. This is accumulated coverage, not a claim that the full
  instrumented suite was repeated after those additions.
- UI line coverage: **22,734 / 26,268 (86.55%)**, calculated from the UI source
  records in the LCOV export with the required tests/examples/benches directory
  exclusions. The 95% development target is not met; decision 0102 makes it
  report-only. New picker/information, selection, keyboard-focus and shared-guard
  logic is reached. The existing Cut on selection-note branches, moved by the
  layout edit (`run_panel.rs` lines 639–640 and 642–644 at this checkpoint), are
  not reached by this crate's tests. The shape collector's fallback in the
  information test helper is also uncovered; it is test code. No coverage-only
  exclusions were added. `cargo llvm-cov clean --workspace` completed afterwards.
- Changed-crate clippy, all targets with warnings denied; formatting and whitespace;
  and `cargo xtask all` passed. Traceability reports 99/99 P1 requirements with tests.
- A real generation/posting comparison confirms that inspecting information and
  choosing a creation draft strategy, then cancelling, preserve the project,
  toolpath and G-code bytes.

Checks draw headless egui frames. They cover keyboard input, focus, layout and
pending-edit actions, not native Windows/macOS interaction, GPU rendering or a
screen-reader session. STEP/STL UI coverage uses the application's eligibility
metadata; it does not claim to import a synthetic STEP file or validate swarf
geometry. Those limits remain explicit for later platform and release checks.
