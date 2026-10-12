# SP-9 result: exact surfaces for `SurfaceSource`

Spike branch `spike/sp-9-exact-surfaces`, code in `spikes/sp9-exact/` (separate Cargo project, empty `[workspace]`, never merged). Run on Linux x86-64, 12 Oct 2026, OpenCASCADE 7.8.1 through `occt-sys`, `curvo` 0.3.2, fixtures from SP-1 (`fixtures/`: box, cylinder, cone, sphere, torus, filleted box, a B-spline face, a drilled rotary blank). Raw tables: `reports/report.md` and `reports/report-as-nurbs.md`. Reproduce: `cargo run --release -- fixtures` and `cargo run --release -- fixtures nurbs`.

Requirements: IMP-02 (exact B-rep evaluation and its conditional waiver), INT-02 (B-rep faces as plain data), OP-05, OP-12 (queries the pathing needs), NFR-05, NFR-06, NFR-07, NFR-10, NFR-14.

## What was built

- `cpp/shim.cpp`: the SP-1 shim plus `occ_face_frame` (analytic frame and size), `occ_face_bspline` (any face as an unwrapped clamped B-spline: knots, poles, weights), `occ_face_project` (`GeomAPI_ProjectPointOnSurf`), `occ_face_distance` (`BRepExtrema`), `occ_face_ray` (`IntCurvesFace_Intersector`, honours trims), `occ_face_contains` (`BRepClass_FaceClassifier`) and `occ_face_trims` (p-curves sampled into UV loops, 48 points per edge).
- `src/analytic.rs`: plane, cylinder, cone, sphere and torus in OpenCASCADE's parametrisation: point and normal, closed-form projection, ray hits (quadrics closed form, torus by scan and bisection). About 190 lines.
- `src/nurbs.rs`: rational tensor-product NURBS, point and first derivatives, Gauss-Newton closest point from an NxN seed grid, Newton ray hit. About 200 lines.
- `curvo` 0.3.2 for NURBS point, normal, `find_closest_parameter` and the experimental surface-curve intersection (a ray as a two-point polyline).
- Tessellation baseline (brute-force closest-point and ray on the kernel mesh at linear/angular deflection 0.1/0.5 and 0.01/0.1).
- 14 x 14 interior (u, v) samples per face (all fixtures, 40 faces); projections from points 0.25 mm off the surface on both sides; rays from 3 mm above each sample along the tilted inward normal. The kernel is the reference.

## Findings

