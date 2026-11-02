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

1. **A setup's missing stock or part is a to-do.** Setup health is *Needs attention* (amber, triangle outline), with the reason "Set the stock" (or "Choose a part"), when the setup lacks only its stock or its part. The requirement stays in the list of required values (decision 0173 item 1) and still stops generation with its own message (NFR-07); only the step's colour and word change.
2. **Errors stay errors.** A setup in error (`ItemState::Error`), a model that cannot be read, and an enabled operation that lacks a tool or a required value are still red ("Error to fix"). The overall health is still the worst of the steps.
3. **Messages agree with the bar.** As for the machine (decision 0173), the Messages list counts no error for a setup's missing stock or part.
4. **Wording.** The reasons are "Set the stock" and "Choose a part", matching "Choose a machine".

## Consequences

- The tutorial's Setup screenshot and badge text change (DOCSUSER re-captures them).
- Code: `openchuck-app::overview` (`is_error_need`, `setup_health`) and the requirement texts; no project format, library API or G-code change.
- Built after lane STATUSICON (#482) merges, because it changes the status marks' inputs.

## Verification

To be filled in when built: tests named `req_ui_11_…` in `crates/openchuck-app/tests/overview.rs` (Add setup with no machine reads Needs attention, "Set the stock"; a missing part likewise; a missing tool or operation value stays Error; Messages count no error for the setup's gaps) and the step bar test in the UI.
