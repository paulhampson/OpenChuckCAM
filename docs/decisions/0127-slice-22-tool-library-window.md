# 0127: Slice 22 tool library window, and tools out of the project tree (lane X9)

- **Status:** proposed. Lane X9, slice 22 ([decision 0114](0114-slice-22-conformance-report.md) C19 and section E, accepted; [decision 0105](0105-slice-22-conformance-pass.md), accepted), with the owner's trial feedback F-10 (issue 148). The owner decided items 1 and 4 to 9 (OI-375 to OI-379, OI-491 and OI-492) on 20 Oct 2026, relayed by the coordinator (decision 0049); the owner agreed the UI-06 wording of item 1 when answering issue 148, also relayed.
- **Date:** 20 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-375 to OI-379 and OI-491 to OI-494 are new. OI-375 to OI-379, OI-491 and OI-492 are closed by the owner's answers; OI-493 and OI-494 are open. OI-342 stays open with this lane's board (image 10) re-rendered.

Numbering: the coordinator assigned this lane records 0127 and 0128, and OI-375 to OI-379 with OI-491 to OI-495. This lane uses 0127 and OI-375 to OI-379 and OI-491 to OI-494. Record 0128 and OI-495 are not used.

## Context

Decision 0114 C19 found no tool library window: a project tool was edited in its own inspector, reached from a Tools group in the project tree. The profile editor (TL-03) was built without the selected segment, "+ Line", "+ Arc", move up and down, the arc radius column or profile files. The library (TL-01), cutting data (TL-05) and machine tool sets (TL-11) were met in the library code and the command line, not in the interface. The owner decided the window is built in slice 22 as lane X9 (OI-343).

In the trial build (trial feedback F-10, issue 148) the owner found tools in the project tree unexpected, and the tool library could not be reached at all: the Tools group's "+" had no action. The owner asked for tools to leave the tree, to be associated with operations, and for a tool library dialog with add, edit, delete, duplicate, import and export. UI-06 asked for tools in the tree, so it conflicted.

## Decision

1. **Tools leave the project tree (OI-375, owner-decided).** UI-06 now reads: "Project tree listing the model, stock, setups and operations; operations can be reordered, enabled, disabled and duplicated." (`requirements.md` edited, the wording the owner agreed for issue 148; the priority stays P1.) The tree's Tools group, its rows and its "+" are removed (`shell/tree.rs`); an operation's row keeps its tool number chip. The tool inspector reached from the tree is removed; a message about a tool opens the tool library on the project's copy.

2. **The window is reachable.** Tools, "Tool library…" (with or without a project); "Tool library…" on the Operations step's workflow bar; an operation's Tool section ("Edit tool…", "From the library…", "Save to library…"); and the New operation dialog ("From the library…"). It is a floating window (`floating`, decision 0029 item 17), 1200 by 760 as board 10.

3. **An operation's inspector shows its tool** (the owner's answer to issue 148). The Tool section, between Geometry and the parameter sections: the tool's name and size; the project tool number (TL-06), editable as one undoable step, refused when another project tool holds it; the machine tool set's number under it when the two differ (TL-11); how the project's copy stands against the library (TL-07, item 6); "Update from library", "Edit tool…" and "Save to library…"; a list of the project's tools and "From the library…". The Setup checks (tabs against tool diameters) are unchanged.

4. **Where a project tool is edited (OI-376, owner-decided: B).** The window lists the project's tools first, as "This project", one row per tool with its number chip. Choosing one edits the project's copy: each change is one undoable command (`tool_library::snapshot_command`), keeping its number and place; the library is untouched. Its right column, "In this project", has its number, its state against the library with "Update from library" and "Save to library", the operations that use it, and "Remove from project". Library rows edit the library.

5. **Deleting a library tool (OI-377, owner-decided: A).** Allowed after a confirmation that names what it affects: the project's copy stays and its operations then show "Not in the library (kept in the project)"; the tool is taken out of every machine tool set that lists it, in the same step, so no set names a missing tool; a default tool is not installed again (TL-12, the ledger of decision 0081). Not undoable.

6. **A project copy that differs from its library tool (OI-378, owner-decided: B).** Shown as "Same as the library", "Differs from the library: " with the fields in words (for example "flute length", "Oak stepover"), or "Not in the library (kept in the project)". "Update from library" replaces the copy, keeping the project's tool number, as one undoable step. "Save to library" writes the copy over the library tool after a confirmation naming the fields it changes (or saying it is new to the library); the number stays with the project. Nothing updates by itself (TL-07: the copy always wins).

