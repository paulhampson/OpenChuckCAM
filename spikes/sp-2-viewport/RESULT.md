# SP-2 Viewport spike: result

Status: **partly complete, not yet decided.** Builds, renders and picks correctly on Linux. The 30 fps criterion has **not** been measured on real graphics, because this container has no GPU. The decision (keep egui and `egui_wgpu`, or switch to the fallback) waits for the runs listed under "To finish SP-2".

Work breakdown reference: Spikes, SP-2 (3 d), requirements NFR-02 and NFR-03, architecture sections 5.8 and 10, verification plan section 4.2. Branch `spike/sp-2-viewport`. This code is throwaway and is never merged into the product (spike rules).

## Question

Does egui with `egui_wgpu` hold 30 fps on 1M triangles on all three platforms, with native graphics?

## Method

`spikes/sp-2-viewport` is a standalone Cargo project (its own workspace, so the product workspace and its dependency rules are untouched).

- Window and render loop: `eframe` with the wgpu backend, 32-bit depth buffer, continuous repaint.
- Mesh draw: one indexed `TriangleList` draw of a procedural bumpy rotary part (X axis), 999,698 triangles and 500,556 vertices, headlight shading, two-sided.
- Edge overlay (the "shaded with edges" mode of verification plan 4.2): every grid edge as a `LineList` on the same vertex buffer, 1,500,254 lines. This is a worst case, because a real viewer draws feature edges only.
- One polyline layer: a 300,000-point helical wrap path as a `LineStrip`, standing in for a toolpath.
- Camera: orbit, pan, zoom. The pick uses `parry3d-f64` `TriMesh` (BVH) ray casts in f64.
- `--bench [secs]` runs the scripted path from verification plan 4.2 (orbit, then pan, then zoom, one third each), exercises the picker every frame, and prints one JSON line with frame-time percentiles. The first 30 frames are discarded as warm-up. Present mode is `AutoNoVsync` in bench mode so frame time reflects work, not the display rate.
- A unit test checks that a pick ray hits the analytic radius (`pick_hits_analytic_radius`).

## Numbers

Only software rendering was available: `llvmpipe` (Mesa lavapipe, Vulkan on the CPU) under Xvfb, 4 cores, 1920x1080. **These numbers say nothing about the 30 fps criterion**; they show the pipeline works and set a floor.

| Run (15 s scripted path) | Mean frame | p95 frame | Frames |
| --- | --- | --- | --- |
| 1M triangles, polyline, no edges | 203.9 ms (4.9 fps) | 256.1 ms | 56 |
| 1M triangles, polyline, edge overlay | 360.9 ms (2.8 fps) | 450.5 ms | 32 |

Picking (does not depend on the GPU):

| Measure | Value |
| --- | --- |
| `TriMesh` + BVH build, 1M triangles | about 0.89 s (one-off, at load) |
| Ray cast, p95 | about 21 µs |
| Ray cast, max seen | 25 to 110 µs |
| Hit accuracy | within 0.05 mm of the analytic radius (test) |

Rendering checked by eye (`results/screenshot-llvmpipe.png`, 200k triangles): shading, depth ordering, edge overlay and polyline layer are correct, with no z-fighting.

## Pass criteria

| Criterion | State |
| --- | --- |
| 30 fps or better on the reference machine at 1M triangles (p95 frame time 33 ms or less) | **Not measured.** Needs real graphics. |
| Builds on three platforms | **Linux only** (Ubuntu 24.04, Rust 1.97). Windows and macOS not tried. |
| Pick within budget | **Met on the measure taken**, but the docs set no pick budget. Assumed 10 ms for interaction; the cast takes about 0.02 ms. The BVH build (0.9 s) is a load-time cost to plan for. |

## Findings that matter for slice 8

- egui 0.36.2 with wgpu 30.0.1 works for a custom pass without egui-side changes: `egui_wgpu::CallbackTrait` and a depth buffer from `NativeOptions::depth_buffer` are enough. Depth and the polyline layer needed no workaround.
- The egui renderer's pipeline layout, `RendererOptions` and wgpu descriptors (`bind_group_layouts` as `Option`, `immediate_size`, `multiview_mask`, `depth_write_enabled` as `Option`) differ from older wgpu releases. This is the churn the architecture warns about (risk register, API churn), and it supports pinning exact versions.
- `parry3d-f64` 0.31 uses `glamx` vectors (`DVec3`-style), not `nalgebra`. Its types must stay behind the `openchuck-geom` interface (architecture rule: no third-party types in public APIs).
- Full-triangle edge overlay roughly halves the frame rate under software rendering. The product should draw feature edges, not every triangle edge; that is not tested here.
- Mesh upload is a single 12 MB vertex buffer and a 12 MB `u32` index buffer for 1M triangles; both fit comfortably in integrated graphics.

## NFR-14 records (crates tried, pinned)

| Crate | Pinned | Survey score | Use |
| --- | --- | --- | --- |
| `eframe` / `egui` | =0.36.2 | 90 | Window and UI |
| `egui-wgpu` (via eframe) | 0.36.2 | 87 | Custom paint callback |
| `wgpu` (via egui-wgpu) | 30.0.1 | 88 | Rendering |
| `parry3d-f64` | =0.31.1 | 83 | BVH ray-cast pick |
| `bytemuck` | =1.25.0 | 96 | Vertex and uniform casts |

Licences were not checked with `cargo deny` (not part of this spike); do that when the dependency is adopted in slice 0.

## To finish SP-2

1. On each reference machine (verification plan 4.2: Apple M1-class on macOS, Intel Xe or recent AMD integrated graphics on Windows and Linux):
   `cargo run --release -- --bench 60 --out results/<platform>.jsonl`, then again with `--wire`.
   A run passes when `p95_ms` is 33 or less. Native graphics only; check `adapter` in the output does not say `Cpu`.
2. Confirm the build on Windows and macOS (`cargo build --release` in `spikes/sp-2-viewport`).
3. Record the numbers here, decide, and note the fallback status. If a platform fails, the fallback is Tauri with a JavaScript viewer (replaces `openchuck-viewport`) or iced, per the work breakdown.
4. Review the result before slice 8 starts (spike rules).

Slice 0 can proceed on the platform-build part (egui and wgpu compile on Linux) but should not treat NFR-03 as proven.
