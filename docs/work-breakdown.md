# OpenChuckCAM – Phase 1 Work Breakdown Structure

Revision 5, 11 Oct 2026. Reconciled with the architecture as updated on 10 Oct 2026 (repository structure, scored dependencies, independent kinematic oracle, traceability), the Software Verification Plan and the Initial Requirements (crates named `openchuck-*`). It covers all 83 P1 requirements (TL-03 and GC-11 moved to P1; TL-11 and GC-14 added by decisions 0014 and 0020; SET-09 and OP-21 added by decision 0035) in 23 vertical slices (0–22; slice 21 is added by decision 0035 and slice 22 by the owner's request for a redesigned interface, both run before slice 20), preceded by 13 timeboxed spikes. The order follows the architecture's build order: guardrails, spikes, a thin P1 slice (STL, project file, one wrap-family strategy, simulator deviation, both posts, golden files, viewport), then P1 breadth.

## P1 assessment

94 of 148 requirements are P1: IMP 6, UI 17, SET 5, TL 8, OP 17, SIM 7, GC 14, PRJ 6, NFR 14 (SUG-04, the setup sheet, repeats GC-12 and is not counted again). The other 53 (P2 and P3) are out of this phase: re-import, DXF and SVG, Y-axis, multiple setups, holders, material library, rotary acceleration limit, geodesic and spiral finishing patterns, collision detection, in-app version control (`openchuck-vcs` is not built), library packaging and bindings, scripting, time estimates, G-code simulation, feature recognition, the interface conveniences of decision 0038 (UI-17 to UI-24: command search, inch display, form aids, light theme, measure and section, undo history, start page, re-selecting a set) and its simulation review aids (SIM-11 to SIM-13).

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
| 17 | Requirement IDs must appear in test names or attributes, and an xtask traceability report fails CI when a P1 requirement has no test. Switched on from slice 0 it would fail every early build. | Slice 0 builds the convention and the report. The fail rule applies to a P1 requirement when the last slice that touches it closes (see the note under the traceability table), and to all 83 at slice 20. |
| 18 | The verification plan defines five G-code gates: static, replay and LinuxCNC simulator in CI, then an air cut and a first cut on the Z1. The reference job set for gates 4 and 5 and the safe-practice checklist for first machine trials are not yet chosen or written. | Slice 6 builds gates 1 to 3 and chooses the reference job set. Slice 20 runs gates 4 and 5 (your hands-on step) for generated programs, rechecks G93 on the firmware then installed (the SP-12 air cut found it ignored on 1.1.2), and includes the safe-practice checklist reviewed by whoever operates the Z1. |
| 19 | The plan sets acceptance limits (section 4.1) and reference hardware for the frame-rate check (section 4.2) as starting values, to be revised after the first measurements. | The limits are the pass criteria in the slice checklists. Slices 5, 7 and 8 and the 3-axis test cut revise them, and each change is recorded in the plan. |

## Spikes

The crate survey compiled and benchmarked nothing, so each risky layer is proved before it is relied on. A spike is throwaway: it answers one question against stated pass criteria, then ends. Timeboxes are proposals in working days.

| Spike | Question | Throwaway build | Pass criteria | If it fails | Run before | Status |
| --- | --- | --- | --- | --- | --- | --- |
| SP-1 STEP kernel (5 d) | Does the `opencascade` crate build on all three platforms and expose topology, surface evaluation and tessellation control? | STEP to B-rep walk, deterministic face and edge numbering, tessellation at two tolerances. | Builds in CI on Windows, macOS and Linux. Faces, edges and surface types readable. Tolerance controls triangle count. Same file gives identical IDs twice. A malformed file returns an error. | Thin in-house wrapper over the C++ kernel, cadrum as a justified risk under NFR-14, or brepkit-io (60; pure Rust, AGPL-3.0-only, 0.2 years old) as a spike candidate only. | Slices 0 and 11 | Decided: [0003](decisions/0003-step-kernel-route.md) accepted (option B, process isolation). Branch `spike/sp-1-step-kernel` has no RESULT.md; the record is the write-up. Built in slice 11a ([0027](decisions/0027-slice-11a-step-import.md), accepted); OI-20 and OI-27 closed there; OI-25 and OI-26 still open. |
| SP-2 Viewport (3 d) | Does egui with `egui_wgpu` hold 30 fps on 1M triangles on all three platforms, with native graphics? | Window, wgpu mesh draw, orbit, one polyline layer, CPU BVH pick. Combine with SP-1 output for STEP to orbitable window. | 30 fps or better on the reference machine at 1M triangles. Builds on three platforms. Pick within budget. | Tauri with a JavaScript viewer (replaces `openchuck-viewport`) or iced. | Slice 8 | Done: [0002](decisions/0002-sp-2-viewport-spike.md) accepted 14 Oct 2026, keep egui and wgpu (OI-11 closed). NFR-03 measured on 14 Oct 2026 on a Ryzen 5 5600X and RTX 2070 Super (Windows 11): p95 0.69 ms, 0.93 ms with all edges, against 33 ms (OI-09 closed by the owner on that margin; integrated graphics and macOS not run). Builds on Linux and Windows, CI on all three (OI-10 closed). OI-13 closed 14 Oct 2026 (feature edges, [0029](decisions/0029-slice-8-viewport-and-shell.md)). OI-12 (pick budget) closed 15 Oct 2026 ([0037](decisions/0037-slice-10-picking-and-selection-panel.md): ray cast 1 ms p95 and hierarchy build 2 s at 1M triangles; measured 4.4 µs and 1.0 s). Still open: OI-14 (combine with SP-1), deferred to slice 11. `spike/sp-2-viewport` has RESULT.md. |
| SP-3 Wrapped path to G-code (4 d) | Does a wrapped engraving path on a cylinder become correct A-word G-code that replays in the LinuxCNC simulator? | Fixed path to IR to hard-coded post to G-code; read back with the SP-5 reader; run in LinuxCNC. | Reached positions match the IR within tolerance and agree with the independent kinematic oracle. A wrap and unwinding behave. Compensated feed correct at the contact point. | Revise IR fields and A conventions before the IR is frozen. | Slices 0, 5, 6 | Result accepted: [0001](decisions/0001-sp3-wrapped-path-result.md), backed by the LinuxCNC runs in [0009](decisions/0009-linuxcnc-fixture-checks.md). Items 3, 4 and 6 to 8 stay open (OI-03, OI-04, OI-06 to OI-08). |
| SP-4 Ray-cast sweep (5 d) | Is continuous 4-axis feasible with `parry3d-f64` ray casts swept around the A axis? | On a real part, step A, cast along X, derive tool centres and tool angle, compare with an analytic surface. | Accuracy within 0.01 mm. Runtime figure for the reference part. Identical results at 1 and N threads. | Own BVH, or analytic-only first for the continuous family. | Slices 5, 13 | Result accepted: [0015](decisions/0015-sp-4-ray-cast-sweep.md); all three criteria met on smooth star-shaped parts; OI-07 closed. `spike/sp-4-ray-cast-sweep` has RESULT.md. Used by slice 5 and by slice 13 ([decision 0036](decisions/0036-slice-13-simultaneous-finishing.md), accepted), which closes OI-67 and does not build the climb for meshes (OI-146, deferred as a future optimisation). OI-64, OI-65 and OI-66 open. |
| SP-5 G-code reader (1 d) | Does the `gcode` crate read A words and the LinuxCNC dialect? | Parse the SP-3 output and a hand-written file with A words, comments and modal codes. | Every code our posts emit is parsed with positions. | `g-code` crate, or a small in-house reader for the subset our posts emit. | Slice 6 | Result accepted: [0005](decisions/0005-sp-5-gcode-reader.md) (in-house reader). The LinuxCNC fixture needs four arc corrections (OI-46), recorded in [0009](decisions/0009-linuxcnc-fixture-checks.md). |
| SP-6 Simulation stock (5 d) | Rotary grid, dexel or voxel: which gives the best accuracy, speed and memory on a rotary part? | Three small implementations behind one `StockModel` interface. Also sketch how a collision query would run on each (COL-05, P2), and include a V-bit and a custom-profile tool in the sweep (SP-13). | Removed volume on cylinder, cone and sphere parts within the resolution error. Time and memory within budget at default resolution. Same results at any thread count. Ray and region queries supported. | Pick the next best; the interface keeps the choice swappable. | Slice 7 | Result accepted: [0022](decisions/0022-sp-6-simulation-stock.md) (dexels; voxel fallback; rotary grid overcuts slot walls). Criteria met by dexel and voxel; the budget is agreed, so SP-13 criterion 3 is met (OI-56, OI-99 closed). Slice 7 built on it ([0025](decisions/0025-slice-7-simulation.md), accepted): OI-100 and OI-101 closed, OI-102 still open. `spike/sp-6-simulation-stock` has RESULT.md. |
| SP-7 2D offsets (2 d) | Do `cavalier_contours` and `i_overlay` give robust offsets and booleans for unrolled regions, with arcs surviving back to A angles and G2 or G3? | Unroll a cylinder with a boundary and holes, offset several passes, convert back. | No failures on a corpus of awkward shapes. Arcs kept or approximated within tolerance. Deterministic. | `clipper2` C++ wrapper, or polylines only. | Slice 5 | Result accepted: [0016](decisions/0016-sp-7-2d-offsets.md) (cavalier_contours for offsets, i_overlay for region booleans; criteria met; G2/G3 not usable with A). Branch `spike/sp-7-2d-offsets` has RESULT.md. Open items OI-68 to OI-70. |
| SP-8 Project format and order keys (2 d) | Which serialisation format and order-key scheme give clean diffs and merges? | Sample project in two candidate formats, real Git merges (reorder on one branch, edit another operation on the other). | Byte-identical re-save. One-line diff for a one-parameter edit. Conflict-free merges. Load then save is a no-op. | Second candidate format or key scheme. | Slice 1 | Result accepted: [0006](decisions/0006-sp-8-project-format.md) (TOML, per-item order keys). `spike/sp-8-project-format` has RESULT.md. |
| SP-9 Exact surfaces (3 d) | What does `SurfaceSource` need for B-rep faces, and who evaluates NURBS? | Points, normals and projections on plane, cylinder, cone, sphere and B-spline faces, by analytic Rust code and by the kernel, compared with tessellation. | Exact-versus-mesh differences measured. Decision recorded with a dependency assessment. | P1 uses tessellation for non-analytic faces and says so (the conditional waiver recorded in IMP-02). | Slice 5 | Result written: `spike/sp-9-exact-surfaces` (`spikes/sp9-exact/RESULT.md`); [0017](decisions/0017-sp-9-exact-surfaces.md) accepted 12 Oct 2026 (waiver not applied; OI-73 to OI-76 open, OI-72 closed by [0027](decisions/0027-slice-11a-step-import.md), accepted). |
| SP-10 Template engine (2 d) | Can a candidate engine express rotary posts (modal state, A wrap, line numbers, feed mode, conditionals) deterministically? | The SP-3 post as a template, plus a Makera-style variant. | Both render identically on repeat runs. Errors name the line. Fixed-decimal number formatting checked (minijinja reached 3.0 on 8 Oct 2026, so pin the version). NFR-14 assessment recorded. | Second engine, or a minimal in-house template language. | Slice 6 | Result written: `spike/sp-10-template-engine` (`spikes/sp-10-template-engine/RESULT.md`); criteria met for `minijinja` 3.0.0. Result accepted: [0019](decisions/0019-sp-10-template-engine.md) (13 Oct 2026; OI-38 closed; OI-83 to OI-87 open). |
| SP-11 Swarf (3 d) | Can ruled-surface detection and flank tool placement be made robust? | Fit rulings on cylinder, cone, helicoid and a near-ruled spline; place a tapered tool; measure flank deviation. | Deviation within tolerance on ruled surfaces. Non-ruled surfaces refused. | Restrict P1 swarf to analytic ruled surfaces (cylinders and cones). | Slice 17 | Result written: `spike/sp-11-swarf` (`spikes/sp-11-swarf/RESULT.md`); both criteria met, fallback not needed. Result accepted: [0021](decisions/0021-sp-11-swarf.md) (14 Oct 2026; OI-94 to OI-96 closed). Slice 17 built on it, lane Q ([decision 0061](decisions/0061-slice-17-swarf.md), accepted 17 Oct 2026): OI-97 and OI-98 close with it, the holder continuing as OI-237. |
| SP-12 Z1 conventions and G93 (2 d plus machine time) | What rotary conventions does the Z1 use, and does it accept inverse-time feed? | Inspect existing Makera machine profiles; write a short test program with A moves; check it by hand and in the LinuxCNC simulator, then run it as an attended air cut with no tool and no material (the bring-up exception in verification policy 6; hands-on with you). | Axis letter, direction, wrap, feed mode, header, footer and tool change documented, each marked confirmed. G93 answer recorded. | Keep unconfirmed values flagged in the profile and post. | Slices 3 and 6 | Done: [0007](decisions/0007-sp-12-z1-conventions.md) accepted 14 Oct 2026 after the air cut on firmware 1.1.2 (direction, wrap, feed rule and G93 confirmed; OI-03, OI-04 and OI-37 closed). Not exercised: the tool change (OI-41); machine Z of the axis still open (OI-60). Z1 values in [0013](decisions/0013-z1-profile-consensus.md) accepted, with four fields confirmed from use. Write-up on `spike/sp-12-z1-conventions` (never merged). |
| SP-13 Custom-profile tools (3 d) | Can one revolved-profile tool model drive tool-centre solving, the independent gouge checker and the simulator sweep for flat, ball, bull-nose, tapered ball, V-bit, chamfer, drill and custom-profile cutters? | A profile type (ordered line and arc segments revolved about the tool axis); drop-cutter against cylinder, cone and sphere test parts; the same profile swept in the SP-6 candidates. | Flat, ball and V-bit results match closed-form values within tolerance. A custom profile agrees with an equivalent standard tool. Sweep time within the SP-6 budget. Identical results at 1 and N threads. | Limit custom profiles to line and arc segments with a validity check, or exclude them from simultaneous strategies (OP-05) with a clear warning. | Slices 4 and 5. | Result accepted: [0012](decisions/0012-sp-13-tool-profiles.md). Criteria 1, 2 and 4 met; criterion 3 met by SP-6 ([decision 0022](decisions/0022-sp-6-simulation-stock.md), OI-56 closed). Slice 13 ([decision 0036](decisions/0036-slice-13-simultaneous-finishing.md), accepted) uses the drop and the normal-offset fast path for every tool type. |

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

## Slice status

State of each slice on `main` (16 Oct 2026). **Closed** slices are in `closed_slices` in `xtask/traceability.toml`, so a P1 requirement of theirs with no test fails CI. A closed slice may still note a measurement on the reference machines that has not been run yet (marked *benchmark pending*); that measurement is tracked as an open item and does not hold the slice open.

| Slice | State | Waiting on |
| --- | --- | --- |
| 0 to 7 | Closed | |
| 8 viewport and shell | In progress | Launch on native graphics on macOS and Linux (OI-129); first run of `ui-smoke.yml` after merge (OI-321) |
| 9 display, playback and jobs | Closed (16 Oct 2026), benchmark pending | Reference-machine measurements (OI-176) |
| 10 mesh selection | Closed (16 Oct 2026), benchmark pending | Pick budget on the reference machines (OI-173) |
| 11 STEP import | In progress | 11b built by lane O on `claude/lane-o-slice-11b-brep-selection` ([decision 0066](decisions/0066-slice-11b-brep-selection.md), accepted; project schema 12) and wired to lane Z's placed B-rep (decision 0056) in the same pull request (OI-242 and OI-119 closed); lane S11 ([decision 0112](decisions/0112-slice-11-closing-items.md), accepted) closes OI-243 (edge-pick hierarchy, measured), OI-245 (wider hovered and selected edges) and OI-332 (save and edit checks as jobs); OI-335 (the edge widths) closed with the owner's approval and a specification 7.4 row; OI-25 and OI-120 open; the owner closes the slice |
| 12 baseline finishing | Closed (16 Oct 2026) | |
| 13 simultaneous 4-axis finishing | Closed (15 Oct 2026) | |
| 14 rotary roughing | Built, pending merge | The checklist is ticked: per-selection stock to leave for roughing is built by lane E2 on `claude/lane-e2-roughing-stock-per-selection` ([decision 0058](decisions/0058-roughing-stock-to-leave-per-selection.md), accepted 17 Oct 2026; OI-261 deferred). Containment (item 5, lane L) is merged. The owner closes the slice |
| 15 rest machining and pencil clean-up | Closed (16 Oct 2026) | Follow-ups OI-189 to OI-193 |
| 16 edge and curve following | In progress | Lane K built the mesh feature edge side ([decision 0047](decisions/0047-slice-16-edge-following-mesh.md), accepted 16 Oct 2026, relayed by the orchestrator session at the owner's instruction); lane P built the B-rep edge side ([decision 0053](decisions/0053-slice-16-edge-following-brep.md), accepted 16 Oct 2026); stored edge selections (OI-204) are built by lane O ([decision 0066](decisions/0066-slice-11b-brep-selection.md), accepted; project schema 12) |
| 17 swarf | In progress | Lane Q built the library strategy on `claude/lane-q-slice-17-swarf` ([decision 0061](decisions/0061-slice-17-swarf.md), accepted 17 Oct 2026, relayed by the orchestrator session at the owner's instruction); lane Q2 wired it through the application on `claude/lane-q2-swarf-wiring` ([decision 0111](decisions/0111-swarf-through-the-application.md), accepted 18 Oct 2026, relayed by the orchestrator session at the owner's instruction, as revised): a compatible face set is required, the B-rep-only message, and the `swarf` reference job; gate 3 replays it once the pull request leaves draft (OI-238) |
| 18 | Not started | |
| 19 output options and setup sheet | In progress | Item 4 built by lane S on `claude/lane-s-slice-19-export-dialogs` ([decision 0071](decisions/0071-slice-19-export-dialogs.md), accepted); the owner closes the slice |
| 20 hardening and first release | In progress | Preparation is done; the rest runs last |
| 21 workholding and retaining tabs | Built, pending merge | 21a and 21b are merged (lanes U and V); 21c, application wiring, simulation, deviation, forms and the command line, is built by lane W on `claude/lane-w-slice-21c-wiring` ([decision 0045](decisions/0045-slice-21c-wiring.md), accepted 16 Oct 2026; OI-195 to OI-198 and OI-209); the fix for walls in the target (OI-209, closed) is merged from lane Y ([decision 0050](decisions/0050-walls-merge-micro-moves.md), accepted 16 Oct 2026, relayed by the orchestrator session at the owner's instruction); 21d ([decision 0104](decisions/0104-held-regions-and-tabs-in-the-interface.md), UI-25): the interface specification (sections 7.5 and 11.7) is written by lane 21d on `claude/lane-21d-workholding-ui-spec` and merged; its code and tests (item 3) are built by lane 21d on `claude/lane-21d-workholding-ui-code` (draft pull request; OI-325 closed, what remains is OI-334) |
| 22 user interface conformance pass | In progress | Reframed as a conformance pass ([decision 0105](decisions/0105-slice-22-conformance-pass.md), accepted). Step 1, the conformance report and component inventory, is written by lane X1 on `claude/lane-x1-slice-22-conformance-report` ([decision 0114](decisions/0114-slice-22-conformance-report.md), accepted 18 Oct 2026). The owner answered its eight questions (OI-338 to OI-341, OI-343, OI-344 closed); OI-342 (mock-ups, re-rendered by each lane's agent as its step 1) and OI-345 (the UI-13 wording) are open; OI-334 is mapped to lanes. Steps 2 to 4 are lanes X2 to X9 (Parallel work) |

## Slice order and dependencies

1. Slice 0 (guardrails), then spikes SP-1, SP-2, SP-3, SP-5, SP-8 and SP-12 in parallel.
2. Thin P1 slice: slices 1 to 9 (project, STL, setup, tools, first strategy, posts, simulator, viewport, display and jobs). SP-9, SP-4, SP-7, SP-10 and SP-13 run before slices 4 to 6; SP-6 before slice 7.
3. Slice 9 needs 8 for its drawing, playback and forms (items 3 to 8); its job manager and operation pipeline (items 1 and 2) need only `openchuck-app` and can start with slice 8.
4. Selection and STEP: slice 10 needs 8 for picking, hover and the panel (items 1 and 6); its selection model, flood fill, sets, persistence and region constraint (items 2 to 5) are headless and do not wait. Slice 11 needs SP-1 and SP-9 for the importer, topology, tessellation, `SurfaceSource` adapter, corpus and CI (items 1 to 4, 6, 8 and 9); only the viewport drawing and B-rep selection storage (items 5 and 7) need 8 and 10.
5. Strategies: 12 and 13 need 5 and 10, and 12 needs the shared entry module, built 15 Oct 2026 as a preliminary change ([decision 0026](decisions/0026-entry-moves.md), accepted; `openchuck-path` `assemble::entry`). 14 needs 7 and the entry module, and its containment (item 5) needs 10. 15 needs 7 and the pipeline of slice 9 (items 1 and 2). 16 needs 10 for mesh feature edges and 11 for B-rep edges. 17 needs SP-11 and 11, because swarf is B-rep only ([decision 0021](decisions/0021-sp-11-swarf.md), OI-96). 18 needs 10 and 11, because holes are found from cylindrical faces.
6. Close: slice 19 needs 6 (its export dialogs need 8); slice 20 needs everything, but its packaging, air-cut procedure and safe-practice checklist (items 8, 10 and 12) can be written earlier.
7. Which parts of these slices can run in parallel, what each lane needs, and the order for project schema numbers are in "Parallel work" below.
8. Slice 21 (workholding and retaining tabs, decision 0035) is added after most of this plan was in progress, so it is split into sub-slices that leave slices 12, 13 and 14 alone: 21a (the setup data) and 21b (the composite target in `openchuck-path`) need nothing merged first; 21c (application wiring, simulation and the UI) needs both, and its roughing and finishing tests need slices 12 and 14. Slice 20 needs slice 21.
9. Slice 22 (user interface conformance pass, [decision 0105](decisions/0105-slice-22-conformance-pass.md)) starts after lanes O and 21d have merged. Lane X1's conformance report ([decision 0114](decisions/0114-slice-22-conformance-report.md)) splits steps 2 to 4 into lanes X2 to X9 in three waves (Parallel work); they also build slice 8 items 8 to 12 (OI-338, owner's choice). Slice 20 needs it.

## Parallel work

Slices 0 to 7 and 13 are closed. The remaining work splits into the lanes below, each of which can run in its own session once what it needs has merged. A lane may be a whole slice or only the part of a slice that does not wait on another lane; the "Slice order and dependencies" list above gives the reasons. Lane letters are for reference in branch names, pull requests and decision records, not requirement or slice numbers.

| Lane | Slice and scope | Wave 0 | Wave 1 | Wave 2 | Wave 3 | Wave 4 |
| --- | --- | --- | --- | --- | --- | --- |
| A | 8 viewport and application shell (continue `claude/slice-8-viewport-shell`) | ██ | ██ | | | |
| B | 9 items 1 and 2: job manager, pipeline caches (headless) | ██ | | | | |
| C | 10 items 2 to 5: selection model, flood fill, sets, persistence, region constraint (headless) | ██ | | | | |
| D | 11 items 1 to 4, 6, 8, 9: STEP importer, topology, tessellation, B-rep `SurfaceSource`, corpus, CI | ██ | ██ | ██ | | |
| E | 14 rotary roughing, items 1 to 4, 6, 7 | ██ | ██ | | | |
| F | 19 items 1 to 3 and 5: output grouping, comments, setup sheet | ██ | ██ | | | |
| G | 20 items 8, 10, 12: packaging route, air-cut procedure, safe-practice checklist (done 14 Oct 2026, [decision 0031](decisions/0031-slice-20-packaging-and-trial-documents.md); the release workflow and bundle command are built in lane T) | ██ | | | | |
| H | 12 baseline finishing | ░░ | ██ | | | |
| I | 13 simultaneous 4-axis finishing (closed 15 Oct 2026, [decision 0036](decisions/0036-slice-13-simultaneous-finishing.md)) | ░░ | ██ | ██ | | |
| J | 15 rest machining and pencil clean-up (built 15 Oct 2026 on `claude/lane-j-slice-15-rest-machining`, [decision 0044](decisions/0044-slice-15-rest-and-pencil.md), accepted) | ░░ | ██ | | | |
| K | 16 edge following on mesh feature edges | ░░ | ██ | | | |
| L | 14 item 5: containment to selected regions | ░░ | ██ | | | |
| M | 10 items 1 and 6: picking, hover, selection panel | ░░ | ░░ | ██ | | |
| N | 9 items 3 to 8: drawing, playback, forms | ░░ | ░░ | ██ | | |
| O | 11 items 5 and 7: viewport faces and edges, B-rep selection storage (built 17 Oct 2026 on `claude/lane-o-slice-11b-brep-selection`, [decision 0066](decisions/0066-slice-11b-brep-selection.md), accepted; project schema 12, the stored mesh edge kind of OI-204 and the keep-tangent-edges choice; the STEP wiring after lane Z, OI-242, in the same pull request) | ░░ | ░░ | ██ | | |
| P | 16 edge following on B-rep edges | ░░ | ░░ | ░░ | ██ | |
| Q | 17 swarf (library strategy built 17 Oct 2026 on `claude/lane-q-slice-17-swarf`, [decision 0061](decisions/0061-slice-17-swarf.md), accepted) | ░░ | ░░ | ░░ | ██ | |
| Q2 | 17 swarf through the application (OI-238): a compatible face set required for swarf, the B-rep-only message, the `swarf` reference job and gate 3 (built 18 Oct 2026 on `claude/lane-q2-swarf-wiring`, [decision 0111](decisions/0111-swarf-through-the-application.md), accepted 18 Oct 2026, relayed by the orchestrator session at the owner's instruction) | | | | | ██ |
| R | 18 radial drilling (library strategy built 16 Oct 2026 on `claude/lane-r-slice-18-drilling`, [decision 0051](decisions/0051-slice-18-radial-drilling.md), accepted) | ░░ | ░░ | ░░ | ██ | |
| S | 19 item 4: export dialogs and the tool-number dialog | ░░ | ░░ | ░░ | ██ | |
| U | 21a workholding and tab data in the setup (SET-09, OP-21): types, validation, project file, commands | ██ | | | | |
| V | 21b composite target in `openchuck-path`: cylinder and frustum solids joined to the model's surface | ██ | ██ | | | |
| W | 21c wiring: application mapping, simulation, deviation, forms (built 16 Oct 2026 on `claude/lane-w-slice-21c-wiring`, [decision 0045](decisions/0045-slice-21c-wiring.md), accepted) | ░░ | ░░ | ██ | | |
| Y | OI-209: path assembly merges the micro-moves path fitting leaves at a wall in the target ([decision 0050](decisions/0050-walls-merge-micro-moves.md), accepted 16 Oct 2026, relayed by the orchestrator session at the owner's instruction; merged in pull request 66; its proof on the default tabs is on `claude/lane-y-default-tabs-proof`) | ░░ | ░░ | ██ | | |
| F | OI-236 (second assignment of the letter; slice 19's lane F is done): the Makera Z1's F word within its limit where A counts in it, and small feeds written to three significant figures ([decision 0076](decisions/0076-feed-word-limit-and-rounding.md), accepted 17 Oct 2026; built on `claude/lane-f-feed-limit-and-rounding`) | ░░ | ░░ | ██ | | |
| Z | OI-219 and OI-226: STEP parts through the application pipeline, for lane R's drilling and lane P's B-rep edges; the `drilling` reference job (merged in pull request 79, 17 Oct 2026; the STEP inspection fix, OI-259, on `claude/lane-z-step-inspection-weld`; [decision 0056](decisions/0056-brep-path-through-the-pipeline.md), accepted 17 Oct 2026) | ░░ | ░░ | ░░ | ░░ | ██ |
| TL | 23 default tool library: the Makera Z1 reference tools and common standard tools, installed on first run ([decision 0060](decisions/0060-gc16-command-line-reading-and-tl12-default-tool-library.md); standard tools and the Z1 set built, open: OI-281, OI-282, the licence OI-269; [decision 0081](decisions/0081-slice-23-default-tool-library.md), accepted) | ░░ | ░░ | ░░ | ░░ | ░░ |
| E2 | 14 item 4 completed: per-selection stock to leave for rotary roughing as the largest value over the whole part, with a note ([decision 0058](decisions/0058-roughing-stock-to-leave-per-selection.md), accepted 17 Oct 2026; built 17 Oct 2026 on `claude/lane-e2-roughing-stock-per-selection`) | ░░ | ░░ | ░░ | ░░ | ██ |
| TR | 20 items 8 and 9: the release workflow (tags `lib-v*` and `app-v*`, dry-run by default, no crates.io publishing) and `cargo xtask release-gate` (built 17 Oct 2026 on `claude/lane-tr-release-workflow-and-gate`, draft pull request 99; decision record 0091 on its branch) | ░░ | ░░ | ░░ | ░░ | ██ |
| TE | 20 items 3, 6 and 7: end-to-end command-line scenarios, the determinism run at 1, 2 and N threads over the sample projects (the reference jobs and the owner's blisk), undo, redo and cancellation, and the no-network run (built 17 Oct 2026 on `claude/lane-te-end-to-end-and-determinism`, pull request 98; [decision 0096](decisions/0096-slice-20-end-to-end-and-determinism.md), accepted) | ░░ | ░░ | ░░ | ░░ | ██ |
| TS | OI-129 and slice 20: a UI smoke test of the built archives on Windows, macOS and Linux under software rendering, `ui-smoke.yml` (built 17 Oct 2026 on `claude/lane-ts-ui-smoke-ci`, draft pull request 95; decision record 0101 on its branch) | ░░ | ░░ | ░░ | ░░ | ██ |
| TP | 20 publish readiness (OI-302, OI-303): make the library family pass `cargo publish --dry-run` (where the built-in posts live is an architecture question for the owner first), then a real crates.io publish step and name reservation; planned by the owner's decision of 17 Oct 2026; not started | ░░ | ░░ | ░░ | ░░ | ░░ |
| FZ | 20 item 4: the malformed-input sweep, a fuzz target for every user file and the fuzz CPU hours per parser (built 18 Oct 2026 on `claude/lane-fz-malformed-input-sweep`, [decision 0107](decisions/0107-malformed-input-sweep-and-fuzz-hours.md), accepted 18 Oct 2026) | ░░ | ░░ | ░░ | ░░ | ██ |
| BT | OI-316 and OI-253: the blisk's gate 2 failures in lane TE's run were from before decision 0076 (lane TE closes OI-316); an older Makera Z1 machine copy with no library ID now migrates by its name ([decision 0109](decisions/0109-z1-copy-without-an-id-migrates-by-name.md), accepted 18 Oct 2026, relayed; built on `claude/lane-bt-gate2-timing`) | ░░ | ░░ | ░░ | ░░ | ██ |
| BX | OI-319: the blisk's programs the same on Linux, Windows and macOS (toolpath choices that rounding cannot flip: ties at mesh edges, the drop-cutter's tie rule, edge contacts to rounding precision, the fitter's deviation along the tool's contact normal; built 18 Oct 2026 on `claude/lane-bx-cross-platform-f-words`, [decision 0113](decisions/0113-toolpath-choices-rounding-cannot-flip.md), accepted 18 Oct 2026; OI-318 investigated) | ░░ | ░░ | ░░ | ░░ | ██ |
| BX-b | OI-331: the blisk's overcut after helical finishing is real and the path fitter's, which tested a move at its middle alone; it now tests the quarter points too, and the contact smoothing follows each point's own tip so an added point does not upset the contact feed beside it (built 18 Oct 2026 on `claude/lane-bx-b-helical-overcut`, [decision 0115](decisions/0115-path-fitting-tests-the-quarter-points.md), accepted 19 Oct 2026; workspace 0.10.0; OI-331 closed, OI-348 new) | ░░ | ░░ | ░░ | ░░ | ██ |
| FIT | OI-348: simultaneous finishing's own path fitter tested a move at its middle alone; measured real on the exact cubic ramp (0.187 mm between points) and absent on the lobed reference job (0.0039 mm, the chord limit; the blisk is refused before any program, OI-318); the fitter now tests the quarter points too, with no golden-file change (built 19 Oct 2026 on `claude/lane-fit-simultaneous-fitter`, [decision 0133](decisions/0133-simultaneous-fitter-tests-the-quarter-points.md), accepted 19 Oct 2026; OI-348 closed, OI-390 new) | ░░ | ░░ | ░░ | ░░ | ░░ |
| X1 | 22 step 1: the conformance report and component inventory ([decision 0114](decisions/0114-slice-22-conformance-report.md), accepted 18 Oct 2026) | ░░ | ░░ | ░░ | ░░ | ██ |
| X2 | 22: shell frame (window, steps, primary action, tree marks, dock and Messages, footer, empty state, settings; slice 8 items 8, 11, 12). Built 18 Oct 2026 on `claude/lane-x2-shell-frame` ([decision 0117](decisions/0117-slice-22-shell-frame.md), accepted 19 Oct 2026; OI-350 and OI-351 answered by the owner and closed); boards 1 and 2 re-rendered | ░░ | ░░ | ░░ | ░░ | ██ |
| X3 | 22: viewport drawing (toolbar, overlays, colour roles, dash and hatch lines, held regions and tabs, the OI-341 widths; built 18 Oct 2026 on `claude/lane-x3-viewport-drawing`, [decision 0119](decisions/0119-slice-22-viewport-drawing.md), accepted 19 Oct 2026; boards 03, 13, 14 and 17 re-rendered; wiring left to X2 and X4 is OI-355) | ░░ | ░░ | ░░ | ░░ | ██ |
| X7a | 22: theme tokens and IBM Plex with the OFL text and notices (new files only; slice 8 item 10; built 18 Oct 2026 on `claude/lane-x7a-theme-tokens-fonts`, pull request 124) | ░░ | ░░ | ░░ | ░░ | ██ |
| X4 | 22: inspectors (field rows, header and error banner, setup sections with two provenance icons, OI-334 layout, part extent). Built 19 Oct 2026 on `claude/lane-x4-inspectors` ([decision 0123](decisions/0123-slice-22-inspectors.md), accepted 19 Oct 2026; OI-366 answered by the owner, A, and closed); image 02's machine section re-rendered; lane X3's viewport hooks wired (OI-355 closed); the default tabs offered passed to lane X5's Setup panes (OI-383) and two findings on its pull request fixed (OI-353, OI-354), and one on lane X6's import window (OI-358), all closed | ░░ | ░░ | ░░ | ░░ | ██ |
| X5 | 22: Setup step's front and end panes (built 19 Oct 2026 on `claude/lane-x5-setup-panes`, [decision 0129](decisions/0129-slice-22-setup-panes.md), accepted 19 Oct 2026; board 16 re-rendered; OI-380 to OI-383) | ░░ | ░░ | ░░ | ░░ | ██ |
| X6 | 22: Simulate results and playback dock, the SIM-07 control, one import window, selection sets. Built 19 Oct 2026 on `claude/lane-x6-simulate-import` ([decision 0125](decisions/0125-slice-22-simulate-and-import.md), accepted 19 Oct 2026; OI-370 to OI-374; OI-372 and OI-374 closed); boards 01b, 04, 15 and 11 re-rendered | ░░ | ░░ | ░░ | ░░ | ██ |
| X7b | 22: plain wording and tokens applied to existing panels (built 19 Oct 2026 on `claude/lane-x7b-wording-tokens`; no decision record; OI-360) | ░░ | ░░ | ░░ | ░░ | ██ |
| X9 | 22: tool library window (12.1) | ░░ | ░░ | ░░ | ░░ | ░░ |
| SU | 22 (SET-01 to SET-06 from the interface): the Setups "+" and the workflow bar's "Add setup" create the project's one setup ([decision 0144](decisions/0144-one-setup-from-the-interface.md), accepted 19 Oct 2026; OI-425 and OI-426 closed; built on `claude/lane-su-add-setup`, draft pull request) | ░░ | ░░ | ░░ | ░░ | ██ |
| X8 | 22 step 3: smoke test, 1366 × 768 screenshots, display checks, frame benchmark rerun | ░░ | ░░ | ░░ | ░░ | ░░ |
| BUG-LOG | Issue 155 (trial finding F-14): no console window behind the Windows application, a local log file beside the executable, panics logged, the log path for the About window. Built 19 Oct 2026 on `claude/lane-bug-log-console-and-log-file` ([decision 0155](decisions/0155-local-log-file.md) and [decision 0156](decisions/0156-windows-gui-subsystem-and-console-attach.md), both accepted 19 Oct 2026; OI-453 to OI-459; requirement NFR-17; no project format change) | ██ | | | | |
| XC | CI hygiene: `cargo xtask conflicts` fails on unresolved merge conflict markers in tracked files, wired into the CI fast checks and `all` (built 19 Oct 2026 on `claude/lane-xc-conflict-marker-check`; no requirement covers it directly, supports NFR-06; no decision record; no open items) | ██ | | | | |
| T | 20 hardening and first release | | | | | ██ |

`██` is working, `░░` is waiting. The entry module of [decision 0026](decisions/0026-entry-moves.md) (pull request 37) must be on `main` before lanes E, H and L start.

| Lane | Needs merged first |
| --- | --- |
| A, B, C, D, F, G, U, V | Nothing |
| E | The entry module (decision 0026); item 5 waits for C |
| H | C and the entry module |
| I | C |
| J | E and B |
| K | C |
| L | C and E |
| M | A and C |
| N | A and B |
| O | A, C and D |
| P | D |
| Q | D (swarf is B-rep only, decision 0021) |
| Q2 | Q, O and Z (all merged): the strategy, B-rep face sets and the B-rep path through the pipeline |
| R | C and D |
| S | A and F |
| W | U and V; its roughing and finishing tests need lanes E and H merged |
| Y | V (the composite target); its proof on the default tabs needs W |
| Z | P and R (both merged), W (the composite target's wiring) |
| TL | Slice 4 (merged); nothing else |
| E2 | E and H (both merged): nothing else |
| TR | Nothing |
| TE | Nothing to start; its sample set grows as strategy lanes merge |
| TS | Nothing |
| TP | The first-release features (so the library API settles), lane TR merged, and the owner's decision on where the built-in posts live (OI-302) |
| FZ | Nothing; its hours accumulate nightly after it merges |
| BT | Nothing |
| BX-b | BX (decision 0113, merged) |
| FIT | BX-b (decision 0115, merged) |
| X1 | O and 21d (decision 0105; both merged) |
| X2 | Pull request 121 (lane S11). Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| X3 | Pull request 121. Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| X7a | Nothing. Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| X4 | X2 and X7a. Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| X5 | X2 and X3. Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| X6 | X2; its Simulate colours after X3. Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| X7b | X2 and X7a. Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| BUG-LOG | Nothing. Touches `apps/openchuck-ui` `main.rs`, `graphics.rs` (the logger) and the new `logging.rs`, the crate's `Cargo.toml` and the docs; stays out of the files of X4, SU, BI, XC and FIT. The About button is OI-454. |
| X9 | X2 and X4. Its step 1: the lane agent re-renders the boards [decision 0114](decisions/0114-slice-22-conformance-report.md) section E names for it and commits them (OI-342) |
| X8 | X2 to X7b and X9 |
| XC | Nothing |
| T | Everything, including W and X1 to X9 |

### Project schema ordering

The project folder has one schema number for every file ([decision 0008](decisions/0008-slice-1-project-model.md) item 12), now 6 (`openchuck-project/src/file.rs`, `SCHEMA` and `MIGRATIONS`). A change to what any project file holds takes the next number, adds one migration step and, where the change is not a plain addition, a fixture folder under `crates/openchuck-project/tests/fixtures/`. Two lanes that both need the next number collide on the constant, the migration list, the fixtures and the `schema = N` text in every golden project, so numbers are claimed before they are used.

1. **Claim first.** Before changing the format, add a row to the claims table below in a small documentation pull request (or in your first push, if the lane has none yet), taking the next free number. Check unmerged branches as well as `main`, as for decision and `OI-` numbers. A row is a claim on the number only; it does not say the lane will merge first.
2. **Merge in claim order where you can.** If a lane with a later number is ready first, it may merge first, and the earlier lane then takes the later number: renumber the constant, the migration step, the fixture folder, the golden projects and the claims row in the same pull request, and say so in the record.
3. **A format change is a bump, however small.** Slice 7 bumped for a table written only when non-default ([decision 0025](decisions/0025-slice-7-simulation.md) item 5). Operation parameters are a free map (decision 0008 item 6), so a new strategy parameter is not a format change; a new field in a file, a new file kind or a changed meaning is.
4. **Migrate, never guess.** A migration renames, moves or fills fields from what is there. If a value cannot be inferred, the older file is refused with a message saying what to add, as for schema 3 tool files ([decision 0014](decisions/0014-slice-4-tool-library.md) item 10).
5. **Rebase before merge.** The lane that merges second merges `main` first, re-runs `cargo test -p openchuck-project` and the fuzz target for project files, and checks the golden projects.
6. **Nothing is released yet,** so a claimed number can be renumbered freely until the first release (slice 20). After it, numbers are fixed.

| Schema | Lane | Change | Status |
| --- | --- | --- | --- |
| 5 | Slice 7 | Simulation settings in `project.toml` | Merged |
| 6 | A (slice 8) | Operations gain `enabled`, written only when false (UI-06); the machine copy in `machine.toml` gains its library source's `id` ([decision 0029](decisions/0029-slice-8-viewport-and-shell.md) items 10 and 16) | Claimed; in the slice 8 pull request |
| 7 | C (slice 10) | Operation references to selection sets: containment, avoid and check surfaces (OP-02); selection kind per model | Claimed and built on `claude/slice-10-mesh-selection` ([decision 0032](decisions/0032-slice-10-mesh-selection.md)): the operation's `boundary`, `avoid` and `check` fields (the selection kind per model was already in schema 1, decision 0010). In pull request 39 |
| 8 | E (slice 14) | Tools gain `centre_cutting` (optional), in tool library files and in the project's tool snapshots (OI-117; decision 0033) | Claimed and built on `claude/slice-14-rotary-roughing` (pull request 45). Lane H takes 9, then lanes D and O and slice 19 item 4 follow |
| 9 | H (slice 12) | Selection sets gain an optional `stock_to_leave`, written only when set (OP-11 per selection; [decision 0034](decisions/0034-slice-12-baseline-finishing.md) item 4) | Claimed as 10, built as 8 and renumbered to 9 when lane E (pull request 45, schema 8) merged first. Lanes D and O and slice 19 item 4 take the next free numbers when they claim |
| 10 | U (slice 21a) | Per-setup workholding zones and tabs: the optional `[workholding]` table in setup files (SET-09, OP-21) | Claimed and built on `claude/lane-u-slice-21a-workholding-data` ([decision 0040](decisions/0041-slice-21a-workholding-data.md), accepted) |
| 11 | M (slice 10) | Selection sets gain an optional `colour` (`#rrggbb`), written only when the user chose one; a set without one takes the default swatch of its place in the list ([decision 0046](decisions/0046-slice-10-selection-ui-to-specification.md) item 2, owner's choice) | Claimed and built on `claude/lane-m-slice-10-picking` (pull request 59); additive, so a no-op migration and no fixture folder, as for schemas 9 and 10 |
| 12 | O (slice 11b) | A selection set's target may be `mesh_edges`: edges of a mesh model's triangles as `[triangle, side]` pairs, relative to the file's content hash (OI-204); a set of B-rep edges gains the optional `keep_tangent_edges`, and `project.toml` the project's default `keep_tangent_edges`, both written only when set (owner's request, 17 Oct 2026), and `warn_on_selection_reset`, written only when false (owner's decision on item 3). B-rep face and edge selections were already in schema 1, and the STEP units override is the model reference's (schema 2), so lane D took no number | Claimed and built on `claude/lane-o-slice-11b-brep-selection` ([decision 0066](decisions/0066-slice-11b-brep-selection.md), accepted); additive, so a migration that changes only the number (a schema 11 file holding a new field is refused) and no fixture folder |
| 13 | F (OI-236, feed word) | The machine copy's feed limits gain the optional `rotary_in_feed`, written only when not `separate`; a copy of the built-in Makera Z1 (known by its library `id`) gains `"makera"`, and machine profile files go to their schema 3 ([decision 0076](decisions/0076-feed-word-limit-and-rounding.md), accepted) | Claimed and built on `claude/lane-f-feed-limit-and-rounding` (pull request 82), as the orchestrator directed (17 Oct 2026; first claimed as 14 while 13 was held for lane S, which took no number, [decision 0071](decisions/0071-slice-19-export-dialogs.md) item 2): 12 passes through a no-change step on that branch, which lane O's merge replaces with its real step; if lane O merges first, the placeholder is dropped. A fixture folder (`schema-11-z1`), since the migration fills a value |

The lanes not listed (B, E, H, I, J, K, L, M, N, P, Q, R, V, W, T) are not expected to change the format: caches live in the cache folder (PRJ-03), strategy settings are operation parameters, and the UI is not expected to store its view state in the project. A lane that finds it needs a bump adds a row. Lane K and lane P may need a row if edge chains are stored as a new selection kind. Lane K takes none: its strategy reads the part's own feature edges and a stored choice of chains is OI-204 ([decision 0047](decisions/0047-slice-16-edge-following-mesh.md)).

### Working in parallel

- **Numbers.** Decision records, `OI-` numbers and schema numbers are taken from the next free number across `main` and every unmerged branch. Slice 8 renumbered to record 0029 and OI-127 to OI-130 when it merged `main`, and lane G took record 0031 and OI-133 to OI-135. Lane C (slice 10) took record 0032, OI-136 to OI-140 and project schema 7. Slice 11a's record 0027 and OI-118 to OI-123 are on `main`, and lane B (slice 9) has merged record 0030 and OI-131 to OI-132. Other unmerged branches hold record 0028 and OI-124 to OI-126. Lane E (slice 14) took record 0033, `OI-150` to `OI-153` and project schema 8. Lane F (slice 19) took record 0028 and OI-141 to OI-144 on `main` first, lane I (slice 13) held OI-145 to OI-148 on its branch and OI-149 went to the CI required-checks job, so lane E's open items are OI-150 to OI-153. Lane H (slice 12) took record 0034, OI-154 to OI-157 (first OI-149 to OI-152, renumbered after lane E and the CI required-checks job took those) and project schema 9. Lane I took record 0036 (0033 went to lane E, and unmerged branches hold 0034 and 0035), OI-145 to OI-148, requirement OP-20 and no schema number. Slice 21 (workholding and retaining tabs, record 0035) took OI-158 to OI-162 and requirement OP-21. Lane M (slice 10 items 1 and 6) took record 0037 and OI-170 to OI-173 (first OI-159 to OI-162, renumbered after workholding took those and the OpenCASCADE cache took OI-169), and no schema number. The interface specification took record 0038 and OI-163 to OI-166 ([decision 0038](decisions/0038-ui-specification-and-requirements.md), accepted; all four closed). Lane N (slice 9 items 3 to 8) took record 0040 (first 0038, renumbered after the interface specification took it) and OI-174 to OI-179 (first OI-163 to OI-168), and no schema number. Lane U (slice 21a) took record 0041, OI-180 and OI-181 (first 0040, OI-169 and OI-170, then OI-174 and OI-175; renumbered after lane N took those) and project schema 10. Lane V (slice 21b) took record 0042 and OI-185 (first 0040 and OI-170, then 0041 and OI-180, renumbered after lanes M, N and U took those; checked 16 Oct 2026, when unmerged branches held records up to 0041 and OI up to 184) and no schema number. Lane L (slice 14 item 5) took record 0043 and OI-187 and OI-188 (both closed in the record) and no schema number. Lane K (slice 16, mesh edges) took record 0047 and OI-204 to OI-208 (assigned by the orchestrator) and no schema number. Lane J (slice 15) took record 0044 and OI-189 to OI-193 (first 0040 and OI-170 to OI-174, then 0041 and OI-180 to OI-184, renumbered after lanes M, N, U, V and L took those) and no schema number. The merge of the interface specification with `main` on 16 Oct 2026 added OI-194. Lane Y (OI-209, walls in the target) took record 0050 and OI-211 to OI-213 of the OI-211 to OI-215 the orchestrator assigned it on 16 Oct 2026 (OI-214 and OI-215 unused), and no schema number. Unmerged branches hold OI-199 to OI-203 (lane M, box selection is OI-201) and OI-204 to OI-208 (lane K). Lane R (slice 18) took record 0051 and OI-216 to OI-220 (assigned by the orchestrator, 16 Oct 2026) and no schema number. Lane W (slice 21c) took record 0045 and OI-195 to OI-198 and OI-209, assigned by the orchestrator on 16 Oct 2026, and no schema number. Next free after that: record and `OI-` numbers are assigned by the orchestrator; project schema 12 (lane M's follow-up, decision 0046, took OI-199 to OI-202 and schema 11, a stored set colour; lane O took 12 and lane F 13). Interface lanes (A, M, N, O, S, T) follow [docs/ui-design/ui-specification.md](ui-design/ui-specification.md), accepted with decision 0038. Lane P (slice 16, B-rep edges) took record 0053 and OI-226 to OI-230, and no schema number. Lane Z (STEP parts through the pipeline) took records 0056 and 0057 and OI-232 to OI-236, of the records 0056 to 0060 and OI-232 to OI-236 the orchestrator assigned it (17 Oct 2026; 0058 to 0060 unused), and no schema number. Lane Q (slice 17, swarf) took record 0061 and OI-237 to OI-241 (assigned by the orchestrator, 17 Oct 2026), then OI-258 (assigned by the orchestrator; lanes O, S and F hold OI-242 to OI-257), and no schema number. Lane S (slice 19 item 4) took record 0071 and OI-247 to OI-251 and OI-257 (OI-247 to OI-251 assigned by the orchestrator, 17 Oct 2026; OI-257 was added by the orchestrator for the fifth item) and no schema number (decision 0071 item 2); checked 17 Oct 2026: no remote branch holds a record from 0065 or an `OI-` number from 247. Lane F's second assignment (OI-236, the feed word) took record 0076 and OI-252 to OI-254 of the records 0076 to 0080 and OI-252 to OI-256 the orchestrator assigned it (17 Oct 2026), and project schema 13 (provisional; first 14, while 13 was held for lane S, which took none). Lane O (slice 11b) took record 0066, OI-242 to OI-246, OI-260 and OI-332 (assigned by the orchestrator, OI-260 for the owner's preferences-screen requirement; OI-332 the next free number, 18 Oct 2026) and project schema 12, which replaced the no-change step lane F had held for it, of the records 0066 to 0070 assigned to it (0067 to 0070 unused). Lane TE (slice 20 items 3, 6 and 7) took record 0096 and OI-310 to OI-315, of the records 0096 to 0100 and OI-310 to OI-319 the orchestrator assigned it (17 Oct 2026; checked free on `main` and every remote branch), and no schema number. Lane TR (slice 20 items 8 and 9) took record 0091 and OI-300 to OI-306 of the records 0091 to 0095 and OI-300 to OI-309 the orchestrator assigned it (17 Oct 2026), and no schema number. Lane Q2 (slice 17 wiring, OI-238) took record 0111, the next free number across `main` and every remote branch on 18 Oct 2026 (not assigned by the orchestrator, so provisional), no `OI-` number (its first proposal's OI-334 was settled by the owner before merging, and lane 21d holds that number) and no schema number. Lane BX (OI-319) took record 0113 and OI-336 and OI-337, the next free across `main` and the unmerged branches on 18 Oct 2026 (records 0107 to 0109, 0111 and 0112 and OI up to OI-335 held by pull requests 115 to 121; first 0112, OI-335 and OI-336, renumbered after lane S11 took 0112 and OI-335), and no schema number. Lane BX-b (OI-331) took record 0115 and OI-348, the next free across `main` and the unmerged branches on 18 Oct 2026 (record 0114 and OI up to OI-345 held by pull request 123), and no schema number. Lane FIT (OI-348) took record 0133 and OI-390 of the records 0133 and 0134 and OI-390 to OI-394 the coordinator assigned it (19 Oct 2026; checked free on `main` and every open pull request and remote branch, which held up to record 0129 and OI-384), and no schema number.
- **Shared code.** Strategies are registered in the registry in `openchuck-path`, and operations live in `openchuck-project`; additions there merge cleanly if parameter schemas stay uniform (OP-16) and each strategy keeps to its own module. Do not reshuffle shared files in a feature lane; propose the change first.
- **Library API.** `cargo-semver-checks` runs on `openchuck-types`, `-path`, `-post` and `-gcode` in every pull request. Before the first release a breaking change is acceptable when it is the better design ([decision 0048](decisions/0048-pre-1-0-api-changes.md)): bump the workspace minor version and add a changelog entry to each affected crate, and the check passes. Do not add a parallel type or function only to avoid the check (decision 0026 did that with `EntryParameters` and `assemble_with_entry`; those stay unless the code is being changed anyway).
- **Golden files** do not change unless the lane says they should; a plunge entry, a default setting or a refactor must leave them byte for byte as they are.
- **Spike status.** A lane that touches a spike updates its Status cell in the same pull request (lane D: SP-1 and SP-9).
- **Before every push** run `cargo fmt --all`, `cargo clippy --workspace --all-targets` (library code denies `unwrap` and unchecked indexing), the tests of the crates you changed, and `cargo xtask deps`, `licences` and `trace`.
- **Review comments.** CodeRabbit reviews code, not `docs/` or Markdown. Answer each finding on its thread and resolve it; a finding that is not worth its code is answered with the path traced and why.

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

**Progress:** items 1 to 7 are built and slice 3 is closed ([decision 0011](decisions/0011-slice-3-machine-stock-setup.md), accepted 11 Oct 2026, with [decision 0013](decisions/0013-z1-profile-consensus.md)). The project keeps a copy of its machine profile in `machine.toml` and setups gain part, stock, material, part and work offsets and clearance, so the project folder schema is now 3. The Makera Z1's rotary axis, A direction, A range and feed limits are confirmed by the owner; the reference point, travel and clearance stay unconfirmed (OI-60), and the SP-12 air cut (14 Oct 2026, decision 0007) has since confirmed the direction. Feed limits are per axis and the Z1 defaults follow [decision 0013](decisions/0013-z1-profile-consensus.md). Open: OI-53 (default machine library location), OI-54 (machine copy against the library), OI-55 (work zero off the axis, deferred) and OI-57.

**Review checklist** (evidence in decision 0011, "Verification", and decision 0013; ticked, slice closed 11 Oct 2026)

- [x] The A-axis direction on a deliberately asymmetric part matches the documented convention and the independent kinematic oracle (shown by CLI report now, visually in slice 8). `req_set_01_a_direction_on_an_asymmetric_part_matches_the_oracle`; `openchuck-cli machine` and `report` state the direction. The owner confirmed the convention from use of the machine on 11 Oct 2026, and the SP-12 air cut confirmed it on 14 Oct 2026
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

**Status:** closed 13 Oct 2026. Design refinement in [decision 0018](decisions/0018-slice-5-pathing-core.md), accepted 13 Oct 2026; the review checklist is ticked. All ten items are built: the SP-7 inputs as the `unroll` module and helical finishing, and the SP-9 result as the shape of the `SurfaceSource` port (the exact evaluators, the face record and exact tool drop are slice 11's, OI-81). OP-11's stepdown, lead-in and lead-out, ramping and per-selection stock to leave stay with slices 12 and 14, and helical entry has no slice yet (OI-80). OI-06, OI-08, OI-63, OI-78 and OI-79 are agreed and closed; the stronger offset result check (OI-82) is deferred to slice 12. `i_overlay` is not declared yet (nothing in the slice calls a boolean; decision 0018 item 14 raises the deviation from [decision 0016](decisions/0016-sp-7-2d-offsets.md) item 8).

**Design refinement:** the public API: `Strategy` trait with self-describing parameter schemas, the ports `SurfaceSource`, `StockModel` and `CollisionOracle`, and the kinematic mapping that takes the rotary axis as a parameter. Design the `SurfaceSource` port for the five queries of [decision 0017](decisions/0017-sp-9-exact-surfaces.md) (point and unit normal at (u, v); closest point with (u, v) and distance; trimmed ray hits; a trim test in (u, v); the tessellation), so B-rep faces slot in later as plain data evaluated in pure Rust. Confirm the feed approach (contact feed plus duration in the IR) with SP-3 and SP-12, and use the tool profile type that every cutter shape maps to, as settled by SP-13 ([decision 0012](decisions/0012-sp-13-tool-profiles.md)). Tool-centre solving is a drop-cutter generic over the profile (vertex, facet and edge tests that need only the profile's lower envelope `h(r)` and its pieces), returning the tip and the contact point; normal-offset solving is a fast path for convex profiles only. Evaluate distances directly (the expanded quadratic loses about 1e-8 mm near the tool axis), and measure deviation along the surface normal, not vertically. The spike's drop took 0.35 to 3.2 ms per position on a 147,000-triangle mesh with a plain grid, so add a bounding structure (SP-4) and cull edges by height before searching.

1. Mesh `SurfaceSource` inside the library using `parry3d-f64` or an own BVH, per SP-4, with point, normal, projection, ray and distance queries.
2. Ports `StockModel` (with a simple analytic implementation for tests and small hosts) and `CollisionOracle` (default accepts everything and relies on the clearance geometry).
3. Rotary kinematic mapping to X, Y, Z and A with wrap, continuity, limits and unwinding (OP-15); the rotary axis is a parameter.
4. `Strategy` trait and registry with parameter schemas covering names, units, ranges, defaults and validation (OP-16).
5. Helical finishing on the whole model as the first wrap-family strategy (built in slice 5; any follow-up is in slice 12), using the offset approach proven in SP-7.
6. Contact-point feed intent and segment duration written into the IR; compensated feed per minute derived by the post (OP-13).
7. Linking moves and retracts through the clearance geometry (OP-14).
8. Operation model in `openchuck-project`: strategy, tool, setup, selection, parameters, regenerate at any time (OP-01), and the core of OP-11 (tolerance, stepover, stock to leave, feeds, retract).
9. The independent gouge and coverage checker, used only in tests and written separately from the strategies, plus analytic-oracle tests on the fixture parts.
10. Mapping in `openchuck-app` from project objects to the library's plain data; the library never sees the project file.

**Review checklist** (evidence in decision 0018; ticked 13 Oct 2026; accepted with that record)

- [x] On a cylinder, helix pitch and A rotation per unit of X match the analytic values within tolerance (`req_op_04_helix_pitch_and_a_per_x_match_the_analytic_values_on_a_cylinder`: 1e-6 degrees at three pitches, both rotations)
- [x] A never jumps beyond the wrap rules, and no output contains NaN (`req_op_15_*` and the helical tests: continuity, no step over half a turn, no reversals, no NaN)
- [x] Contact-point feed matches an analytic check, with the formula documented (`req_op_04_req_op_13_contact_feed_is_the_programmed_feed_on_every_cut`; `t = 60 L / f` in decision 0018 item 8 and `assemble.rs`)
- [x] Links stay inside the clearance geometry and never cross stock (`req_op_14_links_stay_outside_the_clearance_and_never_cross_the_stock`)
- [x] The independent checker reports zero gouge and full coverage on the oracle parts, and fails on a seeded fault (`req_op_12_the_independent_checker_finds_no_gouge_and_full_coverage_on_the_oracle_parts`: cylinder, cone, sphere, helical groove, ruled hyperboloid)
- [x] Forward kinematics of every XYZA point returns the intended tool tip within 1e-6 mm and 1e-6 degrees, checked against the independent oracle (`req_op_05_req_op_06_every_helical_pose_round_trips_through_the_independent_oracle`, both A senses)
- [x] Toolpath deviation stays within 0.01 mm (about 0.005 mm tessellation and 0.005 mm path fitting) (`req_op_04_req_nfr_05_path_fitting_stays_inside_its_budget_on_a_sphere`: 0.0003 mm at the default steps; 0.00405 mm fit error with long moves at the default tolerance; the factor is 0.4, OI-08)
- [x] On simultaneous moves the effective feed at the cutter contact is within ±2% of programmed (`req_op_13_feed_at_the_contact_is_within_two_percent_on_a_grooved_part`: worst 1.03 % away from the part's ends; near a rim 2.06 %, kept open as OI-77)
- [x] Identical output at 1, 2 and N threads (`req_nfr_06_*` for the rings stub, helical finishing and the offset passes)
- [x] The library builds and runs in a small host program using only its public API (`crates/openchuck-path/examples/host.rs`, run in CI)

## Slice 6 – Post-processing and G-code round trip

**Status:** closed 14 Oct 2026; design in [decision 0020](decisions/0020-slice-6-post-and-round-trip.md), accepted 14 Oct 2026. Items 1 to 13 are built. Gates 1 and 2 pass on the posted projects and the reference jobs, and gate 3 passes in LinuxCNC 2.9.10 locally and in the `gate3` CI job (OI-47 closed). The Z1 air cut and first cut are the owner's machine tests (slice 20); the air-cut plan is in [reference-jobs.md](reference-jobs.md). Open items carried forward: OI-60 (axis height), OI-89 (inches, wrapped A), OI-92 (slice 19).

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
9. LinuxCNC simulator replay: documented configuration and a script that compares reached positions and limit faults with the expected toolpath (`cargo xtask replay`, `tools/linuxcnc/`, a Docker image for CI).
10. CLI: regenerate G-code from a project file without the UI, with a report (GC-11); this is also the end-to-end test harness.
11. G-code verification gates 1 to 3 for the reference jobs in CI: gate 1 static checks (syntax, modal state, units, axis limits, feed on every cutting move, safe retract and tool change) as test code in openchuck-gcode; gate 2 replay within G-code rounding only (0.0005 mm and 0.001 degrees at three decimals, scaled to the post's decimal setting); gate 3 the LinuxCNC simulator through the xtask replay runner. Any change to kinematics, a post or a strategy reruns all three.
12. Choose the reference job set (parts, stock, tools) for gates 4 and 5 and write its Z1 programs.
13. Tool numbers before posting (GC-14): the application lists the numbers the program will use and the user confirms them, renumbers automatically in order of first use by the operations, or renumbers by hand; duplicates are refused and the choice is saved in the project as undoable commands. The CLI asks on a terminal and takes an option otherwise; the UI dialog comes with export in the UI (slice 19). The post checks numbers against the machine's range (OI-62).

**Review checklist**

- [x] Re-running gives byte-identical G-code at any thread count (`req_gc_07_*` in `posts.rs`, `posting.rs` and `reference_jobs.rs`: repeats, four threads, and projects reopened at 1, 3 and 4 threads)
- [x] You have reviewed the golden files line by line (replaced, at your request, by an automated audit: `cargo xtask golden`, `crates/openchuck-app/tests/golden_audit.rs`, decision 0020 item 14: conventions, gate 1, the exact parts through the independent oracle, the contact feed under the controller's rule, and agreement between the posts)
- [x] The `openchuck-gcode` round trip matches the IR on positions, A travel and feeds (`round_trip.rs`; the three reference jobs and the posted projects: tip within 0.00064 mm, A exact, cut time within 1 %)
- [x] The helical test replays to completion in the LinuxCNC simulator with matching end position and no limit faults (`cargo xtask replay testdata/golden/reference/groove_linuxcnc.ngc`: all 640 moves reached, ended at the program's end, no fault; also the small part, cone and sphere; decision 0020 item 10)
- [x] Editing a post definition changes the output without recompiling; an unknown variable gives an error naming the line (`req_gc_03_*` in `posts.rs`)
- [x] Z1 output reviewed against SP-12 conventions, with an air-cut plan written (accepted by the owner 14 Oct 2026; the machine tests are run later, slice 20; the output follows the findings table, `req_gc_09_z1_output_follows_the_sp_12_conventions`, and the plan is drafted in [reference-jobs.md](reference-jobs.md))
- [x] Gate 1 fails on seeded faults (missing feed, A beyond its limit, wrong units), and gate 2 agrees within 0.0005 mm and 0.001 degrees (gate 1 does, with more faults than named; gate 2 agrees to 0.00064 mm at the tip and 0 degrees, per axis within 0.0005; the owner confirmed on 13 Oct 2026 that the figures are per axis, so the tip may differ by 0.00087 mm, OI-91)
- [x] The reference job set is chosen and its Z1 programs pass gates 1 to 3 ([reference-jobs.md](reference-jobs.md); gates 1 and 2 pass for the Z1 and LinuxCNC programs in `reference_jobs.rs`; gate 3 passes for the LinuxCNC programs, which share the toolpath and the kinematics with the Z1 programs; the Z1 post's own text, `G92.4` and the Makera feed rule, cannot run in LinuxCNC)
- [x] Confirming keeps the tool numbers, automatic renumbering follows operation order, a number used twice is refused, and a second post after renumbering gives byte-identical G-code (`req_gc_14_*` in `posting.rs`)

## Slice 7 – Material removal simulation (headless)

**Status:** closed 14 Oct 2026. Design in [decision 0025](decisions/0025-slice-7-simulation.md), accepted 14 Oct 2026 with the agreed edits to architecture sections 5.5 and 13 and verification plan 4.1. Items 1 to 8 are built; the review checklist is ticked with the evidence in the record. OI-100 and OI-101 are closed; OI-102 (shank, cross-platform identity, anisotropy) and OI-113 to OI-116 stay open with their owners.

**Goal:** the stock changes as the tool sweeps, results compare with the target, and stock carries from one operation to the next, all runnable from the CLI.

**Requirements:** SIM-03, SIM-05, SIM-06, SIM-07

**Spike inputs:** SP-6 (representation), SP-13 (tool profiles; [decision 0012](decisions/0012-sp-13-tool-profiles.md)). SP-6 times the profile sweep in its candidates and sets the budget SP-13's sweep-time criterion is judged against (OI-56).

**Design refinement:** the representation chosen in SP-6 (dexels, [decision 0022](decisions/0022-sp-6-simulation-stock.md)) behind `StockModel`, resolution settings, checkpoints for jumping to any operation or move, and results returned as plain data.

1. Dexel stock model in `openchuck-sim` implementing the `StockModel` port, with rotation about the A axis (SIM-03): Z dexels in the part frame, the tool span solved exactly per column (decision 0022).
2. Tool sweep along IR segments for every tool type, including V-bit, chamfer, drill and custom profiles, with A rotation.
3. Adjustable resolution trading accuracy for speed (SIM-07): the default cell is 0.1 mm; the user can configure a finer resolution (proposed finest 0.025 mm, to be measured) at the cost of speed and memory, and a finer setting is never the default. The setting is a project or application option, shown with its time and memory cost.
4. Carry-over: the stock left by operation N is the input to operation N+1 (SIM-06), with checkpoints.
5. Deviation against the target model, per-point remaining and overcut values, and worst-case figures (SIM-05).
6. Display meshes and result data for the viewport; no drawing code in this crate.
7. CLI simulation report; analytic-part tests asserting reported deviation, plus a comparison with a high-resolution brute-force run on a few small jobs.
8. Job-friendly: cancellable, progress events, deterministic at any thread count.

**Review checklist** (evidence in decision 0025, "Verification"; ticked, slice closed 14 Oct 2026)

- [x] On cylinder, cone and sphere cases removed volume matches the expected value within one simulation cell at the chosen resolution, and no worse than 0.02 mm at the finest setting (`req_sim_03_turned_parts_match_the_analytic_volume_within_one_cell`: 0.053 to 0.070 mm radial at 0.1 mm; 0.0187 mm at the finest, 0.025 mm)
- [x] A deliberately gouging path is reported as overcut with a location (`req_sim_05_a_gouging_path_is_reported_as_overcut_with_a_location`: 0.506 mm for a 0.5 mm dip, at its place)
- [x] Operation N+1 sees the stock left by operation N (`req_sim_06_*` in the simulator and the application: generated and simulated against it)
- [x] Changing resolution changes time and error as documented; memory use is stated; the default is 0.1 mm and a finer setting is opt-in (`req_sim_07_*`: about 7 times the time and 4 times the memory per halving; the setting states its cost)
- [x] The reference job (52,200 poses, Ø4 ball, 0.1 mm) meets the budget: 30 s on 4 threads, 120 s on one thread, 256 MB (decision 0022) (`tests/budget.rs`, release: 20.7 s, 70.4 s, 27 MB in a cloud container; the reference hardware run is OI-114)
- [x] Results are identical at 1, 2 and N threads (`req_nfr_06_simulation_is_identical_at_1_2_and_n_threads`, `req_gc_07_req_nfr_06_*`)

## Slice 8 – Viewport and application shell

**Goal:** a cross-platform window where you open a project, see the model with the machine setup, and move around it.

**Requirements:** UI-01, UI-02, UI-03, UI-06, UI-07, UI-10, UI-11, UI-12, UI-13, UI-14, UI-15, UI-16, NFR-02, NFR-03, NFR-15

**Interface specification:** [docs/ui-design/ui-specification.md](ui-design/ui-specification.md) ([decision 0038](decisions/0038-ui-specification-and-requirements.md), accepted 15 Oct 2026), including its differences from what is built (section 17, D1 to D11).

**Spike inputs:** SP-2 (egui, wgpu, 1M triangles).

**Design refinement:** confirm egui and `egui_wgpu` from SP-2 or switch to its fallback; the view-model boundary between `openchuck-ui` and `openchuck-app` (the UI never imports an engine crate); picking via CPU ray casts against a BVH in `openchuck-geom`.

1. `openchuck-viewport`: render loop, camera, standard views, perspective and orthographic, shaded, wireframe and edge display (UI-02).
2. Rotary axis, work coordinate system, stock and part drawn together (UI-03).
3. `openchuck-ui`: window, menus, new, open, save, unsaved-change handling.
4. Project tree listing model, stock, setups, operations and tools; operations can be reordered, enabled, disabled and duplicated through commands (UI-06).
5. Parameter forms generated from strategy and tool schemas, with units, validation and defaults (UI-07), and a custom-profile editor with live preview: add, move and delete line and arc segments, with the same validation as file import.
6. Import dialog with the check report; model transform and orientation controls; undo and redo menu.
7. Scripted 60-second camera-path benchmark on the 1M-triangle model, run on the physical reference machines, with a software-rendered smoke test in CI because hosted runners typically have no real GPU (native rendering is used wherever a runner offers it) (NFR-03).
8. Workflow steps with their availability and reasons, the footer and its primary action, and the window layout: resizable and collapsible panels, usable at 1366 by 768 at 100% scale, with 120% as the default scale (UI-10, UI-16; specification sections 3 to 5 and 10).
9. Machine value provenance icons with tooltips on hover and focus, replacing the list of unconfirmed values (UI-13; specification section 8, D8). *Built by slice 22 lane X4 ([decision 0123](decisions/0123-slice-22-inspectors.md), accepted): two icons on the profile's two states, as the owner chose (OI-339); the UI-13 wording waits for the owner (OI-345).*
10. Keyboard map, focus order and tooltips on focus; legibility, scale and contrast (UI-14, NFR-15; specification sections 14 and 16; checked again in slice 20). Bundle the IBM Plex Sans and Mono fonts unmodified, add `LICENSES/OFL-1.1.txt` and the font notice to the nightly and release archives (decision 0038, OI-164; NFR-10). *Bundled fonts and tokens: built by slice 22 lane X7a (`apps/openchuck-ui/src/theme.rs`, `apps/openchuck-ui/assets/fonts/`, `LICENSES/OFL-1.1.txt`, `LICENSES/IBM-Plex-NOTICE.txt`, the archive notices in `build-archives.yml`); only the fonts are installed so far, and applying the tokens, the keyboard map and the focus order stay with lanes X2 and X7b.*
11. Per-user settings in the configuration folder: layout, scale, theme, units, recent projects, acknowledgements (UI-15; specification section 13).
12. Item states in the tree with their legend, and the messages panel collecting import, setup, file, generation and job messages with their actions (UI-11, UI-12; specification sections 6 and 9). First placed in slice 9, which closed on 16 Oct 2026 before they were built.

**Progress:** started 14 Oct 2026 ([decision 0029](decisions/0029-slice-8-viewport-and-shell.md), proposed) on egui and wgpu, as accepted in decision 0002. Built on `claude/slice-8-viewport-shell`: items 1 and 2 (camera, standard views, both projections, shaded, feature-edge and wireframe display, a toolkit-free renderer with offscreen render tests, the scene view model with stock, rotary axis, A+ arrow and work coordinate system), item 3 (window, menus, new, open, save, unsaved-change handling) item 4 (the project tree; operations enabled, disabled, reordered and duplicated through undoable commands; project schema 6, OI-128 closed) item 5 (parameter forms from the strategy schemas and the tool dimensions, with units, ranges, defaults and value sources, invalid values refused with the project unchanged; the custom-profile editor with live preview, which completes TL-03) item 6 (the import dialog with the check report, tessellation statement, unit question and a preview with problems highlighted, OI-51 closed; model unit and placement controls; undo and redo menu; changed model files listed on open, OI-45 closed; OI-52 closed, fixed thresholds kept). Item 7: the product benchmark `openchuck-ui --bench` is built (its hardware run is the owner's) and the `ui-smoke` CI job is added (first runs pending, OI-129). Native file dialogs (OI-127), the default library folders (OI-53) and the machine copy compared with its library source (OI-54) are built. The owner's local run on Windows (14 Oct 2026) passed the benchmark on non-reference hardware and the visual check; five checklist items are ticked (the frame-time item on the owner's decision). The first CI runs of the `ui-smoke` job are recorded (17 Oct 2026; all three systems pass in software, OI-129; Windows and macOS block in `ci.yml` from pull request 103, OI-320 closed 18 Oct 2026), and lane TS adds `ui-smoke.yml`, which launches each platform's built nightly archive ([decision 0101](decisions/0101-ui-smoke-of-the-built-archives.md), accepted; first run OI-321). Still to do: the launch on native graphics on macOS and Linux (the remaining checklist item, hands-on). OI-130 is closed (the import dialog opens in its own window). OI-14 is deferred to slice 11. Items 8 to 12 (UI-10 to UI-16, NFR-15) are built by slice 22's lanes X2, X4, X7a and X7b (OI-338, the owner's choice, [decision 0114](decisions/0114-slice-22-conformance-report.md)); their review boxes below tick when those lanes land. Lane X2 ([decision 0117](decisions/0117-slice-22-shell-frame.md), accepted) builds items 8, 11 and 12: the workflow steps with their availability and reasons, the footer and its primary action, the window's regions with collapsing, the splitter reset and the small-window rule (item 8); the per-user settings in the configuration folder (item 11); the item states in the tree with their pinned legend, and the dock with Messages, Jobs and Setup checks (item 12), from a new overview query in `openchuck-app`. The UI-16 screenshots at 1366 by 768 are lane X8's.
**Review checklist**

- [ ] Launches on Windows, macOS and Linux from CI artefacts using each platform's native graphics on the reference machines; CI may use software rendering (NFR-02). *Windows: launched from a local release build on native graphics (Vulkan, RTX 2070 SUPER; owner, 14 Oct 2026). Linux: llvmpipe in this container. CI, software rendering (17 Oct 2026, CI run 39049447763, debug build from source): Linux llvmpipe, Windows WARP and macOS Apple Paravirtual Metal all start, draw and exit ([decision 0101](decisions/0101-ui-smoke-of-the-built-archives.md), OI-129). Launching the built archives is added by `ui-smoke.yml` (lane TS, decision 0101, accepted); its first run is pending (OI-321). Native graphics on macOS, Linux and the reference machines: hands-on, not done.*
- [x] Orbit, pan and zoom hold a 95th percentile frame time of 33 ms or less over a scripted 60-second camera path on 1M triangles, on the reference hardware (4 cores, 16 GB RAM, integrated graphics, one machine per platform). *Closed by the owner on 14 Oct 2026 on the margin of the product benchmark: with edges, a median p95 of 0.77 ms against 33 ms on Windows with an RTX 2070 SUPER, which is not the reference class ([decision 0029](decisions/0029-slice-8-viewport-and-shell.md), "Owner's local run"), as for OI-09. Integrated graphics and Apple M1 were not run.*
- [x] Only `openchuck-ui` and `openchuck-viewport` depend on egui and wgpu, and the UI imports no engine crate. *`cargo xtask deps` passes; `openchuck-ui` depends only on `openchuck-app`, `openchuck-viewport`, `eframe` and `rfd`.*
- [x] The A-axis direction on the asymmetric test part matches the documented convention. *Owner's visual check V2 (14 Oct 2026); `req_set_01_req_ui_03_a_arrow_follows_the_documented_convention`.*
- [x] Tree reorder, enable, disable and duplicate are all undoable. *`req_ui_06_duplicate_and_reorder_are_undoable`, `req_ui_06_disabled_operations_are_skipped_and_saved`, and the undo property test.*
- [x] Every step shows why it is unavailable when its inputs do not exist, and the footer's primary action follows the specification's rule (UI-10) *Lane X2 ([decision 0117](decisions/0117-slice-22-shell-frame.md)): `req_ui_10_each_step_says_why_it_is_unavailable`, `req_ui_10_the_primary_action_follows_the_rule_on_simulate_and_export`, and the shell's `frame_tests`; the readings of OI-350 are agreed and in the specification.*
- [x] Each machine value shows its provenance on hover and keyboard focus (UI-13) *Lane X4 ([decision 0123](decisions/0123-slice-22-inspectors.md)): `req_ui_13_each_value_says_whether_it_is_confirmed_its_source_and_how_to_confirm`, `req_ui_13_a_value_with_no_source_is_an_unconfirmed_placeholder` and the shell's `req_ui_07_req_ui_13_the_setup_inspector_shows_its_five_sections`; the icons are focusable and `field_row::tip` opens their tooltip on focus. Two states, not three, until the owner settles OI-345.*
- [x] Stale, running, error, warning and off states show in the tree with a visible legend, and each message names its source and fixing action (UI-11, UI-12) *Lane X2: `req_ui_11_an_edit_to_operation_n_marks_n_and_later_operations_stale`, `req_ui_11_a_running_job_gives_the_operation_running`, `req_ui_11_req_ui_12_a_selection_set_with_a_problem_is_an_error_up_the_tree`, `req_ui_11_req_ui_12_a_mesh_finding_warns_on_the_model`, `req_ui_12_each_source_gives_one_message_and_fixing_it_removes_the_message`; the legend is pinned under the tree.*
- [ ] Every menu command has a shortcut or menu path, and a keyboard-only run reaches all five steps (UI-14)
- [x] Layout, scale and theme survive a restart; nothing is written to the project folder (UI-15) *Lane X2: `req_ui_15_settings_round_trip_in_the_configuration_folder`, `req_ui_15_a_damaged_file_gives_defaults_and_a_message`, `req_ui_15_nothing_is_written_to_the_project_folder`, `req_ui_15_the_layout_and_scale_survive_a_restart`.*
- [ ] The reference screens at 1366 by 768 at 100% scale have no overlapping overlays or clipped text (UI-16, NFR-15)
- [x] An invalid parameter shows an inline error and leaves the project unchanged. *`req_ui_07_invalid_parameter_is_refused_and_leaves_the_project_unchanged`, the tool equivalent, and the inline error in the properties panel.*

## Slice 9 – Toolpath and simulation display, playback and background jobs

**Goal:** generate and simulate without freezing the UI, then review exactly what was generated.

**Progress:** closed by the owner (16 Oct 2026); the reference-machine measurements are still to run (OI-176, benchmark pending). Items 1 and 2 are built headless (14 Oct 2026; [decision 0030](decisions/0030-slice-9-pipeline-cache.md), accepted): the job manager existed from slice 0, and the pipeline now keeps an in-memory cache with chained keys, so editing operation N regenerates N and later operations only. Items 3 to 8 are built on branch `claude/lane-n-slice-9-playback` (lane N; [decision 0040](decisions/0040-slice-9-display-and-playback.md), accepted 15 Oct 2026, stacked on lane M's branch because both change the renderer, the viewport and the shell): toolpaths drawn by class and operation, the tool at any point with the workpiece turned by A, playback with jumps, the stock at any move from checkpoints, the part coloured by deviation, the operation add, generate and cancel flow, and levels of detail. Open: OI-131 (cache on disk), OI-132 (simulation checkpoints, progressed), OI-174 to OI-179. The limits it states (17 ms poll gap, 2 s cancel, 1,000,000 drawn points) are measured on the CPU side in one container (OI-176), so the reference-machine run is tracked there and does not hold the slice open.

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

- [x] The UI stays responsive during generation within a stated frame-time budget (17 ms poll gap while a job runs; `req_ui_09_req_nfr_04_polling_a_running_job_never_blocks_the_interface_thread`, worst 9 ms; the shell draws a whole run in `req_ui_06_req_ui_09_the_shell_generates_shows_and_plays_a_run`; OI-176 for the reference machines)
- [x] Cancel stops within a stated time and leaves no partial result (2 s at the default cell; `req_ui_09_cancel_stops_a_run_within_two_seconds_and_leaves_no_partial_result`, 6 to 12 ms on the test job; a large simulation waits for its batch, OI-116)
- [x] Editing operation N invalidates only N and later operations; unchanged operations reuse their cache (decision 0030; `req_nfr_04_editing_operation_n_regenerates_n_and_later_operations_only`, and through the add and edit flow `req_ui_06_an_operation_is_added_edited_generated_and_deleted_as_undoable_steps`)
- [x] The A rotation shown matches the post output and the LinuxCNC replay (the pose's A and the work coordinates are those of the replayed LinuxCNC post lines, `req_sim_04_jumping_to_a_move_shows_the_position_of_its_g_code_line`; the machine view turns the part by `sign * A`, `req_sim_02_the_machine_view_turns_the_part_by_the_post_sign_and_keeps_the_tool_upright`; the LinuxCNC simulator run of the same programs is gate 3 in CI)
- [x] Jumping to move N shows the same position as the corresponding G-code line (`req_sim_04_jumping_to_a_move_shows_the_position_of_its_g_code_line`; the line number is not shown, OI-175)

## Slice 10 – Geometry selection on meshes

**Goal:** pick geometry on a mesh, save it as named sets, and make an operation cut only there.

**Status:** closed by the owner (16 Oct 2026); the strict pick budget on the reference machines is still to be measured (OI-173, benchmark pending). Items 2 to 5 are built headless (lane C; project schema 7; [decision 0032](decisions/0032-slice-10-mesh-selection.md), accepted 14 Oct 2026; OI-137 and OI-138 open or deferred). Items 1 and 6 are built on branch `claude/lane-m-slice-10-picking` (lane M; [decision 0037](decisions/0037-slice-10-picking-and-selection-panel.md), accepted 15 Oct 2026): ray-cast picking in `openchuck-geom`, the picker in the scene, hover and click selection modes, highlights as a renderer overlay, the selection panel, operation roles and the visible warnings (OI-12 and OI-139 closed; OI-170 to OI-173 open). Not built: B-rep picking (slice 11b, lane O), and the offer to select again (OI-172). Follow-up on branch `claude/lane-m-slice-10-picking` (lane M, [decision 0046](decisions/0046-slice-10-selection-ui-to-specification.md), accepted by the owner 16 Oct 2026): the selection interface is brought to the UI specification of decision 0038 (D7: blue working fill, yellow hover outline, a swatch for each set; the tools box, keys Q, W, E, R and Esc, the Selection sets table); project schema 11 (an optional stored set colour); OI-202 closed, OI-199 to OI-201 open. Review checklist: all six boxes are met by the tests named in the records; the strict pick budget has been measured in one container only (OI-173), so the reference-machine run is tracked there and does not hold the slice open.

**Requirements:** UI-04 (mesh), UI-05, PRJ-06 (mesh), OP-02

**Design refinement:** the selection model and geometry reference (mesh selections are triangle sets and apply to mesh models only), CPU ray-cast picking against a BVH in `openchuck-geom`, and how region constraints reach `openchuck-path`. Define the behaviour for a reference that cannot be resolved: flag the operation, never silently drop it.

1. Ray-cast picking and hover highlight; the viewport only draws the highlight.
2. Triangle region selection, including select-by-angle and flood fill, in `openchuck-geom` (UI-04).
3. Named selection sets in `openchuck-project` that operations reference by ID and that update when the set is edited (UI-05).
4. Persist selections so they survive save and load of the same model (PRJ-06).
5. Limit an operation to selected regions, with optional containment boundary and optional avoid and check surfaces (OP-02), wired to the helical strategy.
6. Selection panel listing sets and their contents.

**Review checklist**

- [x] Select, save, close and reopen: the same set is highlighted (`openchuck-app/tests/selection.rs`, and `req_ui_05_a_saved_set_is_highlighted_again_after_the_project_is_reopened` in `openchuck-ui`)
- [x] Editing a set marks dependent operations for regeneration (`req_ui_05_editing_a_set_marks_exactly_the_operations_that_use_it`; the pipeline cache key, OI-140)
- [x] Flood fill and select-by-angle give expected counts on a reference mesh (`openchuck-geom/tests/select.rs`, `openchuck-app/tests/selection.rs`)
- [x] An avoid surface is never cut in the test case (`openchuck-path/tests/region.rs`, `openchuck-app/tests/selection.rs`)
- [x] Picking on a 1M-triangle model stays within its time budget (`req_ui_04_picking_a_million_triangles_stays_within_its_budget`: 4.4 µs and 1.0 s against 1 ms and 2 s; OI-173 for the reference machines)
- [x] A reference to missing geometry shows a visible warning (the tree, the panel and the operation; `req_op_02_a_set_made_on_another_version_of_the_model_shows_a_warning`; a triangle beyond the end of the model is found at generation, OI-172)

## Slice 11 – STEP import and B-rep selection

**Goal:** open a STEP file, keep its exact faces and edges, and select them.

**Requirements:** IMP-02, IMP-06, NFR-13 (also extends UI-04 and PRJ-06 to B-rep)

**Spike inputs:** SP-1 (kernel), SP-9 (exact surfaces).

**Status:** 11a built on branch `claude/busy-lovelace-khabqi` ([decision 0027](decisions/0027-slice-11a-step-import.md), accepted 14 Oct 2026): items 1 to 4, 6, 8 and 9 (wrapper and worker process, topology and deterministic numbering, units, tessellation with face IDs per triangle, the exact `BrepSurface`, the corpus and mutation run, the three-platform kernel CI). 11b is items 5 and 7 (viewport drawing and picking, B-rep selections in the project; OI-119), after slices 8 and 10. The slice is not closed: its review checklist is ticked below only for what 11a proves, and OI-25 (STEP from other CAD tools, deferred by the owner), OI-119 and OI-120 are open. The three-platform kernel CI passed on pull request 46 (15 Oct 2026). This is lane D of "Parallel work" (nothing needed merged first). It takes **no project schema number**: the units override for STEP is the existing model-reference override (schema 2), and the B-rep selection storage that row 7 of the claims table expects belongs to 11b (lanes D and O). The new B-rep fields took the library family to 0.5.0 (OI-123, closed). The nightly archives now carry the STEP worker (OI-118, closed) and a sanitizer job covers the wrapper (OI-19, closed). 11b (items 5 and 7) is built by lane O on `claude/lane-o-slice-11b-brep-selection` ([decision 0066](decisions/0066-slice-11b-brep-selection.md), accepted) behind seams that take a placed B-rep, at the orchestrator's direction independent of lane Z (decision 0056, which reads a STEP part into the application): a STEP part's scene item draws its own edges and is picked by face and edge (an edge within 6 pixels of the pointer, never through the part, else the face; an optional Both, Faces or Edges filter in the tools box, owner's decision on item 3), highlighted to specification 7.4, and face and edge sets are checked, saved and edited against the model's content hash; the UI offers no triangle selection on a STEP model. Project schema 12 adds the stored mesh edge kind (OI-204) and a keep-tangent-edges choice per edge set with a project default (owner's request). The wiring to lane Z's placed B-rep, with the kernel test of review box 3 (`crates/openchuck-app/tests/step_selection.rs`), is in the same pull request (OI-242 and OI-119 closed). Lane S11 ([decision 0112](decisions/0112-slice-11-closing-items.md), accepted) then closed OI-243, OI-245 and OI-332 (the edge-pick hierarchy and its measurement, wider hovered and selected edges, save and edit checks as jobs); OI-335 (the edge widths) closed with the owner's approval.

**Design refinement:** the kernel route chosen in SP-1 (a thin in-house wrapper over OpenCASCADE with an isolated import process, [decision 0003](decisions/0003-step-kernel-route.md)), the deterministic face and edge numbering, how exact evaluation reaches `openchuck-path` ([decision 0017](decisions/0017-sp-9-exact-surfaces.md): the importer emits a face record with the analytic frame or a clamped, unwrapped rational B-spline patch, UV bounds, trim loops and the tessellation, and the library evaluates it in pure Rust; shim additions `occ_face_frame`, B-spline unwrapping and trim sampling; the SP-9 comparison kept as a regression test), and the licence boundary (the LGPL kernel stays in `openchuck-import`).

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

- [x] A reference STEP part matches its CAD dimensions within tolerance (`req_imp_02_box_matches_its_cad_dimensions`: 20 x 30 x 40 mm, area to 1e-6; the other fixtures' radii are exact)
- [x] Changing the tessellation tolerance changes triangle count and deviation as documented (`req_imp_06_tolerance_controls_triangle_count_and_deviation`, numbers in decision 0027 item 7)
- [x] Face and edge IDs are identical after save and reload, and across two imports (`req_ui_04_req_prj_06_faces_and_edges_of_a_step_file_are_the_same_after_reopening_and_a_second_import` on the STEP kernel job: a project's face and edge sets, the scene's faces and edges and the picks are the same after reopening and when the file is imported again; [decision 0066](decisions/0066-slice-11b-brep-selection.md))
- [x] Exact versus mesh evaluation is measured on a curved surface and the difference recorded (exact 7e-15 mm, mesh 9.2e-3 mm on a sphere; `step_exact`)
- [x] The kernel builds on all three platforms in CI (the STEP kernel job passed on Windows, macOS and Ubuntu on pull request 46, 15 Oct 2026)
- [x] Malformed STEP gives an error, not a crash (6 malformed and 12 crash files; 150 mutants, 46 reader crashes caught on Linux)

## Slice 12 – Baseline finishing: helical, axial, indexed

**Goal:** the three baseline finishing strategies work on selected regions with real operation parameters.

**Requirements:** OP-04 (completes slice 5 work); continues OP-11

**Depends on:** slices 5 and 10; the entry moves of [decision 0026](decisions/0026-entry-moves.md) (OI-80) for item 5; the stronger offset result check (OI-82) once regions come from meshes.

**Status:** closed by the owner (16 Oct 2026). Design in [decision 0034](decisions/0034-slice-12-baseline-finishing.md) (accepted 15 Oct 2026, lane H, merged in pull request 46): axial and indexed finishing, scallop control for helical and axial finishing, per-selection stock to leave (project schema 9), region offsets (OP-02: a selection is found through its offset; OI-138 closed, OI-137, the containment boundary as a curve, stays open), `lead` entry on all three strategies, golden jobs `cone_axial`, `cone_indexed` and `cone_axial_lead`. OP-11 and OP-02 close at slice 14.

**Design refinement:** how each strategy parameterises stepover and scallop, how region constraints enter the unrolled 2D development, and what the indexed form outputs for positioning moves.

1. Helical finishing on a selected region (spiral around the axis).
2. Axial finishing, parallel to the axis and stepping in A.
3. Indexed finishing at fixed A positions (3+1 positional), with positioning moves.
4. Tolerance, stepover and scallop controls; stock to leave per operation and per selection.
5. Lead-in and lead-out, ramping, as strategy requests for the shared entry moves in `assemble` (decision 0026, OI-80).
6. Strategy parameter schemas so the forms are generated; golden files for each.

**Review checklist**

- [x] A property test shows every point of the selected region lies within stepover of the path (whole-part case on cones: `req_op_04_every_surface_point_is_within_the_stepover_of_the_path`; the region case is covered by `req_op_02_req_op_04_a_band_narrower_than_the_old_spacing_is_cut_on_every_pass`)
- [x] The independent checker reports no gouging beyond tolerance on the reference parts (`req_op_04_baseline_finishing_golden_files_pass_the_audit`: the checker runs on `cone_axial`, `cone_indexed` and `cone_axial_lead` for both posts, gouge within 0.02 mm; `req_op_04_axial_leaves_no_gouge_and_no_uncovered_surface_on_a_cone` for axial; helical since slice 5)
- [x] Indexed positions land on the exact requested angles (`req_op_04_indexed_positions_land_on_the_requested_angles_in_order`)
- [x] Per-selection stock to leave is respected (`req_op_11_each_zone_is_left_its_own_stock_with_all_three_strategies`, `req_op_11_a_selection_is_left_its_own_stock_in_place_of_the_operations`)
- [x] All three replay cleanly in the LinuxCNC simulator (the Gate 3 job passed on pull request 46 for the helical, axial, indexed and axial-with-lead programs)

## Slice 13 – True simultaneous 4-axis finishing

**Goal:** A varies continuously with X, Y and Z to follow the surface, with the tool held near the surface normal.

**Status:** closed by the owner (15 Oct 2026). Built on branch `claude/lane-i-slice-13-simultaneous` (lane I); design in [decision 0036](decisions/0036-slice-13-simultaneous-finishing.md), accepted: items 1 to 7 as the record's table says, with the drop on exact faces still using their tessellation (OI-81). Lead and tilt as the record defines them (OI-145, closed), with explanatory tooltips in the UI; not building SP-4's climb for meshes is a deferred future optimisation (OI-146); surfaces hidden from a ray from outside are a P1 limitation, and supporting them is requirement OP-20 (P2; OI-148 closed). The gate 3 replay of the new `lobed` reference job passed in the slice's pull request (OI-147, closed). No project schema number.

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

- [x] On surfaces with known normals, tool-angle error stays within the stated bound (`max_tilt` plus 0.03 degrees; decision 0036, Verification)
- [x] A reversal count is lower than the unconstrained version (120 following the normal, 45 within 10 degrees, none within 20)
- [x] The independent checker reports zero gouge on the corpus (at most 0.00008 mm, the mesh's own error, on four parts and eight tool types)
- [x] Lead and tilt limits and A range are obeyed
- [x] Simulator replay passes (the `lobed` reference job, gate 3 in pull request 48's CI, OI-147 closed); deviation against the helical result on the same surface is recorded (up to 17.9 degrees of A and 4.34 mm across the axis on the lobed part)
- [x] Results are identical at 1, 2 and N threads

## Slice 14 – Rotary roughing

**Goal:** remove bulk material in indexed and continuous forms, aware of what is already gone.

**Requirements:** OP-03; completes OP-11

**Depends on:** slice 7 (the `StockModel`), the entry moves of [decision 0026](decisions/0026-entry-moves.md) (OI-80), and slice 10 for item 5 (containment, OP-02).

**Design refinement:** layer definition for rotary parts using the wrap-family approach, how `StockModel` queries avoid air cutting, and entry and retract rules.

1. Stock-aware roughing using the `StockModel` port implemented by `openchuck-sim`.
2. Indexed (3+1) roughing: layers at fixed A positions.
3. Continuous roughing in rotary layers.
4. Stepdown, stock to leave, ramped and helical entry (through the shared entry moves of decision 0026, OI-80) and retract strategy, completing OP-11.
5. Containment to selected regions (OP-02).
6. Link moves checked against the remaining stock, not only the clearance cylinder (OP-14).
7. Simulation check and leftover report.

**Status:** lane E built items 1 to 4, 6 and 7 on `claude/slice-14-rotary-roughing` ([decision 0033](decisions/0033-slice-14-rotary-roughing.md), 15 Oct 2026; in a pull request). Item 5 (containment, lane L) is built on `claude/lane-l-slice-14-containment` ([decision 0043](decisions/0043-slice-14-roughing-containment.md), accepted; OI-187 and OI-188 closed); per-selection stock to leave stays refused for roughing (item 5 of that record); lane L merged on 16 Oct 2026. The slice stays open on one checklist box (every OP-11 parameter has a test), because per-selection stock to leave is not supported for roughing; the owner chose on 17 Oct 2026 that roughing leaves at least the largest value ([decision 0058](decisions/0058-roughing-stock-to-leave-per-selection.md), accepted), and lane E2 built it on `claude/lane-e2-roughing-stock-per-selection` (17 Oct 2026; no number taken), which ticks that box; exact per-zone roughing is OI-261, deferred. The slice is ready for the owner to close. OI-117 is closed; OI-150 to OI-153 are open.

**Review checklist**

- [x] Simulated leftover is uniform and matches stock to leave within tolerance (cylinder: 0.45 to 0.53 mm for 0.5 mm to leave at a 0.1 mm cell; cone and indexed forms within the stepover's scallop; strips the tool cannot reach are reported, OI-150)
- [x] A second roughing pass does not re-cut already removed material (`req_op_03_a_second_roughing_run_over_its_own_result_cuts_nothing`)
- [x] Indexed and continuous forms reach the same final stock envelope (to within the scallop of the stepover, `req_op_03_indexed_and_continuous_forms_reach_the_same_envelope`)
- [x] Every OP-11 parameter exists and has a test (stepdown, stock to leave per operation, entry styles and retract do; per-selection stock to leave is slice 12 item 4, and for roughing the largest amount over the whole part, `req_op_11_req_op_03_*` in `crates/openchuck-app/tests/selection.rs`, [decision 0058](decisions/0058-roughing-stock-to-leave-per-selection.md))
- [x] Link moves never cross the stock (`req_op_14_links_stay_below_the_clearance_cylinder_without_cutting`, simulated, no rapid cuts)

## Slice 15 – Rest machining and pencil clean-up

**Goal:** clean up material that earlier operations could not reach.

**Requirements:** OP-09

**Depends on:** slice 7 (carry-over, SIM-06) and slice 9 items 1 and 2 (pipeline invalidation, item 4).

**Design refinement:** how remaining material is found from the carried-over stock (SIM-06) against the target through the `StockModel` port, and the threshold that counts as worth cutting.

1. Remaining-material detection from simulated stock versus target.
2. Rest strategy limited to areas the previous tool's radius could not reach.
3. Pencil paths along concave creases.
4. Ordering and dependency handling between operations, using the pipeline invalidation from slice 9.
5. Tests on parts with tight inside corners.

**Status:** lane J built items 1 to 5 on `claude/lane-j-slice-15-rest-machining` ([decision 0044](decisions/0044-slice-15-rest-and-pencil.md), accepted 16 Oct 2026). Remaining material is found through the `StockModel` port from the carried stock; `rest_machining` is helical finishing cut only where material is left; `pencil_finishing` follows concave mesh creases. Not built: selections (OI-190), indexed and simultaneous rest forms (OI-189), pencil side passes (OI-192). OI-191 and OI-193 are open. The record is accepted, so the slice is closed; OI-189 to OI-193 carry the follow-ups.

**Review checklist**

- [x] The rest operation cuts only where material remains above the threshold (a Ø1 ball after a Ø6 ball cuts X 9.5 to 13.0 of a 24 mm part; stock at the part has no sample)
- [x] Worst remaining value drops after the rest operation (1.22 to 0.24 mm; 1.86 to 0.35 mm at a true wall; no overcut)
- [x] Pencil paths follow the crease within tolerance without gouging (within 0.02 mm of the closed form; no overcut in the simulation)
- [x] Nothing to clean gives a clear message, not an empty or odd path (an error naming the threshold; OI-172 asks whether it should stop the run)

## Slice 16 – Edge and curve following

**Goal:** engrave or carve along selected edge chains or curves projected onto the surface.

**Requirements:** OP-08

**Depends on:** slice 10 for mesh feature edges and slice 11 for B-rep edges (item 1).

**Design refinement:** the `CurveSource` trait, the projection method, and how A and tool axis behave along a curve. This is a wrap-family strategy and reuses the unrolled-development approach from SP-7.

**Status:** lane K built the mesh feature edge side on `claude/lane-k-slice-16-mesh-edges` ([decision 0047](decisions/0047-slice-16-edge-following-mesh.md), accepted 16 Oct 2026, relayed by the orchestrator session at the owner's instruction): the `CurveSource` seam and `MeshFeatureEdges` (item 1 for meshes), the radial projection and depth (item 2), A and the tool axis along the curve (item 3), engrave and carve with levels and V-bit width following depth (item 4), and tests on open and closed chains on a curved surface (item 5). The slice is not closed: the B-rep half of item 1 is lane P's (below), and a stored edge selection (the "persist across save and load" box) needs a new selection kind and a project schema number (OI-204). Follow-ups OI-205 to OI-208. It takes no schema number.

Lane P built the B-rep edge source on `claude/lane-p-slice-16-brep-edges` ([decision 0053](decisions/0053-slice-16-edge-following-brep.md), accepted 16 Oct 2026 by the owner): `BrepEdges`, a `CurveSource` over a B-rep's own edge polylines that plugs into `EdgeFollowing::with_source`, with the rule for which edges are real (seams never; edges between tangent faces dropped below a 1 degree crease angle, kept at 0; boundary and non-manifold edges always), chains joined by corner, deterministic order, and `with_edges` for the edges of a chosen `Edges` target. It takes no project schema number and touches neither the viewport nor the UI. Not wired into the application: the pipeline has no B-rep path yet (OI-226). Follow-ups OI-226 to OI-230. Lane O built the stored edge selections ([decision 0066](decisions/0066-slice-11b-brep-selection.md), accepted): project schema 12 adds the `mesh_edges` selection kind, edges are picked in the viewport, and edge following follows a mesh's chosen edges (`ChosenSegments`); B-rep edge sets use the existing `Edges` target, with a keep-tangent-edges choice per set and a project default, which reaches edge following on a STEP part through lane Z's seam (wired in the same pull request, OI-242). Every review box is now ticked; the slice closes when lane O has merged and the owner closes it.

Lane Z wired the B-rep source into the application on `claude/lane-z-brep-pipeline` ([decision 0056](decisions/0056-brep-path-through-the-pipeline.md), accepted 17 Oct 2026): edge following on a STEP part follows its B-rep edges (or the edges of a selected `Edges` set) with the operation's crease angle, 1 degree when unset, and shortest chain (OI-226). A crease angle of 0 cannot be set from the operation (OI-232).

1. Edge chains from B-rep edges and mesh feature edges as curve sources.
2. Projection onto the surface, with depth control.
3. Tool-axis and A handling along the curve.
4. Engrave and carve modes with multiple passes, including V-bit engraving where line width follows depth.
5. Tests on open and closed chains on a curved surface.

**Review checklist**

- [x] Engraved depth is constant within tolerance on a curved surface (`req_op_08_engraved_depth_is_constant_on_a_curved_surface`, open and closed chains; mesh edges, lane K; B-rep edges, lane P: `req_op_08_brep_engraved_depth_is_constant_on_a_curved_surface_closed_chains` and `req_op_08_brep_engraved_depth_is_constant_along_an_open_helix_on_a_curved_surface`)
- [x] Chain direction and start point are controllable (`req_op_08_chain_direction_and_start_point_are_controllable`)
- [x] Edge selections persist across save and load (lane O, [decision 0066](decisions/0066-slice-11b-brep-selection.md), accepted: mesh edges in project schema 12, `req_prj_06_req_op_08_a_mesh_edge_selection_survives_save_and_load` and `req_op_08_req_prj_06_chosen_mesh_edges_survive_reopening_and_are_the_only_ones_followed`; B-rep edges in the existing `Edges` target with the keep-tangent-edges choice, `req_prj_06_req_op_08_keep_tangent_edges_survives_save_and_load_per_set_and_for_the_project`)
- [x] Simulator replay passes (`crates/openchuck-sim/tests/edge_following.rs`)

## Slice 17 – Swarf (flank) machining

**Goal:** cut ruled or near-ruled surfaces with the side of the tool.

**Requirements:** OP-07

**Spike inputs:** SP-11 (swarf).

**Depends on:** slice 11 (B-rep faces and the exact `SurfaceSource`): swarf is B-rep only for P1 and the UI says so for mesh selections ([decision 0021](decisions/0021-sp-11-swarf.md), OI-96).

**Design refinement:** the detection method and deviation tolerance from SP-11, which tool shapes can do flank cuts, and whether P1 is limited to analytic ruled surfaces if the spike falls back.

1. Detect ruled or near-ruled selected surfaces and fit rulings, reporting deviation.
2. Toolpath keeping flank contact along the rulings (continuous family).
3. Support for straight and tapered tools.
4. Warn or refuse when deviation exceeds tolerance.
5. Tests on cylinder, cone and helicoid surfaces.

**Review checklist**

- [x] Flank deviation is within tolerance on ruled test surfaces (cylinder, draft cone with the matching taper, plane, helicoid of small pitch; decision 0061)
- [x] A clearly non-ruled surface is refused with a clear message (sphere, torus; a helicoid of large pitch is refused after placement, naming the twist; a cone with a straight tool, naming the tilt and the taper that would remove it)
- [x] The independent checker reports no gouge; A limits are respected (the checker sweeps the machine's motion; the shared A-axis management refuses a face that spans more than the range)
- [x] Simulation result matches the target (the dexel stock is cut to the boss wall and no deeper; through the application, the `swarf` reference job cuts the six flats of a STEP hexagonal bar and nothing of the part, [decision 0111](decisions/0111-swarf-through-the-application.md))
- [x] Simulator replay passes (gate 2, the replay of the posted program within rounding, passes for the library test and for the `swarf` reference job made through the application from a STEP file; gate 3, the LinuxCNC simulator, replayed `swarf_linuxcnc.ngc` in the lane Q2 session, 18 Oct 2026: all 54 moves reached, largest difference 0.0039, no fault, and CI's Gate 3 job replayed it on pull request 120, run 39165411513, all three shares; OI-238 closed, recorded in pull request 122)
- [x] Swarf on a mesh part says it is B-rep only (decision 0021, OI-96): the operation's form and the generation error, naming the part ([decision 0111](decisions/0111-swarf-through-the-application.md) item 3)

**Status:** lane Q built items 1 to 5 as a library strategy on `claude/lane-q-slice-17-swarf` ([decision 0061](decisions/0061-slice-17-swarf.md), accepted 17 Oct 2026, relayed by the orchestrator session at the owner's instruction): the faces reach the strategy through `SurfaceSource::exact_faces`, rulings are fitted per iso-line family, the flank is the straight part of the tool's profile, A comes from `a_facing` of the projected axis, poses are chord-limited with a 5 degree A step, and the swept-motion gate refuses a gouge, notes material left beyond the tolerance and refuses it beyond `max_material_left` (0.1 mm by default, configured per operation). Lane Q2 wired it through the application on `claude/lane-q2-swarf-wiring` ([decision 0111](decisions/0111-swarf-through-the-application.md), accepted 18 Oct 2026, relayed by the orchestrator session at the owner's instruction, as revised): swarf on a STEP part requires a face set, which chooses the walls it cuts and is checked before it runs to be one swarf can cut, every face it cannot cut named (the owner's change, amending decision 0061 item 1 for the application; the application had refused face sets for every strategy but drilling, so swarf could not run on a real part), swarf on a mesh part says it is B-rep only in the form and when generated, and the `swarf` reference job (the OpenCASCADE fixture `hex_bar`, its six flats picked in the scene and cut with a 6 mm flat end) passes gates 1 and 2 with both posts against its golden files, is identical at 1, 2 and 4 threads, and is simulated with nothing cut into the part. Gate 3 passed in the session (LinuxCNC 2.9.10, all 54 moves, no fault) and replays the program with the other reference jobs in CI (OI-238 closes when it has). The slice stays open until then; OP-07 is not in `closed_slices`. Open items OI-237 and OI-239 to OI-241; OI-238 waits on gate 3; OI-97, OI-98 and OI-258 closed.

## Slice 18 – Radial drilling

**Goal:** drill holes at indexed A positions, selected from model geometry.

**Requirements:** OP-10

**Depends on:** slice 11 (cylindrical faces) and slice 10 (selection sets). Hole detection on meshes is not planned.

**Design refinement:** hole detection from cylindrical faces, how each hole's axis maps to an A angle, and whether canned cycles or expanded moves are output (expanded by default). It uses the drill tool type from TL-03, now P1 (finding 7).

1. Hole selection from model geometry, with axis, diameter and depth.
2. Indexed A positions per hole, ordered to minimise rotation.
3. Drilling moves with peck depth, as expanded moves.
4. Post option to output canned cycles.
5. Tests on holes at several angles.

**Status:** lane R built items 1, 2, 3 and 5 as a library strategy on `claude/lane-r-slice-18-drilling` ([decision 0051](decisions/0051-slice-18-radial-drilling.md), accepted 16 Oct 2026): holes are found from the cylindrical faces of a B-rep (`SurfaceSource::holes`, selected by face through `BrepSurface::with_faces`), each is one pass along its own axis at its own A, with equal pecks as expanded moves (`Pass` gains `rapids`, so the workspace is 0.8.0), ordered in one sweep of the angles. Item 4 (canned cycles in the post) is not built (OI-216). Lane Z wired it into the application on `claude/lane-z-brep-pipeline` ([decision 0056](decisions/0056-brep-path-through-the-pipeline.md), accepted 17 Oct 2026): a STEP part is drilled through the pipeline, its holes chosen by a selected `Faces` set or all of them, and the `drilling` reference job (the OpenCASCADE fixture `rotary_blank_drilled`, turned 30 degrees, in pecks) passes gates 1 and 2 against its golden files; gate 3 replays its LinuxCNC program with the other reference jobs, and did so in CI on pull request 79 (OI-219 closed, 17 Oct 2026). Face sets are made by command until lane O's viewport picking; a per-hole list is OI-235 and a fixture with several holes OI-234. The slice stays open; OP-10 is not in `closed_slices`. Open items OI-216 to OI-220.

**Review checklist**

- [x] Hole positions and angles match the model within tolerance (seven holes at several angles, flat, pointed, split and through, an offset one: the tool line is the hole's axis line to 1e-9 mm and the hole is vertical on the machine)
- [x] Peck and retract heights are correct and stay clear of the part (exact peck depths in both modes, rapid retracts, links at the clearance radius, stock above the opening raises the retract height)
- [x] Simulation shows the correct hole depth (dexel stock at 0.1 mm: each hole open to the model's depth within two cells, no deeper, as wide as the tool, with and without pecks)
- [x] Simulator replay passes (gate 2, the replay of the posted program within rounding, passes; gate 3, the LinuxCNC simulator, replays the `drilling` reference job made through the application from a STEP file, [decision 0056](decisions/0056-brep-path-through-the-pipeline.md): passed in CI on pull request 79, 17 Oct 2026; OI-219 closed)

## Slice 19 – Output options and setup sheet

**Goal:** export G-code in the groupings a shop uses, and a printable job summary.

**Status:** in progress. Lane F (items 1, 2, 3 and 5) is built on branch `claude/awesome-sagan-wqv9bz`; design in [decision 0028](decisions/0028-slice-19-output-options-and-setup-sheet.md) (accepted 15 Oct 2026). Item 4 (export dialog, tool-number dialog, CLI options) is built by lane S on `claude/lane-s-slice-19-export-dialogs` ([decision 0071](decisions/0071-slice-19-export-dialogs.md), accepted, in a draft pull request); the slice is not closed here, the owner closes it, and until then GC-05, GC-06, GC-12 and GC-14 are not yet closed in `xtask/traceability.toml`. Open items OI-92 (narrowed), OI-143, OI-248 to OI-251 and OI-257 (OI-144 closed).

**Requirements:** GC-05, GC-06, GC-12, GC-15, GC-16 (GC-14 in the UI)

**Depends on:** slice 6. Items 1 to 3 and 5 are headless; the export dialogs of item 4 need slice 8.

**Design refinement:** file naming and grouping rules, comment content, the setup sheet layout, and the HTML and PDF generation approach, assessed under NFR-14 because the architecture does not score it. Decide whether IR-duration totals appear (finding 6); otherwise times are left out until SIM-08.

1. Output as one file for the project, or one per setup, operation or tool (GC-05).
2. Comments naming the operation and tool (GC-06).
3. Setup sheet as HTML and PDF listing tools, operations and stock, built as a module in `openchuck-app` (GC-12).
4. Export UI and CLI options, including the tool-number dialog (GC-14; the application side is built in slice 6), the checks that block writing (GC-15) and the safety acknowledgement kept per project on each computer (GC-16), as specification section 11.6 describes. **Built (lane S, [decision 0071](decisions/0071-slice-19-export-dialogs.md), accepted):** the dialog in `openchuck-ui` over `openchuck-app`'s export checks, file plan and acknowledgement store; `regenerate --grouping` and `--setup-sheet` in the CLI; nothing stored in the project.
5. Golden files for each grouping and a snapshot of the setup sheet content.
6. Traceability block on the setup sheet (GC-12, [decision 0110](decisions/0110-setup-sheet-traceability.md), accepted 17 Oct 2026; not started): the time and date it was generated (ISO 8601 with the time zone, from an injected clock so the golden snapshot stays deterministic), when the project was last saved (the project file's modification time, nothing stored in the project; "not saved yet" when there is none), the software version, a fingerprint of the saved project file, and a version-control line that reads "not tracked" until `openchuck-vcs` exists. A small lane builds it after lane S19's pull request merges, since both change the setup sheet.

**Review checklist**

- [x] Per-tool files together contain the same moves as the single file (`req_gc_05_every_grouping_reaches_the_same_moves` in `openchuck-app/tests/output_options.rs`: the sorted end positions of the fed moves of every grouping equal the setup file's, and every file passes gates 1 and 2; `req_gc_05_one_file_per_tool_keeps_the_machining_order` and `req_gc_05_one_file_per_tool_gathers_consecutive_operations_of_one_tool` for the tool files)
- [x] Comments are present and deterministic (`req_gc_06_comments_name_the_operation_and_the_tool_by_name_and_number`, `req_gc_06_a_name_with_brackets_or_accents_is_made_safe_in_the_comment`; determinism: `req_gc_05_req_gc_07_every_grouping_gives_the_same_bytes_at_any_thread_count` and the grouping golden files `req_gc_05_req_nfr_08_each_grouping_matches_its_golden_files`)
- [x] The setup sheet lists every tool used with its controller tool number (`req_gc_12_the_sheet_lists_every_tool_used_with_its_controller_number`; the tool numbers come from the dialog in `req_gc_14_numbers_chosen_in_the_dialog_are_saved_and_give_the_same_bytes_again`; snapshot `req_gc_12_req_nfr_08_the_sheet_content_matches_its_snapshot`)
- [x] The sheet opens offline and prints legibly (owner accepted the printed sheet, 18 Oct 2026, confirmed by the owner in lane BM's session; OI-330 closed). **Offline is evidenced** (`req_gc_12_the_html_opens_offline`: no script, link, `src`, `@import`, `url(` or `http`). **Prints legibly is evidenced only in part** (`req_gc_12_the_pdf_text_fits_inside_the_page_and_the_html_has_print_rules`: every PDF text line ends inside the page margins in its fixed-width font, and the HTML sets A4, a margin, black on white and wrapping cells; `req_gc_12_a_long_title_and_heading_wrap_in_the_pdf_and_lose_nothing`, `req_gc_12_a_long_sheet_runs_onto_more_pages_and_keeps_every_row`). No test can show how the HTML and PDF look on paper or in a viewer, so the box waited until the owner had looked at a printed or previewed sheet (OI-330).
- [x] Writing is refused with a stale or failed enabled operation; the acknowledgement is asked once per project per computer and never stored in the project (GC-15, GC-16) (`req_gc_15_an_edit_makes_that_operation_and_the_ones_after_it_stale`, `req_gc_15_a_failed_generation_blocks_and_says_why`, `req_gc_15_writing_is_refused_before_anything_is_generated`, `req_gc_15_disabled_operations_are_listed_as_not_posted_and_do_not_block` in `openchuck-app/tests/export_dialog.rs`; the dialog refuses to write in `export_panel::tests::req_gc_05_req_gc_14_req_gc_15_req_gc_16_the_dialog_writes_files_once_every_check_passes`; `req_gc_16_the_acknowledgement_is_per_project_per_settings_folder_and_never_in_the_project` shows one question per project and settings folder, a fresh folder asks again and the project's files are byte-identical. The failed state is for the whole run, not per operation: OI-249)

## Slice 21 – Workholding and retaining tabs

**Status:** 21a built and merged (lane U, [decision 0041](decisions/0041-slice-21a-workholding-data.md), accepted, pull request 55); 21b built and merged by lane V ([decision 0042](decisions/0042-slice-21b-composite-target.md), accepted 16 Oct 2026; OI-185 open, OI-162 measured); 21c built by lane W on `claude/lane-w-slice-21c-wiring` ([decision 0045](decisions/0045-slice-21c-wiring.md), accepted 16 Oct 2026; OI-195 to OI-198 open, OI-209 and OI-180 closed): the held regions and tabs join the target of every operation, the simulation keeps them, the deviation report counts them as part, the setup form, the viewport and `openchuck-cli setup` edit them, default tabs are computed on request, and the `cone_held_tabs` job is in the golden set. **OI-209 is closed**: a default tab's small end meets the part's end face (a wall), which made helical finishing fail gates 1 and 2 on micro-moves; lane Y fixed it in path assembly ([decision 0050](decisions/0050-walls-merge-micro-moves.md), accepted 16 Oct 2026, relayed by the orchestrator session at the owner's instruction; OI-211 to OI-213 open), and the default tabs now pass the gates through the application (`req_op_13_req_op_14_req_gc_07_req_nfr_06_default_tabs_pass_gates_1_and_2` in `openchuck-app/tests/workholding.rs`: gates 1 and 2 with both posts, the same G-code at 1, 2 and N threads). The slice is not closed here; the owner closes it. Added by [decision 0035](decisions/0035-workholding-and-retaining-tabs.md) (accepted). Split into three sub-slices so slices 12, 13 and 14, already in progress, change in no way: strategies keep working on one target surface and one stock model, and the held region and the tabs join the target before any strategy sees it.

**Goal:** the user says how deep the chuck grips and where the part stays joined to the stock, as a cylindrical or frustum tab, and every operation leaves those regions alone.

**Requirements:** SET-09, OP-21; UI-25 (added by decision 0104; built by 21d)

**Design refinement:** the record's items: the held region and each tab as analytic solids (cylinder, frustum) added to the part for the whole job, where they live (the setup, beside stock), the composite target surface and its determinism, what stock to leave, the gouge checker and the deviation report do with them, and the cut-off.

**21a – Data and setup (lane U)**

1. Types in `openchuck-types`: a held region at each end of the stock (gripped length, optional holding diameter) and a list of tabs (shape: cylinder or frustum; axial position; diameter, or both end diameters for a frustum; for tabs around the axis, count and start angle), validated against the stock and each other (SET-09, OP-21).
2. Setup fields in `openchuck-project` and the project files, a schema bump taken under "Project schema ordering", migration and fixture, undoable commands (UI-08, PRJ-04, PRJ-05).
3. CLI `validate` and `report` state the held regions and tabs and refuse impossible ones (NFR-07).

**21b – Composite target (lane V)**

1. In `openchuck-path`, an analytic surface for a cylinder and for a frustum that implements the surface port exactly, with the tool drop, point, normal, distance and ray queries (extending the analytic evaluators of decision 0017).
2. A composite surface that joins the model's surface and these solids (the highest contact for a drop, the nearest for a distance), without changing the surface port or adding a field to a public struct; deterministic at any thread count (NFR-06).
3. Tests on the oracle parts with a tab and a held region, against the independent gouge checker.

**21c – Wiring (lane W)**

1. Mapping in `openchuck-app` from the setup to the composite target for generation, the gouge checker's target, the simulation's target and the deviation report; the simulated stock keeps the held region and the tabs (SET-09, OP-21, SIM-05, SIM-06).
2. Parameter forms and the viewport show the held region and tabs (UI-03, UI-07), with a warning if an operation's selection reaches one.
3. Default tabs when none are set, from the part's extent (decision 0035, owner's answers, OI-158): two on the axis, one at the head and one at the tail, frustums with the small end on the model, each the smaller of 10 mm and half the part's local diameter across; the head tab runs from the held region's inner edge to the start of the model and the tail tab from the model's end to the stock's end, kept out of held regions and user-excluded regions. Tabs are restricted to the head and tail zones, and 21c refuses one outside them (OI-180). A tab narrower than the tool is refused (OI-159).
4. The example tab and held-region project in the golden set, with G-code through all gates (GC-07, NFR-08).

**21d – Interface specification, display and verification (requirement agreed 17 Oct 2026; specification and mock-ups written by lane 21d on `claude/lane-21d-workholding-ui-spec`, in a draft pull request; the code and tests wait until lane O (pull request 83, B-rep selection and viewport) has merged; added at the owner's request)**

Today the workholding panel (`workholding_panel.rs`, edits a draft of the held regions and tabs) and the viewport's held-region and tab colours exist (21c item 2), but no requirement and no part of the interface specification says that held regions and tabs are shown or edited in the interface: SET-09 and OP-21 describe the data and the machining, and UI-03 lists only the rotary axis, work coordinate system, stock and part. 21d closes that gap.

1. The requirement, agreed by the owner on 17 Oct 2026 ([decision 0104](decisions/0104-held-regions-and-tabs-in-the-interface.md)): UI-03 now lists held regions and retaining tabs among what the viewport shows, and the new P1 requirement UI-25 says the user can view, add, edit and remove a setup's held regions and tabs in the interface, with validation, the default tabs shown as defaults and a warning when an operation's selection reaches one.
2. The interface specification (`docs/ui-design/ui-specification.md`) gains the workholding panel (fields, validation messages, the default tabs and how they are shown as defaults, the warning when an operation's selection reaches a held region or a tab) and the viewport display (colour and shape or outline so the two roles differ by more than colour, a legend entry, hover and selection behaviour), with a mock-up. Written by lane 21d: specification sections 7.5 (the display) and 11.7 (the panel), with mock-ups 16 and 17; what the current code does differently is D13 and OI-325 (open), which item 3 closes.
3. Tests and screenshots that name the new requirement: the panel edits and validates a tab and a held region and the edit is undoable; the viewport draws them; the UI smoke test opens the panel (UI-07, UI-08, NFR-02). The verification plan gains the row. Built by lane 21d on `claude/lane-21d-workholding-ui-code` after lane O (pull request 83) merged, because both change the viewport and the shell; it brings the code to sections 7.5 and 11.7 (OI-325, closed), except the layout parts and the Setup step panes that go with slice 22 (OI-334). The view-model additions are in `openchuck-app` (`workholding_view`); no engine change.
4. Slice 22 (interface redesign) includes the panel and the display in its component inventory.

**Review checklist**

- [x] A requirement in `docs/requirements.md` says that held regions and retaining tabs are shown in the viewport and edited in the interface (UI-03 amended and UI-25, decision 0104, agreed by the owner 17 Oct 2026)
- [x] The interface specification describes the workholding panel and the viewport display, with a mock-up (21d item 2: sections 7.5 and 11.7, mock-ups 16 and 17; the differences in the code are OI-325)
- [x] Tests that name UI-25 cover editing, undo, display and the selection warning (21d item 3: `crates/openchuck-app/tests/workholding_view.rs`, and the panel and viewport tests in `openchuck-ui`; the UI smoke test does not open the panel yet, OI-334)
- [x] A held region and each tab validate, persist, migrate and undo; impossible ones are refused with a message (21a tests in `openchuck-project`, `openchuck-app` and `openchuck-cli`; the form and generation refuse the checks that need the part and the tool)
- [x] Roughing and finishing on the oracle parts never cut into a held region or a tab, and the independent checker agrees (`req_set_09_req_op_21_operations_never_cut_a_held_region_or_a_tab`; tabs around the axis by the simulation, OI-197)
- [x] Links and retracts do not cross a held region (the same test: every fed move checked, and no rapid move removes material)
- [x] The simulated stock keeps them and the deviation report counts them as part (`req_set_09_req_op_21_the_deviation_report_counts_them_as_part`)
- [x] Results are identical at 1, 2 and N threads (`req_gc_07_req_nfr_06_workholding_gives_the_same_gcode_at_any_thread_count`, helical finishing on the held cone at 1, 2 and 4 threads; the composite's drops at 1, 2 and 7 are in lane V's tests)
- [x] A project without a held region or tab produces byte-identical G-code to before this slice (the reference and golden files are unchanged)

## Slice 22 – User interface conformance pass

**Status:** in progress. Reframed on 17 Oct 2026 by the owner ([decision 0105](decisions/0105-slice-22-conformance-pass.md)): it was a redesign waiting on a new design, and the interface specification (decision 0038) is now that design and the lanes built to it. Lane O (selection and viewport, #83) and lane 21d (pull request 119) are merged; the parts of the workholding panel lane 21d did not build are OI-334. Item 1, the conformance report and component inventory, is written by lane X1 ([decision 0114](decisions/0114-slice-22-conformance-report.md), accepted 18 Oct 2026): most of the gap is slice 8 items 8 to 12 (UI-10 to UI-16, NFR-15), which are unbuilt, and the owner chose that slice 22's lanes build them (OI-338). Items 2 and 3 are lanes X2 to X9 (Parallel work); the owner's other answers are in the record (OI-339 to OI-345). Lane X4 builds the inspectors ([decision 0123](decisions/0123-slice-22-inspectors.md), accepted): the shared field row, the header, error banner and sections, the setup and model inspectors, and the workholding section's OI-334 items a, c, d, e, i, j, l and the inspector side of k. Lane X3 (viewport drawing) is built on `claude/lane-x3-viewport-drawing` ([decision 0119](decisions/0119-slice-22-viewport-drawing.md), accepted 19 Oct 2026): C5 to C9 and OI-334 m, n and the drawing of k; what X2 and X4 wire is OI-355.

**Goal:** every screen, panel and dialog follows the interface specification (`docs/ui-design/ui-specification.md`) and its mock-ups, with the same behaviour, requirements and tests. Differences are fixed or recorded as open items; there is no redesign.

**Rework is expected.** Screens built earlier were each built to the specification as it stood when their lane ran, and the specification has changed since (selection, the workholding panel, the export dialogs). The owner expects some rework of earlier interface work; it is planned here and is not a defect of the slices that built it.

**Requirements:** none new. It closes UI-10 to UI-16 and NFR-15 with slice 8, whose items 8 to 12 its lanes build (OI-338, owner's choice, [decision 0114](decisions/0114-slice-22-conformance-report.md)), and re-verifies UI-01 to UI-09, UI-25, NFR-02 and NFR-03 (the tests that already name them still pass or are updated with the specification).

**Needs:** lanes O and 21d merged (they are). The later lanes' needs are in Parallel work.

**Design refinement:** first step, before any rework: a conformance report as a decision record. For every screen, panel, dialog and overlay in the specification, say whether the built interface matches it, and list each difference with a proposed fix (change the code, or change the specification with the owner's agreement). Differences that need a requirement change are raised, not built.

1. The conformance report and the component inventory: today's panels mapped to the specification, with what is removed, moved or new. It includes the setup's workholding panel and the viewport display of held regions and tabs (slice 21d; specification sections 7.5 and 11.7): the panel moves from the setup's whole inspector to its fifth section, the setup gains a "Workholding" tree row, and the Setup step's front and end panes draw the held regions and tabs (OI-325 items a, b and o, now OI-334). *Written by lane X1 ([decision 0114](decisions/0114-slice-22-conformance-report.md), accepted 18 Oct 2026): differences C1 to C24, the inventory, OI-334 mapped to lanes, and the edge widths checked against the mock-ups.*
2. Rework of `openchuck-ui` (and the drawing parts of `openchuck-viewport` the report touches) to the specification, behind the existing view model in `openchuck-app`; view-model additions are the only changes outside the UI crates. *Lanes X2 to X7b and X9 ([decision 0114](decisions/0114-slice-22-conformance-report.md) section E); everything serving a P1 requirement, and the tool library window, blocks slice 20 (OI-344).*
3. Update the UI smoke tests and the screenshots in the guide; keep the 1M-triangle frame benchmark and rerun it on the reference machines (NFR-03). *Lane X8. No user guide exists yet, so the screenshots are the reference screens of the verification plan (UI-16).*
4. No project format change and no engine change. If the specification needs one, it is a separate lane with its own schema claim.

**Review checklist**

- [ ] Every screen, panel and dialog in the specification exists, matches it or has a recorded difference, and nothing the requirements need is missing
- [ ] The UI imports no engine crate, and only `openchuck-ui` and `openchuck-viewport` use egui or wgpu
- [ ] The UI smoke test passes on Windows, macOS and Linux (NFR-02)
- [ ] The frame time on 1M triangles still meets NFR-03 on the reference hardware
- [ ] Undo, redo and the inline errors behave as before (UI-08, UI-07)
- [ ] Anything the specification asks for beyond the requirements is listed as an open item or a requirement proposal, not built

## Slice 23 – Default tool library

**Status:** built except what is still open (lane TL, 17 Oct 2026; [decision 0081](decisions/0081-slice-23-default-tool-library.md), accepted by the owner, relayed; OI-269's licence question stays open): the 23 standard tools and 54 Makera Z1 tools (the 1/8 in set plus the 4 mm and 6 mm shank extension) with the Z1 default tool set (NXX numbers 101 to 504, assigned by this project; the owner approved the tables and the numbers, relayed) are embedded and installed on first run with a ledger, and the explicit restore action is built. **Cutting data (OI-270, decided):** 35 of the 54 Z1 bits ship the Makera reference values (147 entries, source record in decision 0081 item 10); the standard tools and the other 19 Z1 bits ship none (no source states a stepover for them); the sources are combined and their conflicts are listed in the record for the owner. Two 4 mm engraving bits (307, 308) have a derived cutter diameter (OI-283, owner-decided). **Still open:** the UI button for the restore action (the update policy itself is decided, OI-271), thread mills as a P2 item (OI-281, left out), the tools left out of the extension (OI-282) and the licence of the Makera data (OI-269: the owner's route, no legal clearance claimed). The guide is not written. TL-12 is met for the standard tools and the Z1 set except for those items. Added 17 Oct 2026 on the owner's instruction ([decision 0060](decisions/0060-gc16-command-line-reading-and-tl12-default-tool-library.md)): slice 4 built the library mechanism and a test sample, but a new user starts with an empty library. This is lane TL.

**Goal:** a new user finds the tools a Makera Z1 normally holds and a set of common standard tools already in the library.

**Requirements:** TL-12 (P1); uses TL-01 to TL-07 and TL-11.

**Needs:** slice 4 (merged). Nothing else.

**Design refinement:** first step, before any data is copied. Record in a decision: the source and licence of the Makera reference tools (`MakeraInc/CarveraProfiles`, named by the owner; the files, the commit read, and whether its licence allows the data to be copied and shipped under the AGPL, NFR-10; if not, the Z1 tools are written from published specifications and the repository is cited as a reference only), the list of common standard tools (types and sizes, for the owner to approve), whether cutting data (TL-05) is shipped (only values with a source; never invented, NFR-12), how the defaults are delivered (embedded in the program, read offline, NFR-09), how they are installed into the user's library on first run, and the update policy (a tool the user changed is never overwritten, and a deleted tool is not restored).

1. The Makera Z1 reference tools as library tool files, with the Z1 default tool set (TL-11) and its tool numbers from the Makera library.
2. A set of common standard tools of each type in TL-02 and TL-03 (flat, ball, bull-nose, tapered ball, V-bit or chamfer, drill), as library tool files.
3. Delivery: embedded in the program and installed into the user's library folder on first run (`OpenChuckCAM/tools`, decision 0029 item 15); no network.
4. Update policy as recorded, with a test for each rule.
5. Tests, the verification plan row for TL-12, and the guide.

**Review checklist**

- [ ] The source and licence of the Makera data are recorded, and nothing was copied that the licence does not allow *(sources are recorded per value; the repositories have no licence file, the owner chose the route, and no legal clearance is claimed, so this stays unticked: OI-269)*
- [x] A first run installs the Z1 tools with their tool set and a standard tool of every type into an empty library; a second run installs nothing (`req_tl_12_*`)
- [x] A tool the user edited or deleted is not overwritten or restored by an update
- [x] Every default tool validates and its shape matches its closed-form silhouette (TL-02, TL-03)
- [x] The data is embedded and read with no network access (NFR-09)
- [x] Cutting data that ships has a recorded source *(decision 0081 item 10, OI-270, and each file's header)*

## Slice 20 – Hardening and first release

**Goal:** a release you can trust, with every P1 requirement traceable to a passing test or a recorded manual check.

**Requirements:** NFR-12, plus final checks on NFR-03, NFR-04, NFR-06, NFR-07, NFR-08, NFR-09, NFR-10

**Design refinement:** architecture review against what was built, with debt noted, and a check of the architecture's section 10 coverage table against the traceability report.

1. Safety notice (generated G-code must be verified before use) shown at export in the UI, printed by the CLI and available as an optional post header comment (NFR-12).
2. Performance pass against NFR-03 and NFR-04 on the reference machine.
3. Determinism run at 1, 2 and N threads on all sample projects. *Built (lane TE, [decision 0096](decisions/0096-slice-20-end-to-end-and-determinism.md), accepted):* `apps/openchuck-cli/tests/e2e.rs` regenerates every sample project in separate `openchuck-cli` processes at 1, 2 and N threads with both posts and requires byte-identical programs; the Tests job runs it on all three platforms. The STEP `drilling` job stays in the kernel job (OI-312).
4. Malformed-input sweep of every importer, the project loader and every other user-supplied file format (tool library, custom tool profile, machine profile, post definition), with no unresolved crash and a running total of fuzz CPU hours per parser against an advisory target of 24 hours (NFR-07; the 24 hours are advisory, not a release condition, [decision 0107](decisions/0107-malformed-input-sweep-and-fuzz-hours.md) item 7). *In progress (lane FZ):* every user file is listed with its target in [fuzzing.md](fuzzing.md); the whole-folder project target and post rendering are fuzzed, all ten targets run nightly with the corpus kept, and the CPU hours are counted per parser there ([decision 0107](decisions/0107-malformed-input-sweep-and-fuzz-hours.md), accepted 18 Oct 2026). On 18 Oct 2026 no parser has the advisory 24 hours (at most 6) and the 13 Oct crash is fixed; OI-326 to OI-329 are open.
5. Licence review, third-party notices, SPDX headers and an offline run check (NFR-09, NFR-10).
6. Sample projects, including the Z1 reference part, end to end. *Built (lane TE, [decision 0096](decisions/0096-slice-20-end-to-end-and-determinism.md), accepted):* the reference job set on the Z1 profile is built with the command-line tool from STL import (the operation through the API, OI-310), validated, regenerated through gates 1 and 2, compared with the golden programs and simulated. "The Z1 reference part": the owner chose the reference job set plus the owner's blisk (OI-311), which ran by hand only and did not gate CI because its programs differed across platforms (OI-319, owner's decision); lane BX found the cause in choices the platform's rounding could flip and fixed it ([decision 0113](decisions/0113-toolpath-choices-rounding-cannot-flip.md), accepted), so the three tests return to CI; strategies without a reference job are OI-312.
7. CLI end-to-end scenarios in CI, including undo and redo and cancellation mid-job. *Built (lane TE, [decision 0096](decisions/0096-slice-20-end-to-end-and-determinism.md), accepted):* an interrupted `regenerate` writes no G-code and leaves the project unchanged; a job cancelled mid-run leaves the next run identical; undo and redo on a sample project restore the G-code and the files (through the application API, since the CLI has no undo, OI-313). `.github/workflows/e2e.yml` runs the suite with the network unavailable (NFR-09); the owner made its check required (OI-315; the setting is added once the check has run).
8. Release tooling assessed under NFR-14 (`cargo-release` for the library family, `cargo-dist` or a plain build matrix for binaries, `release-plz` later) and the packaging route decided; separate tag schemes for library and application releases. *Route decided:* [decision 0031](decisions/0031-slice-20-packaging-and-trial-documents.md) (plain build matrix, `lib-v` and `app-v` tags). *Workflow built (lane TR, [decision 0091](decisions/0091-release-workflow-and-gate.md), accepted 17 Oct 2026, relayed; not yet run, OI-304):* `release.yml` runs on those tags and by hand as a dry run, over the archive build it shares with the nightly workflow (`build-archives.yml`). The macOS bundle and the Linux glibc floor are still to build (OI-300, OI-301), and crates.io publishing is by hand (OI-302, OI-303).
9. Release gate in `xtask`: dependency-rule check, `cargo deny`, golden G-code comparison and LinuxCNC replay must pass before a tag is cut. *Built (lane TR, decision 0091):* `cargo xtask release-gate` runs all four, has no option to skip one, and is the release workflow's first job; the build and publish jobs need it.
10. Z1 air-cut procedure documented (your hands-on step) and packaging for all three platforms, including a Windows installer and macOS app bundle if the egui app needs them. *Prepared:* the [air-cut procedure](z1-air-cut-procedure.md) is written and reviewed by the operator (14 Oct 2026, OI-133); running it is still the hands-on step; the packaging route is accepted in decision 0031 and the bundle command and release workflow are still to build (signing and extra platforms: OI-135).
11. Gates 4 and 5 on the Z1 for the reference job set (your hands-on step, attended; a manual release-checklist item owned by the owner, not enforced by `xtask release-gate`, [decision 0106](decisions/0106-air-cut-is-a-manual-release-step.md)): an air cut at reduced feed (including a timed simultaneous XYZA move compared with the IR's predicted time), then a first cut in wax or foam. Cut a 3-axis test part first to measure the machine's own repeatability, then set the first-cut limit from it (start at ±0.1 mm). Recheck the G93 result on the installed firmware (SP-12 found it ignored on 1.1.2) and carry the confirmed Z1 rotary conventions into the post definition (also OI-267).
12. Safe-practice checklist for first machine trials, written and reviewed by whoever operates the Z1. *Written and reviewed by the operator (14 Oct 2026):* [z1-safe-practice-checklist.md](z1-safe-practice-checklist.md) (OI-133).
13. Verification report for the release: results against the exit criteria, waivers, open defects, the commit covered, a written result for every spike, and the plan reviewed against what happened.
14. Traceability report generated from the requirement ID tags.

**Review checklist**

- [ ] Every P1 ID has a passing test or a recorded manual check in the traceability report
- [ ] A fresh install on each platform produces verified G-code from a sample STEP part
- [ ] The safety notice is present in the UI, the CLI output and the post option
- [x] The full run works with no network access (the command-line end-to-end suite, from import to checked G-code, in a network namespace on Linux: `.github/workflows/e2e.yml`, [decision 0096](decisions/0096-slice-20-end-to-end-and-determinism.md); Windows and macOS rely on `cargo xtask deps`, OI-315)
- [x] A deliberately failing check blocks the release tag (17 Oct 2026, lane TR: `req_gc_09_req_gc_10_req_nfr_08_a_failing_check_blocks_the_release_gate` in `xtask`; the release workflow's build and publish jobs need the gate's job, [decision 0091](decisions/0091-release-workflow-and-gate.md))
- [ ] The library crates pass a publish dry run, and the `openchuck-*` names are reserved
- [ ] Licences are compatible and the known-issues list is written
- [ ] Gates 1 to 5 have passed for the reference job set with results stored, and the G93 result is recorded
- [ ] No severity 1 or 2 defect is open, and the verification report names the commit it covers
- [x] The safe-practice checklist has been reviewed by the Z1 operator (14 Oct 2026, [z1-safe-practice-checklist.md](z1-safe-practice-checklist.md))

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
| 8 | UI-01, UI-02, UI-03, UI-06, UI-07, UI-10, UI-11, UI-12, UI-13, UI-14, UI-15, UI-16, NFR-02, NFR-03, NFR-15 | SP-2 |
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
| 19 | GC-05, GC-06, GC-12, GC-15, GC-16 | – |
| 20 | NFR-12 | – |
| 21 | SET-09, OP-21, UI-25 | – |
| 22 | UI-10, UI-11, UI-12, UI-13, UI-14, UI-15, UI-16, NFR-15 (with slice 8), NFR-17 | – |
| 23 | TL-12 | – |

**Requirements that span slices.** A requirement closes when the last slice that touches it closes: TL-03 (slices 4, 8), UI-04 and PRJ-06 (10, 11), OP-02 (10, 12, 14), OP-11 (5, 12, 14), OP-13 and OP-15 (5, 13), OP-14 (5, 14), GC-14 (6, 19), NFR-02 (0, 8), and UI-10 to UI-16 and NFR-15 (8, 22: slice 22's lanes build slice 8 items 8 to 12, OI-338). UI-14 and NFR-15 are built in slice 8 and close at slice 20, where they are checked again. The cross-cutting NFR-03, NFR-04, NFR-06, NFR-07, NFR-08, NFR-09, NFR-10 and NFR-14 are tracked from slice 0 or their first slice but close at slice 20.
