# OpenChuckCAM: High-Level Architecture

Revision 4, 11 Oct 2026. Rebuilt against the requirements doc *OpenChuckCAM - Initial Requirements* (IDs such as OP-05 and NFR-01 refer to it) and the crate survey *Rust crates for a CAM / 4th-axis project* (scores out of 100 are that survey's, as of 10 Oct 2026). Section 13 lists what changed from the first draft.

## 1. Scope and architectural drivers

A Rust desktop CAM application: STEP and STL in, true 4-axis (XYZA) G-code out, with simulation, tool management, multiple post-processors and version-control-friendly projects. Priorities are P1 (first usable release), P2 (follows the core) and P3 (suggested, all now approved). Out of scope for the initial release: machine communication, CAD editing, 5-axis and turning.

The requirements that shape the structure most:

| Driver | Requirements | Consequence for the architecture |
| --- | --- | --- |
| Engine is UI-free and headless | NFR-01, GC-11, NFR-08 | Every capability is a library crate; a CLI and the UI are two equal front ends over one application API |
| Reusable libraries for other hosts (FreeCAD) | INT-01 to INT-09 | Pathing and post-processing are standalone libraries: plain-data API, progress and cancellation, versioned, no dependency on the project file or UI |
| Strategies are pluggable and self-describing | OP-16, UI-07 | One `Strategy` trait; each strategy publishes a parameter schema that the UI renders without knowing the strategy |
| Later operations see earlier results | OP-03, OP-09, SIM-06, COL-05, COL-08 | The pathing library defines ports for in-process stock and collision queries; the simulator implements them |
| Machine-neutral toolpaths, machine-specific posts | GC-01 to GC-04, OP-13 | A neutral toolpath IR carries intent (contact feed, tool axis, A angle); all dialect and rotary conventions live in post definitions |
| Posts editable without recompiling | GC-03, GC-13 | Post definitions are data loaded at run time; a scripting engine can be added later behind the same trait |
| Repeatable results | NFR-06, GC-07 | Parallel code must give identical results for any thread count; tested by running with 1 and N threads |
| Diff- and merge-friendly projects | PRJ-01 to PRJ-07, VC-01 to VC-05 | Text files, stable IDs, explicit order keys, hashes for model files, no generated data in the project |
| Large models and long jobs | NFR-03, NFR-04, UI-09 | Custom wgpu viewport sized for 1M triangles; job system with cancellation and progress; the UI never calls the engine directly |
| Robustness and licensing | NFR-07, NFR-09, NFR-10, NFR-13, NFR-14 | No panics on bad input; offline by construction; C++ kernel isolated in one crate; every dependency chosen and recorded against the survey criteria |

## 2. Principles

- **Dependencies point inward.** Engine crates know nothing of the UI, GPU, Git or the project file. Those are adapters behind traits we own.
- **Third-party types never appear in a public API.** Every selected crate sits behind an interface of ours. The survey found heavy API churn (parry3d 0.26 to 0.31 in eight months, wgpu 26 to 30 in twelve, i\_overlay 5.0 to 9.0 in five), and INT-07 promises a stable API, so each crate can be swapped or upgraded in one place.
- **Plain data across boundaries.** Geometry, stock, tools, machine profile, parameters and toolpaths are serialisable structs (INT-02), which also makes them easy to test, snapshot and bind to Python or C.
- **Headless first.** Anything testable without a window is tested without a window.
- **Deterministic by design.** Same project and version give byte-identical G-code (GC-07, NFR-06).
- **No panics at the boundary.** Importers and parsers return errors for damaged input (NFR-07); library and FFI entry points never unwind across a boundary.

## 3. Workspace layout

| Crate | Role | Main requirements | Project dependencies |
| --- | --- | --- | --- |
| `openchuck-types` | Plain data shared by all libraries: units, mesh and B-rep data, stock, tool and tool assembly, machine profile, **neutral toolpath IR**, job context (cancel token, progress sink), error types. Serde-serialisable, double precision, millimetres. | GC-01, INT-02, NFR-05 | none |
| `openchuck-path` | **Reusable 4-axis pathing library.** Strategies, tool-axis control, A-axis management, linking, gouge protection, feed intent. Defines the ports `SurfaceSource`, `StockModel` and `CollisionOracle`. | OP-01 to OP-17, INT-01 to INT-04 | `openchuck-types` |
| `openchuck-post` | **Reusable post-processing library.** IR to neutral machine program, then G-code through user-editable post definitions. The Z1 and LinuxCNC verification posts ship as definitions. | GC-02 to GC-10, GC-13, INT-03 | `openchuck-types` |
| `openchuck-gcode` | G-code reader and interpreter: parses G-code (including A words) back into motion for limit checks, replay against the IR, and later simulation of exported code. | GC-08, SIM-09 | `openchuck-types` |
| `openchuck-geom` | Mesh utilities: adjacency, validity checks and report, transforms, selection algorithms (flood fill, select-by-angle), ray-cast picking. | IMP-04, IMP-05, UI-04 | `openchuck-types` |
| `openchuck-import` | Model and curve readers behind a `ModelImporter` trait: STL, STEP (C++ kernel wrapped here and nowhere else), DXF, SVG. Unit handling, tessellation at a set tolerance, deterministic B-rep face and edge numbering. | IMP-01 to IMP-03, IMP-06, IMP-08, NFR-13 | `openchuck-types`, `openchuck-geom` |
| `openchuck-sim` | Material removal simulation, deviation measurement, tool engagement, time estimation, collision detection. Implements the pathing ports. | SIM-01 to SIM-10, COL-01 to COL-08 | `openchuck-types`, `openchuck-path` |
| `openchuck-project` | Project document model, text persistence, schema migration, stable IDs and order keys, selection sets, content hashes, cache location, autosave, tool and material libraries (separate store), semantic project diff. | PRJ-01 to PRJ-08, TL-01 to TL-11, UI-05, VC-03 | `openchuck-types` |
| `openchuck-vcs` | `VersionControl` trait with a `git2` adapter, built without network features. | VC-01 to VC-06 | `openchuck-project` |
| `openchuck-app` | Application layer: commands and queries, undo and redo, selection model, job manager, operation pipeline, caches, project-to-engine mapping, setup sheet export, event stream. UI-agnostic. | UI-06 to UI-09, GC-05, GC-12, NFR-04 | all engine crates |
| `openchuck-cli` | Headless front end over `openchuck-app`: regenerate G-code, validate, report. Also the harness for end-to-end tests. | GC-11, NFR-08 | `openchuck-app` |
| `openchuck-viewport` | wgpu renderer: scene, camera, shading and wireframe, polyline toolpaths, tool and stock display. Draws data; contains no CAM logic. | UI-02, UI-03, SIM-01, SIM-02, NFR-03 | `openchuck-types` |
| `openchuck-ui` | egui application: panels, project tree, parameter forms, playback controls. Talks only to `openchuck-app` and `openchuck-viewport`. | UI-01 to UI-09, SIM-04 | `openchuck-app`, `openchuck-viewport` |
| `openchuck-testkit` | **Test-only, never published.** The independent kinematic oracle (section 9), the determinism harness, golden-file comparison and procedurally generated analytic test parts. Used only as a dev-dependency; the xtask dependency check fails if any crate takes it as a normal dependency. | NFR-06, NFR-08, OP-05, OP-06, GC-09 | `openchuck-types` |
| `openchuck-benches` | **Not published.** The package behind `benches/`: cross-crate benchmarks (section 12). | NFR-03, NFR-04 | Library crates, as dev-dependencies only |

`openchuck-testkit` and `openchuck-benches` were added in work breakdown slice 0 (decision record 0004). The library family may keep `openchuck-testkit` as a dev-dependency, because it depends only on `openchuck-types`; a split of the library family copies it.

Later, outside the core release: `openchuck-path-py` (Python bindings, INT-05), `openchuck-path-capi` (C interface, INT-06, P3) and an example host program (INT-08). All three depend only on the libraries.

Tool and material libraries and the setup sheet start as modules (in `openchuck-project` and `openchuck-app`) and become crates only if a second consumer appears.

**Figure 1: static structure.** The crates grouped by the layers in section 4. A solid arrow points from a crate to a crate it depends on. A dashed arrow is a test-only dev-dependency, and a dashed box is planned and not yet in the tree. To keep the layers readable, the figure omits the arrows to `openchuck-types` (every crate that holds plain data depends on it) and from `openchuck-app` to `openchuck-path`, `-post` and `-gcode`. The table above is the complete list.

![Figure 1: static structure of the crates, grouped by layer. Front ends sit above the application, which depends on the services and libraries. The libraries and services depend on openchuck-types. Test-only crates and the planned openchuck-vcs are drawn with dashed outlines.](diagrams/static-structure.svg)

## 4. Dependency rules

Layers, top to bottom, each depending only on the layers beneath it:

```
Front ends     openchuck-ui (egui)   openchuck-cli
               openchuck-viewport (wgpu, draws data only)
Application    openchuck-app: commands, undo, jobs, pipeline, caches
Services       openchuck-sim   openchuck-import   openchuck-project -> openchuck-vcs -> git2
               openchuck-geom  (import also wraps the C++ STEP kernel)
Libraries      openchuck-path   openchuck-post   openchuck-gcode      (reusable, UI-free)
Shared data    openchuck-types: plain data, neutral toolpath IR
```

- The library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, -gcode) depends on nothing project-internal beyond itself. A CI check over `cargo metadata` fails the build if it gains a dependency on the application crates, the UI, GPU, Git or the C++ kernel (INT-01).
- `openchuck-sim` may depend on `openchuck-path` (to implement its ports); nothing in the library family depends on `openchuck-sim`.
- Only `openchuck-ui` and `openchuck-viewport` may depend on egui, eframe or wgpu. Only `openchuck-import` may depend on the STEP kernel binding. Only `openchuck-vcs` may depend on `git2`.
- No crate in the tree may depend on a network client or telemetry library (NFR-09); `git2` is built with local features only.
- `cargo deny` enforces licence compatibility and records every dependency's licence (NFR-10). Keeping the LGPL kernel out of the library family keeps the AGPL start, and any later exception or dual licence, simple (INT-09).

