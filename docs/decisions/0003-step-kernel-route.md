# 0003: STEP kernel route (SP-1)

- **Status:** proposed. Needs your decision before slice 11 design starts. Updated 11 Oct 2026 with Linux, macOS and Windows CI results.
- **Date:** 11 Oct 2026
- **Spike code:** branch `spike/sp-1-step-kernel` (throwaway, never merged into `main`)
- **Context:** requirements IMP-01 to IMP-03, IMP-06, NFR-01, NFR-07, NFR-09, NFR-10, NFR-13, NFR-14, INT-02, INT-09. Architecture sections 3, 4, 7, 12 and 13. Work breakdown spike SP-1 and slices 0, 11. Verification plan, C++ bindings and licence flags.

## Decision requested

Take the first fallback named in SP-1: **a thin in-house wrapper over the OpenCASCADE C++ kernel**, confined to `openchuck-import`, instead of depending on the `opencascade` crate's safe API. Also:

1. **Run STEP parsing in an isolated child process** with a timeout, behind `ModelImporter`. The reader crashes on damaged input (below), and NFR-07 forbids crashes.
2. **Keep `brepkit-io` as a watched candidate**, not a dependency. Re-spike it when it reads full spheres and shell-only files and has been run against real STEP exports from other CAD tools.
3. **Do not adopt `cadrum`.**

Rejected: using `opencascade` as it is. It passes some criteria, but it binds too little of the kernel for IMP-02 and IMP-06, and its safe API hides the types we would need to extend it.

## Question and pass criteria (work breakdown, SP-1)

Does the `opencascade` crate build on all three platforms and expose topology, surface evaluation and tessellation control?

| Criterion | Result | Verdict |
| --- | --- | --- |
| Builds in CI on Windows, macOS and Linux | Linux: builds and runs (GitHub runner: 32 to 51 min cold). macOS (Apple silicon): builds and runs (8 to 9 min cold). **Windows: the kernel builds (about 48 min), but `opencascade-sys` 0.3.0 does not compile under MSVC 14.51 (Visual Studio 2026).** The errors are `C2440` in the crate's own `include/b_rep.hxx` (lines 16, 21, 30): `cannot convert from 'opencascade::handle<Geom_Surface> *' to 'std::unique_ptr<Handle_Geom_Surface>'` (and the same for `Geom_Curve` and `Poly_Triangulation`). That is a defect in the crate's C++ glue, not in OpenCASCADE. All three platforms first failed on CMake 4 (see Platform notes). | **Not met: the crate does not build on Windows as published.** |
| Faces, edges, surface types readable | Edge type: yes (safe API). Face surface type: not in the safe API, but readable from `opencascade-sys` as a class name (`Geom_CylindricalSurface`, and so on). Analytic parameters (radius, axis, cone angle), UV bounds and a `GeomAbs_SurfaceType` are not bound. | **Partly met.** |
| Tolerance controls triangle count | Yes: cylinder 124 to 396 triangles, sphere 1,504 to 15,018, filleted box 628 to 3,492 as tolerance goes 0.1 to 0.01 mm. Only linear deflection is exposed; angular deflection is fixed at the default. | **Met, with limits.** |
| Same file gives identical IDs twice | Yes on both Linux and macOS: identical topology digest over 5 separate processes for all 8 fixtures and a 300-solid file, and after a STEP write and re-read. IDs are `TopExp_Explorer` order. **Across platforms** the topology digests match for 8 of 9 files; `rotary_blank_drilled` differs because computed face areas differ in the 7th digit (face 0: 4466.888129 on Linux, 4466.888500 on macOS) while face order, surface types and edges are the same. See "Cross-platform differences". | **Met** per platform. Cross-platform IDs match; cross-platform numbers do not match exactly. |
| Malformed file returns an error | Empty, garbage, truncated, header-only, dangling reference and missing file: clean error. But a mutation fuzz of 450 corrupted files gave **155 segfaults** inside the STEP reader (`StepGeom_Vector::Orientation`, a null handle). | **Not met** in-process. |

Spike rules say a failed criterion triggers the fallback decision, not a longer timebox. This record is that decision.

## What was measured

All numbers are from one Linux x86-64 container (4 cores) unless stated. Fixtures: box, cylinder, cone, sphere, torus, filleted box, a rotary blank with a radial drilled hole, a B-spline face, and 300 filleted boxes (7,800 faces). **Every fixture was written by OpenCASCADE itself**, so this is not a test of real STEP exports from other CAD tools (see Limits).

### `opencascade` 0.3.0 and `opencascade-sys` 0.3.0 (OpenCASCADE 7.8.1, built from source)

