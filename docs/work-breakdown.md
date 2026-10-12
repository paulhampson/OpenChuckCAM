# OpenChuckCAM – Phase 1 Work Breakdown Structure

Revision 5, 11 Oct 2026. Reconciled with the architecture as updated on 10 Oct 2026 (repository structure, scored dependencies, independent kinematic oracle, traceability), the Software Verification Plan and the Initial Requirements (crates named `openchuck-*`). It covers all 79 P1 requirements (TL-03 and GC-11 moved to P1) in 21 vertical slices (0–20), preceded by 13 timeboxed spikes. The order follows the architecture's build order: guardrails, spikes, a thin P1 slice (STL, project file, one wrap-family strategy, simulator deviation, both posts, golden files, viewport), then P1 breadth.

## P1 assessment

79 of 120 requirements are P1: IMP 6, UI 9, SET 4, TL 7, OP 16, SIM 7, GC 11, PRJ 6, NFR 13. The other 41 (P2 and P3) are out of this phase: re-import, DXF and SVG, Y-axis, multiple setups, holders, material library, collision detection, in-app version control (`openchuck-vcs` is not built), library packaging and bindings, scripting, time estimates, G-code simulation, feature recognition.

Findings, after reconciling with the architecture:

| # | Finding | Handling in this plan |
| --- | --- | --- |
| 1 | Architecture section 11 says its build order was not reconciled with this WBS. | This revision follows it. Simulation moves earlier (slice 7) because the thin slice needs deviation, and roughing, rest and carry-over need the `StockModel` port the simulator implements. |
| 2 | Eight of the 16 OP requirements are separate strategies (OP-03 to OP-10), the largest block of P1 work. | Slices 12–18, ordered by dependency. Swarf (OP-07), pencil clean-up (OP-09), radial drilling (OP-10) and the setup sheet (GC-12) are the first candidates to move to P2 if the first release must shrink. |
| 3 | `openchuck-gcode` (G-code reader) serves GC-08 (P2) and SIM-09 (P3), but P1 verification needs it as a test tool for post round trips and replay. | Built in slice 6, limited to reading and replay. |
| 4 | Mesh selections apply to mesh models only; STEP is selected by face and edge (architecture 5.6). | Slice 10 covers meshes, slice 11 B-rep; the UI offers triangle selection only on meshes. |
| 5 | STL tolerance is fixed by the file, so gouge tolerance on STL parts is measured against that mesh. | The UI states this (slices 2 and 13). |
| 6 | The IR carries segment duration, so a total time estimate costs little, but SIM-08 (P2) owns estimates and GC-06 and GC-12 mention them. | Left out of P1: GC-06 and GC-12 now say times appear once SIM-08 is available. Slice 19 may still include IR-duration totals. |
| 7 | TL-03 (V-bit, chamfer, drill and custom-profile tools) moves to P1 on your decision, which also closes the OP-10 drill gap. It adds tool shapes that pathing, gouge checking and simulation must all handle. | Slice 4 adds the tool types, slice 5 defines one tool profile type for all of them, and SP-13 proves it for pathing, gouge checking and simulation. The requirements document, the architecture and the verification plan now all list TL-03 as P1. |
| 8 | The survey's later pass now scores the template engine (minijinja 94), serialisation (serde 93, toml\_edit 93), PDF (printpdf 90) and the test, fuzz, logging and benchmark crates. NURBS evaluation still has no proven crate (curvo 71), and order keys, units and undo stay in-house. | The scores are a screen, not proof. Each pick still gets an NFR-14 record, a justification and a short spike where first needed: slice 0 (test, fuzz, logging, benchmark), SP-8 (serialisation), SP-9 (NURBS), SP-10 (template engine), slice 19 (setup sheet generation). Pre-1.0 crates and release candidates are pinned. |
| 9 | API churn in parry3d, wgpu, egui and i\_overlay. | Exact version pins and an upgrade cadence set in slice 0; third-party types kept out of public APIs. |
| 10 | Decided: all crates are AGPL, including `openchuck-types` and `openchuck-post`. The contributor agreement for a later exception is unsettled. | Slice 0 records the AGPL decision and settles the contributor agreement before outside contributions. |
| 11 | The gcode replay needs machine kinematics, but `openchuck-gcode` depends only on `openchuck-types` and A-axis management lives in `openchuck-path`. | Forward kinematics live once in openchuck-types and are shared by the path library, the post and the replay; independence comes from the test-only kinematic oracle (finding 16). Confirm in slice 6. |
| 12 | Gating risks: the STEP kernel (`opencascade` scores 66 and calls itself a work in progress), G93 support on the Z1, egui and wgpu churn. | Spikes SP-1, SP-12 and SP-2, each with a named fallback. |
| 13 | Architecture section 12 settles on one monorepo (crates/, apps/, posts/, testdata/, docs/, xtask/) and makes openchuck-gcode a fourth library-family crate, so the library rule now covers it. | Slice 0 builds this layout, the xtask checks and path-filtered CI. Each library must build and test alone without the C++ toolchain. |
| 14 | Section 13 now records the application crates as AGPL and the authoritative docs as living in the repository under docs/. Still open: whether test data needs Git LFS, and the packaging route for binaries. | Slice 0 records the decisions already taken (AGPL, docs in the repository) and a test-data size policy; the LFS question is revisited if the corpus grows; slice 20 decides the packaging route. |
| 15 | The survey now scores the release tools (cargo-release 79, release-plz 73, cargo-dist 70, git-cliff 80 for changelogs), but the architecture still asks for an NFR-14 justification before the first release and leaves the packaging route open. | Justified in slice 20 using those scores, with a release gate in xtask that blocks a tag when checks fail. |
| 16 | The path library, the post and the G-code reader share one machine model, so a replay could agree with a wrong A direction or offset. The architecture and verification plan add a test-only closed-form kinematic oracle, needed before any G-code with A words becomes a golden file. | Built in slice 0, written separately from the shared model. SP-3 is checked against it, and slices 3, 5, 6 and 13 use it. No golden file with A words is accepted before it exists. |
| 17 | Requirement IDs must appear in test names or attributes, and an xtask traceability report fails CI when a P1 requirement has no test. Switched on from slice 0 it would fail every early build. | Slice 0 builds the convention and the report. The fail rule applies to a P1 requirement when the last slice that touches it closes (see the note under the traceability table), and to all 81 at slice 20. |
| 18 | The verification plan defines five G-code gates: static, replay and LinuxCNC simulator in CI, then an air cut and a first cut on the Z1. The reference job set for gates 4 and 5 and the safe-practice checklist for first machine trials are not yet chosen or written. | Slice 6 builds gates 1 to 3 and chooses the reference job set. Slice 20 runs gates 4 and 5 (your hands-on step), records the G93 result, and includes the safe-practice checklist reviewed by whoever operates the Z1. |
| 19 | The plan sets acceptance limits (section 4.1) and reference hardware for the frame-rate check (section 4.2) as starting values, to be revised after the first measurements. | The limits are the pass criteria in the slice checklists. Slices 5, 7 and 8 and the 3-axis test cut revise them, and each change is recorded in the plan. |

## Spikes

The crate survey compiled and benchmarked nothing, so each risky layer is proved before it is relied on. A spike is throwaway: it answers one question against stated pass criteria, then ends. Timeboxes are proposals in working days.

