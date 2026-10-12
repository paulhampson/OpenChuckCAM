# 0017: SP-9 result, exact surfaces: plain data in, pure-Rust evaluation in the library, the waiver not applied

- **Status:** proposed (12 Oct 2026), awaiting the project owner. Until it is accepted this is guidance, not authority.
- **Date:** 12 Oct 2026.
- **Spike branch:** `spike/sp-9-exact-surfaces` (throwaway, never merged). Code in `spikes/sp9-exact/` (a separate Cargo project with an empty `[workspace]`); the write-up is `spikes/sp9-exact/RESULT.md` and the raw tables are `spikes/sp9-exact/reports/` on that branch.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-72 to OI-76 are new.

Numbering: records 0001 to 0016 and OI-01 to OI-70 are on `main` (0016 is SP-7, accepted). Unmerged `claude/slice-5-pathing-core` also claims 0016 and OI-68 to OI-71 with different text; this record takes 0017 and OI-72 to OI-76 to avoid both.

## Context

SP-9 asks what `SurfaceSource` needs for B-rep faces and who evaluates NURBS (work breakdown, Spikes). Requirements: IMP-02 (exact B-rep, with the conditional waiver if no usable exact evaluation of NURBS faces exists), INT-02 (B-rep faces and edges as plain data), OP-05 and OP-12 (the queries pathing makes), NFR-05, NFR-06, NFR-07, NFR-10, NFR-14. Architecture section 4 (the port), section 5 and the crate selection's NURBS section leave the NURBS evaluator open "after the STEP spike". [Decision 0003](0003-step-kernel-route.md) showed that exact evaluation is available from the kernel shim and said the waiver should not be applied yet. [Decision 0015](0015-sp-4-ray-cast-sweep.md) puts the mesh `SurfaceSource` on a parry `TriMesh`.

The spike compared, against OpenCASCADE as the reference, our own analytic Rust code, our own NURBS code, `curvo` 0.3.2, and the kernel tessellation, on 45 faces of the SP-1 fixtures (point and normal, closest point, ray hit, trim classification, speed). Details and limits are in `RESULT.md`.

## Results

| Criterion | Result |
| --- | --- |
| Exact-versus-mesh differences measured | Mesh to exact surface, worst case: 0.16 mm at 0.1/0.5 deflection and 0.011 mm at 0.01/0.1 (torus); facet normals off by up to 10 and 2.2 degrees. Projection and ray errors follow. Over the 0.005 mm budget of verification plan 4.1 at both settings. |
| Analytic faces in Rust against the kernel | Plane, cylinder, cone, sphere and torus: at most 1.4e-14 mm in point, projection and ray; normals within 1.2e-6 degrees. 2.6 to 22 times faster evaluation, 2 to 7 times faster projection, 7 to 74 times faster rays. |
| NURBS evaluated in Rust against the kernel | Own evaluator and `curvo` both match to 1e-14 mm (point), also on the kernel's rational form of a sphere and torus. |
| NURBS closest point | `curvo` accurate (4e-8 mm on the B-spline face, 1.4e-7 mm on the sphere) but 2.4 ms per call; ours 122 µs, exact on the B-spline face and the torus, but wrong by up to 0.48 mm at sphere poles (singular parametrisation). |
| Trims | A UV bounding box accepts false hits (103 of 3,528 plane rays, 2 of 2,940 cylinder rays). UV trim loops sampled from the p-curves fix all but one boundary sample; they classify like the kernel on 196 of 196 samples on 34 of 45 faces and on 195 of 196 on the six end discs. |
| Dependency assessment | `curvo` 0.3.2: MIT, 95 crates in its tree, young (2.5 years, two dependents), ray intersection "experimental" and untrimmed, panics on a periodic B-spline input (NFR-07). |

## Decision (proposed)