- **Build:** 22 min on the container, 38 min on a GitHub Linux runner, 8 min on a GitHub macOS runner (Apple silicon), 48 min on a GitHub Windows runner. About 260 MB of static libraries, 30 MB binary. Unavoidable CMake C++ build confined to one crate, as the architecture expects.
- **Walk and surface types:** all 8 fixtures walk correctly. Surface classes seen: plane, cylinder, cone, sphere, torus, B-spline.
- **Safe API gaps:**
  - `Mesh` merges all faces into one list. There is no face index per triangle, so face selection on a tessellation (UI-04, IMP-02) cannot be built on it.
  - Mesh UVs are rescaled to 0 to 1 per face, which loses the true parameters.
  - `Face::normal_at` returns an unnormalised vector (length about 3 on a radius 3 fillet).
  - No access from the safe types down to the raw handles (`pub(crate)`), so extending it from outside is not possible.
- **Sys-crate gaps:** about 1,700 lines of hand-picked bindings. No `BRepAdaptor_Surface`, no `BRepTools::UVBounds`, no surface `D0` or `D1`, no angular deflection or parallel flag for the mesher.
- **Read time:** 300 solids and 7,800 faces: 5.0 s read, 171 MB peak; with a walk and two tessellations, 20.9 s, 253 MB.
- **Tolerance is not a bound on deviation.** Worst triangle-centre deviation from the exact surface at the requested tolerance 0.1 / 0.01 mm: cylinder 0.043 / 0.0044 mm, sphere 0.073 / **0.0173 mm**. The sphere at 0.01 exceeds its tolerance by 1.7 times. IMP-06 verification must measure deviation, not trust the setting.
- **Cross-platform differences (Linux x86-64 against macOS arm64, same OpenCASCADE 7.8.1).** Face and edge order, surface types and edge types are identical. Numbers are not: the trimmed-cylinder areas of the drilled blank differ by 3.7e-4 mm² on a 4,467 mm² face (BRepGProp integration over B-spline-trimmed edges), and the tessellation of the same file at 0.1 mm gives 282 nodes and 276 triangles on Linux but 286 and 280 on macOS (identical at 0.01 mm). Pure analytic shapes matched exactly. So kernel tessellation and kernel-computed areas cannot back byte-identical results across platforms (NFR-06 is stated per software version and project, and GC-07 per post, so it holds if the kernel output is treated as input data, but golden toolpath tests that sit on kernel tessellation need a tolerance on non-analytic faces). Our own face IDs and any digest must not be derived from kernel floating-point text: the harness digest also prints signed zeros (`-0.0000`), which would be a hazard.
- **Silent acceptance:** a file with an unknown entity type loads with exit 0 and an empty shell (0 faces). The importer must treat 0 faces as an error.
- **Diagnostics:** OpenCASCADE prints errors and statistics to standard output through a global printer. Another reason to keep it in a child process.

### Thin in-house shim (about 100 lines of C++ over the same OpenCASCADE build)

Built and run on Linux. It adds exactly what the safe API lacks:

- Surface type and analytic parameters: cylinder radius 10, cone semi-angle 0.2606 rad (matches atan(8/30)) and apex, sphere radius, torus 20 / 10.
- UV bounds for every face.
- Point and unit normal at (u, v), flipped for reversed faces. On the B-spline fixture the centre point is (10, 10, 2.75), which is the exact value for the control net.
- Mesher with linear and angular deflection and a parallel switch. Angular deflection 0.5 to 0.1 on the sphere at 0.1 mm: 1,504 to 8,002 triangles.

This answers the SP-9 input for B-rep faces in principle: exact evaluation is available from the kernel. Closest-point projection (`GeomAPI_ProjectPointOnSurf`) and ray hits (`BRepIntCurveSurface_Inter`) are already bound in `opencascade-sys`.

### Fallbacks tried

| Candidate | Result |
| --- | --- |
| `cadrum` 0.9.0 (MIT, OpenCASCADE 8.0.1, prebuilt static binaries) | Builds in 10 s. Good API: `SurfaceKind` with parameters, per-triangle `face_ids`, angular deflection, `Face::project`. But: reads only solids (the B-spline face gives 0 solids, silently); **face IDs are pointer addresses and differ every run**; mutation fuzz gave **137 segfaults and 12 hangs** in 450 files, so the newer kernel has the same reader fault; its build downloads a binary from one maintainer's release page, and in this container the download failed on the TLS proxy and had to be fetched by hand (a problem for NFR-09 and for reproducible builds); repository carries `CLAUDE.md` and `AGENTS.md`, 41 releases in six months (NFR-14). |
| `brepkit-io` 4.1.48 (pure Rust, AGPL-3.0-only) | **No crashes in 450 mutated files** (209 read, 241 clean errors). Exposes analytic surface structs and evaluation in Rust, which is what `SurfaceSource` wants. Reads 6 of 8 fixtures. **Fails the full sphere** ("empty wire"), **returns 0 solids for the shell-only B-spline face without an error**, and accepted two corrupted files that OpenCASCADE rejected. Its reader says it supports "the entity types produced by our STEP writer"; coverage of real CAD exports is unverified. Torus at 0.01 mm gave 112,124 triangles against OpenCASCADE's 20,000. First release Aug 2026, `CLAUDE.md` in the repository. Same 4.4 s read of the 300-solid file. |

## Options

