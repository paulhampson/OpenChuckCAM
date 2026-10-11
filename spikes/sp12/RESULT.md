# SP-12 result: Makera Z1 rotary conventions and G93 (desk part)

Branch `spike/sp-12-z1-conventions`. Throwaway work in `spikes/sp12/`; not for merging into the product (spike rules).
Requirements touched: GC-02, GC-04, GC-09, GC-10, OP-13, OP-15, SET-01, SET-05, SET-06, NFR-12. Decision record 0007 (proposed) carries the decisions; open items OI-03, OI-04 and OI-37 to OI-41.

**State: desk part done, machine part outstanding.** The pass criterion asks for each convention to be marked *confirmed*. Nothing below is confirmed on a Z1 yet: values are *derived* from Makera's own profiles and firmware source, and the attended air cut in `AIR-CUT.md` is what confirms or refutes them. It needs you and the machine.

## Question

What rotary conventions does the Makera Z1 use, and does it accept inverse-time feed (G93)?

## Method

1. **Makera's machine profiles** (GC-09 says to derive the conventions from them): `MakeraInc/CarveraProfiles` at `de08a65` (5 Aug 2026), which ships Z1 files: `Makera Z1 4 axis.f3d` (Fusion machine definition; the `simulation.mch` JSON inside is zstd-compressed, extracted for the spike), `Freecad/26PostProcesssor_Machine/Makera_Z1.fcm`, the shared Fusion post `Carvera.cps` (v1.4.6, with a "Stock Air/Z1" tool-change option) and the FreeCAD post `makera_26_MCH_post.py`.
2. **The Z1 firmware source**: `MakeraInc/MakeraZ1Firmware` at `b3a2e26` (11 Aug 2026), release notes at version 1.1.2, a Smoothieware fork for the Z1 and Z1 Pro motion board. Read for the G-code dispatcher, the motion planner's feed rule, `G92.4`, `M2`/`M30`, `M600` and soft limits. Compared with the Carvera firmware (`MakeraInc/CarveraFirmware` `83c6691`) and the community firmware (`Carvera-Community/Carvera_Community_Firmware` `2c97111`, which added Z1 support and G93).
3. **Third-party evidence**, weighed lower: `blackveilprecision/z1-macros` `df62c3c` (scripts that write Makera Studio's Z1 dialect and were run on a Z1 Pro with firmware 1.1.2).
4. **Test program**: `z1_conventions_test.cnc`, hand-written in the Z1 dialect, and `linuxcnc/z1_conventions_test.ngc`, its LinuxCNC twin (same moves; `M600` becomes `M0`, and the Z1-only `G92.4 A0 S0` is emulated with `G92`). Checked by hand, by `z1_dialect_check.py` (the firmware's line splitting, transcribed), and in **LinuxCNC 2.9.10** built from source for this spike (the SP-3 session could not install it): an `rs274` interpreter pass and a headless simulator run with a trivkins XYZA config (`linuxcnc/`).
5. **Feed model**: `feed_model.py` transcribes the firmware's feed rule and the LinuxCNC rule, gives the expected times for the air cut, and replays SP-3's G94 file under both.

Not reached: `wiki.makera.com` (including its supported-codes page) is blocked by this environment's network policy.

## Conventions

| Convention | Value for the Z1 | Evidence | State |
| --- | --- | --- | --- |
| Axis letter | **A** | Fusion: rotary part named A, coordinate 0; FreeCAD: axis "A"; firmware: `A_AXIS` | derived; air cut step 1 |
| Units | Degrees | Fusion `units.angle = degrees`; firmware divides A distance by 360 | derived |
| Axis orientation | Parallel to machine X, mounted on the Y table | Fusion: `axis_of_rotation.direction = [1, 0, 0]`, parent Y; FreeCAD: joint `[1, 0, 0]`, parent Y, `table_rotary` | derived |
| Direction | **Right-handed about +X** (`ASense::Positive`): positive A carries the top of the part toward the front (-Y); seen from the +X end, counter-clockwise | Fusion: `conventions.rotation = right-handed`, rotary `reversed = false`; FreeCAD: no reversal. The firmware's direction comes from factory config not in the public source | derived, **unconfirmed**; air cut step 1 |
| Rotary offset | Programs run in G54 with **Y0 Z0 on the A axis** | Fusion: axis point (14, 0, 0) in the setup frame; the firmware's feed rule takes the radius from WCS Y and Z (below), which is only right with this origin | derived; air cut set-up |
| Range | **Continuous**: no A soft limit (soft limits cover X, Y, Z only); with the module enabled the firmware lowers Y's soft minimum to -160 | Firmware `append_milestone`; Fusion: no A limits, `reset = never`; FreeCAD: ±1e7, `wrap_strategy = unwound` | derived from source |
| Wrap and unwind | `G92.4 A0 S0` renumbers A by whole turns with **no motion**, keeping the sign (A370 → A10, A-370 → A-10, exactly ±360 unchanged). Makera's Fusion post uses `G92.4 A<a> R0` before indexed rotations, which **moves** A (by the raw difference of the two angles mod 360, not the shortest way) and rewrites G54 to EEPROM when G54 is active: our post should not use the R form | Firmware `G92.4` handler; `Carvera.cps` `positionABC` | derived; air cut step 2 |
| Feed mode | **G94 only**, with a Makera-specific rule (next section) | Firmware; Makera's Fusion post uses `FEED_DPM` with `DPM_COMBINATION`, never G93 | derived; air cut T3, T4 |
| **G93** | **Not supported.** The stock Z1 firmware (and the stock Carvera firmware) have no case for G93 or G94: both are **silently ignored**, so after `G93` an inverse-time F is read as mm/min. The community firmware added G93; stock firmware is the reference. | Firmware `Robot::on_gcode_received`; community firmware `Robot.cpp` | derived; air cut T5 |
| Header | `G90 G21 G17` (G94 is harmless but ignored), `G54` | Fusion `onOpen`; Makera Studio exports start `G90 G21` after an optional `;@MKR|` block that Studio uses for its preview | derived |
| Tool change | `M5`, then `T<n> M6` (stock Z1: any tool number; the machine measures tool length itself; no `G43`) | `Carvera.cps` "Stock Air/Z1" option; FreeCAD post: no G43, `M05` before tool change; z1-macros: `T1 M6` | derived; **not exercised by the air cut** (no tool) |
| Footer | `M5`, `G53 G0 Z-3` (machine Z top is 0), `G28` (clearance position), `M2`. Use **M2, not M30**: outside grbl mode the firmware treats M30 as "delete file" (a no-op without a file name) and does not end the program | Firmware `M30`/`M2` handling; Makera Studio exports end `G28`/`M02` | derived |
| Coolant | `M7` air on, `M9` off | Fusion post (`M400` first), Studio exports | derived |
| Comments and lines | A `(` or `;` drops **the rest of the line**, so comments go on their own line or at the end; each G or M word starts a new command (`G93 G1 X10 F6` is two commands); no line numbers | Firmware `GcodeDispatch` | derived from source |
| Number format | 3 decimals for X, Y, Z and A (Makera's Fusion post); the firmware parses single-precision floats, so keep A small by unwinding | `Carvera.cps`; firmware | derived |
| Limits | Linear 4000 mm/min, A 21600 deg/min (60 rpm), spindle 13000 rpm; travel X -10..200, Y -200..10, Z 0..100 | Fusion Z1 file. FreeCAD differs (X, Y 0..200); a third-party script cites 1200 mm/min "from Studio's machine table" | **sources disagree** (OI-39) |

## The Z1 feed rule

The firmware (`Robot::append_milestone`) does not apply F the way LinuxCNC does. With F in mm/min, L the XYZ length of the move, ΔA in degrees, and the "A perimeter" P = 2πr + 30 mm, where r is the distance of the move's end point from WCS Y0 Z0 (r ≤ 1 mm counts as 1 mm):

- **XYZ and A together:** the move takes **max(L, ΔA·P/360) / F**. When the A term is larger, the firmware slows the move so the A "surface speed" equals F.
- **A only:** the move takes **ΔA·min(P, 360)/360 / F**: F is treated as surface speed on the perimeter, but never slower than F degrees per minute.
- Then per-axis rate limits and acceleration apply.

LinuxCNC (trivkins) instead takes L/F when XYZ moves (A follows) and ΔA/F when only A moves. So **compensated G94 feed is controller-specific**: to get a move time T on the Z1 the post must write F = max(L, ΔA·P/360)/T (A only: ΔA·min(P, 360)/360/T), not the LinuxCNC value L/T.

| Check | Result |
| --- | --- |
| SP-3's G94 file (compensated for LinuxCNC), replayed under the Z1 rule | 142.8 s against 44.2 s intended: **3.2 times too slow**; worst move 164 times too slow (`X20.0030 A-405.7031 F2.423`) |
| F values in that file | 2.4 to 240.8 mm/min (the near-zero feeds decision 0001 warned about) |
| F the Z1 needs for the same move times | 100.0 to 396.9 mm/min: the Z1 rule has no near-zero feeds, because the A term keeps F up when the linear step vanishes |

Running too slow is the safe direction for an air cut, but it is a feed error for cutting (OP-13), and the rule is read from source, not measured: the air cut's T3 and T4 moves are chosen so the two rules give clearly different times.

## Simulator numbers (LinuxCNC 2.9.10)

`rs274` reads every line without error (`linuxcnc/out/rs274_canon.txt`). The headless run (`linuxcnc/out/sim_report.json`) reaches every commanded position, and the timed moves match the LinuxCNC rule:

| Move | Expected, LinuxCNC rule | Simulator | Expected, Z1 rule |
| --- | --- | --- | --- |
| T3a `G1 X60 F300` | 12.0 s | 11.98 s | 12.0 s |
| T3b `G1 A360 F600`, r = 30 | 36.0 s | 35.99 s | 21.8 s |
| T3c `G1 X20 A540 F600` | 2.0 s | 1.98 s | 10.9 s |
| T3d `G1 X60 A570 F300` | 12.0 s | 11.98 s | 12.0 s |
| T4 `G1 A570 F600`, r = 60 | 36.0 s | 35.98 s | 36.0 s |
| T5 `G93 G1 X10 F6` | 10.0 s | 9.98 s | 100 s (G93 ignored) |
| `G92` emulation of `G92.4 A0 S0` after A370 / A-370, then A0 | 10° back / 10° forward | 0.98 s each (10° at 600°/min) | same |

This confirms the LinuxCNC half of OI-04 (how LinuxCNC applies F to mixed XYZ and A moves, and that it accepts G93). The simulator cannot confirm anything about the Z1: trivkins treats A as a plain number, and the Z1's feed rule, direction and `G92.4` are its own.

## Decision (proposed, record 0007)

1. **Z1 profile values** as in the conventions table, each flagged *derived* until the air cut; the fallback for any value the air cut does not confirm is to keep it flagged in the profile and post (spike fallback).
2. **G93 is not offered for the Z1.** The Z1 post uses G94 only and never writes `G93`, because the stock firmware would silently misread its F words.
3. **The post chooses a feed rule per controller**, as data in the post definition (GC-04's "feed mode for rotary moves"): `linuxcnc` (L/T, or ΔA/T for A only) for the verification post, `makera` (the rule above) for the Z1. The IR keeps contact feed and duration (decision 0001 item 3 stands); only the post's conversion changes. The `makera` rule needs the radius of each end point from the WCS rotary axis, which the post has from the profile and work offset.
4. **Unwind on the Z1 with `G92.4 A0 S0`** (no motion), never the `R` form.
5. **Z1 dialect rules for the post**: comments on their own line, one G or M command per line where order matters, no line numbers, `M2` to end, `M5` before `T<n> M6`.

## Fallback status

Not triggered. If the air cut refutes the derived direction or feed rule, the measured behaviour replaces the derived value and stays flagged until a second run agrees.

## Limits of this spike

- Nothing is confirmed on a Z1 yet (the air cut is outstanding).
- The A direction depends on factory configuration that is not in the public firmware source; only the air cut settles it.
- The feed rule is read from the 1.1.2 source; a later firmware may change it. Record the firmware version with every trial.
- The tool change is not exercised (no tool in an air cut); the first attended cut with a tool, or a separate attended tool-change test, covers it.
- The Makera wiki was not reachable from the spike environment.
