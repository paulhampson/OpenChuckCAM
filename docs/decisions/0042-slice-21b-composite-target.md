# 0042: Slice 21b design refinement: analytic solids and the composite target (SET-09, OP-21)

- **Status:** proposed (16 Oct 2026). Built on `claude/lane-v-slice-21b-composite-target` (lane V). It builds item 2 of [decision 0035](0035-workholding-and-retaining-tabs.md), which the owner accepted on 15 Oct 2026 with answers on tab layout (tabs restricted to the head and tail, a tab narrower than the tool refused, no cut-off in P1), and sits beside [decision 0041](0041-slice-21a-workholding-data.md) (the setup data, merged). This record is proposed until the owner accepts it. It adds nothing to the requirements or the architecture.
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): new OI-185; OI-162 is measured here.

Numbering: on 16 Oct 2026 `main` holds records up to 0041 (lane U) and OI-181, and unmerged branches hold records up to 0041 (lane J, which renumbers) and OI up to 184. This record takes 0042 and OI-185. It first took 0040 and OI-170, then 0041 and OI-180, each renumbered when `main` took those (lanes M, N and U). Lane V claims no project schema number (it changes no file format).

## Context

Decision 0035 item 2 puts the held region and each tab into the target as analytic solids joined to the model's surface, so no strategy changes. Slice 21b builds that part in `openchuck-path`: the solid, its queries (tool drop, point, normal, distance, ray) and the composite `SurfaceSource`, without changing the port or adding a field to a public struct (decision 0026, `cargo-semver-checks`). Slice 21a (the setup data) and 21c (the application wiring) are other lanes.

## Decision (proposed)

1. **One solid type, any axis.** `Solid` is a capped cylinder or frustum on any axis: a base point, a unit axis, a positive length and a positive radius at each end (equal radii give a cylinder). It is not tied to the rotary axis, so the held region (about the rotary axis), a tab on the axis, a tab parallel to the axis at a radial offset (the around-axis layout of decision 0041 item 4), a tab across the axis and a tilted tab are one case. Impossible dimensions (zero, negative or not finite) are refused with a message naming the field (NFR-07). The type is new and its fields are private, so the library family's semver check is unaffected. It also gives the exact volume, area, bounds, a containment test and a closed outward-facing mesh.
2. **Point, normal and distance are closed form.** The query reduces to the solid's half-plane section (axial position, radius), whose boundary is three line segments; the nearest of the three is exact, with the outward normal of the segment it falls on. A point inside gets the nearest boundary point, as a mesh does.
3. **Ray is closed form.** The two discs, and the lateral cone as a quadratic in the ray parameter in its stable form (a cylinder is the case of equal radii). A ray that starts inside meets the surface where it leaves, as on a mesh.
4. **Tool drop maximises `z - h(r)` over the boundary.** `h` is the profile's lower envelope, `z` the height along the tool axis and `r` the distance from it. The maximum over a solid is on its boundary (a point inside has one directly above it with the same `r`). The boundary is ruled, so for a section plane at angle `theta` it is three segments: the bottom disc's radius, the lateral generator, the top disc's radius. On a segment the function is concave for a convex profile (`h` is convex and non-decreasing in `r`, and `r` is convex along a segment), so a golden-section search over the part of the segment within the cutting radius is exact to rounding. The best of the segments is quasi-concave in `theta` for a convex profile, so `theta` is searched over the arc where a section can come within the cutting radius of the tool axis (found by sampling the section's distance from the axis, bisecting the ends of the runs of reachable samples and refining about every sampled local minimum of the distance, so a tool much smaller than the solid is not missed, including the two windows where a line through a large solid enters and leaves), sampled and refined about the best sample. All of this is deterministic and uses no threads (NFR-06). A solid beyond the tool's reach is rejected before any of it runs.
5. **Composite.** `CompositeSurface` holds the model's source (shared, `Arc<dyn SurfaceSource>`) and the solids and answers as their union: the highest contact for a drop, the nearest point for a distance, the first hit for a ray, with ties going to the model and then to the solids in order. With no solids it answers as the model. Its tessellation is the model's plus each solid's, built once at construction (this copies the model's mesh). `SurfaceSource`, `StrategyInput` and the strategies are unchanged: stock to leave (the grown profile drops on the composite), gouge protection and the simulation target treat the solids as part.
6. **Tests.** Named after SET-09, OP-21 and NFR-06: analytic volume and area, closed-form closest points, rays and cylinder drops (ball and flat tools); agreement with a fine inscribed mesh of each solid for drops (3 tools, 4 solids on the rotary axis, across it and tilted, tool axes vertical, across the rotary axis and tilted) and for rays and nearest points; the composite as the union; helical finishing on a cone with a held region and a tab, checked by the independent checker against the exact surface (no gouge over 0.003 mm), and the same drops at 1, 2 and 7 threads.

