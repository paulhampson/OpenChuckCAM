# 0007: SP-12 result, Makera Z1 rotary conventions and G93 (desk part)

- **Status:** proposed. The conventions below are *derived* from Makera's profiles and the Z1 firmware source; none is confirmed on a Z1 until the attended air cut (OI-37) is run. The record is accepted, or amended with the measured values, after that.
- **Date:** 11 Oct 2026.
- **Spike branch:** `spike/sp-12-z1-conventions` (throwaway, never merged; full write-up in `spikes/sp12/RESULT.md`, air-cut procedure and record sheet in `spikes/sp12/AIR-CUT.md`).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-03 and OI-04 (progressed, still open) and OI-37 to OI-41 (new).

## Context

SP-12 (work breakdown, Spikes; a gating risk in finding 12) asks what rotary conventions the Makera Z1 uses and whether it accepts inverse-time feed (G93). Pass criteria: axis letter, direction, wrap, feed mode, header, footer and tool change documented, each marked confirmed, and the G93 answer recorded. It feeds slice 3 (Z1 profile) and slice 6 (posts). Requirements: GC-02, GC-04, GC-09, GC-10, OP-13, OP-15, SET-01, SET-05, SET-06, NFR-12. Verification plan policy 6 (the SP-12 bring-up exception), gate 4 and the feed-mode note in section 6. Decision 0001 (proposed) items 2 and 3, and its open items 3 and 4.

## Method and evidence

- **Makera's profiles** (GC-09): `MakeraInc/CarveraProfiles` at `de08a65`, which ships Z1 files: the Fusion machine definition `Makera Z1 4 axis.f3d`, the FreeCAD machine `Makera_Z1.fcm`, Makera's Fusion post `Carvera.cps` v1.4.6 (with a "Stock Air/Z1" option) and the FreeCAD post `makera_26_MCH_post.py`.
- **The Z1 firmware source**: `MakeraInc/MakeraZ1Firmware` at `b3a2e26` (version 1.1.2), a Smoothieware fork; compared with the stock Carvera firmware and the community firmware.
- **Test program**: hand-written `z1_conventions_test.cnc` and its LinuxCNC twin, checked by hand, by a transcription of the firmware's line splitting, and in LinuxCNC 2.9.10 (built from source; interpreter pass and headless trivkins XYZA simulator). No errors; every timed move within 0.02 s of the LinuxCNC rule.
- **Not done**: the air cut on a Z1 (needs the owner and the machine). The Makera wiki was not reachable from the spike environment.

## Findings

