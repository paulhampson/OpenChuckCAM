# 0173: The step bar's status and the list of required values (lane BUG-STALE, part 2)

- **Status:** proposed (21 Oct 2026). Only the project owner accepts a record.
- **Date:** 21 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-621 to OI-625 are new and open; OI-692 and OI-693 (the two CodeRabbit findings deferred from decision 0172, pull request 229) are closed by this change. Amends decision 0148 item 1 and the interface specification sections 4 and 8.

## Context

Trial findings F-54 (issue 210) and F-68 (issue 224), with issue 140 to verify. Requirements: UI-10, UI-11 (state by shape as well as colour, "an item in error says why and how to fix it"), UI-07, NFR-07 (invalid input gives a clear error), NFR-15 (contrast). The owner's answers: each step also shows its own status, an incomplete setup (a missing tool or stock) is red (issue 210); the highlight appears as soon as the operation is added, calm on a fresh operation (issue 224).

Before this change the bar showed only a green ring for a complete step, the inspectors showed no mark for a missing value, and "Post and export" needed a model, a setup, a machine, a tool and an operation but not a stock or the values an operation needs.

## Decision

1. **One list of required values** (`openchuck_app::requirements`). For a setup: the project's machine, the setup's stock and its part. For an operation: a tool, and each parameter of its strategy that is a number with no default and that neither the operation nor the tool's cutting data (for the setup's material) gives. The parameters come from the strategy's registered schema, so a new strategy supplies its own; the interface has no list. When there is no tool, the feeds and stepover are not listed on their own (they come from the tool's data), so a fresh operation without a tool has one line, "Choose a tool". Each entry has a label and a short plain sentence ("Choose a tool", "Set the stock", "Enter feed").
2. **Generation checks the same values (NFR-07).** `parameter_values` builds its "is required" error from the same function, so the list and the error cannot disagree. A missing tool, stock, machine or part still stops generation with its own message; the marks do not replace those checks.
3. **Step health.** Each step has a health: *good* (green, a filled circle), *needs attention* (amber, a triangle outline) or *error* (red, a filled triangle), the shapes of the item marks (UI-11), and a reason for the tooltip. The whole job's health is the worst of the five.
   - Model: error when a model cannot be read; attention with no model or with model warnings.
   - Setup: error when a machine, a stock or a part is missing, or a setup is in error; attention with no setup or with setup warnings (this includes machine values not confirmed on the machine, OI-624).
   - Operations: error when an enabled operation lacks a value or is in error; attention with no operation, with stale operations or a warning.
   - Simulate: attention until every enabled operation is simulated as it is now (OI-625).
   - Post and export: attention when its blocker applies.
   - A step out of reach reads as attention, so the job is never green while a step cannot be opened; its tab shows no mark, only the dimmed reason.
   A rename is not stale (decision 0172), so it never turns the bar yellow.
4. **The bar.** A badge at the right end of the step bar shows the overall health as a shape and words ("Good to go", "Needs attention", "Error to fix"), with a 2 pixel line in its colour along the foot of the bar. Its tooltip lists each step that is not good and why; clicking it goes to the first problem (the first item with a missing value, else the first in error, else the first stale operation, else the first warning): the step, the item in the tree and, for a missing value, the field. Each available step tab shows its own mark, and its tooltip adds the health and reason.
5. **The inspectors.** The operation and setup inspectors show, under the header, "1 value is missing" or "n values are missing" with a "Show first" button, from the moment the operation exists. Under a missing field (or under the Tool, Stock, Part position or Machine section) the same warning shape and the short sentence are drawn, with an amber 2 pixel border on an empty field. Nothing is red on a fresh operation (red is for a refusal or an error). "Show first" and the bar's jump scroll to the field and open its section.
6. **Post and export (issue 140; amends decision 0148 item 1).** The step and File > Post and export are also blocked while a value is missing: "Fill in the missing values first", after the existing reasons (model, setup, machine, tool, operation). Operations still need not be generated.
7. **OI-692.** The playback panel shows the run again when every operation in it is back and on (undo or redo of a turn-off or a deletion), which the export check already allowed. **OI-693.** A tool's name and position are no longer inputs to an operation's cache key, as an operation's are not (decision 0172). The tool's geometry, cutting data and machining numbers still are. A note kept in a cached result that names the tool keeps the old name until the operation is regenerated (OI-621).

## Specification edits (same change)

Section 4: the step indicator gains the health mark and tooltip, the badge, and the Post and export condition (item 6). Section 8: the header of an operation and of a setup gains the count of missing values and "Show first", and a missing field its mark and sentence.

## Alternatives considered

- A red border and a red sentence on every missing field: rejected, a fresh operation would be a wall of red (the owner asked for calm). The amber shape and a sentence are the mark, red is kept for refusals.
- A fixed list of required fields in the interface: rejected, issue 224 asks for the strategy to say, and generation must check the same values.
- Colouring only the bar: rejected, UI-11 asks for shape as well as colour.

## Consequences

- `openchuck-app`: new `requirements.rs`; `overview.rs` gains `StepHealth`, `Missing`, `StepState::health` and `reason`, `Overview::health`, `health_lines`, `missing_for`, `first_problem`, `ItemRef::step`; `operation.rs` shares the missing-value function; `cache.rs` leaves the tool's name and position out of the key.
- `openchuck-ui`: `field_row.rs` (`needs`, `scroll_to`, `needs_line`), `form_panel.rs`, `shell/inspector.rs`, `shell/frame.rs` (step marks, badge), `shell/marks.rs` (`colour_of`), `shell.rs` (`go_to_first_problem`, the restore), `playback_panel.rs` (`restore`).
- No project format change. Tests: `req_ui_11_*` and `req_op_16_req_nfr_07_*` in `crates/openchuck-app/tests/overview.rs`; `req_ui_11_the_step_bar_*`, `req_ui_11_a_setup_without_stock_*` and `req_nfr_12_req_ui_06_undoing_*` in `shell/frame_tests.rs`; the contrast test in `theme.rs`. The layout of the badge against the step names is tested at 1366 by 768 at 100 % and 120 % scale; it has not been looked at on a real screen (OI-623).
