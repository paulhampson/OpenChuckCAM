# 0277: Exact ball drops on STEP parts for helical, indexed, rest and simultaneous finishing (lane STEPALL-X2; issue #349)

- **Status:** proposed, 29 Oct 2026 (Europe/London). Only the project owner accepts a record (decision 0049).
- **Date:** 29 Oct 2026 (Europe/London).
- **Lane:** STEPALL-X2, branch `claude/lane-stepall-x2-ball-drops`, from `main` at `d5d0a74` (library family 0.26.0), with `main` at `f100c14` (0.27.0) merged once before the pull request. Issue #349 (STEPALL-0211 step 4 of 6), parent #257.
- **Requirements:** IMP-02, OP-02, OP-04, OP-05, OP-11, OP-12, OP-21, OP-23, SET-09, GC-07, NFR-04, NFR-05, NFR-06, NFR-07, NFR-12. Verification plan 4.1 (the 0.005 mm tessellation share of the 0.01 mm toolpath tolerance) and the *Exact surfaces* note of section 5.
- **Builds on:** [decision 0209](0209-step-geometry-for-every-strategy.md), [decision 0210](0210-step-face-sets-for-region-strategies.md), [decision 0211](0211-step-accuracy-measured.md) (accepted: query foundations, then ball drops; the 2-times limits of OI-835), [decision 0255](0255-bounded-tool-contact-checker.md) (accepted: the bounded checker), [decision 0256](0256-no-fixed-toolpath-mesh-qualifies.md) (accepted: no fixed finer toolpath mesh qualifies, accuracy left to STEPALL-X), [decision 0260](0260-exact-face-and-trim-queries.md) (accepted: the exact queries this drop builds on).
- **Open items:** OI-833 closed. Progresses OI-81 (ball tools done; other profiles are lane STEPALL-X3, #350), OI-835 (both limits met for the in-scope strategies, below), OI-1040 and OI-1041. New: OI-1110 (a last-bit library/application difference that exists on `main`), OI-1111 (no normal cone for rational patches), OI-1112 (the containment boundary is still the tessellation's window).
- **Numbers:** record 0277 and OI-1110 to OI-1112 from the coordinator's allocation (0277 to 0281, OI-1110 to OI-1129), checked free on `main` and every unmerged branch on 29 Oct 2026 and again after merging `main`. No project schema number: nothing stored changes. No library version change: the public API is additive and `cargo semver-checks -p openchuck-path --baseline-rev origin/main` passes (202 checks), as the Merge Manager ruled.

## Context

Every drop-based strategy on a STEP part dropped the tool onto the import's tessellation (OI-81), which lies up to 0.043 mm off the exact faces on the blisk (decision 0211, M1), so the zero-stock finishing operations cut 0.032 to 0.044 mm into the exact faces at the dropped poses (decisions 0211 and 0255). Decision 0260 made the exact queries two to three orders of magnitude cheaper, but measured decision 0211's probe (21 unlimited distance queries a drop, no seed) at 12.6 ms a drop on the blisk against 54 µs for a tessellation drop. Issue #349 asks for an exact ball drop with a justified search and error bound (not the kernel's deflection), the region, avoid and check sets as exact faces (OI-833), accuracy shown by the bounded checker, and generation within 2 times the import-tessellation baseline for the whole job and for every operation, before any strategy adopts it.

## Decision

1. **The drop is a branch and bound on the exact faces, not a search band about the tessellation's answer.** A ball of radius `r` moves along the tool line; a face point `p` is first held by the ball at the tip offset `lift(p) = z + sqrt(r^2 - rho^2)` (`z` along the axis from the ball's centre at offset 0, `rho` from the axis), and never when `rho > r`. The drop is the largest `lift` over the faces. No bound is taken from the tessellation.
   - Every point the search evaluates is a point of a face, so its `lift` is a real contact: the best found is never above the answer.
   - Every part the search passes over has a proven upper bound on its `lift` no higher than the best found plus `DROP_TOLERANCE`, 1e-4 mm: the answer is never more than that above the best found. Where a search would stop at its budget (decision 0260's 4,096 pieces), the tip is raised to the bound it stopped at, never below the true contact: the error is then a gap, not a gouge, and `ExactDrop::raised` reports it. No drop in any measurement below reached a budget.
2. **Faces.** Faces are visited highest bound first (the ball against each face's box); a face whose bound cannot beat the best is passed over.
   - **Boundary** (trim loops, or the parameter box of an untrimmed face): segments are visited highest bound first after a pass over runs of 16 segments; each segment is searched by halving its parameter, each piece bounded by the ball and the capsule of decision 0260 (the ball's highest reach to a ball about the piece's middle image, and to a capsule about its chord of radius `bend w^2 / 8`). Boundary points are the images of the loop polylines, as the closest-point search reads them (OI-1040).
   - **Inside an analytic face:** where the largest `lift` is at an inside point, the ball's centre lies on the face's normal at distance `r`, on an offset surface of the same kind: planes, cylinders, cones and spheres by a quadratic, the torus by a quartic, and a cone's apex as a point of its own. Each root is kept when its foot is inside the trims.
   - **Inside a B-spline face:** the hull cells of decision 0260. The distance from a point to a cell's boxes (axis-aligned and along the cell's frame) is convex, so the highest offset at which the ball reaches them is found by Newton steps from above, each an upper bound. A cell's middle gives a contact; a few Newton steps of the contact condition from a middle that beats the best tighten it.
   - **The normal-cone test.** A face's largest `lift` is where the ball rests tangentially (centre at `p + r n` or `p - r n` on the line), on its boundary (settled by the boundary search), or on a crease. A cell whose normals lie in a cone of axis `m` and half angle `theta` (from the cross products of its net's differences, which hold `S_u x S_v` for a polynomial patch) cannot hold a tangent point when the line keeps further than `2 r sin(theta / 2)` from both boxes shifted by `+- r m`; it is passed over. It is not applied to a rational patch whose weights differ by more than rounding (OI-1111), nor along a crease (an interior knot repeated to the degree, where the patch is only continuous: found by a unit test before any part had one). This is the critical-point test decision 0260 left open; for drops it also settles OI-1041's focal case. Without it, cells along a trim edge near the contact, where the untrimmed surface continues higher, were split to the tolerance (up to 1,400 cells and 1.2 ms a drop on the blisk).
