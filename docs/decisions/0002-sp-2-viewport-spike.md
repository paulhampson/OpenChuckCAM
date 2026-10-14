# 0002: SP-2 viewport spike, interim result

- **Status:** accepted 14 Oct 2026. Keep egui, `egui_wgpu` and wgpu as the viewport stack. NFR-03 was measured on a desktop GPU (see "Windows run") and the owner closed that item on the margin; integrated graphics and Apple M1 were not run. Open items 4, 5 and 7 remain (OI-12, OI-13, OI-14).
- **Date:** 10 Oct 2026.

## Context

Work breakdown, Spikes, SP-2 (3 d): does egui with `egui_wgpu` hold 30 fps on 1M triangles on all three platforms, with native graphics? It is a gating risk (work breakdown, finding 12) and must be reviewed before slice 8 starts and checked at slice 0 for platform builds.

Requirements and sections affected: NFR-02 (platforms), NFR-03 (30 fps at 1M triangles), UI-01 to UI-03 (viewport), architecture sections 5.8, 10 and 11 (viewport, performance, spikes), verification plan section 4.2 (reference hardware, scripted 60-second path, p95 frame time of 33 ms or less), and the crate selection (eframe/egui 90, egui_wgpu 87, wgpu 88, parry3d 83, bytemuck 96).

The spike code is throwaway and is not merged into the product (spike rules). It lives on branch `spike/sp-2-viewport`, commit `d773e61`, under `spikes/sp-2-viewport/`. Its write-up, `RESULT.md`, is on that branch.

## Decision

1. **Provisionally continue with egui, `egui_wgpu` and wgpu** as the viewport stack. Nothing found so far argues for the fallback, but the main criterion is unmeasured, so this is not yet a confirmed choice.
2. **Do not treat NFR-03 as proven.** Slice 8 must not start on the strength of this record until the outstanding items below are closed.
3. Record the stack-level findings below so slices 0 and 8 can use them.

## What the spike showed

Method: `eframe` with the wgpu backend, a 32-bit depth buffer and continuous repaint. One indexed draw of a procedural rotary part (999,698 triangles, 500,556 vertices), an optional edge overlay of every grid edge (1,500,254 lines), a 300,000-point polyline layer, orbit, pan and zoom, and a CPU pick through a `parry3d-f64` `TriMesh` BVH. A `--bench` mode runs the verification plan 4.2 camera path (orbit, pan, zoom) and reports frame-time percentiles.

Measured on Linux with software rendering only (llvmpipe, 4 cores, no GPU in the environment). These numbers do not bear on the 30 fps criterion.

| 1M triangles, 15 s scripted path | Mean frame | p95 frame |
| --- | --- | --- |
| Polyline, no edges | 203.9 ms (4.9 fps) | 256.1 ms |
| Polyline and edge overlay | 360.9 ms (2.8 fps) | 450.5 ms |

| Picking | Value |
| --- | --- |
| `TriMesh` and BVH build, 1M triangles | about 0.89 s, once at load |
| Ray cast, p95 | about 21 µs |
| Hit accuracy | within 0.05 mm of the analytic radius (unit test) |

| Pass criterion | State |
| --- | --- |
| 30 fps or better at 1M triangles on the reference machine | **Not measured** |
| Builds on three platforms | **Linux only**; Windows and macOS not tried |
| Pick within budget | Met on the measure taken; the documents set no pick budget (10 ms assumed) |

Correctness checked by eye on a screenshot: shading, depth ordering, edge overlay and the polyline layer render correctly, with no z-fighting.

## Windows run (14 Oct 2026)

Machine: Windows 11 Pro, AMD Ryzen 5 5600X, 32 GB RAM, NVIDIA GeForce RTX 2070 Super 8 GB (Vulkan, discrete GPU). Rust 1.97.0 (the pinned toolchain). Same spike code and the verification plan 4.2 path, 60 s each, no vsync, 999,698 triangles and a 300,000-point polyline. Raw results: `spikes/sp-2-viewport/results/windows-2070super*.jsonl` on `spike/sp-2-viewport` (commit `d911116`), with the write-up in its `RESULT.md`.

