# OpenChuckCAM

OpenChuckCAM is a CAM program: you give it a 3D model, and it works out the tool movements and writes the G-code that a CNC machine reads. It is for **true 4-axis (XYZA) machining**, where the work is held in a chuck and turned by a rotary axis (called A) while the cutter moves along and around it. The first machine it supports is the **Makera Z1**; a LinuxCNC post is also included and is used to check the output.

![The OpenChuckCAM window on its Operations step: the finishing toolpath of the tutorial's baluster drawn in orange around the part on the rotary axis.](user-docs/src/images/hero-1-toolpaths.png)

![The OpenChuckCAM window on its Simulate step: the simulated cut part in 3D, coloured by how far each point is from the model.](user-docs/src/images/hero-2-deviation.png)

*The real program, with the example piece from the tutorial. Top: the toolpath of a finishing operation. Bottom: the simulated cut part, coloured by deviation from the model (grey is on the surface; light blue still has material left).*

> **Read this first.** OpenChuckCAM is in early development. Generated G-code **must be verified by you before it is run on a machine** (NFR-12). OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup, and it has not been approved for any machine. Very little has been run on a real machine: see the [known limitations](https://paulhampson.github.io/OpenChuckCAM/latest/start/limitations.html).

## The user guide

The guide for people who use the program is at **<https://paulhampson.github.io/OpenChuckCAM/>**. Its source is in [`user-docs/`](user-docs/README.md).

- [Install and first run](https://paulhampson.github.io/OpenChuckCAM/latest/start/install.html): a development build from the rolling [`nightly` pre-release](https://github.com/paulhampson/OpenChuckCAM/releases/tag/nightly) (Linux, Windows, macOS on Apple silicon; unsigned), or build from source. There is no stable release yet.
- [Your first project](https://paulhampson.github.io/OpenChuckCAM/latest/tutorial/index.html): one part from a model file to G-code and a setup sheet, with a screenshot of every step. The same pages are in this repository in [`user-docs/src/tutorial/`](user-docs/src/tutorial/index.md); the model is [`testdata/examples/baluster`](testdata/examples/baluster/README.md). The speeds and feeds in the tutorial are an example only, not a tested cutting recipe.
- [Known limitations](https://paulhampson.github.io/OpenChuckCAM/latest/start/limitations.html): what the program does not do, and what has and has not been run on a machine. Read it before you rely on the program.

In short, you can import an STL or STEP model, describe the machine, the stock and how it is held, add roughing and finishing operations, preview the cut in 3D, and write G-code for the Makera Z1 (or the LinuxCNC verification post) with a setup sheet. It is 4-axis rotary work only, it does not check the chuck, the tailstock or the machine body for collisions, and not every requirement is built.

## Getting help

Questions, bugs and requests: [GitHub issues](https://github.com/paulhampson/OpenChuckCAM/issues). Say which build you used, what you did and what you expected. The program writes a local log file that never leaves your computer; the guide says [where it is](https://paulhampson.github.io/OpenChuckCAM/latest/start/install.html#where-the-program-keeps-its-log).

## Licence

AGPL-3.0-or-later for every crate. See [LICENSES/](LICENSES/).

## For contributors

The documents in [docs/](docs/README.md) are the source of truth. Start with [docs/README.md](docs/README.md), [AGENTS.md](AGENTS.md) (also for coding agents) and [CONTRIBUTING.md](CONTRIBUTING.md). Contributors sign the [contributor licence agreement](CLA.md) once. Work is planned by lane in the work breakdown's "Parallel work" section; read it before starting.

### Status

Early development. The work is planned as numbered slices in the [work breakdown](docs/work-breakdown.md), whose slice status table and "Parallel work" section are the authority on what is done and which lanes can run in parallel. The project format (`schema` in every project file) is still changing and is not stable until the first release.

### Documents

The documents in [docs/](docs/README.md) are the source of truth: [requirements](docs/requirements.md), [architecture](docs/architecture.md), [work breakdown](docs/work-breakdown.md), [verification plan](docs/verification-plan.md), the [crate selection](docs/crate-selection.md), the [dependency register](docs/dependencies.md), the [open items](docs/open-items.md) and the [decision records](docs/decisions/). For the Z1, see the [reference jobs](docs/reference-jobs.md), the [air-cut procedure](docs/z1-air-cut-procedure.md) and the [safe-practice checklist](docs/z1-safe-practice-checklist.md). Contributors and coding agents start with [AGENTS.md](AGENTS.md).

### Layout

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

### Building and running

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

### Checking

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run        # or: cargo test --workspace
cargo xtask all          # dependency rules, licences, dependency records, test data, traceability
cargo deny check
```

`cargo xtask golden` audits the golden G-code. `cargo xtask replay --docker IMAGE` runs the LinuxCNC programs in the simulator (gate 3; see [tools/linuxcnc](tools/linuxcnc/README.md)). Each library builds and tests alone, for example `cargo test -p openchuck-path`. Long tests are listed in `.config/nextest.toml` and run nightly.