3. **Profiles.** Only a tool whose lower envelope is a ball centred on its axis one radius above its tip (a ball end, or a ball end grown for stock to leave) has an exact drop (`ball_radius`). Any other profile falls back to the tessellation drop.
4. **Adoption per strategy, by an explicit choice.** `Strategy::exact_contact` (default false) says whether a host machines a STEP part with that strategy on exact contacts. It is true for helical, indexed, rest and simultaneous finishing, each measured within both limits below; rotary roughing, pencil clean-up and axial finishing keep the tessellation (outside this lane; axial and pencil are lane STEPALL-X3). Rest detection is unchanged. The application then builds every surface of the operation with `BrepSurface::with_exact_drops(true)`: the part (and through `CompositeSurface` the held regions and tabs beside it, SET-09 and OP-21), the stock-to-leave zones, and the region, avoid, check and watched face sets, which become exact faces instead of their triangles (OI-833). A surface's `drop_accuracy` (0 for a mesh, the tolerance for exact drops) widens the region test (`RegionConstraint::allows`) and the watched-surface warnings by the drops' accuracy, so mesh behaviour is unchanged; the on-surface test uses a limited search (`SurfaceSource::within`). The containment boundary is still the window of the set's tessellation (OI-1112).
5. **Notes.** On a STEP part the operation note says "drops the ball-end tool on the STEP part's exact faces, to within 0.0001 mm, not on its tessellation" where it does, and names the tessellation and its deflection otherwise (decision 0209 item 4.4, OI-830). Nothing in it implies the output is safe (NFR-12).
6. **Kernel isolation and API.** Everything is in `openchuck-path` (`brep.rs`, new `brep/drop.rs`, additions to `brep/hull.rs` and `brep/trim.rs`, `surface.rs`, `region.rs`, `composite.rs`, `strategy.rs`) on the plain B-rep data, and in `openchuck-app` (`operation.rs`, `selection.rs`). Additive library API: `BrepSurface::drop_ball`, `with_exact_drops`, `exact_drops`, `ExactDrop`, `ball_radius`, `DROP_TOLERANCE`, and the provided methods `SurfaceSource::drop_accuracy`, `SurfaceSource::within` and `Strategy::exact_contact`.

