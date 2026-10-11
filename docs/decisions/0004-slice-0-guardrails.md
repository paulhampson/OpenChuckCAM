# 0004: Slice 0 design refinement, guardrails and shared data

- **Status:** proposed, with decision items 2 and 11 and the contributor agreement accepted by the project owner on 11 Oct 2026 and applied to the architecture (see "Owner decisions"). The other choices are what slice 0 builds on.
- **Date:** 11 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md) (OI-16, OI-17, OI-19 to OI-21, OI-23 and OI-24; OI-10, OI-15, OI-18 and OI-22 are closed).

## Context

Work breakdown slice 0 (guardrails and shared data) asks its design refinement to confirm the monorepo and crate layout, fix the IR fields after SP-3, and define the job context, error model and dependency policy. Requirements: NFR-01, NFR-02, NFR-05, NFR-06, NFR-07, NFR-08, NFR-09, NFR-10, NFR-14 and GC-01. Architecture sections 3, 4, 5.1, 5.7, 7, 8, 9 and 12. Verification plan sections 4.1, 7 and 8.

Spike inputs, used as the project owner asked even though they are not accepted:

- **0001 (SP-3, proposed):** IR fields (item 1), A sign and rotary offset as profile data (item 2), feed intent (item 3) and the oracle design (item 6). Its open items stay open; in particular the simulator run (item 1) and the Z1 conventions (item 3) are not resolved here, so the IR is **not frozen** by this record.
- **0002 (SP-2, proposed):** egui, eframe and wgpu versions (item 6), the three-platform build (item 2), and keeping `parry3d-f64` types behind `openchuck-geom`.
- **SP-1 (STEP kernel)** was in progress when this record was written. It has since been decided by [decision 0003](0003-step-kernel-route.md) (accepted): a thin in-house C ABI wrapper over OpenCASCADE in `openchuck-import`, with STEP parsing in an isolated child process. Its slice 0 consequences are applied here (see "SP-1 follow-up" below); the wrapper itself is slice 11 work, so `openchuck-import` still links no kernel and no CI job builds C++ yet.

## Decision

1. **Layout.** The monorepo follows architecture section 12: `crates/`, `apps/`, `posts/`, `testdata/`, `fuzz/`, `benches/`, `xtask/`, with every crate in section 3 except `openchuck-vcs` (P2). Library crates other than `openchuck-types` and `openchuck-app` are shells.
2. **Two crates not in architecture section 3** (accepted, applied to architecture sections 3, 9 and 12):
   - `openchuck-testkit` (`publish = false`) holds the independent kinematic oracle, the determinism harness, golden-file comparison and the analytic parts. The oracle and harness are used by several crates' tests (slices 3, 5, 6 and 13), which a module inside one crate cannot serve. `cargo xtask deps` fails if any crate takes it as a normal dependency. It depends only on `openchuck-types`, so the library family can keep it as a dev-dependency after a split, or a split can copy it.
   - `openchuck-benches` is the package behind `benches/` (cross-crate benchmarks).
