# 0006: SP-8 result, project format and order keys

- **Status:** accepted (11 Oct 2026). The spike met its pass criteria. The owner chose TOML for the project folder (OI-34, closed); the order-key scheme follows from the spike results.
- **Date:** 11 Oct 2026.

## Context

Work breakdown, Spikes, SP-8 (2 d): which serialisation format and order-key scheme give clean diffs and merges? It runs before slice 1, whose item 2 uses "the serialisation approach chosen in SP-8".

Requirements and sections affected: PRJ-01 (text format with stable ordering), PRJ-04 (schema version in every file), PRJ-05 (stable IDs and explicit order, clean merges of different items), NFR-05 and NFR-06 (exact doubles, deterministic output); architecture sections 5.6 (project folder, IDs and order), 7 (project files: `serde` with `serde_json`, `toml_edit` for hand-edited files; order keys in-house with `fractional_index` as a reference), 9 (project file tests) and 14 (order key scheme risk); verification plan section 2 (project file tests).

The spike code is throwaway (spike rules). It is in `spikes/sp-8-project-format/` on branch `spike/sp-8-project-format`, with its write-up and full output in `RESULT.md` there. It is a separate Cargo project, not a workspace member, and must not be merged into the product.

The slice 1 work on branch `claude/slice-1-project-model` waits on this record: its decision record leaves the file format, order keys, schema versions and migration to SP-8 (its open item "Apply SP-8's result to slice 1").

Numbering: at the time of writing, record 0005 and OI-31 to OI-33 are claimed on two unmerged branches (`claude/slice-1-project-model` and `docs/decision-0005-sp-5-gcode-reader`), so this record takes 0006 and OI-34 to OI-36.

## Decision

1. **Order: a per-item order key**, stored in each item's own file (operations, setups, selection sets, and tools where they are ordered). Keys are in-house variable-length base-62 strings compared bytewise, never ending in `0`, so a key always exists between two others. Items sort by `(key, id)`, so two branches that insert at the same spot and pick the same key still give one deterministic order. Moving or adding an item writes one file; when it lands between two items that share a key, the upper neighbour gets a new key too (two files). No ordered list of IDs is stored anywhere.
2. **Format: TOML for the project folder**, chosen by the owner on 11 Oct 2026. The app reads and writes project files through `serde` with `toml` (`to_string_pretty`), which gave byte-identical re-saves and bit-exact doubles in the spike. Hand-edited files (tool and material libraries, post definitions) are edited through `toml_edit`, which keeps a user's comments and layout when one value changes. `serde_json` stays for the shared plain-data form (INT-02), tests and fuzzing, but is not used for project files. Project files the app rewrites do not keep comments; a user who wants notes uses a `notes` field, not a comment.
3. **Rules slice 1 takes from the spike:** typed structs for fixed field order and `BTreeMap` for maps; one file per item named by its ID; write a file only when its bytes change and remove the files of deleted items; validate every order key and reject a duplicated or dangling ID on load; reject NaN and infinity before saving (JSON writes NaN as `null`, TOML as `nan`); end every file with a newline.

## What the spike showed

Diff counts are (files, lines added, lines removed) from `git diff --numstat` on the sample project (one setup, four operations, one selection set).

| Candidate | Re-save identical | Load then save no-op | One-parameter edit | Reorder one op | Add one op |
| --- | --- | --- | --- | --- | --- |
| JSON, per-item keys | yes | yes | 1, 1, 1 | 1, 1, 1 | 1, 30, 0 |
| TOML, per-item keys | yes | yes | 1, 1, 1 | 1, 1, 1 | 1, 28, 0 |
| JSON, ordered list | yes | yes | 1, 1, 1 | 1, 1, 1 | 2, 30, 0 |
| TOML, ordered list | yes | yes | 1, 1, 1 | 1, 1, 1 | 2, 28, 0 |

Real Git merges (side A / side B). S1 to S6 are the pass criterion (different items, or a reorder and an edit); S7 to S10 are informative.

