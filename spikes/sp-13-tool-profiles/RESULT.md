# SP-13 result: custom-profile tools

Throwaway spike code (work breakdown, Spikes; spike rules). The decision record is [`docs/decisions/0011-sp-13-tool-profiles.md`](../../docs/decisions/0011-sp-13-tool-profiles.md); this file is the write-up, and [`out/report.md`](out/report.md) is the full report.

## Question

Can one revolved-profile tool model drive tool-centre solving, the independent gouge checker and the simulator sweep for flat, ball, bull-nose, tapered ball, V-bit, chamfer, drill and custom-profile cutters?

Pass criteria: (1) flat, ball and V-bit results match closed-form values within tolerance; (2) a custom profile agrees with an equivalent standard tool; (3) sweep time within the SP-6 budget; (4) identical results at 1 and N threads.

## Method

A separate Cargo project with no third-party dependencies (`cargo test`, then `cargo run --release` for the report).

| Module | What it does |
| --- | --- |
| `profile.rs` | The profile: segments in the `openchuck_types::tool::ToolProfile` form, validated by the monotone rule; lower envelope `h(r)`, pieces in radius order, convexity, point for a normal |
| `shapes.rs` | Flat, ball, bull-nose, tapered ball, V-bit, chamfer and drill mapped to the profile, with impossible-value checks |
| `parts.rs` | Cylinder, cone and sphere on the A axis: exact top surface and normal, and an inscribed tessellation |
| `mesh.rs` | Triangle mesh, rotation about X (A), XY grid index |
| `drop.rs` | Drop-cutter: vertex, facet and edge tests generic over the profile pieces; height cull |
| `normal.rs` | Normal-offset tip for convex profiles |
| `gouge.rs` | Independent gouge checker: exact-surface sampling, crossing count against the revolved outline, depth as distance to it |
| `sweep.rs` | Dexel Z-map and rotary-grid stand-ins for SP-6's candidates, swept by the profile |
| `par.rs` | Deterministic parallel map with std threads |
| `tests/sp13.rs` | 14 tests, named after the requirements |

## Numbers

From `out/report.md` (release build, 4 threads available in the session container).

**1. Closed form.** Error = exact tip minus dropped tip. The mesh is inscribed, so the error is never negative and is bounded by the mesh's own error.

| Part | Positions per tool | Flat, ball, V 90: max error vertical (mm) | Along the normal (mm) | Mesh edge chord (mm) |
| --- | --- | --- | --- | --- |
| Sphere r 15 | 1681 | 0.00109, 0.00121, 0.00130 | 0.00099, 0.00100, 0.00097 | 0.0005 (triangles sit up to twice this inside) |
| Cylinder r 20 | 1681 | 0.00052, 0.00054, 0.00052 | 0.00050 | 0.0005 |
| Cone r 10, 20 degrees, top line | 41 | 2e-15 | 2e-15 | 0.0005 (the top line is a mesh edge) |

The tests also check bull-nose and chamfer against closed forms, flank contacts of a 30 and 45 degree V near the side of the cylinder and sphere, and a 60 degree cone where the V touches with the top of its flank.

**2. Custom against standard.** Ball as two arcs, flat with split bottom and side, V with split flank, bull-nose and tapered ball typed in: worst difference 0 (bit-identical) on sphere, cylinder and cone at 169 positions each, and identical dexel sweeps. A non-convex custom profile (flat to r 1, concave arc to (3, 2), side) matches a brute-force search of the exact surface within the mesh error.

**Gouge checker.** Worst gouge at the dropped pose, all eight tools, sphere and cylinder: at most 0.00098 mm (the mesh error). With the tip lowered 0.05 mm: 0.030 to 0.047 mm reported for every tool.

**3. Timings** (one thread unless stated).

| What | Time |
| --- | --- |
| `h(r)` | 5 to 8 ns |
| Normal-offset tip (convex tools) | 80 to 120 ns |
| Drop on a 147,070-triangle sphere, XY grid index | 0.35 ms (flat), 0.79 ms (bull-nose), 2.2 to 3.2 ms (ball, V, chamfer, drill, tapered, custom); about 12 ms before the height cull |
| Dexel Z-map, 0.05 mm cells, line move | 16 to 21 ns per cell and pose; 0.31 to 0.40 s for 19 million, 0.14 to 0.22 s on 4 threads |
| Rotary grid, 161 x 360, 360 degrees of A | 23 ns (flat) to 101 ns (ball) per cell and pose; 0.7 to 3.1 s for 31 million, 0.3 to 1.2 s on 4 threads |

SP-6 has not set a budget, so criterion 3 is not judged here.

**4. Determinism.** Drop grids (41 x 41) and both sweeps are bit-identical at 1, 2 and 4 threads.

## Decision

Proposed in record 0011: keep `ToolProfile`; validate it with the monotone rule; map the standard types by "cutting end to D/2, then a vertical side to the flute length"; solve tool centres with the generic drop-cutter, with normal-offset as a convex-only fast path; keep the gouge checker independent as built; leave the sweep budget to SP-6.

## Fallback status

Not triggered as a failure. The first fallback, "limit custom profiles to line and arc segments with a validity check", is what the design does anyway (the monotone rule). The second, excluding custom profiles from OP-05, is not needed: the generic drop handles non-convex profiles.

## Limits of the spike

- The tool axis is always the drop direction (+Z after rotating the part by A). Tool axis variation is SP-4's and slice 13's.
- The stock stand-ins are not SP-6's candidates; their removal checks are exact groove cross-sections, not SP-6's volume and memory criteria. The rotary grid's ray entry uses stepping and bisection and cannot represent material left above the tool.
- The concave-arc edge test samples 32 points, then refines; a contact narrower than the sample spacing on a very long edge could be missed. The gouge checker and the brute-force comparison found none on the test parts.
- The XY grid index is the simplest that works; it is not a proposal for slice 5.
