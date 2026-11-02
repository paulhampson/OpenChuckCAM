# 0007: SP-12 result, Makera Z1 rotary conventions and G93

- **Status:** accepted (14 Oct 2026) by the project owner, after the attended air cut on a Z1 running firmware 1.1.2. The desk part was written on 11 Oct 2026 as a proposal; the air cut results are in the section *Air cut results* below. Every convention in the findings table is now confirmed on the machine, except the tool change, which the air cut does not exercise (OI-41).
- **Date:** 11 Oct 2026 (desk part); amended 14 Oct 2026 (air cut).
- **Spike branch:** `spike/sp-12-z1-conventions` (throwaway, never merged; full write-up in `spikes/sp12/RESULT.md`, air-cut procedure and record sheet in `spikes/sp12/AIR-CUT.md`).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-03 and OI-04 (settled by the air cut, apart from the machine Z of the axis, OI-60) and OI-37 to OI-41 (OI-37 to OI-40 closed; OI-41 open).

## Context

SP-12 (work breakdown, Spikes; a gating risk in finding 12) asks what rotary conventions the Makera Z1 uses and whether it accepts inverse-time feed (G93). Pass criteria: axis letter, direction, wrap, feed mode, header, footer and tool change documented, each marked confirmed, and the G93 answer recorded. It feeds slice 3 (Z1 profile) and slice 6 (posts). Requirements: GC-02, GC-04, GC-09, GC-10, OP-13, OP-15, SET-01, SET-05, SET-06, NFR-12. Verification plan policy 6 (the SP-12 bring-up exception), gate 4 and the feed-mode note in section 6. Decision 0001 (proposed) items 2 and 3, and its open items 3 and 4.

## Method and evidence

- **Makera's profiles** (GC-09): `MakeraInc/CarveraProfiles` at `de08a65`, which ships Z1 files: the Fusion machine definition `Makera Z1 4 axis.f3d`, the FreeCAD machine `Makera_Z1.fcm`, Makera's Fusion post `Carvera.cps` v1.4.6 (with a "Stock Air/Z1" option) and the FreeCAD post `makera_26_MCH_post.py`.
- **The Z1 firmware source**: `MakeraInc/MakeraZ1Firmware` at `b3a2e26` (version 1.1.2), a Smoothieware fork; compared with the stock Carvera firmware and the community firmware.
- **Test program**: hand-written `z1_conventions_test.cnc` and its LinuxCNC twin, checked by hand, by a transcription of the firmware's line splitting, and in LinuxCNC 2.9.10 (built from source; interpreter pass and headless trivkins XYZA simulator). No errors; every timed move within 0.02 s of the LinuxCNC rule.
- **Air cut**: run by the owner on a Z1 with the 4th-axis module, firmware 1.1.2, no tool and no material (verification policy 6, gate 4 rules); results below. The Makera wiki was not reachable from the spike environment.

## Air cut results (14 Oct 2026)

Run from `z1_conventions_test.cnc` on the spike branch, attended, firmware 1.1.2, G54 with Y0 Z0 on the A axis, stopwatch times. The controller accepted every line and showed no alarm, warning or message. Nothing moved in a way the procedure did not predict.

| Check | Expected | Measured | Result |
| --- | --- | --- | --- |
| `G1 A90`: which way the top moves | Toward the front (-Y) | Toward the front | Direction confirmed: right-handed about +X |
| `G1 A370`, then `G92.4 A0 S0` | No motion, DRO A10; `G1 A0` turns 10 degrees back | As expected; DRO A10 | Wrap confirmed |
| `G1 A-370`, then `G92.4 A0 S0` | No motion, DRO A-10; `G1 A0` turns 10 degrees forward | As expected; DRO A-10.001, mark matches | Wrap keeps the sign, confirmed. The DRO shows 0.001 degrees of float error, so tests compare with a tolerance |
| `G92.4 A0 S0` at A570 | DRO A210 | A210 | Confirmed |
| T3a `G1 X60 F300` | 12.0 s | 12 s | Confirmed |
| T3b `G1 A360 F600`, r = 30, A only | 21.8 s (36.0 s on the LinuxCNC rule) | 21.6 s | Z1 rule confirmed (-1.1 %) |
| T3c `G1 X20 A540 F600`, A term larger | 10.9 s (2.0 s on the LinuxCNC rule) | 10.9 s | Z1 rule confirmed |
| T3d `G1 X60 A570 F300`, X term larger | 12.0 s | 12 s | Confirmed |
| T4 `G1 A570 F600`, r = 60, A only | 36.0 s (the 360 mm cap on the perimeter) | 36 s | Cap confirmed |
| T5 `G93`, then `G1 X10 F6` | 100 s if G93 is ignored; 10.0 s if it works | 100 s, run to the end | **G93 is ignored: the Z1 does not support inverse-time feed** |
| Whole program | No alarm, every line accepted | No warnings or errors | Gate 4 conditions met for this program |

