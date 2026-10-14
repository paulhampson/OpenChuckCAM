# 0026: Slice 8 design refinement: viewport and application shell

- **Status:** proposed. Slice 8 is in progress, not closed. Item 4 (OI-13) was accepted by the owner on 14 Oct 2026, and verification plan 4.2 now says what "edges" means.
- **Date:** 14 Oct 2026.
- **Branch:** `claude/slice-8-viewport-shell`.
- **Numbering:** first written as 0024 with OI-110 to OI-112 while slice 7 was unmerged. It was renumbered when `main` took 0023 to 0025 and OI-103 to OI-116.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-13 and OI-118 are closed (items 4 and 10), OI-117 and OI-119 are open, OI-45 progresses, and OI-14 is deferred to slice 11 (item 1).

## Context

Work breakdown slice 8 (UI-01, UI-02, UI-03, UI-06, UI-07, NFR-02, NFR-03) has SP-2 as its spike input. [Decision 0002](0002-sp-2-viewport-spike.md) is accepted (14 Oct 2026): keep egui, `egui_wgpu` and wgpu. NFR-03 was measured on the SP-2 spike on a desktop GPU (OI-09, closed by the owner on the margin). OI-11 is closed. OI-12 (pick budget, slice 10), OI-13 and OI-14 were left open.

Also relevant: architecture section 4 (only `openchuck-ui` and `openchuck-viewport` may use egui or wgpu) and section 5.8 (viewport, picking in `openchuck-geom`, no CAM logic in the UI), and verification plan rows UI-01 to UI-07 and NFR-03, plus section 4.2.

## Decision

1. **Start.** Building began on 14 Oct 2026, a few hours before decision 0002 was accepted, on the owner's decision to proceed on egui and wgpu. The acceptance removes the need for that exception. Two things carry over:
   - OI-09 measured the spike, not the product. Slice 8 item 7 builds the product's own scripted benchmark (item 9), so the 4.2 test can be repeated on the real viewport and on the reference class that was not run (integrated graphics, Apple M1).
   - OI-14 (STEP to an orbitable window) needs STEP import, which is slice 11. The owner deferred it to slice 11 (14 Oct 2026).
2. **The renderer has no toolkit dependency.** `openchuck-viewport` depends on `openchuck-types` and wgpu only. `Renderer::new`, `set_scene`, `prepare` and `paint` take a wgpu device, queue and render pass. The `egui_wgpu` paint callback lives in `openchuck-ui` (`viewport.rs`). A fallback toolkit that still uses wgpu would replace only that glue; Tauri would replace the crate, as decision 0002 already records. The camera is pure `f64` math, tested without a GPU. Matrices go to the GPU as `f32`.
3. **The scene is a view model in `openchuck-app`.** `Application::start_scene(setup)` runs as a job (models are read and placed off the UI thread; decision 0002 "Findings"). It returns `scene::SceneView`, which holds:
   - the placed part mesh with its feature edges;
   - the stock outline (cylinder rims and four lengthwise lines, or box edges);
   - the rotary axis along model X, past both ends of the scene;
   - an arrow for positive A, drawn from the shared kinematics (`facing_at`), so the viewport cannot disagree with the posts;
   - the work coordinate system at the work zero;
   - labels ("A+", "X", "Y", "Z");
   - the A direction in words, and notes (no machine, no part, unreadable model).

   The UI maps roles to colours and decides which lines are guides drawn over the part (axis, A arrow, work axes). It holds no CAM logic. With no setup, every model is shown at its placement. Imported stock (SET-04) is not drawn yet, and a note says so.
4. **Edge display uses feature edges (proposal for OI-13).** `openchuck_geom::display::feature_edges(mesh, crease_degrees)` returns the edges whose two triangles turn by more than the crease angle (default 30 degrees), plus open and non-manifold edges, in edge-map order. "Shaded with edges" draws these; "wireframe" draws every triangle side. **Accepted by the owner (14 Oct 2026, OI-13 closed):** for the NFR-03 test, "edges" means these feature edges at the default 30 degree crease angle. Verification plan section 4.2 says so.
5. **Shading without normals.** Meshes upload positions only (12 bytes a vertex). The fragment shader takes the flat normal from screen-space derivatives and shades with a two-sided headlight, so inward normals still read correctly and a 1M-triangle part needs no normal buffer. Surfaces get a small depth bias so edges on them win the depth test. No `bytemuck`: floats are written as little-endian bytes, so the recorded-but-undeclared `bytemuck` row in `docs/dependencies.md` is not needed for now.
6. **Camera conventions (UI-02).** The scene is Z-up in the model frame, with model X as the rotary axis. Standard views:

   | View | Looks from | On screen |
   | --- | --- | --- |
   | Front | −Y | X right, Z up |
   | Back | +Y | mirror of front |
   | Left | −X | |
   | Right | +X | |
   | Top | +Z | X right, Y up |
   | Bottom | −Z | |
   | Isometric | (1, −1, 1) | |

   Controls:
   - Orbit with the primary button; pan with the secondary or middle button (the model follows the pointer); zoom with the wheel.
   - Fit to the scene when a project opens and on each standard view.
   - The orthographic view shows the same height at the target as the perspective view, so switching keeps the size on screen.
   - `Camera::ray` gives the pick ray for slice 10.
