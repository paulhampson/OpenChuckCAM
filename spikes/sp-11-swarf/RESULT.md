# SP-11 result: swarf (flank) machining

Throwaway spike code (work breakdown, Spikes; spike rules). The decision record is `docs/decisions/0021-sp-11-swarf.md` on branch `docs/decision-0021-sp-11-swarf` (open items OI-94 to OI-98); this file is the write-up and [`out/report.md`](out/report.md) is the full report.

## Question

Can ruled-surface detection and flank tool placement be made robust? Requirement OP-07; slice 17.

Pass criteria: (1) flank deviation within tolerance (0.01 mm, NFR-05) on ruled surfaces; (2) non-ruled surfaces refused.

## Method

A separate Cargo project with no third-party dependencies (`cargo test --release`, then `cargo run --release` for the report). Surfaces are plain `S(u, v)` evaluators, so the detector sees only points (the SP-9 evaluator shape). The part frame has A along X (SET-01).

| Module | What it does |
| --- | --- |
| `surface.rs` | Radial cylinder and cone, helicoid, coaxial cylinder, plane, sphere, torus, bicubic Bezier; normal by central differences |
| `detect.rs` | For each iso-line family: best-fit line (power iteration, fixed count, deterministic), worst residual, normal twist along the line; accept when a family fits within tolerance; refusal message |
| `place.rs` | Flank axis from the ruling `d` and the tool-side normal `n`: `axis = cos(a) d + sin(a) n`, centre `q + r (cos(a) n - sin(a) d)` (a is the taper half angle). Mode Ideal keeps that axis; mode Axis4 projects it into the YZ plane and sets A so the axis is vertical. Signed deviation to the tool's flank cone, part frame and machine frame. Union-of-poses check over the whole patch, not only along the rulings |
| `cases.rs` | The 12 test surfaces and tools |
| `tests/sp11.rs` | 15 tests named after the requirements |

The deviation check takes a 60 x 30 grid over the whole patch and the smallest signed distance over all poses (the union of the tool bodies), so twist off the ruling, pose spacing and the 4-axis tilt all show.

## Numbers

From `out/report.md`. Tool radius 3 mm, 91 poses unless stated, tolerance 0.01 mm.

**Detection.** Exact ruled surfaces (cylinder, cone, helicoid, coaxial cylinder, plane) fit with residual 0 (below 1e-9). The near-ruled spline with a 0.004 mm bump on the inner control points: residual 0.0019 mm, accepted. With 0.05 mm: residual 0.0234 mm, refused. Sphere (1.14 mm) and torus (1.40 mm) are refused with "not a ruled surface within tolerance: best straight-line fit misses by ...". The verdict is the same from 5 x 9 to 33 x 65 samples; the residual moves by about 12 % for the spline and for the sphere. When both families are ruled (plane) the `v` family is taken.

**Developable surfaces (cylinder, cone with a matching taper, plane).** Axis4 equals Ideal: tilt 0, no gouge, material left 0.0022 mm (cylinder), 0.0006 mm (cone) at 91 poses. Left falls with pose count roughly as the square (cylinder: 0.125 mm at 11 poses, 0.0093 at 41, 0.0007 at 161). The cone with a 6 degree half angle needs a 6 degree tool; A stays constant.

**Taper mismatch.** The same cone with a 4 degree tool: Ideal placement is still exact (gouge 0, left 0.0005 mm) but the vertical axis misses by up to 1.83 degrees (the two degrees of mismatch times cos u), giving 0.20 mm gouge and 0.19 mm left. With a straight tool: 5.5 degrees, 0.66 mm and 0.42 mm. The deviation check catches both.

**Ruled but not developable (helicoid).** The detector accepts it (residual 0) and the normal twist along the ruling shows it (3.0 degrees at pitch 1.5). The tool touches only along the ruling, so the surface to either side gouges it. At 641 poses with the straight 3 mm tool and 18 mm rulings:

| Pitch (mm/rad) | Twist (deg) | Gouge (mm) |
| --- | --- | --- |
| 0.3 | 0.61 | 0.0002 |
| 1.5 | 3.03 | 0.0042 |
| 3 | 5.89 | 0.0159 |
| 6 | 10.59 | 0.0515 |
| 12 | 15.21 | 0.1059 |

Gouge grows about with the square of the twist and crosses 0.01 mm between pitch 1.5 and 3. Material left on the helicoid is pose spacing only (0.0003 mm at 641 poses; 0.0098 at 161). Detection alone is not enough: the measured deviation is the gate.

**Four-axis reach.** The needed axis must be perpendicular to X for the tool to be vertical after rotation. A surface whose rulings run along X (the cylinder coaxial with A) has no solution: `AxisAlongA` (an error value, not a crash), while Ideal places it. For rulings with an X component `dx`, a straight tool is off by about asin(`dx`); a taper can absorb it only where `cos(a) dx + sin(a) nx = 0`.

**A.** Continuous with the unwrap (largest step 1.7 degrees at 91 poses on the helicoid, 153 degrees of travel; 0 on cylinder and matching cone). The ruling direction sign is chosen once at the first pose (smallest tilt), so A does not jump by pi.

**Determinism.** Poses and metrics are bit-identical over repeat runs (single thread; the spike has no parallel code).

## Decision

Both pass criteria are met without the fallback (analytic cylinders and cones only): see the decision record for what slice 17 should take from it, and the five open items it raises.

## Limits of the spike

Analytic and Bezier test surfaces only, no real STEP faces and no mesh input. The tool is an infinite cone: no tip, flute length, corner radius or holder. A direction and sign, A limits and the machine kinematics are the part-frame convention above, not the Z1's. Poses are at fixed parameter spacing, not chord-limited. No trimmed faces, no collision with neighbouring faces, no linking.
