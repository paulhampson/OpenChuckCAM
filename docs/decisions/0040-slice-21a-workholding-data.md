# 0040: Slice 21a design refinement: workholding and tab data in the setup (SET-09, OP-21)

- **Status:** proposed (15 Oct 2026). It refines [decision 0035](0035-workholding-and-retaining-tabs.md), which is itself still *proposed*, so this record relies on proposed guidance and is built on that footing. Nothing here is final until the owner accepts both.
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): new OI-169 and OI-170.

Numbering: the unmerged branches hold records 0037 (lane M) and 0038 (lane N and the UI specification branch both took it, so one of them will renumber to 0039). This record takes **0040** so it does not collide with that renumbering. OI-168 is the highest on an unmerged branch, so this record takes OI-169 and OI-170. Project schema **12** is claimed in the work breakdown table.

## Context

Slice 21a (lane U) is the data half of workholding: types, validation, the project file, undoable edits and the command-line report. Decision 0035 fixes the design (a held region at each end, tabs as cylinder or frustum, held in the setup) and leaves the layout and defaults to the owner (OI-158). The lane has to pick concrete field names and a few conventions to build, and to say so.

## Decision (proposed)

1. **Types** in `openchuck-types::workholding` (new module, so the semver family gains types and no field): `Workholding { start, end, tabs }`, `HeldRegion { length, diameter }`, `Tab { x_start, length, shape, layout }`, `TabShape` (`Cylinder`, `Frustum`) and `TabLayout` (`OnAxis`, `AroundAxis`). `validate_workholding` is a new function beside `validate_setup`; `validate_setup`'s signature is unchanged.
2. **No defaults (OI-158).** Every dimension is explicit. An empty `Workholding` is the default for every setup, so a project without a held region or tab is unchanged and writes nothing.
3. **Positions** are model X, like the stock. `start` is the lower X end of the stock and `end` the higher. A frustum names its diameters by X (`start_diameter` at `x_start`, `end_diameter` at `x_start + length`), so which end faces the part is stated and never inferred.
4. **Tabs around the axis** have their centre lines parallel to the rotary axis at `radial_offset` from it, `count` of them (1 to 64) evenly spaced from `start_angle`, measured from model +Y towards +Z. 0035 item 4 gave count and angle but no radius, which a tab off the axis needs (OI-170 asks the owner to confirm this reading).
5. **Validation (NFR-07).** Always: finite, positive lengths and diameters, a frustum whose diameters differ by at least 0.001 mm, a count in range, a non-negative offset. Where the stock's extent is known (cylinder, box): each held region and tab inside the stock, the two held regions not overlapping, tabs not in a held region, tabs on the axis not overlapping each other, a holding diameter and each tab's reach within the stock radius. Bounding-box offset stock needs the part, so extent is not checked for it. The refusal message names the field. Whether a tab lies between the held region and the part's far end needs the part's extent and is left to OI-169.
6. **Project file.** Setup files gain an optional `[workholding]` table, written only when not empty. This is a format change, so schema **12** (SCHEMA constant, one migration step that changes nothing, golden project text). Schemas 10 and 11 are claimed by nobody yet but are expected by lanes D, O and S; to keep the migration list contiguous, steps 9 to 10 and 10 to 11 are placeholders that change nothing, and the lane that takes 10 or 11 fills its step in or renumbers (a claimed number can be renumbered freely until the first release). No fixture folder is needed: the change is a plain addition.
7. **Commands.** The setup is replaced whole by the existing `Replace` command, which validates, is undoable and is covered by the undo property tests (UI-08). No new command type is added.
8. **CLI.** `report` (and `setup`) print the held regions and tabs; `validate` refuses an impossible set with the field named. There is no flag to set them yet: a front end that edits them is slice 21c (forms) and the example project is edited as TOML.

## Alternatives considered

- **Claim schema 10.** The next number free on `main` and unmerged branches, but the work breakdown lists 10 and 11 as expected by other lanes and tells this lane to take 12.
- **Flatten shape and layout into the tab table.** Simpler TOML, but serde cannot combine flattened enums with `deny_unknown_fields`, which every project file uses; nested tables keep unknown keys refused.

## Consequences

- `docs/work-breakdown.md`: the schema row for 12, the "next free" numbers and the slice 21 status.
- `docs/open-items.md`: OI-169, OI-170. OI-158 stays open (no defaults were chosen).
- Tests named after SET-09, OP-21, PRJ-04 and UI-08 are in `crates/openchuck-project/tests/workholding.rs`, `apps/openchuck-cli/tests/cli.rs` and `crates/openchuck-app/tests/undo.rs`.
