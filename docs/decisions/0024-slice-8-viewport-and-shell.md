# 0024: Slice 8 design refinement: viewport and application shell, started provisionally on egui and wgpu

- **Status:** proposed. Slice 8 is started, not closed. It relies on [decision 0002](0002-sp-2-viewport-spike.md), which is itself only *proposed*: the stack decision (OI-11) is open.
- **Date:** 14 Oct 2026.
- **Branch:** `claude/slice-8-viewport-shell`.
- **Numbering:** record 0023 and OI-103 to OI-109 are left for the slice 7 branch, which is in progress and not pushed, so its numbers could not be checked. This record takes 0024 and OI-110 onwards. Renumber if slice 7 needs more.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-110 to OI-112 are new. OI-11 and OI-45 progress, and OI-13 is proposed (item 4).

## Context

Work breakdown slice 8 (UI-01, UI-02, UI-03, UI-06, UI-07, NFR-02, NFR-03) has SP-2 as its spike input. Decision 0002 says slice 8 must not start until its outstanding items close: OI-09 (NFR-03 measured on the reference hardware), OI-11 (keep egui and wgpu or fall back), OI-13 (what "edges" means for the NFR-03 test) and OI-14 (SP-1 and SP-2 combined). All four are still open. OI-09 needs the owner's physical machines.

Also relevant: architecture section 4 (only `openchuck-ui` and `openchuck-viewport` may use egui or wgpu) and section 5.8 (viewport, picking in `openchuck-geom`, no CAM logic in the UI), and verification plan rows UI-01 to UI-07 and NFR-03, plus section 4.2.

## Decision

1. **Provisional start (owner's decision, 14 Oct 2026).** The owner chose to build all of slice 8 on egui, `egui_wgpu` and wgpu now, rather than wait for the gate in decision 0002 or do only the stack-independent parts. This is a recorded exception to decision 0002 item 2. Accepted consequences:
   - **Rework risk.** If OI-09 fails on a platform and OI-11 ends in a fallback (Tauri or iced), the UI glue is rewritten. Item 2 limits what that would touch.
   - **Checklist item stays unticked.** The slice 8 review checklist item for the 33 ms p95 frame time is not ticked until OI-09 is measured, so the slice cannot close before then.
   - **OI-14 still open.** STEP to an orbitable window is not a slice 8 deliverable here: STEP import is slice 11. It stays open as decision 0002 states it.
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
4. **Edge display uses feature edges (proposal for OI-13).** `openchuck_geom::display::feature_edges(mesh, crease_degrees)` returns the edges whose two triangles turn by more than the crease angle (default 30 degrees), plus open and non-manifold edges, in edge-map order. "Shaded with edges" draws these; "wireframe" draws every triangle side. **Proposed for the NFR-03 test:** "edges" means feature edges at the default 30 degree crease angle. This needs a verification plan section 4.2 change, which is not made until agreed.
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
   - Folders are typed in, because a native file dialog needs a crate outside the crate selection (OI-110).
   - On open, changed or missing model files are reported in the status bar (OI-45, in part).
   - The project tree lists models, setups, operations and tools. Choosing a setup shows it.
   - The status bar shows the A direction, the scene notes and the G-code verification notice (NFR-12).

## Not built yet (rest of slice 8)

| Item | What remains | Note |
| --- | --- | --- |
| 4 (UI-06) | Reorder, enable, disable and duplicate operations in the tree, through commands and undoable | Operations have no enabled flag, so enable and disable need project schema 5 (OI-111) |
| 5 (UI-07) | Parameter forms from strategy and tool schemas; the custom-profile editor with live preview (TL-03 closes here) | |
| 6 | Import dialog with the check report, the tessellation statement and the unit question (OI-51, OI-52); model transform and orientation controls | |
| 7 (NFR-03) | The scripted 60-second camera-path benchmark on `openchuck_testkit::parts::benchmark_model_1m`; the CI software-rendered smoke run (OI-112) | |
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
- Not measured: NFR-03 (OI-09); Windows and macOS launch from CI artefacts.

## Alternatives considered

- **Wait for the gate** (decision 0002 item 2), or **build only the stack-independent parts**. Not chosen by the owner (item 1).
- **Renderer inside the egui callback in `openchuck-viewport`** (the spike's layout). Rejected: it ties the viewport crate to egui, and the fallback would then touch the renderer as well.
- **Per-vertex normals.** They cost 12 more bytes a vertex and break at hard edges unless vertices are split. The derivative normal gives flat shading for free.
- **Stock as a translucent mesh.** Translucency needs sorting. An outline shows the stock without hiding the part.

## Consequences

- No document is edited by this record. The verification plan 4.2 change for OI-13 waits on agreement (item 4).
- The work breakdown shows slice 8 in progress and SP-2's status mentions the provisional start.
- When OI-09 and OI-11 close, update decision 0002 and this record. If the stack changes, supersede item 2's glue.

## Open items

| ID | Item |
| --- | --- |
| OI-110 | Native file dialogs. Folders are typed in until a dialog crate (for example `rfd`) has an NFR-14 record and an AGPL-compatible licence check. |
| OI-111 | Operations need an enabled flag for UI-06 (enable and disable), which changes the project schema (5) and its migration, and says how regeneration and posting treat a disabled operation. |
| OI-112 | CI smoke test for UI-01 and NFR-02: install a software Vulkan driver (Mesa lavapipe) on the Linux runner and set `OPENCHUCK_REQUIRE_ADAPTER=1` for `openchuck-viewport`'s render tests. Check what the Windows (WARP) and macOS runners offer, and add a scripted launch of the binary. |
