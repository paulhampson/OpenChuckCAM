# 0012: Makera Z1 profile values: consensus across Makera's published profiles

**Status:** Proposed (11 Oct 2026). Items 3 to 5 follow the owner's replies of 11 Oct 2026. It amends item 8 of [decision 0011](0011-slice-3-machine-stock-setup.md) (also proposed) and the limits row of [decision 0007](0007-sp-12-z1-conventions.md) (proposed). Nothing here is measured on a Z1. The owner confirmed four fields from use of the machine (11 Oct 2026): the rotary axis, the A direction, the A range and the feed limits, which are marked *confirmed*. The other three stay *unconfirmed*: the rotary reference point (OI-57), the travel frame and the default clearance. The air cut (OI-37) is kept open as a cross check on the direction.

## Context

Decision 0011 item 8 built the Z1 profile from one source per value: Makera's Fusion machine file for travel and feeds, and decision 0007 for the A direction. The owner asked for a consensus across several profiles instead of a single sample, and supplied one fact from Makera's website: the Z1 A axis turns at most **10 RPM** (3600 deg/min). Requirements: SET-01, SET-05, SET-06, OP-13, OP-15, GC-09 (conventions from existing Makera profiles). Open items: OI-03, OI-39, OI-40.

## Method

Every file that targets a Makera machine in `MakeraInc/CarveraProfiles` (at `de08a65`) was read, and the Z1 firmware's default configuration in `MakeraInc/MakeraZ1Firmware` (at `b3a2e26`) and the Carvera firmware's in `MakeraInc/CarveraFirmware` for comparison. Each source was classified before it was counted:

- **Machine-specific** values differ between machines, so they carry information about the Z1.
- **Template** values are identical across Makera's machines (Z1, Carvera, Air), so they carry none about the Z1 and are not counted towards a consensus.
- **Host-relative** values negate or flip an angle in the host program's own convention, which this analysis cannot read without that host's documentation.

| Source | Kind | Used for |
| --- | --- | --- |
| Fusion machines: Z1 4-axis and 3-axis, Carvera 4-axis, Carvera Harmonic 4-axis, Carvera Air 4-axis and 3-axis, Carvera 3-axis (zstd-compressed `simulation.mch` inside each `.f3d`) | One author's template family; four 4-axis files | Rotary axis, direction, reversal, range; Z1 travel |
| `Carvera.cps` (Fusion post) | Same family | Axis definition (`table: true, axis: [1, 0, 0]`) |
| FreeCAD machines Z1, C1, Air (`.fcm`) | Separate host | Axis, range, travel |
| Mastercam post | Separate host | Rotary axis, sign convention, limits |
| SolidCAM 3x and 4x GPP | Separate host | Rotary sign, rapid feed |
| Carveco 4-axis post, VCarve "Wrap X2A" post | Wrap hosts | Work origin; angle sign (host-relative) |
| Siemens NX post | `3_axis_mill`, with the generic NX defaults left in | Excluded: says nothing about the Z1 |
| DeskProto, GibbsCAM, LightBurn, kiri:moto | No rotary data, or a laser | Excluded |
| Z1 and Z1 Pro firmware default config (`configZ1.default`, `configZ1Pro.default`) | Machine-specific where it differs from Carvera and Air | Feed limits, A axis position, direction wiring |
| Carvera and Carvera Air firmware configs | Comparison only | Telling Z1 values from inherited ones |

## Findings

### A direction

