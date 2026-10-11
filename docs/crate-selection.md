# Rust crates for a CAM / 4th-axis project

Oct 10, 2026 · @Paul H

Rust has usable crates for every layer of a CAM tool except 4th-axis toolpath planning, which you would write yourself; STEP import is the other soft spot, with no candidate scoring above 66 out of 100 on long-term health.

Added Oct 10, 2026 after a review of the architecture document: every supporting need it lists (project files, post templates and scripting, setup sheets, Git, NURBS, bindings, error handling, test and CI tooling, CLI and platform services) now has a scored shortlist, and the STEP section gains brepkit, a young pure-Rust kernel the first pass missed.

## Recommended starting stack

This is my judgment from the findings below, not a tested design: use OpenCASCADE bindings for STEP, parry3d for geometry queries, cavalier\_contours for 2D offsets, egui with a custom wgpu viewport for the UI, and write the G-code and 4th-axis layers yourself. The new scores (out of 100, explained in the next section) support the same picks; the only change is that Tauri now ranks ahead of iced as the UI fallback.

| Layer | First pick (score) | Fallback (score) | Why |
| --- | --- | --- | --- |
| STEP import | opencascade (66) or cadrum (63) | truck-stepio (56) or monstertruck (53) if you need pure Rust | Only route backed by a mature kernel; the pure-Rust options are stale or new, and no STEP crate scores above 66 |
| STL import | stl\_io (88) | mesh-loader (57) | Maintained, small, reads both formats, about 5 million downloads |
| Geometry queries | parry3d-f64 (83, scored as parry3d) | Your own BVH | Ray casts, distance queries and meshes in f64; same repo and releases as parry3d |
| 2D offsets and booleans | cavalier\_contours (82) plus i\_overlay (80) | clipper2 (73) | Pure Rust; i\_overlay is the boolean engine inside geo, though it has had five major versions in five months |
| Viewer and UI | eframe/egui (90) with an egui\_wgpu viewport (87) | Tauri with Three.js (89), or iced (75) | You control the render loop for large toolpaths |
| G-code | Your own emitter; gcode (75) to read files back | g-code (73) | You need A-axis words and per-controller dialects |
| 4th-axis planning | Write it; start with wrap/unroll | Continuous 4-axis later | No crate exists |

Supporting layers added after the architecture review (judgment from the scores, which are out of 100 and sit in the sections further down):

| Need | First pick (score) | Fallback (score) | Why |
| --- | --- | --- | --- |
| Project files | serde (93) with toml (92); toml\_edit (93) for hand-edited files (decision 0006) | serde\_json (97), ron (85) | Stable, ordered output under our control; serde\_yaml is archived |
| IDs and content hashes | uuid (99), blake3 (93) | sha2 (85) | Order keys: write them in-house; fractional\_index (64) is the only crate and is a reference |
| Post templates | minijinja (94) | handlebars (93), tera (90) | Runtime templates users can edit; askama (85) compiles them in |
| Post scripting (P2) | Spike rhai (90) | starlark (84), mlua (94) | Fit over rank: rhai is pure Rust, starlark is built to be deterministic, mlua needs a C compiler |
| Setup sheet | HTML from the template engine; printpdf (90) when a PDF is required | typst (88) | typst is heavy; genpdf (53) is abandoned |
| Embedded Git | git2 (96) | gix (79) | gix's own status file lists checkout, merge and restore as not done |
| NURBS evaluation | The kernel's own; analytic surfaces in-house | curvo (71) | No crate is proven for point, normal and closest-point queries; brepkit-math (66) is AGPL |
| Python and C bindings | pyo3 (99) with maturin (82); cbindgen (84) | uniffi (81), diplomat (82) | Standard routes |
| Errors and logging | thiserror (94) in libraries; anyhow (96) and tracing (89) in apps | miette (75), log (95) | Libraries log nothing |
| Tests, fuzzing, benchmarks | proptest (100), insta (94), assert\_cmd (88), cargo-fuzz (82), criterion (88) | quickcheck (85), divan (72) | Match the architecture's verification levels |
| CI tooling | cargo-deny (82), cargo-llvm-cov (83), cargo-semver-checks (78), cargo-nextest (80) | cargo-audit (79) | Dev-only; low adoption scores reflect that they are tools |
| Release and repository tooling | cargo-release (79), cargo\_metadata (89), cargo-hack (77), cargo-about (79) | release-plz (73), git-cliff (80), cargo-dist (70) | Manual lockstep releases first; cargo-vet (60) is optional process |
| CLI, jobs, files | clap (97), indicatif (92), crossbeam-channel (92), notify (92), dirs (87) | argh (82) | Boring and widely used |
| UI helpers | bytemuck (96), egui\_plot (83), egui\_tiles (81) | egui\_dock (76) | egui\_dock has agent-instruction files |
| Units and undo | In-house newtypes and command stack | uom (84) | The undo crates have almost no users |

Build one thin vertical slice first:

1. STEP file to tessellated mesh to a window you can orbit.
2. A wrapped engraving path on a cylinder to G-code with A words, replayed in your own viewer.
3. A parry3d ray-cast sweep around the A axis on a real part, to see whether continuous 4-axis is feasible in your time budget.

## How the crates are scored

Each crate is scored 1 to 5 on six criteria that favour long-term health, then weighted into a total out of 100. Scores are as of Oct 10, 2026, and the measured facts behind them are in the evidence table near the end.

| Criterion | Weight | How it was scored |
| --- | --- | --- |
| Maintenance | 20 | From crates.io release dates. 5 = latest release within 3 months and at least 3 releases in the last 12; 4 = within 8 months; 3 = within 14; 2 = within 30; 1 = older |
| Maturity | 15 | Years since the first crates.io release. 5 = 8 or more; 4 = 5 or more; 3 = 3 or more; 2 = 1.5 or more; 1 = younger |
| Features | 20 | My judgment of how fully the crate does its job for this project, from READMEs and docs. 5 = complete and broad; 3 = partial; 1 = minimal. Nothing was run |
| Quality | 20 | Mean of three 1 to 5 scores. Docs: share of doc-comment lines in the package source, plus 1 if it ships examples. Tests: test functions per 1,000 lines. CI: a workflow found in the repository, plus 1 for a second workflow, dependabot, renovate or coverage |
| Adoption | 15 | Mean of two 1 to 5 bands: dependent crates on crates.io (1,000 or more = 5; 100 = 4; 30 = 3; 5 = 2) and downloads in the last 90 days (5 million = 5; 1 million = 4; 200,000 = 3; 30,000 = 2) |
| AI risk | 10 | Higher is safer. 5 = no agent-instruction files found; 4 = such files in a project 5 or more years old; 3 = in one 1.5 to 5 years old; 2 = in one younger than 1.5 years; 1 = younger than 1.5 years with 8 or more releases in 12 months |

Total = the sum of weight × score, divided by 5, so a crate scoring 5 on everything gets 100. The script that computed the totals is the only place arithmetic was done, so the matrices below can be rechecked from the evidence table.

What the scores cannot tell you:

- **Maintenance is release cadence only.** GitHub's API was blocked from my workspace, so commit activity, contributor counts, stars and issue response times are not in the scores.
- **AI risk sees only visible markers.** It looks for agent-instruction files (CLAUDE.md, AGENTS.md and similar) in the repository or package, plus halfedge's package exclude list, which names AI-tool directories. It cannot measure how much code an AI wrote, a file in an old project such as wgpu may only reflect contributors' tooling, and a crate with no file is not proven human-written.
- **Test counts are partial.** Only test attributes inside the published package are counted. For glam, bevy, wgpu, tauri, iced, egui, egui\_wgpu, usvg and slint the tests live in the wider repository or in macros, so I set their test score to 4 rather than 5 because I could not verify it; I also set bevy's docs score to 4. Those crates show low test densities in the evidence table for that reason.
- **CI evidence is uneven.** I could not locate a workflow for g-code (a coverage badge only) or truck-stepio, so both got 3; ngc and nom\_stl show only Travis or CircleCI badges, so both got 2.
- **Adoption counts dependents, not their maturity.** I read the top dependents of the smaller crates and name the recognisable ones in each section.
- **The scores rank health and breadth, not fit.** A high-scoring crate can still be the wrong tool, so each section keeps its own judgment on fit.

I re-ran the totals with equal weights and with feature-heavy, maintenance-heavy, adoption-heavy and no-AI-factor weightings. The winners of the UI, STL and math sections never changed. STEP stays with opencascade unless AI risk is ignored, when cadrum edges ahead. Visualisation, G-code and the core 2D crates swap places between their top two.

The supporting-crate sections (added Oct 10, 2026) use the same rubric and the same evidence sources, with these extra conventions:

- **Test scores set by hand.** Where the tests live in sub-crates, a workspace test suite or macros, the measured density was unrepresentative, so I raised the test score to 4 and never to 5. This applies to cargo-audit, cargo-deny, cargo-nextest, cbindgen, clap, egui\_dock, egui\_extras, egui\_plot, gix, krilla, minijinja, rhai, serde, sha2, toml, toml\_edit, tracing and typst.
- **Docs scores set by hand.** For command-line tools and facade crates the doc-comment share of the package is not meaningful: cargo-audit, cargo-deny, cargo-fuzz, cargo-llvm-cov, cbindgen, diplomat and sha2 are set to 3, and tracing and typst to 4.
- **CI evidence.** Probes looked for 26 common workflow file names. cbindgen, uniffi, cargo-audit, sha2 and truck-geometry are active projects where none was found, so they got 3 (not located). dirs, genpdf, splines and undo\_2 are hosted outside GitHub and were not probed, so they got 2. bpaf, json5, lexorank, meshopt, slotmap and uom had none found and got 1; any of them may use a file name I did not try.
- **Tools are undercounted on adoption.** The dependents measure suits libraries; command-line tools such as the cargo-\* utilities, maturin and mdbook are used by running them, so their Adoption scores understate real use.
- **Features were judged from each project's own description.** For the Git section I also read gitoxide's feature-status file, which marks checkout, merge and restore as not done.
- **Sensitivity.** Re-ranking each group of new candidates under three alternative weightings changed the leader in only two groups: typst takes the PDF group when features count for more, and mdbook gives way to cargo-llvm-cov in the CI tooling group under two of the three. Every other group kept its leader.

## STEP and B-rep kernels

