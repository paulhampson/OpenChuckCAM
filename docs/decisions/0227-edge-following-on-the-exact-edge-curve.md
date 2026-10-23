# 0227: Edge following cuts along the exact curve of a STEP edge, not the chords of its tessellation (lane STEPALL-E; issue 257; OI-229)

- **Status:** accepted by the project owner (23 Oct 2026), given directly in the lane STEPALL-E session ("Accept 0227"; decision 0049). Step 4 of the order the owner chose in [decision 0209](0209-step-geometry-for-every-strategy.md) (accepted, option A; item 5 there).
- **What it amends:** nothing in the requirements or the architecture. It settles [decision 0053](0053-slice-16-edge-following-brep.md) item 5 (the chains lie on the surface to the tessellation tolerance) for edges whose curve the kernel can give, and closes OI-229. Edge following on a mesh part is unchanged (decision 0047).
- **Date:** 22 Oct 2026.
- **Requirements:** OP-08 (edge and curve following), IMP-02, IMP-06, NFR-05 to NFR-07, GC-07.
- **Open items:** OI-229 closes. OI-960 is new (edge curve kinds that are not read yet).

## Context

Edge following on a STEP part follows the part's real edges (decisions 0053 and 0056). Each edge came from the kernel as a polyline sampled at the model's tolerance (`BrepEdge::polyline`, 0.01 mm by default), and the chain was those points. The engraving depth was exact at the points (the depth comes from a ray on the exact faces, decision 0017), but between them the cutter ran along a chord, up to the tolerance inside a curved edge; a coarser model gave a coarser edge (OI-229). Decision 0209 lists this as the one change edge following needs from the exact geometry: "snapping to the edge curve closes that".

## Decision

1. **The edge carries its exact curve.** `BrepEdge` has a new field, `curve: Option<EdgeCurve>`. `EdgeCurve` is a line, a circle, an ellipse or a clamped rational B-spline (`BSplineCurve`, the curve form of `BSplinePatch`), each with the parameters of the edge's first and last point, in the direction of the polyline. The kernel wrapper writes it from the edge's own curve (a Bézier curve is written as a B-spline). A degenerate edge, and a curve of another kind, are written as `null` and keep only the polyline (OI-960). Files and models written before the field read as `None`.
2. **The chain is the curve sampled finely.** `BrepEdges` replaces an edge's polyline by `edge_curve::sample`: points on the exact curve, bisected until no chord is more than `edge_curve::SAG`, 0.0005 mm, from the curve (a twentieth of the default tessellation tolerance). A line is its two ends. The sampling depends only on the curve, so the chains, and the G-code, are the same at any thread count (GC-07, NFR-06).
3. **The polyline still decides what an edge is.** Whether an edge is real (crease angle, seams) is decided on the kernel's polyline as before (decision 0053 item 2), so the set of chains does not change, only the points on them.
4. **A curve that does not fit its edge is ignored.** If the curve cannot be evaluated, or does not start and end within 0.0001 mm of the polyline's ends, the edge keeps its polyline (NFR-07). `BrepModel::validate` refuses a curve that is not finite or a spline that is not clamped.
5. **Placement moves the curve.** `transform_brep` maps the curve with the edge: frames and poles move, radii and a line's parameters (lengths) scale, angles and spline parameters stay. A curve kind it does not know is dropped, so the edge keeps its polyline.
6. **Still not projected onto the faces.** The points are on the exact edge curve, not cast onto the faces, for the reason in decision 0053 item 5 (a cast from outside can land an edge hidden behind a lobe on the lobe in front). The depth at each point is still the exact ray.

## Alternatives considered

- **A finer tessellation of every edge in the kernel.** Smaller error but never exact, and it makes the model larger for every use of the edges (the viewport draws them).
- **Snapping each chord point to the faces.** Rejected in 0053 item 5.
- **Sending the whole edge curve as a B-spline for every kind.** Simpler, but a circle then goes through a spline conversion that is only approximate; the three analytic kinds are exact in closed form.

## Consequences

- Library family 0.19.0 (decision 0048): `openchuck-types` gains a public field on `BrepEdge` (breaking) and the types `EdgeCurve` and `BSplineCurve`; `openchuck-path` gains the module `edge_curve`.
- The mesh path is unchanged.
- The STEP kernel stays in `openchuck-import`; the path library gets the curve through `BrepModel`, as it gets the faces.
- Changed generated G-code: an edge-following path on a STEP edge with a supported curve has more points on it (chords of 0.0005 mm instead of the tessellation's).

## Tests

- `crates/openchuck-path/tests/edge_curve.rs`: line, circle, reversed range, ellipse, rational spline quarter circle, a curve that cannot be evaluated, validation (`req_op_08_exact_curve_...`).
- `crates/openchuck-path/tests/brep_edge_following.rs`: `req_op_08_brep_a_chain_on_an_edge_with_an_exact_curve_lies_on_the_curve`, `req_op_08_brep_an_edge_without_an_exact_curve_keeps_the_kernels_polyline`, `req_op_08_brep_a_curve_that_does_not_meet_its_edge_is_ignored`, `req_op_08_brep_engraved_depth_is_exact_on_a_coarsely_tessellated_curved_edge`, `req_nfr_06_req_op_08_brep_exact_edge_following_is_identical_at_1_2_and_n_threads`.
- `crates/openchuck-geom/tests/transform.rs`: `req_imp_05_op_08_a_placed_edge_curve_is_the_curve_placed`.
- `crates/openchuck-types/tests/data.rs`: `req_imp_06_nfr_07_an_edge_reads_with_or_without_its_exact_curve`.
- With the kernel: see the kernel tests named in the pull request.
