# 0539: Workholding Apply warns on an automatic tab with no room and on tabs that overlap (lane MSGREVIEW; issue #567)

- **Status:** accepted by the project owner on 7 Nov 2026, relayed by the coordinator (decision 0049), as written. Proposed by lane MSGREVIEW on 7 Nov 2026. It records the owner's answer to question 5 of [decision 0536](0536-message-levels-operation-reference-and-setting-links.md) (7 Nov 2026, relayed by the coordinator, option A). It changes what a check does, so it is a record of its own.
- **What it amends, if accepted:** [decision 0523](0523-tab-checks-follow-the-operations-reach.md) item 4, as built. That item names only a tab outside the end zones as refusing at Apply, but the code refuses on two more findings. The requirements and the architecture are unchanged.
- **Date:** 7 Nov 2026.
- **Open items:** OI-2095 (raised with 0536) closes when this is built.

## Context

The workholding section checks the tabs when Apply is pressed. `workholding::check_tab_zones` (`crates/openchuck-types/src/workholding.rs:450`) finds three things:

- a tab outside the head and tail zones (`TabOutsideEndZones`);
- an automatic tab with no room at its end (`AutoTabNoRoom`);
- an automatic tab that overlaps a tab placed by hand (`TabsOverlap`).

`workholding::findings_of` (`crates/openchuck-app/src/workholding.rs:388-435`) turns each into a finding. It marks the first as refusing, as decision 0523 item 5 says. It also marks every other finding as refusing, through its catch-all arm (`other => ... refuses: true`). The workholding panel stores nothing while any finding refuses (`apps/openchuck-ui/src/workholding_panel.rs:1530-1536`). So a harmless overlap blocks Apply. The overlap's own text says "the part is held by both, so the G-code is not wrong".

Generation, the overview and the setup checks treat both findings as warnings. Decision 0523 item 4 made Apply store and warn on a short tab, and named only the end-zone check as refusing. The catch-all looks like an oversight, not a choice.

## Decision (proposed)

1. **Apply stores the workholding and keeps both findings on their rows as warnings,** with the amber Warning mark, as decision 0523 does for a short tab: an automatic tab with no room, and tabs that overlap.
2. **A tab outside the end zones stays a refusal** (decision 0523 item 5): Apply stores nothing, and the row has the Error mark.
3. **The findings are listed by kind, not by a catch-all.** `findings_of` names each kind it refuses, so a kind added later warns unless a decision makes it refuse.
4. **The command line agrees.** It refuses a workholding with the same findings as Apply (`apps/openchuck-cli/src/main.rs:1089-1102`), so it changes with it.
5. **The level of each finding** follows decision 0536:
   - An automatic tab with no room is a warning: the tab is not placed, so the part may be held by less than the user expects.
   - Tabs that overlap stay a warning, as the owner chose (question 5).

   Neither is a possible collision, so decision 0538 asks no tick for them.
6. **The words** follow decision 0536 section 3, for example "Tabs overlap (setup 'Setup 1'): tab 2, placed automatically, overlaps tab 1, placed by hand, from X 4.000 to 6.000 mm. The part is held by both. Remove one if you do not need both." Nothing says the program is safe (NFR-12).

## Test plan

Each test is named after its requirement, written first and watched fail.

- `req_ui_25_apply_stores_with_an_automatic_tab_that_has_no_room`: Apply stores the workholding, and the finding is on its row with the Warning mark.
- `req_ui_25_apply_stores_with_tabs_that_overlap`: the same, for overlapping tabs.
- `req_ui_25_apply_refuses_a_tab_outside_the_end_zones`: unchanged; nothing is stored, and the row has the Error mark.
- `req_ui_25_a_new_zone_finding_warns_unless_named`: `findings_of` refuses only the kinds it names.
- The command line's workholding check gives the same result as Apply for the three cases.
- The generated G-code is unchanged for a project whose workholding Apply now stores (GC-07).

UI-25 is the requirement for adding and editing a setup's held regions and retaining tabs, with validation; SET-09 defines the workholding.

## Alternatives considered

- **Keep refusing at Apply, and make generation refuse too** (question 5, option B), so the two agree. Not chosen: it would block a project that is not wrong.
- **Leave it as it is** (option C). Not chosen.

## Consequences

- The fix is a few lines in `findings_of`, with tests, in a pull request of its own.
- No change to the project file, the cache key or the G-code.