3. **IR (GC-01), from 0001 item 1.** `Toolpath` is a list of `ToolpathGroup`s in machining order. A group carries the tags (operation, setup, tool, as opaque strings the application maps to its stable IDs), a start pose and its segments; segments inherit the group's tags rather than repeating them. Each segment carries the end pose (tool tip and unit tool axis in the model frame, unwrapped A in degrees), the move kind (rapid, linking, cutting, plunge, lead-in, lead-out), the contact feed (required for fed moves, absent for rapids), the duration in seconds and the contact path length. Validation rejects non-finite values, negative durations and lengths, a fed move without a feed and a rapid with one. Deserialising rejects unknown fields and a tool axis whose length is not one.
4. **Machine profile (SET-01, SET-06), from 0001 item 2.** Rotary axis as an enum (X only; Y under SET-02), the A sense (`positive` or `negative`), the rotary origin (machine coordinates of the model origin at A = 0; its Y and Z are the rotary axis offset), the A range (continuous or limited), linear travel limits, maximum feeds including rapids, and a clearance cylinder. `MachineProfile::example_x_rotary` is for tests and describes no real machine.
5. **Kinematic conventions.** The model frame's X axis is the rotary axis and its origin lies on it. `machine_xyz = rotary_origin + R(sign * A) * tip` and `tip = R(-sign * A) * (machine_xyz - rotary_origin)`, with `R` the right-handed rotation about the rotary axis. The tool axis is machine +Z. These live once in `openchuck_types::kinematics`.
6. **Oracle (architecture section 9, 0001 item 6).** Polar form in `openchuck-testkit::oracle`, reading only plain numbers from the profile. Its tests cover 19 A angles from -1080 to 1234.5 degrees including the 0/360 seam, a zero offset, a flipped A sign (more than 1 mm error) and a wrong offset, at the 1e-6 mm limit of verification plan section 4.1.
7. **Error model.** Libraries return typed `thiserror` errors whose messages name the field path and the problem (`invalid groups[0].segments[2].contact_feed: a fed move needs a contact feed`). Applications use `anyhow`. Clippy denies `unwrap`, `expect`, `panic`, `todo` and unchecked indexing outside tests; `unsafe_code` is denied. The job manager catches a panic in a job and reports it as an error, because rayon aborts the process on a panic in a spawned task.
8. **Job context (INT-04, UI-09).** `CancelToken` (cooperative, shared by clones), `ProgressSink` and `JobContext` live in `openchuck-types`. The `openchuck-app` job manager owns one rayon pool sized by `Threads`, reports start, progress and finish on the event stream, and is the only place threads are created.
9. **Determinism (NFR-06, GC-07).** Clippy denies `HashMap` and `HashSet` (a non-output use may be allowed locally with a reason). The harness runs a job at 1, 2 and N threads (N is the larger of the core count and 4) and twice at N, comparing BLAKE3 hashes. `serde_json` is built with `float_roundtrip`: the IR round-trip property test showed a double does not always survive a JSON save and load without it.
10. **Dependency policy (NFR-14).** Exact `=` pins declared once in `[workspace.dependencies]`; a register in `docs/dependencies.md` with score, licence and justification; monthly review and within a week of an advisory; fast-moving crates upgraded one family per pull request. `cargo xtask records` enforces pins, single declaration and register rows. `predicates` was left out because the survey did not cover it.
11. **Licence identifier** (accepted). Every crate and file uses the SPDX expression `AGPL-3.0-or-later`.
12. **Docs location.** Recorded as decided in architecture section 13: the authoritative documents are in `docs/`.
13. **Test-data size policy.** No file in `testdata/` above 1 MiB and at most 20 MiB in total, checked by `cargo xtask testdata`. Analytic parts and the 1M-triangle benchmark model are generated by `openchuck-testkit::parts`.
14. **Traceability.** Tests are named `req_<id>_..._<what>`, for example `req_op_05_op_06_gc_09_oracle_agrees_with_shared_model`. `cargo xtask trace` reads requirement IDs and priorities from `docs/requirements.md` (section 6 `SUG-` rows excluded), slices from the work breakdown traceability table, and closed slices and late-closing requirements from `xtask/traceability.toml`. It fails on an unknown ID, a P1 requirement no slice covers, or a P1 requirement with no test once its last slice is closed. No slice is closed yet.
15. **Project placeholder.** Until slice 1 defines the project format (SP-8), a project is a folder and an existing folder opens as an empty project. The CLI's `validate`, `regenerate` and `report` run end to end on it and print the NFR-12 notice on `regenerate`.
16. **Platform builds (0002 item 2).** `openchuck-ui` is a shell that opens an eframe window with the wgpu backend, and `openchuck-viewport` links wgpu, so the three-platform CI builds the SP-2 stack from slice 0.

## Owner decisions, 11 Oct 2026

