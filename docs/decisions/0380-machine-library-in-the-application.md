# 0380: Managing machine profiles in the application (lane MACHINES)

- **Status:** proposed, 31 Oct 2026, by lane MACHINES. The plan was approved by the coordinator session on 31 Oct 2026. The owner's choices OI-1470 to OI-1474 have been sent to the coordinator, one per message, and are not yet answered. Items 1 to 6 do not depend on them; item 7 lists what does.
- **Date:** 31 Oct 2026.
- **Branch:** `claude/lane-machines`. Issue [#335](https://github.com/paulhampson/OpenChuckCAM/issues/335) ("GUI has no way to manage machine library").
- **Requirements:** SET-01, UI-13, NFR-07, NFR-12, GC-07 and NFR-06. There is no requirement yet for managing a library of profiles in the application (OI-1470). SET-02, SET-08 and COL-04 (P2) are not built.
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
7. **Waiting for the owner (pull request 2, the window).**
   - OI-1470: the requirement that covers this.
   - OI-1471: marking a value confirmed.
   - OI-1472: what New starts from.
   - OI-1473: the ID of a copy.
   - OI-1474: renaming a profile a tool set names.

   The window, the project copy's full editor and "Save to library…", and the specification section that describes them are built after these are answered.

## Pull requests

1. **The model** (`openchuck-app`): items 1 to 6, with tests named by requirement. This record and the registers.
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

Tests in `crates/openchuck-app/src/machine_edit.rs` and `machine_library.rs`:

- `req_set_01_req_ui_13_the_form_lists_every_value_with_its_provenance`
- `req_ui_13_an_edited_value_is_marked_not_confirmed_and_others_keep_theirs`
- `req_set_01_the_form_shows_every_choice_and_sets_it_back_unchanged`
- `req_set_01_every_kind_of_value_can_be_set`
- `req_nfr_07_bad_values_are_refused_in_plain_words_naming_the_field`
- `req_nfr_07_a_whole_profile_check_names_the_field_in_plain_words`
- `req_nfr_07_setting_any_value_never_panics` (property test)
- `req_set_01_a_draft_is_written_only_on_save`
- `req_nfr_07_save_refuses_a_taken_name_and_an_invalid_profile`
- `req_set_01_delete_removes_only_a_library_profile`
- `req_set_01_export_then_import_round_trips_and_clashes_are_resolved`
- `req_nfr_07_a_damaged_file_is_refused_with_its_reason`

## Open items

| # | Item | State |
| --- | --- | --- |
| OI-1470 | The requirement that covers managing machine profiles in the application | open, with the owner |
| OI-1471 | Marking a value confirmed in the user's own profile | open, with the owner |
| OI-1472 | What a new profile starts from | open, with the owner |
| OI-1473 | The ID of a copied profile | open, with the owner |
| OI-1474 | Renaming a profile that a machine tool set names | open, with the owner |

OI-254 (the profile's `rotary_in_feed` against the post's feed rule) stays open. The form's description of that value points to it. OI-494 (making a tool set for a new machine) stays out of scope, by the coordinator's direction.