1. **Analytic faces: our own Rust code equals the kernel.** Point, projection and ray differences are at most 1.4e-14 mm; normals differ by at most 1.2e-6 degrees (the kernel's own noise). That covers plane, cylinder, cone, sphere and torus, including on the filleted box and the drilled blank. It is 3 to 7 times faster than the kernel for evaluation (0.25 to 0.7 µs against 1.5 µs), 3 to 7 times for projection and 20 to 70 times for rays (0.6 to 1.4 µs against 23 to 40 µs; torus 5.9 µs against 40 µs).
2. **The tessellation is not good enough to stand in for exact surfaces at the plan's budget.** Distance from the exact surface to the mesh, worst case over the fixtures: 0.16 mm at 0.1/0.5 and 0.011 mm at 0.01/0.1 (torus), against the 0.005 mm tessellation budget of verification plan 4.1. Facet normals differ from the true normal by up to 10 degrees (0.1/0.5) and 2.2 degrees (0.01/0.1); SP-4 found 1.2 degrees at its finest. Projection and ray errors follow the same figures. Planes are exact. The kernel's tolerance is not a bound (decision 0003).
3. **A UV box is not a face.** Rays that hit the untrimmed surface inside the UV bounding box but outside the face: 103 of 3,528 on planes (discs and the drilled blank) and 2 of 2,940 on cylinders. Adding the face's trim loops as UV polylines removes all but one boundary case (plane, 1 of 3,528). The same polylines classify (u, v) as the kernel does on 196 of 196 samples on every face except six end discs, where one sample in 196 lies within the polyline's sagitta of the boundary. The two-loop drilled cylinder agrees on all 196. So the plain-data form of a face needs its trim loops, not only the surface and the UV bounds. The loops were tested on rectangular and circular boundaries and holes, not on faces with holes across a periodic seam (OI-70).
4. **B-spline faces (one degree 2 x 2, 3 x 3 non-rational fixture): own evaluator and `curvo` both match the kernel.** Point: 7e-15 mm for both; normal: 0 (ours) and 1.2e-6 degrees (`curvo`; its normal is unnormalised and its sign convention was ignored in the comparison). Projection distance: 2e-15 mm (ours), 3e-15 mm (`curvo`); projection point 1e-14 mm (ours) and 4e-8 mm (`curvo`). Ray first hit: 9e-13 mm (ours) and 2e-6 mm (`curvo`, untrimmed). Mesh error on this face: 0.088 mm and 0.0098 mm.
5. **Rational surfaces (analytic faces converted by the kernel to NURBS: sphere and torus).** Both evaluators agree with the exact surface to 1.5e-14 mm, with each other to 3e-14 mm, and ray hits agree to 1e-12 mm. Projection is the weak point of our code: Gauss-Newton from a 16 x 16 seed grid reaches 1e-14 mm on the torus, but stalls at the sphere's poles (worst 0.48 mm; 8 x 8 seeds gave 0.8 mm and 2.0 mm), where the parametrisation is singular and the Gauss-Newton matrix is singular. `curvo` projects those to 1.4e-7 mm without trouble.
6. **Speed on the B-spline face.** Evaluation: `curvo` 0.99 µs, ours 0.86 µs, kernel 2.4 µs. Projection: ours 122 µs, kernel 212 µs, `curvo` 2.4 ms. Ray: `curvo` 68 µs (experimental, untrimmed only), ours 117 µs, kernel 398 µs. Kernel point-to-trimmed-face distance (BRepExtrema): 57 to 340 µs.
7. **`curvo` 0.3.2 panics (index out of bounds, `nurbs_surface.rs:621`) on the kernel's periodic B-spline form** (cylinders, spheres and tori converted without unwrapping). Fed the clamped, unwrapped form it did not fail. NFR-07 would need either a `catch_unwind` guard or only unwrapped input. Its ray intersection is marked experimental in its documentation. 95 crates in its tree (nalgebra 0.35, argmin, geo, spade, gauss-quad and others); MIT licence.
8. **Conversion is exact**: the kernel's NURBS form of a sphere or torus lies on the analytic surface to 3e-14 mm, so one plain-data form (clamped patch) can carry every non-analytic face and every analytic one the library does not specialise.

## What `SurfaceSource` needs for B-rep faces

Per face: (a) point and unit normal at (u, v) with the face orientation applied; (b) closest point on the surface with its (u, v) and distance, and a distance to the trimmed face; (c) ray hits on the trimmed face (first hit is what SP-4's climb and the drop-cutter of decision 0012 need); (d) a trim test in (u, v); (e) the tessellation, always, for display, for the mesh checker and as the fallback. All five were implemented against plain data in this spike: analytic frame and size, a clamped NURBS patch, UV bounds, and trim loops.

## Limits of this evidence

- One non-rational B-spline fixture (degree 2). No sculpted multi-patch surfaces, no trimmed NURBS with holes, no degenerate edges other than the sphere poles, no offset or revolved/extruded surfaces (the kernel converts them to B-splines in the shim; not exercised).
- STEP files written by OpenCASCADE only (OI-25 is open).
- Linux x86-64 only. Our code is pure `f64` and deterministic by construction (no threads); the kernel's tessellation differs across architectures (decision 0003), which is another reason to evaluate in our own code.
- Timings are single-thread microbenchmarks on a cloud container, not a budget.
- The mesh baseline is brute force; speed of a BVH is SP-4's measurement.