| Spike | Question | Throwaway build | Pass criteria | If it fails | Run before | Status |
| --- | --- | --- | --- | --- | --- | --- |
| SP-1 STEP kernel (5 d) | Does the `opencascade` crate build on all three platforms and expose topology, surface evaluation and tessellation control? | STEP to B-rep walk, deterministic face and edge numbering, tessellation at two tolerances. | Builds in CI on Windows, macOS and Linux. Faces, edges and surface types readable. Tolerance controls triangle count. Same file gives identical IDs twice. A malformed file returns an error. | Thin in-house wrapper over the C++ kernel, cadrum as a justified risk under NFR-14, or brepkit-io (60; pure Rust, AGPL-3.0-only, 0.2 years old) as a spike candidate only. | Slices 0 and 11 | Decided: [0003](decisions/0003-step-kernel-route.md) accepted (option B, process isolation). Branch `spike/sp-1-step-kernel` has no RESULT.md; the record is the write-up. |
| SP-2 Viewport (3 d) | Does egui with `egui_wgpu` hold 30 fps on 1M triangles on all three platforms, with native graphics? | Window, wgpu mesh draw, orbit, one polyline layer, CPU BVH pick. Combine with SP-1 output for STEP to orbitable window. | 30 fps or better on the reference machine at 1M triangles. Builds on three platforms. Pick within budget. | Tauri with a JavaScript viewer (replaces `openchuck-viewport`) or iced. | Slice 8 | Partly done: [0002](decisions/0002-sp-2-viewport-spike.md) proposed. The 30 fps criterion is unmeasured (no GPU here). Keep or fall back is open. `spike/sp-2-viewport` has RESULT.md. |
| SP-3 Wrapped path to G-code (4 d) | Does a wrapped engraving path on a cylinder become correct A-word G-code that replays in the LinuxCNC simulator? | Fixed path to IR to hard-coded post to G-code; read back with the SP-5 reader; run in LinuxCNC. | Reached positions match the IR within tolerance and agree with the independent kinematic oracle. A wrap and unwinding behave. Compensated feed correct at the contact point. | Revise IR fields and A conventions before the IR is frozen. | Slices 0, 5, 6 | Result accepted: [0001](decisions/0001-sp3-wrapped-path-result.md), backed by the LinuxCNC runs in [0009](decisions/0009-linuxcnc-fixture-checks.md). Items 3, 4 and 6 to 8 stay open (OI-03, OI-04, OI-06 to OI-08). |
| SP-4 Ray-cast sweep (5 d) | Is continuous 4-axis feasible with `parry3d-f64` ray casts swept around the A axis? | On a real part, step A, cast along X, derive tool centres and tool angle, compare with an analytic surface. | Accuracy within 0.01 mm. Runtime figure for the reference part. Identical results at 1 and N threads. | Own BVH, or analytic-only first for the continuous family. | Slices 5, 13 | Result accepted: [0015](decisions/0015-sp-4-ray-cast-sweep.md); all three criteria met on smooth star-shaped parts; OI-07 closed. `spike/sp-4-ray-cast-sweep` has RESULT.md. OI-64 to OI-67 open for slices 5 and 13. |
| SP-5 G-code reader (1 d) | Does the `gcode` crate read A words and the LinuxCNC dialect? | Parse the SP-3 output and a hand-written file with A words, comments and modal codes. | Every code our posts emit is parsed with positions. | `g-code` crate, or a small in-house reader for the subset our posts emit. | Slice 6 | Result accepted: [0005](decisions/0005-sp-5-gcode-reader.md) (in-house reader). The LinuxCNC fixture needs four arc corrections (OI-46), recorded in [0009](decisions/0009-linuxcnc-fixture-checks.md). |
| SP-6 Simulation stock (5 d) | Rotary grid, dexel or voxel: which gives the best accuracy, speed and memory on a rotary part? | Three small implementations behind one `StockModel` interface. Also sketch how a collision query would run on each (COL-05, P2), and include a V-bit and a custom-profile tool in the sweep (SP-13). | Removed volume on cylinder, cone and sphere parts within the resolution error. Time and memory within budget at default resolution. Same results at any thread count. Ray and region queries supported. | Pick the next best; the interface keeps the choice swappable. | Slice 7 | Not started. No branch or record. Its budget is needed for SP-13 criterion 3 (OI-56). |
| SP-7 2D offsets (2 d) | Do `cavalier_contours` and `i_overlay` give robust offsets and booleans for unrolled regions, with arcs surviving back to A angles and G2 or G3? | Unroll a cylinder with a boundary and holes, offset several passes, convert back. | No failures on a corpus of awkward shapes. Arcs kept or approximated within tolerance. Deterministic. | `clipper2` C++ wrapper, or polylines only. | Slice 5 | Not started. No branch or record. |
| SP-8 Project format and order keys (2 d) | Which serialisation format and order-key scheme give clean diffs and merges? | Sample project in two candidate formats, real Git merges (reorder on one branch, edit another operation on the other). | Byte-identical re-save. One-line diff for a one-parameter edit. Conflict-free merges. Load then save is a no-op. | Second candidate format or key scheme. | Slice 1 | Result accepted: [0006](decisions/0006-sp-8-project-format.md) (TOML, per-item order keys). `spike/sp-8-project-format` has RESULT.md. |
| SP-9 Exact surfaces (3 d) | What does `SurfaceSource` need for B-rep faces, and who evaluates NURBS? | Points, normals and projections on plane, cylinder, cone, sphere and B-spline faces, by analytic Rust code and by the kernel, compared with tessellation. | Exact-versus-mesh differences measured. Decision recorded with a dependency assessment. | P1 uses tessellation for non-analytic faces and says so (the conditional waiver recorded in IMP-02). | Slice 5 | Result written: `spike/sp-9-exact-surfaces` (`spikes/sp9-exact/RESULT.md`); [0016](decisions/0016-sp-9-exact-surfaces.md) proposed, awaiting the owner (waiver not applied; OI-68 to OI-72). |
| SP-10 Template engine (2 d) | Can a candidate engine express rotary posts (modal state, A wrap, line numbers, feed mode, conditionals) deterministically? | The SP-3 post as a template, plus a Makera-style variant. | Both render identically on repeat runs. Errors name the line. Fixed-decimal number formatting checked (minijinja reached 3.0 on 8 Oct 2026, so pin the version). NFR-14 assessment recorded. | Second engine, or a minimal in-house template language. | Slice 6 | Not started. No branch or record. |
| SP-11 Swarf (3 d) | Can ruled-surface detection and flank tool placement be made robust? | Fit rulings on cylinder, cone, helicoid and a near-ruled spline; place a tapered tool; measure flank deviation. | Deviation within tolerance on ruled surfaces. Non-ruled surfaces refused. | Restrict P1 swarf to analytic ruled surfaces (cylinders and cones). | Slice 17 | Not started. No branch or record. |
| SP-12 Z1 conventions and G93 (2 d plus machine time) | What rotary conventions does the Z1 use, and does it accept inverse-time feed? | Inspect existing Makera machine profiles; write a short test program with A moves; check it by hand and in the LinuxCNC simulator, then run it as an attended air cut with no tool and no material (the bring-up exception in verification policy 6; hands-on with you). | Axis letter, direction, wrap, feed mode, header, footer and tool change documented, each marked confirmed. G93 answer recorded. | Keep unconfirmed values flagged in the profile and post. | Slices 3 and 6 | Desk part done: [0007](decisions/0007-sp-12-z1-conventions.md) proposed. Machine air cut outstanding (OI-37). Z1 values in [0013](decisions/0013-z1-profile-consensus.md) accepted, with four fields confirmed from use. |
| SP-13 Custom-profile tools (3 d) | Can one revolved-profile tool model drive tool-centre solving, the independent gouge checker and the simulator sweep for flat, ball, bull-nose, tapered ball, V-bit, chamfer, drill and custom-profile cutters? | A profile type (ordered line and arc segments revolved about the tool axis); drop-cutter against cylinder, cone and sphere test parts; the same profile swept in the SP-6 candidates. | Flat, ball and V-bit results match closed-form values within tolerance. A custom profile agrees with an equivalent standard tool. Sweep time within the SP-6 budget. Identical results at 1 and N threads. | Limit custom profiles to line and arc segments with a validity check, or exclude them from simultaneous strategies (OP-05) with a clear warning. | Slices 4 and 5. | Result accepted: [0012](decisions/0012-sp-13-tool-profiles.md). Criteria 1, 2 and 4 met; criterion 3 waits on SP-6 (OI-56). |

Spike rules:

- [ ] Code lives on a separate branch or repository and is never merged into the product
- [ ] Each spike ends with a short written result: question, method, numbers, decision, fallback status
- [ ] The result is reviewed before the dependent slice starts
- [ ] A failed spike triggers the fallback decision, not a longer timebox
- [ ] Every crate tried gets an NFR-14 record (survey-style score, justification, pinned version)
- [ ] Test parts and analytic oracles built for a spike are kept as fixtures

## Common review checks

These apply to every slice and are not repeated below.

- [ ] CI is green on Windows, macOS and Linux, including the C++ kernel once it is added
- [ ] Dependency rules hold: the library family (`openchuck-types`, `-path`, `-post`), with openchuck-gcode as a fourth member (architecture section 12), depends on nothing project-internal beyond its own members; only `openchuck-ui` and `openchuck-viewport` touch egui or wgpu; only `openchuck-import` touches the STEP kernel; no network crates; `cargo deny` is clean
- [ ] No third-party type appears in a public API, and each new dependency is recorded with a justification and a pinned version
- [ ] Tests exist for each requirement in the slice, with the requirement ID in the test name or attribute
- [ ] Same inputs give identical outputs at 1, 2 and N threads
- [ ] No `unwrap` or unchecked indexing in library code (lint); bad input gives an error, not a crash
- [ ] Project edits made in the slice go through commands and are undoable (UI-08, from slice 1)
- [ ] Design refinement notes and spike results are recorded, and the architecture doc is updated where it changed

