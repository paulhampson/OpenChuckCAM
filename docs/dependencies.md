# Dependency records

The NFR-14 record for every third-party crate the workspace declares. Each row gives the exact pinned version, the crate survey's score out of 100 (as of 10 Oct 2026, [crate-selection.md](crate-selection.md)), the licence, where the crate may be used and why it was chosen. Architecture section 7 names the picks; this register records their adoption.

`cargo xtask records` fails CI when a crate in `[workspace.dependencies]` (or the separate fuzz workspace) is not pinned with `=`, has no row here, or is pinned to a different version than its row. `cargo deny` checks the licences of the whole tree, including transitive crates that have no row of their own.

## Policy

- **Pins.** Every third-party version is an exact `=` pin, declared once in the root `Cargo.toml` and inherited with `workspace = true` (architecture section 12). `Cargo.lock` is committed.
- **Adding a crate.** Add the pin, a row here with the survey score (or a survey-style score if the survey did not cover it) and a justification. A crate outside the architecture's selection, a crate under two years old, or one with agent-instruction files needs a decision record with its fallback (NFR-14).
- **Upgrade cadence.** Dependency updates arrive as reviewed pull requests, never silently. Review pins once a month, and within a week of a security advisory. Fast-moving crates (egui and eframe, wgpu, parry3d, i\_overlay, minijinja) are upgraded deliberately, one family per pull request, with the spike or adapter tests for that layer re-run; their types stay behind our interfaces so an upgrade touches one crate.
- **Licences.** Must be compatible with AGPL-3.0-or-later (NFR-10). `deny.toml` lists the licences allowed on purpose.

## Register

| Crate | Version | Score | Licence | Used in | Justification |
| --- | --- | --- | --- | --- | --- |
| `serde` | 1.0.229 | 93 | MIT OR Apache-2.0 | All crates | Serialisation for plain data across boundaries (INT-02, GC-01). Architecture section 7, supporting layers. |
| `serde_json` | 1.0.152 | 97 | MIT OR Apache-2.0 | Tests and fuzzing now; `openchuck-project` from slice 1 | JSON text form of shared data. Built with `float_roundtrip`, because without it a double does not always survive a save and load exactly (found by the IR round-trip property test; NFR-05, NFR-06). |
| `thiserror` | 2.0.21 | 94 | MIT OR Apache-2.0 | Libraries and `openchuck-app` | Typed errors in libraries (architecture section 7, errors and logging). |
| `anyhow` | 1.0.104 | 96 | MIT OR Apache-2.0 | `openchuck-cli`, `xtask` | Error reporting in applications only. |
| `rayon` | 1.12.0 | 96 | MIT OR Apache-2.0 | `openchuck-app` job manager, `openchuck-testkit` | Worker pool under the determinism rules (architecture section 8). |
| `blake3` | 1.8.7 | 93 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | `openchuck-testkit` now; `openchuck-project` content hashes later | Output hashes in the determinism harness. CC0-1.0 is allowed on purpose (architecture section 7). |
| `clap` | 4.6.7 | 97 | MIT OR Apache-2.0 | `openchuck-cli`, `xtask` | Command-line parsing. |
| `cargo_metadata` | 0.23.1 | 89 | MIT | `xtask` | Reads the dependency graph for the dependency-rule check (architecture section 12). |
| `toml` | 1.1.8 | 92 | MIT OR Apache-2.0 | `xtask` | Reads manifests and the xtask configuration. `toml_edit` (93) stays the choice for hand-edited project files (SP-8). |
| `eframe` | 0.36.2 | 90 | MIT OR Apache-2.0 | `openchuck-ui` only | egui application shell with the wgpu backend. Version tried in SP-2 (decision record 0002, outstanding item 6). Pulls in `egui` and `egui-wgpu` 0.36.2 (87), which have no direct entries. Breaking releases every two to three months: upgrade deliberately. Linked from slice 0 so CI proves the platform builds (NFR-02). |
| `wgpu` | 30.0.1 | 88 | MIT OR Apache-2.0 | `openchuck-viewport` (and `openchuck-ui` through eframe) | Native graphics on each platform (NFR-02, NFR-03). Version tried in SP-2. The repository carries agent-instruction files (AI-risk flag in the survey); accepted because it is the only cross-platform native graphics layer egui supports, it is maintained by the gfx-rs organisation with many dependents, and the fallback (Tauri or iced) is recorded in decision record 0002. Its types stay inside `openchuck-viewport`. |
| `proptest` | 1.12.0 | 100 | MIT OR Apache-2.0 | Dev-dependency | Property tests for invariants and round trips (verification plan section 8). |
| `insta` | 1.49.0 | 94 | Apache-2.0 | Dev-dependency | Snapshot tests of serialised data (verification plan section 8). Plain-text golden G-code uses `openchuck-testkit::golden` so reviews show plain diffs. |
| `assert_cmd` | 2.2.2 | 88 | MIT OR Apache-2.0 | Dev-dependency of `openchuck-cli` | CLI end-to-end scenarios (verification plan section 8). |
| `criterion` | 0.8.2 | 88 | Apache-2.0 OR MIT | Dev-dependency of `openchuck-benches` | Benchmarks and the 10% regression flag (verification plan section 4.1). |
| `libfuzzer-sys` | 0.4.13 | 79 | (MIT OR Apache-2.0) AND NCSA | `fuzz/` only (separate workspace, nightly) | Fuzz targets through cargo-fuzz (82). NCSA is allowed on purpose for this dev-only crate (architecture section 7). |

## Recorded but not yet declared

These were tried in a spike and are recorded now so the slice that first uses them only adds the pin.

| Crate | Version tried | Score | Licence | For | Notes |
| --- | --- | --- | --- | --- | --- |
| `parry3d-f64` | 0.31.1 | 83 (as parry3d) | Apache-2.0 | `openchuck-geom`, `openchuck-path`, `openchuck-sim` | SP-2 used it for picking. Its public types are `glamx` types, which must stay behind `openchuck-geom` (decision record 0002). Highest API churn of the CAM-critical crates. |
| `bytemuck` | 1.25.0 | 96 | Zlib OR Apache-2.0 OR MIT | `openchuck-viewport` | Vertex buffer casts (SP-2). |
| `gcode` | 0.7.0 | 75 | MIT OR Apache-2.0 | `openchuck-gcode` (test reader) | SP-3 found it stores numbers as `f32`; the reader decision is open until slice 6 (decision record 0001, open item 5). |
