# 0410: Simultaneous finishing turns at a face beside an off-axis section round that section's own centre (OP-26, OP-04, OP-05, OP-12, GC-07, NFR-06, NFR-12; issue #439; lane CRANKEXCESS)

- **Status:** proposed (1 Nov 2026) by lane CRANKEXCESS. The project owner chose option B on 1 Nov 2026, relayed by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7; decision 0049): the face turn goes round the section's own centre, plus a note where a round-ended tool finishes such a corner. The owner did not want a warning to switch on Rings (item 4). The record awaits the owner's acceptance.
- **Date:** 1 Nov 2026.
- **Issue:** #439, "Investigate why the crankshaft still has excess material after the finishing improvements".
- **Builds on:** [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (one turn at each face across the axis), [0363](0363-simultaneous-finishing-off-axis-sections-walls-and-held-ends.md) (OP-26: off-axis sections driven round their own centre), [0368](0368-simultaneous-finishing-of-hidden-surfaces.md) (OP-20), [0390](0390-helical-finishing-in-rings.md) (rings), [0255](0255-bounded-tool-contact-checker.md) and [0277](0277-exact-ball-drops.md) (measurement on the exact faces).
- **Open items:** new OI-1540 (helical finishing, its Rings and rest machining still turn at such faces with the tool's line through the rotary axis) and OI-1541 (the corner note cannot tell a filleted corner from a sharp one, or a tab from the part). OI-1491 (rings for the simultaneous spiral) is informed by the findings below, not closed.

## Context

The owner's crankshaft project (the repository's `crankshaft.step` placed on the axis, outside the repository) still showed material after the recent finishing work. The owner asked why 4-axis simultaneous finishing does not get into the corners in the middle of the part (around X 10, 15 and 25), and whether a flat end could finish them.

The part has three round pins of radius 7.5 mm centred 10 mm off the rotary axis (X 0 to 10 and 30 to 40 on one side, X 15 to 25 on the other), joined by webs at X 10 to 15 and 25 to 30 whose faces lie across the axis. A web face meets a pin in a concave corner only on the pin's side facing the rotary axis. Everywhere else the pin runs into the web's round end.

**Measurement.** The lane regenerated the owner's project on `main` (0.31.0) and measured, at 159,475 points on the exact faces of the placed STEP part, the signed distance to the nearest tool surface over each operation's fed moves. The poses were interpolated on the straight machine move. The harness is outside the repository. Roughing left at least 0.294 mm everywhere (its stock to leave is 0.3), and finishing cut no deeper than 0.010 mm.

Material left within 0.5 mm of each inside corner, on the side facing the axis (median / largest, mm):

| Corner | Ball end, simultaneous | Flat end, simultaneous, stepover 2.2225 | Flat end, simultaneous, stepover 0.3 | Helical finishing, Rings, flat end | Rest machining |
| --- | --- | --- | --- | --- | --- |
| X 10 | 0.46 / 0.77 | 0.02 / 0.46 | 0.04 / 0.20 | 0.39 | 0.41 |
| X 15 | 0.46 / 0.81 | 0.44 / 1.15 | 0.14 / 0.23 | 0.42 | 0.42 |
| X 25 | 0.44 / 0.73 | 0.33 / 0.79 | 0.00 / 0.05 | 0.40 | 0.40 |
| X 30 | 0.47 / 0.75 | 0.42 / 1.05 | 0.03 / 0.08 | 0.39 | 0.38 |

The rest of the pins finished to within 0.003 mm (95th percentile), and the web faces away from the corners to within 0.06 mm.

**Causes.**

1. A ball end of radius r cannot reach into a square corner: it leaves up to r (√2 − 1) across it, 0.66 mm for the 3.175 mm ball. This is the tool's limit, not the strategy's.
2. A flat end can finish the corner. OP-26 drives the spiral round the pin's own centre, so the flat bottom lies on the pin. But the spiral clears the corner only where a pass's side lands on the face, so it leaves a strip up to one stepover wide. The owner's operation took its 2.2225 mm stepover from the tool's cutting data.
3. The face turns of decision 0297 are meant to fill that strip, but they drop the tool with its line through the rotary axis (`helical::turn_passes`). On a pin's side facing the axis, that line meets the pin up to about 60 degrees from its normal, so a flat bottom rides on its rim.
4. Helical finishing with Rings (decision 0390) and rest machining drop the same way and changed nothing in these corners.

## Decision

1. **The turn at a face beside an off-axis section goes round that section's own centre.** In simultaneous finishing, a face turn whose X lies in a zone whose section does not surround the rotary axis (OP-26) is cut as a track at that X round a whole turn of A. It uses the same drive towards the section's centre, the same axis plan (smoothing within the tilt band, no lead), and the same fit, move test and shank check as the spiral in that zone. The tool's axis stays on the section's normal all round, so a flat end's bottom lies on the section and its side on the face, into the corner. The turn's X, its start angle and its direction are as before.
2. **Elsewhere nothing changes.** A face turn in a zone on the axis is made by exactly the code it was before (`helical::turn_passes`), so a part whose every section surrounds the axis gives byte-identical output (GC-07, NFR-06). No parameter, schema or public API changes.
3. **A note for round-ended tools.** Where such a turn is made with a tool whose bottom is not flat, the operation's notes say, once: "at X 10.000, 15.000, 25.000 and 30.000 faces across the rotary axis meet a section that does not surround the axis in an inside corner: where a corner is sharper than the tool's radius, this tool's round end (radius 1.588 mm) leaves up to about 0.66 mm across it; a flat end can finish them". The X is the face's, a tool radius from the turn. The amount is r (√2 − 1) from the tool's cutting radius. The note promises nothing about the result, which the user still checks (NFR-12).
4. **No warning to switch on Rings (the owner's decision).** The measurements show that rings, as built for helical finishing, do not reach these corners: they drop with the tool's line through the axis, as the face turns did. A warning suggesting them would send the user the wrong way. Rings for the simultaneous spiral (OI-1491) would help only if, in an OP-26 zone, each ring went round the section's centre and a ring stood at the face. That finding was sent to lane RINGS-ALL (#440).

## Consequences

- **Output.** Simultaneous finishing changes only for parts with a face across the axis beside a section off the axis, such as the crankshaft. No golden program, reference job or committed project uses such a part, so none changes.
- **Measured on the owner's project after the change** (same harness, same settings, the flat end still at stepover 2.2225):
  - Inside corners, within 0.5 mm, flat end: at most 0.002 mm left at X 10, 15, 25 and 30 (before: median 0.02 to 0.44, up to 1.15).
  - Inside corners, ball end: at most 0.63 to 0.66 mm, its r (√2 − 1).
  - Pins: at most 0.003 mm (before: up to 0.93 at places near the corners). Web faces: at most 0.002 mm (before: up to 0.41).
  - End face X 0 beside tab 1: now at most 0.001 mm away from the tab (before: two sectors where roughing's 0.3 mm was left, because the turn through the axis rode on the tab).
  - Deepest cut unchanged at 0.010 mm, the tolerance.
  - What is left: under the tabs (by design, OP-21), and within one tool radius of tab 2 at X 40 (95th percentile 0.06 mm, at most 0.25 mm).
  - Moves: 5,540 instead of 5,737 for the flat end, and 20,518 instead of 20,437 for the ball. Regeneration time unchanged (about 120 s on 4 cores).
- **Version.** Changed output for such parts: the workspace minor version is bumped with a changelog entry, as for decision 0239 item 13, if the coordinator agrees on the timing.
- **Verification plan.** The OP-26 row's evidence gains the inside corners: on the crankshaft test piece, a flat end finishes the corners where the webs' faces meet the pins within tolerance, at a coarse stepover (proposed wording, to be agreed).
- **Tests** (`openchuck-path/tests/simultaneous_off_axis.rs`, from OP-26 and OP-04, written first and seen to fail):
  - `req_op_26_req_op_04_a_flat_end_finishes_the_inner_corners_of_the_crankshaft`: flat end 3.175 mm, stepover 2.2. Most left within 0.5 mm of each inside corner, over the 40 degrees of the pin facing the axis, ≤ 2 × tolerance; nothing cut in beyond the tolerance. Before: 0.83 mm at X 10. After: at most 0.0014 mm left and 0.0023 mm cut in, at all four corners. (The web joins its round ends by a waist about 7 mm wide, so the face beside the pin is narrow there; 40 degrees keeps the test's points on it.)
  - `req_op_26_req_op_04_face_turns_in_an_off_axis_zone_go_round_the_section_centre`: each turn covers 360 degrees with its tool lines within 0.1 mm of the pin's centre. Before: 8.93 mm. After: 0.015 mm.
  - `req_op_26_a_ball_end_notes_the_corner_it_cannot_finish`: the note once, with the four faces' X, 0.66 mm and "a flat end can finish". No note with a flat end.
  - `req_op_26_no_corner_note_where_every_section_surrounds_the_axis`: a stepped shaft on the axis gets no note.
  - The existing OP-26 tests (pins all round, the off-axis cylinder, 1/2/N threads) and the face tests of decision 0297 are unchanged and pass.

## Open items

- **OI-1540.** Helical finishing (spiral and Rings) and rest machining still turn at a face beside an off-axis section with the tool's line through the rotary axis, and leave the same corners (measured 0.38 to 0.42 mm median). The same change would apply to them through a section-centre drop, which they do not have yet.
- **OI-1541.** The corner note is made for every face turn in an off-axis zone with a round-ended tool. It cannot tell a corner with a fillet as large as the tool's radius (nothing left) from a sharp one. Nor, in a set-up, can it tell a tab or held region on that face (part of the drop surface) from the part.