* [ ] Each library builds and tests alone (cargo test -p \<crate>) without the C++ toolchain or GPU libraries (unless a recorded fallback exception says otherwise, architecture section 4); the xtask dependency-rule and SPDX licence-header checks pass; third-party versions are declared once in the workspace dependency table
* [ ] Path-filtered CI jobs run only what a change touches, and trunk-based rules hold: main stays releasable and merges only with CI green
* [ ] Where a slice measures deviation, kinematics, gouge, simulation, feed, determinism or fuzzing, it uses the acceptance limits in verification plan section 4.1, and any change to a limit is recorded in the plan

## Slice order and dependencies

1. Slice 0 (guardrails), then spikes SP-1, SP-2, SP-3, SP-5, SP-8 and SP-12 in parallel.
2. Thin P1 slice: slices 1 to 9 (project, STL, setup, tools, first strategy, posts, simulator, viewport, display and jobs). SP-9, SP-4, SP-7, SP-10 and SP-13 run before slices 4 to 6; SP-6 before slice 7.
3. Selection and STEP: slice 10 needs 8; slice 11 needs 10 and SP-1, SP-9.
4. Strategies: 12 and 13 need 5 and 10; 14 and 15 need 7; 16 needs 10; 17 needs SP-11; 18 needs 10.
5. Close: slice 19 needs 6; slice 20 needs everything.

## Slice 0 – Guardrails and shared data

**Goal:** a workspace, the shared data types and the neutral toolpath IR, with the application API and CLI running end to end on nothing, and the architecture rules enforced by CI.

**Requirements:** NFR-01, NFR-02, NFR-05, NFR-06, NFR-07, NFR-08, NFR-09, NFR-10, NFR-14, GC-01

**Spike inputs:** SP-3 (IR fields), SP-1 and SP-2 (platform builds).

**Design refinement:** confirm the monorepo layout (architecture section 12), the crate layout and the dependency rules against sections 3 and 4; fix the IR fields after SP-3; define the job context, error model and dependency policy (pins, upgrade cadence).

