# Fuzzing: user files, targets and CPU hours

NFR-07 says invalid input gives a clear error, never a crash or a hang, for every file the user can supply. The verification plan (section 4.1) asks for 1 minute of fuzzing per parser on pull requests, 1 hour nightly, and no unresolved crash before a release; 24 cumulative CPU hours per parser is an advisory target, not a release condition ([decision 0107](decisions/0107-malformed-input-sweep-and-fuzz-hours.md) item 7). The nightly runs and the running total below are kept, and the release report gives each parser's total. This page lists every user-supplied format with its fuzz target, and keeps the running total of CPU hours per parser. How the hours are counted is [decision 0107](decisions/0107-malformed-input-sweep-and-fuzz-hours.md) item 5.

The libFuzzer targets are in `fuzz/fuzz_targets/`, a separate cargo-fuzz workspace on the nightly toolchain. The pull request job is `fuzz-smoke` in `.github/workflows/ci.yml` (groups A, B and C, one minute per target; decision 0052, Option 7). The nightly job is `fuzz` in `.github/workflows/nightly.yml` (one hour per target, the corpus kept between nights in the Actions cache). A failing night opens an issue that only a pull request closes (decision 0102 item 5).

## Inventory

Before pushing a public library API change, compile the separate fuzz workspace with `cargo check --locked --all-targets --manifest-path fuzz/Cargo.toml` as well as benchmark consumers with `cargo check --locked --all-targets --manifest-path benches/Cargo.toml`. Workspace-only checks do not compile `fuzz/`; a missing `PostJob` field stopped group C before its smoke run in PR 284. These compile checks supplement the nightly-toolchain fuzz runs.

| Format | Parser (entry point) | Fuzz target | Pull request | Nightly | Corpus seeds |
| --- | --- | --- | --- | --- | --- |
| STL model (IMP-01) | `openchuck_import::import_bytes`, then `check_mesh` and `place` | `stl_import` | Group B | Yes | `testdata/stl/*.stl` |
| STEP model (IMP-02) | `StepImporter` (the OpenCASCADE reader in a child process, decision 0003) | Mutation run, not libFuzzer: `crates/openchuck-import/examples/step_mutations.rs` (decision 0027 item 9, OI-27) | 150 mutants in the STEP kernel job | 3,000 mutants, seeded by the run number | `testdata/step/valid/` (4 files); `testdata/step/broken/` and `crash/` in the import tests |
| Project file, one at a time, with schema and migration checks (PRJ-01, PRJ-04) | `openchuck_project::file::parse_file` | `project_files` | Group B | Yes | The fixture files in `crates/openchuck-project/tests/fixtures/`, each after the byte that picks its kind (from decision 0107; none before) |
| Project folder, with the checks across files (PRJ-01, PRJ-05, PRJ-06) | `openchuck_project::file::load_files`, the checks of `file::load` | `project_folder` (decision 0107) | Group A | Yes (decision 0107) | The fixture folders in `crates/openchuck-project/tests/fixtures/`, each written as `=== <path>` blocks |
| Recovery checkpoint container (PRJ-08, NFR-07) | `openchuck_project::recovery::Snapshot::decode`, then project validation; fuzz both raw envelopes and checksummed arbitrary inner payloads | `project_recovery` | Group B | Yes | `fuzz/seeds/project_recovery/`; local 60-second smoke is not counted as nightly CPU hours |
| Selection sets (PRJ-06) | In project files: kind 4 of `project_files`, and the set checks of `load` | `project_files`, `project_folder` | As above | As above | As above |
| Workholding and tabs (SET-09, OP-21) | In setup files | `project_files`, `project_folder` | As above | As above | As above |
| Machine profile file (SET-01) | `openchuck_project::machines::parse_profile`, with a write and read back | `machine_profile_toml` | Group A | Yes | `crates/openchuck-project/machines/*.toml` |
| Machine profile, serialised form | `serde_json` into `MachineProfile`, then `validate` | `machine_profile_json` | Group A | Yes | None |
| Tool library tool file and machine tool set (TL-01, TL-11) | `openchuck_project::tools::parse_tool_file`, `parse_tool_set`, with a write and read back | `tool_library_toml` | Group B | Yes | `testdata/tools/library/`, `testdata/tools/library/tool-sets/`, `testdata/tools/broken/` |
| Custom tool profile (TL-03) | `openchuck_project::tools::parse_profile`, then the monotone rule and a write and read back | `tool_profile_toml` | Group C | Yes | `testdata/tools/profiles/` |
| Post definition and template (GC-03) | `PostDefinition::from_parts`, then a render of a small toolpath with `TemplatePost::post` (from decision 0107) | `post_definition` | Group C | Yes (decision 0107) | `crates/openchuck-post/posts/*/` (`post.toml`, a line `---`, `main.post`) |
| G-code program (GC-08, the `gcode-sim` and replay commands) | `openchuck_gcode::read`, then `replay` in both dialects | `gcode_reader` | Group C | Yes (decision 0107) | `testdata/gcode/`, `testdata/golden/post_*.ngc`, `testdata/golden/reference/` |
| Toolpath IR, serialised (GC-01) | `serde_json` into `Toolpath`, then `validate` and `total_duration` | `toolpath_ir_json` | Group A | Yes | None |
| Export acknowledgement, `settings.toml` (GC-16) | `openchuck_app::acknowledgement::Acknowledgements` | None: a corpus test only (OI-327) | No | No | `req_gc_16_a_bad_settings_file_asks_again_with_a_message_and_is_kept_and_other_keys_survive` |
| Tool library folder (TL-01) | `ToolLibrary::open`: each file through the parsers above, then lookups by ID | The per-file target `tool_library_toml` | Group B | Yes | As above |
| DXF and SVG (IMP-06) | Not built | None yet | | | |

