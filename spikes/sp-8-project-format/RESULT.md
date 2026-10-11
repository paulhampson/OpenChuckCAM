<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# SP-8 result: project format and order keys

Work breakdown, Spikes, SP-8 (2 d). Decision record: [0006](https://github.com/paulhampson/OpenChuckCAM/blob/docs/decision-0006-sp-8-project-format/docs/decisions/0006-sp-8-project-format.md) on branch `docs/decision-0006-sp-8-project-format`.

**Question.** Which serialisation format and order-key scheme give clean diffs and merges?

**Pass criteria.** Byte-identical re-save. One-line diff for a one-parameter edit. Conflict-free merges (reorder on one branch, edit another operation on the other). Load then save is a no-op.

**Method.** A sample project shaped like architecture 5.6 (`project`, `setups/`, one file per operation in `operations/`, `selections/`), saved by serde in two candidate formats, under two order schemes:

- *Formats:* JSON (`serde_json` 1.0.152 with `float_roundtrip`, `to_string_pretty`) and TOML (`toml` 1.1.8, `to_string_pretty`). Typed structs fix field order; maps are `BTreeMap`. A file is written only when its bytes change; files of deleted items are removed.
- *Order schemes:* per-item keys (an in-house fractional order key in each item's own file, `src/order.rs`; items sort by `(key, id)`) and an ordered list of operation IDs in the project file.

Every merge scenario runs a real `git merge` (Git 2.43.0) in a fresh repository: base commit, side A on `main`, side B on a branch, merge, then load the merged folder, check the expected result and re-save (which must change nothing). Run with `cargo run --release -- <work-dir>`; `cargo test` runs the order-key property tests (5,000 random inserts, and 2,000 each of append, prepend and insert-after-first). Two runs give byte-identical reports.

**Decision.** Per-item order keys pass every criterion in both formats; the ordered list fails (scenarios S3 and S5 conflict, and in TOML S10 merges silently into an invalid list). Both formats pass, so the format choice rests on other grounds: decision 0006 proposes JSON for the project folder, as the architecture already names, with TOML (through `toml_edit`) kept for the hand-edited libraries and post definitions.

**Fallback status.** Not needed. The named fallback (second format or key scheme) was run as part of the spike: TOML passes too, and the list scheme is rejected.

Diff counts below are (files, lines added, lines removed). Output of the runner:

## Re-save and diff

Diff counts are (files, lines added, lines removed) from `git diff --numstat`.

| Candidate | Re-save byte-identical | Load then save no-op | One-parameter edit | Reorder one op | Add one op | Project bytes |
| --- | --- | --- | --- | --- | --- | --- |
| JSON / per-item keys | yes | yes | (1, 1, 1) | (1, 1, 1) | (1, 30, 0) | 2660 |
| TOML / per-item keys | yes | yes | (1, 1, 1) | (1, 1, 1) | (1, 28, 0) | 1946 |
| JSON / ordered list | yes | yes | (1, 1, 1) | (1, 1, 1) | (2, 30, 0) | 2675 |
| TOML / ordered list | yes | yes | (1, 1, 1) | (1, 1, 1) | (2, 28, 0) | 1972 |

## Real Git merges

| Scenario (side A / side B) | Pass criterion | JSON / per-item keys | TOML / per-item keys | JSON / ordered list | TOML / ordered list |
| --- | --- | --- | --- | --- | --- |
| S1 reorder op-c3 to the top / edit op-b2 stepover | yes | clean | clean | clean | clean |
| S2 reorder op-b2 down one / edit op-b2 stepover (same item) | yes | clean | clean | clean | clean |
| S3 move op-d4 to the top / move op-a1 to the end | yes | clean | clean | conflict in `project.json` | conflict in `project.toml` |
| S4 add op-e5 after op-a1 / add op-f6 at the end | yes | clean | clean | clean | clean |
| S5 add op-e5 after op-b2 / add op-f6 after op-b2 (same spot) | yes | clean | clean | conflict in `project.json` | conflict in `project.toml` |
| S6 delete op-c3 / edit op-d4 feed | yes | clean | clean | clean | clean |
| S7 edit op-b2 stepover / edit op-b2 feed (same item, two lines apart) | informative | clean | clean | clean | clean |
| S8 edit op-b2 tolerance / edit op-b2 stepover (same item, adjacent lines) | informative | conflict in `operations/op-b2.json` | conflict in `operations/op-b2.toml` | conflict in `operations/op-b2.json` | conflict in `operations/op-b2.toml` |
| S9 add face 34 / add face 55 to the same selection set | informative | conflict in `selections/sel-41c0.json` | conflict in `selections/sel-41c0.toml` | conflict in `selections/sel-41c0.json` | conflict in `selections/sel-41c0.toml` |
| S10 move op-c3 to the top / move op-c3 to the end (true conflict) | informative | conflict in `operations/op-c3.json` | conflict in `operations/op-c3.toml` | conflict in `project.json` | clean in Git, but the merged project is invalid: operation_order lists op-c3 twice |

## Float round trip

- json: 19995 values, 0 not bit-exact
  - NaN is written as: Ok("{   \"v\": null }")
- toml: 19995 values, 0 not bit-exact
  - NaN is written as: Ok("v = nan")

## Hand-edited TOML

- toml_edit: 1 line changed, comments kept: true (item replaced: 1 line changed, comments kept: false)
- toml serde re-save: comments kept: false

## Order key length (longest key)

- append, 100 times: 4 characters
- append, 1000 times: 33 characters
- prepend, 100 times: 4 characters
- prepend, 1000 times: 33 characters
- insert after first, 100 times: 21 characters
- insert after first, 1000 times: 201 characters

## Sample files (per-item keys)

`operations/op-b2.json`:

```json
{
  "schema": 1,
  "id": "op-b2",
  "order": "V",
  "name": "Finish",
  "setup": "s-7d2a",
  "strategy": "wrap-finish",
  "enabled": true,
  "selection": "sel-41c0",
  "tool": {
    "library_id": "t-3",
    "kind": "flat",
    "diameter": 3.0,
    "flutes": 2
  },
  "params": {
    "tolerance": 0.01,
    "stepover": 0.3,
    "stepdown": 0.5,
    "feed": 800.0,
    "plunge_feed": 200.0,
    "spindle": 18000.0,
    "safe_radius": 30.0,
    "a_start": 0.0,
    "a_end": 360.0
  },
  "extra": {
    "wrap_overlap": 0.5
  }
}
```

`operations/op-b2.toml`:

```toml
schema = 1
id = "op-b2"
order = "V"
name = "Finish"
setup = "s-7d2a"
strategy = "wrap-finish"
enabled = true
selection = "sel-41c0"

[tool]
library_id = "t-3"
kind = "flat"
diameter = 3.0
flutes = 2

[params]
tolerance = 0.01
stepover = 0.3
stepdown = 0.5
feed = 800.0
plunge_feed = 200.0
spindle = 18000.0
safe_radius = 30.0
a_start = 0.0
a_end = 360.0

[extra]
wrap_overlap = 0.5
```

