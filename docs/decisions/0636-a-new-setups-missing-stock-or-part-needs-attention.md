# 0636: A setup's missing stock or part reads "Needs attention", not "Error to fix" (lane UIBUGS, issue #463 item 3)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator session (decision 0049), unchanged, after reading it in full. The owner's answer it records (a setup's missing stock or part reads "Needs attention", a missing tool or operation value stays red) was relayed the same way.
- **Date:** 2 Nov 2026.
- **Open items:** OI-1989 (this change; closes with it).
- **Amends:** decision 0173 item 3 (Setup health).

## Context

Requirements: UI-10, UI-11 (state by shape and colour, "an item in error says why and how to fix it"), NFR-07. Issue #463 item 3 (lane DOCSUSER's tutorial re-capture): the Setup step shows a red "Error to fix" while the Model step says "Needs attention", before a machine is chosen.

Reproduced on `main` (`484e4b2`) with `crates/openchuck-app/tests/overview.rs`-style fixtures: import a model with no setup and no machine and the Setup step reads "Needs attention: Choose a machine" (decision 0173, FIRSTRUN item 1, as intended). After **Add setup** (`new_setup`: a part, no stock) and with no machine, Setup reads "Error to fix: Setup 'Setup 1' needs: set the stock". The red appears the moment the user clicks Add setup, before they could have set the stock.

Decision 0173 item 3 made a missing stock or part an error. Decision 0173 later (FIRSTRUN item 1) made a missing machine a to-do. A new setup's missing stock is the same kind of thing: nothing is wrong yet, the user has not reached that step.

## Decision

1. **A setup's missing stock or part is a to-do.** Setup health is *Needs attention* (amber, triangle outline), with the reason "Set the stock" (or "Choose the part", the text of the requirement), when the setup lacks only its stock or its part. The requirement stays in the list of required values (decision 0173 item 1) and still stops generation with its own message (NFR-07); only the step's colour and word change.
2. **Errors stay errors.** A setup in error (`ItemState::Error`), a model that cannot be read, and an enabled operation that lacks a tool or a required value are still red ("Error to fix"). The overall health is still the worst of the steps.
3. **Messages agree with the bar.** As for the machine (decision 0173), the Messages list counts no error for a setup's missing stock or part.
4. **Wording.** The reasons are the requirement texts: "Set the stock" and "Choose the part", like "Choose a machine". With both a stock and a machine missing the step names the stock, the nearer step once a setup exists.

## Consequences

- The tutorial's Setup screenshot and badge text change (DOCSUSER re-captures them).
- Code: `openchuck-app::overview` (`is_error_need`, `setup_health`) and the requirement texts; no project format, library API or G-code change.
- Built after lane STATUSICON (#482) merges, because it changes the status marks' inputs.

## Verification

Built on `main` `b3407ef7` (lane UIBUGS, second pull request). Tests, written from this record before the code and seen to fail first:

- `req_ui_11_req_ui_12_a_new_setups_missing_stock_is_attention_not_an_error` (`crates/openchuck-app/tests/overview.rs`): import a model, Add setup, no machine: Setup is Needs attention, "Set the stock"; the bar's health is below Error; Messages count no error; the value is still listed for the setup; with a machine chosen the step still names the stock.
- `req_ui_11_req_ui_12_a_missing_part_is_attention_and_a_missing_tool_stays_an_error`: a setup with no part is Needs attention, "Choose the part", and is listed; an enabled operation with no tool still turns the step and the bar red.
- `req_ui_11_a_missing_tool_is_red_and_a_missing_stock_needs_attention` (the earlier test of decision 0173 item 3, renamed): the stock half now expects Needs attention; its Post and export blocker and first-problem jump are unchanged.
- `req_ui_11_a_setup_without_stock_says_so_under_its_section_and_in_the_bar` (`apps/openchuck-ui/src/shell/frame_tests.rs`): the bar's Setup step is Needs attention with "Set the stock"; the jump, the "1 value is missing" line and "Set the stock" under the section are unchanged.

Run uninstrumented with `cargo test` (nextest is not installed in the session): all 668 `openchuck-ui` bin tests (the run takes about 8 minutes), the 233 `openchuck-app` library tests, and 129 `openchuck-app` integration tests selected by name (stock, requirement, health, missing, setup, status, overview) pass; the `overview` module's 52 pass. Coverage was not run (`cargo-llvm-cov` is not installed in the session); every changed line is reached by the tests above.

Not changed: the item marks and tooltips of the inspector (a setup row's own state is unaffected), generation (a missing stock still stops it with its own message, NFR-07) and the Post and export blocker.