- Decision items 2 and 11 accepted; the architecture is edited to match.
- **Contributor agreement:** a standard agreement that supports relicensing later. `CLA.md` (individual) and `CLA-ENTITY.md` (organisations) adapt the Apache ICLA v2.2 and CCLA, the most widely adapted CLAs. Their copyright grant includes the right to sublicense, which lets the Maintainer add a licence exception or dual licence (INT-09); section 2 says so explicitly, and section 9 commits that contributions stay available under AGPL-3.0-or-later or another OSI-approved or FSF-recognised licence. Signing is by pull request comment. The Harmony agreements were the other candidate; their site was unreachable from the working environment, and the Apache text is the more familiar to contributors. Follow-ups: OI-22 (signature collection) and OI-23 (legal review).
- **Fonts:** OFL-1.1 and Ubuntu-font-1.0 accepted for egui's bundled fonts (OI-15 closed).
- **Dedicated runners deferred** (OI-17).
- **Activity check:** accepted as recommended: a weekly `cargo-unmaintained` job, Dependabot update pull requests checked against the register, and `cargo deny` advisories (OI-18 closed).
- **CLA signatures** are collected with CLA Assistant Lite (OI-22 closed; follow-up OI-24).

## SP-1 follow-up, 11 Oct 2026

Decision 0003 lists consequences for slice 0. Applied in this change:

- **CI:** `CMAKE_POLICY_VERSION_MINIMUM=3.5` is set for every CI job, because CMake 4 on the Windows and macOS runners rejects OpenCASCADE 7.8.1's `cmake_minimum_required`. The kernel build job itself, with a per-platform build cache, a path filter so only `openchuck-import` rebuilds it, `TKernel` linked first and a clean-exit test, is added with the wrapper in slice 11 (OI-20).
- **Dependency rule:** the xtask STEP-kernel rule already names `occt-sys`, `opencascade` and `opencascade-sys`, so no `-sys` crate can reach the library family or any crate but `openchuck-import`; a unit test now covers `occt-sys`.
- **NFR-14 records** for `occt-sys`, `opencascade`/`opencascade-sys`, `cadrum` and `brepkit-io` are in `docs/dependencies.md` (tried, rejected or watched; none declared).
- **B-rep data:** the `openchuck-types::brep` documentation now states the decided numbering (`TopExp_Explorer` order, assigned by the importer) and that zero faces is an importer error. Surface parameters stay out of the shared types until SP-9 decides how exact evaluation reaches `openchuck-path`.
- **Open items** from decision 0003 are added to the register (OI-25 to OI-29), and OI-14, OI-19 and OI-20 are updated.

## Alternatives considered

- **Oracle and harness as modules inside `openchuck-types`, behind a feature.** Rejected: test code would ship in the published library and a feature could be enabled by accident. A module in one crate's `tests/` cannot be shared.
- **Tags on every segment.** Rejected for size: the work breakdown asks for tags to operation, tool and setup, and a group supplies them to every segment it holds.
- **Contact feed as a plain number with zero for rapids.** Rejected: an absent feed on a fed move must be an error, not a zero.
- **`insta` for golden G-code.** Kept for serialised data snapshots; golden G-code uses plain files through `openchuck-testkit::golden` so reviews show plain diffs (architecture section 12).

## Consequences

- New: `docs/dependencies.md` (register), a row for it in `docs/README.md`, and this record in the index.
- Architecture sections 3, 9, 12 and 13 and work breakdown slice 0 item 2 are edited for items 2 and 11 and the contributor agreement.
- New: `CLA.md`, `CLA-ENTITY.md`, `CONTRIBUTING.md` and `docs/open-items.md`.
- No rule exception and no `xtask` allow-list entry. `deny.toml` carries one crate-scoped licence exception (open item 1).

## Open items