7. **Application additions for a front end.**
   - `JobHandle::try_result` polls a job without blocking.
   - `Application::is_modified` compares the project with a copy taken at open, create and save. Undoing back to the saved state counts as unmodified.
   - `Application::location` returns the project folder.

   `save` stays `&self`.
8. **Shell (items 3 and 6, in part).**
   - Menus: File (new, open, save, quit), Edit (undo, redo, with shortcuts) and View (standard views, fit, projection, display mode).
   - New, open and quit ask Save, Discard or Cancel when there are unsaved changes, and so does closing the window.
   - A folder without a project offers to create one (decision 0008 item 8).
   - Folders are typed in, because a native file dialog needs a crate outside the crate selection (OI-117).
   - On open, changed or missing model files are reported in the status bar (OI-45, in part).
   - The project tree lists models, setups, operations and tools. Choosing a setup shows it.
   - The status bar shows the A direction, the scene notes and the G-code verification notice (NFR-12).

9. **The product frame-time benchmark (item 7; NFR-03, verification plan 4.2).** OI-09 measured the SP-2 spike. This benchmark runs the 4.2 test on the product viewport, so it can be repeated on each release and on the reference class not yet run (integrated graphics, Apple M1).
   - **Camera path.** `openchuck_viewport::bench::CameraPath` is a function of time only, so every run moves the camera the same way whatever the frame rate. From a camera fitted to the scene it does two turns of orbit with the pitch swinging 30 degrees either way, then pans once round a circle of a third of the scene radius, then zooms in to about a third of the distance and out to almost three times it.
   - **Statistics.** `FrameTimes` gives the mean and the 50th, 95th and 99th percentiles (nearest rank) and the maximum. A run passes at p95 of 33 ms or less.
   - **Running it.** `cargo run --release -p openchuck-benches --example bench_model -- model.stl` writes the testkit's NFR-03 model (1,002,000 triangles). `openchuck-ui --bench model.stl [--seconds N] [--mode shaded|edges|wireframe] [--out report.txt]` then:
     - adds the model to a temporary project through the application API;
     - opens a 1920 by 1080 window with vsync off;
     - draws 30 warm-up frames;
     - plays the path, recording the interval between frames;
     - prints the report (adapter, viewport size in pixels, triangle and feature-edge counts, mode, statistics, PASS or FAIL);
     - exits with code 2 on a fail.

     The default mode is shaded with feature edges, as 4.2 now says. Frame time is the interval between frames, as in SP-2, not a GPU timestamp.

10. **Operation tree commands (item 4; UI-06; OI-118 settled by the owner, 14 Oct 2026).**
    - **Enable and disable.** `Operation.enabled` is new. A disabled operation stays in the project and the tree, its name struck through, but is skipped everywhere, as if it were absent: no toolpath, no stock removal in simulation or carry-over, no G-code, and its tool is not in the tool-number plan (GC-14). `Command::SetOperationEnabled` is undoable.
    - **Storage.** The flag is written to the operation's file as `enabled = false` only while the operation is disabled, so enabling it again restores the file's bytes and existing projects do not change. Project schema 6, with a migration that changes only the number (a schema 5 operation is enabled).
    - **Duplicate.** `duplicate_operation` builds an `Add` of a copy with a new ID, named "<name> (copy)", placed just after the original. It keeps the strategy, tool, setup, selections, parameters and enabled state.
    - **Reorder.** Uses the existing `Command::Move`, which changes one order key (decision 0006).
    - **In the UI.** Each operation row in the tree has an enable box and move-earlier, move-later and duplicate buttons, each one undoable step.

## Not built yet (rest of slice 8)

