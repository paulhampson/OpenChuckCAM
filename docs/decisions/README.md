# Decision records

A decision record captures a choice made after the documents in `docs/` were written, or one that changes them. The architecture's "Open decisions" section (architecture.md, section 13) lists what is still open.

## Format

One file per decision, named `NNNN-short-title.md` (four digits, starting at `0001`). Each record contains:

- **Status:** proposed, accepted, or superseded by `NNNN`.
- **Context:** the requirement IDs and document sections it affects.
- **Decision:** what was chosen, in a sentence or two.
- **Alternatives considered:** with reasons, including any spike results.
- **Consequences:** documents or code that must change, and any rule exceptions (for example an entry in the `xtask` dependency allow-list, which also needs the NFR-14 justification).

When a record is accepted, update the affected document in the same pull request.

## Index

Add a row for each new record. Read the records that touch your task before you start.

| Record | Title | Status |
| --- | --- | --- |
| [0001](0001-sp3-wrapped-path-result.md) | SP-3 result, wrapped path to G-code with A words | Accepted (simulator criterion met by [0009](0009-linuxcnc-fixture-checks.md); open items OI-03, OI-04 and OI-06 to OI-08 stay in [open-items.md](../open-items.md)) |
| [0002](0002-sp-2-viewport-spike.md) | SP-2 viewport spike, interim result | Proposed (keep or fall back is open until the NFR-03 measurements are recorded) |
| [0003](0003-step-kernel-route.md) | SP-1 STEP kernel route: thin in-house wrapper over OpenCASCADE, isolated import process | Accepted (third-party STEP samples still required before slice 11 import work is accepted) |
| [0004](0004-slice-0-guardrails.md) | Slice 0 design refinement, guardrails and shared data | Accepted (IR not frozen until a later slice). Open items in [open-items.md](../open-items.md) |
| [0005](0005-sp-5-gcode-reader.md) | SP-5 result, G-code reader: in-house reader | Accepted (LinuxCNC check of the fixture done in [0009](0009-linuxcnc-fixture-checks.md): the file needs four arc lines corrected, OI-46) |
| [0006](0006-sp-8-project-format.md) | SP-8 result, project format and order keys: per-item order keys; TOML for the project folder | Accepted |
| [0007](0007-sp-12-z1-conventions.md) | SP-12 result, Makera Z1 rotary conventions and G93 (desk part) | Proposed (air cut outstanding, OI-37) |
| [0008](0008-slice-1-project-model.md) | Slice 1 design refinement: project model, project folder layout, order keys, schema migration and undo | Accepted |
| [0009](0009-linuxcnc-fixture-checks.md) | LinuxCNC checks of the SP-3 and SP-5 fixtures | Accepted (open items OI-46 and OI-47 in [open-items.md](../open-items.md)) |
| [0010](0010-slice-2-stl-import.md) | Slice 2 design refinement: STL import, mesh checks, model placement and project schema 2 | Accepted (open items OI-50 to OI-52 in [open-items.md](../open-items.md)) |
| [0011](0011-slice-3-machine-stock-setup.md) | Slice 3 design refinement: machine profile, stock, setup frames and clearance; project schema 3 | Proposed (built; awaiting the owner's review. Z1 values unconfirmed, OI-37 and OI-39) |
| [0012](0012-sp-13-tool-profiles.md) | SP-13 result, one revolved-profile tool model: monotone profiles, generic drop-cutter | Accepted (the sweep budget waits on SP-6, OI-56) |
| [0013](0013-z1-profile-consensus.md) | Makera Z1 profile values: consensus across Makera's published profiles (A direction, travel, feeds) | Proposed (amends 0011 item 8; all values unconfirmed, OI-60) |
