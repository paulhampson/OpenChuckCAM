# Changelog

All notable changes to `openchuck-project` are recorded here.

## Unreleased

- Model references gain `units` and `transform`; the project folder schema is 2, migrated from 1; selections must match the model's kind (slice 2, decision 0010, OI-44).
- Project document model: models, setups, tool snapshots, selection sets and operations with stable IDs, edits that keep the document valid, and whole-document validation (slice 1, PRJ-05).
- Model references by relative path with a BLAKE3 content hash and a change check (PRJ-02).
- Cache folder for generated data with its own ignore file (PRJ-03).
- Selection sets with reserved descriptor fields (PRJ-06, PRJ-07).
- Order keys on setups, tools, selection sets and operations, with renumbering and a long-key report (PRJ-05, decisions 0006 and 0008).
- The project folder as TOML files: `file::save`, `file::load`, `file::create`, schema version in every file, step-by-step migration, merge-conflict detection and strict loading (PRJ-01, PRJ-04).
- Crate created as a shell (work breakdown slice 0).
