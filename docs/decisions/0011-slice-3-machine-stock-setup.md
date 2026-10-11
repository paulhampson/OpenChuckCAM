# 0011: Slice 3 design refinement: machine profile, stock, setup frames and clearance

**Status:** Proposed (11 Oct 2026). Built and tested on branch `claude/slice-3-machine-setup`; awaiting the owner's review. Items marked **(owner)** are choices the owner may want to change; the rest follow from the documents. It relies on [decision 0007](0007-sp-12-z1-conventions.md), which is itself proposed (air cut outstanding), for every Makera Z1 value.

## Context

Work breakdown slice 3 (machine, stock and setup) covers SET-01, SET-03, SET-05 and SET-06, with NFR-07 for the profile file. Its design refinement asks for the coordinate frames (part, setup, machine), the A-axis sign convention, the machine profile schema with the rotary axis orientation as a parameter (X now, Y later under SET-02, never hard-coded) and how clearance geometry is represented. Spike inputs: SP-12 (decision 0007) and SP-3 (decision 0001). Architecture sections 5.1, 5.2, 5.6 and 9; verification plan sections 4 (SET-01 to SET-06 row, NFR-07) and 4.1 (kinematic round trip, 1e-6 mm). Accepted records it touches: 0004 items 4 and 5 (profile fields, kinematic conventions) and 0008 (project folder, OI-43). Open items it depends on: OI-03 (A sign), OI-37 (air cut), OI-39 (Z1 limits), OI-40 (work offset) and OI-43 (machine reference).

| Slice 3 item | Where it is decided |
| --- | --- |
| 1. Machine profile in `openchuck-types`, stored separately, fuzz target (SET-01, SET-06, NFR-07) | Items 3, 4, 7, 12 |
| 2. Makera Z1 profile, each value confirmed or unconfirmed | Item 8 |
| 3. Stock: cylinder, box, bounding box plus offset, material name (SET-03) | Item 9 |
| 4. Setup: part position, work coordinate system and offset reference (SET-05) | Items 1, 2, 9 |
| 5. Clearance: safe cylinder and safe Z, validated against stock and part (SET-06) | Items 5, 10 |
| 6. Undoable commands; CLI `validate` reports setup problems | Item 11 |
| 7. Frame conversion tests | Items 1, 6, and "Verification" |

## Decision

1. **Frames.** Three frames, one shared kinematics implementation (`openchuck_types::kinematics`, architecture section 5.1):
   - The **part (model) frame** is unchanged from decision 0004 item 5: fixed to the part, X along the rotary axis, origin on the axis. Placement steps (IMP-05) put the part in it, so they also centre it on the axis. The IR stays in this frame.
   - The **machine frame** is the machine's coordinates. The model origin lies on the rotary axis, `part_offset` along it from the profile's rotary reference point (`rotary_origin`).
   - The **work frame** is what a program is written in (G54 and so on). **(owner)** Its zero is always on the rotary axis, at model X = `work_offset.x`, so a program's Y0 Z0 is the axis. This is what the Z1 needs (decision 0007, OI-40), and it suits any 4-axis controller; a Y or Z offset of the work zero is not offered (OI-55).
   - A `RotaryFrame { axis, sense, origin }` describes where the model frame sits in a fixed frame at A = 0. `RotaryFrame::machine(profile, setup)` and `RotaryFrame::work(profile, setup)` build the two fixed frames of a setup, and `expected_work_offset` gives what the register should hold (the machine position of the work zero). The machine and work frames differ only by that translation, so the post (slice 6) writes work coordinates and travel checks use machine coordinates from the same maths.