**Exceptions.** A fallback chosen after a failed spike may break a rule above, for example a C++ 2D-offset library in `openchuck-path`. The exception is recorded with its justification (NFR-14) and added to the xtask allow-list, so the check still fails on any unrecorded breach.

## 5. Component design

### 5.1 Shared data and the neutral toolpath (`openchuck-types`)

- **Neutral toolpath IR (GC-01).** An ordered list of segments grouped by operation and setup. Each segment carries the tool tip position, the tool axis, the explicit A angle, the move kind (rapid, linking, cutting, plunge, lead-in or lead-out), the **cutter-contact feed**, the computed segment duration and the contact path length, plus tags back to the operation, tool and setup. It names no controller, axis letter or dialect.
- **Why contact feed and duration, not a feed number.** OP-13 makes compensated feed in units per minute the default and inverse-time (G93) an optional post capability. Keeping the contact-point intent in the IR lets the post choose the feed mode (GC-04) without the engine knowing the controller. The Z1 does not support G93 (confirmed by the SP-12 air cut, decision 0007), and its compensated feed follows the controller's own rule, not LinuxCNC's, so the post converts duration to F by a named per-controller rule.
- **Machine profile as data (SET-01, SET-06).** Rotary axis orientation as a parameter (X in the first release, Y later under SET-02), A range and continuous rotation, travel limits, maximum feeds, clearance geometry, and optional head and fixture solids for collision work (COL-04). Strategy maths takes the rotary axis as a parameter and never hard-codes X, so SET-02 is a profile change rather than a rewrite. The forward kinematics (XYZA to tool tip) live once in openchuck-types and are shared by the path library, the post and the G-code reader; independence comes from the test-only oracle in section 9, not from separate copies.
- **Tool and stock as data.** Tool types TL-02 and TL-03 (flat, ball, bull-nose, tapered ball, V-bit, chamfer, drill and custom profile), all described by one revolved profile of line and arc segments that path solving, the gouge checker and the simulation sweep share (spike SP-13); custom profiles are imported from a text file or drawn in the UI; assembly with holder and collet (TL-08), stock as cylinder, box, offset bounding box or imported solid (SET-03, SET-04).
- **Job context (INT-04, UI-09).** A cancel token and a progress sink passed into every long-running call.

### 5.2 Pathing library (`openchuck-path`)

Built as if it already belonged to another project (INT-01): small public API, plain data in and out, no application concepts.

**Inputs.** Geometry as a triangle mesh or as B-rep faces and edges (INT-02), selected regions and boundaries (OP-02), stock, tool, machine profile, tolerance and operation parameters.

**Ports it defines and the host implements.**

- `SurfaceSource`: evaluate point and normal, project, ray and distance queries on the selected surfaces. Implemented inside the library for meshes and for B-rep faces given as plain data: analytic surfaces (plane, cylinder, cone, sphere, torus) and clamped rational B-spline patches, each with UV bounds and trim loops, evaluated in pure Rust ([decision 0017](decisions/0017-sp-9-exact-surfaces.md)). A host that has its own kernel (FreeCAD has OpenCASCADE) can implement it directly for exact evaluation.
- `StockModel`: query remaining material along a ray or region. Needed for rest machining and remaining-stock-aware roughing (OP-03, OP-09) and for carrying stock between operations (SIM-06). Implemented by `openchuck-sim`; a simple analytic implementation ships for tests and small hosts.
- `CollisionOracle`: answer collision queries for tool, holder and linking moves (OP-14, COL-08). Implemented by `openchuck-sim`; the default accepts everything and relies on the clearance geometry.

**Strategies.** One `Strategy` trait and a registry (OP-16). A strategy exposes a self-describing parameter schema (names, units, ranges, defaults, validation) so the parameter panel (UI-07) is generated from it. The P1 set, grouped by how it is computed:

- *Wrap and unroll family*: helical, axial and indexed finishing (OP-04), edge and curve following (OP-08), indexed and continuous rotary roughing (OP-03). Map the surface to a 2D development, use 2D offsets and booleans, convert back to A angles.
- *Continuous family*: simultaneous 4-axis finishing and swarf (OP-05, OP-07) with tool-axis control (OP-06). Sweep A in small steps, query the surface along X, then offset for the cutter, link passes and check gouging.
- *Support*: rest and pencil clean-up (OP-09), radial drilling at indexed A (OP-10), adaptive roughing (OP-17, P2).

**Shared services used by every strategy.** Gouge protection against the model and holder (OP-12), linking and retracts through the clearance geometry (OP-14), A-axis management: range limits, reversal minimising, wrap-around and unwinding (OP-15), common parameters (OP-11).

**Library rules.** Pure functions over immutable inputs; no globals, filesystem or logging side effects beyond a diagnostics sink; semver-stable API with a changelog (INT-07); documented with a small example host (INT-08); optional Python and C adapter crates later (INT-05, INT-06).

### 5.3 Post-processing library (`openchuck-post`)

