# 0008: Slice 1 design refinement: project model, project folder and undo

**Status:** Accepted (11 Oct 2026). Items 9, 11, 12 and 13 were chosen by the owner in the slice 1 design review. The owner accepted items 1 to 7 as proposed and amended item 8 (a folder with no project is a warning with an offer to create one, not an error). Items 10 and 14 to 17 were proposed in the same review and went ahead without objection, and item 18, never put to the owner separately, is accepted with the record as a whole. It builds on [decision 0006](0006-sp-8-project-format.md) (accepted: TOML for the project folder, a per-item order key).

Numbering: this record was drafted as 0005 with open items OI-31 to OI-33, renumbered to 0007 with OI-37 to OI-39 when `main` took 0005 (SP-5) and 0006 (SP-8), and renumbered again to 0008 with OI-42 to OI-45 when the SP-12 record merged first as 0007 (open items OI-37 to OI-41). Decision 0004 item 15 and the SP-8 record's open items OI-35 and OI-36 keep their numbers.

## Context

Work breakdown slice 1 (project documents, persistence and undo) covers PRJ-01 to PRJ-05 and UI-08. Its design refinement asks for the file layout, how order keys are stored, schema versioning, the cache location and the command pattern for undo (architecture sections 5.6 and 5.7). Spike SP-8 was in progress when the work started, so it was done in two steps: items 1 to 8 below first, without anything that needed SP-8, then items 9 to 18 once decision 0006 was accepted.

| Slice 1 item | Where it is decided |
| --- | --- |
| 1. Project model, stable IDs, explicit order keys (PRJ-05) | Items 1, 4, 10 |
| 2. Text persistence in small files (PRJ-01) | 0006 item 2; items 9, 13, 14, 15 |
| 3. Model files by relative path with content hash (PRJ-02) | Item 2 |
| 4. No generated data; cache folder with generated ignore file (PRJ-03) | Item 3 |
| 5. Schema version in every file; migration with a fixture (PRJ-04) | Item 12 |
| 6. Command, undo and redo stack (UI-08) | Items 7, 10 |
| 7. Reserved selection descriptor fields (PRJ-07) | Item 5 |
| 8. CLI validate, load and save; loader fuzz target (NFR-07) | Items 16, 17 |

## Decision

### First step (without SP-8)

1. **IDs (PRJ-05).** One ID type per kind of item (`SetupId`, `OperationId`, `ToolId`, `SelectionSetId`, `ModelId`), each a random 128-bit value from `uuid` v4 (survey score 99), so an operation ID cannot be passed where a setup ID is expected. The `uuid` type stays private (architecture section 2). The text form is the hyphenated lower-case UUID and only that form parses, so every ID has one spelling. IDs are made when the user adds an item, never during pathing (NFR-06).
2. **Model files (PRJ-02).** A `ModelRef` holds a relative path and a BLAKE3 content hash written `blake3:` plus 64 lower-case hex digits; the prefix leaves room for another algorithm. Paths use `/` on every platform and are checked when created (non-empty, relative, no empty or `.` components); `..` is allowed so a model can sit beside the project folder, though Git will not track it there. `ModelRef::check` reports unchanged, changed (with both hashes) or missing; `Application::check_models` checks every model and `openchuck-cli validate` fails on a changed or missing one. What the UI does with a changed model is for the import slices.
3. **Generated data (PRJ-03).** The project model has no field for toolpaths, simulation results or G-code. Anything kept between runs goes in `.openchuck-cache/` inside the project folder, which holds its own generated `.gitignore` containing `*`; the folder ignores itself, including the ignore file, and the user's own ignore files are never touched. The application rewrites the ignore file if it has changed.
4. **Document model.** `Project` holds models, setups, tool snapshots (TL-07), selection sets and operations, plus a machine reference. Fields are private; every change goes through an edit (`insert`, `remove`, `replace`, `move_item`, `set_order_keys`, `renumber`, `set_name`, `set_machine`) that keeps the document valid: IDs unique, references to existing items, values finite, keyed lists in key order. Removing an item that another uses is refused rather than cascading; a cascading delete can later be one composite command. `Project::validate` checks the same rules for a whole document, for loaded files.
5. **Selections (PRJ-06, PRJ-07, UI-05).** A selection set names its model, records the model's content hash at selection time (face and edge IDs are relative to that file version, architecture section 5.6), and selects a sorted set of faces, edges or triangles. `descriptors` (position, normal, area) are reserved: P1 writes none and keeps any it reads.
6. **Operation parameters.** A map from name to value (integer, number, flag or text), kept in name order and written as plain TOML values (`stepover = 0.5`), so a one-parameter edit is a one-line diff. A whole number reads as an integer and a number with a decimal point as a number, as TOML itself distinguishes them. The strategy's parameter schema (OP-16, slice 5) gives units and validation.
7. **Undo and redo (UI-08).** Commands live in `openchuck-app`. Applying a command returns its inverse, and the history keeps those: undo applies the latest inverse and keeps the inverse of that for redo, and a new command clears redo. A command that fails changes nothing and is not recorded; a `Batch` command applies several as one step, all or none. The history is linear and unbounded. `openchuck-app` re-exports `openchuck-project` so front ends build commands without an engine crate of their own (architecture section 5.8).
8. **Opening a project (amended by the owner).** `Application::open` loads `project.toml`. A folder without one is not a crash and not silently an empty project: `open` returns `AppError::NoProject`, which a front end shows as a warning ("has no project.toml, so it is not a project yet") and answers by offering `Application::create`. `openchuck-cli` does so: `--create` accepts the offer, a terminal is asked "Create a project here? [y/N]", and otherwise (a script, a pipe) it stops with an error naming `new` and `--create`, so nothing carries on by accident and nothing is created unasked. This replaces decision 0004 item 15, under which any existing folder opened as an empty project. A missing path, a path to a file and a damaged project stay errors.

