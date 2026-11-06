# 0523: A tab is checked against an operation's tool only where the operation's path can touch it, and a tab shorter than the tool is warned of, not refused; the audit of checks that ignore an operation's region (OP-21, NFR-07, NFR-12; issue #548; lane FLUTE-TABS)

- **Status:** accepted by the project owner on 6 Nov 2026 with the change that the tab-length rule also warns, relayed by the coordinator (decision 0049). Proposed by lane FLUTE-TABS on 6 Nov 2026; the owner read it in full and answered "Accept, but make the tab-length rule a warning too", and then, asked about the setup panel's Apply, chose that Apply also warns and stores the tab (answer B, 6 Nov 2026, relayed).
- **Date:** 6 Nov 2026.
- **Number:** 0523, with OI-2045 to OI-2049, confirmed free by the Merge Manager on 6 Nov 2026.
- **Issue:** [#548](https://github.com/paulhampson/OpenChuckCAM/issues/548) (sev1). Related: [#547](https://github.com/paulhampson/OpenChuckCAM/issues/547) (decision 0522), [#438](https://github.com/paulhampson/OpenChuckCAM/issues/438) (decision 0494).
- **Amends:** [0494](0494-tab-length-is-checked-against-the-tool.md) items 1 and 2: the rule (a tab shorter along X than the tool is wide) is a warning, not a refusal, and it applies to an operation limited by selections only where its path can touch the tab. The end-zone rule of [0035](0035-workholding-and-retaining-tabs.md) (OI-180) stays a refusal, with the same narrowed scope.
- **Open items:** OI-2045 to OI-2049 new: the audit's follow-ups, below. No project schema change; no library change (the application only).

## Context

The owner's carving project (private; numbers only) could not be generated. 'Axial Finishing' stopped with "invalid workholding: tab 1 is 5.000 mm long along X, shorter than the tool 'Z1 V-bit 20 deg tip 0.3 shank 6' (6.000 mm)".

- **The operation never reaches the tab.** Its selection set holds the part's side only, so the tool stays near the side's radius. The tab is 10 mm across on the axis, at X -5 to 0 between the 5 mm grip and the part.
- **Why it was refused anyway.** Decision 0494's rule (a tab shorter along X than the tool is wide is refused) was checked before generation for every operation on the setup, whatever it cuts.
- **A second check in the same place.** The zone check (a tab not wholly in the head or the tail zone) also refused every operation, though it is already a setup warning.

The owner asked that the check apply only where the operation can touch the tab, and for an audit of every check for the same fault.

The fault was reproduced on a public part: a cone in round stock with a 4 mm tab, 6 mm across, between a grip and the part, and a 6 mm ball. Axial finishing limited to the cone's side was refused.

## Decision

1. **Whole-part operations: every tab.** An operation with no selections (no machine, boundary, avoid or check set) is checked against every tab before it is generated, as 0494 says. It may cut anywhere the part and the tabs are, including down beside a tab.
2. **Limited operations: checked on their path.** An operation limited by selections is generated first. It is then checked only against the tabs its path can touch. That covers both rules of the gate: the tab's length against the tool (0494) and a tab outside the end zones (0035, OI-180). A tab it cannot touch says nothing.
3. **What "can touch" means (`workholding::path_reaches_tab`).** Each fed move of the path is read from the pose before it: cuts, plunges, leads and fed links. Rapids are not cuts.
   - **Why the test is sound.** Every tool profile rises from its tip, so the tool's body is at or above its tip along the tool axis, and within its largest radius (cutting profile or shank) of that axis, which is square to the rotary axis. The tab lies within its farthest radius of the rotary axis: half its largest diameter on the axis, or its offset plus that around the axis.
   - **The test.** A move can touch the tab only where its X range comes within the tool's largest radius of the tab, and its lower end is no higher above the rotary axis, along the tool axis, than the tab's farthest radius plus 0.01 mm of rounding. The test reads the move's X range and its lower end together, so it may count a move that does not touch. It never misses one that does.
   - **Edge cases.** A pose that is not a finite number counts as touching (NFR-07). A tab layout the check does not know is taken as reaching any height.
4. **A tab shorter than the tool is warned of, not refused (the owner's choice, 6 Nov 2026).** This amends 0494 item 1; 0494's option C, declined then, is taken now, as the flute check was made a warning (decision 0522). The path is kept, with one warning per tab (a `PathWarning`, also a note, needing no acknowledgement; decision 0175). It starts "Tab length: " (`workholding::TAB_LENGTH_LEAD`) and gives:
   - the tab's number, its X range and its length along X;
   - the tool's width, and that the tool may be unable to cut the gap beside the tab to shape;
   - that the path was made and must be checked before it is run;
   - the options: a smaller tool, or a longer tab.

   The operation and its tool are named where the warning is shown, as for the flute warning of decision 0522: "Tab length (operation '<name>', tool '<tool name>'): ..." in Messages, Results, the operation's status and the export checks, named when shown so a rename is followed. Nothing in it says the program is safe (NFR-12). It applies to every tab for a whole-part operation and to the tabs its path can touch for a limited one, so it is not noise.

   **Apply in the setup panel warns too (the owner's answer B).** 0494 item 2 refused a short tab when the workholding was applied, storing nothing. Now Apply stores the workholding and keeps the finding on the tab's row with the Warning mark, in amber (`WorkholdingFinding::refuses` false). A finding that refuses, such as a tab outside the end zones, still has the Error mark and stores nothing. Interface specification 11.7 says so.
5. **A tab outside the end zones stays a refusal.** It is a separate rule (0035, OI-180): the target is built from the tabs, and a tab across the part would leave the part uncut there. It refuses only operations whose path can touch the tab (item 2); it is also a setup warning. The lane's recommendation is to keep it a refusal.
6. **The cost.** A limited operation is checked after it is generated, not before, so a refusal for a tab outside the end zones costs a generation, as a refusal from any strategy does.

## The audit (issue #548 item 2)

Every check, refusal or warning that might read the whole part, the whole stock or every tab instead of the operation's region. "Region" means the operation's machine, boundary, avoid and check sets, the held regions and tabs, and its tool.

| # | Check (code) | Decision | Reads now | Kind | Ignores the region? | Risk | Proposed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Tab outside the end zones, in the generation gate (`workholding::check_for_tool`) | 0035, OI-180 | Every tab of the setup | Refusal | Yes | Blocks every operation for a setup fault that is already a setup warning | **Fixed here** (item 2): a limited operation counts only the tabs it can touch |
| 2 | Tab shorter than the tool (`check_for_tool`) | 0494 | Every tab | Refusal | Yes | Blocks generation (#548) | **Fixed here** (items 1 to 4): narrowed to the tabs the path can touch, and a warning, not a refusal (the owner's choice) |
| 3 | The same rule's notes in "Check against the part and tools", the overview and the export notes (`narrow_tab_notes`, `findings_of`) | 0494 | Every operation's tool against every tab | Warning | Yes | Noise; disagrees with generation now | OI-2045: give the setup check the same scope; it needs the path, or a pre-generation bound of the region |
| 4 | No room to cut beside a tab (`uncleared_tab_notes`) | 0503 | Each tab zone, for whole-part operations | Note | Partly: it is skipped only for machine selections (`operation.selections`), not for boundary, avoid or check sets | Noise | OI-2046: gate it on all of an operation's sets, and read the zone against the region (with OI-2011) |
| 5 | Free-end facing and its flute refusal (`finishing::free_ends`) | 0344 item 7, 0509 item 4 | The part's free ends over the whole part; the stock's depth from its farthest material anywhere for stock earlier operations left | Refusal, and it adds cuts | Yes: a selection-limited helical, axial or rest operation still faces both ends of the part, cutting rings outside its region, and is refused for an end it was not asked to cut | Blocks generation (the owner's #547 project, Rest Machining); changes the path outside the region | OI-2047: face only the free ends within the operation's region (or none when it has one); the refusal itself waits on the owner's choice (A, B or C, put on 6 Nov 2026; option B built on `claude/flute-tabs-pr1b-free-ends`) |
| 6 | The free end's cut-free and held-by-nothing warnings | 0344 item 8, 0500 | The rings of row 5 | Warning | Yes, through row 5 | Noise | With OI-2047 |
| 7 | The flute check (`flutes::check_path`) | 0509, 0522 | The operation's own path (region respected); the stock's bound from its farthest material anywhere | Warning (0522) | Partly: the path is local, the bound is global | Noise only now | OI-2025, #540; PR3 of this lane narrows the bound to the stock within the tool's reach |
| 8 | End facing's depth within the flutes (`facing/end/routing.rs`) | 0504, 0252 | Its own coverage at the target X; the setup's original stock, not what earlier operations left | Refusal | No, for the region; the stock is the whole original stock on purpose (0504) | Conservative after roughing | OI-2048: read the stock earlier operations left (low priority) |
| 9 | Selection reach notes (`selection_reach_notes`) | SET-09, OP-21 | The operation's machine triangle sets against the held and tab solids | Note | No; but it is skipped for STEP face sets | A missing note, not a false refusal | OI-2049: cover STEP face sets |
| 10 | Stepdown past the flutes: roughing and Finishing layers | 0509, 0345 | The parameter and the tool | Refusal | No geometry to narrow | None | None |
| 11 | Keep-out of the held regions (`keep_out_of`, `check_clear`) | 0161 | The operation's own moves | Path limit; refusal of a move into it | No | None | None |
| 12 | Clearance and tool length (`effective_clearance`, `check_setup`) | 0495, 0318 | The setup and each tool | Setup warning | No (setup-wide by nature) | None | None |
| 13 | Swarf's set checks (`watched_warnings`, `swarf_compatibility`) | 0176, 0111 | The operation's own sets | Warnings, refusals | No | None | None |
| 14 | Unreached areas, simultaneous finishing (`hidden.rs`, `in_scope`) | 0368 | Filtered by the operation's sets | Warning | No | None | None |
| 15 | Unreached walls, rest machining | 0505 | The whole part; rest machining does not take a region (selections are refused) | Warning | Not applicable | None | None |
| 16 | Workholding solid errors (`labelled_solids`) | SET-09 | The whole workholding | Refusal | Formally, but these are bad setup data the target needs | None | None |

## Consequences

- The owner's #548 project (numbers only): with this change and decision 0522's, 'Axial Finishing' is made; its path does not touch the tab. The next operation, 'Rest Machining', is then refused by the free-end check (row 5; OI-2047 and the owner's choice A, B or C).
- A tab shorter than the tool is warned of and the path is made, for a whole-part operation and for a limited one whose path touches it; it was refused before.
- In the setup panel, Apply stores a tab shorter than a tool with the warning on its row; it stored nothing before. Apply still stores nothing while a tab is outside the end zones.
- A limited operation over a tab outside the end zones is refused after it is generated.
- The setup's own notes (row 3) still name every tool against every tab until OI-2045, so the setup check may say a tab is shorter than a tool whose limited operation generates.

## Verification

Unit tests (`crates/openchuck-app/src/workholding.rs`):

- `req_op_21_a_path_reaches_a_tab_only_within_the_tool_and_the_tab`, on a tab 6 mm across (radius 3) at X 80 to 84 with a tool of radius 3:
  - a cut over the tab 10 mm from the axis does not touch it;
  - a plunge to the tab's top within the tool's radius along X touches it (X 77.1 and 86.9, height 3);
  - past the tool's radius along X it does not, even at the axis (X 76.9 and 87.1);
  - over the tab but 0.1 mm above it, it does not;
  - a low fed move across the tab touches it, though neither end is within reach;
  - a rapid is not read, and a fed move after a rapid is read from where the rapid left the tool;
  - a tab around the axis reaches its offset plus its radius (6.9 touches, 7.1 does not).
- `req_op_21_a_tab_the_operation_cannot_touch_is_not_warned_of`: `short_tab_warnings_where` and `check_for_tool_where` leave out a tab their filter does not keep, and warn or refuse when they keep it.
- The tests of 0494 that expected a refusal are warning tests with the same numbers: `req_op_21_a_tab_shorter_than_the_tool_is_warned_of_whatever_its_diameter` (was `..._is_refused_...`) and `req_op_21_an_automatic_tab_that_shrank_is_warned_of_at_its_resolved_length` (was `..._is_refused_...`); the others pass unchanged. The end-zone tests keep their refusals.

Application tests (`crates/openchuck-app/tests/tab_scope.rs`), on a cone, radius 10 to 5 and X 0 to 30, in round stock from X -12 to 42 gripped 4 mm at its start. The tab is on the axis from X -4 to 0, 6 mm across, and the tool is a 6 mm ball:

- `req_op_21_req_nfr_12_a_whole_part_operation_with_a_tab_shorter_than_the_tool_is_warned_of`: made, with one warning naming tab 1, X -4.000 to 0.000 mm, 4.000 mm long, the tool's 6.000 mm, that it may be unable to cut the gap to shape, to check the path before running it, and the options; never "safe". Messages show it as "Tab length (operation 'Axial Finishing', tool '6 mm ball'): tab 1 (X ...". It was refused before.
- `req_op_21_req_nfr_12_an_operation_on_the_side_above_a_short_tab_is_made`: the owner's case, axial finishing limited to the whole side; made with no tab warning. It failed on the code before.
- `req_op_21_an_operation_far_from_a_short_tab_is_made`: limited to the side from X 10 to 25; made with no tab warning. It failed on the code before.
- In the setup panel (`apps/openchuck-ui/src/workholding_panel.rs`), the check-findings test applies a tab 1 mm long beside a 4 mm tool: the workholding is stored with the tab, the finding stays on tab 1's row as a warning (not refused, not an error), and it names the operation. It failed on the code before, which stored nothing. The end-zone finding's refusal on Apply is unchanged (the same test applies a tab outside the zones first and nothing is stored).
- In `tests/workholding.rs`, `req_op_21_a_tab_outside_the_end_zones_is_refused_and_one_shorter_than_the_tool_warned_of` (was `..._or_narrower_than_the_tool_is_refused`): the end-zone refusal is unchanged; the 9 mm tool against an 8 mm tab is made with the warning.