1. **Apply no waiver.** IMP-02's conditional waiver is not triggered: exact evaluation of NURBS faces is usable and small (about 200 lines of evaluation). The first release evaluates all faces on exact data, and the tessellation remains the display, mesh-check and fallback form.
2. **Exact faces reach `openchuck-path` as plain data**, not as a kernel handle. The face record (in `openchuck-types`, built by `openchuck-import` from the kernel through the shim) holds: the surface (a plane, cylinder, cone, sphere or torus with its frame and size, or a clamped, unwrapped rational B-spline patch with knots, poles and weights), the face orientation, the UV bounds, the trim loops as UV polylines, and the tessellation with a face index per triangle. The shim converts every other kernel surface (revolution, extrusion, offset, Bezier) to the B-spline form. This keeps the kernel inside `openchuck-import` and the pathing library free of it (architecture section 12).
3. **The B-rep `SurfaceSource` is pure Rust inside `openchuck-path`**, with these queries per face: point and unit normal at (u, v) with orientation applied; closest point on the surface with (u, v) and distance; first (or all) ray hits restricted to the trimmed face; a trim test in (u, v); and the tessellation. Analytic code is in-house (the test oracles need it anyway); it is closed form, deterministic and has no thread dependence (NFR-06).
4. **NURBS evaluation is in-house** (basis and first derivatives; Newton for ray and closest point), with a regression test against the kernel and the analytic surfaces. `curvo` is not adopted: it matches ours in accuracy and speed of evaluation (1.0 against 0.9 µs), its projection is 20 times slower than ours, and its ray tool is experimental and untrimmed; the panic on periodic input is a reason to check any later adoption again. It is recorded as tried and not adopted in `docs/dependencies.md`. The closest-point solver must be made robust at singular points (OI-72) before slice 11 accepts NURBS faces.
5. **A host with its own kernel** (FreeCAD) can still implement `SurfaceSource` directly, as the architecture says. The kernel-backed route is not built into the product: it would cost 7 to 74 times more for the cases the spike measured and would put C++ behind a library port.
6. **Process.** The tessellation setting for B-rep faces is chosen from the measured deviation, not the kernel tolerance (verification plan 4.1; OI-65, OI-28).

## Alternatives considered

- **Kernel-backed `SurfaceSource` for STEP models** (decision 0003's default): exact and available, but slow for rays (23 to 400 µs), couples pathing to the C++ kernel and its crash isolation, and gives platform-dependent tessellation. Kept as the host option.
- **`curvo` for NURBS:** see item 4.
- **`brepkit-math`** (AGPL-3.0-only, 66): not tried in this spike; the licence rules out a later dual licence (INT-09) and its reader failed two of eight fixtures (decision 0003). Revisit with OI-26.
- **Apply the waiver (tessellation for non-analytic faces):** unnecessary given the above; the mesh budget fails at both tested settings.

## Consequences

- **Work breakdown:** the SP-9 Status cell is updated in this change. Slice 5 designs the `SurfaceSource` port for the five queries; slice 11 builds the shim additions (`occ_face_frame`, B-spline unwrapping, trim sampling), the face record and the Rust implementations, and keeps the SP-9 comparison as a regression test against the kernel.
- **Architecture** (sections 4, 5 and 13: "needs a decision after the STEP spike") and **crate selection** (NURBS section) should be edited to match once this record is accepted. Not edited here, per the repository rule.
- **`openchuck-types::brep`:** its documentation says surface parameters stay out of the shared types until SP-9 decides (decision 0004). The face record of item 2 is the proposal; it is not added until this record is accepted.
- **Verification plan:** add the exact-versus-kernel comparison of the spike (analytic, rational and B-spline faces, trims) to the STEP checks (OI-28 covers tolerance on golden tests).
- **Dependencies:** `docs/dependencies.md` gains a `curvo` row (tried, not adopted). No dependency is added to the workspace.

## Open items

OI-72 to OI-76 in the register: closest point at singular points and in general; trims across a periodic seam and the sampling density; more NURBS evidence (sculpted multi-patch surfaces, trimmed NURBS with holes, STEP from other tools, with OI-25); cross-platform bit identity of the pure-Rust evaluators; mesh normals and deflection setting for B-rep faces.