## Verification

Tests written from the requirement, without the kernel (`crates/openchuck-path/tests/brep_drop.rs`, `src/brep/tests.rs`):

- every kind of trimmed face (plane, cylinder, cone, sphere, torus, a rational B-spline, a polynomial one and a creased one) against the highest `lift` of a brute-force sampling, 60 random lines each: never below a sample (less the tolerance), and the answer is a point of the face; a ball on the point of a cone; a ball grown for stock to leave drops as the larger ball; the same answers on four threads and in reverse order; a line that is not a number and a flat tool give no exact drop; region, avoid and check face sets judged as exact faces on a banded bar, where most exact contacts are off the triangles (OI-833);
- the normal cone holds every normal of polynomial cells, stored and split; a rational cell has no cone; the cell distance is a lower bound on the distance to the cell, with its gradient as its slope;
- in the application, the face-set tests of decision 0210 (helical, indexed, simultaneous) pass on exact contacts, and the note test names exact or tessellated contact; all 39 STEP-kernel tests pass.

Local coverage of the new file on its own tests: `brep/drop.rs` 94.7 % of lines; the lines left are the defensive branches of NFR-07 (a budget reached, a Newton step that is not finite, a B-spline face without hull cells).

## Measurements

Machine: Intel Xeon @ 2.10 GHz, 4 cores, 15 GB, cloud container, release build, nothing else running for the timings. Inputs: `testdata/step/valid/blisk.step` (SHA-256 `0e472ff5…b646`) placed as decision 0211 places it; the corpus parts of `testdata/step/valid/`. Harness: decision 0211's `step_accuracy` example, with a new form (d), the STEP part with exact contacts for the strategies that take them, which is what the application now makes; (b) is the STEP part on the import tessellation. Source of the timings `02f0da2` (binary SHA-256 `4971f053…70fd`, worker `39cb62c2…38f7`); raw outputs are kept with the lane's evidence.

