# 0005: SP-8 result, project format and order keys

- **Status:** proposed. The spike met its pass criteria. The order-key scheme follows from the results; the choice of format (JSON or TOML) awaits the project owner (OI-31).
- **Date:** 11 Oct 2026.

## Context

Work breakdown, Spikes, SP-8 (2 d): which serialisation format and order-key scheme give clean diffs and merges? It runs before slice 1, whose item 2 uses "the serialisation approach chosen in SP-8".

Requirements and sections affected: PRJ-01 (text format with stable ordering), PRJ-04 (schema version in every file), PRJ-05 (stable IDs and explicit order, clean merges of different items), NFR-05 and NFR-06 (exact doubles, deterministic output); architecture sections 5.6 (project folder, IDs and order), 7 (project files: `serde` with `serde_json`, `toml_edit` for hand-edited files; order keys in-house with `fractional_index` as a reference), 9 (project file tests) and 14 (order key scheme risk); verification plan section 2 (project file tests).

The spike code is throwaway (spike rules). It is in `spikes/sp-8-project-format/` on branch `claude/sp-8-dhnvjs`, with its write-up and full output in `RESULT.md` there. It is a separate Cargo project, not a workspace member, and must not be merged into the product.

## Decision

1. **Order: a per-item order key**, stored in each item's own file (operations, setups, selection sets, and tools where they are ordered). Keys are in-house variable-length base-62 strings compared bytewise, never ending in `0`, so a key always exists between two others. Items sort by `(key, id)`, so two branches that insert at the same spot and pick the same key still give one deterministic order. Moving or adding an item writes one file; when it lands between two items that share a key, the upper neighbour gets a new key too (two files). No ordered list of IDs is stored anywhere.
2. **Format (proposed, OI-31): JSON for the project folder** through `serde_json` with `float_roundtrip` and `to_string_pretty`, as architecture section 7 already names, so no document changes. TOML passed every criterion as well and is the recorded alternative. Hand-edited files (tool and material libraries, post definitions) stay TOML through `toml_edit`, as the architecture says.
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
- **TOML for the project folder.** Passes every criterion, about 27% smaller, reads more easily, has no trailing-comma churn, and with `toml_edit` users could keep comments in operation files. Against it: hosts in other languages read JSON everywhere (INT-05), `schemars` and `jsonschema` publish and check a JSON schema, and the architecture already names JSON. The owner decides (OI-31); choosing TOML means editing architecture section 7.
- **Integer order keys with gaps.** Not built: when a gap runs out, renumbering touches many files and conflicts with any concurrent edit, which is what PRJ-05 rules out.
- **`fractional_index` (64).** Read as a reference, not used: the in-house scheme is about 130 lines without its tests, and the crate is small and unmaintained since Sep 2024 (crate selection). Its fixed-length integer prefix would make appends grow more slowly; slice 1 can adopt that refinement if OI-32 needs it.

## Outstanding items

1. **OI-31: format for the project folder**, JSON (recommended) or TOML. Owner decision before slice 1 item 2.
2. **OI-32: order-key growth.** Keys grow under repeated insertion at one spot. Decide in slice 1 whether to add an explicit "renumber" action (it touches every item, so it is a deliberate commit, never automatic) or a length-prefixed scheme.
3. **OI-33: concurrent edits to the same item.** Git conflicts when both branches edit adjacent lines of one item (S8) or append to the same selection set (S9). PRJ-05 only asks that edits to different items merge, so this is met; a semantic merge driver belongs with in-app version control (VC, P2). Slice 1 could sort face lists so concurrent additions land apart more often.

## Consequences

- If OI-31 is settled as JSON, no document changes: architecture sections 5.6 and 7 already describe this design. If TOML is chosen, architecture section 7 and the crate selection's project-file row change, and this record is updated.
- `toml_edit` 0.25.17 was tried and is recorded in `docs/dependencies.md` under "Recorded but not yet declared". No rule exception or `xtask` allow-list entry is needed.
- Slice 1 builds the format in `openchuck-project` with the tests the verification plan names (save twice, load then save, a Git merge test, PRJ-05) and keeps the sample project as a fixture. The spike's merge scenarios S1 to S6 are the model for its Git merge test.