7. **Library tools are drafts until Save (OI-379, owner-decided: B, not the lane's recommendation).** New tool, Duplicate and edits of a library tool change a draft; Save writes the tool file and its tool set numbers, after checking all of them, so a refusal writes nothing; Cancel drops the changes. Leaving a draft with changes (another tool, New tool, Duplicate, Import library, Close) asks "Discard the changes?" with Discard and Keep editing. New tool starts from the default standard tool of the chosen type (TL-12), named "New <type>", with no cutting data and no material; Duplicate makes a library draft "<name> copy" with a new ID, no tool set numbers and not in the project (a project tool duplicates into the library too). "Add to project" is disabled while a draft has unsaved changes. Project tool edits (item 4) keep one undo step per change.

8. **Import conflicts (OI-491, owner-decided: B).** Import library reads a folder and lists, before writing anything, the new tools, the tools the same as the library's and the tools that differ, with their fields. For each that differs, or for all: Keep mine (chosen at first), Replace, or Keep both (the import added with a new ID and " (imported)" after its name). Nothing is written until the user confirms; tool sets are not imported. `ToolLibrary::import`, used by the command line, keeps its replace rule.

9. **Tools into and out of the project (OI-492, owner-decided: A).** "Add to project" adds the library tool's copy, numbered from the project machine's tool set or the next free number (TL-11), as one undoable command. Opened from an operation, it also gives the operation the tool in the same step ("Use for the operation" for a tool the project holds already); from the New operation dialog, the tool is handed back to the dialog. A tool no operation uses shows "in project" in the list and can be removed with "Remove from project", refused while an operation uses it, naming the operations.

10. **The profile editor** (`tool_panel.rs`), shared by both kinds of tool: the selected segment and its end points highlighted in the preview; "+ Line", "+ Arc" (a convex quarter turn of 1 mm), move up, move down and delete on the selected segment; the table's arc radius column, with the selected arc's centre editable under it; profile file import (into the editor) and export (only a valid profile is written), with the file's checks.

11. **The view model** is `openchuck-app`'s new `tool_library` module: the grouped and searched list, the project rows, the draft edits (`edit_name`, `edit_type`, `edit_field`, `edit_material`, `edit_profile`, `edit_cutting_data`), `new_draft`, `duplicate_draft`, `save_draft`, `delete_tool` with `delete_impact`, the tool set numbers, the import preview and apply, export, and the project commands (`add_to_project`, `update_from_library`, `save_to_library`, `snapshot_command`, `set_operation_tool`, `set_project_tool_number`, `remove_from_project`, `operation_tool`). `tool_edit` gains `insert_arc`, `move_segment` and `arc_radius`. Every library write goes through `ToolLibrary`'s own save and remove; the default-tool ledger is never written, so an edited default stays edited and a deleted one stays deleted (TL-12). No project format change, no engine change, and no change to the library family.

12. **The board first (OI-342).** `ToolLibrary.dc.html` (image 10) gains the "This project" group with number chips, Delete under the list, the draft's Save and Cancel with "Changed: not saved yet" (and "Add to project" disabled meanwhile), the profile editor's Cancel and Apply, the "V-bit, chamfer and drill" group name and the cutting data note. It was rendered again in headless Chromium at 1200 by 760 with the bundled IBM Plex files in place of Google Fonts (Plex Medium is not bundled, so weight 500 uses Regular).

## Alternatives considered

- **Keep tools in the tree, with the window added.** The owner chose to take them out (issue 148).
- **Edit the library only, and update the project from it (OI-376 A),** or edit the copy in the operation inspector (C). The owner chose B: one editor, for both kinds.
- **Library edits saved at once (OI-379 A, the lane's recommendation).** The owner chose drafts with Save.
- **Refuse to delete a library tool the project uses (OI-377 B),** or delete without asking and offer an undo (C). Rejected by the owner.
- **Import replacing silently (OI-491 A)** can lose a user's edits, including an edited default; never replacing (C) is awkward.
- **Tools that follow operations (OI-492 B):** loses a tool's project number when its last operation goes. Rejected by the owner.

## Verification

Tests named after the requirement IDs:
- `crates/openchuck-app/tests/tool_library.rs` (12 tests): new tools of every type are drafts until saved and listed by group; search by name, type and material with the "in project" mark; edits wait for Save and refusals change nothing; type changes keep the sizes or name the problem; Duplicate; cutting data added, checked, renamed and removed; tool set numbers saved, refused when taken, removed; delete keeps the project copy, clears the tool sets and stays deleted after an update; an edited default is not overwritten; profile files with the file's checks; the import preview and each choice; add, update, save and the operation's Tool section, the project copy edited by commands and Remove from project.
- `crates/openchuck-app/tests/tool_edit.rs`: `req_tl_03_profile_editor_adds_arcs_moves_segments_and_gives_arc_radii`.
- `apps/openchuck-ui/src/shell/tool_library_tests.rs` (7 tests, the shell drawn without a window): no Tools group and the operation's Tool section with its number change undone; the window opening and adding a tool as one undoable step; choosing a tool for an operation in one step; a tool handed back to the New operation dialog; library drafts with the discard prompt; a project tool edited by undoable commands; the window without a project. The empty-state test no longer expects a TOOLS group.

## Consequences

- `docs/requirements.md`: UI-06 as in item 1 (agreed by the owner).
- `docs/ui-design/ui-specification.md`: status line; sections 5 (the dialog rule), 6 (no Tools group, the "+" row removed, where tools are), 8 (no tool inspector; the operation's Tool section), 11.4 (New operation's "From the library…"), 12.1 (rewritten to items 2 and 4 to 10), 18 (TL-12 for section 12) and 20 (image 10).
- `docs/verification-plan.md`: the UI-06 and UI-07, TL-01 group and TL-07 rows.
- `docs/open-items.md`: OI-375 to OI-379 and OI-491 to OI-494 added; OI-342 updated.
- `docs/work-breakdown.md`: lane X9's row.
- Code: `openchuck-app` (`tool_library`, `tool_edit`, `tools` re-exports `Millimetres`), `openchuck-ui` (`tool_library_window.rs`, `tool_panel.rs`, `run_panel.rs`, `shell.rs`, `shell/tree.rs`, `shell/inspector.rs`, `shell/frame.rs`).

## Open items

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | OI-493: the window's *(display)* checks: its layout at 1366 by 768 (UI-16), contrast, focus tooltips and AccessKit names, and the smoke test opening it, are lane X8's. | Lane X8 | |
| 2 | OI-494: the window lists only the machine tool sets the library holds (the Makera Z1 default set); a set for another machine is made by the command line or by hand. | A later slice | Owner |