| Source | Statement | Reads as |
| --- | --- | --- |
| Fusion, 4 four-axis machines (Z1, Carvera, Carvera Harmonic, Carvera Air) | Rotary axis direction `[1, 0, 0]`, convention `right-handed`, `reversed: false`; the post builds the axis as `table: true`, `axis: [1, 0, 0]` with no reversal | Positive A turns the part right-handed about +X |
| Mastercam | `rot_on_x = 1`; `rot_ccw_pos = 0` ("CW positive"), where the file defines the direction as "around the part ... viewed from the plus direction of the rotating axis" | The tool goes clockwise around the part, so the part turns counter-clockwise seen from +X: right-handed about +X |
| SolidCAM 4x | `cR1 = 'A'`, `iR1dir = 1  ;1=+CCW -1=+CW` | Counter-clockwise positive, which for a table rotary is right-handed about +X |
| FreeCAD, 3 machines | Joint axis `[1, 0, 0]`, role `table_rotary`; no reversal field | Right-handed by the default, not stated |
| Carveco 4-axis | `ROTARY_AXIS = Y`, A written as `-1.0 x` the host's angle | Host-relative; not counted |
| VCarve wrap | `ROTARY_WRAP_Y = "-A"` | Host-relative; not counted |
| NX | Fourth axis about Y, 0 to 360 | Generic 3-axis template; excluded |

**Consensus: positive A is a right-handed rotation of the part about +X** (`ASense::Positive`): the side facing up at A 0 faces machine -Y at A +90. Three host families state it, a fourth implies it, none contradicts it, and the two wrap hosts only invert their own sign, which neither supports nor refutes it. This agrees with decision 0007.

Limits of the evidence: the Fusion files are one author's template, so they count as one family however many files; the Mastercam and SolidCAM readings depend on how each documents "around the part" and "CCW", and they are consistent only under the table-versus-tool readings given above; none of it is a physical observation. The firmware does not settle it: the A direction pin is `delta_dir_pin 1.20!` on the Z1, the Z1 Pro and the Carvera (the Air uses `1.23!`), which shows the Z1 shares the Carvera's electrical convention, not which way the chuck turns. The air cut (OI-37, step 1) is still the only confirmation.

### A range and rotary reference

| Source | Statement |
| --- | --- |
| Fusion | No A limits; `reset: never` |
| FreeCAD | Limits plus or minus 1e7; `wrap_strategy: unwound` |
| Mastercam | `rot_type = 0`, signed continuous |
| Z1 firmware | No A soft limit |

**Consensus: continuous**, four of four.

**Work origin on the axis (OI-40).** Four hosts say the work zero belongs on the axis centre: Mastercam ("always set the work origin at the center of rotation"), Carveco (`ROTARY_ORIGIN_IS_CENTRE = YES`, "tool Z Zero ... set to the centre of the rotary axis"), VCarve ("set your Z to be the center of your stock") and the Fusion machine files (axis point Y and Z both 0 in the model frame). Carveco's post also starts with `G00 Y0` and repeats it after each tool change, putting the tool over the axis. The firmware's chuck routine sets the working Z to `rotation_offset_z` (23 mm) at a probed point 23 mm above the chuck centre. This supports the work zero on the axis that decision 0011 item 1 uses.