### Second step (on decision 0006)

9. **Folder layout (owner's choice, option a).**

   ```text
   project.toml            schema, name, machine reference, model references
   setups/<id>.toml        one file per setup
   tools/<id>.toml         one file per tool snapshot
   selections/<id>.toml    one file per selection set
   operations/<id>.toml    one file per operation
   .openchuck-cache/       generated data, ignored by Git (item 3)
   ```

   Model references stay a list in `project.toml`, in list order and without order keys: adding models on two branches at once is rare. It does conflict in Git when it happens (both branches append to the same list), so the owner asked that the failure be detected. The loader checks every project file for unresolved Git conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`, `|||||||`) before parsing and reports the file and line ("has an unresolved merge conflict at line N; resolve it in Git, then open the project again") instead of a TOML syntax error; a Git merge test covers it. In-app version control (VC, P2) can detect it before the merge; that belongs with OI-36.
10. **Order keys (0006 item 1).** Setups, tool snapshots, selection sets and operations carry an `order` key and are kept sorted by `(key, id)`. Adding or moving an item gives it a key between its new neighbours. When the neighbours share a key (two branches inserted at the same spot), the run of items with that key above the insertion point gets new increasing keys first, keeping their order; in practice that is one item. Undo restores keys exactly and never computes new ones: removing an item returns it with its key, and re-adding it with `Placement::Keep` puts it back where the key says; a move or renumber is undone by `SetOrderKeys` with the old keys.
11. **Order-key growth, OI-35 (owner's choice, option a).** The 0006 scheme is kept. A `Renumber` command gives one list short, evenly spaced keys (three characters or fewer for up to 5,000 items) and is only ever an explicit user action, since it changes every item's file. Keys longer than 32 characters are reported (`Project::long_order_keys`; `openchuck-cli validate` prints a warning). No length-prefixed scheme.
12. **Schema versions and migration, PRJ-04 (owner's choice, option a).** One schema number for the whole project folder, written as the first field (`schema = 1`) of every file. A file with a newer number is refused with a message naming both versions. An older file is migrated one step at a time on its raw TOML table, before the typed read, so a migration can rename, move or drop fields that the current types no longer have. Schema 1 is the first, so the real migration list is empty. The framework is proven by a test-only step from a made-up schema 0 (the project name was `title`) on a fixture folder in `crates/openchuck-project/tests/fixtures/schema-0/`. The checklist item "a fixture for every older schema version migrates" therefore holds and is empty until schema 2 exists; the first real migration adds its fixture.
13. **File-form types (owner accepted).** The `file` module has its own structs for each kind of file, mapped to and from `Project`. The schema field and the folder split stay out of the model, and migrations have a stable target. Order keys live in the in-memory items, because undo must restore them byte for byte.
14. **Strict loading.** After migration, unknown fields are refused. Item files are read in file-name order; a `.toml` file in an item folder that is not named by a valid ID is refused, and so is a file whose `id` is not its name. Files that do not end in `.toml` are ignored, so a user's notes or an editor's backup file do no harm. Order keys, duplicate and dangling IDs and non-finite numbers are refused (0006 item 3).
15. **Writing.** Every file ends with a newline and uses `\n`. A file is written only when its text changes, comparing with any `\r\n` read as `\n`, so a Windows checkout with `autocrlf` is not rewritten by a load and save. Each file is written to `<name>.tmp` and renamed into place; saving the whole folder is still not atomic (crash recovery is PRJ-08, P2). The files of removed items are deleted, and an item folder left empty is removed.
16. **CLI (item 8).** `openchuck-cli new <dir> [--name]` creates a project, `validate` loads it, checks model hashes and warns about long keys, and `resave` loads and saves it (which rewrites nothing when the folder is already in the saved form).
17. **Fuzzing (item 8, NFR-07).** A `project_files` fuzz target feeds any text to `file::parse_file` as each kind of file (conflict check, schema, migration and typed read). It runs in the pull request smoke job and the nightly job, which now also run when `openchuck-project` changes. Two property tests run the same parser on random text and on damaged real files in every test run.
18. **What a project file holds.** No comments are kept, as 0006 says; a project file is the application's to write. The `notes` field 0006 mentions is not added yet: nothing needs it until the UI exists.

## Alternatives considered

- **One generic `Id<T>` type with a marker.** Equivalent, but its derives need hand-written bounds; a small macro gives plain types with readable debug output.
- **`ulid` (89) for shorter IDs.** Nothing needs time ordering, and UUID text is what other tools expect.
- **Adding the cache folder to the project's root `.gitignore`.** Rejected: it edits a file the user owns and merges, and it does nothing for a folder that is not yet a repository. A per-user cache outside the project (an OS cache folder) was also rejected: it is harder to find and to delete with the project.
- **Snapshots of the whole project for undo.** Simpler, but memory grows with project size times history length, and an inverse command is already needed to describe each edit to the user.
- **Cascading removal** (removing a setup removes its operations). Deferred: it hides a large change behind a small command. It can be added as a composite command that removes the users first, which stays undoable.
- **One file per model reference** (option b of item 9). Merges cleanly when two branches add models, but needs a folder name other than `models/`, where users keep their model files. Not chosen; the conflict is detected instead.
- **A length-prefixed order-key scheme** (option b of item 11). Grows more slowly under repeated insertion at one spot, but changes the scheme 0006 proved with real merges. Not chosen.
- **Shipping schema 2 now** so a real older fixture exists (option b of item 12). Not chosen: a deliberate format change only to exercise the framework.
- **Treating any folder as an empty project** (the 0004 behaviour). A mistyped path would quietly start a new project. Not chosen; the warning with an offer keeps the convenience.
- **Serde attributes on the model types** instead of file-form structs. Less code, but ties the in-memory model to the file layout and leaves migrations without a stable target.
- **Externally tagged parameter values** (`stepover = { number = 0.5 }`). Unambiguous, but harder to read and review; TOML already tells integers from floats.

## Verification

Tests named after the requirement IDs (decision 0004 item 14):

- **PRJ-01:** saving twice is byte-identical (and identical in a second folder); load then save rewrites no file (modification times unchanged); Windows line endings are not rewritten; a one-parameter edit changes one line of one file; every file ends with a newline and has no carriage return; `openchuck-cli new`, `validate` and `resave`.
- **Opening (item 8):** a bare folder gives `NoProject`, creating a project there then opens; the CLI warns and fails without `--create` (not on a terminal) and creates with it.
- **PRJ-02:** relative paths; changed and missing models detected by hash; `openchuck-cli validate` fails on a changed model.
- **PRJ-03:** the cache folder ignores itself under a real `git status`; a saved folder holds only the project file and the four item folders.
- **PRJ-04:** every file starts with `schema = 1`; a newer schema is refused; the schema-0 fixture migrates through a test step, and is refused without it.
- **PRJ-05:** one text form per ID; unique IDs and known references; reorder and add each touch one file; removed items lose their files; order keys stay ordered under random and adversarial insertion; renumber gives short ordered keys and clears the long-key report; real Git merges of the SP-8 scenarios S1 to S6 are clean and give the expected order; two branches adding models give a merge conflict that loading reports by file and line.
- **PRJ-06:** selection sets survive save and load. PRJ-06 closes in slice 11 (B-rep selections).
- **UI-08:** a property test over random sequences of every command kind (including renumber) against a randomly built project compares the saved form of every file byte for byte after full undo and after full redo. Seeded faults are caught: a wrong move inverse, re-adding a removed item at the end instead of by its key, and a renumber whose undo does nothing.
- **NFR-07:** damaged folders (syntax, unknown field, missing schema, newer schema, bad order key, bad ID, dangling reference, NaN, a file not named by an ID, a file named after another item, no `project.toml`) each give a clear error; the parser property tests and the fuzz target.

## Consequences

- New dependency `uuid` 1.28.0 (`v4`), recorded in `docs/dependencies.md`. `blake3` and `toml` are now also used by `openchuck-project`. All are in the architecture's selection (section 7).
- `fuzz/` gains the `project_files` target and a path dependency on `openchuck-project`; `ci.yml` and `nightly.yml` run it.
- No change to `requirements.md` or `architecture.md`. Section 5.6 lists the project folder's files as "project, setups, one file per operation, selection sets, machine"; this record adds `tools/` for the TL-07 snapshots and leaves the machine file to slice 3 (OI-43).
- Decision 0004 item 15 (any folder opens as an empty project) no longer applies.

## Open items

Tracked in [open-items.md](../open-items.md).

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 (OI-42) | Build slice 1 persistence on decision 0006. **Closed** by items 9 to 17. | Slice 1 | Done |
| 2 (OI-43) | `MachineRef` is a placeholder (a profile name). How a project refers to or stores its machine profile (the "machine" file in architecture section 5.6). | Slice 3 | SET-01 design |
| 3 (OI-44) | Triangle selections must apply to mesh models only (architecture section 5.6). `ModelRef` does not yet record whether a model is a mesh or a B-rep, so the rule is not enforced. | Slices 2 and 10 | Model kind from import |
| 4 (OI-45) | The UI should check model hashes when it opens a project (`Application::check_models`) and tell the user which models changed or are missing (PRJ-02). The CLI already does so in `validate`. | Slice 8 | The application shell |
