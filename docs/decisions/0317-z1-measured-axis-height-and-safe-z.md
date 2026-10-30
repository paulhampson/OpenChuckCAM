# 0317: The Makera Z1's axis height is the owner's probing result for the probe as the reference tool; the built-in safe Z and safe cylinder become 40 mm (lane Z1AXIS; OI-60)

- **Status:** proposed 30 Oct 2026 by lane Z1AXIS. The owner's answers to each choice below were relayed by the coordinator session (decision 0049) on 30 Oct 2026; the record itself is proposed until the owner accepts it.
- **What it amends:**
  - **[Decision 0013](0013-z1-profile-consensus.md) item 2** (accepted): "The machine Z of the axis is unknown (OI-60)". It is now the owner's measurement, −48.6 mm, confirmed (items 1 and 3). Item 1's default clearance is replaced (item 2).
  - **[Decision 0020](0020-slice-6-post-and-round-trip.md) item 11** (accepted): the Z1 profile's axis at machine Z −55 with the safe Z 50 mm above it. Both are replaced (items 1 and 2).

  SET-01, SET-06, UI-13 and NFR-12 are unchanged. The post, the path generation and the project and machine file formats are unchanged: no schema number is taken and no public API changes.
- **Date:** 30 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): narrows OI-60 to the X datum of the rotary reference point; updates OI-917 and OI-471; opens OI-1290 (the spindle-height warning for a tool longer than the probe, waiting on the probe's stick-out and on where it is stored).

Numbering: the coordinator gave lane Z1AXIS records 0317 to 0321 and OI-1290 to OI-1309. On 30 Oct 2026 none of them was used on `main` (f100c14) or on any pushed branch.

## Context

Gate 4 (step P8, [gate 4 and 5 instructions](../z1-gates-4-5-test-instructions.md) section 10) recorded the axis height as "−48.6 (after probing routine)". The G54 machine coordinates, the tool or probe and the tool length offset were not recorded. The built-in Z1 profile held a placeholder: machine Z −55, from the chuck centre 45 mm above a bed taken at the bottom of the Z travel. Its default safe Z was 50 mm above the axis, radius 45.

What the axis height does in the software (measured by changing it, lane Z1AXIS):

- The programs are written in work coordinates with G54 Y0 Z0 on the axis (decision 0011 item 1), so the axis height changes no motion word. The safe Z and the safe cylinder do: the first and last rapid of a program, and the retracts between passes.
- The setup check compares axis height + safe Z with the Z travel (SET-06). While the axis height is unconfirmed, every Z1 setup also says the safe Z cannot be checked against the Z travel.
- Path generation refuses a connection above the travel. At −48.6 with safe Z 50 the safe Z is machine Z +1.4, above the top of the travel (0): every Z1 operation failed to generate, and gate 1 reported `Z 50 is outside -51.4 to 48.6` on every committed Z1 golden program.
- The Z1 post's end block, `G53 G0 Z-3`, is fixed. Its check (facing) needs −3 − axis height ≥ safe Z.

**What the firmware's probing routine computes** (MakeraZ1Firmware v1.1.2, commit 3426fa8; v1.1.3 does not change it). These are facts read from the code:

- `ATCHandler::fill_zprobe_abs_scripts` (`src/modules/tools/atc/ATCHandler.cpp:407-475`) fits and calibrates the probe as tool 0. It then probes down at G53 X = anchor1_x + rotation_offset_x − 3, Y = anchor1_y + rotation_offset_y (line 434), and sends `G10 L20 P0 Z<rotation_offset_z>` (line 470).
- The Z1 configuration (`src/configZ1.default:405-417`) gives `rotation_offset_z 23.0`, "Rotation module Z Offset relative to chuck center".
- `G10 L20` with a Z word first clears the tool length offset and makes the current tool the reference tool (`src/modules/robot/Robot.cpp:576-579`; `ATCHandler.cpp:3183-3190`). Then it sets G54 Z so that the current position reads 23.
- A later tool's offset is its tool-setter reading minus the reference tool's (`ATCHandler.cpp:1909-1919`). Converting a work position to machine coordinates adds that offset (`Robot.cpp:498-506`).
- No constant in the firmware gives the machine Z of the probed point, of the chuck centre or of the axis.

So −48.6 is the machine Z of the axis **for the probe as the reference tool**. A tool that sticks out further than the probe raises the spindle by the difference at the same work Z. That is inference from the lines above: the setter is fixed and both readings are tip contacts.

## Decision

1. **Axis height (owner, D1, D3 and D7).** The built-in Z1's rotary reference point has Z −48.6. The field is marked **confirmed**, and its source says:
   - Z is the owner's probing result for the probe as the reference tool.
   - G54 X, Y and Z in machine coordinates, the tool length offset state and gate steps P8 and M4 were not recorded (OI-471).
   - Y is the firmware's probing position, not measured separately.
   - X is from the firmware configuration, and its datum (chuck face or module reference) is not stated (OI-60).

   The owner's words, relayed: "The −48.6 is after completing a probe with the Makera Z height probing routine. It touches off a specific point on the 4th axis. −48.6 is the measurement relative to the z home but is aligned to the 4th axis zero." The owner asked for the firmware's nominal. The firmware has none for machine Z (see Context), so the measured value is kept.

   Provenance is kept per field, so confirming the axis height confirms X and Y too. The owner accepted that rather than splitting the field, which would need a library API change and a schema bump (D7, option A).
2. **Default clearance (owner, D2).** Safe Z 40 mm and safe cylinder radius 40 mm, still an unconfirmed OpenChuckCAM default:
   - With the probe fitted, the spindle is at machine Z −8.6 at the safe Z.
   - The end block's `G53 G0 Z-3` is a 5.6 mm rise.
   - A stock wider than 80 mm in diameter is inside the safe cylinder, and the setup check warns (SET-06). The source text says so.
   - Every sample stock is at most 35 mm square or 27 mm in diameter.
3. **Saved copies (owner, D4).** A project's machine copy is the project's own (SET-01), so it is not migrated (as decision 0109 option C rejected). When the copy is the Z1 and still holds the old axis height −55, the setup checks and Messages say so. The copy counts as the Z1 by its library ID, or by its name when it has no ID, as in decision 0109. The text:

   "This project's Makera Z1 copy has the old placeholder axis height, machine Z -55 mm. The built-in Makera Z1 profile now has -48.6 mm, measured on one Z1 with Makera's probing routine. Choose the Makera Z1 again in the setup's machine section to update the copy, then check the axis height and the safe Z on your machine."

   A copy holding any other value is the user's own measurement and is not flagged.
4. **Machine inspector (UI-13).** For an unconfirmed axis height, the inspector now says how to measure it: run the controller's 4th-axis Z probing routine and read G54 Z in machine coordinates. It also says the value holds for the tool fitted then, the reference tool, and that a longer tool raises the spindle.
5. **Sample projects and golden programs (owner, D5 and its scope correction).**
   - The 11 sample machine copies (`testdata/projects/*/machine.toml`) take items 1 and 2.
   - The golden programs are blessed again; the pull request lists each file's diff. Only Z words of rapid moves change:
     - the first and last safe-Z rapids, Z50 to Z40;
     - the retracts to the safe cylinder between passes, Z45 to Z40;
     - two connection heights in `first_cut`, 0.001 and 0.265 mm lower.
   - No cutting move, feed or A word changes.
6. **Not built here: the spindle-height warning (OI-1290).** The owner asked for a warning when a tool's assumed stick-out (flute length + 5 mm) would take the spindle past the top of the Z travel at the safe Z. The formula is: spindle Z = axis height + safe Z + (tool stick-out − probe stick-out). The owner accepted it. The owner also asked for the text "The recommended minimum is 30 mm. This must be checked before machining.", shown on the Z1 when the safe Z that fits is under 30 mm. Two things are still open:
   - The probe's own stick-out is not known; the owner's first answer gave the probe's offset, which is 0 by definition.
   - Whether it and the 30 mm minimum become profile fields (a library API and schema change) or constants is not decided.

## Consequences

- `crates/openchuck-project/machines/makera-z1.toml`: the axis height, the clearance and their provenance.
- `openchuck-project` `machines.rs`: `has_old_z1_axis_height`.
- `openchuck-app`: the overview's setup check and message; the machine view's measuring text.
- The Z1 documents (air-cut procedure, safe practice checklist, gate 4 and 5 instructions, reference jobs) quote −48.6, the safe Z 40 and the reference tool.
- **Gate evidence.** The programs run at gates 4 and 5 (build dc610e26) no longer equal the committed golden files. The new programs have not been air-cut.
- **Block-rate program.** The pinned block-rate program of decision 0232 (69d532f) is unchanged and still moves at work Z50 and Z55. With the probe as the reference, that is machine Z +1.4 and +6.4 plus the fitted tool's offset; the gate 4 run of it raised no alarm. It is not regenerated here, and a later trial should check it (OI-1290).

## Tests

Each new test was written before the code it tests and failed first, except where noted:

- `openchuck-project` `machines.rs`:
  - `req_set_01_req_set_06_the_z1_axis_height_is_measured_and_its_safe_z_is_in_the_travel`: the values, the confirmation, the source's facts, and the safe Z inside the travel.
  - `req_set_01_req_prj_04_a_z1_copy_with_the_old_axis_height_is_recognised`: by ID, by name, another ID, a renamed copy, a user's own value.
  - `req_set_01_builtin_z1_reads_and_marks_every_value` is updated: the axis height is confirmed.
- `openchuck-types` `setup.rs` `req_set_06_the_safe_z_is_checked_against_the_top_of_the_z_travel`: the boundary with the Z1's figures (40 and 48.6 pass, 50 warns at machine Z 1.400). It characterises existing behaviour and passed when written.
- `openchuck-app` `machine_view.rs`:
  - `req_ui_13_each_value_says_whether_it_is_confirmed_its_source_and_how_to_confirm` is updated.
  - `req_ui_13_an_unconfirmed_axis_height_says_how_to_measure_it` is new.
- `openchuck-app/tests/overview.rs` `req_set_01_req_ui_13_an_old_z1_copy_warns_and_the_measured_z1_does_not`. It was written before the overview change but first run after it.
- The golden suites (reference jobs, first cut, blisk, the golden audit, the command-line end-to-end tests) compare against the blessed files at 1, 2 and N threads (GC-07, NFR-06).