| Option | For | Against |
| --- | --- | --- |
| A. `opencascade` as is | Least code. | Cannot give face-to-triangle mapping, surface parameters, UV bounds or angular control. Safe types cannot be extended from outside. Weakest scored pick (66) with no dependents. |
| B. **Thin in-house wrapper over OpenCASCADE** (recommended) | Proven here to give everything IMP-02 and IMP-06 need. Mature kernel. Our own face numbering and error model. Fits the architecture's named fallback. | We own a C++ shim and the OpenCASCADE build (22 to 48 min cold, 260 MB, CMake workarounds on new runners). Needs caching in CI. Crash isolation is still needed. |
| C. `cadrum` | Fast build, good API. | Findings above: unstable IDs, solids only, same crashes, binary download, AI-risk flags. |
| D. `brepkit-io` | No crashes, pure Rust, exact surfaces, no C++ toolchain. | Unproven on real STEP files, fails two of eight fixtures, young, `CLAUDE.md`. AGPL-3.0-only suits INT-09 but rules out a later dual licence. |

## Consequences if accepted

- **Architecture section 7 and 13** (STEP kernel row and open decision) are updated to name option B, in the same change that accepts this record.
- **Slice 11 design refinement** is settled: shim over `occt-sys`'s OpenCASCADE build (or our own CMake), deterministic face and edge numbering from `TopExp_Explorer` order, an `ImportWorker` child process with timeout and a zero-face check.
- **Windows is the open risk for option B.** The published crate does not build there, so option B means our own bridge, which must build with current MSVC. That is a reason to prefer B over A, not a reason to expect it to be free. Budget a Windows spike before slice 11 and do not adopt `opencascade-sys` just for its build glue.
- **Slice 0 CI**: set `CMAKE_POLICY_VERSION_MINIMUM=3.5` on Windows and macOS; cache the OpenCASCADE build per platform; path-filter so only `openchuck-import` rebuilds it. No `-sys` crate may leak into the library family (the xtask rule already forbids it).
- **NFR-07** is met for STEP by process isolation, not by trusting the reader. Fuzz the worker in nightly CI with the mutation script from the spike.
- **IMP-02 conditional waiver:** SP-9 can still decide, but exact evaluation through the shim works for plane, cylinder, cone, sphere, torus and B-spline on this evidence. Do not apply the waiver yet.
- **NFR-10 / INT-09 licence check, not decided here:** `opencascade`, `opencascade-sys` and `occt-sys` are LGPL-2.1 and OpenCASCADE has an LGPL exception text. Whether static linking into an AGPL application needs anything beyond shipping source is for you to confirm. LGPL-2.1 section 3 names GPL version 2 only, which may matter next to AGPL v3.
- **NFR-14 records** (pinned versions, survey scores, justification): `opencascade` / `opencascade-sys` 0.3.0 (66; tried, not adopted), `occt-sys` 7.8.1 (build only), `cadrum` 0.9.0 (63; rejected), `brepkit-io` and companions 4.1.48 (60; watched).

## Limits of this spike

- No STEP file from another CAD system was available offline, so every fixture came from OpenCASCADE's own writer. Re-reading its own output flatters it. **Before slice 11, collect exports from at least FreeCAD, Fusion and SolidWorks** (small parts with cylinders, cones, B-spline faces and holes) and rerun the walk. This also decides `brepkit-io`.
- Unit handling (IMP-03), re-export face matching (IMP-07, PRJ-07) and assemblies with transforms were not tested.
- The fuzz is random byte mutation, not a coverage-guided fuzzer. It shows the reader is unsafe; it does not bound how many faults exist.
- Nothing of ours has been built or run on Windows: the crate's bridge fails to compile there (above), so the fixtures, determinism, fuzz and shim results are Linux and macOS only. Before this record is accepted, build our own shim and a minimal reader on Windows (a small Windows build spike). A compiler older than MSVC 14.51 might build the crate as published; that was not tried, and would not fix the other gaps.
- The shim is shown working on Linux x86-64 (local) only. In CI the shim step failed on Linux and macOS because the second build pass did not recompile the shim (a harness fault: forcing the build script to rerun would fix it); the shim was not verified on macOS or Windows.
- Performance figures come from one container and the GitHub runners and are not tuned. Cold OpenCASCADE builds on hosted runners took 32 to 51 min (Linux), 8 to 9 min (macOS arm64) and about 48 min or more (Windows) for one crate, so CI needs a build cache from day one.

## Platform notes from CI

- GitHub's Windows and macOS runners now ship CMake 4.x, which rejects `cmake_minimum_required` below 3.5 in OpenCASCADE 7.8.1. `CMAKE_POLICY_VERSION_MINIMUM=3.5` works around it.
- The Windows runner uses Visual Studio 18 (MSVC 14.51), a toolchain newer than the one the crates were tested with, and `opencascade-sys` 0.3.0 fails to compile there (error C2440, above).
- A build script that needs another crate's output cannot rely on ordering. Build the shim in a separate step after the kernel exists.
