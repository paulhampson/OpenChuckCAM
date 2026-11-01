# 0620: An operation whose own steps are done in a run that has not ended shows "Finalising", not stale (lane STATUSICON; issue #474)

- **Status:** proposed. The owner chose the state's name and icon (option 1) and agreed the UI-11 amendment on 1 Nov 2026, relayed by the coordinator (decision 0049); the record as a whole is not yet accepted.
- **Date:** 1 Nov 2026.
- **Lane:** STATUSICON, `claude/lane-statusicon`.
- **Issue:** #474.
- **Requirements:** UI-11 (amended below), UI-16, NFR-12, NFR-15.
- **Amends:** UI-11 in `docs/requirements.md` (adds "finalising" to the state list, agreed by the owner) and the status-marks table of `docs/ui-design/ui-specification.md` section 6 (one row). [Decision 0238](0238-simulate-display-and-operation-navigation.md) (the "Simulation out of date" banner) is not changed.
- **Numbers:** takes 0620 and OI-1960. No project schema number. No library-family change, so no version bump (only `openchuck-app` and `openchuck-ui` change).

## Context

An operation showed Running while its toolpath was made, then Stale until the whole run ended. `GenerationRecord::generated` is written only by `succeeded`, once, at the end of a run (`lib.rs`), and an operation is up to date only when that record matches its current key (`overview.rs`).

After one operation's own toolpath the run still does: the link to the previous group, the facing check, the notes, the stock simulation; and after the last operation the comparison of each setup's stock with the part, the check that no model file changed during the run (which refuses the whole run), and the commit.

## Decision

1. **Up to date only at the commit.** A result the run can still throw away (cancel, a later failure, the model-file-changed refusal) is not shown as up to date (NFR-12). Showing green at once and flipping back would be untrue and would flicker.
2. **New state Finalising** (`ItemState::Finalising`): the operation's path, link, checks and stock are done in a run that has not ended. Mark: a blue circle with its left half filled (colour role accent, as Running; the shape differs from the gapped ring and from the filled green circle, so it is told apart without colour, UI-11, NFR-15). Name "Finalising". Tooltip and inspector cause: "Finalising: toolpath made; the checks and final steps are still running, so it does not count as up to date yet".
3. **Order.** Per operation a run moves forward only: Stale, Running (when it is made or its stock simulated; a cached operation skips this), Finalising, then Up to date for every operation at the commit. Cancel, failure or refusal returns the finalising operations to Stale with the cause "the run was cancelled before it finished" or "the run stopped before it finished (<message>)".
4. **Not counted as stale.** A finalising operation is not in the stale list, so no "Generate stale" is offered for it during the run; an edit during the run changes its key and makes it Stale.
5. **Unchanged:** the "Simulation out of date" banner and its meaning, the stale test (chained keys, decision 0520's part-changed rule is read when VIEW-OVERLAYS is on `main`), and the Messages panel (the failure message stays; no new row).
6. The legend has seven entries; the wrap and width tests run over all states.

## Open items

- OI-1960: optionally commit each operation's record as it finishes, so it is up to date before the run ends. This needs a rollback for the end-of-run refusal and a way to show it.
- Decision 0520 (VIEW-OVERLAYS, #472) was not on `main` when this was written; it only reads `ItemState`, and a new variant may need a match arm in its code after it merges.

## Verification

Tests, named by requirement: `req_ui_11_req_nfr_12_a_made_operation_shows_finalising_until_the_run_ends` and `req_ui_11_req_nfr_12_every_operation_becomes_up_to_date_when_the_run_ends` (a real run, state sequence monotonic, cancel returns to Stale with its cause), the three record tests in `overview.rs` (commit, failure and cancel, edit during the run) and `req_ui_11_req_nfr_15_every_state_has_its_own_shape` in `marks.rs`. The existing legend tests run over `ItemState::ALL`. Generated G-code remains the user's to verify before running on a machine (NFR-12).
