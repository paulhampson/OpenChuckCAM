# OpenChuckCAM

A Rust CAM application that turns a 3D model into G-code for true 4-axis (XYZA) CNC machining. The first reference machine is the Makera Z1, with a LinuxCNC post used for verification.

Generated G-code must be verified by the user before it is run on a machine (NFR-12). Nothing here has been run on a machine beyond one attended air cut for the Z1's rotary conventions ([decision 0007](docs/decisions/0007-sp-12-z1-conventions.md)); the first cuts of generated programs are still to come ([work breakdown](docs/work-breakdown.md), slice 20).

## Status

Early development. The work is planned as numbered slices in the [work breakdown](docs/work-breakdown.md), which also lists the lanes that can be built in parallel and what each needs; that file is the authority on what is done. In short:

- **Built and closed (slices 0 to 7):** workspace and guardrails; project files with undo; STL import and mesh checks; machine, stock and setup; the tool library; the pathing core with helical finishing; post-processing for the Makera Z1 and LinuxCNC with a G-code reader and replay; material removal simulation.
- **Built since, on `main`:** the egui and wgpu viewport and application shell (slice 8); the background job pipeline and its cache (slice 9, headless parts); selection sets and region constraints on meshes (slice 10, headless parts); STEP import through an isolated worker process (slice 11a); axial and indexed finishing (slice 12); simultaneous 4-axis finishing (slice 13); rotary roughing (slice 14, contained to selected regions); rest machining and pencil clean-up (slice 15); the workholding setup data and the composite target for retaining tabs (slice 21, parts a and b); output grouping, comments and the setup sheet (slice 19, without the export dialogs); the entry moves (plunge, lead, ramp and helix).
- **Still to come:** picking and the selection panel, toolpath drawing and playback, more strategies (edge following, swarf, drilling), the application wiring and interface for workholding and retaining tabs, a redesigned interface, and release hardening. Several are in review.

The project format (`schema` in every project file) is still changing and is not stable until the first release.

## Documents

The documents in [docs/](docs/README.md) are the source of truth: [requirements](docs/requirements.md), [architecture](docs/architecture.md), [work breakdown](docs/work-breakdown.md), [verification plan](docs/verification-plan.md), the [crate selection](docs/crate-selection.md), the [dependency register](docs/dependencies.md), the [open items](docs/open-items.md) and the [decision records](docs/decisions/). For the Z1, see the [reference jobs](docs/reference-jobs.md), the [air-cut procedure](docs/z1-air-cut-procedure.md) and the [safe-practice checklist](docs/z1-safe-practice-checklist.md). Contributors and coding agents start with [AGENTS.md](AGENTS.md).

## Layout

| Path | Contents |
| --- | --- |
| `crates/openchuck-types` | Shared plain data: units, meshes, B-rep faces, stock, tools, machine profile, the neutral toolpath IR, kinematics. |
| `crates/openchuck-path` | The reusable 4-axis pathing library: strategies, A-axis management, linking, entry moves, gouge protection, feed intent. |
| `crates/openchuck-post` | Post-processing from the IR to controller G-code, with user-editable template posts. The built-in post definitions (the Makera Z1 and the LinuxCNC verification post) are in its `posts/` folder (decision 0103). |
| `crates/openchuck-gcode` | A reader and replay for the G-code the posts emit, used for checks and round trips. |
| `crates/openchuck-geom` | Mesh checks, transforms and selection algorithms. |
| `crates/openchuck-import` | Model readers: STL, and STEP through the OpenCASCADE worker (optional `kernel` feature). |
| `crates/openchuck-sim` | Material removal simulation (dexel stock) and deviation reports. |
| `crates/openchuck-project` | The project folder: files, IDs, order keys, schema migration, the tool library. |
| `crates/openchuck-app` | The application layer: commands, undo, jobs, caches, the pipeline. |
| `crates/openchuck-viewport` | The wgpu viewport. Only this crate and `openchuck-ui` use egui or wgpu. |
| `crates/openchuck-testkit` | Test-only: analytic parts, the independent kinematic oracle and gouge checker. |
| `apps/` | `openchuck-cli` and `openchuck-ui` (wiring only). |
| `testdata/` | Verification corpus, golden G-code and its size policy. |
| `fuzz/` | Fuzz targets (separate workspace, nightly toolchain). |
| `benches/` | Cross-crate benchmarks, including the 1M-triangle viewport model. |
| `tools/`, `docker/` | The LinuxCNC simulator replay (gate 3) and its image. |
| `xtask/` | Repository checks. |

The library family (`openchuck-types`, `-path`, `-post`, `-gcode`) depends on nothing outside itself, so it can be used by another host.

## Building and running

The toolchain is pinned in `rust-toolchain.toml`.

```
cargo build --workspace
cargo run --release -p openchuck-ui -- [PROJECT_FOLDER]
cargo run -p openchuck-cli -- --help
```

The command line works without a window: `new`, `import`, `setup`, `machine`, `tool`, `library`, `validate`, `report`, `inspect`, `regenerate` (writes the G-code), `simulate`, `tool-numbers` and the placement commands `transform` and `units`. Nightly builds of both programs for Linux, Windows and macOS are produced when `main` changes ([decision 0023](docs/decisions/0023-nightly-builds.md)); they are unsigned.

STEP import needs the OpenCASCADE kernel, which is not built by default and takes a long time the first time. See [crates/openchuck-import](crates/openchuck-import/README.md):

```
cargo build -p openchuck-import --features kernel --bins
```

## Checking

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run        # or: cargo test --workspace
cargo xtask all          # dependency rules, licences, dependency records, test data, traceability
cargo deny check
```

`cargo xtask golden` audits the golden G-code. `cargo xtask replay --docker IMAGE` runs the LinuxCNC programs in the simulator (gate 3; see [tools/linuxcnc](tools/linuxcnc/README.md)). Each library builds and tests alone, for example `cargo test -p openchuck-path`. Long tests are listed in `.config/nextest.toml` and run nightly.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Contributors sign the [contributor licence agreement](CLA.md) once. Work is planned by lane in the work breakdown's "Parallel work" section; read it before starting.

## Licence

AGPL-3.0-or-later for every crate. See [LICENSES/](LICENSES/).