Not user files under NFR-07: the LinuxCNC simulator report read by `gcode-sim --report` (`openchuck_gcode::parse_sim_report`, written by the verification driver), the worker's reply to the STEP importer (written by our own child process), and the in-memory pipeline cache (OI-131).

## CPU hours per parser

Updated from the nightly runs on `main` by a session with the Actions API (decision 0107 item 5): at each slice close, before a release and at least every 60 days. Pull request smoke minutes and local runs are not counted. A night whose job failed counts zero.

| Target | Nights counted | CPU hours | Unresolved crashes | Last counted night |
| --- | --- | --- | --- | --- |
| `stl_import` | 6 (12 to 17 Oct 2026) | 6.0 | 0 | 17 Oct 2026, run 39011834775 |
| `project_files` | 6 | 6.0 | 0 | 17 Oct 2026 |
| `project_folder` | 0 (new) | 0.0 | 0 | |
| `project_recovery` | 0 (new, CHECKPOINT) | 0.0 | Not yet measured | |
| `machine_profile_toml` | 6 | 6.0 | 0 | 17 Oct 2026 |
| `machine_profile_json` | 6 | 6.0 | 0 | 17 Oct 2026 |
| `tool_library_toml` | 4 (14 to 17 Oct; 13 Oct failed) | 4.0 | 0 (13 Oct crash fixed in 070e7ee) | 17 Oct 2026 |
| `tool_profile_toml` | 5 (13 to 17 Oct) | 5.0 | 0 | 17 Oct 2026 |
| `post_definition` | 0 (not in the nightly job before decision 0107) | 0.0 | 0 | |
| `gcode_reader` | 0 (not in the nightly job before decision 0107) | 0.0 | 0 | |
| `toolpath_ir_json` | 6 | 6.0 | 0 | 17 Oct 2026 |
| STEP mutation run | 3 runs (15 to 17 Oct) of the same 3,000 mutants | 0.06 distinct (0.17 run) | 0 | 17 Oct 2026 |

Nights before decision 0107 had no CPU annotation; they are counted at 60 minutes for each successful fuzz step (the steps took 60.5 to 61.6 minutes, the build included). The STEP mutation steps took 2.9, 4.0 and 3.5 minutes.

## Crashes found

| Date | Target | Input | Cause | Fixed |
| --- | --- | --- | --- | --- |
| 13 Oct 2026 (run 38376224534) | `tool_library_toml` | A tool set whose `machine` string, written back, had a line starting with seven vertical bars | The reader looked for Git merge conflict markers before parsing, so its own output read as a conflict | 070e7ee, 13 Oct 2026: markers are looked for only when the TOML does not parse; regression test `req_nfr_07_tool_set_with_marker_like_machine_name_round_trips` in `crates/openchuck-project/src/tools/mod.rs` |