| Convention | Derived value |
| --- | --- |
| Axis letter, units | A, degrees |
| Orientation | Parallel to machine X, mounted on the Y table |
| Direction | Right-handed about +X (`ASense::Positive`): positive A carries the top of the part toward the front (-Y). The firmware's direction is factory configuration, not in the public source. |
| Rotary offset | Programs run in G54 with Y0 Z0 on the A axis (Makera's setup frame; the firmware's feed rule depends on it) |
| Range | Continuous; the firmware has no A soft limit. With the module enabled, Y's soft minimum becomes -160 mm. |
| Wrap and unwind | `G92.4 A0 S0` drops whole turns from A with no motion, keeping the sign. `G92.4 A<a> R0` (used by Makera's Fusion post) moves A and rewrites G54 to EEPROM. |
| G93 | **Not supported**: the stock Z1 firmware has no case for G93 (or G94) and ignores it silently, so an inverse-time F is then read as mm/min. |
| Feed on rotary moves | G94 with a Makera-specific rule: a move with XYZ and A takes max(L, ΔA·P/360)/F; an A-only move takes ΔA·min(P, 360)/360/F; P = 2πr + 30 mm with r the end point's distance from WCS Y0 Z0. LinuxCNC takes L/F (or ΔA/F for A only). |
| Header, footer | `G90 G21 G17`, `G54`; end with `M5`, `G53 G0 Z-3`, `G28`, `M2` (M30 does not end a program outside grbl mode) |
| Tool change | `M5`, then `T<n> M6`; no `G43` (the machine measures tool length) |
| Dialect | A `(` or `;` drops the rest of the line; each G or M word is its own command; no line numbers; 3 decimals |
| Limits | Fusion: 4000 mm/min linear, 21600 deg/min A, X -10..200, Y -200..10, Z 0..100. Other sources disagree (OI-39). |

Replayed under the Z1 rule, SP-3's G94 file (compensated for LinuxCNC) takes 142.8 s instead of 44.2 s, and its worst move 164 times too long. The F values the Z1 needs for the same move times range from 100 to 397 mm/min, against 2.4 to 241 in the file: the Z1 rule has no near-zero feeds.

## Decision (proposed)

1. **Z1 profile and post values** as in the findings table, each flagged *derived* until the air cut confirms it. A value the air cut does not confirm stays flagged in the profile and post (the spike's fallback).
2. **No G93 on the Z1.** The Z1 post definition never writes `G93`. G93 stays an optional post capability for controllers that support it (OP-13), as the LinuxCNC verification post does.
3. **The post converts feed by a named, per-controller rule** held in the post definition (GC-04, "feed mode for rotary moves"): `linuxcnc` for the verification post and `makera` for the Z1. The IR is unchanged: it keeps contact feed and duration (decision 0001 item 3); only the post's conversion from duration to F differs. Where the rule is computed (template or Rust stage) is open (OI-38).
4. **Unwinding on the Z1 uses `G92.4 A0 S0`**, never the `R` form.
5. **Z1 dialect rules** for the post definition: comments on their own line, no line numbers, `M2` to end, `M5` before `T<n> M6`.

## Alternatives considered

- **G93 on the Z1**: rejected; the stock firmware ignores it silently and the feeds would be read as mm/min. The community firmware supports G93, but stock firmware is the reference.
- **One compensated G94 rule for every controller** (decision 0001 item 3 as written): rejected by the firmware source; on the Z1 it runs rotary-heavy moves far too slow.
- **Makera's `G92.4 A<a> R0` for indexing**: rejected; it moves A the long way round in some cases and writes EEPROM on every use with G54 active.

## Consequences

- No edits to `requirements.md` or `architecture.md`: OP-13 (compensated feed by default, G93 optional), GC-04 (feed mode in the post) and the architecture's G93 assumption all hold. The architecture's description of compensated feed does not say the compensation is controller-specific; whether to add that sentence is for agreement when this record is accepted.
- Decision 0001 item 3 ("G94 compensated uses XYZ length over duration") describes the `linuxcnc` rule only; when this record is accepted it amends that item for the Z1.
- Slice 3: the Z1 profile from the findings table, with the work-offset convention (OI-40) and limits (OI-39) settled. Slice 6: the feed rule in the post (OI-38), the Z1 dialect rules, and the air-cut results for the gate 4 band.
- The LinuxCNC half of OI-04 is settled by the simulator run (F applies to XYZ length on mixed moves, degrees per minute on A-only moves; G93 accepted).
- No dependency enters the product; the spike's Python scripts and LinuxCNC config stay on the spike branch.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Run the air cut (`spikes/sp12/AIR-CUT.md`) and record the results: direction, wrap, feed rule, G93. (OI-37) | Before slice 3's Z1 profile is accepted | Project owner and the Z1 |
| 2 | Where the per-controller feed rule is computed: in the post template (SP-10's engine) or in the neutral machine program stage with the rule named by the post. (OI-38) | Slice 6 (with SP-10) | Air cut T3 and T4 results |
| 3 | Z1 limits: feeds and travel disagree between Makera's Fusion and FreeCAD files and Studio's table. (OI-39) | Slice 3 | The owner's Z1 and Makera Studio |
| 4 | Work offset: Z1 programs need G54 Y0 Z0 on the A axis; map SET-05 and the profile's `rotary_origin` onto that. (OI-40) | Slice 3 | |
| 5 | The tool change is not exercised by the air cut (no tool). (OI-41) | Slice 20 (gate 5), or an attended tool-change test before it | Project owner and the Z1 |