- **Two stages.** The IR is first converted to a neutral machine program (motion, spindle, coolant, tool change, work offset, comments), then rendered to text by a post definition. This keeps templates simple and lets a host use post-processing alone (INT-03).
- **Post definitions are data (GC-03, GC-04).** User-editable text templates plus options: units, A-axis letter, direction and wrap behaviour, header and footer, tool change sequence, coolant, work offsets, line numbering, precision, maximum feed, and feed mode for rotary moves. Machine conventions live here and never in the engine.
- **Feed mode implementations.** Compensated feed in units per minute (default) and inverse-time (optional, for controllers that support it), both derived from the IR's contact feed and segment duration.
- **Shipped posts.** Makera Z1 (GC-09), with its conventions derived from existing Makera profiles during development, and a LinuxCNC-dialect verification post (GC-10).
- **Output control.** One file for the project or one per setup, operation or tool (GC-05); comments naming operation, tool and estimated time (GC-06); byte-identical output (GC-07); revision identifier written into the header when supplied (VC-06); the tool numbers are shown for the user to confirm, renumber in operation order or renumber by hand before the file is written, and the choice is saved in the project (GC-14). The application also shows the NFR-12 notice that generated G-code must be verified before it is run.
- **Scripting later (GC-13).** A `PostProcessor` trait with the template implementation first, so a scripting engine slots in without changing callers.

### 5.4 Import and geometry (`openchuck-import`, `openchuck-geom`)

- `ModelImporter` adapters produce `openchuck-types` data: STL (IMP-01), STEP with B-rep retained (IMP-02), and DXF and SVG curves (IMP-08, P2). Units are read from the file, overridable by the user, and converted to millimetres at this boundary (IMP-03).
- STEP faces and edges are numbered **deterministically**, so the same file always gives the same IDs. Selection storage (PRJ-06) depends on this.
- B-rep tessellation at a user-controlled tolerance (IMP-06) happens in the kernel adapter. STL carries triangles only, so its tessellation tolerance is fixed before our code sees it; gouge tolerance on STL parts is measured against that mesh, and the UI states this.
- `openchuck-geom` holds format-independent mesh work: adjacency and validity checks reported to the user (holes, non-manifold edges, flipped normals, degenerate triangles, IMP-04), transforms and orientation to the rotary axis (IMP-05), select-by-angle and flood fill (UI-04), and ray-cast picking for the viewport.

### 5.5 Simulation and collision (`openchuck-sim`)

- **Stock representation (decided, [decision 0022](decisions/0022-sp-6-simulation-stock.md)).** No crate exists for material removal simulation (survey). It is written in-house, in `openchuck-sim`, as dexels in the part frame (material intervals along part Z on an (x, y) grid, the tool span solved exactly per column, so the tool may tilt as the part turns), behind the `StockModel` port so the representation can change. Voxel is the recorded fallback; the rotary grid is not used as the main stock, because a ray from the axis that leaves the tool through a wall also removes the material beyond it, which is wrong for slots, pockets and islands. The default cell is 0.1 mm. The user may configure a finer resolution at the cost of speed and memory (SIM-07); a finer setting is never the default; the finest the user may choose is 0.025 mm ([decision 0025](decisions/0025-slice-7-simulation.md)). Moves are sampled as the controller interpolates them (linear in machine X, Y, Z and A, through the shared kinematics) into poses no further apart than the cell.
- **Carry-over and playback.** Stock state flows from one operation to the next (SIM-06). The simulator keeps checkpoints so playback can jump to any operation or move (SIM-04) without replaying from the start.
- **Results as data.** Display meshes, per-point remaining-material and overcut values, worst-case figures (SIM-05), tool engagement for later feed optimisation (SIM-10), and time estimates (SIM-08) are returned as data; the viewport draws them.
- **Collision (P2).** Tool cutting part, shank, collet and holder, machine head and fixtures against model and in-process stock, with a safety margin and a list of hits with operation, move and location (COL-01 to COL-07). The same queries feed the `CollisionOracle` port for automatic avoidance (COL-08, P3).
- **G-code simulation (SIM-09, P3).** Uses `openchuck-gcode` to run exported G-code through the same simulator. The reader is built early anyway as a test tool (section 9).

### 5.6 Project, libraries and version control (`openchuck-project`, `openchuck-vcs`)

- **Project folder.** TOML text files with stable key ordering, split into small files (project, setups, one file per operation, selection sets, machine) so diffs and merges are meaningful (PRJ-01). Large models stay as separate files referenced by relative path with a content hash checked on load (PRJ-02). Every file records a schema version and older versions are migrated on load (PRJ-04).
- **IDs and order.** Operations, tools, setups and selection sets have stable unique IDs. Order is stored as an explicit order key on each item, so reordering touches one item and two branches that edit different items merge cleanly (PRJ-05).
- **Generated data is not stored (PRJ-03).** Toolpaths, simulation caches and G-code are regenerated; any cache lives in a directory the app keeps in a generated ignore file. Autosave and crash recovery write outside the tracked files (PRJ-08).
- **Selection references (UI-05, PRJ-06, PRJ-07).** Named selection sets are objects operations point to by ID, so editing a set updates every operation that uses it. B-rep selections store face or edge IDs relative to the model file's content hash. Mesh selections store triangle sets and apply to mesh models only; a STEP model is selected by face and edge, because triangle indices change when the tessellation tolerance changes. The schema reserves descriptor fields (position, normal, area) from the start, so P2 re-matching after re-export (IMP-07, PRJ-07) needs no format change.
- **Tool and material libraries.** Stored separately from projects in a human-readable format with import and export (TL-01, TL-09). The tool library is a folder with one file per tool and, in `tool-sets/`, each machine's default tool set with its tool numbers (TL-11); tool numbers belong to the project, not the library tool. Each project stores a snapshot of every tool it uses, with its number (TL-07, decision 0014). Importing external tool tables such as LinuxCNC's is an import adapter (TL-10, P3).
- **Version control (VC-01 to VC-06).** `VersionControl` trait with a `git2` adapter, built without network features. The project folder stays a standard Git repository, so using the built-in system never stops external Git tools working (VC-05). The structured comparison of operations, tools, setups and parameters (VC-03) is computed by `openchuck-project` over stable IDs, not by diffing text. The revision identifier is passed to the post for the G-code header (VC-06).

### 5.7 Application layer (`openchuck-app`)

- **One API for every front end.** Commands change the project (all undoable, UI-08), queries return view models, and an event stream reports job progress and results. The UI and the CLI use exactly this API.
- **Job manager (UI-09, NFR-04).** Runs pathing, simulation and import on a worker pool with cancellation and progress, and never on the UI thread.
- **Operation pipeline.** See section 6.
- **Mapping.** Converts project-model objects to the library's plain data, so the libraries never see the project file (INT-01).
- **Export.** G-code through `openchuck-post`, setup sheet as HTML or PDF (GC-12), with times once SIM-08 exists.

### 5.8 Viewport and UI (`openchuck-viewport`, `openchuck-ui`)

- `openchuck-viewport` owns the render loop, camera, standard views, perspective and orthographic projection, shaded, wireframe and edge display, and large polyline rendering for toolpaths (UI-02, UI-03, SIM-01, SIM-02). Target: 30 frames per second or better on models up to 1 million triangles (NFR-03), through each platform's native graphics via wgpu (NFR-02).
- Picking uses CPU ray casts against a BVH in `openchuck-geom`, so it is testable without a window; the viewport only draws the highlight.
- `openchuck-ui` holds panels, the project tree (UI-06), parameter forms generated from strategy schemas (UI-07) and playback controls (SIM-04). It contains no CAM logic and never imports an engine crate.

## 6. Operation pipeline

1. **Import** produces model data with deterministic IDs and a content hash.
2. **Setup** fixes machine profile, stock, part position, work offset and clearance geometry (SET-01 to SET-07).
3. **Select** geometry in the viewport; the app stores it as a named selection set (UI-04, UI-05).
4. **Operation** combines strategy, tool, setup, selection and parameters (OP-01) and is regenerated on demand.
5. **Generate** runs the strategy as a background job. Inputs for operation N include the stock left by operations before it, supplied through `StockModel`.
6. **Simulate** applies the toolpath to the stock, produces deviation and engagement, and hands the updated stock to operation N+1 (SIM-06).
7. **Post** converts all operations to G-code through the selected post (GC-02).