2. **Part position (SET-05).** A setup holds a `SetupFrame`: `part_offset` (mm along the rotary axis from the rotary reference point to the model origin) and `work_offset` (`register`, G54 to G59, and `x`). Across the axis the part is placed by its placement steps (IMP-05); an angular position of the part at A = 0 is a rotation step about X. Nothing else is needed to position a part on a rotary axis.
3. **The rotary axis as a parameter (SET-02 later).** `RotaryAxis` gains `Y`. The kinematics map model directions to machine directions with a fixed rotation `B` (the identity for X; a quarter turn about Z for Y, so model X lies along machine Y and model Y along machine -X) before turning about the axis: `fixed = origin + B * R(sign * A) * tip`. Strategies keep working in a model frame whose X is the rotary axis, so SET-02 is a profile change. **`MachineProfile::validate` refuses Y** ("not supported yet (SET-02)") because SET-02 is P2; Y exists so the slice's checklist test with the rotary axis set to Y runs at unit level against the oracle.
4. **Profile fields (amends decision 0004 item 4 once accepted).** `rotary_origin` is now the machine coordinates of the **rotary reference point**, a point on the rotary axis such as the chuck face centre; across the axis it is still the rotary axis offset, and along the axis it is where `part_offset` is measured from. Decision 0004 defined it as the machine coordinates of the model origin, which is the special case `part_offset = 0`, so existing kinematics callers (`machine_xyz(&profile, …)`) keep their meaning through `AsRotaryFrame`. `Clearance` becomes a struct (item 5). `provenance` maps each field to `{ state = "confirmed" | "unconfirmed", source }` (item 8); a field not listed counts as unconfirmed. **(owner)** No spindle limit or post reference yet: SET-01 does not list the first, and the second belongs to slice 6 (GC-02).
5. **Clearance geometry (SET-06).** `Clearance { radius, safe_z }`, both measured from the rotary axis so they hold at every A: a safe cylinder that retracts and rapid links stay outside, and a safe Z (height above the axis, which is work Z since the work zero is on the axis) for large rotations, tool changes and the start and end of a program. Validation refuses non-positive values and a safe Z below the cylinder. **(owner)** The profile's clearance is the machine's default and each setup may set its own (`clearance` absent means the profile's): a fixed machine value cannot enclose every stock, and a setup's clearance is what is checked against its stock and part (work breakdown slice 3 item 5). The architecture's "machine profile ... clearance geometry" (section 5.1) still holds as the default.
6. **A-axis sign convention.** Unchanged from decisions 0001 and 0004: `ASense::Positive` turns the part right-handed about the positive rotary axis. For the Z1 that is derived, not confirmed (decision 0007, OI-03): at A +90 the side of the part that faced up faces machine -Y (the front). `openchuck-app::setup::a_direction` states this in words from the shared kinematics, the CLI prints it with the profile (the viewport shows it in slice 8), and a test on an asymmetric part checks the shared kinematics, the independent oracle and the statement agree.
7. **Profiles stored separately (SET-01); the project keeps a copy (OI-43).** A profile file is TOML with its own schema (`schema = 1`, profile under `[machine]`), read by `openchuck_project::machines` (`parse_profile`, `render_profile`, `load_profile`, `save_profile`, `list_library` for any folder of profile files). **(owner)** The project keeps a full copy of its profile in `machine.toml` (the architecture's "machine" file, section 5.6), not a name: the same project then gives the same G-code on any computer (GC-07), and editing a library profile never changes a project, as tool snapshots do for tools (TL-07). The `MachineRef` placeholder is removed. Reporting differences between the copy and a library profile is left for later (OI-54). **(owner)** Built-in profiles ship inside the program (`crates/openchuck-project/machines/`, read with `include_str!`), not in a new top-level folder; a default user library location needs a platform-folder crate that is not in the selection, so for now the CLI takes `--library <DIR>` (OI-53).
8. **The Makera Z1 profile** (`makera-z1`). *Amended by [decision 0012](0012-z1-profile-consensus.md): the values below were from single sources; 0012 replaces the travel, feeds and rotary reference with a consensus across Makera's profiles and the Z1 firmware (A max 3600 deg/min, X and Y 1200 and Z 600 mm/min, spans 200 x 200 x 100 mm).* As first built, from decision 0007, **every value unconfirmed** with its source: rotary axis X, `positive` sense (OI-03, air cut step 1), continuous A (firmware source), travel X -10..200, Y -200..10, Z 0..100 and feeds 4000 mm/min and 21600 deg/min from Makera's Fusion machine (OI-39; Makera's FreeCAD machine and Studio's table disagree; rapids are not stated, so the feed limits are used). The rotary reference point is a **placeholder** at the origin of Makera's Fusion machine frame, which appears to lie on the A axis; it must be measured on the machine (OI-39). The default clearance (radius 45 mm, safe Z 60 mm) is an OpenChuckCAM default, not a machine value. A setup check that uses an unconfirmed value says so in its message.
9. **Setups (SET-03, SET-05) in `openchuck-project`.** `Setup` gains `part` (the model machined, optional), `stock` (optional), `material` (optional plain name, never empty), `frame` (item 2) and `clearance` (optional, item 5). Stock is the existing `openchuck_types::stock::Stock`: a cylinder on the rotary axis (diameter, length, X start), a box (model-frame bounds) or the part's bounding box grown by an offset; `Stock::Imported` is SET-04 (P2) and is refused when a setup is edited or loaded. A model a setup uses as its part cannot be removed (as for selection sets). A setup file:

   ```toml
   schema = 3
   id = "…"
   order = "V"
   name = "Setup 1"
   part = "…"
   material = "oak"
   part_offset = 20.0

   [work_offset]
   register = "g54"
   x = -2.0

   [stock]
   type = "cylinder"
   diameter = 20.0
   length = 14.0
   x_start = -2.0

   [clearance]
   radius = 30.0
   safe_z = 40.0
   ```

   **Project schema 3**: `machine.toml` holds the profile copy and `project.toml` no longer names a machine; setups gain the fields above, all optional or defaulted. The 2 to 3 step drops a schema 2 `[machine] name`, which no front end could set and which cannot become a profile, so the machine is chosen again; a fixture (`tests/fixtures/schema-2/`) covers it.
10. **Setup checks (SET-06).** `openchuck_types::setup::check_setup` returns warnings in a fixed order, never errors, since the user may have reasons: no stock; bounding-box stock without a part; imported stock; the safe cylinder not enclosing the stock or the part; the part outside the stock; and, in machine coordinates, the stock (or part) ends and the work zero outside the travel along the axis, the rotary axis outside the travel across it, and the safe Z above or below the Z travel. Each warning lists the machine values it used that are unconfirmed. The part is measured by `openchuck_geom::part_extent` on the placed mesh (bounds and largest radius about the axis). **(owner)** No margin is added between clearance and stock; a margin is a later parameter (OP-14, slice 14) if wanted.
11. **Commands and CLI.** `Command::SetMachine` now carries a profile; setup edits use `Command::Replace`, so both are undoable (UI-08) and both are in the random undo property test. `Application::check_setups` imports each setup's part and runs the checks. The CLI gains `machines` (built-in and library profiles), `machine <project> [profile] [--library] [--clear]` (copy a profile in, print it with each value's source and the A direction) and `setup <project> [setup | --new NAME] …` (edit and print a setup with what the work offset register should hold and its checks); `validate` and `report` print setup warnings.
12. **Fuzzing (NFR-07).** A `machine_profile_toml` fuzz target reads any text as a profile file and checks that a profile that reads also writes and reads back the same; it runs for 1 minute on pull requests and 1 hour nightly, seeded from the built-in profiles. `project_files` also covers `machine.toml`. Property tests run the profile parser on random and damaged text in every test run. The `machine_profile_json` target stays for the JSON wire form.

## Alternatives considered

- **A work zero anywhere (Y and Z offsets too).** General, but the Z1's feed rule assumes Y0 Z0 on the axis (decision 0007), and with the zero on the axis the safe Z and every rotary move read the same at every A. It can be added as optional fields later (OI-55).
- **Keeping `rotary_origin` as the model origin's machine position, with no part offset.** Then moving the part along the axis would mean editing the machine profile. Rejected; the reinterpretation keeps the old meaning at part offset zero.
- **A project that names a library profile instead of copying it.** Smaller files, but a project would change when the library does and could not open on another computer with the same result (GC-07).
- **Clearance only in the profile** (the 0004 field as was). A fixed radius cannot suit both a 10 mm and a 60 mm stock; the profile keeps the default.
- **Clearance only in the setup.** Leaves a new setup with nothing to retract to; the profile default fills it.
- **Wrapping every profile value in a `{ value, state }` pair.** Every reader (strategies, post, simulator) would unwrap it; a side map keeps the plain data plain (INT-02).
- **Leaving `RotaryAxis` as X only.** The slice checklist asks for a unit-level Y test; the shared kinematics would then have to be tested through a test-only copy, which defeats the point.
- **Refusing setups whose clearance does not enclose the stock.** The work breakdown says "or the app warns", and the stock may be set later than the clearance.

## Verification

Tests named after the requirement IDs (decision 0004 item 14):

- **SET-01:** the built-in Z1 profile reads, names every field's source and marks all seven unconfirmed; the profile file round-trips byte for byte; a library folder lists good and broken files; the project's copy survives save and load, a one-value edit changes one line of `machine.toml`, and removing it removes the file; an invalid profile is refused by the project and on load; the A direction statement follows the sense; on an asymmetric part at A +90 the shared kinematics, the oracle and the statement agree with the documented Z1 convention; with the rotary axis set to Y the shared kinematics agree with an independent Y oracle for both senses within 1e-6 mm and a flipped sense is caught (`req_set_01_oracle_agrees_with_a_y_rotary_axis`).
- **SET-03:** all three stock types and the material survive save and load, and saving again rewrites nothing; stock bounds and radius; imported stock is refused.
- **SET-05:** the machine and work frames each agree with the oracle over 19 A angles from -1080 to 1234.5 degrees, machine = work + expected work offset everywhere, the work zero is Y0 Z0 at any A, and a wrong part offset is caught (`req_set_05_setup_frames_agree_with_oracle_and_each_other`); invalid setups (safe Z below the cylinder, a non-finite offset, a zero stock diameter, an empty material, an unknown part) are refused and leave the project unchanged; a part in use cannot be removed.
- **SET-06:** a clean setup has no warnings; a small clearance warns about stock and part; a part outside short stock warns; bounding-box stock without a part warns; travel along and across the axis and the safe Z are checked; warnings name unconfirmed machine values and stop naming them once confirmed; the CLI's `setup`, `validate` and `report` show the warnings and exit successfully.
- **UI-08:** the random undo property test includes setting the machine and editing setup part, stock, material, offsets, register and clearance.
- **PRJ-04:** the schema 2 fixture migrates (the machine name is dropped; setups get defaults); the schema 1 fixture migrates through both steps.
- **NFR-07:** damaged profile files (syntax, missing or newer schema, Y axis, negative feed, safe Z below the cylinder, an unknown sense, an unknown field) give messages naming the problem; parser property tests on random and damaged text. The fuzz target compiles; it was not run locally (no nightly toolchain in this environment), so its first run is the pull request's smoke job.

## Consequences

- No new dependency. No change to `requirements.md` or `architecture.md`. When this record is accepted it amends decision 0004 item 4 (`rotary_origin` is the rotary reference point, `Clearance` is a safe cylinder and a safe Z, `provenance` added) and item 5 (the base rotation `B` for a Y rotary axis), and settles OI-43 and OI-40.
- `openchuck-types`: `kinematics::RotaryFrame`, `setup` module, `machine` changes. `openchuck-geom`: `part_extent`. `openchuck-project`: `machines` module and built-in Z1 profile, setup fields, `machine.toml`, schema 3. `openchuck-app`: `setup` module, `check_setups`, `find_setup`. CLI: `machines`, `machine`, `setup`.
- The `openchuck-types` public API breaks (`Clearance` becomes a struct; `MachineProfile` gains `provenance`), so the workspace version goes from 0.1.0 to 0.2.0 and the change is recorded in the crate's changelog; `cargo semver-checks` against `main` reports exactly these two (INT-07).
- `fuzz/` gains `machine_profile_toml`; `ci.yml` and `nightly.yml` run it.
- The work breakdown's slice 3 checklist line "Every Z1 value is marked confirmed or unconfirmed" is met with every value unconfirmed; confirming them needs the air cut (OI-37) and a look at the owner's Z1 (OI-39).

## Open items

Tracked in [open-items.md](../open-items.md).

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 (OI-43) | How a project refers to its machine. **Settled** by item 7 when this record is accepted. | Slice 3 | Owner review |
| 2 (OI-40) | Map SET-05 and `rotary_origin` onto the Z1's G54 Y0 Z0 convention. **Settled** by items 1, 2 and 4 when this record is accepted. | Slice 3 | Owner review |
| 3 (OI-39) | Z1 limits, plus the machine coordinates of the A axis and chuck face (the rotary reference point is a placeholder) and the rapid feeds. | Before the Z1 profile is accepted | The owner's Z1 and Makera Studio |
| 4 (OI-53) | A default location for the user's machine library (a platform config folder needs a crate such as `directories`, not in the selection; NFR-14). Until then the CLI takes `--library`. | Slice 8 (the UI needs it) | An NFR-14 record, or a decision to use a folder beside the projects |
| 5 (OI-54) | Report differences between a project's machine copy and the library profile it came from (as TL-07 does for tools), and offer to update the copy. Needs the copy to record which library profile it came from. | Slice 4 (with the tool snapshot report) or slice 8 | |
| 6 (OI-55) | The work zero is always on the rotary axis. Add optional Y and Z offsets of the work zero if a controller other than the Z1 needs them. | When a need appears | User feedback |