1. Monorepo layout from architecture section 12: `crates/` for the library crates, `apps/` for `openchuck-ui` and `openchuck-cli` (wiring only), plus `posts/`, `testdata/`, `fuzz/`, `benches/`, `docs/` and `xtask/`; `bindings/` is created later.
2. Crates as shells: `openchuck-types`, `-path`, `-post`, `-gcode`, `-geom`, `-import`, `-sim`, `-project`, `-app` and `-viewport` in `crates/`, with `-ui` and `-cli` in `apps/` (`openchuck-vcs` is P2 and not built); the test-only `openchuck-testkit` in `crates/` and the `openchuck-benches` package in `benches/` (decision record 0004).
3. Repository root: workspace `Cargo.toml` with every third-party version declared once in `[workspace.dependencies]`, committed `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, `LICENSES/` and `.github/` workflows; trunk-based branching.
4. `openchuck-types`: millimetre and degree types in double precision, mesh and B-rep data, stock, tool, machine profile, the neutral toolpath IR (tool tip, tool axis, explicit A, move kind, contact feed, contact path length, segment duration, tags to operation, tool and setup), job context (cancel token, progress sink), typed errors and the one shared forward-kinematics implementation (XYZA to tool tip); the data types are serde-serialisable (GC-01, NFR-05).
5. CI on Windows, macOS and Linux with path-filtered jobs: build, test, `clippy`, `rustfmt`, lints against `unwrap` and unchecked indexing, and `cargo deny` for licences and advisories. Each library builds and tests alone without the C++ toolchain or GPU libraries (NFR-01, NFR-07, NFR-10).
6. `xtask`: the dependency-rule check reading `cargo metadata` (every rule in architecture section 4 and the library-family rule in section 12), an SPDX and licence-header check, and a traceability report built from the requirement-ID tags (CI fails when a P1 requirement of a closed slice has no test); the LinuxCNC replay runner and release steps are added in slices 6 and 20 (NFR-09, NFR-10).
7. Library hygiene: each library has its own README, `license` field, changelog, rustdoc and an example, internal path dependencies carry version numbers, the four library crates share one version before 1.0, and the `openchuck-*` crate names are reserved early.
8. `openchuck-app` skeleton (commands, queries, event stream, job manager shell) and a CLI that runs end to end on an empty project.
9. Determinism harness: the same job at 1, 2 and N threads with hash comparison; rules for fixed chunking and ordered containers (NFR-06).
10. Test scaffolding: golden-file and property-test tooling, fuzz targets, a 1M-triangle benchmark model, the requirement-ID tag convention with a traceability report skeleton, and `testdata/` with the analytic oracle parts (cylinder, cone, sphere, helical groove, ruled surface), procedurally generated where possible, a README recording each model's source and licence, and a size policy (NFR-08).
11. Independent kinematic oracle (OP-05, OP-06, GC-09): a small test-only module that maps XYZA back to the tool tip in model space from first principles for rotation about X, written separately from the shared machine model. No G-code with A words is accepted as a golden file until it exists.
12. Dependency policy: every dependency recorded with a survey-style score, exact version pins and an upgrade cadence; record the survey score and NFR-14 justification for the test, fuzz, logging and benchmark crates (proptest, insta, assert\_cmd, cargo-fuzz, criterion, thiserror, anyhow, tracing).
13. CI tooling from the verification plan: nextest, cargo-hack (each library alone), cargo-semver-checks on the library family, cargo-audit, cargo-about for notices, llvm-cov for trends, Miri and sanitizers on unsafe and FFI code, a scheduled beta build and activity check, fuzzing for 1 minute per parser on pull requests and 1 hour nightly in a separate nightly job, and a benchmark regression flag at 10% on a dedicated runner.
14. Decisions recorded: AGPL for every crate, including `openchuck-types` and `openchuck-post` and the application crates (decided: AGPL), the contributor agreement approach, and the docs location (decided: the authoritative copy lives in `docs/` in the repository, and the Claude artifacts become working drafts).

**Review checklist** (closed 11 Oct 2026, [decision 0004](decisions/0004-slice-0-guardrails.md))

- [x] Each dependency rule, including the library-family rule, fails CI when deliberately broken
- [x] Each library builds and tests alone with no C++ toolchain; a change to `docs/` or `posts/` does not rebuild the STEP kernel (no kernel yet; its CI job arrives with the slice 11 wrapper, decision 0003)
- [x] The IR carries every field listed above and round-trips through serde
- [x] The CLI runs on an empty project and gives clear errors for bad input
- [x] The determinism harness catches a deliberately scheduling-dependent test
- [x] Every dependency has a record, a pinned version and one workspace declaration; the licence list is generated and the SPDX check passes
- [x] The licence, contributor agreement and docs-location decisions are written down
- [x] The kinematic oracle agrees with the shared machine model over a set of A angles including negative angles and the 0/360 seam, and fails when the A sign in the shared model is flipped
- [x] The traceability report flags a seeded gap in a closed slice and fails CI

## Slice 1 – Project documents, persistence and undo

**Goal:** create, save and reopen a project in a diff-friendly format, with every edit undoable.

**Requirements:** PRJ-01, PRJ-02, PRJ-03, PRJ-04, PRJ-05, UI-08

**Spike inputs:** SP-8 (format and order keys).

**Design refinement:** file layout from architecture 5.6 (project, setups, one file per operation, selection sets, machine), how order keys are stored, schema versioning, the cache location and generated ignore file, and the command pattern for undo.

1. Project model in `openchuck-project`: project, setups, operations, selection sets, machine reference and tool snapshots, with stable unique IDs and explicit order keys (PRJ-05).
2. Text persistence with stable key ordering and formatting, split into small files (PRJ-01), as TOML with per-item order keys, the approach chosen after SP-8 ([decision 0006](decisions/0006-sp-8-project-format.md)).
3. Model files referenced by relative path with a content hash checked on load (PRJ-02).
4. No generated data in the project; cache directory with a generated ignore file (PRJ-03).
5. Schema version in every file and a migration framework with a real fixture migration (PRJ-04).
6. Command, undo and redo stack in `openchuck-app`; every project edit is a command (UI-08).
7. Reserve descriptor fields (position, normal, area) in the selection schema so P2 matching needs no format change; not implemented.
8. CLI commands to validate a project and to load and save it; a fuzz target for the project loader (NFR-07).

**Progress:** items 1 to 8 are built ([decision 0008](decisions/0008-slice-1-project-model.md), accepted, on [decision 0006](decisions/0006-sp-8-project-format.md)). CI on pull request #10 was green on Windows, macOS and Linux.

**Review checklist** (closed 11 Oct 2026, [decision 0008](decisions/0008-slice-1-project-model.md))

- [x] Saving twice gives byte-identical files, and load then save is a no-op
- [x] A one-parameter edit changes one line in a plain diff; a reorder touches one item
- [x] The real Git merge test passes: two branches editing different operations merge without conflict
- [x] A fixture for every older schema version migrates (none exists before schema 1; the framework is proven on a test-only schema 0 fixture, decision 0008 item 12)
- [x] A changed model file is detected by its hash on load
- [x] Undo then redo restores the exact prior project

## Slice 2 – STL import and mesh checks (headless)

**Goal:** load an STL, check it, transform it and orient it to the rotary axis, all without a window.

**Requirements:** IMP-01, IMP-03, IMP-04, IMP-05

**Design refinement:** the imported model data and deterministic face and edge numbering scheme that STEP will share later; the `stl_io` adapter behind `ModelImporter`; where the unit boundary sits.

1. `ModelImporter` trait with an STL adapter for ASCII and binary (IMP-01).
2. Units read from the file where present, user override stored in the project, conversion to millimetres at the import boundary (IMP-03).
3. `openchuck-geom`: adjacency and edge map; checks for holes, non-manifold edges, flipped normals and degenerate triangles, with a report the UI can show (IMP-04).
4. Transforms (translate, rotate, scale, mirror with winding corrected) and orientation relative to the rotary axis, stored as project data (IMP-05).
5. Corpus tests and fuzzing of the STL reader (NFR-07).
6. CLI `inspect` command printing the model summary and check report.
7. A clear statement in the report that STL tessellation is fixed by the file and gouge tolerance will be measured against that mesh.

**Progress:** items 1 to 7 are built ([decision 0010](decisions/0010-slice-2-stl-import.md), accepted; slice closed 11 Oct 2026). Model references gain a units override and placement steps, so the project folder schema is now 2. Units: millimetres by default, with a question for the user when the file names another unit or the part's size is implausible (owner's choice, decision 0010 item 4). Open: OI-50 (more placement step kinds, deferred), OI-51 (the UI shows the statement, report and unit question, slice 8) and OI-52 (size thresholds from the machine profile, slice 3).

**Review checklist** (closed 11 Oct 2026, [decision 0010](decisions/0010-slice-2-stl-import.md))

- [x] ASCII and binary versions of one part give identical meshes
- [x] Truncated and corrupted files give clear errors, never a crash
- [x] Known-bad samples report the right counts for each defect type
- [x] A mirrored part has outward normals
- [x] A unit override survives save and load
- [x] The fuzz target runs for 1 minute on pull requests and 1 hour nightly (`stl_import` is in both workflows; the 1-minute run passed on pull request #12. **Waived by the owner, 11 Oct 2026:** the first nightly hour has not run yet, and any nightly failure is fixed when it occurs, with the failing input added to the corpus)

## Slice 3 – Machine, stock and setup

**Goal:** describe the machine, stock and part position so toolpaths have a checkable frame of reference.

**Requirements:** SET-01, SET-03, SET-05, SET-06

**Spike inputs:** SP-12 (Z1 conventions), SP-3 (A sign).

**Design refinement:** coordinate frames (part, setup, machine), the A-axis sign convention, the machine profile schema with the rotary axis orientation as a parameter (X now, Y later under SET-02, never hard-coded), and how clearance geometry is represented.

1. Machine profile in `openchuck-types`: rotary axis orientation, A range, continuous or limited, linear travel limits, maximum feeds, clearance geometry (SET-01, SET-06); stored separately from projects, with a fuzz target for the profile file (NFR-07).
2. Makera Z1 profile from SP-12, each value marked confirmed or unconfirmed.
3. Stock as cylinder, box, or model bounding box plus offset, with a plain material name (SET-03).
4. Setup in `openchuck-project`: part position relative to the rotary centreline and work coordinate system, with a work offset reference such as G54 (SET-05).
5. Clearance geometry: safe cylinder around the axis and safe Z, validated against stock and part (SET-06).
6. All edits as undoable commands; CLI `validate` reports setup problems.
7. Frame conversion tests across part, setup and machine.

**Progress:** items 1 to 7 are built and slice 3 is closed ([decision 0011](decisions/0011-slice-3-machine-stock-setup.md), accepted 11 Oct 2026, with [decision 0013](decisions/0013-z1-profile-consensus.md)). The project keeps a copy of its machine profile in `machine.toml` and setups gain part, stock, material, part and work offsets and clearance, so the project folder schema is now 3. The Makera Z1's rotary axis, A direction, A range and feed limits are confirmed by the owner; the reference point, travel and clearance stay unconfirmed (OI-57), and the air cut (OI-37) stays open as a cross check. Feed limits are per axis and the Z1 defaults follow [decision 0013](decisions/0013-z1-profile-consensus.md). Open: OI-53 (default machine library location), OI-54 (machine copy against the library), OI-55 (work zero off the axis, deferred) and OI-57.

**Review checklist** (evidence in decision 0011, "Verification", and decision 0013; ticked, slice closed 11 Oct 2026)

- [x] The A-axis direction on a deliberately asymmetric part matches the documented convention and the independent kinematic oracle (shown by CLI report now, visually in slice 8). `req_set_01_a_direction_on_an_asymmetric_part_matches_the_oracle`; `openchuck-cli machine` and `report` state the direction. The owner confirmed the convention from use of the machine on 11 Oct 2026; the air cut (OI-37) stays open as a cross check
- [x] A kinematic test with the rotary axis set to Y passes at unit level, proving X is a parameter (`req_set_01_oracle_agrees_with_a_y_rotary_axis`)
- [x] The clearance cylinder always encloses stock and part, or the app warns (setup checks in `setup`, `validate` and `report`)
- [x] Every Z1 value is marked confirmed or unconfirmed (all seven marked, with sources; the rotary axis, A direction, A range and feed limits confirmed by the owner, the reference point, travel frame and clearance unconfirmed)
- [x] All three stock types survive save and load (`req_set_03_req_set_05_all_three_stock_types_survive_save_and_load`)

## Slice 4 – Tool library

**Goal:** define cutters and have each project carry its own copy of the tools it uses.

**Requirements:** TL-01, TL-02, TL-03, TL-04, TL-05, TL-06, TL-07, TL-11

**Spike inputs:** SP-13 (tool profiles; [decision 0012](decisions/0012-sp-13-tool-profiles.md), accepted).

**Status:** closed 12 Oct 2026. Design refinement in [decision 0014](decisions/0014-slice-4-tool-library.md), accepted 12 Oct 2026 (tool numbers belong to the project; machine tool sets under the new TL-11; the library is a folder; GC-14 agreed for slice 6). Items 1 to 8 are built, except drawing a profile in the UI (slice 8, where TL-03 closes). New open items OI-61 to OI-63; OI-54 moved to slice 8.

**Design refinement:** tool data model in `openchuck-types` (the assembly types for holder and collet exist but are P2), a single revolved-profile description that every tool type maps to, including V-bit, chamfer, drill and custom profiles (SP-13), the separate library store inside `openchuck-project`, how cutting data is keyed by tool and stock material name, and what happens when the library changes after a snapshot.

1. Tool profile in `openchuck-types`: one revolved-profile description (ordered line and arc segments) that every tool type maps to, as proven in SP-13. Standard types map by one rule: the cutting end (flat, ball, corner, taper, V or point) runs from the tip to the cutting radius, then a vertical side runs to the flute length; a cutting end taller than the flute length is rejected ([decision 0012](decisions/0012-sp-13-tool-profiles.md) item 3).
2. Tool types: flat end mill, ball end mill, bull-nose and tapered ball nose (TL-02); V-bit or chamfer, drill, and tools defined by a custom profile (TL-03); with diameter, flute length, overall length, shank diameter, flute count, corner radius, taper angle, tip angle, material and tool number (TL-04).
3. Custom profile validation by the monotone rule (along the profile neither radius nor height decreases; no zero-length segments; an arc's ends on one circle within 1e-6 mm and within one quarter turn; the first segment leaves the axis; every value finite), which makes a profile connected, non-self-intersecting and revolvable and rejects undercut tools in P1 ([decision 0012](decisions/0012-sp-13-tool-profiles.md) item 2, OI-57 closed), replacing the finiteness check `Tool::validate` makes today. Two ways to define a profile: import from a TOML text file with the `ToolProfile` fields and an arc given by `centre` or by `radius` (decision 0012 item 6, OI-58 closed; slice 4 fixes the field names), or draw and edit it in the UI (the editor with live preview arrives in slice 8); the profile text parser and the tool library import get corpus tests and fuzz targets (NFR-07).
4. Library stored separately from projects as a folder with one human-readable file per tool, with import and export (TL-01).
5. Cutting data per tool and material name: spindle speed, feed per tooth, plunge feed, default stepover and stepdown (TL-05).
6. Controller mapping: tool number, length offset, diameter offset (TL-06), held by the project's snapshot, not the library tool. A machine's default tool set in the library gives the number when a tool is added to a project for that machine; otherwise the next free number (TL-11).
7. Project snapshot of every tool in use; the snapshot wins on load and differences from the library are reported (TL-07).
8. Tool shape data in the form `openchuck-path` consumes; commands and CLI listing.

**Review checklist** (evidence in decision 0014, "Verification"; ticked, slice closed 12 Oct 2026)

- [x] Each tool type validates, and impossible values (corner radius above tool radius, a zero tip angle) are rejected (`req_tl_04_every_type_validates_and_carries_its_parameters`, `req_tl_02_req_tl_03_impossible_values_are_rejected`)
- [x] The profile of every standard type matches its closed-form silhouette for diameter and flute length (`req_tl_02_req_tl_03_standard_profiles_match_their_closed_form_silhouettes`, within 1e-12 mm at 301 radii)
- [x] A self-intersecting, disconnected or undercut custom profile is rejected with a message naming the segment (`req_tl_03_bad_custom_profiles_are_rejected_naming_the_segment`; in profile files, `req_tl_03_req_nfr_07_bad_profile_files_name_the_segment` and the profile corpus)
- [x] A project opens correctly with the library absent, and editing the library never silently changes an existing project (`req_tl_07_project_opens_with_the_library_absent`, `req_tl_07_editing_the_library_never_changes_a_project`, and the CLI scenario)
- [x] Export and re-import of the library is lossless and diffs cleanly (`req_tl_01_export_then_import_is_lossless`; one file per tool, a one-value edit changes one line of one file, `req_tl_01_library_folder_holds_one_file_per_tool`)
- [x] A tool added to a project for a machine takes its number from that machine's tool set, or the next free number the set does not keep for another tool (`req_tl_11_numbers_come_from_the_machine_tool_set_or_the_next_free`, and the CLI scenario adding `T4` through the Z1 tool set)

## Slice 5 – First strategy and the pathing core

**Goal:** `openchuck-path` produces its first real XYZA toolpath through a public API that a host other than this application could use.

**Requirements:** OP-01, OP-11 (core parameters), OP-13, OP-14, OP-15, OP-16

**Spike inputs:** SP-3, SP-4, SP-7, SP-9, SP-13.

**Design refinement:** the public API: `Strategy` trait with self-describing parameter schemas, the ports `SurfaceSource`, `StockModel` and `CollisionOracle`, and the kinematic mapping that takes the rotary axis as a parameter. Fix the `SurfaceSource` design using SP-9 so B-rep faces slot in later. Confirm the feed approach (contact feed plus duration in the IR) with SP-3 and SP-12, and use the tool profile type that every cutter shape maps to, as settled by SP-13 ([decision 0012](decisions/0012-sp-13-tool-profiles.md)). Tool-centre solving is a drop-cutter generic over the profile (vertex, facet and edge tests that need only the profile's lower envelope `h(r)` and its pieces), returning the tip and the contact point; normal-offset solving is a fast path for convex profiles only. Evaluate distances directly (the expanded quadratic loses about 1e-8 mm near the tool axis), and measure deviation along the surface normal, not vertically. The spike's drop took 0.35 to 3.2 ms per position on a 147,000-triangle mesh with a plain grid, so add a bounding structure (SP-4) and cull edges by height before searching.

1. Mesh `SurfaceSource` inside the library using `parry3d-f64` or an own BVH, per SP-4, with point, normal, projection, ray and distance queries.
2. Ports `StockModel` (with a simple analytic implementation for tests and small hosts) and `CollisionOracle` (default accepts everything and relies on the clearance geometry).
3. Rotary kinematic mapping to X, Y, Z and A with wrap, continuity, limits and unwinding (OP-15); the rotary axis is a parameter.
4. `Strategy` trait and registry with parameter schemas covering names, units, ranges, defaults and validation (OP-16).
5. Helical finishing on the whole model as the first wrap-family strategy (completed in slice 12), using the offset approach proven in SP-7.
6. Contact-point feed intent and segment duration written into the IR; compensated feed per minute derived by the post (OP-13).
7. Linking moves and retracts through the clearance geometry (OP-14).
8. Operation model in `openchuck-project`: strategy, tool, setup, selection, parameters, regenerate at any time (OP-01), and the core of OP-11 (tolerance, stepover, stock to leave, feeds, retract).
9. The independent gouge and coverage checker, used only in tests and written separately from the strategies, plus analytic-oracle tests on the fixture parts.
10. Mapping in `openchuck-app` from project objects to the library's plain data; the library never sees the project file.

**Review checklist**

- [ ] On a cylinder, helix pitch and A rotation per unit of X match the analytic values within tolerance
- [ ] A never jumps beyond the wrap rules, and no output contains NaN
- [ ] Contact-point feed matches an analytic check, with the formula documented
- [ ] Links stay inside the clearance geometry and never cross stock
- [ ] The independent checker reports zero gouge and full coverage on the oracle parts, and fails on a seeded fault
- [ ] Forward kinematics of every XYZA point returns the intended tool tip within 1e-6 mm and 1e-6 degrees, checked against the independent oracle
- [ ] Toolpath deviation stays within 0.01 mm (about 0.005 mm tessellation and 0.005 mm path fitting)
- [ ] On simultaneous moves the effective feed at the cutter contact is within ±2% of programmed
- [ ] Identical output at 1, 2 and N threads
- [ ] The library builds and runs in a small host program using only its public API

## Slice 6 – Post-processing and G-code round trip

**Goal:** an operation becomes G-code for the Z1 and for the LinuxCNC simulator, with the same IR behind both and an independent check that the G-code means what the IR meant.

**Requirements:** GC-02, GC-03, GC-04, GC-07, GC-09, GC-10, GC-11, GC-14 (GC-01 proven end to end; NFR-08 golden files)

**Spike inputs:** SP-3, SP-5, SP-10, SP-12.

**Design refinement:** the neutral machine program stage, the template engine chosen in SP-10, the post option schema, the Makera conventions from SP-12, and the kinematics for the replay (the shared model from openchuck-types, also used by `openchuck-path` and the post; independence comes from the kinematic oracle, not a second copy).

1. IR to neutral machine program: motion, spindle, coolant, tool change, work offset, comments.
2. `PostProcessor` trait with the template implementation first; one post selected per machine profile (GC-02).
3. Template engine with user-editable definitions loaded at run time (GC-03), with a fuzz target for definition files (NFR-07).
4. Post options: units, A letter, direction and wrap, header and footer, tool change, coolant, work offsets, line numbers, precision, maximum feed, feed mode (GC-04). Compensated feed in units per minute by default; inverse-time as an optional capability.
5. Makera Z1 post definition (GC-09) and LinuxCNC verification post definition (GC-10).
6. Byte-identical output through fixed number formatting (GC-07).
7. `openchuck-gcode`: reader and replay for the codes our posts emit, comparing positions, A travel, feeds and limits with the IR. The reader is in-house (SP-5, [decision 0005](decisions/0005-sp-5-gcode-reader.md)): f64 values from the source text, errors naming line and column, a known-code list kept in step with the posts, and a fuzz target. Its fixtures come from SP-5; LinuxCNC has read them ([decision 0009](decisions/0009-linuxcnc-fixture-checks.md)), and the post-subset file takes four corrected arc lines (OI-46).
8. Golden files on small hand-reviewed parts (NFR-08).
9. LinuxCNC simulator replay: documented configuration and a script that compares reached positions and limit faults with the expected toolpath.
10. CLI: regenerate G-code from a project file without the UI, with a report (GC-11); this is also the end-to-end test harness.
11. G-code verification gates 1 to 3 for the reference jobs in CI: gate 1 static checks (syntax, modal state, units, axis limits, feed on every cutting move, safe retract and tool change) as test code in openchuck-gcode; gate 2 replay within G-code rounding only (0.0005 mm and 0.001 degrees at three decimals, scaled to the post's decimal setting); gate 3 the LinuxCNC simulator through the xtask replay runner. Any change to kinematics, a post or a strategy reruns all three.
12. Choose the reference job set (parts, stock, tools) for gates 4 and 5 and write its Z1 programs.
13. Tool numbers before posting (GC-14): the application lists the numbers the program will use and the user confirms them, renumbers automatically in order of first use by the operations, or renumbers by hand; duplicates are refused and the choice is saved in the project as undoable commands. The CLI asks on a terminal and takes an option otherwise; the UI dialog comes with export in the UI (slice 19). The post checks numbers against the machine's range (OI-62).

**Review checklist**

- [ ] Re-running gives byte-identical G-code at any thread count
- [ ] You have reviewed the golden files line by line
- [ ] The `openchuck-gcode` round trip matches the IR on positions, A travel and feeds
- [ ] The helical test replays to completion in the LinuxCNC simulator with matching end position and no limit faults
- [ ] Editing a post definition changes the output without recompiling; an unknown variable gives an error naming the line
- [ ] Z1 output reviewed against SP-12 conventions, with an air-cut plan written (hands-on step for you)
- [ ] Gate 1 fails on seeded faults (missing feed, A beyond its limit, wrong units), and gate 2 agrees within 0.0005 mm and 0.001 degrees
- [ ] The reference job set is chosen and its Z1 programs pass gates 1 to 3
- [ ] Confirming keeps the tool numbers, automatic renumbering follows operation order, a number used twice is refused, and a second post after renumbering gives byte-identical G-code

## Slice 7 – Material removal simulation (headless)

**Goal:** the stock changes as the tool sweeps, results compare with the target, and stock carries from one operation to the next, all runnable from the CLI.

**Requirements:** SIM-03, SIM-05, SIM-06, SIM-07

**Spike inputs:** SP-6 (representation), SP-13 (tool profiles; [decision 0012](decisions/0012-sp-13-tool-profiles.md)). SP-6 times the profile sweep in its candidates and sets the budget SP-13's sweep-time criterion is judged against (OI-56).

**Design refinement:** the representation chosen in SP-6 behind `StockModel`, resolution settings, checkpoints for jumping to any operation or move, and results returned as plain data.

1. Stock model in `openchuck-sim` implementing the `StockModel` port, with rotation about the A axis (SIM-03).
2. Tool sweep along IR segments for every tool type, including V-bit, chamfer, drill and custom profiles, with A rotation.
3. Adjustable resolution trading accuracy for speed (SIM-07).
4. Carry-over: the stock left by operation N is the input to operation N+1 (SIM-06), with checkpoints.
5. Deviation against the target model, per-point remaining and overcut values, and worst-case figures (SIM-05).
6. Display meshes and result data for the viewport; no drawing code in this crate.
7. CLI simulation report; analytic-part tests asserting reported deviation, plus a comparison with a high-resolution brute-force run on a few small jobs.
8. Job-friendly: cancellable, progress events, deterministic at any thread count.

**Review checklist**

- [ ] On cylinder, cone and sphere cases removed volume matches the expected value within one simulation cell at the chosen resolution, and no worse than 0.02 mm at the finest setting
- [ ] A deliberately gouging path is reported as overcut with a location
- [ ] Operation N+1 sees the stock left by operation N
- [ ] Changing resolution changes time and error as documented; memory use is stated
- [ ] Results are identical at 1, 2 and N threads

## Slice 8 – Viewport and application shell

**Goal:** a cross-platform window where you open a project, see the model with the machine setup, and move around it.

**Requirements:** UI-01, UI-02, UI-03, UI-06, UI-07, NFR-02, NFR-03

**Spike inputs:** SP-2 (egui, wgpu, 1M triangles).

**Design refinement:** confirm egui and `egui_wgpu` from SP-2 or switch to its fallback; the view-model boundary between `openchuck-ui` and `openchuck-app` (the UI never imports an engine crate); picking via CPU ray casts against a BVH in `openchuck-geom`.

1. `openchuck-viewport`: render loop, camera, standard views, perspective and orthographic, shaded, wireframe and edge display (UI-02).
2. Rotary axis, work coordinate system, stock and part drawn together (UI-03).
3. `openchuck-ui`: window, menus, new, open, save, unsaved-change handling.
4. Project tree listing model, stock, setups, operations and tools; operations can be reordered, enabled, disabled and duplicated through commands (UI-06).
5. Parameter forms generated from strategy and tool schemas, with units, validation and defaults (UI-07), and a custom-profile editor with live preview: add, move and delete line and arc segments, with the same validation as file import.
6. Import dialog with the check report; model transform and orientation controls; undo and redo menu.
7. Scripted 60-second camera-path benchmark on the 1M-triangle model, run on the physical reference machines, with a software-rendered smoke test in CI because hosted runners typically have no real GPU (native rendering is used wherever a runner offers it) (NFR-03).

**Review checklist**

- [ ] Launches on Windows, macOS and Linux from CI artefacts using each platform's native graphics on the reference machines; CI may use software rendering (NFR-02)
- [ ] Orbit, pan and zoom hold a 95th percentile frame time of 33 ms or less over a scripted 60-second camera path on 1M triangles, on the reference hardware (4 cores, 16 GB RAM, integrated graphics, one machine per platform)
- [ ] Only `openchuck-ui` and `openchuck-viewport` depend on egui and wgpu, and the UI imports no engine crate
- [ ] The A-axis direction on the asymmetric test part matches the documented convention
- [ ] Tree reorder, enable, disable and duplicate are all undoable
- [ ] An invalid parameter shows an inline error and leaves the project unchanged

## Slice 9 – Toolpath and simulation display, playback and background jobs

**Goal:** generate and simulate without freezing the UI, then review exactly what was generated.

**Requirements:** SIM-01, SIM-02, SIM-04, UI-09, NFR-04

**Design refinement:** job manager and event stream behaviour, operation pipeline caches keyed by a hash of the operation's inputs and the engine version, invalidation of operation N and everything after it, and level of detail for long paths.

1. Job manager in `openchuck-app`: one worker pool, cancellation and progress for pathing, simulation and import (UI-09, NFR-04).
2. Operation pipeline with caches and chain invalidation, so editing operation N regenerates N and later operations only.
3. Draw toolpaths with rapid, link and cutting moves distinguished and per-operation visibility (SIM-01).
4. Show the tool at any point on the path with the workpiece rotated by A (SIM-02).
5. Draw simulation results: stock, remaining material and overcut colouring.
6. Playback: play, pause, step, speed, jump to an operation or move, using simulator checkpoints (SIM-04).
7. Operation add, edit, generate and regenerate flow in the UI.
8. Level of detail for paths with millions of segments.

**Review checklist**

- [ ] The UI stays responsive during generation within a stated frame-time budget
- [ ] Cancel stops within a stated time and leaves no partial result
- [ ] Editing operation N invalidates only N and later operations; unchanged operations reuse their cache
- [ ] The A rotation shown matches the post output and the LinuxCNC replay
- [ ] Jumping to move N shows the same position as the corresponding G-code line

## Slice 10 – Geometry selection on meshes

**Goal:** pick geometry on a mesh, save it as named sets, and make an operation cut only there.

**Requirements:** UI-04 (mesh), UI-05, PRJ-06 (mesh), OP-02

**Design refinement:** the selection model and geometry reference (mesh selections are triangle sets and apply to mesh models only), CPU ray-cast picking against a BVH in `openchuck-geom`, and how region constraints reach `openchuck-path`. Define the behaviour for a reference that cannot be resolved: flag the operation, never silently drop it.

1. Ray-cast picking and hover highlight; the viewport only draws the highlight.
2. Triangle region selection, including select-by-angle and flood fill, in `openchuck-geom` (UI-04).
3. Named selection sets in `openchuck-project` that operations reference by ID and that update when the set is edited (UI-05).
4. Persist selections so they survive save and load of the same model (PRJ-06).
5. Limit an operation to selected regions, with optional containment boundary and optional avoid and check surfaces (OP-02), wired to the helical strategy.
6. Selection panel listing sets and their contents.

**Review checklist**

- [ ] Select, save, close and reopen: the same set is highlighted
- [ ] Editing a set marks dependent operations for regeneration
- [ ] Flood fill and select-by-angle give expected counts on a reference mesh
- [ ] An avoid surface is never cut in the test case
- [ ] Picking on a 1M-triangle model stays within its time budget
- [ ] A reference to missing geometry shows a visible warning

## Slice 11 – STEP import and B-rep selection

**Goal:** open a STEP file, keep its exact faces and edges, and select them.

**Requirements:** IMP-02, IMP-06, NFR-13 (also extends UI-04 and PRJ-06 to B-rep)

**Spike inputs:** SP-1 (kernel), SP-9 (exact surfaces).

**Design refinement:** the kernel route chosen in SP-1 (a thin in-house wrapper over OpenCASCADE with an isolated import process, [decision 0003](decisions/0003-step-kernel-route.md)), the deterministic face and edge numbering, how exact evaluation reaches `openchuck-path` (analytic Rust for simple surface types, kernel-backed or NURBS crate otherwise, per SP-9), and the licence boundary (the LGPL kernel stays in `openchuck-import`).

1. Wrapped C++ kernel inside `openchuck-import` only, behind `ModelImporter`; exceptions converted to errors.
2. B-rep topology (faces, edges, vertices) with deterministic numbering, so the same file always gives the same IDs.
3. Units from the file with the user override (IMP-03 for STEP).
4. Tessellation at a user-controlled tolerance, with face and edge IDs mapped to triangles (IMP-06).
5. Viewport draws faces and edges distinctly; select faces and edges (UI-04).
6. B-rep `SurfaceSource` adapter giving exact points, normals and projections (IMP-02).
7. B-rep selections stored as face and edge IDs relative to the model's content hash (PRJ-06); the UI does not offer triangle selection on STEP models.
8. STEP corpus: curved, trimmed, multi-body and malformed files, plus fuzzing (NFR-07).
9. Build on all three platforms in CI (NFR-13); kernel licence recorded.

**Review checklist**

- [ ] A reference STEP part matches its CAD dimensions within tolerance
- [ ] Changing the tessellation tolerance changes triangle count and deviation as documented
- [ ] Face and edge IDs are identical after save and reload, and across two imports
- [ ] Exact versus mesh evaluation is measured on a curved surface and the difference recorded
- [ ] The kernel builds on all three platforms in CI
- [ ] Malformed STEP gives an error, not a crash

## Slice 12 – Baseline finishing: helical, axial, indexed

**Goal:** the three baseline finishing strategies work on selected regions with real operation parameters.

**Requirements:** OP-04 (completes slice 5 work); continues OP-11

**Design refinement:** how each strategy parameterises stepover and scallop, how region constraints enter the unrolled 2D development, and what the indexed form outputs for positioning moves.

1. Helical finishing on a selected region (spiral around the axis).
2. Axial finishing, parallel to the axis and stepping in A.
3. Indexed finishing at fixed A positions (3+1 positional), with positioning moves.
4. Tolerance, stepover and scallop controls; stock to leave per operation and per selection.
5. Lead-in and lead-out, ramping.
6. Strategy parameter schemas so the forms are generated; golden files for each.

**Review checklist**

- [ ] A property test shows every point of the selected region lies within stepover of the path
- [ ] The independent checker reports no gouging beyond tolerance on the reference parts
- [ ] Indexed positions land on the exact requested angles
- [ ] Per-selection stock to leave is respected
- [ ] All three replay cleanly in the LinuxCNC simulator

## Slice 13 – True simultaneous 4-axis finishing

**Goal:** A varies continuously with X, Y and Z to follow the surface, with the tool held near the surface normal.

**Requirements:** OP-05, OP-06, OP-12

**Spike inputs:** SP-4 (ray-cast sweep), SP-13 (tool profiles; [decision 0012](decisions/0012-sp-13-tool-profiles.md)).

**Design refinement:** the tool-axis policy (A can only tilt the tool in the plane perpendicular to the rotation axis), lead and tilt limit semantics, the continuous-family algorithm from SP-4 (sweep A in small steps, query the surface along X, offset for the cutter, link passes), and the gouge-check method.

1. Drive-curve generation on selected surfaces, using the exact `SurfaceSource` where available.
2. Tool-centre solving for every tool profile (flat, ball, bull-nose, tapered ball, V-bit, chamfer, drill, custom) with the generic drop-cutter proven in SP-13, which returns the contact point for any profile and any tool offset from the A axis; normal-offset solving is a fast path for convex profiles only, and a non-convex custom profile always uses the drop. Tool axis variation stays with SP-4 (OI-07).
3. A chosen to keep the tool as close to the normal as the plane allows, within user lead and tilt limits (OP-06).
4. Smooth continuous A, with limits respected and reversals minimised (OP-15).
5. Gouge protection against the model for the cutting tool at the set tolerance, with fix-up or rejection (OP-12; holders are P2). On STL parts, tolerance is measured against the mesh and the UI says so.
6. Feed compensation for simultaneous motion verified against the analytic value (OP-13).
7. Scenario tests on reference surfaces: sphere-like, tapered, S-curve.

**Review checklist**

- [ ] On surfaces with known normals, tool-angle error stays within the stated bound
- [ ] A reversal count is lower than the unconstrained version
- [ ] The independent checker reports zero gouge on the corpus
- [ ] Lead and tilt limits and A range are obeyed
- [ ] Simulator replay passes; deviation against the helical result on the same surface is recorded
- [ ] Results are identical at 1, 2 and N threads

## Slice 14 – Rotary roughing

**Goal:** remove bulk material in indexed and continuous forms, aware of what is already gone.

**Requirements:** OP-03; completes OP-11

**Design refinement:** layer definition for rotary parts using the wrap-family approach, how `StockModel` queries avoid air cutting, and entry and retract rules.

1. Stock-aware roughing using the `StockModel` port implemented by `openchuck-sim`.
2. Indexed (3+1) roughing: layers at fixed A positions.
3. Continuous roughing in rotary layers.
4. Stepdown, stock to leave, ramped entry and retract strategy, completing OP-11.
5. Containment to selected regions (OP-02).
6. Link moves checked against the remaining stock, not only the clearance cylinder (OP-14).
7. Simulation check and leftover report.

**Review checklist**

- [ ] Simulated leftover is uniform and matches stock to leave within tolerance
- [ ] A second roughing pass does not re-cut already removed material
- [ ] Indexed and continuous forms reach the same final stock envelope
- [ ] Every OP-11 parameter exists and has a test
- [ ] Link moves never cross the stock

## Slice 15 – Rest machining and pencil clean-up

**Goal:** clean up material that earlier operations could not reach.

**Requirements:** OP-09

**Design refinement:** how remaining material is found from the carried-over stock (SIM-06) against the target through the `StockModel` port, and the threshold that counts as worth cutting.

1. Remaining-material detection from simulated stock versus target.
2. Rest strategy limited to areas the previous tool's radius could not reach.
3. Pencil paths along concave creases.
4. Ordering and dependency handling between operations, using the pipeline invalidation from slice 9.
5. Tests on parts with tight inside corners.

**Review checklist**

- [ ] The rest operation cuts only where material remains above the threshold
- [ ] Worst remaining value drops after the rest operation
- [ ] Pencil paths follow the crease within tolerance without gouging
- [ ] Nothing to clean gives a clear message, not an empty or odd path

## Slice 16 – Edge and curve following

**Goal:** engrave or carve along selected edge chains or curves projected onto the surface.

**Requirements:** OP-08

**Design refinement:** the `CurveSource` trait, the projection method, and how A and tool axis behave along a curve. This is a wrap-family strategy and reuses the unrolled-development approach from SP-7.

1. Edge chains from B-rep edges and mesh feature edges as curve sources.
2. Projection onto the surface, with depth control.
3. Tool-axis and A handling along the curve.
4. Engrave and carve modes with multiple passes, including V-bit engraving where line width follows depth.
5. Tests on open and closed chains on a curved surface.

**Review checklist**

- [ ] Engraved depth is constant within tolerance on a curved surface
- [ ] Chain direction and start point are controllable
- [ ] Edge selections persist across save and load
- [ ] Simulator replay passes

## Slice 17 – Swarf (flank) machining

**Goal:** cut ruled or near-ruled surfaces with the side of the tool.

**Requirements:** OP-07

**Spike inputs:** SP-11 (swarf).

**Design refinement:** the detection method and deviation tolerance from SP-11, which tool shapes can do flank cuts, and whether P1 is limited to analytic ruled surfaces if the spike falls back.

1. Detect ruled or near-ruled selected surfaces and fit rulings, reporting deviation.
2. Toolpath keeping flank contact along the rulings (continuous family).
3. Support for straight and tapered tools.
4. Warn or refuse when deviation exceeds tolerance.
5. Tests on cylinder, cone and helicoid surfaces.

**Review checklist**

- [ ] Flank deviation is within tolerance on ruled test surfaces
- [ ] A clearly non-ruled surface is refused with a clear message
- [ ] The independent checker reports no gouge; A limits are respected
- [ ] Simulation result matches the target

## Slice 18 – Radial drilling

**Goal:** drill holes at indexed A positions, selected from model geometry.

**Requirements:** OP-10

**Design refinement:** hole detection from cylindrical faces, how each hole's axis maps to an A angle, and whether canned cycles or expanded moves are output (expanded by default). It uses the drill tool type from TL-03, now P1 (finding 7).

1. Hole selection from model geometry, with axis, diameter and depth.
2. Indexed A positions per hole, ordered to minimise rotation.
3. Drilling moves with peck depth, as expanded moves.
4. Post option to output canned cycles.
5. Tests on holes at several angles.

**Review checklist**

- [ ] Hole positions and angles match the model within tolerance
- [ ] Peck and retract heights are correct and stay clear of the part
- [ ] Simulation shows the correct hole depth
- [ ] Simulator replay passes

## Slice 19 – Output options and setup sheet

**Goal:** export G-code in the groupings a shop uses, and a printable job summary.

**Requirements:** GC-05, GC-06, GC-12 (GC-14 in the UI)

**Design refinement:** file naming and grouping rules, comment content, the setup sheet layout, and the HTML and PDF generation approach, assessed under NFR-14 because the architecture does not score it. Decide whether IR-duration totals appear (finding 6); otherwise times are left out until SIM-08.

1. Output as one file for the project, or one per setup, operation or tool (GC-05).
2. Comments naming the operation and tool (GC-06).
3. Setup sheet as HTML and PDF listing tools, operations and stock, built as a module in `openchuck-app` (GC-12).
4. Export UI and CLI options, including the tool-number dialog (GC-14; the application side is built in slice 6).
5. Golden files for each grouping and a snapshot of the setup sheet content.

**Review checklist**

- [ ] Per-tool files together contain the same moves as the single file
- [ ] Comments are present and deterministic
- [ ] The setup sheet lists every tool used with its controller tool number
- [ ] The sheet opens offline and prints legibly

## Slice 20 – Hardening and first release

**Goal:** a release you can trust, with every P1 requirement traceable to a passing test or a recorded manual check.

**Requirements:** NFR-12, plus final checks on NFR-03, NFR-04, NFR-06, NFR-07, NFR-08, NFR-09, NFR-10

**Design refinement:** architecture review against what was built, with debt noted, and a check of the architecture's section 10 coverage table against the traceability report.

1. Safety notice (generated G-code must be verified before use) shown at export in the UI, printed by the CLI and available as an optional post header comment (NFR-12).
2. Performance pass against NFR-03 and NFR-04 on the reference machine.
3. Determinism run at 1, 2 and N threads on all sample projects.
4. Malformed-input sweep of every importer, the project loader and every other user-supplied file format (tool library, custom tool profile, machine profile, post definition), with at least 24 cumulative fuzz CPU hours per parser and no unresolved crash (NFR-07).
5. Licence review, third-party notices, SPDX headers and an offline run check (NFR-09, NFR-10).
6. Sample projects, including the Z1 reference part, end to end.
7. CLI end-to-end scenarios in CI, including undo and redo and cancellation mid-job.
8. Release tooling assessed under NFR-14 (`cargo-release` for the library family, `cargo-dist` or a plain build matrix for binaries, `release-plz` later) and the packaging route decided; separate tag schemes for library and application releases.
9. Release gate in `xtask`: dependency-rule check, `cargo deny`, golden G-code comparison and LinuxCNC replay must pass before a tag is cut.
10. Z1 air-cut procedure documented (your hands-on step) and packaging for all three platforms, including a Windows installer and macOS app bundle if the egui app needs them.
11. Gates 4 and 5 on the Z1 for the reference job set (your hands-on step, attended): an air cut at reduced feed (including a timed simultaneous XYZA move compared with the IR's predicted time), then a first cut in wax or foam. Cut a 3-axis test part first to measure the machine's own repeatability, then set the first-cut limit from it (start at ±0.1 mm). Record the G93 result and confirm the Z1 rotary conventions in the post definition.
12. Safe-practice checklist for first machine trials, written and reviewed by whoever operates the Z1.
13. Verification report for the release: results against the exit criteria, waivers, open defects, the commit covered, a written result for every spike, and the plan reviewed against what happened.
14. Traceability report generated from the requirement ID tags.

**Review checklist**

- [ ] Every P1 ID has a passing test or a recorded manual check in the traceability report
- [ ] A fresh install on each platform produces verified G-code from a sample STEP part
- [ ] The safety notice is present in the UI, the CLI output and the post option
- [ ] The full run works with no network access
- [ ] A deliberately failing check blocks the release tag
- [ ] The library crates pass a publish dry run, and the `openchuck-*` names are reserved
- [ ] Licences are compatible and the known-issues list is written
- [ ] Gates 1 to 5 have passed for the reference job set with results stored, and the G93 result is recorded
- [ ] No severity 1 or 2 defect is open, and the verification report names the commit it covers
- [ ] The safe-practice checklist has been reviewed by the Z1 operator

## Traceability: P1 requirement to slice

| Slice | P1 requirements | Spikes feeding it |
| --- | --- | --- |
| 0 | GC-01, NFR-01, NFR-02, NFR-05, NFR-06, NFR-07, NFR-08, NFR-09, NFR-10, NFR-14 | SP-3 |
| 1 | PRJ-01, PRJ-02, PRJ-03, PRJ-04, PRJ-05, UI-08 | SP-8 |
| 2 | IMP-01, IMP-03, IMP-04, IMP-05 | – |
| 3 | SET-01, SET-03, SET-05, SET-06 | SP-12, SP-3 |
| 4 | TL-01, TL-02, TL-03, TL-04, TL-05, TL-06, TL-07, TL-11 | SP-13 |
| 5 | OP-01, OP-11, OP-13, OP-14, OP-15, OP-16 | SP-3, SP-4, SP-7, SP-9, SP-13 |
| 6 | GC-02, GC-03, GC-04, GC-07, GC-09, GC-10, GC-11, GC-14 | SP-3, SP-5, SP-10, SP-12 |
| 7 | SIM-03, SIM-05, SIM-06, SIM-07 | SP-6, SP-13 |
| 8 | UI-01, UI-02, UI-03, UI-06, UI-07, NFR-02, NFR-03 | SP-2 |
| 9 | SIM-01, SIM-02, SIM-04, UI-09, NFR-04 | – |
| 10 | UI-04, UI-05, PRJ-06, OP-02 | – |
| 11 | IMP-02, IMP-06, NFR-13 | SP-1, SP-9 |
| 12 | OP-04 | SP-7 |
| 13 | OP-05, OP-06, OP-12 | SP-4, SP-13 |
| 14 | OP-03 | SP-6 |
| 15 | OP-09 | – |
| 16 | OP-08 | SP-7 |
| 17 | OP-07 | SP-11 |
| 18 | OP-10 | – |
| 19 | GC-05, GC-06, GC-12 | – |
| 20 | NFR-12 | – |

**Requirements that span slices.** A requirement closes when the last slice that touches it closes: TL-03 (slices 4, 8), UI-04 and PRJ-06 (10, 11), OP-02 (10, 12, 14), OP-11 (5, 12, 14), OP-13 and OP-15 (5, 13), OP-14 (5, 14), GC-14 (6, 19) and NFR-02 (0, 8). The cross-cutting NFR-03, NFR-04, NFR-06, NFR-07, NFR-08, NFR-09, NFR-10 and NFR-14 are tracked from slice 0 or their first slice but close at slice 20.