**One drop, one thread** (`step_queries`, decision 0260's lines: a 1 mm ball dropped along the normal at every eighth vertex moved 1 mm off; mean µs):

| Part | Tessellation drop | Exact drop |
| --- | ---: | ---: |
| blisk | 44.3 | 52.0 |
| box / cylinder / cone | 8.4 / 20.2 / 19.2 | 3.2 / 6.2 / 5.3 |
| sphere / torus | 24.1 / 28.3 | 32.4 / 16.1 |
| filleted_box / rotary_blank_drilled | 24.4 / 77.6 | 11.7 / 7.1 |
| bspline_face / extruded_spline | 49.9 / 52.0 | 27.3 / 45.1 |
| revolved_spline / revolved_spline_partial | 31.2 / 29.1 | 89.3 / 66.9 |
| hex_bar / shouldered_shaft | 15.4 / 66.6 | 4.4 / 20.5 |

On helical-finishing lines of the blisk (radial, X every 0.5 mm, A every 6 degrees, 6,900 lines) the exact drop's mean is 20.9 µs against 24.2 µs (p99 108 µs, max 511 µs; at most 60 cells and pieces a drop). Before the normal-cone test it was 87 µs (p99 1.15 ms). The revolved splines are rational patches with varying weights, which get no cone (OI-1111).

**The 2-times limits (OI-835)**, blisk, full stepovers, the six operations that succeed in machining order on the stock the ones before left, five interleaved runs of each form at one and four threads, all 60 timed trials successful; median generation seconds:

| Operation | 1 thread: b | d | d/b | 4 threads: b | d | d/b |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Rotary roughing (unchanged) | 1.289 | 1.313 | 1.02 | 0.982 | 0.973 | 0.99 |
| Rest machining | 8.343 | 10.072 | **1.21** | 5.124 | 3.091 | **0.60** |
| Pencil clean-up (unchanged) | 2.596 | 2.664 | 1.03 | 3.504 | 3.620 | 1.03 |
| Helical finishing | 7.613 | 9.413 | **1.24** | 2.610 | 2.606 | **1.00** |
| Axial finishing (unchanged) | 28.835 | 28.998 | 1.01 | 11.980 | 13.452 | 1.12 |
| Indexed finishing | 0.0761 | 0.0994 | **1.31** | 0.0439 | 0.0395 | **0.90** |
| Job generation | 48.98 | 52.58 | **1.07** | 23.11 | 23.76 | **1.03** |
| Job wall time (with simulation) | 764 | 767 | 1.00 | 221 | 217 | 0.98 |

The unchanged operations run the same code in both forms; their spread (0.99 to 1.12) is the machine's. Rest and helical on four threads vary most from run to run (the drops are shared by four threads), which is why each of the five runs alternates the forms. The exact forms make fewer drops (helical 238,775 against 257,515), as the fitter needs fewer between smoother contacts.

**Simultaneous finishing** refuses the blisk on both forms at the same move, as decision 0211 documents: "cannot keep the move to X 20.006, A 12039.640 within the tolerance: it would cut 0.1401 mm into the surface" on (b) and 0.1432 mm on (d), after 100 s on (d). Its limits are measured on the two corpus parts decision 0211 used (quick stepovers, five runs each, medians), with helical beside it:

| Part | Operation | 1 thread: b | d | d/b | 4 threads: b | d | d/b |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| cone | helical | 0.898 | 0.096 | 0.11 | 0.250 | 0.037 | 0.15 |
| | simultaneous | 1.506 | 0.165 | 0.11 | 0.820 | 0.060 | 0.07 |
| revolved_spline | helical | 1.282 | 2.341 | **1.83** | 0.359 | 0.656 | **1.83** |
| | simultaneous | 3.946 | 5.797 | 1.47 | 1.614 | 1.718 | 1.06 |

The revolved spline's helical finish is the closest to the limit: its patch is rational, so it gets no normal cone (OI-1111).

**Accuracy** (the bounded checker of decision 0255: every fed pose and the quarter, middle and three-quarter points of every fed move, intervals at most 0.0005 mm wide; M2 qualified in every run). "Within 0.01 mm" is the harness's new verdict on the proven deepest cut and the largest contact gap against the toolpath tolerance.

| Part | Operation | Form | Deepest cut | Proven at most | At the poses | Largest gap | Within 0.01 mm (cut, gap) |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| cone | helical | b | 0.0090 | 0.0094 | 0.0090 | 0.0005 | yes, yes |
| | | d | 0.0001 | 0.0006 | 0.0000 | 0.0005 | yes, yes |
| | simultaneous | b | 0.0090 | 0.0094 | 0.0090 | 0.0002 | yes, yes |
| | | d | 0.0000 | 0.0005 | 0.0000 | 0.0002 | yes, yes |
| revolved_spline | helical | b | 0.0191 | 0.0196 | 0.0191 | 0.0095 | **no**, yes |
| | | d | 0.0000 | 0.0005 | 0.0000 | 0.0002 | yes, yes |
| | simultaneous | b | 0.0169 | 0.0174 | 0.0169 | 0.0095 | **no**, yes |
| | | d | 0.0000 | 0.0005 | 0.0000 | 0.0002 | yes, yes |

The blisk at full stepovers (all six successful operations, both forms; 40,100 to 782,464 stations an operation):

| Operation | Form | Deepest cut | Proven at most | At the poses | Largest gap | Within 0.01 mm (cut, gap) |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Rotary roughing (0.3 left) | b, d | 0 | 0 | 0 | 0.0071 | yes, yes |
| Rest machining | b | 0.0426 | 0.0430 | 0.0426 | 0.0102 | **no**, **no** |
| | d | 0.0041 | 0.0045 | 0.0001 | 0.0040 | yes, yes |
| Pencil clean-up (unchanged) | b, d | 0.0342 | 0.0347 | 0.0342 | 0.0065 | **no**, yes |
| Helical finishing | b | 0.0426 | 0.0430 | 0.0426 | 0.0102 | **no**, **no** |
| | d | 0.0041 | 0.0045 | 0.0001 | 0.0040 | yes, yes |
| Axial finishing (unchanged) | b, d | 0.0428 | 0.0432 | 0.0428 | 0.0092 | **no**, yes |
| Indexed finishing | b | 0.0318 | 0.0322 | 0.0317 | 0.0086 | **no**, yes |
| | d | 0.0039 | 0.0044 | 0.0001 | 0.0036 | yes, yes |

With exact contacts the dropped poses lie within the checker's resolution of the exact faces, at face edges as elsewhere: the trim polylines of OI-1040 did not show at any pose on the blisk or the corpus. What remains between poses is the path fitter's chord (its 0.004 mm limit, decision 0211 M2 item 3). Pencil and axial finishing keep the tessellation's cut until lane STEPALL-X3.

**Determinism and equivalence** (M4): on the corpus parts and the blisk, each form's toolpath and its posted G-code for both posts (Makera Z1 and LinuxCNC) are byte-identical at 1, 2 and 4 threads; through the application, both posts' programs are identical at 1, 2 and 4 threads, pass the replay gates, and equal the library's form (d) byte for byte. The library's and the application's paths are equal on the corpus; on the blisk's full sequence they differ by 1.2e-12 mm in indexed finishing's entry move at quick stepovers and 1.35e-9 mm in axial finishing's contact length at full stepovers, with byte-identical G-code; the first exists on `main` with the tessellation drop too (OI-1110). On the blisk's full sequence (form d, both posts, application regeneration 760 s at one thread, 217 s at four) every check passed; the harness now compares the paths to 1e-6 mm, a thousandth of the written resolution, and reports the largest difference and where it is (1.35e-9 mm, axial finishing's contact length).

