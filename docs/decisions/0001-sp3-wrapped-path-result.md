# 0001: SP-3 result, wrapped path to G-code with A words

- **Status:** proposed. The IR and profile fields below are proposals for slice 0. The simulator criterion is outstanding, so this record is not accepted until the open items are closed.
- **Date:** 10 Oct 2026
- **Spike branch:** `spike/sp-3-wrapped-path-gcode` (throwaway, never merged; full write-up in `spikes/sp3/RESULT.md` on that branch)

## Context

SP-3 (work breakdown, Spikes) asked: does a wrapped engraving path on a cylinder become correct A-word G-code that replays in the LinuxCNC simulator? Its result feeds slice 0 (IR fields), slice 3 (A sign), slice 5 (feed approach, kinematic mapping) and slice 6 (post, replay). Requirements touched: OP-05, OP-06, OP-13, OP-15, GC-01, GC-02, GC-07, GC-08, GC-09, GC-10, NFR-05. Architecture sections 3 (IR, machine profile), 8 (post) and 9 (kinematic oracle). Verification plan 4.1 and the G-code gates. It also gives the first evidence for SP-5.

## Method and numbers

A sine path (amplitude 10 mm, 3 periods) wrapping 540 degrees round a 30 mm cylinder, 0.2 mm deep, contact feed 300 mm/min, rotary axis offset from the origin. Path to IR (model frame) to the shared machine model to a hard-coded LinuxCNC-style post (G94 compensated, G93, and a naive G94 baseline), read back with `gcode` 0.7.0, replayed through a separate polar-form oracle. 13 tests, 312 moves.

| Check | Result | Limit |
| --- | --- | --- |
| Shared model vs oracle, 17 A angles including negative, 0/360 and the seam | agree to 1e-9 mm | 1e-6 mm and 1e-6 degrees |
| A sign flipped in the model or in the post | oracle reports more than 1 mm error | must fail |
| Replayed node positions vs IR | 5.1e-5 mm (4-decimal output) | rounding only |
| Real motion between nodes vs intended curve | 0.0056 mm | 0.01 mm (NFR-05) |
| Cut time, replay vs IR, G94 compensated and G93 | within 0.01 % | |
| Cut time, naive raw feed | 37 % too short (1.6 times too fast) | shows compensation is needed |
| Wrap through the seam and unwind | A continuous (largest step 8.4 degrees); unwind keeps the same machine position | |
| Repeat run | byte-identical | GC-07 |

## Decision (proposed)

1. **IR fields (GC-01).** Per segment: tool tip and unit tool axis in the *model* frame, an explicit *unwrapped* A in degrees, move kind, contact feed, duration, and contact path length. Machine XYZ is derived only in the post from the machine profile. Unwinding needs no IR field: it is a rapid to the same model tip with A changed by whole turns. Contact length is added to the field list in slice 0 item 4.
2. **Machine profile fields (SET-01, SET-06).** The A sign and the rotary axis offset (Y and Z for rotation about X) are profile data. Neither is hard-coded in the engine or the post.
3. **Feed modes (OP-13, GC-04).** The IR keeps contact feed plus duration. The post chooses the mode per move. G94 compensated uses XYZ length over duration when XYZ moves and degrees per minute when only A moves; G93 uses the reciprocal of the move time. This stays unconfirmed for the Z1 (see open items).
4. **Chord tolerance.** Set on the unrolled curve. Half the toolpath tolerance gave 0.0056 mm measured, which is close to the 0.01 mm limit; strategies should use about 0.4 times the tolerance, or measure the real motion.
5. **Reader (SP-5 input).** `gcode` 0.7.0 parses every code the post emits, with comments and modal lines, but stores numbers as `f32`. Use it for structure and spans and re-read numbers as f64, or use a small in-house reader. Decide at slice 6.
6. **Oracle.** The independent oracle design is confirmed: polar form, no shared code, and it catches a flipped A sign. Slice 0 builds it as specified.

## Alternatives considered

- Machine-frame positions in the IR: rejected, it names a controller convention and the A sign would leak into strategies.
- Wrapped (0 to 360) A in the IR: rejected, it loses the continuity that unwinding and limit checks need.
- Using the `gcode` crate's values directly: rejected for the f32 precision; harmless below 100 mm at 4 decimals but unsafe against the 1e-6 limit.
- Naive G94 with the raw contact feed: measured and rejected (37 % time error).

## Consequences

- Work breakdown slice 0 item 4: add contact path length to the IR field list. Slice 6 design refinement: record the reader decision and the per-move feed mode choice. Verification plan 4.1: note the chord tolerance factor. Edits to `requirements.md` or `architecture.md` need agreement first; none are made here.
- Findings that limit what the replay proves:
  - LinuxCNC with trivkins treats A as a plain number, so the simulator confirms dialect, reached values and timing but **not** the A direction. Only the oracle and the SP-12 air cut confirm it.
  - Compensated G94 feed on mixed moves is fragile: F fell to 2.4 mm/min while A moved about 1160 degrees/min, near the sine peaks where the axial step goes to zero. It relies on a LinuxCNC feed rule that was not confirmed.
- Dependency record (NFR-14): `gcode` 0.7.0, pinned `=0.7.0`, survey score 75, MIT or Apache-2.0 (AGPL compatible). Spike-only until slice 6 decides.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | **LinuxCNC simulator run not done.** LinuxCNC could not be installed in the spike session (no reachable package source). SP-3's pass criterion is not met until `run_linuxcnc.sh` is run on a machine with LinuxCNC 2.9 or later and the printed positions and run times agree with the files. | Before slice 6 accepts any golden file | Someone with LinuxCNC |
| 2 | **No golden file with A words yet.** The spike's `out/*.ngc` are not golden files until item 1 and item 3 are done. | Slice 6 | Items 1 and 3 |
| 3 | **A sign and axis offset convention for the Z1.** The spike used sign +1 as a placeholder. | SP-12, slice 3 | Machine profiles and the air cut |
| 4 | **LinuxCNC feed rule on mixed XYZ and A moves**, and whether the Z1 accepts G93. Choose the per-move feed strategy, and whether to cap or switch mode where the linear step is near zero. | SP-12, slice 6 | Simulator run and Z1 test |
| 5 | **Reader choice:** `gcode` with an f64 layer, or an in-house reader for the subset our posts emit. | Slice 6 (SP-5 result) | SP-5 |
| 6 | **A limits and unwinding policy** (range limits, when to unwind, direction of unwind) was exercised with unlimited A only. | Slice 5 (OP-15) | A range in the machine profile |
| 7 | **Off-top contact and tilted tools.** The spike kept the tool vertical with the contact at the top. Tool axis variation and cutter shape (contact point offset from the tip) were not covered. | Slices 5 and 13, with SP-4 and SP-13 | |
| 8 | **Chord tolerance factor** (0.4 or 0.5 of the toolpath tolerance). | Slice 5 | Measure on the first strategy |