| Item | What remains | Note |
| --- | --- | --- |
| 5 (UI-07) | Parameter forms from strategy and tool schemas; the custom-profile editor with live preview (TL-03 closes here) | |
| 6 | Import dialog with the check report, the tessellation statement and the unit question (OI-51, OI-52); model transform and orientation controls | |
| 7 (NFR-02) | The CI software-rendered smoke run (OI-119). The benchmark is built (item 9); its run on the reference hardware is the owner's | |
| — | OI-53 (default library location) and OI-54 (machine copy against the library) | Both were placed in slice 8 |

## Verification (so far)

- `openchuck-geom` `tests/display.rs` (5 tests): the cube's 12 feature edges and no face diagonals; open edges; smooth parts show under a tenth of their edges; crease-angle limits; deterministic.
- `openchuck-app` `tests/scene.rs` (6 tests): part, stock, axis and work zero together (UI-03); the A arrow follows the documented convention, positive A turning the top toward −Y and the negative sense turning the other way (`req_set_01_req_ui_03_...`); missing machine and part give notes, not errors; `is_modified` across edit, save, undo and redo; polling a job; deterministic scene.
- `openchuck-viewport` `tests/camera.rs` (8 tests, no GPU): standard views put the model axes where the table says, in both projections; isometric direction; fit holds the box for three aspects and seven views; orbit, pan (the model follows the pointer), zoom, projection switch; the pick ray passes through the projected point.
- `openchuck-viewport` `tests/render.rs` (3 tests): renders offscreen and reads the pixels back.
  - What they check: a red block at +X shows on the right in the front view and on the left in the back view, in both projections; the three display modes draw what they should; guides show through the part and depth-tested lines do not.
  - Where they ran: on llvmpipe (Mesa 25.0.7, Vulkan) with `OPENCHUCK_REQUIRE_ADAPTER=1`. Without an adapter they report a skip.
- `openchuck-ui`: a unit test that guides are drawn on top and the stock is not. The binary launched with a project and ran for 8 s under X11 with no error. No screenshot: X refused to capture the GPU-composited window. A visual check by the owner is still to do.
- `cargo clippy --workspace --all-targets` is clean; `cargo test --workspace` passes; `cargo xtask deps`, `licences`, `records`, `trace` and `testdata` pass.
- Benchmark (item 9): `openchuck-viewport` `tests/camera.rs` gains 3 tests: the path orbits, pans and zooms and returns to its start; it keeps the scene centre on screen at every pose; the percentile and pass rules. An `openchuck-ui` unit test covers option parsing.
- **End-to-end benchmark run in the cloud container (not reference hardware, so not a 4.2 result).** llvmpipe (CPU), 1,002,000 triangles, 2,000 feature edges, viewport 1751 by 1003 pixels, 10 s path: 107 frames, p50 93.5 ms, p95 102.1 ms, FAIL (exit code 2), as a CPU renderer should. This shows the harness works end to end. For comparison, SP-2 measured p95 256 ms on llvmpipe with its polyline layer.
- Tree commands (item 10): `openchuck-app` `tests/operation.rs` gains `req_ui_06_duplicate_and_reorder_are_undoable` and `req_ui_06_disabled_operations_are_skipped_and_saved`. The second checks that a disabled operation is left out of regeneration, saved as `enabled = false`, reloaded, and that undo removes the line again. Tests that state the current schema were moved to 6, and the newer-schema test now uses `SCHEMA + 1`.
- Not measured: the 60 s product run on hardware; Windows and macOS launch from CI artefacts.

## Alternatives considered

- **Renderer inside the egui callback in `openchuck-viewport`** (the spike's layout). Rejected: it ties the viewport crate to egui, and the fallback would then touch the renderer as well.
- **Per-vertex normals.** They cost 12 more bytes a vertex and break at hard edges unless vertices are split. The derivative normal gives flat shading for free.
- **Stock as a translucent mesh.** Translucency needs sorting. An outline shows the stock without hiding the part.

## Consequences

- Verification plan 4.2 defines the edges of the NFR-03 test (item 4).
- The work breakdown shows slice 8 in progress, and SP-2's status shows OI-13 closed.

## Open items

| ID | Item |
| --- | --- |
| OI-117 | Native file dialogs. Folders are typed in until a dialog crate (for example `rfd`) has an NFR-14 record and an AGPL-compatible licence check. |
| OI-118 | Operations need an enabled flag for UI-06, with a rule for disabled operations. Closed 14 Oct 2026: skipped everywhere, schema 6 (item 10). |
| OI-119 | CI smoke test for UI-01 and NFR-02: install a software Vulkan driver (Mesa lavapipe) on the Linux runner and set `OPENCHUCK_REQUIRE_ADAPTER=1` for `openchuck-viewport`'s render tests. Check what the Windows (WARP) and macOS runners offer, and add a scripted launch of the binary. |