OpenCASCADE bindings are the only route to a mature STEP reader today, and even they score only 66 out of 100; the pure-Rust options are younger or stale.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [opencascade](https://github.com/bschwind/opencascade-rs) | Rust wrapper over the OpenCASCADE kernel via cxx; STEP and STL import/export are stated goals, and it ships a viewer that opens STEP files | 0.3.0 published Aug 24, 2026 (previous release 0.2.0 in 2023) | Needs CMake and a C++ compiler; LGPL-2.1; README calls it a major work in progress; no other crate on crates.io depends on it |
| [cadrum](https://docs.rs/cadrum) | OpenCASCADE statically linked and headless; STEP, BRep and STL read/write, booleans, fillet, chamfer, loft, sweep, mass properties | 0.9.0, published Oct 10, 2026; MIT | Single maintainer and 41 releases since Mar 29, 2026, so the API is still moving; its repository carries CLAUDE.md and AGENTS.md; bundles an OpenCASCADE 8.0.0 beta according to its docs |
| [brepkit-io](https://crates.io/crates/brepkit-io) | Pure-Rust solid-modelling kernel (math, topology, algorithms and I/O layers) whose I/O crate reads and writes STEP with analytic surfaces and NURBS kept, plus STL, 3MF, OBJ and PLY. Added after the architecture review; the first survey missed it | 4.1.48, published Oct 9, 2026; first release Aug 7, 2026 with 189 releases since | AGPL-3.0-only; 0.2 years old with one dependent; CLAUDE.md in its repository; its STEP writer saves outer shells only, and IGES support is marked experimental |
| [truck-stepio](https://lib.rs/crates/truck-stepio) | Pure-Rust STEP read/write into the truck B-rep kernel, with a step-to-mesh example | 0.3.0, Sep 20, 2024 | No release in about two years |
| [monstertruck](https://docs.rs/monstertruck) | Renamed fork of truck: B-rep, NURBS, meshing, booleans, fillet, STEP back-end, OBJ/STL output | 0.4.1, published Sep 19, 2026; Apache-2.0 | First published Mar 2026 by one maintainer; its repository carries CLAUDE.md and AGENTS.md; adoption is tiny (about 460 downloads) |
| [ruststep](https://docs.rs/ruststep) | ISO 10303-21 file reader/writer plus generated types for AP201 and AP203 | 0.4.0, Sep 2024 | Parses the file only, no geometry; its README says it is experimental and not for products; bundled schemas do not list AP214 or AP242, which many CAD exports use |
| [Foxtrot](https://github.com/Formlabs/foxtrot) | STEP viewer with an EXPRESS parser, generated AP214 parser, triangulation and NURBS helpers, wgpu GUI | n/a | Its README calls it a proof-of-concept, not an industrial-strength kernel; not scored because it is a viewer, not a library |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| opencascade | 4 | 3 | 4 | 3 | 1 | 5 | **66** |
| cadrum | 5 | 1 | 4 | 4.7 | 1 | 1 | **63** |
| brepkit-io | 5 | 1 | 3 | 5 | 1 | 1 | **60** |
| truck-stepio | 2 | 3 | 3 | 3 | 1.5 | 5 | **56** |
| ruststep | 2 | 4 | 2 | 3.3 | 1.5 | 5 | **56** |
| monstertruck | 5 | 1 | 3 | 3.3 | 1 | 1 | **53** |

opencascade leads because it is older (3.7 years) and has no agent-instruction files. cadrum is better tested and documented (Quality 4.7) but loses on maturity (0.5 years) and AI risk (a CLAUDE.md, an AGENTS.md and 41 releases in six months). Adoption is 1 for both OpenCASCADE wrappers: no crate depends on opencascade and one depends on cadrum. truck-stepio and ruststep tie at 56 on age and a clean history, but neither has been released since Sep 2024.

brepkit-io (60) slots in third. It has the best Quality score of the group (5: 14 test functions per 1,000 lines, a CI workflow, dependabot, a changelog and a security policy), but it is 0.2 years old and carries a CLAUDE.md, so Maturity and AI risk both score 1; with no agent files its total would be 68. Its AGPL-3.0-only licence matches the project's AGPL library start (INT-09) but would rule out a later closed-source or dual-licensed host, and its STEP writer serialises outer shells only. It is worth a spike behind `ModelImporter` as the pure-Rust fallback, not a first choice.

Fornjot, the best-known Rust B-rep kernel, was [archived on Jun 19, 2026](https://github.com/hannobraun/fornjot), so it is not a base to build on.

For CAM, tessellating the STEP faces is not enough on its own if you want exact tool contact on curved surfaces; a kernel that can evaluate the NURBS surfaces (OpenCASCADE, truck) keeps that option open.

## STL and mesh handling

STL reading is a solved problem in Rust, and stl\_io is the strongest crate in this survey for it (88 out of 100); mesh repair and processing crates exist but are young.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [stl\_io](https://crates.io/crates/stl_io) | Reads binary and ASCII STL, writes binary STL | 0.11.0, published Mar 15, 2026; MIT; about 5 million downloads and 42 dependent crates | Writes binary only |
| [nom\_stl](https://lib.rs/crates/nom_stl) | Fast binary and ASCII parser; its README claims a 30 MB binary file in under 20 ms | 0.2.2, Mar 2021 | No release in over five years; its CI badge points to CircleCI |
| [mesh-loader](https://docs.rs/mesh-loader/latest/mesh_loader/stl/index.html) | STL parser that takes bytes and detects binary or ASCII | 0.1.13, Oct 2024; Apache-2.0 | Only its STL module was checked; no release in two years; its dependents are robotics tools such as urdf-viz and openrr-planner |
| [parry3d](https://parry.rs) TriMesh | Triangle mesh with a BVH, ray casts and distance queries (see the collision section) | 0.31.1, Sep 18, 2026 | Not a repair tool |
| [halfedge](https://docs.rs/crate/halfedge/latest) | Half-edge mesh with QEM decimation, booleans, ray-mesh intersection and robust predicates | 0.3.0; first published Jul 5, 2026 | About 35,000 lines and 9 releases in three months; its package excludes AI-tool directories; the boolean module was marked unstable before 1.0 when I read it at 0.2.0 |
| [csgrs](https://docs.rs/crate/csgrs/latest) | Mesh CSG using BSP trees, built on geo, spade and parry3d | 0.20.1, Jul 2025 | Meant for modelling, not toolpath accuracy; no release in 14 months; its repository carries AGENTS.md |
| [procedural\_modelling](https://www.lib.rs/crates/procedural_modelling) | Half-edge meshes, extrusion, planned booleans and tessellation | 0.5.0, Jun 23, 2026 (first published Feb 2024) | Its README warned of an early stage at 0.1.2 |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| stl\_io | 4 | 5 | 4 | 5 | 3.5 | 5 | **88** |
| parry3d | 5 | 4 | 4 | 4 | 3 | 5 | **83** |
| halfedge | 5 | 1 | 4 | 5 | 1 | 1 | **64** |
| procedural\_modelling | 4 | 2 | 2 | 3.7 | 1 | 5 | **58** |
| mesh-loader | 2 | 3 | 3 | 3 | 2 | 5 | **57** |
| csgrs | 2 | 2 | 4 | 4.3 | 1 | 3 | **56** |
| nom\_stl | 1 | 4 | 3 | 3.3 | 1 | 5 | **54** |

Only stl\_io, mesh-loader and nom\_stl compete as STL readers, and stl\_io wins on every health measure: it is 9.3 years old, has about 18 tests per 1,000 lines, three CI workflows and 42 dependents. halfedge has the longest feature list and a full Quality score, but its age, lack of dependents and AI-tool markers hold it to 64. nom\_stl is the weakest on maintenance, with no release since 2021.

STL carries only triangles, so the tessellation tolerance is baked in before your code sees it. If surface accuracy matters for finishing passes, import STEP and tessellate yourself.

## 3D visualisation

Most current options sit on wgpu, so the real choice is how much engine you want around it; wgpu, egui\_wgpu and Bevy score within two points of each other.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [wgpu](https://github.com/gfx-rs/wgpu) | Cross-platform graphics API over Vulkan, Metal, DX12 and WebGPU | 30.0.1, Aug 22, 2026 | Five major versions in 12 months (26 in Jul 2025, 30 in Jul 2026), and the crates above it must match; its repository carries CLAUDE.md and AGENTS.md |
| [egui\_wgpu](https://docs.rs/egui-wgpu) | Paint callbacks for custom wgpu drawing inside an egui app; one example project renders a B-rep viewport to an offscreen texture and blits it into egui | 0.36.2, Sep 8, 2026 | You write the camera, picking and line rendering yourself |
| [Bevy](https://bevy.org/news/) | Full engine on wgpu with ECS, picking and asset loading | 0.20.0, Oct 8, 2026 | Breaking changes most releases (0.17 to 0.20 in 12 months); heavy for a tool UI |
| [kiss3d](https://docs.rs/kiss3d) | Simple 2D/3D engine, now on wgpu, with an optional egui feature | 0.47.0, Oct 4, 2026 | Eight releases since Jan 2026; its docs say it is not designed to be feature-complete or fast; its dependents are mostly dimforge test beds and small projects |
| [three-d](https://lib.rs/crates/three-d) | OpenGL/WebGL renderer with egui support and asset loading | 0.19.0, Apr 17, 2026 | One release in the last 12 months; no tests in the published package |
| [rend3](https://docs.rs/crate/rend3/latest) | Customisable 3D renderer on wgpu | 0.3.0, Feb 2022 | No release in over four years |
| [monstertruck render crates](https://docs.rs/monstertruck) | GPU rendering of B-rep shapes from the kernel itself | 0.4.1 | Tied to that kernel; not scored separately |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| bevy | 5 | 4 | 4 | 4.3 | 4.5 | 5 | **89** |
| wgpu | 5 | 4 | 4 | 4.3 | 5 | 4 | **88** |
| egui\_wgpu | 5 | 4 | 4 | 4.3 | 4 | 5 | **87** |
| kiss3d | 5 | 5 | 3 | 3.3 | 2.5 | 5 | **78** |
| three-d | 4 | 4 | 3 | 3 | 2 | 5 | **68** |
| rend3 | 1 | 4 | 2 | 3 | 1.5 | 5 | **50** |

The top three are indistinguishable on health, so fit decides: a CAM viewer mostly draws shaded meshes plus very large polylines (toolpaths), with picking and section views. That favours owning the render loop through wgpu and egui\_wgpu rather than a game engine's scene graph, even though Bevy edges ahead on the numbers. rend3 is effectively unmaintained, and three-d is a step behind the wgpu-based options.

## Cross-platform UI

For a CAM tool with a custom 3D viewport, egui is the shortest path and also the top scorer (90 out of 100); Tauri is a close second (89) if you accept JavaScript for the viewer.

| Option | Model | Status seen | Fit for a CAM tool |
| --- | --- | --- | --- |
| [egui / eframe](https://github.com/emilk/egui) | Immediate-mode, native and web | 0.36.2, Sep 8, 2026 | Easiest place to embed a custom wgpu viewport; dense property panels are quick to build; pre-1.0 with a breaking release every two to three months |
| [iced](https://iced.rs) | Elm-style, renderer-agnostic, wgpu renderer | 0.14, Dec 7, 2025 | Cleaner state model; the 0.14 release added reactive rendering, hot reloading and headless testing; no release since, now 10 months |
| [Slint](https://slint.dev) | Declarative markup language, desktop and embedded | 1.18.1, Sep 21, 2026 | Strong widgets; embedding a custom 3D view needs more work; licensed GPL-3.0, royalty-free or commercial; its repository carries CLAUDE.md and AGENTS.md |
| [Tauri 2](https://tauri.app) | Rust backend with a webview front end | 2.12.3 stable (3.0 is in alpha); 26 releases in 12 months | Used by ivacam, CADmium and Trove, mostly with Three.js for 3D; geometry stays in Rust, rendering in JavaScript |
| Others | Dioxus, GPUI, fltk-rs, gtk-rs | not scored | One app, Frame, moved from Tauri to a native GPUI-based UI in its 0.30.0 release |

Scores for the options above (egui and eframe share one row; eframe, egui and egui\_wgpu come from the same repository):

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| egui / eframe | 5 | 4 | 4 | 4.3 | 5 | 5 | **90** |
| tauri | 5 | 4 | 4 | 4.3 | 4.5 | 5 | **89** |
| slint | 5 | 3 | 4 | 4.3 | 3.5 | 3 | **79** |
| iced | 3 | 4 | 3 | 4.7 | 3.5 | 5 | **75** |

egui has the widest adoption (1,271 dependent crates) and no agent-instruction files. iced has top documentation and CI scores but loses on maintenance because of its 10-month gap between releases. Slint is actively released, but it is the youngest of the four and has agent-instruction files, and its licence needs a decision before you commit. Dioxus, GPUI, fltk-rs and gtk-rs were not scored.

Whichever you pick, keep the geometry and toolpath code in a UI-free crate. The Rust projects found do this (CADmium and the Tauri-based Outline app both say so), and it lets you swap the front end later.

## G-code: parsing and emitting

Two maintained crates exist and score within two points of each other (gcode 75, g-code 73); there is no standalone post-processor crate, so the emit side is yours to write.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [gcode](https://crates.io/crates/gcode) | no\_std parser with diagnostics and an optional zero-allocation visitor API | 0.7.0, Apr 2, 2026; MIT or Apache-2.0; first published 2018 | Parsing only; 2 dependent crates |
| [g-code](https://github.com/sameer/g-code) | Parsing and emission, with formatting, checksums and line numbers; used by the svg2gcode project | 0.6.0, Mar 4, 2026; MIT or Apache-2.0; first published 2021 | Its README lists some commands and parameters as still to do; I could not locate its CI workflow |
| [ngc](https://lib.rs/crates/ngc) | LinuxCNC-dialect parser into an AST, with a work-in-progress evaluator | 0.2.6, Mar 2022 | Not updated in over four years; CI badge points to Travis |
| [cnccoder](https://docs.rs/cnccoder) | Writes GRBL G-code for 3-axis machines from cutting instructions, plus Camotics simulation projects | 0.2.0, Apr 2024 | 3-axis and GRBL only; no release in over two years |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| gcode | 4 | 5 | 3 | 4.7 | 1 | 5 | **75** |
| g-code | 4 | 4 | 4 | 4 | 1 | 5 | **73** |
| cnccoder | 2 | 3 | 3 | 4.3 | 1 | 5 | **59** |
| ngc | 1 | 4 | 2 | 2.3 | 1 | 5 | **46** |

Both leaders are maintained, tested and free of agent-instruction files, but neither is widely depended on (2 and 3 dependent crates), so adoption is their weak point. gcode scores higher on age and tests; g-code scores higher on features because it can emit as well as parse, and it moves ahead if you weight features more heavily. Neither advertises A-axis words, so confirm your chosen emitter can write them before relying on it.

*Update, 11 Oct 2026:* spike SP-5 tested both readers. `gcode` 0.7.0 reads A words but stores numbers as `f32` and drops a code it cannot store without a diagnostic; `g-code` 0.6.0 panics on malformed input and rejects a file ending in `%` and a newline. The project reads G-code with an in-house reader instead ([decision 0005](decisions/0005-sp-5-gcode-reader.md)). The scores above are unchanged.

ivacam ships LinuxCNC, GRBL and HPGL post-processors plus a runtime for Autodesk-style .cps post scripts, but it is a GPL-3 application rather than a library.

## 2D toolpath geometry

Offsetting and polygon booleans are well covered; cavalier\_contours (82) and i\_overlay (80) lead the four crates that actually offset, and the pure-Rust Clipper2 port is the riskiest of them.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [cavalier\_contours](https://crates.io/crates/cavalier_contours) | 2D polyline and shape offsetting and combining; ivacam builds on it | 0.9.0, Aug 20, 2026; MIT or Apache-2.0; first published 2021 | Confirm arc-segment support against its docs if you want G2/G3 output; only 2 dependent crates on crates.io; its repository carries AGENTS.md |
| [i\_overlay](https://docs.rs/crate/i_overlay/latest) | Pure-Rust polygon union, intersection and difference, plus outline and stroke offsetting; the boolean engine inside geo | 9.0.1, Oct 8, 2026; MIT or Apache-2.0 | Five major versions in five months (5.0 on Apr 22, 2026, 9.0 on Sep 19, 2026) after 4.0 in May 2025 |
| [clipper2](https://docs.rs/crate/clipper2/latest) | Safe wrapper over C++ Clipper2: booleans, offsetting, Minkowski sum | 0.6.0, May 6, 2026 | Needs a C++17 toolchain; pre-1.0 |
| [clipper2-rust](https://docs.rs/clipper2-rust) | Pure-Rust port of Clipper2 with the same operations | 1.2.0, Sep 18, 2026; BSL-1.0; first published Feb 2026 | Newer; its repository carries CLAUDE.md; ivacam lists it as a dependency |
| [spade](https://github.com/Stoeoef/spade) | Robust 2D Delaunay and constrained Delaunay triangulation | 2.15.1, Mar 24, 2026 | 2D only; used by geo, among others |
| [geo](https://github.com/georust/geo) | General 2D geometry types and algorithms, with earcut and Delaunay triangulation | 0.33.1, Apr 20, 2026 | Geospatial focus; its repository carries AGENTS.md |
| [dxf](https://github.com/ixmilia/dxf-rs) | DXF read/write, as used by ivacam | 0.6.2, Oct 3, 2026 | Only if you want 2D drawing input |
| [usvg](https://github.com/linebender/resvg) | SVG parsing and simplification, as used by ivacam | 0.48.1, Aug 2, 2026 | Only if you want 2D drawing input |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| usvg | 5 | 5 | 3 | 3.7 | 4.5 | 5 | **85** |
| spade | 4 | 5 | 3 | 4.7 | 4 | 5 | **84** |
| geo | 4 | 5 | 3 | 5 | 4 | 4 | **83** |
| cavalier\_contours | 5 | 4 | 5 | 4.3 | 1.5 | 4 | **82** |
| i\_overlay | 5 | 3 | 4 | 3.7 | 3.5 | 5 | **80** |
| clipper2 | 4 | 2 | 4 | 4.7 | 2 | 5 | **73** |
| dxf | 4 | 5 | 3 | 3.3 | 2 | 5 | **72** |
| clipper2-rust | 5 | 1 | 4 | 4.3 | 1.5 | 2 | **65** |

Spade, geo, dxf and usvg are complements rather than rivals, so compare only the four offsetting crates: cavalier\_contours 82, i\_overlay 80, clipper2 73, clipper2-rust 65. cavalier\_contours has the best features score (5) but the weakest adoption on crates.io, and its AI-risk score is 4 only because it added an AGENTS.md to a 5.6-year-old project. i\_overlay is the engine inside geo, so it is widely exercised, but its pace of major versions will cost you upgrade work. clipper2-rust is under 1.5 years old with agent-instruction files and a rapid release cadence, which is why it trails the C++ wrapper it ports.

Pocketing, contouring and isolation routing in Rust CAM projects found here all lean on Clipper-style offsetting: CopperCrab uses clipper2, and ivacam uses cavalier\_contours and clipper2-rust.

## 4th-axis machining

I found no Rust crate that plans rotary toolpaths, so this layer is original work and has no scoring matrix; the useful prior art is in other languages, and the crates that would feed it are scored in the 2D and collision sections.

Existing Rust CAM projects are 2.5D or 3-axis. [ivacam](https://github.com/aalarchiv/ivacam) (GPL-3) imports DXF and SVG and has a 3D toolpath and material-removal simulation. OpenCAMStudio is a Rust tool for DXF/DWG with 2.5D toolpaths, material simulation and six post-processor dialects. CopperCrab handles PCB routing, and cnccoder handles 3-axis GRBL.

Rotary prior art outside Rust shows what to build:

- [RotatoCAM](https://github.com/revjmoney/RotatoCAM) turns STL or STEP into simultaneous 4-axis rotary toolpaths, wrapped engraving and a material-removal simulation. I did not confirm its language.
- [3dCamSlicer](https://github.com/topics/cam?o=desc&s=updated) (Python) makes simulated X/Z/A toolpaths from STL or OBJ.
- [cnc\_4th\_axis\_sim](https://github.com/SergioReyesSan/cnc_4th_axis_sim) (JavaScript) unrolls the cylinder to a 2D plane with U = θ·r and V = Z, then maps paths back to synchronised X, Z and A moves.
- A [CaseMaker design issue](https://github.com/SlyWombat/CaseMaker/issues/223) plans rotary stock simulation on a grid of A and X with radius as the stored value.

Two ways to build it with the crates above (my sketch, not from a source):

1. **Wrap or unroll (indexed or wrapped 4th axis).** Map the surface to a 2D development, do the pocketing or engraving with cavalier\_contours or Clipper-style offsets, then convert unrolled coordinates to A angles. It reuses almost everything in the 2D section and suits engraving and cylindrical features.
2. **Continuous 4-axis.** Step A in small increments; at each angle, ray-cast the part with a parry3d BVH, or evaluate the B-rep surface via OpenCASCADE, to get the radius along X, then offset for the cutter and link the passes. This needs your own gouge checking and linking logic.

rs-opw-kinematics covers six-axis robot kinematics with collision-aware path planning, which is more than a single rotary axis needs, but it is a Rust example of that planning style.

## Collision, spatial indexing and math

parry3d covers most of the geometry queries a CAM tool needs and scores 83, but it changes API often; the general-purpose crates around it (rayon, nalgebra, glam, rstar) all score 88 or more.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [parry3d](https://parry.rs) | BVH, ray casts, distance and contact queries, triangle meshes, convex hulls, voxels; an f64 variant, parry3d-f64, comes from the same repo | 0.31.1, Sep 18, 2026; Apache-2.0; 38 dependent crates, including bevy\_xpbd\_3d | Moved from nalgebra to glam in 0.26 (Jan 2026) and reached 0.31 by Sep, five minor-version series in eight months; the default build is f32 |
| [glam](https://crates.io/crates/glam) | Fast graphics-style vectors and matrices | 0.34.1, Oct 6, 2026; 21 releases in 12 months; 1,450 dependent crates | Pre-1.0 |
| [glamx](https://lib.rs/crates/glamx) | Adds poses, SVD and eigendecomposition on top of glam; from the same maintainer as parry3d | 0.3.1, Sep 17, 2026; first published Jan 2026 | Under a year old; convert to nalgebra types through feature flags when mixing libraries |
| [nalgebra](https://docs.rs/nalgebra) | General linear algebra and geometry | 0.35.0, May 24, 2026; 2,094 dependent crates | A second math type to bridge if you also use glam-based crates |
| [rstar](https://crates.io/crates/rstar) | R-tree spatial index for 2D and 3D queries | 0.13.0, May 24, 2026; 180 dependent crates | Also pulled in by parry3d |
| [robust](https://crates.io/crates/robust) | Shewchuk exact geometric predicates, used by spade and halfedge | 1.2.0, May 2025; 50 dependent crates | Slower than plain floats, but avoids silent failures; no release in 17 months |
| [rayon](https://crates.io/crates/rayon) | Data parallelism; parry3d and i\_overlay both expose it as a feature | 1.12.0, Apr 14, 2026; 11,566 dependent crates | Worth enabling for toolpath sampling |
| [mesh\_to\_sdf](https://lib.rs/crates/mesh_to_sdf) | Signed distance fields from a triangle mesh, accelerated with an R-tree and BVH | 0.4.0, Sep 2024 | Could support stock or clearance checks; not tested; no release in 25 months |
| [rs-opw-kinematics](https://docs.rs/rs-opw-kinematics) | Forward and inverse kinematics for six-axis robots, with collision checks and path planning | 3.0.0, Sep 10, 2026; BSD-3-Clause | Aimed at robot arms, not mills; no dependent crates |

Scores for the crates above (parry3d-f64 comes from the same repository and releases as parry3d and is not scored separately):

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| rayon | 4 | 5 | 5 | 5 | 5 | 5 | **96** |
| nalgebra | 4 | 5 | 5 | 4.3 | 5 | 5 | **93** |
| glam | 5 | 4 | 4 | 4 | 5 | 5 | **89** |
| rstar | 4 | 4 | 4 | 5 | 4.5 | 5 | **88** |
| parry3d | 5 | 4 | 4 | 4 | 3 | 5 | **83** |
| glamx | 5 | 1 | 3 | 4.3 | 3 | 5 | **71** |
| robust | 2 | 4 | 3 | 3.3 | 4 | 5 | **67** |
| rs-opw-kinematics | 5 | 2 | 2 | 4.7 | 1 | 5 | **66** |
| mesh\_to\_sdf | 2 | 2 | 3 | 5 | 1 | 5 | **59** |

The foundations (rayon, nalgebra, glam, rstar) are the safest dependencies in the whole survey. parry3d is the one CAM-critical crate here: its health is good, but its test density is modest (about 3 tests per 1,000 lines) and its API churn is the risk to plan for. glamx is held back mainly by its age, and mesh\_to\_sdf by two years without a release.

I found no standalone crate for material-removal simulation. The Rust apps that have one (ivacam, OpenCAMStudio) implement it inside the application, so plan to write it, probably on a height-field or dexel grid.

## Project files, identifiers and hashing

The architecture (section 5.6) wants a project folder of small text files with stable key ordering, a schema version in every file, stable IDs with explicit order keys, and content hashes for the model files. This is the dependency every other crate ends up touching, so the aim is the most boring choice available.

### Serialisation formats

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [serde](https://crates.io/crates/serde) | The serialisation framework nearly every Rust data format plugs into; derive macros for our plain-data types | 1.0.229, Jul 18, 2026; 129,067 dependents | Only 1 release in 12 months, but it is a stable interface, not a moving one. It defines no format itself |
| [serde\_json](https://crates.io/crates/serde_json) | JSON reading and writing, pretty-printed output, optional insertion-order maps (`preserve_order`) | 1.0.152, Oct 10, 2026; 108,195 dependents | JSON has no comments. NaN and infinity are written as null, so validate numbers before saving |
| [toml](https://crates.io/crates/toml) | TOML reader and writer on serde | 1.1.8, Oct 9, 2026; 19,961 dependents | Awkward for long arrays of nested tables, so a poor fit for the operation list but good for small settings files |
| [toml\_edit](https://crates.io/crates/toml_edit) | Edits TOML while keeping comments and formatting; the engine under Cargo | 0.25.17, Oct 9, 2026 (pre-1.0); 1,804 dependents | Lets a user hand-edit a tool or post file and have the app save it without wrecking their layout. Pre-1.0 version numbers |
| [ron](https://crates.io/crates/ron) | Rust Object Notation, a Rust-like text format on serde | 0.12.2, Jun 22, 2026; 1,003 dependents | Few tools outside Rust read it, which hurts hosts in other languages (INT-05) |
| [kdl](https://crates.io/crates/kdl) | KDL document language with a format-preserving parser | 6.7.1, May 31, 2026; 134 dependents | Its repository carries an AGENTS.md. Few people know KDL, and serde support is separate |
| [json5](https://crates.io/crates/json5) | JSON5 on serde, allows comments and trailing commas | 1.3.1, Feb 7, 2026; 330 dependents | No CI workflow located; sparse docs |
| [serde\_yaml](https://crates.io/crates/serde_yaml) | YAML on serde | 0.9.34+deprecated, Mar 25, 2024 | Archived by its author and versioned "+deprecated"; YAML is also easy to get wrong in diffs. Do not start new code on it |
| [schemars](https://crates.io/crates/schemars) | Generates JSON Schema from the same Rust types | 1.2.2, Jul 27, 2026; 6,353 dependents | Publishes the project-file schema for hosts and editors; migration code (PRJ-04) is still ours |
| [jsonschema](https://crates.io/crates/jsonschema) | Validates JSON against a schema | 0.58.6, Oct 6, 2026; 67 releases in 12 months | Fast release cadence; pin the version |
| [indexmap](https://crates.io/crates/indexmap) | Hash map that keeps insertion order | 2.14.2, Sep 5, 2026; 6,192 dependents | Only needed where file order should differ from sorted order; `BTreeMap` in the standard library already gives sorted, deterministic output |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| serde\_json | 5 | 5 | 5 | 4.3 | 5 | 5 | **97** |
| indexmap | 5 | 5 | 4 | 4.7 | 5 | 5 | **95** |
| serde | 4 | 5 | 5 | 4.3 | 5 | 5 | **93** |
| toml\_edit | 5 | 5 | 4 | 4.3 | 5 | 5 | **93** |
| toml | 5 | 5 | 4 | 4 | 5 | 5 | **92** |
| schemars | 5 | 4 | 4 | 4.3 | 5 | 5 | **90** |
| jsonschema | 5 | 4 | 4 | 4 | 5 | 5 | **89** |
| ron | 4 | 5 | 3 | 4.3 | 5 | 5 | **85** |
| kdl | 4 | 4 | 3 | 4.7 | 4 | 4 | **79** |
| serde\_yaml | 1 | 5 | 2 | 4 | 5 | 5 | **68** |
| json5 | 3 | 5 | 2 | 2.3 | 4.5 | 5 | **68** |

serde with toml (93 and 92) is the choice for the project folder, made by the owner after spike SP-8 (decision 0006): in the spike both toml and serde\_json gave byte-identical re-saves, exact doubles and clean Git merges, and toml gave files about 27% smaller and easier to review. serde\_json (97) stays the safe default for the shared plain-data form and tests, and is the recorded alternative. Output order is under our control. Struct fields are written in declaration order, and maps should be `BTreeMap` or sorted before writing, which is what the "save twice gives identical bytes" test in the architecture checks. `toml_edit` (93) suits the human-edited files, the tool and material libraries and the post definitions (TL-01, GC-03), where preserving a user's comments matters more than nesting depth. `serde_yaml` is archived and should not be used; `ron`, `kdl` and `json5` work but few tools outside Rust read them, which would make them a poor fit for outside hosts. `schemars` (90) and `jsonschema` (89) are optional extras for publishing and checking the schema.

### Identifiers, order keys and hashing

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [uuid](https://crates.io/crates/uuid) | UUID generation and parsing; v4 random and v7 time-ordered | 1.28.0, Oct 10, 2026; 21,927 dependents | IDs are created when the user adds an item and then saved, never made during pathing, so randomness does not threaten determinism (NFR-06) |
| [ulid](https://crates.io/crates/ulid) | Sortable 128-bit IDs written in 26 characters | 3.0.0, Jul 16, 2026; 795 dependents | Shorter than a UUID string, but nothing in the requirements needs time ordering |
| [nanoid](https://crates.io/crates/nanoid) | Short random string IDs | 0.5.0, Apr 13, 2026; 340 dependents | Short IDs are readable in diffs, but collision safety depends on the length chosen |
| [slotmap](https://crates.io/crates/slotmap) | Arena containers with generational keys | 1.1.1, Dec 6, 2025; 519 dependents | Keys are not stable between sessions, so use them for in-memory handles only, never in saved files. No CI workflow found |
| [fractional\_index](https://crates.io/crates/fractional_index) | Fractional-index order keys, so inserting between two items touches only the new item | 2.0.2, Sep 17, 2024; 4 dependents (one is loro\_fractional\_index) | Only usable candidate for PRJ-05 and still tiny; no release since Sep 2024 |
| [lexorank](https://crates.io/crates/lexorank) | LexoRank-style order strings | 2.0.0, Apr 27, 2023; no dependents | Last release Apr 2023; its licence field reads "non-standard" on crates.io, which cargo-deny would flag |
| [blake3](https://crates.io/crates/blake3) | Fast cryptographic hash, multi-thread capable | 1.8.7, Aug 20, 2026; 3,946 dependents | Licence is CC0-1.0 or Apache-2.0, so allow it explicitly in cargo-deny. Not a NIST digest |
| [sha2](https://crates.io/crates/sha2) | SHA-256 and SHA-512 from the RustCrypto family | 0.11.0, Mar 25, 2026; 21,066 dependents | Slower than blake3; useful only if another tool must reproduce the hash |
| [xxhash-rust](https://crates.io/crates/xxhash-rust) | Non-cryptographic XXH32, XXH64 and XXH3 | 0.8.19, Sep 28, 2026; 939 dependents | Boost Software Licence (BSL-1.0), the same family as clipper2-rust; not suitable for hashes that guard against hostile files |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| uuid | 5 | 5 | 5 | 4.7 | 5 | 5 | **99** |
| blake3 | 5 | 4 | 5 | 4 | 5 | 5 | **93** |
| ulid | 5 | 5 | 3 | 4.7 | 4.5 | 5 | **89** |
| sha2 | 4 | 5 | 4 | 3.3 | 5 | 5 | **85** |
| xxhash-rust | 5 | 4 | 3 | 3.7 | 4.5 | 5 | **82** |
| nanoid | 4 | 5 | 2 | 4.7 | 4.5 | 5 | **81** |
| slotmap | 3 | 5 | 4 | 3 | 4.5 | 5 | **78** |
| fractional\_index | 2 | 4 | 3 | 4.3 | 1.5 | 5 | **64** |
| lexorank | 1 | 3 | 2 | 2.3 | 1 | 5 | **43** |

`uuid` (99) covers IDs and `blake3` (93) covers the model content hash (PRJ-02, PRJ-06) and the cache keys in section 6 of the architecture; use `sha2` (85) only if an outside tool needs a standard digest. Order keys are the thin spot: `fractional_index` (64) is the only usable crate, has four dependents and has had no release for two years, and `lexorank` (43) is worse. The algorithm is a page of code, so the safer plan is to write it in-house, test it as a property (keys stay strictly ordered under random inserts), and use `fractional_index` as a reference implementation. The architecture's open decision, a prototype with a real Git merge, still applies either way.

## Post-processor templates and scripting

The post is data first (architecture 5.3): a user-editable template renders the neutral machine program into G-code (GC-03), and a scripting engine may follow in P2 (GC-13). Both must be deterministic (GC-07), run only inside the app and never touch the filesystem or network (NFR-09), and build on all three platforms (NFR-13).

### Template engines

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [minijinja](https://crates.io/crates/minijinja) | Jinja2-style templates (loops, conditionals, filters, macros) with custom filters and functions, designed for a small footprint | 3.0.0, Oct 8, 2026; 19 releases in 12 months | 3.0 is days old, so pin it and expect minor breakage. Check number-format control (decimals, trailing zeros) in a spike |
| [tera](https://crates.io/crates/tera) | Jinja-style engine with inheritance and filters | 2.4.0, Sep 11, 2026; 864 dependents | 16 releases in 12 months, so its API may keep moving |
| [handlebars](https://crates.io/crates/handlebars) | Handlebars templates with helpers and partials, and optional rhai-scripted helpers | 6.4.4, Aug 12, 2026; 825 dependents | Logic-less style makes loops with computed values clumsy |
| [liquid](https://crates.io/crates/liquid) | Shopify Liquid templates | 0.26.11, Feb 4, 2025 | No release since Feb 2025; few tests in the package |
| [upon](https://crates.io/crates/upon) | Small template engine with filters and a clean API | 0.11.0, Jul 25, 2026; 26 dependents | Few users, so little track record |
| [askama](https://crates.io/crates/askama) | Jinja-like templates compiled into the binary at build time | 0.16.1, Sep 4, 2026; 514 dependents | Templates are fixed at compile time, so users cannot edit them. Wrong for GC-03 |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| minijinja | 5 | 4 | 5 | 4.7 | 4.5 | 5 | **94** |
| handlebars | 5 | 5 | 4 | 4.7 | 4.5 | 5 | **93** |
| tera | 5 | 5 | 4 | 4 | 4.5 | 5 | **90** |
| askama | 5 | 5 | 2 | 4.7 | 4.5 | 5 | **85** |
| upon | 4 | 3 | 3 | 4 | 2 | 5 | **69** |
| liquid | 2 | 5 | 3 | 3 | 3.5 | 5 | **68** |

`minijinja` (94) leads and has the best match to the need: runtime templates, Jinja syntax that many developers already know, and a small footprint. `handlebars` (93) and `tera` (90) are the credible alternatives. Whatever is chosen stays behind the `PostProcessor` trait. The spike should render a real Makera-style header, tool change and rotary move with fixed decimals and compare the output bytes.

### Scripting engines

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [mlua](https://crates.io/crates/mlua) | Bindings to several Lua versions, LuaJIT and Luau with a safe API | 0.12.2, Oct 3, 2026; 496 dependents | Builds the C Lua source, so needs a C compiler (NFR-13 builds must prove it on all three platforms). Lua is a plausible language for post authors |
| [rhai](https://crates.io/crates/rhai) | Pure-Rust scripting language designed for embedding and sandboxing, Rust-like syntax | 1.26.1, Sep 10, 2026; 350 dependents | Its repository carries an AGENTS.md; interpreted, which is fine for post-sized scripts |
| [starlark](https://crates.io/crates/starlark) | Meta's Starlark (the Bazel and Buck language), designed to be hermetic and deterministic | 0.14.2, Jun 5, 2026; 32 dependents | Few users outside Meta's tools; Python-like syntax with deliberately few features |
| [rquickjs](https://crates.io/crates/rquickjs) | QuickJS JavaScript engine bindings | 0.14.0, Sep 18, 2026; 211 dependents | Builds C code; JavaScript is familiar but a large language for a post script |
| [boa\_engine](https://crates.io/crates/boa_engine) | JavaScript engine written in Rust | 0.22.0, Aug 28, 2026; 98 dependents | Pure Rust but pre-1.0 and large (about 138,000 lines) |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| mlua | 5 | 4 | 5 | 5 | 4 | 5 | **94** |
| rhai | 5 | 5 | 4 | 4.7 | 4 | 4 | **90** |
| rquickjs | 5 | 4 | 4 | 4.7 | 4 | 5 | **89** |
| starlark | 4 | 5 | 4 | 4 | 3.5 | 5 | **84** |
| boa\_engine | 5 | 3 | 3 | 5 | 3.5 | 5 | **82** |

`mlua` (94) scores highest, ahead of `rhai` (90), `rquickjs` (89) and `starlark` (84). For this project the fit matters more than a four-point gap. `rhai` builds with Cargo alone, has sandboxing as a design goal and needs no C compiler; `starlark` is the closest match to the determinism rule (NFR-06) because the language was designed to be hermetic and deterministic; `mlua` is the most capable and familiar but adds a C dependency. My suggestion, which is judgment: leave this until P2, start the spike with `rhai`, and keep `starlark` as the alternative if byte-identical output is hard to guarantee.

## Setup sheets and PDF output

GC-12 asks for a setup sheet as HTML or PDF. HTML needs no extra crate: it is the same template engine as the posts, and the user can print it to PDF from a browser. A PDF crate is only needed if the app must write the file itself.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [printpdf](https://crates.io/crates/printpdf) | Generates PDFs from text, shapes and images | 0.12.8, Sep 5, 2026; 16 releases in 12 months | Large API, 101 dependents; layout is manual |
| [typst](https://crates.io/crates/typst) | The Typst typesetting compiler as a library: markup in, high-quality PDF out | 0.15.1, Jul 17, 2026; 157 dependents | Large dependency, and the host must supply fonts; pre-1.0, so the library API changes between releases |
| [krilla](https://crates.io/crates/krilla) | High-level PDF writer (text, fonts, gradients, images) used by typst-pdf | 0.8.2, Jun 4, 2026; 28 dependents | 2.1 years old with one main dependent; no layout engine |
| [pdf-writer](https://crates.io/crates/pdf-writer) | Low-level PDF writer from the Typst team; krilla builds on it | 0.15.0, May 27, 2026; 66 dependents | You write PDF objects yourself, so layout and fonts are all ours |
| [genpdf](https://crates.io/crates/genpdf) | Document layout on top of printpdf | 0.2.0, Jun 17, 2021 | No release since Jun 2021, and its repository is on sourcehut so CI was not probed |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| printpdf | 5 | 5 | 4 | 4.3 | 4 | 5 | **90** |
| typst | 5 | 3 | 5 | 4.3 | 4 | 5 | **88** |
| pdf-writer | 4 | 4 | 3 | 3.7 | 3.5 | 5 | **75** |
| krilla | 4 | 2 | 4 | 4.3 | 3 | 5 | **74** |
| genpdf | 1 | 4 | 2 | 3.3 | 2 | 5 | **53** |

`printpdf` (90) is the pragmatic choice for a plain tabular setup sheet. `typst` (88) gives much better output (real typesetting) and ranks first if features are weighted more heavily, but it is a large dependency and ties the sheet to Typst's release pace. `genpdf` (53) has had no release for over five years. Recommendation, as judgment: ship HTML first, add `printpdf` when the PDF requirement is firm, and look at `typst` only if layout quality becomes a user complaint.

## Embedded version control

The architecture (5.6) puts embedded Git behind a `VersionControl` trait with a `git2` adapter and says `gix` can replace it later if restore and merge mature. The requirements need commit, log, structured comparison, restore and branch merge (VC-01 to VC-06).

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [git2](https://crates.io/crates/git2) | Rust bindings to libgit2: repositories, commits, diffs, checkout, merge, revert, cherry-pick, branches | 0.21.0, May 18, 2026; 2,028 dependents (Cargo uses it) | Needs a C compiler to build libgit2; the default features pull in SSH and HTTPS, so build with `default-features = false` as the architecture says. Release cadence is 3 per year |
| [gix](https://crates.io/crates/gix) | Pure-Rust Git implementation (gitoxide), modular and fast | 0.89.0, Oct 8, 2026; 18 releases in 12 months; 456 dependents | Its own status file (read Oct 10, 2026) lists commit, status, diffing and merge-base as done but leaves checkout/restore/reset, merge, cherry-pick/revert, rebase, stash and push unticked. The repository carries an AGENTS.md |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| git2 | 4 | 5 | 5 | 5 | 5 | 5 | **96** |
| gix | 5 | 3 | 3 | 4.7 | 4.5 | 3 | **79** |

`git2` (96) is the right choice and the gap is wide: it is nearly twelve years old, has over two thousand dependents, and has the commit, diff, checkout and merge calls VC-01 to VC-05 need today. `gix` (79) is healthy and fast moving, but the restore and merge workflows the project needs are listed as not yet done in its own status file, so I scored its features 3. Earlier drafts left `git2` unscored; it now has a score and nothing here changes the architecture's decision. The fallback if libgit2 ever causes build trouble is calling the system `git` command, which needs Git installed but no crate.

## NURBS curves and surfaces

INT-02 needs the pathing library to take B-rep faces and edges as plain data and ask for points, normals and nearest-surface queries through the `SurfaceSource` port. If the STEP kernel is OpenCASCADE, its geometry evaluation can sit behind that port and no other crate is needed. The crates below matter for the pure-Rust route, for spline guide curves, and for surfaces the library builds itself.

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [curvo](https://crates.io/crates/curvo) | NURBS curves and surfaces, with extrude, loft, sweep and revolve operations (per its crates.io summary) | 0.3.2, Oct 7, 2026; 27 releases in 12 months | 2.5 years old with 2 dependents; API still moving (0.3) |
| [brepkit-math](https://crates.io/crates/brepkit-math) | Math layer of the brepkit kernel: NURBS, analytic surfaces, BVH, triangulation | 4.1.48, Oct 9, 2026; 207 releases in about two months | AGPL-3.0-only. Created in Aug 2026 and its repository has a CLAUDE.md. See the STEP section for the whole kernel |
| [enterpolation](https://crates.io/crates/enterpolation) | Generic interpolation: Bézier, B-spline and NURBS curves, linear and more | 0.3.0, May 8, 2025 | Curves and interpolation only, no surfaces; no release since May 2025 |
| [splines](https://crates.io/crates/splines) | Spline interpolation (Catmull-Rom, Bézier, cubic and others) for animation-style use | 5.0.0, Apr 26, 2025 | Not NURBS and no surfaces; no release since Apr 2025; hosted on sourcehut so CI was not probed |
| [truck-geometry](https://crates.io/crates/truck-geometry) | Curves and surfaces of the truck kernel (B-splines, NURBS) | 0.5.0, Sep 20, 2024 | Tied to the truck kernel and unreleased since Sep 2024 |
| [monstertruck-geometry](https://crates.io/crates/monstertruck-geometry) | Same lineage as truck-geometry in the monstertruck fork | 0.4.1, Sep 19, 2026; 9 releases in 12 months | Under a year old; CLAUDE.md and AGENTS.md in its repository; about 1,300 recent downloads |
| [bspline](https://crates.io/crates/bspline) | B-spline curve evaluation | 1.1.0, Feb 11, 2022 | Last release Feb 2022; two dependents |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| curvo | 5 | 2 | 4 | 3.7 | 1.5 | 5 | **71** |
| splines | 2 | 5 | 3 | 4 | 2 | 5 | **67** |
| brepkit-math | 5 | 1 | 4 | 5 | 1.5 | 1 | **66** |
| enterpolation | 2 | 4 | 3 | 4.3 | 1.5 | 5 | **64** |
| truck-geometry | 2 | 4 | 3 | 3.7 | 1.5 | 5 | **61** |
| monstertruck-geometry | 5 | 1 | 3 | 4.3 | 1.5 | 1 | **59** |
| bspline | 1 | 5 | 2 | 4.3 | 1 | 5 | **57** |

No candidate scores above 71, and none is proven to supply what a `SurfaceSource` needs (point and normal at (u, v), closest-point projection, ray intersection). `curvo` (71) is the healthiest pure-Rust surface library but young and thinly used. `brepkit-math` (66) is more complete but has an AGPL-3.0-only licence: that fits the AGPL library family in INT-09 but would block a later dual licence or closed-source host. Practical advice: use the kernel's evaluation for STEP models, write the analytic surfaces (cylinder, cone, sphere, helical) in-house since the test oracles need them anyway, and spike `curvo` only if NURBS surfaces must be evaluated without a C++ kernel.

## Python and C bindings

INT-05 asks for Python bindings, mainly so a FreeCAD workbench can call the library, and INT-06 for a C interface (P3). Both are adapter crates over the public API, so the choice does not affect the core.

### Python

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [pyo3](https://crates.io/crates/pyo3) | Rust bindings for Python: write Python modules and classes in Rust, or embed Python | 0.29.3, Sep 30, 2026; 2,252 dependents | The `abi3` feature builds one wheel for many Python versions, which matters for FreeCAD's bundled interpreter. Pre-1.0 (0.29), so expect breaking releases |
| [maturin](https://crates.io/crates/maturin) | Builds and publishes pyo3 projects as wheels | 1.15.0, Aug 24, 2026; 22 releases in 12 months | A build tool, so the dependents count understates its use; its repository carries an AGENTS.md |
| [numpy](https://crates.io/crates/numpy) | NumPy array interop for pyo3 | 0.29.0, Jun 13, 2026; 386 dependents | Needed only if toolpaths are returned as arrays |
| [uniffi](https://crates.io/crates/uniffi) | Generates Python, Kotlin, Swift and Ruby bindings from one interface description | 0.32.2, Sep 23, 2026; 233 dependents (Mozilla) | MPL-2.0 licence; generated Python is less idiomatic than pyo3's and does not help with the C interface |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| pyo3 | 5 | 5 | 5 | 4.7 | 5 | 5 | **99** |
| numpy | 4 | 5 | 4 | 5 | 4.5 | 5 | **90** |
| maturin | 5 | 4 | 5 | 4.3 | 1.5 | 4 | **82** |
| uniffi | 5 | 4 | 3 | 3.7 | 4 | 5 | **81** |

`pyo3` (99) with `maturin` (82) is the standard route and no alternative comes close; `numpy` (90) is a small add-on. `uniffi` (81) is worth a look only if bindings for languages other than Python become a goal.

### C interface

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [cbindgen](https://crates.io/crates/cbindgen) | Generates C and C++ headers from `extern "C"` Rust code | 0.29.4, Jun 9, 2026; 450 dependents (Mozilla) | MPL-2.0 licence; it describes the interface, it does not make it safe. Catch panics at the boundary as the architecture requires |
| [diplomat](https://crates.io/crates/diplomat) | Generates C, C++ and other language bindings from annotated Rust (ICU4X's tool) | 0.16.1, Aug 20, 2026; 10 dependents | Few users outside ICU4X |
| [interoptopus](https://crates.io/crates/interoptopus) | Generates bindings for C, C# and Python from one Rust definition | 0.16.6, Oct 9, 2026; 35 releases in 12 months | CLAUDE.md in the repository; 14 dependents |
| [safer-ffi](https://crates.io/crates/safer-ffi) | Safer FFI layer with header generation | 0.1.13, Jan 16, 2026; 0.2.0-rc1 is a pre-release | Few tests in the package; 24 dependents |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| cbindgen | 4 | 5 | 4 | 3.3 | 4.5 | 5 | **84** |
| diplomat | 5 | 4 | 3 | 4.3 | 3.5 | 5 | **82** |
| interoptopus | 5 | 4 | 3 | 5 | 2 | 4 | **78** |
| safer-ffi | 3 | 4 | 3 | 3.3 | 2.5 | 5 | **67** |

`cbindgen` (84) is the straightforward answer for INT-06: a small hand-written `extern "C"` layer over plain-data types plus a generated header. `diplomat` (82) is the alternative if C++ and other languages are wanted from the same definitions. `cbindgen` and `uniffi` are MPL-2.0 (file-level copyleft), so add that licence to the cargo-deny allow list for build tools.

## Errors, logging and diagnostics

Architecture section 8 requires typed errors from every importer and parser, no panics in library code, and a diagnostics sink instead of logging inside the libraries. The application and CLI may log.

### Errors and diagnostics

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [thiserror](https://crates.io/crates/thiserror) | Derive macro for typed error enums | 2.0.21, Sep 23, 2026; 73,088 dependents | Library-side; it does not give context chains |
| [anyhow](https://crates.io/crates/anyhow) | Single flexible error type with context | 1.0.104, Jul 18, 2026; 50,928 dependents | Application-side only; its type erases the error kind, so keep it out of library APIs |
| [snafu](https://crates.io/crates/snafu) | Error enums with context selectors | 0.9.2, Jul 21, 2026; 997 dependents | More ceremony than thiserror for similar results |
| [miette](https://crates.io/crates/miette) | Diagnostics with source spans and pretty reports | 7.6.0, Apr 27, 2025; 1,571 dependents | Last release Apr 2025 and an AGENTS.md in the repository. Useful for pointing at the line of a broken post template |
| [eyre](https://crates.io/crates/eyre) | Fork of anyhow with customisable reports | 0.6.14, Aug 11, 2026; 1,651 dependents | Overlaps anyhow |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| anyhow | 5 | 4 | 5 | 4.7 | 5 | 5 | **96** |
| thiserror | 5 | 4 | 5 | 4.3 | 5 | 5 | **94** |
| snafu | 5 | 4 | 4 | 4.7 | 4.5 | 5 | **90** |
| eyre | 4 | 4 | 3 | 5 | 5 | 5 | **85** |
| miette | 2 | 4 | 4 | 4 | 5 | 4 | **75** |

`thiserror` (94) for library error types and `anyhow` (96) in `openchuck-app` and `openchuck-cli` is the common split and is what the architecture implies. `miette` (75) is optional, for showing template and parse errors with source locations. `snafu` and `eyre` add nothing the two leaders do not cover.

### Logging and tracing

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [tracing](https://crates.io/crates/tracing) | Structured, span-based instrumentation | 0.1.44, Dec 18, 2025; 44,554 dependents | Still on 0.1.x; a subscriber crate does the output |
| [tracing-subscriber](https://crates.io/crates/tracing-subscriber) | Collects and formats tracing events, with filtering | 0.3.23, Mar 13, 2026; 19,576 dependents | Needed with tracing to print or record anything |
| [log](https://crates.io/crates/log) | The standard logging facade | 0.4.34, Aug 22, 2026; 31,309 dependents | No spans, so job timing needs extra work |
| [env\_logger](https://crates.io/crates/env_logger) | Simple log output configured by environment variable | 0.11.11, Jun 25, 2026; 12,081 dependents | Plain text only |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| log | 5 | 5 | 4 | 4.7 | 5 | 5 | **95** |
| tracing | 3 | 5 | 5 | 4.3 | 5 | 5 | **89** |
| tracing-subscriber | 4 | 4 | 4 | 5 | 5 | 5 | **89** |
| env\_logger | 4 | 5 | 3 | 4.3 | 5 | 5 | **85** |

`tracing` with `tracing-subscriber` (both 89) fits the job manager, because spans give per-operation timing for the progress and performance work (NFR-03, NFR-04), and `log` (95) is the fallback if a smaller footprint is wanted. Libraries should not depend on either; they report through the diagnostics sink and the app turns those reports into log events.

## Testing, fuzzing, benchmarking and CI tooling

The verification architecture (section 9) depends on property tests, snapshot tests of G-code and setup sheets, importer fuzzing, CLI scenarios, benchmarks, a semver check on the public API and a dependency check in CI. Most of these crates are used only in development and do not ship, so a weak result here is cheaper to fix than elsewhere.

### Test libraries

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [proptest](https://crates.io/crates/proptest) | Property testing with shrinking | 1.12.0, Oct 10, 2026; 11,495 dependents | Larger API than quickcheck; commit the regression files it writes |
| [quickcheck](https://crates.io/crates/quickcheck) | Original QuickCheck for Rust | 1.1.0, Feb 10, 2026; 1,553 dependents | Simpler shrinking; fewer strategy tools |
| [insta](https://crates.io/crates/insta) | Snapshot testing with review tooling (`cargo insta`) | 1.49.0, Oct 3, 2026; 4,756 dependents | Fits the golden-file tests for G-code and setup sheets |
| [expect-test](https://crates.io/crates/expect-test) | Inline snapshot tests that update in the source | 1.5.1, Dec 21, 2024; 335 dependents | No release since Dec 2024 |
| [similar](https://crates.io/crates/similar) | Text diffing library, used by insta | 3.2.0, Aug 17, 2026; 1,190 dependents | Needed if the app shows its own diffs of generated G-code |
| [approx](https://crates.io/crates/approx) | Float comparison macros (absolute, relative, ULPs) | 0.5.1, Feb 5, 2026; 2,654 dependents | 0.6.0-rc2 exists as a pre-release; stable 0.5.1 is a year old |
| [rstest](https://crates.io/crates/rstest) | Fixtures and table-driven tests | 0.27.0, Sep 6, 2026; 3,432 dependents | CLAUDE.md in the repository |
| [assert\_cmd](https://crates.io/crates/assert_cmd) | Run and check command-line programs in tests | 2.2.2, May 11, 2026; 5,332 dependents | Suits the CLI scenarios |
| [arbitrary](https://crates.io/crates/arbitrary) | Structured random data for fuzzing and property tests | 1.5.0, Oct 6, 2026; 1,003 dependents | Used with cargo-fuzz |
| [tempfile](https://crates.io/crates/tempfile) | Temporary files and directories | 3.27.0, Mar 11, 2026; 36,042 dependents | Needed for project save and load tests |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| proptest | 5 | 5 | 5 | 5 | 5 | 5 | **100** |
| insta | 5 | 4 | 5 | 4.3 | 5 | 5 | **94** |
| similar | 5 | 4 | 4 | 5 | 5 | 5 | **93** |
| tempfile | 4 | 5 | 4 | 5 | 5 | 5 | **92** |
| arbitrary | 4 | 5 | 4 | 4.7 | 5 | 5 | **91** |
| rstest | 4 | 5 | 4 | 5 | 5 | 4 | **90** |
| assert\_cmd | 4 | 5 | 4 | 4 | 5 | 5 | **88** |
| approx | 3 | 5 | 4 | 4.7 | 5 | 5 | **87** |
| quickcheck | 4 | 5 | 3 | 4.3 | 5 | 5 | **85** |
| expect-test | 2 | 4 | 3 | 4.7 | 4.5 | 5 | **74** |

`proptest` (100) for invariants (A-axis continuity, order keys, round trips), `insta` (94) with `similar` (93) for the golden files, `approx` (87) for tolerance comparisons, `assert_cmd` (88) for CLI scenarios and `tempfile` (92) for project folders cover the architecture's levels directly. `rstest` (90) is optional.

### Fuzzing and benchmarking

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [cargo-fuzz](https://crates.io/crates/cargo-fuzz) | `cargo fuzz` runner for libFuzzer targets | 0.13.2, Jun 9, 2026 | Needs a nightly toolchain; Linux and macOS best supported |
| [libfuzzer-sys](https://crates.io/crates/libfuzzer-sys) | libFuzzer bindings that cargo-fuzz targets link | 0.4.13, Jun 4, 2026 | Licence expression includes NCSA (from LLVM); add it to the cargo-deny allow list. Dev-only |
| [criterion](https://crates.io/crates/criterion) | Statistical benchmarking with reports | 0.8.2, Feb 4, 2026; 18,794 dependents | Last release Feb 2026 |
| [divan](https://crates.io/crates/divan) | Benchmark harness with a simple attribute API | 0.1.21, Apr 10, 2025; 497 dependents | No release since Apr 2025 |
| [iai-callgrind](https://crates.io/crates/iai-callgrind) | Instruction-count benchmarks using Valgrind, stable across noisy machines | 0.16.1, Jul 30, 2025; 120 dependents | Needs Valgrind, so effectively Linux; AGENTS.md in the repository; no release since Jul 2025 |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| criterion | 3 | 5 | 5 | 4 | 5 | 5 | **88** |
| cargo-fuzz | 4 | 5 | 4 | 4 | 3 | 5 | **82** |
| libfuzzer-sys | 4 | 4 | 4 | 3.3 | 4 | 5 | **79** |
| divan | 2 | 3 | 4 | 4.3 | 4 | 5 | **72** |
| iai-callgrind | 2 | 3 | 4 | 4 | 3.5 | 3 | **66** |

`cargo-fuzz` (82) with `libfuzzer-sys` (79) and `arbitrary` is the established route for the importer fuzzing in section 9. For benchmarks `criterion` (88) is the proven choice; `divan` and `iai-callgrind` score lower on maintenance, though `iai-callgrind`'s instruction counts could suit a CI gate on a shared runner.

### CI tooling

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [cargo-deny](https://crates.io/crates/cargo-deny) | Licence, advisory, duplicate and source checks | 0.20.2, Jul 9, 2026; 17 releases in 12 months | Required by NFR-10 and the architecture; the config file is ours |
| [cargo-audit](https://crates.io/crates/cargo-audit) | RustSec advisory database check | 0.22.2, Jun 5, 2026 | Overlaps cargo-deny's advisory check |
| [cargo-semver-checks](https://crates.io/crates/cargo-semver-checks) | Detects public API breaks against the last release | 0.51.0, Oct 3, 2026 | Backs INT-07; AGENTS.md in the repository |
| [cargo-nextest](https://crates.io/crates/cargo-nextest) | Faster test runner with per-test timeouts and retries | 0.9.148, Oct 8, 2026; 48 releases in 12 months | CLAUDE.md and AGENTS.md in the repository |
| [cargo-llvm-cov](https://crates.io/crates/cargo-llvm-cov) | Code coverage with LLVM | 0.9.1, Sep 6, 2026 | Fits the coverage tracking in section 9 |
| [mdbook](https://crates.io/crates/mdbook) | Builds a documentation book from Markdown | 0.5.4, Jul 6, 2026 | MPL-2.0 licence; for the example host and user guide (INT-08, NFR-11) |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| mdbook | 4 | 5 | 4 | 4 | 4 | 5 | **85** |
| cargo-llvm-cov | 5 | 4 | 4 | 4 | 3 | 5 | **83** |
| cargo-deny | 4 | 4 | 5 | 3.7 | 3 | 5 | **82** |
| cargo-nextest | 5 | 3 | 5 | 4 | 3 | 3 | **80** |
| cargo-audit | 4 | 5 | 4 | 3.3 | 3 | 5 | **79** |
| cargo-semver-checks | 5 | 3 | 5 | 4.3 | 2 | 3 | **78** |

These are tools, not libraries, so the dependents count understates their use and their scores are lower on adoption than their standing deserves. `cargo-deny` (82) is a requirement; `cargo-llvm-cov` (83), `cargo-semver-checks` (78) and `cargo-nextest` (80) are sound choices for coverage, API checks and test running. Agent-instruction files in `cargo-nextest` and `cargo-semver-checks` lower their AI score, but neither ships in the product.

## CLI, jobs and platform services

The CLI (GC-11) is also the end-to-end test harness, the job manager (NFR-04) needs a way to send progress and results from workers to the UI, and the project folder needs safe saves, autosave (PRJ-08) and standard locations.

### Command line

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [clap](https://crates.io/crates/clap) | Argument parsing, subcommands, help and shell completion | 4.6.7, Sep 14, 2026; 48,218 dependents | Heavier to compile than the minimal parsers |
| [indicatif](https://crates.io/crates/indicatif) | Progress bars and spinners | 0.18.6, Jul 1, 2026; 5,450 dependents | Terminal only; the UI shows progress from the event stream instead |
| [argh](https://crates.io/crates/argh) | Derive-based argument parser by Google | 0.1.19, Mar 16, 2026; 506 dependents | Opinionated, fewer features than clap |
| [lexopt](https://crates.io/crates/lexopt) | Minimal argument lexer, no dependencies | 0.3.2, Feb 28, 2026; 183 dependents | You write the help text and subcommand handling |
| [bpaf](https://crates.io/crates/bpaf) | Combinator-based argument parser | 0.9.28, Sep 17, 2026; 102 dependents | No CI workflow located among common names |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| clap | 5 | 5 | 5 | 4.3 | 5 | 5 | **97** |
| indicatif | 4 | 5 | 4 | 5 | 5 | 5 | **92** |
| argh | 4 | 4 | 3 | 5 | 4 | 5 | **82** |
| lexopt | 4 | 4 | 3 | 4.7 | 4 | 5 | **81** |
| bpaf | 5 | 3 | 3 | 3 | 4 | 5 | **75** |

`clap` (97) is the default for the CLI, with `indicatif` (92) for progress output.

### Jobs, files and platform services

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [crossbeam-channel](https://crates.io/crates/crossbeam-channel) | Multi-producer, multi-consumer channels | 0.5.17, Sep 5, 2026; 2,673 dependents | The standard library's `mpsc` has been built on the same code since Rust 1.67 and may be enough |
| [flume](https://crates.io/crates/flume) | Fast multi-producer channels, also async | 0.12.0, Dec 8, 2025; 957 dependents | Overlaps crossbeam |
| [atomic-write-file](https://crates.io/crates/atomic-write-file) | Writes a file atomically through a temporary and rename | 0.3.1, Aug 11, 2026; 75 dependents | Small; serves PRJ-08 and crash-safe saves. The same pattern is a dozen lines with `tempfile` |
| [notify](https://crates.io/crates/notify) | Watches files and directories for changes | 8.2.0, Aug 30, 2026; 3,088 dependents | 8.2 is stable and 9.0.0-rc.5 is a pre-release; CC0-1.0 licence. Useful for noticing a changed model file |
| [rfd](https://crates.io/crates/rfd) | Native open and save dialogs | 0.17.2, Jan 12, 2026; 653 dependents | Widely used native file dialogs; works alongside egui |
| [directories](https://crates.io/crates/directories) | Per-platform config, cache and data folders | 6.0.0, Jan 12, 2025; 3,008 dependents | No release since Jan 2025 |
| [dirs](https://crates.io/crates/dirs) | Smaller sibling of directories | 7.0.0, Sep 5, 2026; 10,834 dependents | Repository is on Codeberg, so CI was not probed |
| [opener](https://crates.io/crates/opener) | Opens a file or URL with the system handler | 0.9.0, Sep 29, 2026; 151 dependents | For opening the generated setup sheet |
| [fs-err](https://crates.io/crates/fs-err) | `std::fs` wrappers with the file path in every error | 3.3.2, Oct 4, 2026; 473 dependents | No tests in the packaged source |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| crossbeam-channel | 4 | 5 | 4 | 5 | 5 | 5 | **92** |
| notify | 5 | 5 | 4 | 4 | 5 | 5 | **92** |
| opener | 5 | 5 | 3 | 4.3 | 4.5 | 5 | **88** |
| dirs | 4 | 5 | 4 | 3.7 | 5 | 5 | **87** |
| directories | 2 | 5 | 4 | 4.3 | 5 | 5 | **81** |
| fs-err | 5 | 4 | 3 | 3.3 | 4.5 | 5 | **81** |
| atomic-write-file | 4 | 3 | 4 | 4.3 | 3 | 5 | **77** |
| rfd | 3 | 4 | 4 | 3.3 | 4.5 | 5 | **77** |
| flume | 3 | 4 | 3 | 3.7 | 4.5 | 5 | **74** |

`crossbeam-channel` (92) is the proven channel for the job manager's event stream, though `std::sync::mpsc` may do. `notify` (92), `opener` (88), `dirs` (87) and `directories` (81) are low-risk helpers. `atomic-write-file` (77) and `rfd` (77) score lower on maturity and tests but each does a single job. Rayon, already scored in the math section (96), stays the parallel runtime.

## UI layout, units and undo

These support the egui front end (architecture 5.8), undo (UI-08) and the typed-units rule in section 8. None is essential; each saves some effort.

### UI layout and viewport helpers

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [egui\_dock](https://crates.io/crates/egui_dock) | Dockable, tabbed panels for egui | 0.21.1, Aug 6, 2026; 27 dependents | CLAUDE.md and AGENTS.md in the repository; third-party and follows egui's breaking releases |
| [egui\_tiles](https://crates.io/crates/egui_tiles) | Tiling layouts for egui (from Rerun) | 0.17.1, Aug 18, 2026; 32 dependents | Pre-1.0; also tracks egui releases |
| [egui\_plot](https://crates.io/crates/egui_plot) | Plots inside egui | 0.37.0, Aug 5, 2026; 127 dependents | For feed, A-angle or deviation graphs after simulation |
| [egui\_extras](https://crates.io/crates/egui_extras) | Tables, image loaders and date widgets from the egui project | 0.36.2, Sep 8, 2026; 316 dependents | Releases with egui |
| [egui\_kittest](https://crates.io/crates/egui_kittest) | UI testing and screenshot comparison for egui | 0.36.2, Sep 8, 2026; 130 dependents | 1.8 years old; fits the thin screenshot checks in section 9 |
| [bytemuck](https://crates.io/crates/bytemuck) | Safe casting between plain data and bytes, used for GPU buffers | 1.25.2, Jul 19, 2026; 4,151 dependents | Zlib, Apache-2.0 or MIT; nothing to flag |
| [meshopt](https://crates.io/crates/meshopt) | Bindings to meshoptimizer: simplification and cache optimisation for large meshes | 0.6.2, Oct 9, 2025; 26 dependents | No release since Oct 2025; C++ build; no CI workflow found |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| bytemuck | 5 | 4 | 5 | 4.7 | 5 | 5 | **96** |
| egui\_plot | 5 | 3 | 4 | 4 | 4 | 5 | **83** |
| egui\_extras | 5 | 3 | 4 | 4 | 4 | 5 | **83** |
| egui\_tiles | 5 | 3 | 4 | 4.3 | 3 | 5 | **81** |
| egui\_kittest | 5 | 2 | 3 | 4.7 | 3.5 | 5 | **77** |
| egui\_dock | 5 | 3 | 4 | 4.3 | 2.5 | 3 | **76** |
| meshopt | 3 | 5 | 4 | 2.7 | 2 | 5 | **70** |

`bytemuck` (96) is a given for the viewport. `egui_plot` (83), `egui_extras` (83) and `egui_tiles` (81) are maintained under the egui project or by Rerun and are low risk. `egui_dock` (76) works but has agent-instruction files. `meshopt` (70) matters only if the 30 fps target at 1 million triangles (NFR-03) needs mesh simplification.

### Units and undo

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [uom](https://crates.io/crates/uom) | Compile-time units of measure (lengths, speeds, angles) | 0.38.0, Feb 14, 2026; 198 dependents | Type-level generics add compile time; conversion to and from mm and mm/min is explicit |
| [dimensioned](https://crates.io/crates/dimensioned) | Compile-time dimensional analysis | 0.8.0, Apr 29, 2022 | No release since Apr 2022 |
| [measurements](https://crates.io/crates/measurements) | Runtime unit conversions | 0.11.1, Oct 18, 2025; 32 dependents | Runtime values only, so units are not checked by the compiler |
| [undo](https://crates.io/crates/undo) | Undo and redo with a command pattern | 0.52.0, Mar 8, 2025; 7 dependents | No release since Mar 2025 |
| [undo\_2](https://crates.io/crates/undo_2) | Undo and redo with a history tree | 0.2.1, Dec 12, 2024; 1 dependent | No release since Dec 2024; hosted on GitLab so CI was not probed |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| uom | 4 | 5 | 4 | 3.7 | 4 | 5 | **84** |
| measurements | 3 | 5 | 2 | 4.7 | 3 | 5 | **73** |
| dimensioned | 1 | 5 | 3 | 4.7 | 2 | 5 | **66** |
| undo | 2 | 5 | 3 | 4 | 1.5 | 5 | **66** |
| undo\_2 | 2 | 3 | 2 | 3.7 | 1 | 5 | **53** |

`uom` (84) is the only well-founded unit crate and fits the "typed values" rule, but millimetres, degrees and mm/min as in-house newtypes may be lighter and keep third-party types out of public APIs. The undo crates (66 and 53) have almost no users; the architecture's command API (5.7) is small enough to write directly, with the crates as a reference for history trees.

## Release and repository tooling

The architecture's repository section (12) names release tools but marks them as reasoned picks, not scored ones, and asks for them to be checked against NFR-14. This section does that, and adds the tools for the `xtask` checks and the licence notices that shipped binaries need (NFR-10).

### Release tooling

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [cargo-release](https://crates.io/crates/cargo-release) | Local command that bumps versions, updates the changelog, tags and publishes; supports workspaces with a shared version | 1.1.6, Sep 16, 2026; 10 releases in 12 months | Part of the crate-ci organisation. Local and manual, which suits the lockstep rule before 1.0 |
| [release-plz](https://crates.io/crates/release-plz) | Opens a release pull request with version bumps and changelog, then tags and publishes on merge | 0.3.171, Oct 10, 2026; 24 releases in 12 months | Pre-1.0 version numbers; AGENTS.md in its repository; works best with conventional commits |
| [cargo-dist](https://crates.io/crates/cargo-dist) | Builds binaries and installers and generates the release workflow; now branded `dist` | 0.32.0, May 22, 2026 on crates.io; its repository changelog lists 0.33.0 on Sep 10, 2026 | The crates.io release is almost four months behind the repository; platform packaging such as a macOS app bundle is still ours |
| [git-cliff](https://crates.io/crates/git-cliff) | Generates a changelog from commit history using templates | 2.14.2, Sep 18, 2026; 4 dependents | Needs a commit convention to produce useful output; pairs with either release tool |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| git-cliff | 5 | 4 | 4 | 4.3 | 1.5 | 5 | **80** |
| cargo-release | 5 | 5 | 4 | 3.3 | 1.5 | 5 | **79** |
| release-plz | 5 | 3 | 4 | 4.3 | 1.5 | 3 | **73** |
| cargo-dist | 4 | 3 | 4 | 3.7 | 1.5 | 5 | **70** |

These are tools that people run, so they all score low on adoption (1.5) and the ranking mostly reflects project health. `cargo-release` (79) is the best match to the lockstep, maintainer-controlled releases the architecture describes, and `release-plz` (73) is the later option it names. The architecture says `cargo-dist`'s maintenance state is unconfirmed: it has 6 releases in 12 months on crates.io and CI workflows, and the repository is a version ahead of crates.io, so it is active (70). `git-cliff` (80) is not a rival to the others; it only writes the changelog. If the maintenance weight is raised sharply, `cargo-release` takes first place from it.

### Repository checks and licence notices

| Crate | What it gives you | Status seen | Catch |
| --- | --- | --- | --- |
| [cargo\_metadata](https://crates.io/crates/cargo_metadata) | Parses `cargo metadata` output into Rust types | 0.23.1, Nov 12, 2025; 1,195 dependents | What the `xtask` dependency-rule check reads. CLAUDE.md in its repository, but it is nearly ten years old and widely used |
| [cargo-hack](https://crates.io/crates/cargo-hack) | Runs a command over each workspace member or feature combination | 0.6.45, May 30, 2026 | Fits the rule that each library builds and tests on its own without the C++ toolchain |
| [cargo-about](https://crates.io/crates/cargo-about) | Generates a third-party licence notice file from dependency licences | 0.9.2, Aug 18, 2026 | Same organisation as cargo-deny; configured with an allow list like deny.toml |
| [cargo-vet](https://crates.io/crates/cargo-vet) | Records audits of third-party crates and checks that each dependency has been reviewed | 0.10.2, Jan 13, 2026 | Mozilla project; heavy process for a small team, and the package shows few tests |

Scores for the crates above:

| Crate | Maintenance (20) | Maturity (15) | Features (20) | Quality (20) | Adoption (15) | AI risk (10) | Total /100 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| cargo\_metadata | 3 | 5 | 5 | 4.7 | 5 | 4 | **89** |
| cargo-about | 5 | 4 | 4 | 3.7 | 2 | 5 | **79** |
| cargo-hack | 4 | 4 | 4 | 4.3 | 2 | 5 | **77** |
| cargo-vet | 3 | 3 | 3 | 3 | 1.5 | 5 | **60** |

`cargo_metadata` (89) is the sound way to write the dependency-rule check in `xtask`. `cargo-about` (79) is worth adding to the release gate so binaries ship with their licence notices; `cargo-hack` (77) turns the "each library builds on its own" rule into a CI job. `cargo-vet` (60) is optional and adds ongoing review work.

Facts behind these scores, collected the same way as the evidence tables below:

| Crate | Age (yrs) | Last release | Releases in 12 mo | Dependent crates | Tests per 1k lines | Doc-comment % | CI workflow | Agent files | Licence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| cargo-release | 10.6 | Sep 2026 | 10 | 22 | 3.8 | 3.0 | yes | none | MIT/Apache-2.0 |
| release-plz | 4.7 | Oct 2026 | 24 | 1 | 19.8 | 4.3 | yes, plus extras | AGENTS.md | MIT/Apache-2.0 |
| cargo-dist | 4.1 | May 2026 | 6 | 6 | 2.2 | 15.4 | yes, plus extras | none | MIT/Apache-2.0 |
| git-cliff | 5.3 | Sep 2026 | 7 | 4 | 9.7 | 7.5 | yes, plus extras | none | MIT/Apache-2.0 |
| cargo\_metadata | 9.7 | Nov 2025 | 1 | 1,195 | 7.9 | 22.9 | yes, plus extras | CLAUDE.md | MIT |
| cargo-hack | 6.9 | May 2026 | 6 | 0 | 11.5 | 1.9 | yes, plus extras | none | Apache-2.0/MIT |
| cargo-about | 6.9 | Aug 2026 | 5 | 1 | 8.0 | 3.6 | yes | none | MIT/Apache-2.0 |
| cargo-vet | 4.6 | Jan 2026 | 1 | 0 | 0.1 | 9.4 | yes, plus extras | none | Apache-2.0/MIT |

## Evidence behind the scores

These are the measured facts for the 42 crates scored in the first pass (the 110 supporting crates follow in a second table, and the eight release and repository tools have a table in their own section), read from crates.io, each crate's published source package and each project's GitHub files on Oct 10, 2026. Age is years since the first crates.io release; dependents are crates on crates.io that depend on it; tests per 1,000 lines counts test attributes in the packaged source; doc-comment % is the share of non-blank source lines that are doc comments.

| Crate | Age (yrs) | Last release | Releases in 12 mo | Dependent crates | Tests per 1k lines | Doc-comment % | CI workflow | Agent files | Licence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| opencascade | 3.7 | Aug 2026 | 1 | 0 | 6.8 | 2.8 | yes | none | LGPL-2.1 |
| cadrum | 0.5 | Oct 2026 | 41 | 1 | 13.7 | 16.5 | yes | AGENTS.md, CLAUDE.md | MIT |
| truck-stepio | 4.4 | Sep 2024 | 0 | 3 | 5.8 | 2.9 | not located | none | Apache-2.0 |
| monstertruck | 0.6 | Sep 2026 | 9 | 1 | 0.0 | 41.0 | yes | AGENTS.md, CLAUDE.md | Apache-2.0 |
| ruststep | 5.0 | Sep 2024 | 0 | 2 | 2.4 | 5.3 | yes, plus extras | none | Apache-2.0 |
| stl\_io | 9.3 | Mar 2026 | 4 | 42 | 17.7 | 17.2 | yes, plus extras | none | MIT |
| nom\_stl | 5.9 | Mar 2021 | 0 | 0 | 17.5 | 7.5 | defunct service | none | MIT |
| mesh-loader | 4.5 | Oct 2024 | 0 | 5 | 0.8 | 4.4 | yes, plus extras | none | Apache-2.0 |
| parry3d | 5.7 | Sep 2026 | 15 | 38 | 2.9 | 32.5 | yes | none | Apache-2.0 |
| halfedge | 0.3 | Sep 2026 | 9 | 0 | 21.3 | 11.4 | yes, plus extras | AI-tool dirs in exclude list | MIT/Apache-2.0 |
| csgrs | 1.7 | Jul 2025 | 0 | 3 | 6.7 | 16.3 | yes | AGENTS.md | MIT |
| procedural\_modelling | 2.6 | Jun 2026 | 4 | 0 | 4.0 | 9.7 | yes | none | MIT/Apache-2.0 |
| wgpu | 7.7 | Aug 2026 | 8 | 1765 | 0.4 | 12.7 | yes, plus extras | AGENTS.md, CLAUDE.md | MIT/Apache-2.0 |
| egui\_wgpu | 5.5 | Sep 2026 | 10 | 118 | 0.7 | 15.2 | yes | none | MIT/Apache-2.0 |
| bevy | 6.7 | Oct 2026 | 13 | 2096 | 0.1 | 5.3 | yes, plus extras | none | MIT/Apache-2.0 |
| kiss3d | 11.9 | Oct 2026 | 15 | 50 | 0.2 | 16.2 | yes | none | BSD-3-Clause |
| three-d | 6.6 | Apr 2026 | 1 | 18 | 0.0 | 15.0 | yes | none | MIT |
| rend3 | 5.6 | Feb 2022 | 0 | 9 | 0.2 | 9.9 | yes, plus extras | none | MIT/Apache-2.0/Zlib |
| egui / eframe | 6.4 | Sep 2026 | 10 | 1271 | 1.2 | 23.5 | yes | none | MIT/Apache-2.0 |
| iced | 7.4 | Dec 2025 | 1 | 396 | 0.4 | 34.2 | yes, plus extras | none | MIT |
| slint | 4.7 | Sep 2026 | 10 | 106 | 14.0 | 13.4 | yes, plus extras | AGENTS.md, CLAUDE.md | GPL-3.0 / Slint royalty-free / Slint commercial |
| tauri | 6.9 | Oct 2026 | 26 | 704 | 2.7 | 22.2 | yes | none | Apache-2.0/MIT |
| g-code | 5.7 | Mar 2026 | 1 | 3 | 12.9 | 7.8 | not located | none | MIT/Apache-2.0 |
| gcode | 8.2 | Apr 2026 | 3 | 2 | 18.0 | 14.9 | yes | none | MIT/Apache-2.0 |
| cnccoder | 3.3 | Apr 2024 | 0 | 0 | 5.1 | 14.3 | yes | none | MIT/Apache-2.0 |
| ngc | 7.4 | Mar 2022 | 0 | 0 | 1.2 | 7.9 | defunct service | none | MIT/Apache-2.0 |
| cavalier\_contours | 5.6 | Aug 2026 | 3 | 2 | 8.3 | 10.9 | yes | AGENTS.md | MIT/Apache-2.0 |
| i\_overlay | 3.1 | Oct 2026 | 25 | 38 | 25.2 | 5.7 | yes | none | MIT/Apache-2.0 |
| clipper2 | 2.6 | May 2026 | 2 | 6 | 14.9 | 33.4 | yes | none | MIT/Apache-2.0 |
| clipper2-rust | 0.7 | Sep 2026 | 6 | 4 | 24.0 | 6.5 | yes | CLAUDE.md | BSL-1.0 |
| spade | 10.0 | Mar 2026 | 1 | 40 | 10.9 | 14.4 | yes | none | MIT/Apache-2.0 |
| geo | 11.7 | Apr 2026 | 3 | 405 | 21.3 | 17.6 | yes, plus extras | AGENTS.md | MIT/Apache-2.0 |
| dxf | 10.1 | Oct 2026 | 2 | 12 | 15.7 | 1.5 | yes | none | MIT |
| usvg | 8.5 | Aug 2026 | 4 | 337 | 0.0 | 8.8 | yes | none | Apache-2.0/MIT |
| glam | 7.4 | Oct 2026 | 21 | 1450 | 0.3 | 9.4 | yes, plus extras | none | MIT/Apache-2.0 |
| glamx | 0.8 | Sep 2026 | 7 | 39 | 19.9 | 12.1 | yes | none | MIT/Apache-2.0 |
| nalgebra | 11.9 | May 2026 | 3 | 2094 | 7.2 | 17.4 | yes | none | Apache-2.0 |
| rstar | 7.9 | May 2026 | 1 | 180 | 9.7 | 26.9 | yes, plus extras | none | MIT/Apache-2.0 |
| robust | 6.8 | May 2025 | 0 | 50 | 4.0 | 1.5 | yes | none | MIT/Apache-2.0 |
| rayon | 10.8 | Apr 2026 | 1 | 11566 | 13.0 | 19.5 | yes, plus extras | none | MIT/Apache-2.0 |
| mesh\_to\_sdf | 2.7 | Sep 2024 | 0 | 1 | 8.3 | 14.9 | yes, plus extras | none | MIT/Apache-2.0 |
| rs-opw-kinematics | 2.5 | Sep 2026 | 14 | 0 | 14.0 | 8.3 | yes, plus extras | none | BSD-3-Clause |

The test counts for glam, bevy, wgpu, tauri, iced, egui, egui\_wgpu, usvg and slint are unreliable guides to their real testing, and their test scores were set by hand, as explained in the scoring section. Licences are those of the latest stable release on crates.io; check them before shipping closed-source.

### Evidence for the supporting crates

The same measures for the 110 crates scored in the sections added on Oct 10, 2026 before the release tooling section, read from crates.io, each published source package and each project's GitHub files that day. Tests per 1,000 lines for the crates listed under "Test scores set by hand" in the scoring section are unrepresentative.

| Crate | Age (yrs) | Last release | Releases in 12 mo | Dependent crates | Tests per 1k lines | Doc-comment % | CI workflow | Agent files | Licence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| brepkit-io | 0.2 | Oct 2026 | 189 | 1 | 14.1 | 13.9 | yes, plus extras | CLAUDE.md | AGPL-3.0-only |
| serde | 11.8 | Jul 2026 | 1 | 129,067 | 0.1 | 23.7 | yes | none | MIT/Apache-2.0 |
| serde\_json | 11.2 | Oct 2026 | 7 | 108,195 | 7.5 | 15.4 | yes | none | MIT/Apache-2.0 |
| toml | 11.9 | Oct 2026 | 21 | 19,961 | 1.0 | 9.0 | yes | none | MIT/Apache-2.0 |
| toml\_edit | 8.8 | Oct 2026 | 23 | 1,804 | 1.9 | 15.1 | yes | none | MIT/Apache-2.0 |
| schemars | 7.2 | Jul 2026 | 5 | 6,353 | 15.0 | 9.1 | yes | none | MIT |
| jsonschema | 6.5 | Oct 2026 | 67 | 1,454 | 7.8 | 8.9 | yes, plus extras | none | MIT |
| indexmap | 8.7 | Sep 2026 | 7 | 6,192 | 11.2 | 21.8 | yes | none | Apache-2.0/MIT |
| ron | 9.2 | Jun 2026 | 3 | 1,003 | 19.7 | 3.9 | yes, plus extras | none | MIT/Apache-2.0 |
| kdl | 5.8 | May 2026 | 4 | 134 | 10.8 | 7.5 | yes, plus extras | AGENTS.md | Apache-2.0 |
| json5 | 8.2 | Feb 2026 | 5 | 330 | 5.8 | 4.5 | none found | none | MIT |
| serde\_yaml | 10.6 | Mar 2024 | 0 | 7,762 | 7.5 | 12.2 | yes | none | MIT/Apache-2.0 |
| uuid | 11.9 | Oct 2026 | 17 | 21,927 | 15.7 | 32.5 | yes | none | Apache-2.0/MIT |
| blake3 | 7.1 | Aug 2026 | 5 | 3,946 | 7.9 | 10.9 | yes | none | CC0-1.0/Apache-2.0/Apache-2.0 w/ LLVM exc. |
| sha2 | 10.4 | Mar 2026 | 4 | 21,066 | 0.7 | 1.4 | not located | none | MIT/Apache-2.0 |
| slotmap | 8.3 | Dec 2025 | 2 | 519 | 2.2 | 40.9 | none found | none | Zlib |
| ulid | 9.3 | Jul 2026 | 3 | 795 | 28.7 | 30.6 | yes | none | MIT |
| xxhash-rust | 6.0 | Sep 2026 | 4 | 939 | 4.7 | 6.3 | yes | none | BSL-1.0 |
| fractional\_index | 5.1 | Sep 2024 | 0 | 4 | 16.6 | 13.7 | yes | none | MIT |
| nanoid | 8.8 | Apr 2026 | 1 | 340 | 34.9 | 43.1 | yes | none | MIT |
| lexorank | 3.5 | Apr 2023 | 0 | 0 | 39.1 | 0.0 | none found | none | non-standard |
| minijinja | 5.0 | Oct 2026 | 19 | 744 | 1.6 | 22.0 | yes, plus extras | none | Apache-2.0 |
| tera | 10.5 | Sep 2026 | 16 | 864 | 7.2 | 8.0 | yes | none | MIT |
| handlebars | 11.8 | Aug 2026 | 5 | 825 | 19.8 | 9.0 | yes, plus extras | none | MIT |
| liquid | 11.6 | Feb 2025 | 0 | 73 | 0.0 | 10.1 | yes | none | MIT/Apache-2.0 |
| upon | 4.5 | Jul 2026 | 1 | 26 | 2.8 | 13.3 | yes | none | MIT/Apache-2.0 |
| askama | 9.6 | Sep 2026 | 9 | 514 | 6.7 | 24.4 | yes, plus extras | none | MIT/Apache-2.0 |
| mlua | 6.9 | Oct 2026 | 7 | 496 | 11.5 | 11.0 | yes, plus extras | none | MIT |
| rhai | 10.6 | Sep 2026 | 7 | 350 | 1.8 | 24.1 | yes, plus extras | AGENTS.md | MIT/Apache-2.0 |
| starlark | 8.0 | Jun 2026 | 3 | 32 | 10.9 | 8.7 | yes | none | Apache-2.0 |
| rquickjs | 6.6 | Sep 2026 | 7 | 211 | 7.4 | 13.9 | yes, plus extras | none | MIT |
| boa\_engine | 4.6 | Aug 2026 | 3 | 98 | 8.0 | 16.6 | yes, plus extras | none | Unlicense/MIT |
| typst | 3.6 | Jul 2026 | 8 | 157 | 0.0 | 20.0 | yes, plus extras | none | Apache-2.0 |
| printpdf | 9.2 | Sep 2026 | 16 | 101 | 5.2 | 7.9 | yes, plus extras | none | MIT |
| krilla | 2.1 | Jun 2026 | 5 | 28 | 1.0 | 13.6 | yes | none | MIT/Apache-2.0 |
| pdf-writer | 5.4 | May 2026 | 1 | 66 | 2.0 | 26.4 | yes | none | MIT/Apache-2.0 |
| genpdf | 6.0 | Jun 2021 | 0 | 25 | 3.2 | 29.1 | not probed (off GitHub) | none | Apache-2.0/MIT |
| git2 | 11.9 | May 2026 | 3 | 2,028 | 8.7 | 16.7 | yes, plus extras | none | MIT/Apache-2.0 |
| gix | 3.7 | Oct 2026 | 18 | 456 | 0.9 | 19.1 | yes, plus extras | AGENTS.md | MIT/Apache-2.0 |
| curvo | 2.5 | Oct 2026 | 27 | 2 | 3.6 | 8.0 | yes | none | MIT |
| brepkit-math | 0.2 | Oct 2026 | 207 | 11 | 16.9 | 10.9 | yes, plus extras | CLAUDE.md | AGPL-3.0-only |
| enterpolation | 5.1 | May 2025 | 0 | 15 | 4.7 | 28.6 | yes | none | MIT/Apache-2.0 |
| splines | 8.2 | Apr 2025 | 0 | 13 | 14.0 | 30.0 | not probed (off GitHub) | none | BSD-3-Clause |
| truck-geometry | 5.8 | Sep 2024 | 0 | 4 | 3.9 | 30.4 | not located | none | Apache-2.0 |
| monstertruck-geometry | 0.6 | Sep 2026 | 9 | 7 | 7.5 | 18.5 | yes | AGENTS.md, CLAUDE.md | Apache-2.0 |
| bspline | 11.0 | Feb 2022 | 0 | 2 | 7.0 | 11.8 | yes | none | MIT |
| pyo3 | 9.2 | Sep 2026 | 11 | 2,252 | 18.5 | 14.7 | yes, plus extras | none | MIT/Apache-2.0 |
| maturin | 7.1 | Aug 2026 | 22 | 1 | 8.2 | 9.2 | yes, plus extras | AGENTS.md | MIT/Apache-2.0 |
| numpy | 9.4 | Jun 2026 | 4 | 386 | 12.7 | 21.7 | yes, plus extras | none | BSD-2-Clause |
| uniffi | 6.1 | Sep 2026 | 7 | 233 | 5.0 | 14.6 | not located | none | MPL-2.0 |
| cbindgen | 9.5 | Jun 2026 | 4 | 450 | 0.2 | 3.2 | not located | none | MPL-2.0 |
| safer-ffi | 6.4 | Jan 2026 | 1 | 24 | 0.9 | 11.6 | yes | none | MIT |
| diplomat | 5.6 | Aug 2026 | 4 | 10 | 12.5 | 1.6 | yes, plus extras | none | MIT/Apache-2.0 |
| interoptopus | 5.3 | Oct 2026 | 35 | 14 | 8.3 | 28.9 | yes, plus extras | CLAUDE.md | MIT |
| thiserror | 7.0 | Sep 2026 | 4 | 73,088 | 24.1 | 10.8 | yes | none | MIT/Apache-2.0 |
| anyhow | 7.0 | Jul 2026 | 4 | 50,928 | 14.5 | 19.8 | yes | none | MIT/Apache-2.0 |
| snafu | 7.7 | Jul 2026 | 3 | 997 | 31.0 | 23.8 | yes | none | MIT/Apache-2.0 |
| miette | 5.2 | Apr 2025 | 0 | 1,571 | 3.4 | 24.0 | yes | AGENTS.md | Apache-2.0 |
| eyre | 6.7 | Aug 2026 | 2 | 1,651 | 13.8 | 33.0 | yes, plus extras | none | MIT/Apache-2.0 |
| tracing | 8.9 | Dec 2025 | 3 | 44,554 | 2.6 | 4.6 | yes, plus extras | none | MIT |
| log | 11.8 | Aug 2026 | 6 | 31,309 | 15.2 | 26.8 | yes | none | MIT/Apache-2.0 |
| tracing-subscriber | 7.3 | Mar 2026 | 3 | 19,576 | 10.1 | 28.4 | yes, plus extras | none | MIT |
| env\_logger | 11.7 | Jun 2026 | 3 | 12,081 | 8.0 | 33.8 | yes | none | MIT/Apache-2.0 |
| proptest | 9.3 | Oct 2026 | 4 | 11,495 | 10.1 | 14.7 | yes, plus extras | none | MIT/Apache-2.0 |
| insta | 7.7 | Oct 2026 | 15 | 4,756 | 15.8 | 9.0 | yes, plus extras | none | Apache-2.0 |
| approx | 10.9 | Feb 2026 | 2 | 2,654 | 62.9 | 11.6 | yes, plus extras | none | Apache-2.0 |
| rstest | 8.0 | Sep 2026 | 1 | 3,432 | 55.2 | 22.9 | yes, plus extras | CLAUDE.md | MIT/Apache-2.0 |
| assert\_cmd | 8.4 | May 2026 | 7 | 5,332 | 3.8 | 45.1 | yes | none | MIT/Apache-2.0 |
| arbitrary | 9.4 | Oct 2026 | 1 | 1,003 | 11.6 | 27.6 | yes | none | MIT/Apache-2.0 |
| similar | 5.7 | Aug 2026 | 5 | 1,190 | 10.4 | 13.1 | yes, plus extras | none | Apache-2.0 |
| tempfile | 11.5 | Mar 2026 | 4 | 36,042 | 15.9 | 40.2 | yes, plus extras | none | MIT/Apache-2.0 |
| quickcheck | 11.9 | Feb 2026 | 1 | 1,553 | 23.4 | 9.5 | yes | none | Unlicense/MIT |
| expect-test | 6.1 | Dec 2024 | 0 | 335 | 8.8 | 21.4 | yes | none | MIT/Apache-2.0 |
| criterion | 8.9 | Feb 2026 | 3 | 18,794 | 2.7 | 11.5 | yes, plus extras | none | Apache-2.0/MIT |
| cargo-fuzz | 9.6 | Jun 2026 | 1 | 7 | 11.1 | 4.8 | yes | none | MIT/Apache-2.0 |
| libfuzzer-sys | 7.1 | Jun 2026 | 3 | 38 | 0.0 | 56.7 | yes | none | (MIT/Apache-2.0) AND NCSA |
| divan | 3.3 | Apr 2025 | 0 | 497 | 6.6 | 26.1 | yes | none | MIT/Apache-2.0 |
| iai-callgrind | 3.6 | Jul 2025 | 0 | 120 | 0.7 | 48.0 | yes, plus extras | AGENTS.md | Apache-2.0/MIT |
| cargo-deny | 7.4 | Jul 2026 | 17 | 5 | 1.4 | 2.7 | yes | none | MIT/Apache-2.0 |
| cargo-nextest | 4.7 | Oct 2026 | 48 | 11 | 1.3 | 7.6 | yes, plus extras | AGENTS.md, CLAUDE.md | Apache-2.0/MIT |
| cargo-semver-checks | 4.2 | Oct 2026 | 7 | 6 | 8.3 | 7.4 | yes, plus extras | AGENTS.md | Apache-2.0/MIT |
| cargo-llvm-cov | 5.7 | Sep 2026 | 16 | 28 | 6.1 | 3.9 | yes, plus extras | none | Apache-2.0/MIT |
| cargo-audit | 9.7 | Jun 2026 | 3 | 11 | 1.2 | 10.7 | not located | none | Apache-2.0/MIT |
| mdbook | 11.2 | Jul 2026 | 7 | 169 | 25.4 | 3.2 | yes | none | MPL-2.0 |
| clap | 11.6 | Sep 2026 | 21 | 48,218 | 0.7 | 35.3 | yes | none | MIT/Apache-2.0 |
| indicatif | 9.5 | Jul 2026 | 6 | 5,450 | 12.0 | 12.3 | yes, plus extras | none | MIT |
| argh | 7.3 | Mar 2026 | 6 | 506 | 23.5 | 24.6 | yes, plus extras | none | BSD-3-Clause |
| lexopt | 5.2 | Feb 2026 | 1 | 183 | 10.5 | 21.0 | yes | none | MIT |
| bpaf | 4.6 | Sep 2026 | 8 | 102 | 3.2 | 36.3 | none found | none | MIT/Apache-2.0 |
| crossbeam-channel | 8.9 | Sep 2026 | 2 | 2,673 | 22.4 | 14.2 | yes, plus extras | none | MIT/Apache-2.0 |
| flume | 7.2 | Dec 2025 | 1 | 957 | 19.3 | 2.8 | yes | none | Apache-2.0/MIT |
| atomic-write-file | 3.1 | Aug 2026 | 1 | 75 | 4.4 | 36.9 | yes | none | BSD-3-Clause |
| notify | 11.8 | Aug 2026 | 5 | 3,088 | 3.8 | 12.9 | yes, plus extras | none | CC0-1.0 |
| rfd | 5.9 | Jan 2026 | 4 | 653 | 1.2 | 6.3 | yes | none | MIT |
| directories | 8.8 | Jan 2025 | 0 | 3,008 | 5.6 | 38.1 | yes | none | MIT/Apache-2.0 |
| dirs | 10.9 | Sep 2026 | 1 | 10,834 | 4.9 | 50.7 | not probed (off GitHub) | none | MIT/Apache-2.0 |
| opener | 8.2 | Sep 2026 | 3 | 151 | 19.3 | 7.5 | yes, plus extras | none | MIT/Apache-2.0 |
| fs-err | 6.7 | Oct 2026 | 6 | 473 | 0.0 | 23.4 | yes | none | MIT/Apache-2.0 |
| egui\_dock | 4.2 | Aug 2026 | 7 | 27 | 0.7 | 20.2 | yes | AGENTS.md, CLAUDE.md | MIT |
| egui\_tiles | 3.4 | Aug 2026 | 6 | 32 | 4.2 | 12.9 | yes | none | MIT/Apache-2.0 |
| egui\_plot | 3.0 | Aug 2026 | 5 | 127 | 0.3 | 14.5 | yes | none | MIT/Apache-2.0 |
| egui\_extras | 4.6 | Sep 2026 | 10 | 316 | 0.9 | 11.1 | yes | none | MIT/Apache-2.0 |
| bytemuck | 7.0 | Jul 2026 | 3 | 4,151 | 8.5 | 28.9 | yes | none | Zlib/Apache-2.0/MIT |
| meshopt | 8.0 | Oct 2025 | 0 | 26 | 3.0 | 12.2 | none found | none | MIT/Apache-2.0 |
| egui\_kittest | 1.8 | Sep 2026 | 11 | 130 | 10.2 | 16.8 | yes | none | MIT/Apache-2.0 |
| uom | 9.6 | Feb 2026 | 1 | 198 | 14.8 | 10.2 | none found | none | Apache-2.0/MIT |
| dimensioned | 11.7 | Apr 2022 | 0 | 17 | 4.2 | 13.7 | yes, plus extras | none | MIT/Apache-2.0 |
| measurements | 11.3 | Oct 2025 | 1 | 32 | 55.4 | 13.6 | yes | none | MIT |
| undo | 9.8 | Mar 2025 | 0 | 7 | 2.7 | 18.1 | yes | none | MIT/Apache-2.0 |
| undo\_2 | 3.9 | Dec 2024 | 0 | 1 | 5.5 | 36.7 | not probed (off GitHub) | none | MIT/Apache-2.0 |

## Gaps and risks

1. **No 4th-axis planner, post-processor or material-removal simulator crate exists.** These are the parts of the project that are original work.
2. **STEP means OpenCASCADE, and even that scores only 66.** The mature route needs a C++ toolchain, and the opencascade crate lists LGPL-2.1; cadrum bundles OCCT too. Check the licence obligations if you plan to ship closed-source.
3. **Fast API churn.** parry3d went from 0.26 to 0.31 in eight months, wgpu from 26 to 30 in 12 months, Bevy from 0.17 to 0.20 in 12 months, and i\_overlay from 5.0 to 9.0 in five months; egui ships a breaking release every two to three months. Pin exact versions and isolate each crate behind your own interface.
4. **Young crates with visible AI-tool use.** cadrum and monstertruck (both first published in 2026, 41 and 9 releases in the last 12 months, each with CLAUDE.md and AGENTS.md), halfedge (AI-tool directories in its package exclude list) and clipper2-rust (CLAUDE.md) are the crates your criteria penalise most. If you use one, put it behind a trait you own and plan to swap it. truck-stepio is the opposite case: no release since Sep 2024.
5. **Agent-instruction files also appear in established projects.** wgpu, geo, cavalier\_contours and Slint have them; the scores treat that as a mild signal for projects over five years old, and you should decide whether your own policy treats it as a risk.
6. **Precision.** parry3d builds f32 by default, while CAM tolerances usually want f64; use parry3d-f64 or check how your queries behave at your tolerance.
7. **Licences.** ivacam is GPL-3, so read it for ideas rather than copying code unless your project is also GPL. Slint is GPL-3.0, royalty-free or commercial, opencascade is LGPL-2.1, and clipper2-rust is BSL-1.0.
8. **What this survey did not do.** It rests on crates.io records, published source packages and GitHub files read on Oct 10, 2026. GitHub's API was blocked, so there is no commit, contributor or issue data. Nothing was compiled or benchmarked, and the features scores are my judgment, so a one-day spike per layer is the real test.

Added with the supporting-crate sections:

- **Licences to decide on.** brepkit-io and brepkit-math are AGPL-3.0-only, which suits the AGPL library start (INT-09) but rules out a later closed-source or dual-licensed host. cbindgen, uniffi and mdbook are MPL-2.0 (file-level copyleft; build and documentation tools). xxhash-rust is BSL-1.0, like clipper2-rust; blake3 and notify are CC0-1.0 (alone or as one option); libfuzzer-sys adds NCSA and is dev-only; lexorank's licence field reads "non-standard". Everything else in the new sections is MIT, Apache-2.0, BSD or Zlib. Put each of these in the cargo-deny allow list on purpose, not by default.
- **Thin spots in the supporting layers.** Order keys (PRJ-05) have one small crate; NURBS surface evaluation has no crate proven for point, normal and closest-point queries; undo has two tiny crates; PDF output has one good simple option. For these the advice is to write a small in-house module and keep a crate as a reference.
- **Young and fast-moving picks.** minijinja reached 3.0 on Oct 8, 2026; jsonschema made 67 releases in 12 months; pyo3, toml\_edit and egui\_tiles are pre-1.0; notify has a 9.0 release candidate. Pin versions as with the first-pass crates.
- **Agent-instruction files among the supporting crates.** brepkit-io and brepkit-math (0.2 years old), monstertruck-geometry, interoptopus, egui\_dock, rhai, miette, kdl, gix, maturin and rstest, plus the dev tools cargo-nextest, cargo-semver-checks and iai-callgrind. Only the first three, all under a year old, score 1 on AI risk; the dev tools never ship.
- **Nothing was built.** Build cost matters for NFR-13 and is untested: mlua, rquickjs and git2 compile C code, typst and boa\_engine are large, and the tools-only crates need a nightly toolchain (cargo-fuzz) or Valgrind (iai-callgrind).

## Sources

Read in full on Oct 10, 2026 (all versions, dates and scores come from these):

- crates.io records (versions, release dates, owners, dependents, downloads, licences) for every crate in the evidence table, at https://crates.io/crates/NAME; for example [stl\_io](https://crates.io/crates/stl_io), [parry3d](https://crates.io/crates/parry3d), [cavalier\_contours](https://crates.io/crates/cavalier_contours), [opencascade](https://crates.io/crates/opencascade) and [cadrum](https://crates.io/crates/cadrum)
- The published source package of each of those crates from static.crates.io, used for the test, doc-comment and agent-file counts
- Raw files from each project's GitHub repository (README, workflow files, CLAUDE.md, AGENTS.md and similar), probed by path
- [ivacam README](https://github.com/aalarchiv/ivacam)

* The same crates.io records and source packages for the 118 supporting crates, read on Oct 10, 2026
* [gitoxide crate-status.md](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md), read as a raw file to see which Git workflows gix has not yet completed
* [brepkit](https://github.com/andymai/brepkit) README and repository files (STEP and format table, CLAUDE.md, CI workflows)

Read as search-result excerpts only (pages not opened):

- STEP and kernels: [opencascade-rs](https://github.com/bschwind/opencascade-rs), [truck](https://github.com/ricosjp/truck), [ruststep](https://docs.rs/ruststep), [Foxtrot](https://github.com/Formlabs/foxtrot), [Fornjot](https://github.com/hannobraun/fornjot)
- Mesh and geometry: [nom\_stl](https://lib.rs/crates/nom_stl), [mesh-loader](https://docs.rs/mesh-loader/latest/mesh_loader/stl/index.html), [halfedge](https://docs.rs/crate/halfedge/latest/source/README.md), [csgrs](https://docs.rs/crate/csgrs/latest), [spade](https://github.com/Stoeoef/spade), [parry glam migration](https://github.com/dimforge/parry/pull/401)
- Rendering and UI: [wgpu changelog](https://newreleases.io/project/github/gfx-rs/wgpu/release/v29.0.1), [Bevy 0.19](https://gamedev.net/news/4045-bevy-019-released/), [kiss3d](https://docs.rs/kiss3d), [three-d](https://lib.rs/crates/three-d), [egui\_wgpu](https://docs.rs/egui-wgpu), [iced 0.14](https://www.linux.org/threads/phoronix-iced-0-14-released-for-popular-rust-cross-platform-gui-library.59755/latest), [Slint](https://lib.rs/crates/slint), [Frame 0.30.0](https://github.com/66HEX/frame/releases/tag/0.30.0), [CADmium](https://github.com/WC3D/CADmium), [Outline](https://github.com/AndersonJ293/outline)
- G-code and CAM: [CopperCrab](https://github.com/jnthbdn/CopperCrab), [cnccoder](https://docs.rs/cnccoder), [ngc](https://lib.rs/crates/ngc), [GitHub CNC topic (Rust)](https://github.com/topics/cnc?l=rust&o=desc&s=forks), [GitHub CAM topic](https://github.com/topics/cam?o=desc&s=updated)
- Rotary prior art and kinematics: [RotatoCAM](https://github.com/revjmoney/RotatoCAM), [cnc\_4th\_axis\_sim](https://github.com/SergioReyesSan/cnc_4th_axis_sim), [CaseMaker issue 223](https://github.com/SlyWombat/CaseMaker/issues/223), [rs-opw-kinematics](https://docs.rs/rs-opw-kinematics)