The register in [docs/open-items.md](../open-items.md) tracks the current state; this table is the record as raised, with the register ID added.

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 (OI-15) | **egui's bundled fonts.** `epaint_default_fonts` (pulled in by eframe's `default_fonts`) is `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`. `deny.toml` allows OFL-1.1 and Ubuntu-font-1.0 for that crate only, pending review. Options: accept (fonts are data shipped alongside, not linked code), or drop `default_fonts` and bundle a font under a licence already allowed. | Before the first binary release; slice 8 at latest | Project owner's licence decision |
| 2 (OI-C2, closed) | Add `openchuck-testkit` and `openchuck-benches` to the architecture (decision item 2). | Review of this record | Agreement |
| 3 (OI-C3, closed) | `AGPL-3.0-or-later` or `AGPL-3.0-only` (decision item 11). | Review of this record | Project owner |
| 4 (OI-C4, closed) | **Contributor agreement** allowing a later licence exception or dual licence (INT-09; work breakdown slice 0 item 14). Not decided here. A DCO alone would not give the project the right to relicense; a CLA or a copyright assignment would. | Before outside contributions | Project owner |
| 5 (OI-16) | **Reserve the `openchuck-*` names on crates.io** (slice 0 item 7). This publishes placeholder crates, so it is the owner's action. | Slice 0 close | Project owner |
| 6 (OI-17, deferred) | **Benchmark regression flag at 10%** needs a dedicated runner (verification plan section 4.1). The criterion baseline exists (`benches/`), but no workflow runs it yet. | Slice 0 close or when a runner exists | A self-hosted runner |
| 7 (OI-18, proposed) | **Scheduled dependency activity check** (verification plan section 7): no tool chosen. Candidates: a small xtask over the crates.io index, or a scheduled `cargo outdated` report. | Slice 0 close | Choice of tool (NFR-14 record) |
| 8 (OI-19) | **Miri and sanitizers** have nothing to check: `unsafe_code` is denied and there is no FFI. Add the job with the first `unsafe` or FFI code: the OpenCASCADE wrapper in slice 11 (decision 0003). | Slice 11 | The wrapper |
| 9 (OI-20) | **STEP kernel CI job**, path-filtered so a change to `docs/` or `posts/` does not rebuild it (slice 0 checklist). The `changes` job in `ci.yml` has the filter structure; the job is added with the wrapper in slice 11, as decision 0003 specifies. | Slice 11 | The wrapper result |
| 10 (OI-10, closed) | **Three-platform CI has not run yet.** Everything was built and tested on Linux only. The first pull request runs Windows and macOS (0002 outstanding item 2). | First pull request | CI run |
| 11 (OI-21) | **IR not frozen.** 0001 open items 1 (simulator run), 3 (Z1 A sign and offset), 4 (feed rule on mixed moves) and 7 (tilted tools, off-top contact) may still change the IR or profile fields. | Slices 3, 5 and 6 | 0001 open items |

## Slice 0 review checklist status

| Checklist item | State |
| --- | --- |
| Each dependency rule, including the library-family rule, fails CI when deliberately broken | Done: xtask unit tests break each rule on a synthetic graph; `cargo xtask deps` passes on the real workspace |
| Each library builds and tests alone with no C++ toolchain; a change to `docs/` or `posts/` does not rebuild the STEP kernel | Library part done (`libraries-alone` CI job). The kernel arrives with the slice 11 wrapper (decision 0003; open item 9, OI-20) |
| The IR carries every field listed and round-trips through serde | Done, including a property test that doubles survive exactly |
| The CLI runs on an empty project and gives clear errors for bad input | Done (`apps/openchuck-cli/tests/cli.rs`) |
| The determinism harness catches a deliberately scheduling-dependent test | Done: thread-dependent chunking and shared push order are both caught |
| Every dependency has a record, a pinned version and one workspace declaration; the licence list is generated and the SPDX check passes | Done; `cargo about` generates the notices in CI. Open item 1 |
| The licence, contributor agreement and docs-location decisions are written down | Done: licence and docs location here, contributor agreement in `CLA.md` (follow-ups OI-22, OI-23) |
| The kinematic oracle agrees with the shared model over A angles including negatives and the 0/360 seam, and fails when the A sign is flipped | Done |
| The traceability report flags a seeded gap in a closed slice and fails CI | Done (xtask unit test); the real report passes with no slice closed |
