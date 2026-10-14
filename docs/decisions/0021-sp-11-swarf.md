# 0021: SP-11 result, swarf: line-fit detection plus a measured deviation gate

- **Status:** accepted by the project owner (14 Oct 2026), including items 1 to 7 and the resolutions of OI-94 to OI-96. The spike met both pass criteria without the fallback. OI-97 and OI-98 stay open for slice 17.
- **Date:** 13 Oct 2026.
- **Spike branch:** `spike/sp-11-swarf` (throwaway, never merged). Code in `spikes/sp-11-swarf/`, a separate Cargo project with an empty `[workspace]`; the write-up is `spikes/sp-11-swarf/RESULT.md` on that branch, and the full report is `out/report.md` beside it.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-94 to OI-98 are new.

## Context

SP-11 (work breakdown, Spikes, 3 days) asks: can ruled-surface detection and flank tool placement be made robust? Pass criteria: deviation within tolerance on ruled surfaces; non-ruled surfaces refused. Fallback: restrict P1 swarf to analytic ruled surfaces (cylinders and cones). It runs before slice 17.

Requirement: OP-07 (P1). Related: OP-06, OP-12, OP-15, NFR-05 (0.01 mm), NFR-06, NFR-07. Architecture section 4 (continuous family) and the analytic oracles row (ruled surface for swarf). Decision 0017 (exact surfaces: the evaluators the detector would read), 0012 (revolved-profile tool model) and 0018 (A-axis management).

## Method

A dependency-free spike on parametric surfaces `S(u, v)`: cylinder and cone with radial axes, helicoid, coaxial cylinder, plane, sphere, torus and bicubic Bezier patches with a controlled bend. Detection fits a straight line to the samples of each iso-line family and reports the worst residual and how far the normal turns along the line. Placement builds the tool axis from the ruling `d` and the tool-side normal `n` as `cos(a) d + sin(a) n` (taper half angle `a`), either as is (Ideal, a 5-axis reference) or projected into the YZ plane with A chosen to make it vertical (Axis4, the XYZA case). The deviation check is independent of the placement: signed distance to the tool's flank cone, over a grid on the whole patch, against the union of all poses. 15 tests.

## Results

- **Detection.** Exact ruled surfaces (cylinder, cone, helicoid, coaxial cylinder, plane) fit with residual under 1e-9 mm. A near-ruled spline with a 0.004 mm bump fits to 0.0019 mm and is accepted; with 0.05 mm it misses by 0.0234 mm and is refused. Sphere and torus miss by 1.1 and 1.4 mm and are refused with a message. The verdict is the same from 5 x 9 to 33 x 65 samples.
- **Developable surfaces** (cylinder, cone with a matching taper, plane): no gouge, material left 0.0006 to 0.0022 mm at 91 poses, falling with the square of the pose count. Axis4 equals Ideal. Pass criterion 1 holds.
- **Ruled is not enough.** The helicoid is exactly ruled but not developable: the tool touches along the ruling and gouges either side. Gouge grows about with the square of the normal twist along the ruling (0.0042 mm at 3.0 degrees, 0.0159 mm at 5.9 degrees; tool radius 3 mm, rulings 18 mm). The detector's residual says nothing about this; the twist and the measured deviation do.
- **Four axes limit what can be flank cut.** The needed tool axis must be perpendicular to X. A cone with a 6 degree half angle cut with a 4 degree tool is off by up to 1.83 degrees and gouges 0.20 mm; with a straight tool 5.5 degrees and 0.66 mm. A surface whose rulings run along X (a cylinder coaxial with A) has no solution, reported as an error value. Rulings perpendicular to the A axis (the cylinder, the helicoid) are the natural fit for a straight tool.
- **A** is continuous with an unwrap, provided the ruling direction sign is chosen once per path (smallest tilt at the first pose).
- **Determinism:** bit-identical over repeat runs.

## Decision (accepted)

1. **Both criteria are met; the fallback (analytic cylinders and cones only) is not adopted.** The detector reads point samples, so it applies to any face the SP-9 evaluators can sample, including B-spline faces.
2. **Detection is a straight-line fit per iso-line family** (best-fit line by a fixed number of power iterations, worst residual). A face is a swarf candidate when one family fits within the operation's tolerance; the family with the smaller residual is used, `v` on a tie. A face that fits in neither is refused with a message that names the residual and the limit (slice 17 item 4).
3. **The measured deviation is the gate, not the fit.** After placement the operation runs the union-of-poses check on the face and warns or refuses when gouge exceeds the tolerance (OP-12) or material left exceeds the stock-to-leave allowance (OI-95). The normal twist along the ruling is shown to the user as the early warning for non-developable faces.
4. **Placement:** axis `cos(a) d + sin(a) n`, centre `q + r (cos(a) n - sin(a) d)`, ruling direction sign decided once per path, A unwrapped. The 4-axis tilt (angle between the ideal axis and the vertical-after-rotation axis) is reported per pose, and an operation whose tilt makes the deviation check fail says so by name, with the tool taper that would remove it where one exists (cone half angle equals tool half angle).
5. **Straight and tapered tools use one model:** the flank is a cone, straight for half angle 0. A V-bit flank, a tapered ball's cone and the side of a flat or ball are the same case. Tip, corner radius and flute length are not modelled (OI-97).
6. **Pose spacing is chosen from the tolerance and the largest ruling radius,** as for other continuous passes (decision 0018 item 12), not a fixed count: material left fell from 0.125 mm at 11 poses to 0.0007 mm at 161 on the cylinder.
7. **Spike code and fixtures kept for slice 17:** the surface cases and the independent flank-cone deviation check, as the swarf analytic oracle (architecture section 10).

## Alternatives considered

- **Analytic cylinders and cones only (the fallback):** not needed; the line fit handles the spline case and costs nothing extra.
- **Accept on the fit residual alone:** wrong; the helicoid passes with residual 0 and gouges 0.016 mm at 5.9 degrees of twist.
- **Fit a developable surface and project onto it:** not tried; the measured gate is simpler and meets the criteria.
- **Tilt-and-lead of the axis to absorb the 4-axis mismatch:** not tried; OP-06 limits apply and the mismatch grows quickly (0.2 mm for 2 degrees on a 15 mm ruling).

## Consequences

- **Slice 17:** takes items 2 to 6, and its checklist line "A clearly non-ruled surface is refused" is covered by the sphere and torus cases. Items 1 to 4 of the slice stay as written; the "support for straight and tapered tools" item gains the matching-taper finding.
- **Work breakdown:** SP-11 Status updated in this change. No edit to `requirements.md` or `architecture.md`.
- **Dependencies:** none; the spike used none.

## Open items

- **OI-94** (resolved): a ruled face the four axes cannot reach is refused with a message that suggests the matching taper where one exists. Coaxial faces are always refused, since no taper helps. Slice 17 builds it.
- **OI-95** (resolved): gouge is checked against the toolpath tolerance (0.01 mm); material left against the stock-to-leave allowance. Pose spacing (item 6) is set so the leftover meets that allowance.
- **OI-96** (resolved): swarf is B-rep only for P1. The UI says so for mesh selections. Recovering a ruled patch from a mesh is separate work, not planned.
- **OI-97**: tool detail the spike left out: tip position and engaged flute length against the ruling length, corner radius on ball and bull-nose tools (flank contact must sit above it), and the holder (OP-12). Slice 17 design refinement.
- **OI-98**: machine conventions. The spike uses rotation about X and a unit-vector axis, not the Z1's A direction, range or wrap limits (decisions 0007 and 0013). Slice 17 reuses the slice 5 A-axis management and checks the sign against the kinematic oracle.
