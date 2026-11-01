# 0630: The Results window becomes a tab of the dock, with a count and level filters (UI-12, UI-16, NFR-12; issue #475; lane RESULTS-DOCK)

- **Status:** proposed. The owner's answers of 1 Nov 2026, relayed by the coordinator, are recorded below; only the owner accepts the record (decision 0049).
- **Date:** 1 Nov 2026.
- **Issue:** #475.
- **Requirements:** UI-12, UI-16, UI-14, NFR-12.
- **Amends:** [0430](0430-results-feed-limit-summary-and-copyable-results.md) item 6 (the Results lines and the Copy all button are in a tab of the dock, not a window); [0125](0125-slice-22-simulate-and-import.md) (the floating Results window and "Project > Results…"); the interface specification, section 1 (layout table) and section 9 (Dock).
- **Open items:** OI-373 (noted: the window it names is the tab now); new OI-1980 (filter by operation). No project schema change; no library change; no user setting.

## Context

The Results window was a floating window opened from Project > Results… and after a failed run. The owner asked for it to live in the lower dock beside Messages and Jobs.

## Decision (the owner's answers, 1 Nov 2026, relayed by the coordinator)

1. **A tab named "Results", after Jobs, on every step** while a project is open: Messages, Jobs, Results; Setup adds "Setup checks" first; Simulate adds "Playback" last. Clicking it on the collapsed dock's rail opens the dock on it.
2. **No menu entry.** "Results…" is removed from the Project menu and none is added to the View menu (the other dock tabs have none). There is no status-bar link: that link is the running job and opens Jobs.
3. **A failed run selects the Results tab and opens the dock** (the run's error is its first line). A cancelled run does not.
4. **The tab label shows a count: "Results (n)"**, n being the warning and error lines of the latest run, one more for a failed run, a grouped line once, information lines not counted. The filters do not change it. A clean run, or no run, shows "Results".
5. **Level filters as in Messages:** chips Errors, Warnings and Information with their unfiltered counts, all on by default, kept for the session only (not in the settings or the project). They filter the lines shown. **Copy all and the plain-text box always hold every line** (NFR-12; the chips' tooltips say so). When the filters hide every line the tab says "No lines at this level.".
6. **Content in a short dock (UI-16).** The summary line, Copy all and the chips are in one wrapping row; everything else (the error, the changed-project line, the lines with their groups, "Results as plain text" and the verify-G-code notice) is in one scroll area that fills the dock, so the dock's 120 pt minimum reaches all of it. Selection, the plain-text box and its keyboard use are unchanged.
7. **The window is gone.** The run panel no longer counts as a window, so the single-key shortcuts work while the tab is shown (the text boxes keep their own focus rule).
8. **No filter by operation** (a result line carries no operation id): OI-1980.
9. **NFR-12.** Nothing here says the program is safe or checked; the notice stays at the end of the tab.

## Verification

Tests `req_ui_12_*` and `req_ui_16_*` in `apps/openchuck-ui/src/run_panel.rs` and `apps/openchuck-ui/src/shell/results_tab_tests.rs`: the menus have no Results item; the tab order on each step; no tab without a project; the count; the rail click and the failed run open the dock on the tab; the chips and the unfiltered Copy all; the tab in a 380 by 90 pt area with Copy all on screen and the notice reached by scrolling.
