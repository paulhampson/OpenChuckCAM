# OpenChuckCAM

A Rust CAM application that turns a 3D model into G-code for true 4-axis (XYZA) CNC machining. The first reference machine is the Makera Z1, with a LinuxCNC post used for verification.

Generated G-code must be verified by the user before it is run on a machine (NFR-12).

**Status:** early development (work breakdown slice 0: workspace, shared data and guardrails). Nothing here machines anything yet.

## Documents

The documents in [docs/](docs/README.md) are the source of truth: requirements, architecture, work breakdown, verification plan, crate selection, the dependency register and the decision records. Contributors and coding agents start with [AGENTS.md](AGENTS.md).

## Layout

| Path | Contents |
| --- | --- |
| `crates/` | Library crates. The reusable library family is `openchuck-types`, `openchuck-path`, `openchuck-post` and `openchuck-gcode`. `openchuck-testkit` is test-only. |
| `apps/` | `openchuck-cli` and `openchuck-ui` (wiring only). |
| `posts/` | Built-in post definitions (from slice 6). |
| `testdata/` | Verification corpus and its size policy. |
| `fuzz/` | Fuzz targets (separate workspace, nightly toolchain). |
| `benches/` | Cross-crate benchmarks. |
| `xtask/` | Repository checks. |

## Building and checking

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask all          # dependency rules, licences, dependency records, test data, traceability
cargo deny check
cargo run -p openchuck-cli -- regenerate <project-folder>
```

The toolchain is pinned in `rust-toolchain.toml`.

## Licence

AGPL-3.0-or-later for every crate. See [LICENSES/](LICENSES/).
