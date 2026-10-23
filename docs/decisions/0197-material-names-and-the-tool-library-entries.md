# 0197: Stock material names match ignoring case; one list of materials; the tool library's tooltips and boxes (lane BUG-SPINDLE; issues 249 and 250, trial findings F-79 and F-80)

- **Status:** accepted on 21 Oct 2026, by the owner, directly in the lane BUG-SPINDLE session. The owner's answer on matching (decision 3, with the tool library addition) was relayed by the coordinator session on 21 Oct 2026.
- **What it amends:** [decision 0014](0014-slice-4-tool-library.md) item 5 (the stock material name a setup gives is matched exactly to a tool's cutting data). TL-05 and TL-09 are unchanged.
- **Date:** 21 Oct 2026.
- **Open items:** OI-752 (answered, closed), OI-756 (new).

## Context

The failure message of issue 249 ends "(names must match exactly)". A setup that says "softwood" or "Soft wood" against a tool's "Softwood" lost the tool its cutting data and so, until decision 0196, its spindle speed. The owner asked for the match to be forgiving, for the setup's material to be chosen from the materials the tools know, and for the tool library to use the same list so materials are consistent across tools. Issue 250 asked that the tool library's spindle speed box be wide enough and that every entry have a tooltip in plain words.

## Decision

1. **Matching (OI-752; the owner's option A).** A stock material matches a cutting data entry when their names are equal ignoring case and ignoring leading, trailing and repeated spaces (`openchuck_types::tool::material_key`, `find_cutting_data`). An exact name wins. Names are stored as typed. "Soft wood" and "Softwood" are different materials.
2. **One entry per material on a tool.** `validate_entry` refuses a tool with two cutting data entries whose names differ only by case or spacing, naming both. The tool library shows the message at once; a library file that holds both is refused when it is read, with the same message.
3. **The list of materials is derived, not stored.** `materials::known_materials` is every name in the cutting data of the library's tools and the project's tools, plus the materials the project's setups use, one spelling for each material (the first in name order). Adding a material to one tool's cutting data therefore adds it for every tool and setup, and no file format changes: **no schema number and no migration** for the list. Existing libraries need no migration; only a library that already holds two spellings on one tool is refused until one is removed.
4. **Picking.** The setup's Material row has a "Pick a known material…" list under it, and a typed name that matches a known one takes the known spelling (`canonical_material`). The tool library's cutting data editor has the same list beside the Stock material box, and the same rule applies when Apply is pressed, so "softwood" typed on a second tool becomes "Softwood". A new name is accepted and joins the list.
5. **The tool library's boxes and tooltips (issue 250).** The cutting data boxes take the room the pane leaves, never less than 110 points, so a five-digit speed shows in full at 100% and 120%. Every entry of the tool library has a tooltip in plain words that gives its unit and what it changes: the tool's name, type and material, the tool sizes of TL-04 (the descriptions in `tool_edit::tool_form` are rewritten), the five cutting data entries, the stock material, and the tool set numbers. The tooltips open on hover and on keyboard focus (`field_row::tip`, UI specification section 16), and the boxes carry an accessible name. No text says a value is safe to run (NFR-12); the spindle speed tooltip says the user checks it against the tool, the material and the machine. The wording is for the owner to review (OI-756).

## Tests

- `crates/openchuck-types/tests/tools.rs`: `req_tl_05_two_entries_that_differ_only_by_case_are_refused`.
- `crates/openchuck-app/tests/tool_library.rs`: `req_tl_05_a_material_added_to_one_tool_is_known_to_every_tool_and_setup`; `crates/openchuck-app/src/materials.rs` unit tests.
- `crates/openchuck-app/tests/operation.rs`: `req_op_11_req_tl_05_each_material_gives_its_own_feed_and_a_missing_one_is_named` now also asserts that "Steel" and " STEEL " find the data of "steel".
- `apps/openchuck-ui/src/shell/tool_library_tests.rs`: `req_ui_16_req_tl_05_the_spindle_speed_box_shows_a_five_digit_speed_in_full` (100% and 120%, at several pane widths), `req_tl_04_req_tl_05_every_tool_library_entry_has_a_plain_tooltip`.

## Alternatives considered

- **Keep exact matching and improve the message ("did you mean Softwood?").** Leaves the block in place. Not chosen.
- **Store the list of materials in the library.** A file format change and a migration for a value the tools already hold. Not chosen: derive it.
- **Treat "Soft wood" as "Softwood".** Dropping inner spaces would join names that are different words. Not chosen.
