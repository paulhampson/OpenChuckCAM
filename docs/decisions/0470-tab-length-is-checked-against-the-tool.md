# 0470: A tab's length along X, not its diameter, is checked against the tool (OP-21, SET-09, NFR-07; issue #438; lane TABS-CHECK)

- **Status:** accepted by the project owner on 1 Nov 2026, relayed by the coordinator (decision 0049): the owner chose option A of three (below). Amends item 6 of [decision 0035](0035-workholding-and-retaining-tabs.md) and the owner's answer on OI-159.
- **Date:** 1 Nov 2026.
- **Issue:** #438, "Tabs diameter is checked against tool width" (build 0.30.0).
- **Open items:** OI-159 is closed by 0035's answer and stays closed; its register text is corrected to the new rule. New OI-1660 (rename `narrowest_line`). No project schema change; no library version change.

## Context

Decision 0035 item 6 and the owner's answer on OI-159 say a tab narrower than the tool is not permitted. The code read "narrower" as the tab's smallest diameter (`narrowest`), and refused or warned when that was under the tool's diameter. A retaining tab is a cylinder or frustum on the rotary axis (or parallel posts around it) in the head or tail zone, with a length along X (`Tab::length`). The tool works around the tab, so the tab's diameter does not stop the tool. What the tool has to fit into is the gap the tab fills: its length along X. A short tab between the part and the held region leaves a groove narrower than the tool, which the tool cannot cut. The owner reported the check was on the wrong dimension.

## Decision

1. **The rule.** A tab is refused, or warned about, when its **length along X is less than the tool's cutting diameter** (with the existing 1e-9 mm tolerance, so a length equal to the tool is allowed). The tab's diameter, and a frustum's two diameters, no longer take part in this check. This holds for both shapes and both layouts (on the axis, around the axis). The owner chose this (option A); the alternatives were to keep a diameter check for tabs around the axis too (B), and to make the check a warning only (C).
2. **Where it applies, unchanged.** A refusal when generating (`check_for_tool`) and when applying the workholding, and a warning in "Check against the part and tools", the overview and the export notes, as before.
3. **Messages (decision 0114, plain words).** Generation: "tab 2 is 3.000 mm long along X, shorter than the tool 'T2 Flat Ø6' (6.000 mm); make the tab longer or use a smaller tool". Check and notes: "Tab 2 is 3.000 mm long along X, shorter than the tool T2 Flat Ø6 (6.000 mm) of 1 Rough. Make the tab longer or use a smaller tool." The panel's read-only line is renamed "Length vs tools" and reads "3.000 mm, shorter than the tool T2 Flat Ø6 (6.000 mm) of 1 Rough".
4. **Interface specification.** Section 11.7's field table (the read-only line) and its checks table (the OI-159 row) are updated to the above in the same change. `workholding_view::narrowest_line` keeps its name and signature, so the library API does not change (decision 0048 would otherwise call for a minor bump); only its meaning does.
5. **Effect on projects.** A project that passed the old check had every tab at least as wide as its tools. It is refused now only if a tab is also shorter than a tool, in which case the refusal is correct under the new rule. Stored data is unchanged. The golden and test projects were run and none changed (see the pull request).

## Alternatives considered

- **B, diameter check kept for tabs around the axis:** the tool does not need to pass between posts for the cut to be possible, and the owner did not ask for it.
- **C, a warning only:** a tab shorter than the tool leaves a groove the tool cannot cut, and 0035's answer is that this is refused, not left to produce a wrong shape.

## Consequences

- OP-21 is unchanged; its verification rows are met by `req_op_21_a_tab_shorter_than_the_tool_is_refused_whatever_its_diameter` and `req_op_21_a_thin_tab_at_least_as_long_as_the_tool_is_accepted`.
- A tab that is long and thin is now accepted for a tool wider than it. Whether the tool can reach into very thin gaps with its shank is the shank checks' business (decision 0368), not this one.