Both feed terms of the `makera` rule (the mixed XYZ and A form and the A-only form with its cap) match the measured times to within the stopwatch's resolution. The two rules are far apart on T3b and T3c (21.6 s against 36 s, and 10.9 s against 2 s), so the result does not depend on a close margin. The rule is measured on firmware 1.1.2 only; a later firmware may change it, so record the firmware version with every trial.

## Findings

The findings table gives the value for each convention. Each was derived from the sources above and is now **confirmed** by the air cut, except the tool change and the footer's clearance move (not exercised, OI-41) and the machine Z of the axis (OI-60).

| Convention | Derived value |
| --- | --- |
| Axis letter, units | A, degrees |
| Orientation | Parallel to machine X, mounted on the Y table |
| Direction | Right-handed about +X (`ASense::Positive`): positive A carries the top of the part toward the front (-Y). The firmware's direction is factory configuration, not in the public source. |
| Rotary offset | Programs run in G54 with Y0 Z0 on the A axis (Makera's setup frame; the firmware's feed rule depends on it). The measured times match the rule only with this origin, which supports it. The machine Z of the axis is still unmeasured (OI-60) |
| Range | Continuous; the firmware has no A soft limit. With the module enabled, Y's soft minimum becomes -160 mm. |
| Wrap and unwind | `G92.4 A0 S0` drops whole turns from A with no motion, keeping the sign. `G92.4 A<a> R0` (used by Makera's Fusion post) moves A and rewrites G54 to EEPROM. |
| G93 | **Not supported**: the stock Z1 firmware has no case for G93 (or G94) and ignores it silently, so an inverse-time F is then read as mm/min. |
| Feed on rotary moves | G94 with a Makera-specific rule: a move with XYZ and A takes max(L, ΔA·P/360)/F; an A-only move takes ΔA·min(P, 360)/360/F; P = 2πr + 30 mm with r the end point's distance from WCS Y0 Z0. LinuxCNC takes L/F (or ΔA/F for A only). |
| Header, footer | `G90 G21 G17`, `G54`; end with `M5`, `G53 G0 Z-3`, `G28`, `M2` (M30 does not end a program outside grbl mode) |
| Tool change | `M5`, then `T<n> M6`; no `G43` (the machine measures tool length). Not exercised by the air cut (OI-41) |
| Dialect | A `(` or `;` drops the rest of the line; each G or M word is its own command; no line numbers; 3 decimals |
| Limits | Fusion: 4000 mm/min linear, 21600 deg/min A, X -10..200, Y -200..10, Z 0..100. Other sources disagree (OI-39). *Superseded by [decision 0013](0013-z1-profile-consensus.md): the Fusion feed figures are template values shared by every Makera machine; the Z1 firmware gives 1200, 1200, 600 mm/min and 3600 deg/min.* |

Replayed under the Z1 rule, SP-3's G94 file (compensated for LinuxCNC) takes 142.8 s instead of 44.2 s, and its worst move 164 times too long. The F values the Z1 needs for the same move times range from 100 to 397 mm/min, against 2.4 to 241 in the file: the Z1 rule has no near-zero feeds.

## Decision

1. **Z1 profile and post values** as in the findings table. The air cut confirmed the direction, the wrap and unwind, the feed rule and the G93 answer, so those are marked confirmed. The tool change (`M5`, `T<n> M6`) and the footer's `G28` clearance move are not exercised by a tool-free air cut and stay as derived values (OI-41). The spike's fallback was not triggered.
2. **No G93 on the Z1, confirmed by the air cut (T5).** The Z1 post definition never writes `G93`. G93 stays an optional post capability for controllers that support it (OP-13), as the LinuxCNC verification post does.
3. **The post converts feed by a named, per-controller rule** held in the post definition (GC-04, "feed mode for rotary moves"): `linuxcnc` for the verification post and `makera` for the Z1, whose rule the air cut measured. The IR is unchanged: it keeps contact feed and duration (decision 0001 item 3); only the post's conversion from duration to F differs. The rule is computed in the neutral machine program stage ([decision 0019](0019-sp-10-template-engine.md) item 5, OI-38).
4. **Unwinding on the Z1 uses `G92.4 A0 S0`**, never the `R` form.
5. **Z1 dialect rules** for the post definition: comments on their own line, no line numbers, `M2` to end, `M5` before `T<n> M6`.

## Alternatives considered

- **G93 on the Z1**: rejected; the stock firmware ignores it silently and the feeds would be read as mm/min. The community firmware supports G93, but stock firmware is the reference.
- **One compensated G94 rule for every controller** (decision 0001 item 3 as written): rejected by the firmware source; on the Z1 it runs rotary-heavy moves far too slow.
- **Makera's `G92.4 A<a> R0` for indexing**: rejected; it moves A the long way round in some cases and writes EEPROM on every use with G54 active.

## Consequences

- Document edits agreed by the owner on 14 Oct 2026: the G93 "assumed unsupported until confirmed" wording in `requirements.md`, `architecture.md` and `verification-plan.md` now records the confirmation, and the architecture says the compensation is controller-specific. The requirements themselves (OP-13, GC-04) are unchanged and hold.
- Decision 0001 item 3 ("G94 compensated uses XYZ length over duration") describes the `linuxcnc` rule only; this record amends that item for the Z1, whose measured rule is the `makera` rule above.
- Slice 3: the Z1 profile from the findings table, with the work-offset convention (OI-40) and limits (OI-39) settled; its direction and A range now cite the air cut as well as the owner's use of the machine. Slice 6: the `makera` rule in the post, the Z1 dialect rules, and a replay check that the post's output takes the IR's predicted time under the Z1 rule to within about 1 %, the agreement the air cut showed.
- OI-04 is settled: the LinuxCNC half by the simulator run (F applies to XYZ length on mixed moves, degrees per minute on A-only moves; G93 accepted), the Z1 half by the air cut (the `makera` rule; G93 ignored).
- No dependency enters the product; the spike's Python scripts and LinuxCNC config stay on the spike branch.

## Open items for later decisions

Items 1 to 4 are closed (see the table's *State* column); item 5 stays open.

| # | Item | Decide in | Needs | State |
| --- | --- | --- | --- | --- |
| 1 | Run the air cut (`spikes/sp12/AIR-CUT.md`) and record the results: direction, wrap, feed rule, G93. (OI-37) | Before slice 3's Z1 profile is accepted | Project owner and the Z1 | Closed 14 Oct 2026: run, results above |
| 2 | Where the per-controller feed rule is computed: in the post template (SP-10's engine) or in the neutral machine program stage with the rule named by the post. (OI-38) | Slice 6 (with SP-10) | Air cut T3 and T4 results | Closed: decision 0019 item 5 chose Rust; the air cut measured the `makera` rule |
| 3 | Z1 limits: feeds and travel disagree between Makera's Fusion and FreeCAD files and Studio's table. (OI-39) | Slice 3 | The owner's Z1 and Makera Studio | Closed 11 Oct 2026 by decision 0013 |
| 4 | Work offset: Z1 programs need G54 Y0 Z0 on the A axis; map SET-05 and the profile's `rotary_origin` onto that. (OI-40) | Slice 3 | | Closed 11 Oct 2026 by decision 0011 |
| 5 | The tool change is not exercised by the air cut (no tool). (OI-41) | Slice 20 (gate 5), or an attended tool-change test before it | Project owner and the Z1 | Open |
