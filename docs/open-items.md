# Open items

One register for the open items raised in decision records and slice reviews, so none is lost between documents. The decision record that raised an item stays the authority for its detail; this page tracks its state.

**Keeping it current.** When a record or a slice review raises an open item, add a row here in the same pull request. When an item closes, set its state to *closed* with the date and where it was settled, and keep the row: IDs are never reused. Review the register at each slice close.

States: *open* (needs work or a decision), *proposed* (a recommendation awaits agreement), *deferred* (agreed to wait; the trigger says when it returns), *closed*.

## Open, proposed and deferred

| ID | Item | Source | State | Decide in | Needs |
| --- | --- | --- | --- | --- | --- |
| OI-01 | LinuxCNC simulator run for SP-3 not done; SP-3's pass criterion is not met. | 0001 item 1 | open | Before slice 6 accepts any golden file | Someone with LinuxCNC 2.9 or later |
| OI-02 | No golden file with A words yet. | 0001 item 2 | open | Slice 6 | OI-01, OI-03 |
| OI-03 | A sign and rotary offset convention for the Z1 (the spike used +1 as a placeholder). | 0001 item 3 | open | SP-12, slice 3 | Machine profiles and the air cut |
| OI-04 | LinuxCNC feed rule on mixed XYZ and A moves; whether the Z1 accepts G93; per-move feed strategy near zero linear step. | 0001 item 4 | open | SP-12, slice 6 | OI-01 and a Z1 test |
| OI-06 | A limits and unwinding policy (range limits, when and which way to unwind). | 0001 item 6 | open | Slice 5 (OP-15) | A range in the machine profile |
| OI-07 | Off-top contact and tilted tools (tool axis variation, cutter shape). | 0001 item 7 | open | Slices 5 and 13 | SP-4, SP-13 |
| OI-08 | Chord tolerance factor, 0.4 or 0.5 of the toolpath tolerance. | 0001 item 8 | open | Slice 5 | Measurement on the first strategy |
| OI-09 | Measure NFR-03 (30 fps at 1M triangles) on the reference hardware, without and with edges. | 0002 item 1 | open | Before slice 8 | Project owner's physical machines |
| OI-11 | Keep egui and wgpu or fall back to Tauri or iced. | 0002 item 3 | open | Before slice 8 | OI-09, OI-10 |
| OI-12 | Define the pick budget (10 ms assumed) for the verification plan. | 0002 item 4 | open | Before slice 10 | Agreement on a verification plan change |
| OI-13 | Define "edges" for the NFR-03 test (feature edges or all triangle edges). | 0002 item 5 | open | Before slice 8 | Agreement on a verification plan change |
| OI-14 | Combine SP-2 with SP-1 (STEP to an orbitable window). SP-1 is decided (decision 0003), so this is no longer blocked. | 0002 item 7 | open | Before slice 8 | The `sp1-shim` prototype and the SP-2 viewport |
| OI-17 | Benchmark regression flag at 10% needs a dedicated runner. The criterion baseline exists in `benches/`; no workflow runs it. | 0004 open item 6 | deferred (owner, 11 Oct 2026) | When a dedicated runner is available | A self-hosted runner |
| OI-19 | Miri and sanitizers: nothing to check yet (`unsafe_code` is denied, no FFI). Add the job with the first unsafe or FFI code: the OpenCASCADE C ABI wrapper (decision 0003). | 0004 open item 8 | deferred | Slice 11 | The wrapper |
| OI-20 | STEP kernel CI job for the wrapper chosen by SP-1 (decision 0003): build OpenCASCADE through `occt-sys` with a per-platform cache (cold builds take 8 to 71 minutes); path-filter it so only `openchuck-import` rebuilds it and the other jobs exclude that crate; link `TKernel` first and test a clean process exit on every platform; add `occt-sys` with a pin, a survey-style score and LGPL-2.1 entries in `deny.toml`. `CMAKE_POLICY_VERSION_MINIMUM=3.5` is already set in `ci.yml`. | 0004 open item 9; decision 0003 | deferred | Slice 11 | The wrapper |
| OI-21 | The IR is not frozen: OI-01, OI-03, OI-04 and OI-07 may still change IR or profile fields. The owner confirmed on 11 Oct 2026 that it need not be frozen until a later slice. | 0004 open item 11 | deferred (owner, 11 Oct 2026) | Slices 3, 5 and 6 | Those items |
| OI-23 | Legal review of `CLA.md` and `CLA-ENTITY.md`. They adapt the Apache ICLA and CCLA, and name Paul Hampson as Maintainer; sections 2 and 9 are additions. A lawyer has not reviewed them. | Contributor agreement, 11 Oct 2026 | open | Before the first outside contribution | Project owner |
| OI-24 | Confirm the CLA check works on the first outside pull request (test pull request #6 from the maintainer's account showed the check judges commit authors: commits authored as `Claude <noreply@anthropic.com>` failed it, and commits authored as the maintainer's GitHub account pass through the allowlist. The signing flow itself still needs a second account): the repository's Actions settings must let workflows write (the workflow requests `contents`, `pull-requests` and `statuses` write), the `cla-signatures` branch must stay unprotected, and the CLA check should be made a required status for `main` in the branch protection rules. | OI-22 | open | First outside pull request | Project owner (repository settings) |
| OI-25 | STEP exports from other CAD tools (at least FreeCAD, Fusion and SolidWorks: small parts with cylinders, cones, B-spline faces and holes). SP-1 used only files written by OpenCASCADE. Rerun the `sp1-shim` walk and isolation on them; the result also decides `brepkit-io` (OI-26). Deferred by agreement. | Decision 0003, limits | deferred (owner, 11 Oct 2026) | Before slice 11's import work is accepted | Sample files |
| OI-26 | Re-spike `brepkit-io` (pure Rust, no crashes in the fuzz) when it reads full spheres and shell-only files and has been run on real CAD exports. | Decision 0003, item 2 | open | When those are fixed, or with OI-25 | `brepkit-io` releases |
| OI-27 | Nightly fuzzing of the STEP import worker with the spike's mutation script (the OpenCASCADE reader segfaults on damaged files; NFR-07 is met by process isolation). | Decision 0003, consequences | deferred | Slice 11 | The import worker |
| OI-28 | Golden tests that depend on kernel tessellation or kernel-computed numbers need a tolerance on non-analytic faces, or per-platform golden files (verification plan section 2, policy 7): tessellation differs between x86-64 and arm64. | Decision 0003, cross-platform differences | open | Slice 11 | Agreement on the verification approach |
| OI-29 | Add the OpenCASCADE licence notice and source offer to the release checklist (LGPL-2.1, statically linked). | Decision 0003, NFR-10 | open | Slice 20 | Release checklist |
| OI-30 | Examples for the library crates other than `openchuck-types` (slice 0 item 7). Each gets one in the slice that gives it a public API. | 0004, slice close | deferred | Slices 5 (path), 6 (post, gcode), 2 (geom) and later | A public API |
| OI-31 | Run the SP-5 hand-written file `linuxcnc_post_subset.ngc` through LinuxCNC's own interpreter (`rs274` standalone or the simulator) before it becomes a `testdata/` fixture; it was checked by hand only. The owner runs it separately. | 0005 item 2 | open | Before slice 6 adds the fixture | Project owner, with LinuxCNC 2.9 or later (with OI-01) |
| OI-34 | Format for the project folder: JSON (recommended, as architecture section 7 names) or TOML. Both passed SP-8. | Decision 0006, item 1 | proposed | Before slice 1 item 2 | Project owner |
| OI-35 | Order-key growth under repeated insertion at one spot (about one character per six inserts): an explicit renumber action, or a length-prefixed key scheme. | Decision 0006, item 2 | open | Slice 1 | Slice 1 design refinement |
| OI-36 | Concurrent edits to the same item conflict in Git (adjacent fields of one operation, additions to one selection set). PRJ-05 is met; a semantic merge driver would belong with in-app version control (P2). | Decision 0006, item 3 | deferred | When VC work starts (P2) | None for P1 |

## Closed

| ID | Item | Source | Closed | Settled in |
| --- | --- | --- | --- | --- |
| OI-C1 | NFR-14 adoption records for eframe/egui, egui-wgpu, wgpu, parry3d-f64 and bytemuck. | 0002 item 6 | 11 Oct 2026 | `docs/dependencies.md` (parry3d-f64 and bytemuck recorded, not yet declared) |
| OI-C2 | Add `openchuck-testkit` and `openchuck-benches` to the architecture. | 0004 open item 2 | 11 Oct 2026 | Agreed by the owner; architecture sections 3, 9 and 12 |
| OI-C3 | `AGPL-3.0-or-later` or `AGPL-3.0-only`. | 0004 open item 3 | 11 Oct 2026 | Owner chose `AGPL-3.0-or-later`; architecture sections 12 and 13 |
| OI-C4 | Contributor agreement allowing a later licence exception or dual licence (INT-09). | 0004 open item 4 | 11 Oct 2026 | `CLA.md`, `CLA-ENTITY.md`, `CONTRIBUTING.md`; follow-ups OI-22 and OI-23 |
| OI-15 | egui's bundled fonts (`epaint_default_fonts`): OFL-1.1 and Ubuntu-font-1.0. | 0004 open item 1 | 11 Oct 2026 | Owner accepted both; `deny.toml` keeps them scoped to that crate |
| OI-18 | Scheduled dependency activity check. | 0004 open item 7 | 11 Oct 2026 | Owner accepted the recommendation: weekly `cargo-unmaintained` job (`.github/workflows/weekly.yml`), Dependabot (`.github/dependabot.yml`) and `cargo deny` advisories; recorded in `docs/dependencies.md` |
| OI-22 | How CLA signatures are collected. | Contributor agreement | 11 Oct 2026 | Owner chose CLA Assistant Lite (`.github/workflows/cla.yml`); signatures on the `cla-signatures` branch. Follow-up OI-24 |
| OI-10 | Builds on Windows and macOS (NFR-02). | 0002 item 2; 0004 open item 10 | 11 Oct 2026 | PR #3 CI at `44ed463`: build, clippy, tests and each library alone pass on Windows, macOS and Linux. The NFR-03 frame-rate measurement stays open (OI-09) |
| OI-16 | Reserve the `openchuck-*` crate names on crates.io. | 0004 open item 5 | 11 Oct 2026 | The owner publishes six `0.0.0` placeholders (`openchuck`, `openchuckcam`, `openchuck-types`, `-path`, `-post`, `-gcode`) and treats them as claimed; application crates are not reserved |
| OI-05 | G-code reader: `gcode` 0.7.0 with an f64 layer, or an in-house reader. | 0001 item 5; 0005 item 1 | 11 Oct 2026 | Owner accepted [decision 0005](decisions/0005-sp-5-gcode-reader.md): an in-house reader in `openchuck-gcode`; architecture section 7 updated |