| Scenario | Per-item keys (JSON and TOML) | Ordered list (JSON / TOML) |
| --- | --- | --- |
| S1 reorder one op / edit another op | clean | clean / clean |
| S2 reorder one op / edit the same op | clean | clean / clean |
| S3 move one op to the top / move another to the end | clean | conflict / conflict |
| S4 add an op after the first / add one at the end | clean | clean / clean |
| S5 both add an op at the same spot | clean, order by ID | conflict / conflict |
| S6 delete one op / edit another | clean | clean / clean |
| S7 edit two parameters of one op, two lines apart | clean | clean / clean |
| S8 edit two adjacent parameters of one op | conflict | conflict / conflict |
| S9 both add a face to the same selection set | conflict | conflict / conflict |
| S10 both move the same op to different places | conflict (correct) | conflict / **clean in Git, invalid result** (the ID listed twice; the loader rejects it) |

Other measurements:

- Doubles: 19,995 values (random bit patterns and edge cases) round-trip bit-exactly in both formats.
- Size: the sample folder is 2,660 bytes in JSON and 1,946 in TOML.
- `toml_edit` changes one value of a hand-edited file in one line and keeps its comments, provided the value's decor is carried over; replacing the item drops the comment on that line. Re-saving through serde drops all comments.
- Key length (longest key): 4 characters after 100 appends or prepends and 33 after 1,000; inserting repeatedly right after the same item grows keys by about one character per six inserts (21 after 100, 201 after 1,000).

## Alternatives considered

- **Ordered list of IDs in the project file** (the second key scheme): fails the merge criterion (S3, S5) and, in TOML, can merge silently into an invalid list (S10). Rejected.
- **JSON for the project folder.** Passes every criterion and was the architecture's original choice. Against it: about 38% larger in the spike (2,660 bytes against 1,946), harder to read and review, and no comments. In its favour: hosts in other languages read JSON everywhere (INT-05), and `jsonschema` checks files directly. The owner chose TOML; the JSON-schema point is covered because `schemars` generates a JSON Schema from the Rust types whatever the file format, and the Python binding (P2) can read TOML through the standard `tomllib`.
- **Integer order keys with gaps.** Not built: when a gap runs out, renumbering touches many files and conflicts with any concurrent edit, which is what PRJ-05 rules out.
- **`fractional_index` (64).** Read as a reference, not used: the in-house scheme is about 130 lines without its tests, and the crate is small and unmaintained since Sep 2024 (crate selection). Its fixed-length integer prefix would make appends grow more slowly; slice 1 can adopt that refinement if OI-35 needs it.

## Outstanding items

1. **OI-34: format for the project folder**: closed on 11 Oct 2026 with TOML.
2. **OI-35: order-key growth.** Keys grow under repeated insertion at one spot. Decide in slice 1 whether to add an explicit "renumber" action (it touches every item, so it is a deliberate commit, never automatic) or a length-prefixed scheme.
3. **OI-36: concurrent edits to the same item.** Git conflicts when both branches edit adjacent lines of one item (S8) or append to the same selection set (S9). PRJ-05 only asks that edits to different items merge, so this is met; a semantic merge driver belongs with in-app version control (VC, P2). Slice 1 could sort face lists so concurrent additions land apart more often.

## Consequences

- Architecture section 7 (project files row), the crate selection (summary row and the project-file paragraph), the dependency register (`toml` now also serves `openchuck-project`; `serde_json` no longer does) and work breakdown slice 1 item 2 are updated in the same pull request. Section 5.6 already describes the layout without naming a format.
- `toml_edit` 0.25.17 was tried and is recorded in `docs/dependencies.md` under "Recorded but not yet declared"; `toml` 1.1.8 is already declared and gains `openchuck-project` as a user. The two share one parser, so the project folder and the hand-edited files read the same dialect. No rule exception or `xtask` allow-list entry is needed.
- Slice 1 builds the format in `openchuck-project` with the tests the verification plan names (save twice, load then save, a Git merge test, PRJ-05) and keeps the sample project as a fixture. The spike's merge scenarios S1 to S6 are the model for its Git merge test.
