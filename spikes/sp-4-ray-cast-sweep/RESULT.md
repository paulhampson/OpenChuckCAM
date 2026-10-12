# SP-4 result: ray-cast sweep around the A axis

Throwaway spike code (work breakdown, Spikes; spike rules). The decision record is `docs/decisions/0014-sp-4-ray-cast-sweep.md` on branch `docs/decision-0014-sp-4-ray-cast-sweep` (open items OI-61 to OI-64; OI-07 progresses). This file is the write-up and [`out/report.md`](out/report.md) is the full report.

## Question

Is continuous 4-axis feasible with `parry3d-f64` ray casts swept around the A axis? Pass criteria: (1) accuracy within 0.01 mm; (2) a runtime figure for the reference part; (3) identical results at 1 and N threads.

## Method

A separate Cargo project (`parry3d-f64 =0.31.1`, default features, nothing else; `cargo test --release`, then `cargo run --release` for the report, about 10 minutes).

| Module | What it does |
| --- | --- |
| `part.rs` | Analytic reference part, 80 mm long: `r(x, th) = 14 + 3 sin(2 pi x / 40) + a2 cos(3 th)`, with exact points and normals. `vase()` has no lobes (a2 = 0); `lobed()` has 1.5 mm lobes, so the normal leaves the radial plane |
| `mesh.rs` | Inscribed tessellation (vertices on the surface) into a parry `TriMesh` with its BVH |
| `tool.rs` | Flat and ball tools, 3 mm radius, as the lower envelope `h(r)` (decision 0012 item 5) |
| `sweep.rs` | The solving methods and the two sweeps below |
| `exact.rs` | Reference answers on the analytic surface, with no mesh: dense drop of a tool along a line, and point-to-surface distance |
| `par.rs` | Deterministic parallel map with std threads (from SP-13) |
| `tests/sp4.rs` | 9 tests, named after the requirements |

"Step A, cast along X" is read as: at each A, rays perpendicular to the A axis at stations along X. In the part frame the tool approach direction at angle `psi` is `u(psi)`; with a vertical tool the machine A equals `psi` (the sign is the spike's own, not the Z1's; OI-03).

Methods (each returns the tip height along the tool axis line and the contact point):

- **Stencil**: parallel rays over the tool disc (rings 0.75 mm apart, 66 rays), tip = max over rays of (hit height minus `h(d)`). Valid for any profile. Coarse, or refined by a pattern search.
- **Climb**: one central ray, then the same pattern search from it (step halves from half the radius down to 0.02 mm). About 100 rays.
- **Distance**: ball only; bisection on the distance from the mesh along the axis line (about 50 `distance_to_local_point` queries).
- **Normal offset**: ball only; one ray, centre = hit point + radius times the facet normal.

Sweeps: **through-axis** (the tool axis line passes through the A axis at angle `th`; the machine Y stays 0) and **normal-following** (a central ray finds the hit point and facet normal; the tool axis angle is the normal's angle in the plane perpendicular to X, the best A can do; the tool is dropped along that line through the hit point, so machine Y is not 0).

## Numbers

From `out/report.md` (release build, 4 threads available in the session container).

**1. Accuracy** (mesh chord tolerance parameter 0.001, 1000 poses per row, error = mesh result minus the exact drop on the analytic surface along the same line; negative means deeper than the surface):

| Method | Vase, ball | Lobed, ball | Lobed, flat |
| --- | --- | --- | --- |
| Stencil, coarse | -0.054 to -0.001 | -0.055 to -0.001 | -0.018 to 0.001 |
| Stencil, refined | -0.002 to 0.001 | -0.003 to 0.001 | -0.003 to 0.001 |
| Climb | -0.002 to 0.001 | -0.003 to 0.001 | -0.003 to 0.001 |
| Distance | -0.002 to 0.001 | -0.003 to 0.001 | n/a |
| Normal offset (centre distance from surface minus radius) | -0.002 to 0.001 | -0.002 to 0.001 | n/a |

Both modes give the same ranges. The error is the mesh's own: with the chord tolerance parameter at 0.001, the mesh sits up to 0.0025 mm inside the lobed part (the parameter underestimates the real error by about 2.5 times); at 0.004 it is 0.0105 mm, past the pass limit.

**2. Runtime**, reference part (80 mm, 0.1 mm X stations, 0.5 degree A steps: about 573,000 poses), 3 mm tool:

| Mesh | Method | 1 thread | 4 threads | Rays per pose |
| --- | --- | --- | --- | --- |
| 148,470 triangles | Climb, ball | 33.6 s | 9.5 s | 99 |
| 148,470 triangles | Climb, flat | 36.7 s | 10.0 s | 108 |
| 148,470 triangles | Refined stencil, ball | 47.9 s | 13.8 s | 136 |
| 148,470 triangles | Distance, ball | 67.0 s | 20.0 s | 53 queries |
| 984,060 triangles | Climb, ball | 39.4 s | 10.8 s | 99 |
| 148,470 triangles | Refined stencil, flat, normal-following | 66.2 s | 18.6 s | 196 |

A single ray costs 1.2 to 2.7 microseconds from 37,000 to 984,000 triangles; BVH build is 0.03 to 0.7 s. The count falls short of 576,000 because stations past the end of the part (X above 80 mm) miss.

**3. Threads.** Bit-identical pose hashes at 1, 2, 3, 4 and 7 threads for every method in both modes, and in the report's 1-versus-4 runs on the reference part. Repeat runs are identical.

**Tool angle** (normal-following, lobed part, 4,800 poses): the mesh's in-plane normal angle is within -1.18 to +1.08 degrees of the exact one (facet normals at chord 0.001); it differs from the through-axis angle by up to 22.4 degrees; the angle that remains between the exact normal and the tool axis (the X slope, which A cannot remove) is 0.3 to 25.2 degrees. The machine frame check keeps the tool vertical and puts the tip at a non-zero machine Y.

## Decision

Pass on all three criteria. Continuous 4-axis is feasible with `parry3d-f64`: no fallback needed. Recommended method: **Climb** (about 100 rays per pose), with the refined stencil as the cross-check and the normal offset as a cheap path for ball tools. Details, limits and open items are in the decision record.

Limits: only smooth star-shaped parts (the analytic part and its tessellation); no concave pockets, undercuts, thin walls, scanned meshes or tools offset from the A axis (SP-13 covered that half); Linux x86-64 only, so cross-platform bit-identity is untested; the refinement is a local search with no proof it finds the global maximum on awkward geometry.
