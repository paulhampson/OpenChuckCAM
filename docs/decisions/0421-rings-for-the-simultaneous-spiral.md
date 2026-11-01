# 0421: Rings instead of a spiral for simultaneous finishing's spiral pattern (OP-05, OP-06, OP-11, OP-12, OP-14, OP-15, OP-20, OP-26, GC-07, NFR-06, NFR-07, NFR-12; issue #440; lane RINGS-ALL)

- **Status:** proposed by lane RINGS-ALL, 1 Nov 2026, for the owner to accept. Item 3 is the owner's choice D3 (option 1 of 3), relayed by the coordinator the same day.
- **Date:** 1 Nov 2026.
- **Issue:** #440, "Rings option for every strategy that currently cuts a spiral". Pull request 2 of the lane.
- **Builds on:** [0390](0390-helical-finishing-in-rings.md) (helical finishing in rings), [0420](0420-rings-for-rest-roughing-and-round-facing.md) (rings for rest machining, continuous roughing and round Facing), [0036](0036-slice-13-simultaneous-finishing.md) (simultaneous finishing: the tool axis policy and the smoothing of A), [0363](0363-simultaneous-finishing-off-axis-sections-walls-and-held-ends.md) (the section-centre drive, OP-26), [0368](0368-simultaneous-finishing-of-hidden-surfaces.md) (hidden surfaces, OP-20, and the shank), [0410](0410-face-turns-round-an-off-axis-section.md) (face turns round an off-axis section's centre, lane CRANKEXCESS), [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (a turn at each face across the axis).
- **Open items:** closes OI-1491. No new open item.

## Context

Decision 0420 gave the **Passes** setting to every strategy that cuts a spiral except simultaneous finishing, whose spiral smooths A within a tilt band and leans the tool by a lead angle (OI-1491). Lane CRANKEXCESS (#439, decision 0410) found that rings help the crankshaft's inside corners only when, in a zone whose section does not surround the rotary axis, the ring is driven round the section's own centre, and only with a ring where the tool touches each face across the axis. Its change drives the face turn that way, and this record reuses it.

## Decision

1. **The setting.** Simultaneous finishing takes the same `pass_shape` parameter (**Passes**: Spiral or Rings) as decisions 0390 and 0420, shown for the spiral pattern only; the axial pattern ignores it. Spiral is the default for new and saved operations, and an operation without the setting or with Spiral is made by exactly the code it was before, so its output is byte-identical.
2. **The rings.** The rings are at the places of `helical::ring_places` (0390 item 3): equal gaps no wider than the stepover from end to end, with a ring at each end and at each face across the axis, in X order. Each ring is one whole turn of the drive at one X, driven as the spiral is: the tool axis follows the surface normal with the lead the way the ring travels, A smoothed within the tilt band, and in a zone whose section does not surround the axis the drive aims at the section's own centre (OP-26). A ring at a face is the face's own turn as on the spiral: round the section's centre in such a zone (0410), with no lead, otherwise across the axis (0297). A turns the Rotation way, one whole turn a ring, so it builds up as with the spiral (OP-15).
3. **The steps (D3, option 1 of 3, the owner's choice, relayed).** Between two rings the tool steps along X with A held at the angle the ring ended at, driven at the angle the ring ended at, and the next ring starts at that A, so ring, step and ring are one cut where nothing splits them. Where holding that A would lean the tool more than the lead and the tilt from the surface normal (a section whose normal turns along X), the step is split there and the shared services link the pieces (OP-14). The step is split where the section changes zone, as the spiral is. Every move is checked against the surface as before (OP-12). Options 2 (the step follows the normal, with a short link at every ring) and 3 (separate rings joined by retract and entry) were not chosen.
4. **What is unchanged.** Hidden surfaces (OP-20) and the shank breaks (0368) are made after the main passes as before; the corner note of 0410, and the notes on the slope along X, the tilt used and the mesh, are made as before; a region, a boundary and the held bands split rings and steps as they split the spiral. The simultaneous spiral has no adaptive spacing (0239 gave it to the axial pattern), and rings add none.
5. **Notes.** A note gives the number of rings and how far apart they are.
6. **On a part that slopes along X** the tool sits a radius off its contact along the normal, so the tip of a ring moves a little along X as the normal turns, as the spiral's does; the drive and the contact are at the ring's X. On a mesh each ring's ends are at the smoothed normal of the facets there, so a ring turns a whole turn to within a quarter degree at each end; the path over all the rings builds up whole turns.
7. **Refused, not guessed (NFR-07).** Any other value is refused, naming **Passes**.
8. **NFR-12.** Nothing in the wording says the program is safe or checked; generated G-code remains the user's to verify before running on a machine.

## Documents

- No change to the requirements or the architecture.
- UI specification, section 8: the **Passes** entry covers simultaneous finishing's spiral pattern.
- Verification plan: "Rings instead of a spiral" gains the simultaneous cases.

## Consequences

- **Library:** one added public constant, `simultaneous::parameter::PASS_SHAPE` (equal to `helical::parameter::PASS_SHAPE`); no version bump. Inside the crate, `helical::join` is shared, and simultaneous finishing's axis plan can start a track at a held angle (`axis_plan_from`; the spiral passes none, so its plan is unchanged) and hold an angle along a step (`held_plan`).
- **Tests,** each written before the code: `crates/openchuck-path/tests/simultaneous_rings.rs` (the setting and its default, for the spiral pattern only; whole turns no wider apart than the stepover with no reversal; steps holding A, with ring, step and ring one cut with a lead of 0, 5 and -5 degrees; on a lobed part, steps holding A at two tilts; no gouge and no more material left than the spiral, graded with the test kit's gouge and coverage checks on a cone and a lobed part; the spiral unchanged and the axial pattern ignoring the setting; byte-identical at 1, 2 and N threads; a bad value, and a stepover too fine to cut (1e-20 and the smallest positive number), refused by name), `simultaneous_off_axis.rs` (on the CrankShaft model a flat end finishes the inner corners in rings as with the spiral) and `helical_faces.rs` (the CrankShaft's faces are cleaned in rings).
