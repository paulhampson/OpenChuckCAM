# 0015: SP-4 result, ray-cast sweep around the A axis with parry3d-f64

- **Status:** accepted by the project owner (12 Oct 2026). OI-07 is closed; OI-64 to OI-67 stay open as the items the record names for later slices.
- **Date:** 12 Oct 2026.
- **Spike branch:** `spike/sp-4-ray-cast-sweep` (throwaway, never merged). The code is in `spikes/sp-4-ray-cast-sweep/`, a separate Cargo project whose only dependency is `parry3d-f64 =0.31.1`, never a workspace member; the write-up is `spikes/sp-4-ray-cast-sweep/RESULT.md` and the full report `spikes/sp-4-ray-cast-sweep/out/report.md`, both on that branch.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-07 closed on acceptance; OI-64 to OI-67 are new.

Numbering: records 0001 to 0013 and OI-01 to OI-60 are on `main`. The unmerged branch `claude/slice-4-tool-library` claims record 0014 (`0014-slice-4-tool-library.md`) and OI-61 to OI-63, so this record takes 0015 and OI-64 to OI-67. The unmerged branch `claude/sp-13-slice-4-dh7j4e` carries an older copy of the slice 4 work (its `0011-sp-13-tool-profiles.md` repeats a number `main` gives to slice 3, and its OI-53 to OI-55 are already on `main`); it claims nothing new.

## Context

SP-4 (work breakdown, Spikes, 5 days) asks: is continuous 4-axis feasible with `parry3d-f64` ray casts swept around the A axis? Pass criteria: accuracy within 0.01 mm; a runtime figure for the reference part; identical results at 1 and N threads. Named fallback: own BVH, or analytic-only first for the continuous family. It runs before slices 5 and 13.

Requirements: OP-05 (simultaneous 4-axis), OP-06 (tool axis control), OP-12 (gouge protection), OP-13 (feed at the contact point), NFR-04, NFR-05 (0.01 mm default tolerance), NFR-06 and GC-07 (identical results at any thread count), NFR-07. Architecture section 5.2 (`SurfaceSource`), the crate selection row for geometry queries, section 8 (determinism) and verification plan 4.1 (0.01 mm deviation, 0.005 mm of it for tessellation). Decision 0012 (generic drop-cutter, `h(r)`) and decision 0001 item 7 (OI-07, tool axis variation).

## Method

The spike builds, behind plain types of its own (no parry type is exposed):