**Where the axis is in machine coordinates.** The firmware's chuck calibration (`ATCHandler.cpp`) moves to `G53 X = anchor1_x + rotation_offset_x - 3`, `Y = anchor1_y + rotation_offset_y` and probes there. The Z1 values (anchor1 -192.4, -194.3; offsets 12.0, 85.5) differ from the Carvera's and the Air's, so they are the Z1's own. They put the axis at **machine Y -108.8** (near the middle of a 200 mm travel) and the chuck reference at **machine X -180.4** (the exact datum, face or module reference, is not stated). No Makera source gives the machine Z of the axis. (The Fusion Z1 4-axis model sits the axis 45.0 mm above the table plane, from its head offset against the 3-axis model's, but that does not fix where the tool reaches in Z.)

### Travel

| Source | X | Y | Z | Frame |
| --- | --- | --- | --- | --- |
| Fusion Z1 (3-axis and 4-axis agree) | -10 to 200 (210) | -200 to 10 (210) | 0 to 100 (100) | Model frame |
| FreeCAD Z1 | 0 to 200 (200) | 0 to 200 (200) | 0 to 100 (100) | Positive from the low end |
| Z1 firmware soft limits (disabled by default) | min -206 (span 206) | min -206 (206) | min -102 (102) | Machine (G53), home at 0 |
| Z1 firmware `coordinate.worksize` | 300 | 200 | n/a | Inherited from the Air (300 x 200); contradicts both CAM profiles; **rejected** |

**Consensus: spans of 200 x 200 x 100 mm.** Z agrees across all three sources; X and Y agree within the margins each source adds (10 mm in Fusion, 6 mm in the firmware). The profile uses the nominal spans, the smallest the sources support, in the firmware's machine frame (home at 0, travel negative: X -200 to 0, Y -200 to 0, Z -100 to 0), because the Z1 post's footer is `G53 G0 Z-3` (decision 0007) and `G53` is that frame. With the rotary module attached and the soft limits enabled, the firmware also holds Y above -160 (`Robot.cpp`), which does not apply under the Z1 default of soft limits off.

The CAM profiles' frames cannot be merged: Fusion's origin is a model frame (its Y range -200 to 10 would put the axis at the end of the travel, which the firmware contradicts), and FreeCAD's is the low corner. Only the spans are comparable.

### Feed limits

| Source | X, Y (mm/min) | Z (mm/min) | A (deg/min) | Kind |
| --- | --- | --- | --- | --- |
| Makera website (supplied by the owner) | n/a | n/a | **3600** (10 RPM) | Published specification |
| Z1 and Z1 Pro firmware `alpha/beta/gamma/delta_max_rate` | **1200** | **600** | **3600** | Machine-specific: Carvera and Air use 3000, 2000 and 1800 |
| Makera Studio machine table (reported, decision 0007) | 1200 | n/a | n/a | Third party |
| Fusion, all machines | 4000 | 4000 | Z1 21600; Carvera 4000 | Template (the Z1, Carvera and Air are identical) |
| FreeCAD Z1, C1, Air | 6000 | 6000 | 4000 | Template |
| Mastercam | 10000 | 10000 | 2000 | Generic post defaults ("SET_BY_MD") |
| NX | 10000 | 10000 | `max_dpm` 10 | Generic NX defaults |
| SolidCAM | `nRapidfeed` 650 | 650 | n/a | Maximum feed for non-synchronised rapids |

**Consensus: X and Y 1200 mm/min, Z 600 mm/min, A 3600 deg/min.** Two independent machine-specific sources agree on A exactly (Makera's website and the Z1 firmware), and two on X and Y (the firmware and Makera Studio's table). The CAM profiles' numbers disagree with each other (A alone: 21600, 4000, 2000) and are the same on every machine, so they are not evidence about the Z1. The earlier profile's 21600 deg/min (60 RPM) was the Fusion template value and was six times the machine's limit.

The firmware enforces these rates per axis when it plans a move (`Robot.cpp`, rate scaling against each actuator's maximum), including G0, which `default_seek_rate 2000` cannot exceed. The profile therefore sets the rapid rates equal to the feed rates. These are the firmware's *default* configuration; a Z1 can carry a different `config.txt`, which is a reason to read the machine's own file (OI-39).

## Decision

1. **Z1 profile values** (amends decision 0011 item 8): rotary axis X; `positive` A sense; continuous A; travel X -200 to 0, Y -200 to 0, Z -100 to 0 in the firmware's machine frame; rotary reference point X -180.4, Y -108.8; max feeds 1200 mm/min linear, 3600 deg/min rotary, rapids equal. The default clearance stays an OpenChuckCAM default (radius 45 mm, safe Z 60 mm). The profile file records each value's evidence. The rotary axis, A direction, A range and feed limits are *confirmed* by the owner; the other three fields are *unconfirmed*.
2. **The machine Z of the axis is unknown (OI-57).** The profile keeps a placeholder of 0, which puts every Z1 setup's safe Z outside the Z travel, so the setup checks warn that the safe Z cannot be verified, naming `rotary_origin` as unconfirmed. This is deliberate: a silent pass on an invented number would be worse. The owner can measure the value (see below) and the warning goes away.
3. **Per-axis feed limits (OI-56, owner agreed).** `MaxFeeds` gains `z` and `rapid_z`; `linear` and `rapid_linear` are X and Y. The Z1 holds 1200 mm/min on X and Y, 600 on Z and 3600 deg/min on A, so a Z move is no longer planned faster than the machine will run it. This is a breaking change to `openchuck-types` already covered by the 0.2.0 version bump.
4. **The A direction stays as decision 0007 has it**, now with a stated consensus. No change to `ASense::Positive`, the oracle tests or the post conventions.
5. **Stock defaults (OI-39, owner's choice).** The built-in Z1 profile carries the stock values above. A user who changes the machine's limits creates their own profile; nothing reads the machine's `config.txt`. Four fields are marked confirmed (item 7); the others keep the unconfirmed mark until checked on a machine.
6. **Default work Z0 (owner's experience).** On a Z1 the default work Z0 is the chuck centre, which is how the work frame already places it (decision 0011 item 1). It is a work coordinate: it does not give the machine Z of the axis (OI-57), so the placeholder and its warning stay.
7. **Confirmed by the owner from use of the machine (11 Oct 2026):** the rotary axis (X), the A direction (positive is right-handed about +X), the A range (continuous) and the feed limits (X and Y 1200 mm/min, Z 600 mm/min, A 3600 deg/min). Each is marked `confirmed` in the profile. OI-37 (the air cut) stays open as a cross check on the direction, and the A direction text in the profile says so. The safe-Z check against the travel is kept, and the machine Z of the axis stays unconfirmed (OI-57). Unconfirmed: `rotary_origin`, `travel` and `clearance`.

## Alternatives considered

- **Keeping the Fusion values** (4000 and 21600). They are template defaults shared by every Makera machine, and the A figure is six times the manufacturer's published rate.
- **Taking the largest value across the sources** for feeds. That is the opposite of safe: it would adopt Mastercam's and NX's generic 10000 mm/min.
- **Averaging the CAM profiles' numbers.** The values are not samples of one quantity: most are placeholders that repeat across machines.
- **Using the firmware's `worksize` for travel.** It is the Air's 300 x 200, left in the Z1 config, and no CAM profile agrees.
- **Guessing the axis Z** from the Fusion model (45 mm above the table plane) to avoid the warning. The tool's reach below the spindle nose depends on tool and holder length, which no profile gives.
- **Counting Carveco's and VCarve's negated A** as evidence for or against the direction. Without their documentation the sign is relative to a convention this analysis cannot see.

## Consequences

- `crates/openchuck-project/machines/makera-z1.toml` is updated and its sources rewritten; tests that depended on the old values are updated. No schema change.
- Decision 0011 item 8 is amended by this record when both are accepted; decision 0007's limits row is superseded for feeds and travel (its direction and wrap findings stand).
- OI-39 and OI-56 close; OI-03 gains the consensus; OI-57 is new (below).

## Open items

Tracked in [open-items.md](../open-items.md).

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 (OI-39, closed) | Confirm the travel spans and limits. **Closed** by item 5: stock defaults. | Done | Owner's choice |
| 2 (OI-56, closed) | Per-axis feed limits. **Closed** by item 3. | Done | Owner's agreement |
| 3 (OI-57) | Machine Z of the A axis (and the X datum of the chuck reference): jog the tool tip to touch the centre of the chuck with a probe or gauge pin and read G53 Z, and note which point the X value refers to. Until then every Z1 setup warns that the safe Z cannot be checked. | Before the Z1 profile is accepted | The owner's Z1 |
| 4 (OI-03, OI-37) | The owner confirmed the A direction from use of the machine; the air cut stays open as a cross check. | SP-12 air cut | The Z1 |