| Run | Mean frame | p95 | p99 | Max | Mean fps |
| --- | --- | --- | --- | --- | --- |
| Shaded and polyline | 0.46 ms | 0.69 ms | 0.88 ms | 2.24 ms | 2187 |
| And every triangle edge (1.5M lines) | 0.61 ms | 0.93 ms | 1.07 ms | 3.05 ms | 1649 |

Both pass the 33 ms p95 limit by about 35 to 50 times. Picking on this machine: BVH build about 535 ms, ray cast p95 5.7 µs. The app builds and runs on Windows (NFR-02). Frame time is the interval between frames as the app sees it, not a GPU timestamp query, and the machine is a desktop discrete GPU, not the plan's reference class (integrated graphics, Apple M1). The owner closed outstanding item 1 on the size of the margin; a run on integrated graphics stays welcome if a machine turns up.

## Findings for later slices

- An `egui_wgpu` paint callback with a depth buffer from `NativeOptions::depth_buffer` is enough for a custom 3D pass, with no egui changes. This supports the architecture's choice of a custom wgpu viewport inside egui.
- wgpu 30 changed several descriptor fields from older releases (optional bind group layouts, `immediate_size`, `multiview_mask`, optional depth write and compare). This is the API churn the architecture warns about and supports exact version pins and an upgrade cadence (slice 0).
- `parry3d-f64` 0.31 uses `glamx` vector types, not `nalgebra`. They must stay behind the `openchuck-geom` interface (no third-party types in public APIs).
- Drawing every triangle edge roughly halves the frame rate under software rendering. The product should draw feature edges, which the spike did not test. The 4.2 test says "shaded with edges"; what counts as "edges" for that test needs defining.
- The BVH build (about 0.9 s at 1M triangles) is a load-time cost. Build it off the UI thread behind a job with progress (UI-09, NFR-04).

## Alternatives considered

- **Tauri with a JavaScript viewer** (replaces `openchuck-viewport`) and **iced**: the named fallbacks. Not tried, because the primary criterion is unmeasured and nothing so far has failed.
- A mesh-edge overlay as a polygon-mode line pass instead of a line list: not tried; it needs an optional wgpu feature that not every platform offers.

## Outstanding items for later decisions

1. **Closed 14 Oct 2026 (OI-09), measured on a desktop GPU only; see "Windows run".** **Measure NFR-03 on the reference hardware** (verification plan 4.2: Apple M1-class on macOS, Intel Xe or recent AMD integrated graphics on Windows and Linux), 60 s scripted path, without and with the edge overlay, native graphics only. A run passes at p95 of 33 ms or less. Owner: the project owner, since it needs the physical machines.
2. **Closed 11 Oct 2026 (OI-10).** **Confirm the build on Windows and macOS** (NFR-02). Slice 0 sets up the three-platform CI and covers this.
3. **Decide keep or fall back** (OI-11): closed 14 Oct 2026, keep. If any platform fails, apply the fallback (Tauri or iced) and supersede this record. Review the result before slice 8.
4. **Define the pick budget.** Propose a figure (10 ms assumed here) for the verification plan; this needs a document change, so agree it first.
5. **Define "edges" for the NFR-03 test** (feature edges or all triangle edges) and update verification plan 4.2 if the wording changes.
6. **Adoption records (NFR-14)** for `eframe`/`egui`, `egui-wgpu`, `wgpu`, `parry3d-f64` and `bytemuck`, with licence checks through `cargo deny`, to be made in slice 0. The versions tried were `eframe` 0.36.2, `wgpu` 30.0.1, `parry3d-f64` 0.31.1 and `bytemuck` 1.25.0.
7. **Combine with SP-1** (STEP to an orbitable window), which the work breakdown asks for and the spike did not do.

## Consequences

- No document is changed by this record, and no rule exception or `xtask` allow-list entry is needed (nothing from the spike enters the product).
- The stack does not change; if a later measurement fails, edit the architecture (sections 4, 5.8 and the crate selection) and the crate selection in the same change.
- Items 4 and 5 may lead to edits of the verification plan; propose them before editing.
