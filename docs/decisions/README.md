# Decision records

A decision record captures a choice made after the documents in `docs/` were written, or one that changes them. The architecture's "Open decisions" section (architecture.md, section 13) lists what is still open.

## Format

One file per decision, named `NNNN-short-title.md` (four digits, starting at `0001`). Each record contains:

- **Status:** proposed, accepted, or superseded by `NNNN`.
- **Context:** the requirement IDs and document sections it affects.
- **Decision:** what was chosen, in a sentence or two.
- **Alternatives considered:** with reasons, including any spike results.
- **Consequences:** documents or code that must change, and any rule exceptions (for example an entry in the `xtask` dependency allow-list, which also needs the NFR-14 justification).

When a record is accepted, update the affected document in the same pull request.

## Index

Add a row for each new record. Read the records that touch your task before you start.

| Record | Title | Status |
| --- | --- | --- |
| [0001](0001-sp3-wrapped-path-result.md) | SP-3 result, wrapped path to G-code with A words | Accepted (simulator criterion met by [0009](0009-linuxcnc-fixture-checks.md); open items OI-06 to OI-08 stay in; OI-03 and OI-04 closed 14 Oct 2026 by [0007](0007-sp-12-z1-conventions.md) in [open-items.md](../open-items.md)) |
| [0002](0002-sp-2-viewport-spike.md) | SP-2 viewport spike, interim result | Accepted 14 Oct 2026 (keep egui and wgpu; NFR-03 measured on a desktop GPU only; open items OI-12 to OI-14) |
| [0003](0003-step-kernel-route.md) | SP-1 STEP kernel route: thin in-house wrapper over OpenCASCADE, isolated import process | Accepted (third-party STEP samples still required before slice 11 import work is accepted) |
| [0004](0004-slice-0-guardrails.md) | Slice 0 design refinement, guardrails and shared data | Accepted (IR not frozen until a later slice). Open items in [open-items.md](../open-items.md) |
| [0005](0005-sp-5-gcode-reader.md) | SP-5 result, G-code reader: in-house reader | Accepted (LinuxCNC check of the fixture done in [0009](0009-linuxcnc-fixture-checks.md): the file needs four arc lines corrected, OI-46) |
| [0006](0006-sp-8-project-format.md) | SP-8 result, project format and order keys: per-item order keys; TOML for the project folder | Accepted |
| [0007](0007-sp-12-z1-conventions.md) | SP-12 result, Makera Z1 rotary conventions and G93 | Accepted 14 Oct 2026 (air cut run on firmware 1.1.2; tool change still open, OI-41; machine Z of the axis, OI-60) |
| [0008](0008-slice-1-project-model.md) | Slice 1 design refinement: project model, project folder layout, order keys, schema migration and undo | Accepted |
| [0009](0009-linuxcnc-fixture-checks.md) | LinuxCNC checks of the SP-3 and SP-5 fixtures | Accepted (open items OI-46 and OI-47 in [open-items.md](../open-items.md)) |
| [0010](0010-slice-2-stl-import.md) | Slice 2 design refinement: STL import, mesh checks, model placement and project schema 2 | Accepted (open items OI-50 to OI-52 in [open-items.md](../open-items.md)) |
| [0011](0011-slice-3-machine-stock-setup.md) | Slice 3 design refinement: machine profile, stock, setup frames and clearance; project schema 3 | Accepted (11 Oct 2026; open items OI-53 to OI-55, OI-60; OI-37 cross check closed 14 Oct 2026) |
| [0012](0012-sp-13-tool-profiles.md) | SP-13 result, one revolved-profile tool model: monotone profiles, generic drop-cutter | Accepted (the sweep budget waits on SP-6, OI-56) |
| [0013](0013-z1-profile-consensus.md) | Makera Z1 profile values: consensus across Makera's published profiles (A direction, travel, feeds) | Accepted (11 Oct 2026; amends 0011 item 8; OI-37 cross check closed 14 Oct 2026 by 0007, OI-60 open) |
| [0014](0014-slice-4-tool-library.md) | Slice 4 design refinement: tool data, profile validation, the tool library folder, machine tool sets (TL-11) and snapshots; project schema 4; GC-14 agreed | Accepted (12 Oct 2026; slice 4 closed; open items OI-61 to OI-63; OI-54 moved to slice 8) |
| [0015](0015-sp-4-ray-cast-sweep.md) | SP-4 result, ray-cast sweep around the A axis with parry3d-f64: climb method, tool axis from the in-plane normal | Accepted (12 Oct 2026; OI-07 closed; OI-64 to OI-67 open) |
| [0016](0016-sp-7-2d-offsets.md) | SP-7 result, 2D offsets and booleans: `cavalier_contours` for offsets (always from the original region), `i_overlay` for region booleans; arcs chorded at the A conversion | Accepted (12 Oct 2026; open items OI-68 to OI-70; numbers 0015 and OI-64 to OI-67 are claimed by the unmerged SP-4 work) |
| [0017](0017-sp-9-exact-surfaces.md) | SP-9 result, exact surfaces: plain data in, pure-Rust evaluation in the library, the IMP-02 waiver not applied | Accepted (12 Oct 2026; open items OI-72 to OI-76 in [open-items.md](../open-items.md)) |
| [0018](0018-slice-5-pathing-core.md) | Slice 5 design refinement: the pathing core (ports, drop-cutter, A-axis management, strategies and schemas, linking, contact feed), helical finishing with stock to leave, the 2D offsets of SP-7, the `SurfaceSource` port shaped for SP-9's exact faces, and the application mapping | Accepted (13 Oct 2026; slice 5 closed; OI-77, OI-80 and OI-81 open, OI-82 deferred to slice 12) |
| [0019](0019-sp-10-template-engine.md) | SP-10 result, post template engine: `minijinja` 3.0.0 with an IEEE-exact numeric surface; feed rule in Rust (closes OI-38); community posts as folders | Accepted (13 Oct 2026; open items OI-83 to OI-87) |
| [0021](0021-sp-11-swarf.md) | SP-11 result, swarf: line-fit ruling detection plus a measured deviation gate; four-axis reach limits what can be flank cut | Accepted (14 Oct 2026; OI-94 to OI-96 closed; OI-97 and OI-98 open) |
| [0022](0022-sp-6-simulation-stock.md) | SP-6 result, simulation stock: dexels in the part frame, voxel as fallback, rotary grid rejected as the main stock; the budget for SP-13 criterion 3 | Accepted (14 Oct 2026; OI-56 and OI-99 closed; OI-100 to OI-102 open) |
| [0023](0023-nightly-builds.md) | Nightly development builds: a separate scheduled workflow builds the CLI and UI for Linux, Windows and macOS when `main` has changed, one archive per platform with licence notices and the G-code notice, published as artifacts and as a rolling `nightly` pre-release | Accepted (14 Oct 2026; OI-103 and OI-104 deferred to slice 20) |
| [0024](0024-pathing-review-p2-additions.md) | Pathing review: a rotary acceleration limit (SET-08) and alternative finishing patterns (OP-19) added as P2; rotary precision logged as OI-105 | Accepted (14 Oct 2026; open items OI-105 to OI-107; number 0023 and OI-103, OI-104 are claimed on `claude/nightly-build`) |
| [0025](0025-slice-7-simulation.md) | Slice 7 design refinement: dexel stock behind `StockModel`, exact span solver for every tool, kinematic sampling, checkpoints, the resolution as a project setting (schema 5), deviation as the closest distance, carry-over between a setup's operations, CLI `simulate` | Accepted (14 Oct 2026; slice 7 closed; OI-100 and OI-101 closed; OI-102 and OI-113 to OI-116 open) |
| [0026](0026-entry-moves.md) | Entry moves: lead-in, lead-out, ramping and helical entry as requests to one shared module in `openchuck-path` (answers OI-80) | Accepted (15 Oct 2026; OI-80 closed, OI-117 new) |
| [0027](0027-slice-11a-step-import.md) | Slice 11a design refinement: STEP import through a worker process, the B-rep face record, units, tessellation and the exact `SurfaceSource` checked against the kernel | Accepted 14 Oct 2026 (OI-19, OI-20, OI-27, OI-72, OI-118 and OI-123 closed; OI-119 to OI-122 new) |
| [0029](0029-slice-8-viewport-and-shell.md) | Slice 8 design refinement: viewport and application shell; toolkit-free renderer, scene view model, feature edges for the edge display (OI-13), the product frame-time benchmark, operation enable, disable, duplicate and reorder (project schema 6), parameter and tool forms, the custom-profile editor, the import dialog, file dialogs, library folders, machine IDs | Proposed (slice 8 in progress; OI-13, OI-45, OI-51, OI-52, OI-53, OI-54, OI-127 and OI-128 closed; OI-130 closed; OI-129 open; OI-14 deferred to slice 11; owner's Windows run recorded) |
| [0030](0030-slice-9-pipeline-cache.md) | Slice 9, operation pipeline cache: keys chained from operation to operation, a configurable number of results kept per operation in memory, part file hashed each run | Accepted |
| [0031](0031-slice-20-packaging-and-trial-documents.md) | Slice 20 preparation: packaging route (plain build matrix, no `cargo-dist`), macOS bundle, release tag schemes, and the drafted Z1 air-cut procedure and safe-practice checklist | Accepted (14 Oct 2026; OI-133 and OI-134 closed, OI-135 open) |
| [0032](0032-slice-10-mesh-selection.md) | Slice 10 design refinement (items 2 to 5): triangle selection algorithms, sets edited as commands, boundary, avoid and check roles, a region constraint that the helical strategy honours, and errors for unresolved references. Accepted. |
| [0033](0033-slice-12-baseline-finishing.md) | Slice 12 design refinement (lane H): axial and indexed finishing, stepover and scallop meaning, per-selection stock to leave, region offsets replacing sampling, entry on finishing passes. Accepted (15 Oct 2026; OI-146, OI-147 and OI-138 closed, OI-145, OI-148 and OI-137 open). |