## Alternatives considered

- **A search band about the tessellation's drop** (decision 0211's probe, refined with `project_within` and `project_from`). Its band needs a bound on the tessellation's error, which the kernel's deflection is not (decision 0211 M1), and its cost is in many whole-part distance queries.
- **A finer fixed tessellation** (decision 0256, accepted: none qualifies).
- **A smaller cell budget** (OI-1041): not needed for drops; the normal-cone test keeps a drop to tens of cells.
- **Splitting cells along the direction that bends most**, tried for the blisk's ruled blades: cells stayed long along the straight direction and never reached the tolerance near contacts (1,144 drops raised to their bound); the longer direction is kept.
- **A per-operation choice stored in the project**: not needed; the strategy decides, and none of the four exceeds the limits.

## Consequences

- **Output:** on STEP parts, helical, indexed, rest and simultaneous finishing with a ball end change: the poses lie on the exact faces (at most 0.0006 mm into them where measured, against 0.0094 to 0.0426 mm before), and their face sets are exact faces. STL parts, other tools and the other strategies are unchanged.
- **Documents:** this record; the decision index; `docs/open-items.md` (OI-833 closed; OI-81, OI-835, OI-1040, OI-1041 noted; OI-1110 to OI-1112 new); the verification plan's *Exact surfaces* note; the work breakdown's STEPALL row; the changelogs of `openchuck-path` and `openchuck-app`.
- **Later lanes:** STEPALL-X3 (#350: tapered-ball axial and flat-tool pencil) and STEPALL-Q (#351, OI-950, OI-951, OI-960, OI-961, OI-970) edit the same files: `crates/openchuck-path/src/brep.rs`, `brep/drop.rs`, `brep/hull.rs`, `brep/trim.rs`, `surface.rs`, `region.rs`, `strategy.rs`, `crates/openchuck-app/src/operation.rs`, `selection.rs` and the `step_accuracy` example.