**Figure 2: dataflow.** The pipeline above as data moves through it. Setup, selection and operation are user edits, stored in the project folder. Import, generate, simulate and post are engine stages. Generate and simulate write to the cache, which is regenerated and never stored in the project (PRJ-03). The simulated stock returns to Generate for the next operation through the `StockModel` port (SIM-06). The G-code replay checks the output against the IR (GC-08).

![Figure 2: dataflow from import through setup, selection and operation to generate, simulate and post, then G-code. The stock from simulation feeds the next operation's generation. User edits go to the project folder; generated results go to the cache; the G-code file is replayed for checking.](diagrams/dataflow.svg)

Operations form a dependency chain, so editing operation N invalidates the cached results of N and everything after it, and nothing before it. Caches are keyed by a hash of the operation's inputs and the engine version, which also makes regeneration after load cheap when nothing changed.

## 7. Crate selection

Taken from the crate survey, which scores crates out of 100 against NFR-14's criteria (maintenance, maturity, features, quality, adoption, AI-contribution risk). Each pick is used behind an interface we own (principle 2), and the survey's advice to pin exact versions applies to all of them. The survey compiled and benchmarked nothing, so each layer needs a short spike before it is relied on. The supporting layers that the first survey pass did not cover are in a second table after this one; the survey scored them later.

| Layer | Pick (survey score) | Where it lives | Fallback and notes |
| --- | --- | --- | --- |
| STEP import and B-rep | Thin in-house C ABI wrapper over OpenCASCADE 7.8.1, the kernel built by `occt-sys` (build dependency only) and the wrapper compiled with `cc`. Decided by SP-1, [decision 0003](decisions/0003-step-kernel-route.md). | `openchuck-import` only, behind `ModelImporter`; STEP parsing runs in an isolated child process with a timeout | `opencascade` (66) was the survey pick and was rejected by SP-1: its safe API lacks per-face triangles, surface parameters, UV bounds and angular deflection, and `opencascade-sys` does not build under MSVC. Survey notes on it, kept for the record: weakest pick in the stack: README calls it a work in progress, no dependents, needs CMake and a C++ compiler, LGPL-2.1. Fallbacks: `cadrum` (63; 0.5 years old, 41 releases in 12 months, agent-instruction files, so a justified-risk choice under NFR-14), `truck-stepio` (56, no release since Sep 2024). `brepkit-io` (60) is a pure-Rust option found in the survey's later pass; it is AGPL-3.0-only, 0.2 years old and carries agent-instruction files, so it is a spike candidate only. No STEP crate scores above 66. Needs a build on all three platforms (NFR-13). |
| STL import | `stl_io` (88) | `openchuck-import` | `mesh-loader` (57). Reads ASCII and binary. |
| Geometry queries (BVH, ray, distance, contact) | `parry3d-f64` (83, scored as parry3d) | Internal to `openchuck-path`, `openchuck-geom`, `openchuck-sim` | Own BVH. Use the f64 variant because the default build is f32 and NFR-05 needs double precision. Highest API churn among the CAM-critical crates. |
| Maths | `glam` (89), f64 types | Internal | `nalgebra` (93) where linear algebra such as SVD is needed. parry3d moved to glam in 0.26, so glam avoids a second maths type. Avoid `glamx` (71, under a year old). Public APIs use our own plain types. |
| Parallelism | `rayon` (96) | Job manager and library internals | Used under the determinism rules in section 8. |
| Spatial index | `rstar` (88) | Internal | Also pulled in by parry3d. |
| 2D offsets and booleans | `cavalier_contours` (82) plus `i_overlay` (80) | Internal to `openchuck-path` unroll-family strategies | `clipper2` (73, C++ wrapper). `i_overlay` had five major versions in five months, so pin it. Arc support confirmed (SP-7, decision 0016): arcs are kept through the 2D stage and chorded when converted to A angles; G2 and G3 are not used with A. Offset the original region by k times the step, and validate input first. |
| Viewer and UI | `eframe`/`egui` (90) with `egui_wgpu` (87), which uses `wgpu` (88) | `openchuck-ui`, `openchuck-viewport` | Fallbacks: Tauri (89, JavaScript viewer, so `openchuck-viewport` would be replaced) or `iced` (75, no release for 10 months). egui releases a breaking version every two to three months; wgpu went 26 to 30 in twelve. Slint (79) has agent-instruction files and a GPL-3.0 or commercial licence question. Bevy is heavy for a tool UI. |
| G-code reading | In-house reader for the codes our posts emit | `openchuck-gcode` | Decided by SP-5 ([decision 0005](decisions/0005-sp-5-gcode-reader.md)). `gcode` (75) reads A words but stores numbers as `f32` and drops a code it cannot store without a diagnostic; `g-code` (73) panics on malformed input and rejects a file ending in `%` and a newline. `ngc` (46) is stale. The reader refuses parameters, expressions and O-words by name; a slice that needs them widens it. |
| G-code writing | In-house emitter driven by post definitions | `openchuck-post` | The survey found no post-processor crate and says the emit side is ours. |
| DXF and SVG curves (P2) | `dxf` (72), `usvg` (85) | `openchuck-import` | Only needed for IMP-08. |
| 4th-axis planning | In-house | `openchuck-path` | No crate exists. Prior art is in other languages and informs the strategy families in 5.2. |
| Material removal simulation | In-house | `openchuck-sim` | No crate exists; `mesh_to_sdf` (59) is stale. |
| Mesh validity checks | In-house over an edge map | `openchuck-geom` | `halfedge` (64) is 0.3 years old with AI-tool markers, so it is not selected under NFR-14. |
| Embedded Git | `git2` (96) | `openchuck-vcs` | Decided in the requirements (section 4.3). `gix` (79) scores lower because its own status file lists checkout and restore, merge, rebase and stash as not done; it can replace `git2` later if they mature. libgit2 is C code, so it needs a C compiler on all three platforms (NFR-13); the fallback is calling the system git command. |

**Supporting layers.** Scores are from the survey's later sections, out of 100, using the same criteria.

| Layer | Pick (survey score) | Where it lives | Fallback and notes |
| --- | --- | --- | --- |
| Post templates (GC-03) | `minijinja` (94) | `openchuck-post` | `handlebars` (93), `tera` (90). Runtime templates that users can edit; `askama` (85) compiles them in, so users could not edit them. minijinja reached 3.0 on 8 Oct 2026, so pin it and check fixed-decimal number formatting in the spike. |
| Post scripting (GC-13, P2) | Spike `rhai` (90) | `openchuck-post` | `starlark` (84) is built to be deterministic; `mlua` (94) scores higher but needs a C compiler. Pick on fit, not rank. rhai's repository carries an AGENTS.md; the survey scores it 4 on AI risk because the project is old. |
| Project files (PRJ-01 to PRJ-04) | `serde` (93) with `toml` (92); `toml_edit` (93) for hand-edited files (decision 0006) | `openchuck-project` | `serde_json` (97), `ron` (85). `serde_yaml` is archived. Use ordered maps so saved files are stable (NFR-06). |
| IDs and hashes | `uuid` (99), `blake3` (93) | `openchuck-project` | `sha2` (85). Order keys (PRJ-05) are written in-house: `fractional_index` (64) is the only crate and serves as a reference. |
| Setup sheet (GC-12) | HTML from the template engine; `printpdf` (90) if a PDF is required | `openchuck-app` (setup sheet module), rendering through the template engine | `typst` (88) is heavy; `genpdf` (53) is abandoned. |
| NURBS evaluation | In-house, pure Rust, from plain data (decision 0017); analytic surfaces in-house | `openchuck-path` | `curvo` (71) tried by SP-9 and not adopted (slow projection, experimental ray tool, panics on periodic input); `brepkit-math` (66) is AGPL-3.0-only. |
| Python bindings (INT-05, later) | `pyo3` (99) with `maturin` (82) | `openchuck-path-py` | `uniffi` (81). |
| C interface (INT-06, later) | `cbindgen` (84) over a hand-written `extern "C"` layer | `openchuck-path-capi` | `diplomat` (82). `cbindgen` and `uniffi` are MPL-2.0; check against INT-09 and NFR-10. |
| Errors and logging | `thiserror` (94) in libraries; `anyhow` (96) and `tracing` (89) in applications | All crates; libraries log nothing | `miette` (75), `log` (95). |
| Tests, fuzzing, benchmarks | `proptest` (100), `insta` (94), `assert_cmd` (88), `cargo-fuzz` (82), `criterion` (88) | Dev-dependencies and fuzz targets | `quickcheck` (85), `divan` (72). Match the verification levels in section 9. |
| CI tooling | `cargo-deny` (82), `cargo-llvm-cov` (83), `cargo-semver-checks` (78), `cargo-nextest` (80) | CI | `cargo-audit` (79). Dev-only; low adoption scores reflect that these are tools. |
| Repository checks and release (section 12) | `cargo_metadata` (89) for the dependency-rule check; `cargo-hack` (77); `cargo-about` (79); `cargo-release` (79) | `xtask` and the release gate | `release-plz` (73); `git-cliff` (80) for changelogs; `cargo-dist` (70). `cargo-vet` (60) is optional process. |
| CLI, jobs, files | `clap` (97), `indicatif` (92), `crossbeam-channel` (92), `notify` (92), `dirs` (87) | `openchuck-cli`, `openchuck-app` | `argh` (82). |
| UI helpers | `bytemuck` (96), `egui_plot` (83), `egui_tiles` (81) | `openchuck-ui`, `openchuck-viewport` | `egui_dock` (76) has agent-instruction files. |
| Units and undo | In-house newtypes and command stack | `openchuck-types`, `openchuck-app` | `uom` (84). The undo crates have almost no users. |

**Decided in-house or by spike, so no crate is named:** order keys (PRJ-05), units and the undo stack (UI-08), NURBS evaluation, G-code writing, 4th-axis planning, simulation and mesh validity checks. Every other layer in this document now has a scored candidate in the survey. The scores are a screen, not a proof: nothing was compiled, the Features score is judgment, maintenance is judged from release cadence, and AI risk only sees visible agent-instruction files.

**Licence flags to settle against INT-09 and NFR-10 before adopting:** OpenCASCADE (LGPL-2.1 with its exception) and `occt-sys` (LGPL-2.1, build only), statically linked into `openchuck-import`, are accepted (decision 0003); the `brepkit` crates are AGPL-3.0-only, and `cbindgen` and `uniffi` are MPL-2.0. The other picks above are not flagged, but the survey did not review every licence in the dependency tree; `cargo-deny` in CI does that. Also allow on purpose: blake3 and notify are CC0-1.0 (alone or as one option), and libfuzzer-sys (behind cargo-fuzz, dev-only) adds NCSA.

## 8. Cross-cutting rules

- **Determinism (NFR-06, GC-07).** Parallel work uses fixed chunking that does not depend on the thread count, ordered collection of results, and no floating-point reductions whose order depends on scheduling. Maps and sets that affect output use ordered containers. G-code is written with fixed number formatting. Tested with 1, 2 and N threads.
- **Errors, never crashes (NFR-07).** Typed errors with user-readable messages from every importer and parser; no `unwrap` or unchecked indexing in library code (enforced by lints); panics are caught at FFI and binding boundaries; the C++ kernel boundary converts exceptions to errors.
- **Units and precision (IMP-03, NFR-05).** Millimetres and degrees internally as typed values, double precision throughout, default tolerance 0.01 mm. Inch display and input is a user setting in the UI layer only.
- **Concurrency (NFR-04).** One worker pool owned by the job manager; libraries take a context rather than spawning their own threads.
- **Offline (NFR-09).** No account, licence server, update check or telemetry code exists; enforced by the dependency check in section 4.
- **Safety notice (NFR-12).** Shown at export in the UI, printed by the CLI, and available as an optional header comment from the post.
- **Licences (NFR-10).** Recorded by `cargo deny`; the LGPL kernel stays in `openchuck-import`; the library family starts under AGPL (INT-09).

## 9. Verification architecture

The crate split makes each layer testable alone, and everything except pixel output runs headless (NFR-01, NFR-08). Requirement IDs go into test names or attributes so a traceability report can be generated from the test run.

| Level | What is checked | How |
| --- | --- | --- |
| Unit and property | Kinematic mapping, A-axis wrap and unwinding (OP-15), unit conversion, order keys, parameter validation, template rendering | Standard tests plus property tests for invariants: A continuity within limits, no NaN, round trips |
| Analytic oracles (NFR-08) | Toolpaths on shapes whose ideal path is computable: cylinder, cone, sphere, helical groove, ruled surface for swarf | Compare generated paths against closed-form results within tolerance; regression-lock the results |
| Independent gouge and coverage check (OP-12) | No cut into the model beyond tolerance; selected region fully covered | A separate checker, used only in tests, that uses different code from the strategies so they do not grade themselves |
| Independent kinematic oracle (OP-05, OP-06, GC-09) | The A-axis mapping is right, not only consistent: direction, offset and wrap-around agree with a machine model that shares no code with the engine | A small test-only module (`openchuck-testkit`) that maps XYZA back to the tool tip in model space from first principles for rotation about X, written separately from the shared machine model. The path library, the post and the G-code reader all use that shared model, so without this check a replay could agree with a wrong model. Used for round-trip tests and for the replay in the post-processor round trip |
| Determinism (NFR-06, GC-07) | Identical toolpaths and byte-identical G-code for any thread count and across repeated runs | Run the same project with 1, 2 and N threads and compare hashes |
| Golden files (NFR-08) | G-code from the Z1 post and the verification post, setup sheet content | Snapshot tests on small hand-reviewed parts, so every change shows as a diff |
| Post-processor round trip | The G-code means what the IR meant | `openchuck-gcode` parses the output, replays it through the machine kinematics and compares positions, A travel, feeds and limits with the IR (also the basis for GC-08) |
| Simulator replay (GC-10) | Toolpaths and posting confirmed by a second implementation | Verification post output run in the LinuxCNC simulator; compare reached positions and limits. This validates the path and the post, not the Z1 controller, so output still needs proving on the machine, starting with air cuts (NFR-12) |
| Simulation result (SIM-05) | Machined stock matches the target within tolerance | Run the simulator over the analytic test parts and assert the reported deviation against the agreement limits in the verification plan (within one simulation cell at the chosen resolution, and no worse than 0.02 mm at the finest setting the user may choose); SP-6 set the baseline (decision 0022) |
| Project files (PRJ-01 to PRJ-06) | Stable, diffable, migratable | Save twice gives identical bytes; load then save is a no-op; fixtures for every older schema version migrate; a test branches a project in Git, edits different operations on each side, and merges without conflict; model hash change is detected |
| Importers (NFR-07) | Damaged or hostile input gives an error, not a crash | Corpus tests and fuzzing for STL, STEP, DXF and SVG, and for every other file the user can supply: project files, tool library import, custom tool-profile text files (TL-03), machine profiles and post definitions; IMP-04 checks tested on meshes with known defects; same STEP file imports with identical face and edge IDs |
| Selection references (PRJ-06, IMP-07) | References survive save and load; P2 re-matching after a changed model | Round-trip tests now; re-import scenarios when P2 lands |
| Application and CLI | End-to-end behaviour with no window | `openchuck-cli` scenarios: project in, G-code and report out; undo and redo; cancellation mid-job; NFR-12 notice present |
| Library boundary (INT-01 to INT-08) | The library works for a host that is not us | A small example host built and run in CI using only the public API; doctests; a semver compatibility check on the public API; the dependency check from section 4 |
| Platforms (NFR-02, NFR-13) | Builds and tests pass on Windows, macOS and Linux, including the C++ kernel | CI matrix on all three |
| Performance (NFR-03, NFR-04) | 30 fps or better on a model of 1 million triangles; multi-core scaling; UI stays responsive | Benchmarks on the reference hardware defined in the verification plan (4 cores, 16 GB RAM, integrated graphics, one physical machine per platform), a stress model, and a scripted viewport frame-time check: 95th percentile frame time of 33 ms or less over a 60-second camera path on a 1 million triangle model. This part is partly manual, because hosted CI typically has no real GPU, so it runs a software-rendered smoke test and uses native rendering wherever a runner offers it |
| Dependency health (NFR-10, NFR-14) | Licences compatible, advisories clear, no network crates, new crates justified | `cargo deny` and an advisory check in CI; each new dependency recorded with its survey-style score and justification |
| UI | Layout and interaction | Thin view-model tests and a few screenshot or manual checks; logic lives below the UI so little is left to test here |

Also in CI: `clippy` and `rustfmt`, coverage tracking, and `criterion`-style benchmarks on the hot paths in pathing and simulation.

## 10. Requirements coverage

Every requirement ID in the requirements doc maps to a component here; the verification column refers to the levels in section 9.

| Requirements | Component | Verified by |
| --- | --- | --- |
| IMP-01 to IMP-03, IMP-06 | `openchuck-import` | Importer corpus and fuzzing; deterministic ID test; unit conversion tests |
| IMP-04, IMP-05 | `openchuck-geom` | Meshes with known defects; transform tests |
| IMP-07, PRJ-07 (P2) | Descriptor fields in selection references, matching in `openchuck-app` | Re-import scenarios when built |
| IMP-08 (P2) | `openchuck-import` curve adapters | Corpus tests |
| UI-01 to UI-03, UI-06 to UI-08 | `openchuck-ui`, `openchuck-viewport`, `openchuck-app` commands and undo | View-model tests; CLI scenarios for undo and redo; platform CI |
| UI-04, UI-05 | `openchuck-geom` selection algorithms, selection sets in `openchuck-project` | Selection and picking tests; reference round trips |
| UI-09, NFR-04 | Job manager in `openchuck-app` | Cancellation and progress scenarios; responsiveness checks |
| SET-01 to SET-07 | Machine profile and stock in `openchuck-types`, setups in `openchuck-project` | Schema and persistence tests; profile-driven strategy tests (X now, Y under SET-02) |
| TL-01 to TL-11 | Tool and material libraries in `openchuck-project`, assembly types in `openchuck-types` | Persistence and snapshot tests; import fixtures for TL-10 |
| OP-01, OP-02, OP-11, OP-16 | `openchuck-path` strategy trait, registry and parameter schemas; operations in `openchuck-project` | Schema-driven parameter tests; regeneration tests |
| OP-03 to OP-10, OP-17 | `openchuck-path` strategies | Analytic oracles; independent gouge and coverage check |
| OP-12, OP-14, OP-15 | `openchuck-path` shared services | Property tests; independent gouge checker; A-axis limit tests |
| OP-13 | IR contact feed in `openchuck-types`, feed modes in `openchuck-post` | Feed calculation tests; round trip through `openchuck-gcode` |
| OP-18 (P3) | A feature recognition strategy module over `openchuck-geom` and B-rep data | Reference parts with known features |
| SIM-01 to SIM-07 | `openchuck-sim` and `openchuck-viewport` | Simulation deviation on analytic parts; playback jump tests; viewport benchmark |
| SIM-08 to SIM-10 | `openchuck-sim` timing and engagement, `openchuck-gcode` for SIM-09 | Timing against hand-computed paths; replay tests |
| GC-01 | IR in `openchuck-types` | Serialisation round trips |
| GC-02 to GC-07 | `openchuck-post` | Golden files; determinism tests |
| GC-08 | `openchuck-gcode` checks | Seeded faults (limit violation, missing feed, bad number) are caught |
| GC-09, GC-10 | Post definitions in `openchuck-post` | Golden files; LinuxCNC simulator replay; air cuts on the Z1 |
| GC-11 | `openchuck-cli` | CLI scenarios |
| GC-12 | Setup sheet module in `openchuck-app` | Snapshot of generated content |
| GC-13 (P2) | Scripting implementation of the `PostProcessor` trait | Same golden files as templates |
| PRJ-01 to PRJ-06, PRJ-08 | `openchuck-project` | Project file tests (section 9) |
| COL-01 to COL-08 | `openchuck-sim` collision module, `CollisionOracle` port | Scenarios with known colliding and clear setups |
| VC-01 to VC-06 | `openchuck-vcs`, semantic diff in `openchuck-project`, provenance passed to `openchuck-post` | Temporary-repository integration tests; merge test; header test |
| INT-01 to INT-09 | Library family: `openchuck-types`, `openchuck-path`, `openchuck-post`, with binding crates later | Dependency check; example host; semver check; licence check |
| NFR-01, NFR-08 | Crate layering and test strategy | CI dependency check; golden and regression tests |
| NFR-02, NFR-03, NFR-13 | wgpu viewport; STEP kernel wrapper | Platform CI matrix; viewport benchmark |
| NFR-05, NFR-06 | `openchuck-types` precision; determinism rules (section 8) | Thread-count tests |
| NFR-07 | Error handling rules (section 8) | Fuzzing; corpus tests |
| NFR-09, NFR-10, NFR-14 | Dependency rules (section 4) and selection table (section 7) | `cargo deny`; network-crate check; recorded justifications |
| NFR-11 (P2), NFR-12 | Documentation; safety notice in UI, CLI and post | Notice present in export tests |

## 11. Suggested build order

This follows the requirement priorities and the survey's advice to spike each layer first. It is reconciled with the Phase 1 work breakdown, which lists all thirteen spikes (SP-1 to SP-13) and the slice order.

1. **Guardrails.** Workspace, CI on all three platforms, dependency and licence checks, the `openchuck-types` data and IR, an application API with a CLI that runs end to end on nothing, and the test-only kinematic oracle for rotation about X (section 9), which must exist before any G-code with A words is accepted as a golden file.
2. **Spikes (short, throwaway, each with a written result; the five below run first, and the work breakdown lists all thirteen).**
   - STEP to tessellated mesh to an orbitable window, which also proves the STEP kernel route builds on all platforms and exposes the topology and surface data IMP-02 needs. SP-1 settled the route as a thin in-house wrapper over OpenCASCADE (decision 0003).
   - A wrapped engraving path on a cylinder to G-code with A words, replayed through `openchuck-gcode` and in LinuxCNC, and checked against the kinematic oracle.
   - A `parry3d-f64` ray-cast sweep around the A axis on a real part, to test whether continuous 4-axis is feasible.
   - A G-code reader for A words and the LinuxCNC dialect. SP-5 settled it as an in-house reader for the codes our posts emit (decision 0005).
   - A simulation stock representation compared on accuracy, speed and memory. SP-6 settled it as dexels (decision 0022).
3. **Thin P1 slice.** STL import, project file, one wrap-family strategy, simulator showing deviation, Z1 and verification posts, golden files, viewport.
4. **P1 breadth.** STEP with selection by face and edge, remaining strategies (OP-03 to OP-15), tool library and snapshots, setup sheet, undo, full project-file guarantees.
5. **P2.** Collision detection, in-app version control, Y-axis rotary, multiple setups, re-import matching, curve import, adaptive roughing, scripting, Python bindings, CLI polish.
6. **P3.** G-code simulation in the UI, collision avoidance in linking, feature recognition, feed optimisation, tool table import, C interface.

## 12. Repository structure

**Decision: one monorepo.** The whole project lives in a single Git repository containing one Cargo workspace. The library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`) is kept separable inside it, so it can be split out later if an outside project needs it, without code changes.

**Why a monorepo now**

- Before 1.0 the shared types and the library APIs change together. One commit can change a port in `openchuck-path`, its implementation in `openchuck-sim` and the callers in `openchuck-app`. Across several repositories the same change needs linked pull requests, version bumps and temporary patch overrides.
- The verification approach (section 9) runs crates against each other: analytic oracles, golden G-code, replay through `openchuck-gcode` and the LinuxCNC simulator. One repository gives one CI run and one copy of the test data.
- One toolchain, one lock file, one dependency and licence policy and one issue tracker suit a small team.
- Reuse is not lost. The dependency rules in section 4 already make the libraries independent of the application, so the monorepo only decides where the code is stored.

**Why not several repositories (or submodules) now.** Separate repositories for the library family and the application would add release coordination before the APIs are stable. Git submodules add a second place to look for every change and are easy to leave out of sync. Neither buys anything until a second consumer exists.

**Keeping the libraries separable**

- The library family may depend only on its own members and third-party crates, never on `openchuck-geom`, `openchuck-sim`, `openchuck-app` or any front end. A check in `xtask`, reading `cargo metadata`, fails CI if that rule is broken.
- Each library builds and tests on its own (`cargo test -p openchuck-path`) without the C++ toolchain, GPU libraries or the other application crates. Only `openchuck-import` needs the STEP kernel.
- Each library has its own README, `license` field, changelog, rustdoc and examples, and its internal path dependencies carry version numbers so `cargo publish` works unchanged.
- **Triggers for splitting:** an outside project such as FreeCAD needs a release cadence independent of the application; a licence exception or dual licence (section 13) is easier with a separate repository; or outside contributors want the library without the application.
- **How to split:** `git filter-repo` on the library paths keeps their history. The application then depends on the published crates instead of path dependencies. The libraries need no code changes.

**Layout**

| Path | Contents |
| --- | --- |
| `crates/` | All library crates: `openchuck-types`, `-path`, `-post`, `-gcode`, `-geom`, `-import`, `-sim`, `-project`, `-vcs`, `-app`, `-viewport`, and the test-only `openchuck-testkit`. |
| `apps/` | The binaries `openchuck-ui` (egui front end) and `openchuck-cli`. Wiring only, no CAM logic. |
| `bindings/` | Later: `openchuck-path-py` and `openchuck-path-capi`. Not in the first release. |
| `posts/` | Built-in post definitions as data files: the Makera Z1 post and the LinuxCNC verification post, each with golden tests. |
| `testdata/` | Verification corpus: small STEP and STL models, analytic-oracle cases, golden G-code, expected toolpaths. A README records where each model came from and its licence. |
| `fuzz/` and `benches/` | Fuzz targets for the importers and the G-code reader (a separate workspace, because cargo-fuzz needs a nightly toolchain); cross-crate benchmarks in the `openchuck-benches` package. Per-crate benchmarks stay inside their crate. |
| `docs/` | Requirements, architecture, work breakdown, verification plan and decision records. |
| `xtask/` | Repository automation written in Rust: the dependency-rule check, SPDX and licence header check, traceability report, LinuxCNC replay runner and release steps. |
| Repository root | Workspace `Cargo.toml`, committed `Cargo.lock` (the project ships binaries), `rust-toolchain.toml`, `deny.toml`, `LICENSES/` and `.github/` workflows. |

**Conventions**

- **Dependencies:** every third-party version is declared once in `[workspace.dependencies]` and inherited by the crates. That is the one place to review for maintenance and licence (section 7). `cargo-deny` runs in CI.
- **Licences:** each crate sets its own `license` field and files carry SPDX identifiers. Every crate, library family and application, is licensed `AGPL-3.0-or-later`; the STEP kernel stays isolated in `openchuck-import`. Contributors sign the contributor licence agreement in `CLA.md` (or `CLA-ENTITY.md` for an organisation), which allows a later licence exception or dual licence (section 13).
- **Versioning:** before 1.0 the four library crates share one version number and are released together. The application is tagged separately. Release tooling is described below.
- **Branching:** trunk-based. `main` is always releasable, branches are short-lived and merge only with CI green.
- **CI:** jobs are path-filtered, so a change to `docs/` or `posts/` does not rebuild the STEP kernel. The library family builds on Linux, Windows and macOS without the C++ toolchain. The OpenCASCADE build is confined to `openchuck-import` and cached. Its static libraries are linked with `TKernel` first, and a clean process exit is tested on every platform (decision 0003).
- **Test data size:** the corpus stays small. Shapes are generated procedurally where possible and golden files are plain text so reviews show real diffs. Large models, if ever needed, go to Git LFS or a separate test-data repository (section 13).
- Tests and traceability: every test names the requirement IDs it covers, in its test name or an attribute (for example req\_op\_05\_roundtrip). An xtask command builds a traceability report from the test run and fails CI when a P1 requirement has no test. The verification plan lists the method and acceptance limit for each requirement.

**Release tooling.** The crate survey now scores these tools, out of 100: `cargo-release` 79, `release-plz` 73, `cargo-dist` 70 and `git-cliff` 80 (changelogs only). They are tools people run, so adoption scores are low for all of them and the ranking mostly reflects project health. The picks below stand, and the scores answer the NFR-14 check that this paragraph used to leave open.

- **Library family: `cargo-release`.** It is a local command that bumps versions, updates the changelog, tags and publishes, so a maintainer stays in control before 1.0. It supports workspaces and a shared version across crates, which matches the lockstep rule above. It belongs to the `crate-ci` organisation, which also hosts widely used Rust tooling.
- **Release gate:** the `xtask` runs the dependency-rule check, `cargo-deny`, the golden G-code comparison and the LinuxCNC replay before a tag is cut. A release with a failing gate is not published. The survey suggests `cargo_metadata` (89) for the dependency-rule check, `cargo-hack` (77) to build and test each library on its own, and `cargo-about` (79) to produce the licence notices shipped with binaries (NFR-10).
- **Application binaries: a GitHub Actions build matrix.** A workflow triggered by an application tag builds Linux, Windows and macOS and attaches archives to the release. `cargo-dist` (70, now branded `dist`) is a candidate to generate this workflow and installers. The survey found it active: 6 releases in 12 months on crates.io, and its repository changelog is a version ahead of the crates.io release. The egui app will probably still need platform packaging (a Windows installer, a macOS app bundle) that it does not fully cover. Decide after the first manual releases.
- **Later option: `release-plz`.** It automates the cycle with a release pull request, changelog, tag and publish. It depends on conventional-commit discipline and becomes more useful once there are outside contributors. Revisit it then, and check how it handles a shared library version.
- **Names and tags:** reserve the `openchuck-*` names on crates.io early. Library releases and application releases use separate tag schemes so they never collide.

## 13. Open decisions, risks and changes from the first draft

**Open decisions and risks**

- **STEP kernel fitness.** Decided by SP-1 ([decision 0003](decisions/0003-step-kernel-route.md)): a thin in-house wrapper over OpenCASCADE, with STEP parsing in an isolated child process because the reader crashes on damaged input. `cadrum` is rejected; `brepkit-io` (60) is watched and re-spiked when it reads full spheres and shell-only files. Remaining risk: the route is proven only on files written by OpenCASCADE itself, so exports from other CAD tools must be tested before slice 11's import work is accepted.
- **Exact surfaces in the library (decided, [decision 0017](decisions/0017-sp-9-exact-surfaces.md)).** INT-02 requires B-rep faces and edges as plain data. The face record carries the surface (analytic frame and size, or a clamped, unwrapped rational B-spline patch), the face orientation, UV bounds, trim loops as UV polylines and the tessellation with a face index per triangle; the shim converts every other kernel surface to the B-spline form. Analytic surfaces and B-spline patches are evaluated in pure Rust inside `openchuck-path` (point and normal, closest point, trimmed ray hits, trim test), so the library never touches the kernel; spike SP-9 matched the kernel to 1e-14 mm on analytic faces. `curvo` was tried and not adopted. The application can still supply a kernel-backed `SurfaceSource`, which is what a FreeCAD-style host would do. Open: robust closest point at singular points, trims across a periodic seam, cross-platform bit identity (OI-72 to OI-76).
- **Stability of geometry references.** Deterministic face numbering is enough for the same file (PRJ-06). Matching after a re-export (IMP-07, PRJ-07) is the hard part and needs its own prototype.
- **Simulation representation.** Decided by SP-6 ([decision 0022](decisions/0022-sp-6-simulation-stock.md)): dexels, voxel as the fallback. A collision query against in-process stock (COL-05) is the same span test as a cut, without the subtraction, on the dexel columns, with the holder as a second revolved profile. The finest setting (0.025 mm) and the span solver speed-up were settled in slice 7 ([decision 0025](decisions/0025-slice-7-simulation.md); OI-100 and OI-101 closed).
- **Library licensing.** Decided: every crate is `AGPL-3.0-or-later` (INT-09, NFR-10), including `openchuck-types` and `openchuck-post`. Decided: contributors sign a contributor licence agreement adapted from the Apache ICLA (`CLA.md`, `CLA-ENTITY.md`) that allows a later exception or dual licence, with a commitment that contributions stay available under an open source licence (decision record 0004). Dependencies bring their own licences: the brepkit crates are AGPL-3.0-only, cbindgen and uniffi are MPL-2.0 and OpenCASCADE is LGPL-2.1 (accepted in decision 0003); check the others against INT-09 and NFR-10 before adopting.
- **G93 on the Z1.** Inverse-time feed is unsupported on the Z1 (confirmed by the SP-12 air cut, decision 0007); the IR and post support both modes either way, and the Z1 post uses G94 with the `makera` feed rule.
- **API churn.** parry3d, wgpu, egui and i\_overlay change quickly. Exact version pins and a planned upgrade cadence are needed. The supporting crates add more: minijinja reached 3.0 on 8 Oct 2026, pyo3, toml\_edit and egui\_tiles are pre-1.0, and notify 9.0 is at release candidate.
- **Second-tier dependencies.** The survey now scores the template engine (`minijinja` 94), scripting engine (`rhai` 90, to be spiked), serialisation (`serde`, `toml_edit`), setup-sheet PDF (`printpdf` 90), Python (`pyo3` 99) and C bindings (`cbindgen` 84), and the error, logging, test and release tools (section 7). Each pick still needs its NFR-14 justification recorded and a short spike. Order keys, units, undo and NURBS evaluation have no crate worth adopting, so they stay in-house unless a spike shows otherwise. Scores are a screen: nothing was compiled and the Features score is judgment. Build cost is also untested (NFR-13): git2 and mlua compile C code, typst is large, and cargo-fuzz needs a nightly toolchain.
- **Order key scheme.** The exact scheme for PRJ-05 order keys (so reordering and merging stay clean) should be prototyped with a real Git merge. The survey found no crate worth adopting: fractional\_index (64) is the only one and serves as a reference.
- **Model files in Git.** Binary STEP and STL versions grow the repository (requirements section 4.3). PRJ-02 keeps models separate so a different handling can be added later.
- **Repository details.** Section 12 settles on a monorepo. Decided: the application crates are AGPL, and the authoritative copy of these documents lives in the repository under `docs/` (the Claude artifacts become working drafts). Still open: whether the test corpus needs Git LFS or a separate test-data repository; and the packaging route for the application binaries (plain build matrix or cargo-dist, section 12).

**Changes from the first draft**

- Requirement IDs now drive the structure and the coverage table; the first draft had no ID list to work from.
- Machine profile, tool, stock and the neutral toolpath IR moved into a shared plain-data crate (`openchuck-types`), because INT-02 requires the library to take them as plain data. The former `openchuck-core` became `openchuck-project`.
- Post-processing became its own reusable library (`openchuck-post`), since INT-03 requires it to work without the rest of the application.
- Added `openchuck-gcode` (G-code reading) for GC-08, SIM-09 and as an independent check on posts.
- Added the `StockModel` and `CollisionOracle` ports so strategies can use in-process stock and collision queries (OP-03, OP-09, COL-08) without depending on the simulator.
- Revision 5 (14 Oct 2026): the stock representation is decided (dexels, voxel as fallback; decision 0022); section 5 and section 13 updated.
- Revision 5, later the same day (decision 0025, slice 7): the finest simulation cell (0.025 mm) and the sampling of moves through the shared kinematics (section 5.5); the span solver speed-up settled (section 13).
- Added the contact-feed-plus-duration design for OP-13, and the rule that machine conventions live only in post definitions (GC-04).
- Selection references, project file layout, caching and the operation pipeline made explicit (PRJ, UI-05).
- The CLI is listed as a front end from the start because GC-11 is P1 and it is the end-to-end test harness.
- Added crate selection from the survey (section 7), a determinism rule for parallel code, and a verification table keyed to requirement IDs.
- Reviewed the crate needs against the survey's later sections: section 7 gained a supporting-layers table, the git2 score (96, ahead of gix at 79) and the licence flags; section 12's release tools now carry scores; and the open decisions were updated for brepkit-io as a STEP option, NURBS evaluation and the second-tier dependencies.
- Re-checked every score and pick against the survey as updated on 10 Oct 2026: all scores match. Added the survey's remaining cautions: the licences to allow on purpose (CC0-1.0, NCSA), the pre-1.0 and release-candidate crates, rhai's agent-instruction file, minijinja 3.0, the C-compiler build cost of git2, and the setup sheet's crate (`openchuck-app`).
- SP-5 (decision record 0005): the G-code reading row of section 7 names an in-house reader in place of `gcode`.
- SP-3 (decision record 0001, accepted): section 5.1's IR field list gains the contact path length, which slice 0 built as `contact_length`.
- Slice 0 (decision record 0004): added the test-only `openchuck-testkit` and the `openchuck-benches` package to sections 3 and 12, fixed the licence identifier as `AGPL-3.0-or-later`, and recorded the contributor licence agreement.
- Verification review: added an independent kinematic oracle to the verification table (it covers the case where the path library, post and G-code reader share one wrong machine model), the acceptance limits and reference hardware from the verification plan to the simulation and performance rows, and the requirement-ID test convention and traceability report to the repository section.
- Revision 4 (diagrams only, no design change): added Figure 1, the static structure of the crates (section 3), and Figure 2, the dataflow of the operation pipeline (section 6). Both are SVG files in `docs/diagrams/`, drawn from sections 3, 4 and 6, and follow the viewer's light or dark setting.
- Slice 4 (decision record 0014, agreed 12 Oct 2026): section 5.6 describes the tool library as a folder of tool files with machine tool sets (TL-11), tool numbers belonging to the project; section 5.3 gains the tool-number confirmation before posting (GC-14), which the application layer asks and the post receives as data.
