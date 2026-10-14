# Changelog

All notable changes to `openchuck-project` are recorded here.

## Unreleased

- Machine profile files schema 2 (decision 0029 item 16): an optional `id` kept by project copies; the built-in Makera Z1 has a fixed ID; `new_machine_id`.
- Project schema 6 (decision 0029 item 10): `Operation::enabled`, written to the operation's file only when false; the schema 5 to 6 migration changes only the number.
- Slice 7 (decision 0025): the project holds `SimulationSettings` (`simulation`, `set_simulation`), written to `project.toml` as `[simulation]` only when not the default; project folder schema 5, migrated from 4.
- Slice 4 (decision 0014): tool snapshots gain the controller mapping and cutting data; project folder schema 4 (a schema 3 tool snapshot is refused with what to do); new `tools` module: the tool library folder (`ToolLibrary`, one TOML file per tool, import, export, `find`), machine tool sets in `tool-sets/` (`ToolSet`, TL-11), custom profile text files (arcs by centre or by radius and curve) and `compare_snapshot` (TL-07). `file::write_if_changed` is shared within the crate.
- Slice 3 (decision 0011): the project keeps a copy of its machine profile in `machine.toml` (replacing the `MachineRef` placeholder, OI-43); setups gain part, stock, material, part and work offsets and clearance; project folder schema 3, migrated from 2; new `machines` module (profile files with their own schema, the built-in Makera Z1 profile, library folders).
- Model references gain `units` and `transform`; the project folder schema is 2, migrated from 1; selections must match the model's kind (slice 2, decision 0010, OI-44).
- Project document model: models, setups, tool snapshots, selection sets and operations with stable IDs, edits that keep the document valid, and whole-document validation (slice 1, PRJ-05).
- Model references by relative path with a BLAKE3 content hash and a change check (PRJ-02).
- Cache folder for generated data with its own ignore file (PRJ-03).
- Selection sets with reserved descriptor fields (PRJ-06, PRJ-07).
- Order keys on setups, tools, selection sets and operations, with renumbering and a long-key report (PRJ-05, decisions 0006 and 0008).
- The project folder as TOML files: `file::save`, `file::load`, `file::create`, schema version in every file, step-by-step migration, merge-conflict detection and strict loading (PRJ-01, PRJ-04).
- Crate created as a shell (work breakdown slice 0).
