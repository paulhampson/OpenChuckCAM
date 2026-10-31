# 0380: Managing machine profiles in the application (lane MACHINES)

- **Status:** proposed, 31 Oct 2026, by lane MACHINES, for the owner's acceptance. The plan was approved by the coordinator session on 31 Oct 2026. The owner answered the five choices (OI-1470 to OI-1474) on 31 Oct 2026, relayed by the coordinator (decision 0049), each as recommended; items 7 to 11 record them.
- **Date:** 31 Oct 2026.
- **Branch:** `claude/lane-machines`. Issue [#335](https://github.com/paulhampson/OpenChuckCAM/issues/335) ("GUI has no way to manage machine library").
- **Requirements:** SET-10 (new, item 7), SET-01, UI-13, TL-11, NFR-07, NFR-12, GC-07 and NFR-06. SET-02, SET-08 and COL-04 (P2) are not built.
- **What it amends:** `docs/requirements.md` gains SET-10 (item 7), with its row in the verification plan and in the work breakdown's traceability table (slice 22).
- **Records it relies on:** [0011](0011-slice-3-machine-stock-setup.md) item 7 (profiles are files; the project keeps a full copy), [0029](0029-slice-8-viewport-and-shell.md) items 15 and 16 (library folder; IDs and the comparison, OI-53 and OI-54 closed), [0127](0127-slice-22-tool-library-window.md) and [0199](0199-tool-library-sections-filter-and-default-tools.md) (the tool library window this follows), [0222](0222-short-moves-minimum-block-time.md) (GC-17 fields), and [0317](0317-z1-measured-axis-height-and-safe-z.md) and [0318](0318-z1-spindle-height-warning-switched-on.md) (the Z1's measured and derived values).
- **Schema and versions:** no machine profile schema and no project schema change (the profile file already holds every value, its ID and its provenance). The library family is not touched, so its version does not change.

## Context

Machine profiles are TOML files: built in (the Makera Z1, in `crates/openchuck-project/machines/`) or in the user's library folder (`OpenChuckCAM/machines` in the platform configuration folder). A project keeps a full copy in `machine.toml`. The application can choose a profile, show its values with UI-13 provenance, compare the project's copy with its source and update it. It can edit only the short-move settings and the spindle-height values, and only on the project's copy. Creating, editing, duplicating, importing, exporting or deleting a library profile means editing files by hand. The owner asked for this on 24 Oct 2026 (#335) and made it a priority on 31 Oct 2026.

## Decision

1. **One form for every profile value (`openchuck_app::machine_edit`).** `machine_form` lists each value of a `MachineProfile` as a `MachineField`. Each field has:
   - its label, description and unit;
   - its group: Profile, Rotary axis, Travel, Maximum feeds, Clearance, Tools and spindle, Short moves, Spindle height;
   - its provenance entry;
   - whether it may be blank;
   - whether it applies now (the A limits of a continuous axis do not).

   The library window and the project copy's editor both use this form.
2. **Setting a value (`set_machine_field`).**
   - Each value is checked on its own first. Then the rules between values are checked: the lowest A below the highest, travel minimum not above maximum, safe Z not below the safe cylinder, the lowest spindle speed below the highest. Last comes `MachineProfile::validate`.
   - A refusal is a `FieldProblem` in plain words that names the field by its label, for example "Z feed: Must be more than 0 mm/min; 0 mm/min was entered." A rule between two values is reported on the field being edited, and its message names both values.
   - Nothing panics on any input (NFR-07; a property test sets every field to random values).
   - A rotary axis about Y is refused with "not supported yet (SET-02)".
   - Choosing a limited A range starts at −360 to 360 degrees, for the user to set. Like every edit, it is marked not confirmed.
3. **Provenance of an edit (UI-13).**
   - A value that changes is marked "not confirmed on the machine". Its source becomes "Changed by the user; check it on the machine", with no date, so files stay the same for the same edits.
   - Setting a value to what it already is keeps its provenance.
   - Other values keep theirs.
   - Built-in profiles are never written. The Makera Z1's values keep exactly the provenance decisions 0317 and 0318 gave them: axis height −48.6 confirmed; safe Z and safe cylinder 40, spindle to axis 73.1 (derived) and recommended minimum safe Z 30, all not confirmed. A copy keeps these sources until a value is edited.
4. **Library drafts (`openchuck_app::machine_library`, after decision 0127).**
   - A library profile is edited as a `MachineDraft`. Nothing is written until Save. The status reads "New: not saved yet", "Changed: not saved yet" or "Saved in the library", and a draft with changes asks before it is left.
   - Save writes to the profile's own file. On a first save it writes to a new file named from the profile name (lower-case letters, digits and hyphens, then `-2`, `-3` if taken), and creates the library folder if it is missing.
   - A profile saved from the editor always has an ID, so project copies of it can be compared with it (OI-54).
   - A name that a library or built-in profile already has (ignoring letter case) is refused, because the CLI chooses a profile by name and machine tool sets are tied to a machine by name (OI-770).
5. **Delete, export and import.**
   - **Delete:**
     - Removes one readable profile file directly in the library folder, after a question that says what it affects: projects keep their own copy, and a project whose copy came from it will then say its source is not in the library.
     - A built-in profile cannot be deleted.
     - A file that cannot be read as a profile is not deleted.
   - **Export:** writes any profile (built-in, library or the project's copy) with its ID to a file the user chooses.
   - **Import:** reads a profile file and says, before anything is written, whether a profile with its ID is new, the same, or different (naming the values that differ). For one that differs the choices are:
     - "Keep mine" (chosen at first);
     - "Replace" (writes over that library file and keeps its ID; refused for a built-in profile);
     - "Keep both" (added with a new ID and " (imported)" after its name).

     A file with no ID is new and is given one. A new profile whose name is taken gets " (imported)" too.
   - A damaged, empty, newer-schema or missing file is refused with its reason, and nothing is written (NFR-07).
6. **No change to G-code or its wording.** The editor changes profile values only. The same project, post and software version still give byte-identical G-code (GC-07, NFR-06). The window describes values as "confirmed on the machine" or "not confirmed on the machine" (specification section 15). It never says G-code is safe, verified or ready to run, and the export warning (NFR-12) is unchanged.
7. **The requirement (OI-1470; the owner chose option A).** A new P1 requirement, in the owner's agreed words: "SET-10 | Manage machine profiles in the application: view, create, edit, duplicate, import and export as files, and delete library profiles; edit the project's copy and save it to the library. Built-in profiles are read-only and can be copied to edit. | P1". Tests are named `req_set_10_*`. SET-01's meaning and ID are unchanged.
8. **Confirming a value (OI-1471; option A).** `confirm_value` marks a value confirmed on the machine only with a note saying how it was confirmed. The note must not be blank, and it becomes the value's source. This applies to library profiles and to the project's copy. A built-in profile is never written, so its values cannot be confirmed by the user. The short-move mode is a software setting and cannot be confirmed (as `MachineProfile::is_confirmed` already says). Editing the value again makes it not confirmed. "Confirmed" describes the profile value, not the G-code: the export warning (NFR-12) is unchanged.
9. **A new machine starts blank (OI-1472; option B).** `MachineDraft::new_machine` leaves these values blank:
   - the name;
   - the rotary reference point;
   - the six travel limits;
   - the six maximum feeds and rapids;
   - the safe cylinder and the safe Z.

   The rest are neutral: A continuous and positive, A separate from the feed word, tool numbers to 99, short moves Off, no spindle range, and no tool length measurement. Every value is not confirmed, with the source "Default for a new machine; check it on the machine".

   Save names every value still blank ("Fill in before saving: …"). The blanks hold placeholders far from any real value. They are never shown or saved, so filling them one at a time meets only the rules between values the user has given. A value given for the first time is the user's, even when it equals its placeholder. To start from the Makera Z1, the user duplicates it.
10. **Copies get a new ID (OI-1473; option A).**
    - Duplicate, Copy to edit (on a built-in) and "Save to library as a new profile" always give a new ID (`MachineDraft::copy_of`, `save_copy_as_new`), so a copy never becomes the source that projects made from the original compare with.
    - A copy is named "<name> copy", then "<name> copy 2" and so on, to a name no library or built-in profile has.
    - "Save to library" onto the library profile the project's copy came from (`save_copy_to_source`) keeps that profile's ID. Its confirmation names the values that change (`copy_changes`).
    - After saving as a new profile, the project's copy records the new ID (an undoable `SetMachineFrom`, in pull request 2).
11. **A renamed machine keeps its tool set (OI-1474; option A).** Machine tool sets are tied to a machine by name (OI-770). When a saved library profile is renamed, `tool_set_rename_question` gives the question for the same confirmation as Save, with "Also rename the tool set" ticked at first. `rename_tool_set` then rewrites that tool set's own file under the new name (`ToolLibrary::rename_tool_set` in `openchuck-project`, not in the library family). A name another tool set already holds is refused, and nothing is written. Built-in profiles cannot be renamed, so their tool sets are never touched.

## Pull requests

1. **The model** (`openchuck-app`, and `ToolLibrary::rename_tool_set` in `openchuck-project`): items 1 to 11, with tests named by requirement. This record, SET-10 in the requirements, the verification plan and the work breakdown, and the registers.
2. **The window** (`openchuck-ui`): a machine library window built like the tool library window:
   - the list (this project, built in, library);
   - the form with a provenance icon and tooltip on each value;
   - Save, Cancel, New, Duplicate, Copy to edit, Delete, Import and Export;
   - the project copy edited as undoable steps, with "Save to library…";
   - "Manage machines…" in the machine inspector.

   Specification section 12 gains its description.

## Alternatives considered

- **Editing the profile files in a text editor only.** That is what exists. It gives no checks before the file is read and no provenance help, and #335 asks for more.
- **Wrapping each profile value with its confirmation state.** Rejected in decision 0011; the provenance map stays beside the plain values (INT-02).
- **Dating the "changed by the user" source.** It would make two identical edits write different files, and the date adds nothing a version control history does not.

## Verification

Tests in `crates/openchuck-app/src/machine_edit.rs`, `machine_library.rs` and `crates/openchuck-project/src/tools/mod.rs`; they take under a second together, uninstrumented:

- `req_set_10_req_ui_13_the_form_lists_every_value_with_its_provenance`
- `req_set_10_the_form_shows_every_choice_and_sets_it_back_unchanged`
- `req_set_10_req_ui_13_an_edited_value_is_marked_not_confirmed_and_others_keep_theirs`
- `req_set_10_every_kind_of_value_can_be_set`
- `req_set_10_req_ui_13_a_value_is_confirmed_only_with_how`
- `req_set_10_req_nfr_07_bad_values_are_refused_in_plain_words_naming_the_field`
- `req_set_10_req_nfr_07_a_whole_profile_check_names_the_field_in_plain_words`
- `req_set_10_req_nfr_07_setting_any_value_never_panics` (property test)
- `req_set_10_a_draft_is_written_only_on_save`
- `req_set_10_req_nfr_07_save_refuses_a_taken_name_and_an_invalid_profile`
- `req_set_10_a_new_machine_starts_blank_and_saves_only_when_filled`
- `req_set_10_a_copy_has_a_free_name_and_a_new_id`
- `req_set_10_the_project_copy_saves_to_its_source_or_as_a_new_profile`
- `req_set_10_delete_removes_only_a_library_profile`
- `req_set_10_export_then_import_round_trips_and_clashes_are_resolved`
- `req_set_10_req_nfr_07_a_damaged_file_is_refused_with_its_reason`
- `req_set_10_req_tl_11_renaming_a_machine_offers_to_rename_its_tool_set`
- `req_set_10_req_tl_11_a_tool_set_follows_its_renamed_machine` (`openchuck-project`)

## Open items

| # | Item | State |
| --- | --- | --- |
| OI-1470 | The requirement that covers managing machine profiles in the application | closed: SET-10 (item 7) |
| OI-1471 | Marking a value confirmed in the user's own profile | closed: only with a note (item 8) |
| OI-1472 | What a new profile starts from | closed: blank required values (item 9) |
| OI-1473 | The ID of a copied profile | closed: always a new ID (item 10) |
| OI-1474 | Renaming a profile that a machine tool set names | closed: offer to rename the tool set too (item 11) |

OI-254 (the profile's `rotary_in_feed` against the post's feed rule) stays open. The form's description of that value points to it. OI-494 (making a tool set for a new machine) stays out of scope, by the coordinator's direction.
