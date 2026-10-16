# 0040: Slice 21a design refinement: workholding and tab data in the setup (SET-09, OP-21)

- **Status:** accepted by the project owner (15 Oct 2026), with project schema **10** and the tab layout of item 4 as written (this closes OI-175). It refines [decision 0035](0035-workholding-and-retaining-tabs.md), which the owner accepted the same day.
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): new OI-174 and OI-175 (OI-175 closed by the acceptance).

Numbering: 0039 (the OpenCASCADE cache) and 0037 (lane M) are on `main`, and an unmerged branch holds 0038, so this record is **0040**. Open items OI-169 to OI-173 are on `main` and OI-168 is the highest on an unmerged branch, so this record takes OI-174 and OI-175 (first OI-169 and OI-170, renumbered when `main` took those). Project schema **10** is claimed in the work breakdown table.

## Context

Slice 21a (lane U) is the data half of workholding: types, validation, the project file, undoable edits and the command-line report. Decision 0035 fixes the design (a held region at each end, tabs as cylinder or frustum, held in the setup) and leaves the layout and defaults to the owner (OI-158). The lane has to pick concrete field names and a few conventions to build, and to say so.

## Decision

1. **Types** in `openchuck-types::workholding` (new module, so the semver family gains types and no field): `Workholding { start, end, tabs }`, `HeldRegion { length, diameter }`, `Tab { x_start, length, shape, layout }`, `TabShape` (`Cylinder`, `Frustum`) and `TabLayout` (`OnAxis`, `AroundAxis`). `validate_workholding` is a new function beside `validate_setup`; `validate_setup`'s signature is unchanged.
2. **No defaults (OI-158).** Every dimension is explicit. An empty `Workholding` is the default for every setup, so a project without a held region or tab is unchanged and writes nothing.
3. **Positions** are model X, like the stock. `start` is the lower X end of the stock and `end` the higher. A frustum names its diameters by X (`start_diameter` at `x_start`, `end_diameter` at `x_start + length`), so which end faces the part is stated and never inferred.
4. **Tabs around the axis** have their centre lines parallel to the rotary axis at `radial_offset` from it, `count` of them (1 to 64) evenly spaced from `start_angle`, measured from model +Y towards +Z. 0035 item 4 gave count and angle but no radius, which a tab off the axis needs (confirmed by the owner on acceptance, closing OI-175).
5. **Validation (NFR-07).** Always: finite, positive lengths and diameters, a frustum whose diameters differ by at least 0.001 mm, a count in range, a non-negative offset. Where the stock's extent is known (cylinder, box): each held region and tab inside the stock, the two held regions not overlapping, tabs not in a held region, tabs on the axis not overlapping each other, a holding diameter and each tab's reach within the stock radius. Bounding-box offset stock needs the part, so extent is not checked for it. The refusal message names the field. Whether a tab lies in the head or tail zone needs the part's extent, so it is not part of this validation (item 9).
6. **Project file.** Setup files gain an optional `[workholding]` table, written only when not empty. This is a format change, so schema **10**, the next free number on `main` and the unmerged branches (the owner chose 10 over 12): the `SCHEMA` constant, one migration step that changes nothing, and the golden project text. Lanes D, O and S, which expected 10 and 11, take the next free numbers when they claim. No fixture folder is needed: the change is a plain addition.
7. **Commands.** The setup is replaced whole by the existing `Replace` command, which validates, is undoable and is covered by the undo property tests (UI-08). No new command type is added.
8. **CLI.** `report` (and `setup`) print the held regions and tabs; `validate` refuses an impossible set with the field named. There is no flag to set them yet: a front end that edits them is slice 21c (forms) and the example project is edited as TOML.

9. **Head and tail only** (decision 0035, owner's answers). `check_tab_zones` and `tab_zones` (new functions in `openchuck-types::workholding`) say which tabs are not wholly in the head zone or the tail zone, as a new `SetupWarningKind::TabOutsideEndZones`. The application adds these to the setup report beside the other setup warnings, where the part is measured (`check_setups`). It is a warning at this stage, because the project cannot measure a mesh when it validates an edit; generation (slice 21c) refuses a tab outside the zones (OI-174).

## Alternatives considered

- **Claim schema 12**, as the work breakdown suggested, with placeholder steps for 10 and 11: rejected by the owner in favour of the next free number, 10.
- **Flatten shape and layout into the tab table.** Simpler TOML, but serde cannot combine flattened enums with `deny_unknown_fields`, which every project file uses; nested tables keep unknown keys refused.

## Consequences

- `docs/work-breakdown.md`: the schema row for 10, the "next free" numbers and the slice 21 status.
- `docs/open-items.md`: OI-174 (open); OI-175 closed. OI-158 stays open (no defaults were chosen).
- Tests named after SET-09, OP-21, PRJ-04 and UI-08 are in `crates/openchuck-project/tests/workholding.rs`, `apps/openchuck-cli/tests/cli.rs` and `crates/openchuck-app/tests/undo.rs`.