1. An analytic star-shaped part, 80 mm long, `r(x, th) = 14 + 3 sin(2 pi x / 40) + a2 cos(3 th)`, with exact points and normals: a vase (a2 = 0) and a lobed variant (a2 = 1.5 mm) whose normal leaves the radial plane. An inscribed tessellation of it goes into a parry `TriMesh` at five chord tolerances, up to 984,060 triangles.
2. Four tool-centre methods on ray casts and distance queries: a ray stencil over the tool disc (66 rays at 0.75 mm rings), the same refined by a pattern search, a climb (central ray, then the pattern search), a ball-only distance bisection, and a ball-only normal offset.
3. Two sweeps: *through-axis* (stepped A, rays across the A axis along X stations; the machine Y stays 0) and *normal-following* (tool axis angle is the facet normal's angle in the plane perpendicular to X; the tool is dropped along that line through the hit point).
4. An exact reference with no mesh: a dense drop of the tool along the same line on the analytic surface, and the distance from a point to the analytic surface.

9 tests (named after the requirements) and a report binary. "Step A, cast along X" is read as rays perpendicular to the A axis at stations along X.

## Results

| Criterion | Result |
| --- | --- |
| 1. Accuracy within 0.01 mm | Pass on the parts covered. Climb, refined stencil, distance and normal offset are within -0.003 to +0.0015 mm of the exact surface for a 3 mm ball and a 3 mm flat tool, 1000 poses per row, both parts, both sweeps, at a mesh whose real error is 0.0025 mm. The unrefined 0.75 mm stencil is not enough: up to 0.055 mm (ball) and 0.018 mm (flat) off. |
| 2. Runtime for the reference part | About 573,000 poses (0.1 mm X stations, 0.5 degree A steps, 80 mm): Climb 33.6 s on one thread and 9.5 s on four with 148,470 triangles; 39.4 s and 10.8 s with 984,060 triangles. Refined stencil 47.9 s and 13.8 s; distance bisection 67.0 s and 20.0 s. About 100 rays per pose. One ray costs 1.2 to 2.7 microseconds from 37,000 to 984,000 triangles. |
| 3. Identical at 1 and N threads | Pass. Bit-identical pose hashes at 1, 2, 3, 4 and 7 threads for every method in both sweeps, using fixed chunks with results in input order. Repeat runs are identical. |

Findings:

- **The mesh is the error budget, not the method.** The chord tolerance parameter understates the real error by about 2.5 times (parameter 0.001 gives 0.0025 mm; 0.002 gives 0.0054; 0.004 gives 0.0105, which breaks the 0.01 mm limit). Verification plan 4.1 gives tessellation 0.005 mm, so slice 5 must set and measure the real error, not trust the parameter (OI-65; also OI-08).
- **Refinement is required.** Ring sampling at 0.75 mm misses the contact point; a local pattern search from the best ray (or from the central ray alone) reaches the mesh error. Climb costs about 100 rays against 136 for stencil plus refinement, and gives results within 0.00003 mm of it on every row.
- **Normal offset is valid for ball tools, judged by distance.** The centre `hit + radius * facet normal` is within the mesh error of the radius from the exact surface (-0.0022 to +0.0011 mm), at 2 rays and 3 to 4 microseconds a pose. It is not on the tool-axis line, because the normal has an X component that A cannot follow, so the tip height along the axis is wrong by up to 0.6 mm; a strategy must place the tool at the centre, not drop it along the axis. The facet normal also moves the contact along the surface by about radius times the facet angle, which jitters the path; OP-13 feed at the contact point must allow for it.
- **Tool axis variation (OI-07).** With a fixed vertical tool, A is the only control: the best axis is the normal's angle in the plane perpendicular to X. On the lobed part it differs from the through-axis angle by up to 22.4 degrees, and the angle left between the normal and the axis, from the X slope, is 0.3 to 25.2 degrees and cannot be removed. Normal-following moves the machine Y off 0, so Y is a real axis in the continuous family, not a constant.
- **Facet normals give the tool angle only roughly.** The mesh's in-plane normal angle is within -1.18 to +1.08 degrees of the exact one at chord 0.0025 mm. Slice 13's lead and tilt limits and its smoothness of A need an averaged or smoothed normal (OI-67).
- **Cost.** The BVH build is 0.03 to 0.7 s from 37,000 to 984,000 triangles. Ray cost grows slowly with mesh size, so the tessellation can follow the accuracy budget without a large time penalty.
- **Dependency.** `parry3d-f64 =0.31.1` with default features pulls 46 crates (glam, glamx, rstar, spade, simba and others). Its `Vector` is a `glamx` type, kept behind our own type in the spike as decision 0002 and `docs/dependencies.md` require. The `parallel` feature was not used.

## Decision (proposed)

1. **Continuous 4-axis is feasible with `parry3d-f64` queries: no fallback.** Slice 5 builds the mesh `SurfaceSource` on a parry `TriMesh` behind our own interface, pinned at 0.31.1; a hand-written BVH is not needed.
2. **Tool-centre solving for the continuous family uses the climb** (central ray, then a pattern search over the tool disc down to 0.02 mm) as the default generic method for any profile `h(r)` from decision 0012, with the ball-only normal offset as a fast path judged by distance, and the refined stencil and the exact checks as cross-checks. Slice 13 may speed it by starting each station's search from the previous one (not tried here).
3. **Tool axis policy for OP-06**: A is chosen so the tool axis is the surface normal's component in the plane perpendicular to X, within the user's lead and tilt limits; the remaining X-slope angle is reported, not hidden.
4. **Determinism**: sweeps use fixed contiguous chunks and write results in input order, as in SP-13; no result depends on the thread count.
5. **Tessellation tolerance**: the mesh's real error against the exact surface is measured (as the spike did), and the budget in verification plan 4.1 is checked against it.

## Alternatives considered

- **Own BVH**: not needed; parry's ray cost is 1 to 3 microseconds up to a million triangles.
- **Analytic-only first for the continuous family**: not needed, but the spike's exact reference shows how a `SurfaceSource` for analytic faces would be checked.
- **Distance bisection as the default**: exact for balls only, and 2 times slower than the climb; kept as a cross-check.
- **A larger ring stencil instead of refinement**: matching 0.01 mm would need a spacing near 0.25 mm, which is about 490 rays a pose for a 3 mm tool.

## Consequences

- No change to `requirements.md` or `architecture.md`: section 5.2 and the crate selection already name parry3d-f64 for geometry queries, behind our interface.
- **Slice 5**: mesh `SurfaceSource` on a parry `TriMesh` (point, normal, ray, distance); the drop-cutter of decision 0012 uses `climb` for the generic case; tessellation error measured, not assumed (OI-65).
- **Slice 13**: items 2 and 3 use decision items 2 and 3; smoothed normals for the tool angle (OI-67); a corpus with concave features and tools offset from the A axis before the slice is accepted (OI-66).
- **OI-07** closes: SP-13 (decision 0012) answered the cutter-shape half and this record answers the tool-axis half, accepted 12 Oct 2026.
- `docs/dependencies.md`: the `parry3d-f64` row notes SP-4's use and the feature set tried.
- Cross-platform bit-identity of parry's results was not tested (OI-64).

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Bit-identical results across platforms (x86-64, arm64, Windows, macOS): parry3d-f64 has an `enhanced-determinism` feature (libm); run the spike's pose hash on each platform, with and without the feature. (OI-64) | Slice 5 | CI runners on three platforms |
| 2 | The chord tolerance parameter understates the real mesh error by about 2.5 times; set tessellation from a measured error and check verification plan 4.1's 0.005 mm. (OI-65) | Slice 5, with OI-08 | A measurement on the first strategy |
| 3 | The climb is a local search and the parts were smooth and star-shaped. Test it on concave pockets, thin walls, undercuts, scanned meshes and tools offset from the A axis; consider a warm start from the previous station. (OI-66) | Slice 13 | A corpus with concave features |
| 4 | Facet normals give the tool angle within about 1.2 degrees; smooth or average them for lead and tilt limits and for smooth A. (OI-67) | Slice 13 | The A-axis smoothing design |
