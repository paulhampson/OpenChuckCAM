# OpenChuckCAM: Software Verification Plan

Oct 10, 2026 · @Paul H

This plan sets the verification policies, strategy and release gates for the Rust 4-axis CAM tool, so every requirement is checked by a named method at a named level before first release.

Source documents: [OpenChuckCAM – Initial Requirements](https://claude.ai/code/artifact/78c00b68-4f19-4e2b-b703-c734f6422702) and [OpenChuckCAM: High-Level Architecture](https://claude.ai/code/artifact/7b38ebec-d6ab-4edd-82ef-4e7ada1177a7) (revision 3, 11 Oct 2026). This plan uses the requirement IDs and priorities (P1 core, P2 secondary, P3 suggested) and the crate names from those documents without renaming them. It implements the verification architecture in section 9 of the architecture document and adds the policies, gates, limits and records around it.

## 1. Purpose and scope

This plan defines how the Rust CAM tool is shown to meet its requirements: STEP/STL in, true 4-axis (XYZA) toolpaths out, as G-code only. It follows the structure of IEEE 1012 (verification and validation) and ISO/IEC/IEEE 29119-3 (test documentation), scaled to a small open-source team. Verification effort follows priority: P1 requirements gate the first release, P2 requirements are verified when they are built, and P3 requirements are approved and are verified when they are built.

**In scope**

- Model import and geometry (IMP), the cross-platform UI and 3D viewport (UI), and machine, stock and setup definition (SET).
- Tool management (TL) and the machining strategies (OP): rotary roughing, helical, axial and indexed finishing, simultaneous 4-axis finishing, swarf, curve following, rest machining and radial drilling, with gouge protection, feed calculation and A-axis management.
- Toolpath visualisation and material removal simulation (SIM).
- G-code generation through text-template post-processors (GC), including the Makera Z1 post-processor (GC-09) and the LinuxCNC verification post-processor (GC-10).
- Version-control-friendly project files (PRJ) and the non-functional requirements (NFR): performance, precision, repeatability, error handling, offline use and licensing.
- Secondary goals, verified when built: collision detection (COL), in-app version control on git2 (VC) and the pathing library for other projects such as FreeCAD (INT).

**Out of scope**

- Direct interfacing with, or streaming to, a CNC machine, so there is nothing to verify there.
- CAD modelling or editing of the imported geometry; 5-axis machining; lathe and turning operations.
- Whether a finished part is fit for its end use, which depends on the operator, machine and material. The software only states that the user must verify G-code before running it (NFR-12).

**Terms.** *Verification* asks whether we built the software right (against requirements and design). *Validation* asks whether we built the right software (against user need); this plan covers it only through the acceptance trial in section 6.

## 2. Verification policies

These rules apply to every change. A change that breaks one is not merged.

1. **Every requirement is verifiable.** Each requirement already has a unique ID in the requirements document; this plan gives each one a verification method and a pass criterion before implementation starts. A requirement that cannot be checked is rewritten or rejected.
2. **Traceability both ways.** Requirement IDs go into test names or attributes, and an xtask generates a traceability report from the test run. CI fails when a P1 requirement has no test, so gaps are not found by hand.
3. **Automate by default.** Test, analysis and inspection steps that can run in CI do run in CI. Manual steps are written as scripts or checklists and their results are recorded.
4. **Tests come with the change.** A feature or fix merges with its tests, and a bug fix merges with a regression test that fails without the fix.
5. **Machine output is checked by something other than the generator.** G-code is never judged only by the code that produced it. An independent interpreter (the LinuxCNC simulator) and a test-only gouge and coverage checker, built on different code from the strategies, decide.
6. **Nothing reaches a real machine unverified.** No G-code is run on the Makera Z1 until it has passed the simulator gate in section 6. The one exception is the SP-12 conventions program: it is hand-written, checked by hand and in the LinuxCNC simulator first, and then run only as an attended air cut with no tool, no material and reduced feed (the gate 4 rules).
7. **Determinism.** Given the same model, settings and tool, toolpaths and G-code are byte-identical across runs and thread counts on one platform and build, so results can be diffed and stored in Git. Across platforms the same golden files must match; a difference (f64 maths and the C++ kernel can vary by platform) is investigated and, only if unavoidable, recorded as a per-platform golden file with the reason.
8. **Failure is explicit.** Invalid or unsupported input produces a clear error, never a silently wrong toolpath.
9. **Independence.** The author of a change does not alone approve it. With a single maintainer, independence comes from CI, the independent simulator, and a delay-and-reread step before release sign-off.
10. **Risk drives effort.** Areas where an error can damage a machine or part (kinematics, feed rates, post-processors) get the deepest verification; cosmetic UI gets the lightest.

## 3. Verification strategy

Verification follows a V-model: each level of specification on the left has a matching level of checking on the right, and risk decides how deep each level goes.

&#91;embedded content: V-model · 4 design levels paired with 4 test levels\]

The dashed lines pair each design level with its test level. Only the acceptance trial (highlighted) needs the real machine; every level below it runs in CI or on a workstation.

- **Unit level:** each crate on its own (for example cargo test -p openchuck-path), with analytic oracles, property tests and fuzzing. The library family (openchuck-types, -path, -post and -gcode) builds and tests without the C++ toolchain, GPU libraries or application crates.
- **Integration level:** the boundaries between crates: importer into geometry, strategies against the StockModel and CollisionOracle ports, the neutral toolpath into post definitions, G-code read back through openchuck-gcode, project files into Git through openchuck-vcs, and the STEP kernel boundary.
- **System level:** the whole application driven through openchuck-cli, which is a front end over the same application API as the UI. It also runs G-code gates 1 to 3 on reference jobs, with cross-platform UI smoke tests.
- **Acceptance level:** the gated trial on the Makera Z1 (section 6), which is the only validation against real machining.

**Methods.** Each requirement uses one primary method: test (T), analysis (A), inspection (I) or demonstration (D). Analysis covers licence and kinematic reasoning, inspection covers design and code review, and demonstration covers UI behaviour and the machine trial.

## 4. Requirements coverage

Every requirement ID in the requirements document is covered below, grouped where several IDs share a method. Methods: **T** test, **A** analysis, **I** inspection, **D** demonstration. Priority is P1 (gates first release), P2 (verified when built) or P3 (approved; verified when built). When the requirements document changes, this table changes with it, and CI reports any ID with no test.

| ID | Pri | Method | Level | How it is verified |
| --- | --- | --- | --- | --- |
| IMP-01 | P1 | T | Unit | ASCII and binary STL corpus loads; triangle counts and bounds match references |
| IMP-02 | P1 | T | Unit, integration | STEP corpus keeps faces and edges; surface evaluation matches analytic shapes, not the mesh; the in-house evaluators (analytic, rational B-spline, trims) match the kernel within 1e-6 mm in point, closest point and ray hit on the corpus, including converted rational forms and trimmed faces (SP-9, decision 0017); the same file always gives identical face and edge IDs |
| IMP-03 | P1 | T | Unit | File units read, override applied, internal values in mm for mm, inch and metre files |
| IMP-04 | P1 | T | Unit | Seeded holes, non-manifold edges, flipped normals and degenerate triangles are all reported; clean meshes give no report |
| IMP-05 | P1 | T | Unit | Translate, rotate, scale, mirror and orient-to-axis match matrix reference results; mirror keeps normals outward |
| IMP-06 | P1 | T, A | Unit | Tessellation deviation from the exact surface stays within the user tolerance on analytic shapes |
| IMP-07, IMP-08 | P2 | T | Integration | Re-import keeps matchable operations and selections; DXF and SVG curves load with correct geometry |
| UI-01, NFR-02 | P1 | T, D | System | CI builds, launches and runs a smoke script on Windows, macOS and Linux from one codebase, software-rendered where a runner has no GPU and natively rendered where it has one; native graphics on each platform is also confirmed on the physical reference machines |
| UI-02, UI-03 | P1 | D, T | System | Manual viewport script (camera modes, display modes, axis, WCS, stock and part together) plus screenshot comparison on reference models |
| UI-04, UI-05 | P1 | T | Integration | Face, edge, region, angle and flood-fill selection return expected IDs; named sets update when the selection is edited |
| UI-06, UI-07 | P1 | T, D | Integration | Project tree operations (reorder, enable, disable, duplicate) and parameter validation tested on view-models; units and defaults checked in the demo script |
| UI-08 | P1 | T | Unit | Property test: any random edit sequence followed by full undo returns the original project byte for byte; redo restores the edited one |
| UI-09, NFR-04 | P1 | T | System | UI thread stays responsive during generation and simulation (measured frame gaps); progress is reported; cancel stops work; multi-core speed-up measured |
| SET-01, SET-03, SET-05, SET-06 | P1 | T | Unit | Machine profile, stock, part placement with G54-style offset and clearance geometry validate and persist; invalid values rejected |
| SET-02, SET-04, SET-07 | P2 | T | Integration | Y-axis rotary, custom stock and multiple setups produce correct coordinate systems and starting stock |
| SET-08 | P2 | T | Unit | A rotary acceleration limit validates and persists; an unset value means no limit; time estimates use it once SIM-08 exists |
| TL-01, TL-04, TL-05, TL-06 | P1 | T | Unit | Tool library round-trips in its human-readable format; parameters, cutting data and controller mappings validate |
| TL-02 | P1 | T | Unit | Flat, ball, bull-nose and tapered ball tools give the expected profile geometry |
| TL-03 | P1 | T | Unit, integration | V-bit, chamfer, drill and custom-profile tools give the expected revolved profile; a custom profile imported from a text file or drawn in the UI is validated and invalid profiles are rejected; the same profile drives tool-centre solving, the independent gouge checker and the simulation sweep and agrees with an equivalent standard tool (SP-13) |
| TL-07 | P1 | T | Integration | A project opens with the tool library absent and still generates identical output |
| TL-11 | P1 | T | Unit, integration | A tool added to a project for a machine takes the number the machine's tool set gives it, or the lowest number no project tool uses and the set does not keep for another tool; tool set files round-trip and invalid sets (a number or tool listed twice, number 0) are rejected |
| TL-08, TL-09 | P2 | T | Unit | Holder and collet assembly, and material library pre-fill tested when built |
| TL-10 | P3 | T | Unit | LinuxCNC tool table import on sample tables, when built |
| OP-01, OP-02 | P1 | T | Integration | An operation regenerates identically; toolpath stays within selected faces, containment boundary and avoid or check surfaces |
| OP-03 | P1 | T, A | System | Indexed and continuous roughing remove stock down to stock-to-leave and never leave more than that; remaining stock is respected |
| OP-04 | P1 | T | Integration | Helical, axial and indexed finishing on analytic cylinders and cones match exact path and stepover |
| OP-05, OP-06 | P1 | T, A | Unit, system | Kinematic round trip: forward kinematics of each XYZA point gives the intended tool tip; tool axis stays within lead and tilt limits |
| OP-07, OP-08, OP-09, OP-10 | P1 | T | Integration | Swarf contact deviation on ruled surfaces; curve following on projected chains; rest machining clears only leftover material; radial holes at correct indexed A and depth |
| OP-11 | P1 | T | Unit | Each common parameter changes the toolpath as specified; stock to leave applies per operation and per selection |
| OP-12 | P1 | T, A | System | Property tests and simulation show no overcut beyond tolerance for the tool, and for the holder where defined |
| OP-13 | P1 | T, A | Integration | Feed at the cutter contact point matches the programmed feed within a set band on simultaneous moves; compensated feed is the default; G93 appears only when the post-processor enables it; a round trip through the G-code reader confirms the feed values |
| OP-14 | P1 | T | Integration | Linking moves use clearance geometry and never cross stock or part |
| OP-15 | P1 | T | Unit | A-axis range limits, reversal count, wrap-around at the 0/360 seam and unwinding checked on generated sequences |
| OP-16 | P1 | I, T | Design, unit | Design review of the strategy interface; a stub strategy plugs in with no change to other crates, and its parameter schema renders in the panel with no UI change |
| OP-17 | P2 | T, A | System | Adaptive roughing keeps engagement within limits, measured in simulation |
| OP-18 | P3 | T | Integration | Feature recognition finds known holes, grooves and cylinders in corpus models, when built |
| OP-19 | P2 | T, A | System | Geodesic-parallel and spiral patterns keep scallop height within the set limit on surfaces with known geodesics (cylinder, cone), measured in simulation, when built |
| SIM-01, SIM-02, SIM-04 | P1 | D, T | System | Path display, rapid versus cutting distinction, tool pose with correct A rotation and playback controls checked in the demo script and by pose tests |
| SIM-03, SIM-06, SIM-07 | P1 | T, A | System | Stock update, including A rotation, matches an independent reference; carry-over between operations; resolution setting changes accuracy as documented |
| SIM-05 | P1 | T | System | Remaining material and overcut colouring and worst values match reference results on analytic cases |
| SIM-08 | P2 | T | Unit | Estimated time matches hand calculation from feeds, rapids and tool changes |
| SIM-09, SIM-10 | P3 | T | System | Exported G-code runs in the in-app simulator; engagement-based feed adjustment, when built |
| GC-01, GC-02, GC-03, GC-04 | P1 | T, I | Integration | Neutral intermediate form has no controller types; several post-processors selectable per machine profile; template edits take effect without recompiling; every post option changes the output as specified |
| GC-05, GC-06, GC-11, GC-12 | P1 | T | Integration | One-file and per-setup, operation or tool output; comment headers; setup sheet in HTML or PDF with correct tools, stock and operations; the command-line run regenerates the same G-code as the UI |
| GC-07, NFR-06 | P1 | T | System | Same project and post-processor give byte-identical G-code and toolpaths on repeat runs and with 1, 2 and many threads on each platform; the golden files are compared on all three platforms |
| GC-09 | P1 | T, D | Integration, acceptance | Golden-file tests, simulator pass, then the gated trial on the Makera Z1 (section 6) |
| GC-10 | P1 | T | Integration | Verification post-processor output runs to completion in the LinuxCNC simulator; end position and A angle match the plan |
| GC-14 | P1 | T | Integration | Confirming keeps the project's numbers; automatic renumbering gives T1, T2 and so on in order of first use by the operations; manual renumbering refuses a number used twice; the chosen numbers are saved and a second post gives byte-identical G-code |
| GC-08, GC-13 | P2 | T | Integration | Limit, feed and number checks fire on seeded faults; scripting hook gives output templates cannot |
| PRJ-01, PRJ-03, PRJ-05 | P1 | T | Unit, integration | Saving twice is byte-identical; a small edit gives a small diff; no generated data in the file; two branches editing different items merge cleanly |
| PRJ-02, PRJ-06 | P1 | T | Integration | Model files stay separate with relative paths; a changed model is detected by hash; selections survive save and load |
| PRJ-04 | P1 | T | Integration | Files from every earlier schema version load and migrate to the current one |
| PRJ-07, PRJ-08 | P2 | T | Integration | Selections re-match after re-export using descriptors; autosave recovers a project after a forced kill |
| COL-01 to COL-07 | P2 | T | System | Known collision and clearance cases for tool cutting part, shank, collet, holder, machine head and fixtures, against in-process stock, with the safety margin and a correct, navigable collision list |
| COL-08 | P3 | T | System | Generated linking moves pass COL-01 to COL-05 checks, when built |
| VC-01 to VC-04 | P2 | T | Integration | Named revisions, history, restore, structured diff and branch and merge on temporary repositories on all three platforms |
| VC-05 | P2 | T, I | Integration | A project folder is a standard Git repository that the git command reads; the revision code sits behind a small interface |
| VC-06 | P2 | T | Integration | Revision ID appears in the G-code header and matches the project state |
| INT-01, INT-02, INT-03, INT-04 | P2 | I, T | Design, integration | An xtask check over cargo metadata fails if the library family gains an application, UI, GPU, Git or C++ kernel dependency, and each library builds and tests alone; an example host passes plain data and receives neutral toolpaths; post-processing runs alone; progress and cancel work |
| INT-05, INT-07, INT-08 | P2 | T, I | Integration | Python binding tests in CI; semver check and changelog on every API change; the example host builds and runs in CI |
| INT-06 | P3 | T | Integration | C interface tests, when built |
| NFR-10 | P1 | A | Release | Licence scan of the full tree, including C++ code, is GPL-compatible |
| INT-09 | P2 | A | Release | The pathing library carries the AGPL notice, and the licence scan shows the library family and its dependencies are compatible with it |
| NFR-01 | P1 | I, T | Design, system | Engine crates have no UI dependency; the whole engine test suite runs headless |
| NFR-03 | P1 | T | System | Frame rate of 30 fps or better for orbit, pan and zoom on a 1 million triangle model, measured on defined reference hardware |
| NFR-05 | P1 | T, I | Unit | Geometry and toolpaths use double precision (type audit); the default tolerance is 0.01 mm and can be changed |
| NFR-07 | P1 | T | Unit, system | Fuzzing and a corpus of damaged files give a clear error and never a crash or hang, for every file the user can supply: STL, STEP, project files, tool library and custom tool-profile files, machine profiles and post definitions (and DXF and SVG when built) |
| NFR-08 | P1 | I | Release | Golden-file G-code tests and known-geometry regression tests exist for every strategy and post-processor |
| NFR-09 | P1 | T, A | System | The application runs with the network blocked; code and dependency scan finds no accounts, licence checks or telemetry |
| NFR-11 | P2 | I | Release | The user guide and the post-processor writing guide exist and their examples run |
| NFR-12 | P1 | D, I | System | The notice is shown at export in the UI, printed by the CLI and available as an optional post header comment; export tests and a CLI scenario check each |
| NFR-13 | P1 | T | System | Wrapped C++ libraries build and pass tests on Windows, macOS and Linux |
| NFR-14 | P1 | A, I | Design | Each dependency has a recorded review against the health criteria, with a written justification for newer crates and those with high AI contribution |

### 4.1 Acceptance limits

These are starting values, chosen before there is anything to measure. They are revised after the first measurements and each change is recorded here. Where a limit says "tolerance", it means the toolpath tolerance, 0.01 mm by default (NFR-05).

| Check | Requirement | Limit |
| --- | --- | --- |
| Toolpath deviation | NFR-05, OP-01, OP-02 | 0.01 mm, split as an error budget: about 0.005 mm tessellation (IMP-06) and 0.005 mm path fitting. Path fitting by bisection uses 0.4 times the tolerance as its chord limit (decision 0018 item 12; the measured fit error is 1.01 times the limit), measured along the surface normal |
| Kinematic round trip | OP-05, OP-06 | 1e-6 mm and 1e-6 degrees (double-precision arithmetic only) |
| G-code replay (gate 2) | GC-09, GC-10 | G-code rounding only, because replay compares the G-code with the same toolpath in the IR: 0.0005 mm and 0.001 degrees at three decimals, scaled to the post's decimal setting. These are per axis (half a unit of the last decimal, twice that for A); the tool tip combines three axes, so the check on the tip allows half a unit times root 3 (0.00087 mm at three decimals), confirmed by the owner 13 Oct 2026 (OI-91) |
| Gouge | OP-12 | Zero overcut beyond the tolerance |
| Simulation against reference | SIM-03, SIM-05 | Within one simulation cell at the chosen resolution, and no worse than 0.02 mm at the finest setting the user may choose, 0.025 mm (the default is 0.1 mm; decisions 0022 and 0025; measured 0.0187 mm) |
| Feed on simultaneous moves | OP-13 | Effective feed at the cutter contact within ±2% of programmed |
| Determinism | GC-07, NFR-06 | Zero difference, byte for byte, at 1, 2 and all threads |
| Fuzzing | NFR-07 | 1 minute per parser on pull requests; 1 hour nightly; at least 24 cumulative CPU hours per parser with no unresolved crash for release |
| Performance regression | NFR-03, NFR-04 | Flag at 10% on the median of several runs, on a dedicated runner |
| First-cut part (gate 5) | GC-09 | Start at ±0.1 mm. First cut a 3-axis test part to measure the machine's own repeatability, then set the limit from that result; software tolerance cannot beat what the machine holds |

### 4.2 Reference hardware for NFR-03

The 30 fps requirement is checked on a modest baseline machine, so that faster machines pass with margin.

- **Baseline:** 4 CPU cores, 16 GB RAM, integrated graphics, 1080p display. One physical machine per platform: Apple M1-class on macOS, Intel Xe or recent AMD integrated graphics on Windows and Linux.
- **Test:** a scripted 60-second camera path (orbit, pan, zoom) over a 1 million triangle model in shaded-with-edges mode, where the edges are the model's feature edges: those whose two triangles turn by more than 30 degrees, plus open and non-manifold edges, as the viewport draws them (OI-13, [decision 0026](decisions/0026-slice-8-viewport-and-shell.md) item 4). It passes if the 95th percentile frame time is 33 ms or less.
- **Where it runs:** on the physical machines before each release, not only on hosted CI, whose runners typically have no real GPU. CI runs a software-rendered smoke test (UI-01, NFR-02) and uses native rendering wherever a runner offers it.
- **Core counts:** generation and simulation also run at 1 core and at all cores. This checks the speed-up (NFR-04) and the identical output (NFR-06).

## 5. Test approach by component

The architecture splits the system into library crates behind two equal front ends, the CLI and the UI, over one application API. That sets the test approach: each crate is verified alone, boundaries are verified through the ports and plain-data types that cross them, and the CLI is the end-to-end harness. Everything except pixel output runs without a window.

| Crate | Verified by | Requirements |
| --- | --- | --- |
| openchuck-types | Serialisation round trips of the IR, machine profile, tool and stock types; unit newtype tests; double-precision audit | GC-01, INT-02, NFR-05 |
| openchuck-path | Analytic oracles, property tests, the independent gouge and coverage checker, A-axis limit tests, strategy parameter schema tests. Runs against the analytic StockModel and the accept-all CollisionOracle, so no simulator is needed | OP-01 to OP-17, INT-01 to INT-04 |
| openchuck-post | Golden files for the Z1 and LinuxCNC post definitions, feed-mode tests, determinism, post definitions edited without recompiling | GC-02 to GC-10, GC-13, INT-03 |
| openchuck-gcode | A-word and dialect parsing (in-house reader chosen by SP-5, decision 0005), seeded-fault tests, replay against the IR, fuzzing of the reader | GC-08, SIM-09 |
| openchuck-geom | Meshes with known defects, transform tests, selection and BVH ray-cast picking, all without a window | IMP-04, IMP-05, UI-04 |
| openchuck-import | Corpus tests and fuzzing for STL, STEP, DXF and SVG; unit conversion; deterministic face and edge IDs; tessellation tolerance; C++ kernel boundary tests | IMP-01 to IMP-03, IMP-06, IMP-08, NFR-13 |
| openchuck-sim | Deviation on analytic parts, playback jumps from checkpoints, timing against hand calculation, collision scenarios, conformance to the StockModel and CollisionOracle ports | SIM-01 to SIM-10, COL-01 to COL-08 |
| openchuck-project | Save twice gives identical bytes, load then save is a no-op, schema migration fixtures, order-key merge test with a real Git merge, model hash change detection, library snapshots | PRJ-01 to PRJ-08, TL-01 to TL-11, UI-05, VC-03 |
| openchuck-vcs | Temporary-repository tests for commit, history, restore, branch and merge, built without network features | VC-01 to VC-06 |
| openchuck-app | Command and query tests, undo and redo property tests, job manager cancellation and progress, pipeline cache invalidation | UI-06 to UI-09, GC-05, GC-12, NFR-04 |
| openchuck-cli | End-to-end scenarios with assert\_cmd: project in, G-code and report out, including the NFR-12 notice | GC-11, NFR-08 |
| openchuck-viewport, openchuck-ui | View-model tests, the frame-time benchmark, screenshot comparison and a short manual script | UI-01 to UI-03, SIM-01, SIM-02, SIM-04, NFR-03 |

The architecture also sets rules that need their own tests:

- **Pipeline caching:** editing operation N invalidates the cached results of N and every later operation, and none before it. A test edits each position in a chain and checks which results are rebuilt.
- **Determinism rules:** fixed chunking, ordered collection of results, ordered containers on any path that affects output, and fixed number formatting. Each is covered by the 1, 2 and N thread comparison.
- **No panics at boundaries:** lints deny unwrap and unchecked indexing in library code; panics are caught at FFI and binding boundaries; C++ kernel exceptions become errors. Fuzzing and boundary tests exercise this.
- **Selection references:** B-rep selections are stored against the model's content hash and survive a change of tessellation tolerance; mesh selections apply to mesh models only.
- **STL tolerance statement:** gouge tolerance on STL parts is measured against the supplied mesh, and the UI says so. A test checks the statement appears.

**Import (IMP).** Parsers are fed a corpus of valid files, deliberately broken files and files from several CAD exporters. Property tests check round trips and invariants (for example, bounding box unchanged by a load and save). Fuzzing with cargo-fuzz runs on the STL and STEP entry points, since malformed input is the main source of crashes. Where a C++ library is wrapped for STEP, the FFI boundary gets its own tests for error paths, memory ownership and large files.

**Geometry and selection (IMP-04, IMP-05, UI-04, UI-05, OP-02).** Unit tests on small analytic shapes (cylinder, cone, sphere, plane) where the exact answer is known: surface normals, curve lengths, offsets. Selection tests use fixed models and assert the set of IDs returned.

**4-axis toolpaths (OP).** This is the highest-risk area. Three checks apply to every strategy:

- *Kinematic round trip:* map each XYZA point back through the machine model to the tool tip in model space and compare it with the intended point.
- *Geometric tolerance:* distance from tool to the target surface stays inside the stated tolerance, with no gouging.
- *Motion limits:* A-axis wrap-around, axis travel limits and rotary-axis direction changes behave as specified, including at the 0/360 degree seam and at the pole of cylindrical parts.

**Material removal simulation (SIM-03, SIM-05, SIM-06).** Verified against an independent reference: analytic cases (a straight pass in a bar gives a known groove volume), then comparison with a second implementation or a high-resolution brute-force run on a few small jobs.

**G-code and post-processors (GC).** Golden-file tests compare generated programs with reviewed expected output, so any change in output is a visible diff. A parser reads the G-code back and replays it to check position, feed and modal state. Section 6 covers the simulator and machine gates.

**Project files and version control (PRJ).** Round-trip and stability tests (same input gives identical bytes), forward-compatibility tests with files from earlier releases, and git2 operations run against temporary repositories.

**UI (UI, NFR-01).** Logic lives in testable view-models; the thin rendering layer is covered by smoke tests on each platform, screenshot comparison for the reference models, and a short manual demonstration script before each release.

**Non-functional.** Specific checks follow the NFR requirements:

- *Performance (NFR-03):* a benchmark orbits, pans and zooms a 1 million triangle model and fails below 30 fps on the defined reference hardware. Criterion benchmarks on large models also give a baseline for toolpath generation, and a regression beyond a set margin is flagged.
- *Precision (NFR-05):* the default toolpath tolerance of 0.01 mm is the pass limit for geometric deviation tests unless an operation sets its own.
- *Repeatability (NFR-06, GC-07):* every golden-file job runs with 1, 2 and the maximum thread count, and the outputs are compared byte for byte.
- *Offline use (NFR-09):* the full system suite runs with the network blocked.
- *Memory:* use on the largest corpus model is recorded.

## 6. G-code and post-processor verification

Generated G-code passes five gates in order. A program cannot move to the next gate until it passes the one before, and a change to a post-processor or kinematics model sends affected programs back to gate 1.

| Gate | Check | Where | Pass criterion |
| --- | --- | --- | --- |
| 1. Static | Syntax, modal state, units, axis limits, feed present on every cutting move, safe retract and tool-change sequence | CI, openchuck-gcode checks (test code until GC-08 is built) | No violations; output matches the reviewed golden file |
| 2. Replay | Parse the program with openchuck-gcode, replay it through the machine kinematics and compare positions, A travel, feeds and limits with the neutral toolpath | CI | Maximum deviation from the IR within G-code rounding (limit in 4.1); the toolpath tolerance itself is checked against the model, not here |
| 3. Simulator | Run the verification post-processor output in the LinuxCNC simulator with the 4-axis (XYZA) configuration, using the xtask replay runner | CI or workstation (Linux) | Runs to completion, no errors or limit faults, end position and A-axis angle match the plan |
| 4. Air cut | Run the Makera Z1 program with no tool and no material, attended, at reduced feed, including a simultaneous XYZA move whose measured duration is compared with the IR's predicted time | Makera Z1 | Motion matches the simulator; controller accepts every command; no alarms; measured move time matches the IR duration, scaled for the reduced feed, within a band set at the first run (this shows how the controller applies feed to combined linear and rotary moves) |
| 5. First cut | Cut soft, low-cost material (wax or foam) attended, then compare with the model | Makera Z1 | Part matches the model within the limit in 4.1; findings recorded |

**Rules for gates 4 and 5.** The operator keeps the emergency stop in reach and runs attended. Work offsets and stock position are checked by hand. Any unexpected motion stops the trial and is logged as a defect with the G-code attached.

**Feed mode (OP-13).** Compensated feed in units per minute is the default for simultaneous moves. Inverse-time feed (G93) is an optional post-processor capability, not supported by the Makera Z1 controller: the SP-12 air cut (14 Oct 2026, firmware 1.1.2) showed it ignores `G93`, so the Z1 post never writes it ([decision 0007](decisions/0007-sp-12-z1-conventions.md)). Gate 4 keeps a short G93 test so that a later firmware is rechecked, and the result is recorded in the Z1 post-processor definition (GC-04, GC-09). The Z1 rotary conventions (axis letter, units, wrap-around) come from existing Makera machine profiles inspected during development, so gate 4 also checks them against real behaviour; the SP-12 air cut confirmed direction, wrap and the Z1 feed rule, and gate 4 for generated programs repeats the timed move against the IR's predicted time under that rule. The Z1 applies feed to combined moves with its own rule, not LinuxCNC's, so the predicted time uses the post's rule for the controller.

**Why a verification post-processor (GC-10).** It writes the same toolpath for LinuxCNC, a controller with a mature, independent interpreter. Agreement between that run and the Makera Z1 output is evidence that both post-processors share one correct toolpath. It does not prove the Z1 post-processor is right, which is why gates 4 and 5 stay. If SIM-09 (running exported G-code in the application's own simulator) is approved and built, it becomes an extra automatic check at gate 2.

## 7. Dependency and licence verification

Third-party crates and wrapped libraries are verified before adoption and on every release, because the project's long-term success depends on them.

**Adoption review (once per dependency).** Each pick is scored against the NFR-14 criteria in the crate survey (maintenance, maturity, features, quality, adoption, AI-contribution risk) and recorded with its justification in the repository. Newer crates and crates with agent-instruction files need a written justification and a fallback. The scores are a screen, not proof, because the survey compiled and benchmarked nothing, so each layer gets a short spike with a written result before it is relied on.

**Containment.** Every selected crate sits behind an interface we own, and third-party types never appear in a public API. This is the main defence against API churn (parry3d, wgpu, egui and i\_overlay change quickly): a crate can be upgraded or swapped in one place, and the same adapter tests check the swap. Exact versions are pinned and an upgrade cadence is planned. Reviewers check each public API change for third-party types. A fallback chosen after a failed spike may break a rule here (for example a C++ 2D-offset library in openchuck-path); the exception is recorded with its justification and added to the xtask allow-list, so any unrecorded breach still fails CI.

**Automated checks in CI.**

- An xtask dependency-rule check over cargo metadata: only openchuck-ui and openchuck-viewport may depend on egui, eframe or wgpu; only openchuck-import on the STEP kernel binding; only openchuck-vcs on git2; the library family on none of these; and no crate on a network client or telemetry library (INT-01, NFR-09).
- cargo-deny for licence compatibility, banned crates and duplicate versions, with SPDX identifiers checked on files. Allowed on purpose: CC0-1.0 (blake3, notify) and NCSA (libfuzzer-sys, dev-only).
- cargo-audit for known security advisories.
- cargo-semver-checks on the library family's public API (INT-07), and cargo-hack to build and test each library on its own.
- cargo-about to produce the licence notices shipped with binaries (NFR-10).
- A scheduled job that flags crates with no release or commit activity for a set period. Dependency updates arrive as reviewed pull requests, never silently.

**Specific checks.**

- *Spikes (architecture section 11 and the Phase 1 work breakdown):* thirteen short, throwaway spikes, each with a written result and pass criteria in the work breakdown. SP-1 STEP kernel: STEP to a tessellated mesh, with topology, surface data and tessellation control, on all three platforms. SP-2 viewport: egui with wgpu at 30 fps on 1 million triangles. SP-3 a wrapped engraving path on a cylinder to G-code with A words, replayed through openchuck-gcode and in LinuxCNC and checked against the kinematic oracle; it becomes the first golden file and the first run of gates 1 to 3. SP-4 a parry3d-f64 ray-cast sweep around the A axis on a real part. SP-5 the gcode crate reading A words and the LinuxCNC dialect. SP-6 simulation stock representations compared on accuracy, speed and memory, which sets the simulation limit in 4.1. SP-7 2D offsets and booleans. SP-8 project format and order keys with a real Git merge. SP-9 exact surfaces and NURBS evaluation. SP-10 template engine for rotary posts. SP-11 swarf. SP-12 Z1 conventions and G93. SP-13 custom-profile tools (TL-03).
- *Embedded Git (VC-05, requirements section 4.3):* before the revision system is built on git2, a prototype runs commit, restore and merge on a real project folder on all three platforms. The revision code stays behind the VersionControl trait, git2 is built without network features, and libgit2 needs a C compiler on all three platforms (NFR-13). The fallback is the system git command.
- *Exact surfaces (IMP-02, decision 0017):* the SP-9 comparison is kept as a regression test in `openchuck-import`'s kernel-backed tests: for each face of the STEP corpus, the pure-Rust evaluators against the kernel for point and normal, closest point and trimmed ray hits (at most 1e-6 mm, 1e-3 degrees), plus trim classification against the kernel's point-in-face test. The tessellation is checked by measured deviation, never by the kernel tolerance setting (OI-72 to OI-76).
- *C++ bindings (NFR-13):* the STEP kernel (opencascade is the weakest pick at a survey score of 66) builds and passes tests on Windows, macOS and Linux, and stays inside openchuck-import. Whatever crate fills the role is checked with the same corpus.
- *Licence flags (INT-09, NFR-10):* opencascade links LGPL-2.1 code, the brepkit crates are AGPL-3.0-only, and cbindgen and uniffi are MPL-2.0. Each is checked before adoption, and the LGPL kernel stays out of the library family.
- *Licence report:* generated for the full tree, including C++ code built from source, and reviewed before each release. Any change to the AGPL, dual-licence or FreeCAD exception plan is reviewed against it.

## 8. Environment, tools and CI

| Purpose | Tool | Used for |
| --- | --- | --- |
| Unit and integration tests | cargo test, cargo-nextest | All crates; nextest for speed and isolation |
| Property-based tests | proptest | Geometry invariants, round trips, A-axis limits, undo and redo |
| Snapshot and golden files | insta, plain-text golden files | Neutral toolpath, G-code, project files, setup sheet |
| CLI end-to-end tests | assert\_cmd | openchuck-cli scenarios, the system-level harness |
| Fuzzing | cargo-fuzz (needs a nightly toolchain) | STL, STEP, DXF and SVG importers and the G-code reader |
| Memory and UB checks | Miri, sanitizers | Unsafe code and the C++ FFI boundary |
| Static analysis | clippy (deny warnings, deny unwrap and unchecked indexing in libraries), rustfmt | Every pull request |
| Coverage | cargo-llvm-cov | Trend and gap finding, not a target on its own |
| Benchmarks | criterion | Hot paths in pathing and simulation, cross-crate benches |
| Repository checks | xtask with cargo\_metadata | Dependency rules, SPDX headers, traceability report, LinuxCNC replay runner, release gate |
| Library checks | cargo-hack, cargo-semver-checks | Each library builds alone; public API compatibility |
| Dependency checks | cargo-deny, cargo-audit, cargo-about | Section 7 |
| Simulator | LinuxCNC (sim configuration, 4-axis XYZA) | Gate 3 |

**Platforms.** CI builds and tests on Linux, Windows and macOS for every pull request, matching the cross-platform requirement. Jobs are path-filtered, so a change to docs/ or posts/ does not rebuild the STEP kernel. The library family builds and tests on all three platforms without the C++ toolchain; the OpenCASCADE build is confined to openchuck-import and cached. The simulator gate runs on Linux. The stable Rust toolchain is pinned in rust-toolchain.toml; cargo-fuzz runs on a nightly toolchain in a separate job, and a scheduled job builds on beta to catch breakage early. A nightly build job (decision record 0023) builds the release binaries for all three platforms when `main` has changed since its last successful run, keeps one archive per platform as a workflow artifact for 30 days and, for `main`, replaces the rolling `nightly` pre-release with them; these are development builds, not releases, and pass no release gate.

**Test data.** The corpus lives in testdata/ in the repository and stays small. Shapes are generated procedurally where possible, and golden files are plain text so reviews show real diffs. It holds the analytic-oracle cases (cylinder, cone, sphere, helical groove, ruled surface), small real STEP and STL parts suited to 4-axis work, damaged files for each importer, expected toolpaths and golden G-code. A README records each model's origin and licence so the corpus stays redistributable. Whether larger models need Git LFS or a separate test-data repository is an open decision in the architecture.

**Reproducibility.** Every test run records the commit, toolchain version, dependency lockfile and, for machine trials, the controller firmware and settings, so a result can be repeated.

## 9. Criteria, defects and records

**Pull request gate (entry to merge).** Build passes on all three platforms; all tests, clippy and format checks pass; new code has tests; no new advisory or licence finding; traceability report shows no requirement lost its tests.

**First-release exit criteria.** All of the following hold:

- Every P1 requirement in section 4 has passing verification evidence, or an approved, recorded waiver. P2 and P3 requirements are verified when they are built and do not block the first release.
- No open defect of severity 1 or 2.
- The release gate passes: the xtask runs the dependency-rule check, cargo-deny, the golden G-code comparison and the LinuxCNC replay before a tag is cut, and a release with a failing gate is not published.
- G-code gates 1 to 5 passed for the reference job set on the Makera Z1, with results stored, and the G93 test result recorded.
- Fuzzing ran for a set period on each parser with no unresolved crash (NFR-07).
- Licence and advisory reports are clean or have recorded decisions (NFR-10, NFR-14), and each spike in section 7 has a written result.
- The warning that G-code must be verified before use is present and checked (NFR-12).
- A verification report is issued (below) and the plan has been reviewed against what actually happened.

**Defect severity.**

| Severity | Meaning | Rule |
| --- | --- | --- |
| 1 Critical | Wrong or unsafe G-code, crash or data loss | Blocks merge and release; fix first, add regression test |
| 2 Major | Feature fails or output outside tolerance, with no workaround | Blocks release |
| 3 Minor | Feature works with a workaround, or cosmetic fault with effect on use | Fix when scheduled; may ship if listed in release notes |
| 4 Trivial | Cosmetic or wording | Backlog |

**Regression.** The full automated suite runs on every pull request. Any change to kinematics, a post-processor or a strategy also reruns the whole G-code gate set 1 to 3 for the reference jobs.

**Records.** The repository keeps the traceability matrix, golden files, corpus, CI logs, defect list, machine-trial reports (date, program, settings, result, photos) and this plan. A verification report at each release summarises results against the exit criteria, lists waivers and open defects, and names the commit it covers. This plan is versioned in Git with the code and updated when requirements, tools or risks change.

## 10. Risks, assumptions and open items

| Risk | Effect | Mitigation |
| --- | --- | --- |
| Reference machine is a single Makera Z1; one controller may hide post-processor faults | Wrong G-code on other machines | Simulator gate (independent interpreter); post-processors as separate, tested definitions; record controller settings with each trial |
| Common-mode kinematics error: the path library, the post and the G-code reader share one machine model, so a replay can agree with a wrong model | Wrong A direction or offset passes gates 1 and 2 | A separate closed-form kinematic oracle in test code; the LinuxCNC gate; the air cut |
| Incorrect 4-axis kinematics (A direction, wrap-around, rotary offset) | Crash or scrap part | Kinematic round-trip tests; air cut first; documented machine model; profile-driven tests so the Y axis (SET-02) is a profile change |
| Unconfirmed controller behaviour (G93 support, arc and rotary handling) | Feed errors on 4-axis moves | Gate 4 test to confirm; keep the path as short linear segments within tolerance |
| STEP kernel fitness: opencascade scores 66, its README calls it a work in progress and it has no dependents | Missing topology, surface or tessellation control; crashes at the boundary | SP-1 first; fuzzing and boundary tests; the ModelImporter interface lets a fallback (cadrum, an in-house wrapper or brepkit-io) run the same corpus |
| NURBS surface evaluation has no proven pure-Rust crate (written in-house, decision 0017) | Wrong normals or distances on STEP surfaces | Oracle tests against analytic shapes and the kernel's own evaluation; evaluators written in-house per decision 0017 |
| Material removal simulation is written in-house (dexels, decision 0022) | Wrong or slow results; collision queries depend on it | SP-6 settled the representation; independent reference checks; first cut compared with the simulation |
| API churn and abandonment in dependencies | Breakage and schedule loss | Section 7: containment behind our own interfaces, pinned versions, planned upgrade cadence, scheduled activity check |
| Order-key scheme for merge-friendly files is unproven | Merge conflicts in project files | Prototype with a real Git merge; keep the merge test in openchuck-project |
| Selection matching after re-export (IMP-07, PRJ-07, P2) is hard | Lost selections after a model change | Own prototype; descriptor fields reserved in the schema; verified when the P2 work is built |
| Single maintainer limits independent review | Missed defects | CI automation, independent simulator, test-only independent checker, delay-and-reread before sign-off, external reviewers from the FreeCAD and LinuxCNC communities when available |

**Assumptions** (from the requirements document). The A axis rotates the workpiece about the machine X axis with a vertical tool axis; the target is a mill with a rotary axis programmed in degrees; the Makera Z1 is the reference machine and G93 is unsupported (confirmed by the SP-12 air cut); the internal unit is millimetres; work is for a single user on a single machine; threading, turning and 5-axis orientation are not needed. The project is open source under a GPL-compatible licence, with the pathing library starting under AGPL.

**Open items.**

- Keep section 4 in step with the requirements document and section 5 in step with the architecture; a change to either needs a matching change here.
- The limits in 4.1 and the hardware in 4.2 are starting values. Revise them after the first benchmarks, the simulation spike and the 3-axis test cut, and record each change.
- Open architecture decisions that change verification: simulation stock representation (decided by SP-6, decision 0022: dexels) and Git LFS or a separate test-data repository.
- The reference job set (parts, stock, tools) for gates 4 and 5 is chosen in work breakdown slice 6.
- A safe-practice checklist for first machine trials is written in work breakdown slice 20 and reviewed by whoever operates the Makera Z1.
- P2 groups (COL, VC, INT) have outline verification only; detailed test design waits until their designs exist.