## Measurement (OI-162)

Release build, the 1,002,000-triangle benchmark model, 3 solids (a held region, a tab on the axis and a tab across it, which is a harder case than the owner's layout needs), 4,000 drops with the tool axis turning about the rotary axis and the tool tracking along X:

| Tool | Model alone | Composite | Solid, one query (average) | Solid, one query reached |
| --- | --- | --- | --- | --- |
| Ball, 4 mm | 40 µs | 83 µs | 18 µs | about 0.5 ms (1,272 of 12,000 reached) |
| Flat, 6 mm | 199 µs | 228 µs | 20 µs | about 0.5 ms (1,424 of 12,000 reached) |

A solid out of reach costs next to nothing; the cost is the drops that reach one, 0.5 ms each, on the order of ten thousand profile evaluations per solid, which a strategy meets only near a held region or a tab. For the ball tool on the dense model the composite roughly doubles the cost of a drop on average, because the mesh drop is itself fast there. A budget to hold it against has not been set (OI-162 stays open for that). Cheaper options if it is needed: a looser search tolerance (the maximum is stationary, so the height error is the square of the position error), a closed form for the flat and ball tools, and stopping the search once the best so far exceeds the model's drop.

## Alternatives considered

- **Drop on a fine mesh of each solid:** simple, and the mesh drop-cutter is already there, but the error is a chord error that only the segment count holds under the 0.005 mm budget of verification plan 4.1 (0.0005 mm at 360 segments on a 12 mm radius, more on a larger tab), and every query then walks thousands more triangles. Used here only as the check.
- **A rotary-axis-only solid** (a revolved profile): covers the held region and tabs on the axis but not tabs parallel to the axis at a radial offset, which decision 0041 item 4 stores for tabs around the axis.
- **Closed-form drop per tool shape:** exact and fast, but one case per shape and per solid orientation; the search covers every profile and the custom ones (see OI-185).

## Consequences

- `openchuck-path` gains `Solid` and `CompositeSurface` (new types and functions only; its changelog says so). No file format changes and no golden file changes. A project without a held region or tab uses the model's source and produces byte-identical G-code.
- Work breakdown: the slice 21 status and the lane note are updated in this change; 21a and 21c are not started.
- Lane W (21c) builds the composite from the setup and passes it where the model's source goes, for generation, the checker's target and the simulation target. The checker used here is the testkit's, which samples a surface of revolution about the rotary axis; for a tab parallel to the axis at a radial offset (decision 0041 item 4), which is not a surface of revolution about the rotary axis, 21c needs a checker target that can take a `SurfaceSource` (not built).
- The owner's answers in the accepted 0035 do not change this code. Tabs lie in the head or tail zone and a tab narrower than the tool is refused when the tool is chosen (slice 21c); both are checks on the data and the tool, made before a solid is built. The shapes `Solid` takes (cylinder and frustum, any axis) are the shapes decision 0041 stores.

## Open items

- **OI-185:** for a concave custom tool profile the drop on a solid is found by sampling, not by splitting at every profile piece as the mesh drop-cutter does, so it is accurate to the sampling and not to rounding. Standard tools are convex and exact. Decide whether to refuse a concave profile (as stock to leave does), split by piece, or accept.
- **OI-162** (measured above; open for a budget).
