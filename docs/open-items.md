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
| OI-05 | G-code reader: `gcode` 0.7.0 with an f64 layer, or an in-house reader. | 0001 item 5 | open | Slice 6 (SP-5) | SP-5 |
| OI-06 | A limits and unwinding policy (range limits, when and which way to unwind). | 0001 item 6 | open | Slice 5 (OP-15) | A range in the machine profile |
| OI-07 | Off-top contact and tilted tools (tool axis variation, cutter shape). | 0001 item 7 | open | Slices 5 and 13 | SP-4, SP-13 |
| OI-08 | Chord tolerance factor, 0.4 or 0.5 of the toolpath tolerance. | 0001 item 8 | open | Slice 5 | Measurement on the first strategy |
| OI-09 | Measure NFR-03 (30 fps at 1M triangles) on the reference hardware, without and with edges. | 0002 item 1 | open | Before slice 8 | Project owner's physical machines |
| OI-10 | Builds on Windows and macOS. Slice 0's CI covers it but has not run yet: everything so far was built and tested on Linux only. | 0002 item 2; 0003 open item 10 | open | First pull request of slice 0 | CI run |
| OI-11 | Keep egui and wgpu or fall back to Tauri or iced. | 0002 item 3 | open | Before slice 8 | OI-09, OI-10 |
| OI-12 | Define the pick budget (10 ms assumed) for the verification plan. | 0002 item 4 | open | Before slice 10 | Agreement on a verification plan change |
| OI-13 | Define "edges" for the NFR-03 test (feature edges or all triangle edges). | 0002 item 5 | open | Before slice 8 | Agreement on a verification plan change |
| OI-14 | Combine SP-2 with SP-1 (STEP to an orbitable window). | 0002 item 7 | open | After SP-1 | SP-1 result |
| OI-16 | Reserve the `openchuck-*` names on crates.io (publishes placeholder crates). | 0003 open item 5 | open | Slice 0 close | Project owner |
| OI-17 | Benchmark regression flag at 10% needs a dedicated runner. The criterion baseline exists in `benches/`; no workflow runs it. | 0003 open item 6 | deferred (owner, 11 Oct 2026) | When a dedicated runner is available | A self-hosted runner |
| OI-19 | Miri and sanitizers: nothing to check yet (`unsafe_code` is denied, no FFI). Add the job with the first unsafe or FFI code. | 0003 open item 8 | deferred | First unsafe or FFI code (STEP kernel, slice 11) | SP-1 result |
| OI-20 | STEP kernel CI job, path-filtered so a change to `docs/` or `posts/` does not rebuild it. | 0003 open item 9 | deferred | When the kernel is added | SP-1 result |
| OI-21 | The IR is not frozen: OI-01, OI-03, OI-04 and OI-07 may still change IR or profile fields. | 0003 open item 11 | open | Slices 3, 5 and 6 | Those items |
| OI-23 | Legal review of `CLA.md` and `CLA-ENTITY.md`. They adapt the Apache ICLA and CCLA, and name Paul Hampson as Maintainer; sections 2 and 9 are additions. A lawyer has not reviewed them. | Contributor agreement, 11 Oct 2026 | open | Before the first outside contribution | Project owner |
| OI-24 | Confirm the CLA check works on the first outside pull request: the repository's Actions settings must let workflows write (the workflow requests `contents`, `pull-requests` and `statuses` write), the `cla-signatures` branch must stay unprotected, and the CLA check should be made a required status for `main` in the branch protection rules. | OI-22 | open | First outside pull request | Project owner (repository settings) |

## Closed

| ID | Item | Source | Closed | Settled in |
| --- | --- | --- | --- | --- |
| OI-C1 | NFR-14 adoption records for eframe/egui, egui-wgpu, wgpu, parry3d-f64 and bytemuck. | 0002 item 6 | 11 Oct 2026 | `docs/dependencies.md` (parry3d-f64 and bytemuck recorded, not yet declared) |
| OI-C2 | Add `openchuck-testkit` and `openchuck-benches` to the architecture. | 0003 open item 2 | 11 Oct 2026 | Agreed by the owner; architecture sections 3, 9 and 12 |
| OI-C3 | `AGPL-3.0-or-later` or `AGPL-3.0-only`. | 0003 open item 3 | 11 Oct 2026 | Owner chose `AGPL-3.0-or-later`; architecture sections 12 and 13 |
| OI-C4 | Contributor agreement allowing a later licence exception or dual licence (INT-09). | 0003 open item 4 | 11 Oct 2026 | `CLA.md`, `CLA-ENTITY.md`, `CONTRIBUTING.md`; follow-ups OI-22 and OI-23 |
| OI-15 | egui's bundled fonts (`epaint_default_fonts`): OFL-1.1 and Ubuntu-font-1.0. | 0003 open item 1 | 11 Oct 2026 | Owner accepted both; `deny.toml` keeps them scoped to that crate |
| OI-18 | Scheduled dependency activity check. | 0003 open item 7 | 11 Oct 2026 | Owner accepted the recommendation: weekly `cargo-unmaintained` job (`.github/workflows/weekly.yml`), Dependabot (`.github/dependabot.yml`) and `cargo deny` advisories; recorded in `docs/dependencies.md` |
| OI-22 | How CLA signatures are collected. | Contributor agreement | 11 Oct 2026 | Owner chose CLA Assistant Lite (`.github/workflows/cla.yml`); signatures on the `cla-signatures` branch. Follow-up OI-24 |
