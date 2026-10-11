# 0007: Slice 1 project model and undo

**Status:** Proposed. Records the slice 1 design refinement for the work that did not depend on spike SP-8. SP-8 has since been decided by [decision 0006](0006-sp-8-project-format.md) (accepted: TOML for the project folder, a per-item order key). The file layout, schema versioning, migration and the order-key details 0006 leaves open (OI-35) are slice 1 design refinement still to agree; they will be added here or in a follow-up record.

Numbering: this record was drafted as 0005 with open items OI-31 to OI-33 on its branch; those numbers were taken on `main` by decisions 0005 (SP-5) and 0006, so it is 0007 with OI-37 to OI-39.

## Context

Work breakdown slice 1 (project documents, persistence and undo) covers PRJ-01 to PRJ-05 and UI-08; its design refinement asks for the file layout, order-key storage, schema versioning, cache location and the command pattern for undo (architecture section 5.6, 5.7). Spike SP-8 (project format and order keys) was in progress when this work started, and the owner asked that nothing depending on it go ahead until its decision record existed. That record is now [0006](0006-sp-8-project-format.md).

Slice 1 items, sorted by whether they needed SP-8:

| Item | Needed SP-8? | State |
| --- | --- | --- |
| 1. Project model with stable IDs and explicit order keys (PRJ-05) | Order keys only | Model and IDs built; order keys (scheme in 0006 item 1) still to build (OI-37) |
| 2. Text persistence (PRJ-01) | Yes | TOML through `serde` and `toml` (0006 item 2); layout still to agree (OI-37) |
| 3. Model files by relative path with content hash (PRJ-02) | No | Built |
| 4. No generated data; cache folder with generated ignore file (PRJ-03) | No | Built |
| 5. Schema version in every file, migration framework (PRJ-04) | Yes: where the version sits and what a migration operates on depend on the format | Still to agree and build (OI-37) |
| 6. Command, undo and redo stack (UI-08) | No | Built; the byte-for-byte check waits for the saved form (OI-37) |
| 7. Reserved descriptor fields in the selection schema (PRJ-07) | No (field set only; spelling on disk is SP-8's) | Built |
| 8. CLI validate, load and save; loader fuzz target (NFR-07) | Yes | Still to build (OI-37) |

## Decision

1. **IDs (PRJ-05).** One ID type per kind of item (`SetupId`, `OperationId`, `ToolId`, `SelectionSetId`, `ModelId`), each a random 128-bit value from `uuid` v4 (survey score 99), so an operation ID cannot be passed where a setup ID is expected. The `uuid` type stays private (architecture section 2). The text form is the hyphenated lower-case UUID and only that form parses, so every ID has one spelling. IDs are made when the user adds an item, never during pathing (NFR-06).
2. **Model files (PRJ-02).** A `ModelRef` holds a relative path and a BLAKE3 content hash written `blake3:` plus 64 lower-case hex digits; the prefix leaves room for another algorithm. Paths use `/` on every platform and are checked when created (non-empty, relative, no empty or `.` components); `..` is allowed so a model can sit beside the project folder, though Git will not track it there. `ModelRef::check` reports unchanged, changed (with both hashes) or missing. What the application does with a changed model is for the import slices.
3. **Generated data (PRJ-03).** The project model has no field for toolpaths, simulation results or G-code. Anything kept between runs goes in `.openchuck-cache/` inside the project folder, which holds its own generated `.gitignore` containing `*`; the folder ignores itself, including the ignore file, and the user's own ignore files are never touched. The application rewrites the ignore file if it has changed.
4. **Document model.** `Project` holds models, setups, tool snapshots (TL-07), selection sets and operations, each list in its user-visible order, plus a machine reference. Fields are private; every change goes through an edit (`insert`, `remove`, `replace`, `move_item`, `set_name`, `set_machine`) that keeps the document valid: IDs unique, references to existing items, values finite. Removing an item that another uses is refused rather than cascading; a cascading delete can later be one composite command. `Project::validate` checks the same rules for a whole document, for loaded files. How list order is written to disk is SP-8's order-key scheme.
5. **Selections (PRJ-06, PRJ-07, UI-05).** A selection set names its model, records the model's content hash at selection time (face and edge IDs are relative to that file version, architecture section 5.6), and selects a sorted set of faces, edges or triangles. `descriptors` (position, normal, area) are reserved: P1 writes none and keeps any it reads.
6. **Operation parameters.** A map from name to value (number, integer, flag or text), kept in name order. The strategy's parameter schema (OP-16, slice 5) gives units and validation.
7. **Undo and redo (UI-08).** Commands live in `openchuck-app`. Applying a command returns its inverse, and the history keeps those: undo applies the latest inverse and keeps the inverse of that for redo, and a new command clears redo. A command that fails changes nothing and is not recorded. The history is linear and unbounded. `openchuck-app` re-exports `openchuck-project` so front ends build commands without an engine crate of their own (architecture section 5.8).
8. **Opening a project.** Until loading is built (OI-37), an existing folder still opens as an empty project (decision 0004 item 15), now named after the folder.

## Alternatives considered

- **One generic `Id<T>` type with a marker.** Equivalent, but its derives need hand-written bounds; a small macro gives plain types with readable debug output.
- **`ulid` (89) for shorter IDs.** Nothing needs time ordering, and UUID text is what other tools expect.
- **Adding the cache folder to the project's root `.gitignore`.** Rejected: it edits a file the user owns and merges, and it does nothing for a folder that is not yet a repository. A per-user cache outside the project (an OS cache folder) was also rejected: it is harder to find and to delete with the project.
- **Snapshots of the whole project for undo.** Simpler, but memory grows with project size times history length, and an inverse command is already needed to describe each edit to the user.
- **Cascading removal** (removing a setup removes its operations). Deferred: it hides a large change behind a small command. It can be added as a composite command that removes the users first, which stays undoable.

## Verification

Tests named after the requirement IDs (decision 0004 item 14):

- `openchuck-project`: `req_prj_05_*` (one text form per ID, unique IDs, known references, items in use cannot be removed, reorder keeps IDs and contents), `req_prj_02_*` (relative paths, changed and missing models detected by hash), `req_prj_03_cache_folder_ignores_itself` (a real `git status` lists nothing from the cache), `req_nfr_07_bad_values_are_errors`, and a property test that hash text round-trips.
- `openchuck-app`: `req_ui_08_full_undo_restores_and_redo_reapplies`, a property test over random sequences of every command kind against a randomly built project, which compares the full debug form (stricter than equality: it tells `-0.0` from `0.0`); a seeded wrong inverse makes it fail. Order keys (0006) will change how moves are undone: an inverse must restore the old keys exactly, not compute new ones. `req_ui_08_new_command_clears_redo_and_failed_command_is_not_recorded`.

The slice 1 review checklist is not ticked: each of its items needs the saved form except "Undo then redo restores the exact prior project", which holds in memory and gets its byte-for-byte check with the saved form.

## Consequences

- New dependency `uuid` 1.28.0 (`v4`), recorded in `docs/dependencies.md`; `blake3` is now also used by `openchuck-project`. Both are in the architecture's selection (section 7).
- No change to `requirements.md` or `architecture.md`: the decisions above fill in section 5.6 and 5.7 without changing them.

## Open items

Tracked in [open-items.md](../open-items.md).

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 (OI-37) | Build slice 1 persistence on decision 0006: file layout (item 2), order keys on items (item 1), schema version and migration with a real fixture (item 5), CLI load, save and validate and the loader fuzz target (item 8), the byte-for-byte undo check (verification plan UI-08) and the four checklist items that need a saved form. Needs the remaining design refinement decisions first, including whether `Project` gets serde attributes or a separate file-form type, and OI-35. | Slice 1 | Design refinement agreement |
| 2 (OI-38) | `MachineRef` is a placeholder (a profile name). How a project refers to or stores its machine profile (the "machine" file in architecture section 5.6). | Slice 3 | SET-01 design |
| 3 (OI-39) | Triangle selections must apply to mesh models only (architecture section 5.6). `ModelRef` does not yet record whether a model is a mesh or a B-rep, so the rule is not enforced. | Slices 2 and 10 | Model kind from import |
