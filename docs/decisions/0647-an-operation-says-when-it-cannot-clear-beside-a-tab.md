# 0647: An operation says when its tool cannot cut down beside a tab (OP-21, SET-09, NFR-12; issue #511; lane TABREGRESS)

- **Status:** proposed (lane TABREGRESS, 2 Nov 2026). The owner chose this approach, relayed by the coordinator on 2 Nov 2026: "(b): roughing does not clear round the tab... go ahead with your proposal". Only the owner accepts this record (decision 0049).
- **Date:** 2 Nov 2026.
- **Issue:** [#511](https://github.com/paulhampson/OpenChuckCAM/issues/511), "Regression: roughing does not include the retaining tabs (blisk)".
- **Open items:** new OI-2011 (show the same finding in the setup checks before generating). No project schema change; no library API change, so no library version bump (decision 0048).

## Context

The owner reported that roughing on a blisk project "no longer includes the retaining tabs". The project (private; numbers only here) uses the same model as the public `testdata/step/valid/blisk.step`. The stock runs from X -8 to 56 and is held for its first 8 mm. An automatic Ø10 tab fills the head zone, X 0 to 4. The part starts at X 4 and is 20 mm in radius. Rotary roughing runs along the length with a 3.175 mm flat tool and leaves 0.5 mm of stock.

What was found (lane TABREGRESS):

1. **No tab is cut.** On `main` (`92b9ba4`, the same code as `71eb139b`), the command-line `regenerate` of the owner's project shows that no fed move of any of its four operations puts the tool's tip inside the tab's cylinder (radius under 5 mm over X 0 to 4). Roughing never goes below 20.38 mm from the axis over the head zone. Rest machining clears down to 5.000 mm there, the tab's surface. `simulate` finds the tab whole. Its only overcut is 0.105 mm at the blisk's disk face (X 14, 22 samples), which has nothing to do with the tab.
2. **Not a regression in the program.** A public STL copy of the setup gives byte-identical roughing G-code on `main` of 31 Oct (`4d7e337`) and of 2 Nov (`92b9ba4`).
3. **Why the tab zone stays a block.** Decision 0161 keeps every cutting move's tool off a held region by the tool's largest radius plus `HELD_CLEARANCE` (0.5 mm). The tool's cutting radius plus the stock to leave keep it off the part's end. That leaves 4 − 0.5 − 0.5 = 3.0 mm of room in the 4 mm zone, less than the 3.175 mm tool. Everywhere in the zone the tool lands on the disk's face at radius 20 and never cuts down to the tab. The owner saw the stock round the tab left whole: answer (b) of the coordinator's question, "roughing does not clear round the tab". With no stock to leave there is 3.5 mm of room, and roughing cuts down to the tab's surface (checked: lowest 5.000 mm, none below).
4. **Why nothing said so.** Decision 0494 checks a tab's length along X (4 mm) against the tool's diameter (3.175 mm), and that check passes. It does not count the held clearance or the stock to leave. Nothing else reports a zone the tool cannot get into.
5. **Why the tests did not cover it.** The tabbed tests and the `cone_held_tabs` reference job use an analytic cone with roomy head and tail zones (8 mm and 10 mm), and the reference job is helical finishing only. No test used a tab zone the tool barely fits, a blisk, or checked that roughing clears down beside a tab.

## Decision

1. **The rule.** A tab zone is the part of the stock between a held region and the part's end (decision 0035). For a zone with a held region and at least one tab, the room an operation's tool has is the zone's length less `HELD_CLEARANCE`, less the operation's stock to leave, and less what the tool's largest radius (profile or shank) has over its cutting radius. When the room is less than the tool's cutting diameter (1e-9 mm tolerance), generation adds a note for that operation. The operation still generates; its path is unchanged. A zone without a held region is not reported, because there the tool can pass off the end of the stock.
2. **Which operations.** Operations with no selections whose strategy clears the whole part: rotary roughing, rest machining, and helical, axial, indexed and simultaneous finishing. The note uses the stock to leave the operation actually cuts to, including roughing's raised amount (decision 0058). Selection-limited operations, drilling, swarf, edge following, pencil finishing and Facing (decision 0252 has its own end-zone rules) are left out.
3. **Message (decision 0114, plain words; NFR-12).** "1 Roughing cannot cut down beside tab 1: between the held region and the part (X 0.000 to 4.000 mm) the tool Flat 3.175 (3.175 mm) has 3.000 mm of room, once the 0.500 mm it keeps from the held region and the 0.500 mm stock to leave are taken off. It leaves the stock round the tab above it there. Use a smaller tool or less stock to leave, shorten the held region or move the part away from it, or clear it with a later operation. Check the program and the simulation before running it." A tool with a wider shank or body adds "and the 0.500 mm its shank or body is wider than its cut". Several tabs in one zone are named together ("tabs 1 and 2").
4. **Severity.** No tab material is cut, so the report is not a safety fault (decision 0344's hazard is a part that comes loose). The lane proposes sev3: the program is correct, the result is surprising, and nothing said why.

## Alternatives considered

- **Refuse the operation.** The path is correct and the stock round the tab can be cleared by a later, smaller tool. The owner asked for a warning that still generates.
- **Let roughing reach closer to the held region or the part.** Decision 0161's clearance protects the jaws. Stock to leave is the user's setting. Either change would move the cut nearer to something it should not touch.
- **Extend the 0494 check instead (setup checks).** That check runs without an operation's parameters and reads the tool only, so it cannot know the stock to leave. Showing this finding in "Check against the part and tools" before generating is OI-2011.

## Consequences

- `openchuck-app`: `workholding::uncleared_tab_notes` (crate-private) and the note in operation generation; `operation::keep_out_of` and the note share `tool_reach`. The library API does not change.
- Tests: unit tests in `workholding.rs` (`req_op_21_req_set_09_a_tool_with_no_room_beside_a_tab_is_reported`, `..._a_tool_that_fits_beside_a_tab_is_not_reported`, `..._a_wider_shank_takes_room_beside_a_tab`, `..._only_a_held_zone_with_a_tab_is_reported`). The new integration test `crates/openchuck-app/tests/tab_zone_clearance.rs` runs on the public blisk STL with the report's setup in numbers. `req_op_21_req_set_09_roughing_says_when_it_cannot_cut_down_beside_a_tab` checks the note and that no roughing move enters the tab. `req_op_21_req_set_09_roughing_and_rest_clear_round_a_tab_and_keep_it` checks that, with room, roughing and rest machining never enter the tab, roughing cuts down to the tab's surface, and the simulated stock keeps the tab and loses the stock round it. A mutation check confirmed that it fails when the tabs are dropped from the target.
- The verification plan's OP-21 row names the blisk test.
- No golden file changes: the note does not change any program, and the golden jobs' tab zones have room.

## Verification

- Owner's project, command-line `regenerate` and `simulate` on `main` before this change: the numbers in Context items 1 to 3. The program files are not committed (private project).
- Public STL copy of the setup, with this change: the roughing G-code is byte-identical to before, and the note appears with "3.000 mm of room". With no stock to leave there is no note, and roughing reaches 5.000 mm over the tab and never less.
- `cargo nextest run -p openchuck-app --test tab_zone_clearance`: 6 s and 41 s uninstrumented on this container, under the one-minute rule.
