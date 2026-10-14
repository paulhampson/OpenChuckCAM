# 0026: Slice 8 design refinement: viewport and application shell

- **Status:** proposed. Slice 8 is in progress, not closed. Item 4 (OI-13) was accepted by the owner on 14 Oct 2026, and verification plan 4.2 now says what "edges" means.
- **Date:** 14 Oct 2026.
- **Branch:** `claude/slice-8-viewport-shell`.
- **Numbering:** first written as 0024 with OI-110 to OI-112 while slice 7 was unmerged. It was renumbered when `main` took 0023 to 0025 and OI-103 to OI-116.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-13, OI-45, OI-51, OI-52, OI-53, OI-54, OI-117, OI-118 and OI-120 are closed (items 4, 10, 13, 15, 16 and 17), OI-119 is open, and OI-14 is deferred to slice 11 (item 1).

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

11. **Parameter forms (item 5, operations; UI-07).**
    - **View model.** `openchuck_app::form::parameter_form` builds the form from the strategy's schema. Each field has:
      - its label and description;
      - its kind: a number with its unit symbol and range, a whole number, a flag, or a choice from a list;
      - the value the operation sets;
      - the value that applies, and where it comes from: the operation, the tool's cutting data for the stock material (TL-05), the strategy default, or missing and required.

      Notes report overrides of the cutting data, or why a required value is missing.
    - **Validation.** `Application::set_parameter` checks the value against that one parameter's schema (name, kind, finiteness, range, choice) before executing `Command::SetParameter`. A refused value returns an error naming the parameter and leaves the project and the undo history unchanged. Clearing a value is always allowed: the default or the cutting data then applies.
    - **In the UI.** Clicking an operation in the tree shows its form in a right-hand panel:
      - number boxes show the applying value as a hint and the unit beside them, and the allowed range is in the tooltip;
      - a value is applied when the box loses focus, and a refused value stays in its box with the reason under it;
      - flags are check boxes and choices are drop-down lists;
      - "Reset" clears a value the operation sets.
    - **Units.** Values are shown and entered in the units the schema states (millimetres, degrees, mm/min), which are the internal units (NFR-05). Inch display is not part of P1.

12. **Tool forms and the custom-profile editor (item 5, tools; UI-07, TL-03).**
    - **Tool form.** `openchuck_app::tool_edit::tool_form` lists a tool's dimensions as the same `Field` view model as an operation's parameters:
      - diameter, flute length, overall length, shank diameter, flutes;
      - then the fields its shape needs: corner radius; tip radius and taper half angle; included angle and tip diameter; or point angle.

      A custom-profile tool has no diameter field, because the profile's end sets it. `Application::set_tool_field` sets one field, checks the whole tool (`Tool::validate`), and refuses an invalid value naming the field, with the project unchanged.
    - **Profile editor.** The UI holds a draft `ToolProfile`:
      - it starts from the tool's own outline (`starting_profile`), so converting a standard tool keeps its shape;
      - segments can be added, deleted, turned into a line or a convex quarter-turn arc, and their end and centre points moved;
      - `check_profile` gives the verdict live, and the file import gives the same verdict, because both end in `ToolProfile::validate` under the monotone rule (decision 0012 item 2);
      - the preview (`profile_outline`) draws the revolved silhouette to scale on both sides of the axis, in green when valid and in red with the reason (naming the segment) when not;
      - Apply (`Application::set_tool_profile`) is the only step that changes the project, and it is one undoable step; the diameter follows the profile's end.
    - **TL-03.** This completes TL-03's UI half. Drawing and editing a profile was the part left from slice 4.

13. **Import dialog and model controls (item 6; IMP-03, IMP-04, IMP-05; OI-45, OI-51 and OI-52 closed).**
    - **Preview job.** `Application::start_import_preview(path, units, steps)` reads, places and checks a file that is not yet in the project. It returns the inspection and a scene of the placed model: the part with its feature edges, the triangles the checks list (flipped, degenerate, normal mismatches) as a `Problem` mesh, the open and non-manifold edges as `Problem` lines, and the rotary axis.
    - **Adding.** `Application::add_model_with` adds the model with its unit and placement as one undoable step.
    - **The dialog.** File > Import model takes a typed path (OI-117) and opens a non-modal dialog. Changing the unit or the placement re-runs the check. The dialog shows:
      - the unit used and the unit question (IMP-03), with a choice of unit;
      - the placement: as in the file, centred on the rotary axis, or any file direction turned onto the rotary axis and centred;
      - the size and the largest radius about the rotary axis;
      - the STL tessellation statement;
      - the check report.

      The viewport shows the preview, with flagged triangles outlined in red and drawn over the part (their fill would fight the part's for depth), and the open and non-manifold edges in red.
    - **Model panel.** Clicking a model in the tree shows:
      - its unit (`SetModelUnits`);
      - its placement steps, which can be added (centre, orient, translate, rotate, scale, mirror) or removed (`SetModelTransform`), each undoable;
      - its check, re-run when the unit or placement changes, with the note when the file has changed.
    - **Changed files (OI-45).** `Application::model_file_warnings` names each changed or missing file, and the UI lists them in a dialog when a project opens.
    - **OI-52 (accepted by the owner, 14 Oct 2026).** Keep the fixed unit-size thresholds. A model is checked at import, before any setup uses it, and the setup checks (SET-06) compare the part with the machine's travel once one does.

14. **CI smoke test (item 7; UI-01, NFR-02; OI-119).** A `ui-smoke` job in `ci.yml` runs on Linux, Windows and macOS. It runs `openchuck-viewport`'s offscreen render tests, then launches the real binary with `--bench testdata/stl/cube_ascii.stl --seconds 3`, which opens a window, draws the scripted path and exits by itself. Exit 0 (pass) or 2 (frame-time fail) both mean the window opened, drew and closed.
    - **Linux.** Installs Mesa's lavapipe and Xvfb and requires an adapter (`OPENCHUCK_REQUIRE_ADAPTER=1`), so it cannot pass by skipping.
    - **Windows (WARP) and macOS (the runner's Metal device).** Not yet tried on hosted runners, so they do not block until their first runs are recorded under OI-119.
    - **Local run (this container, debug build).** The command passed with exit 0: 178 frames, p95 19.4 ms on llvmpipe.

15. **Native file dialogs and the library location (OI-117, OI-53; owner's choices, 14 Oct 2026).**
    - **File dialogs.** `rfd` 0.17.2 (MIT, survey score 77, NFR-14 record in `docs/dependencies.md`) is added to `openchuck-ui` only, with its default features: the XDG portal and Wayland on Linux, so no GTK libraries are needed to build. It adds no network client (NFR-09). A Browse button beside each typed path (new, open, import) fills it from the platform's dialog. The typed path stays, so a missing portal or a cancelled dialog never blocks the user.
    - **Library location.** The owner chose the platform configuration folder: `OpenChuckCAM/machines` and `OpenChuckCAM/tools` under `$XDG_CONFIG_HOME` (else `~/.config`) on Unix, `%APPDATA%` on Windows, and `~/Library/Application Support` on macOS (`openchuck_app::library`).
    - **No `dirs` crate.** The crate selection's pick for this, `dirs` 7.0.0 (87), pulls in `option-ext`, which is MPL-2.0. MPL-2.0 is compatible with the AGPL, but it is not on the `deny.toml` allow list, and verification plan section 7 asks for an MPL crate to be checked before adoption. `directories` shares the same dependency. The rule needs only a few lines over environment variables, so it is written in-house, with tests for each platform. `dirs` is listed under "Tried and not adopted".
    - **CLI unchanged.** The CLI still takes `--library` explicitly. Defaulting it would make CLI tests depend on the machine's configuration folder.
16. **The machine copy against its library source (OI-54; the owner chose stable IDs, 14 Oct 2026).**
    - **The ID.** `MachineProfile` gains an optional `id`: opaque text, usually a UUID, shared by a library profile and every project copy made from it, as tool snapshots share an ID with library tools. It is validated as 1 to 64 characters with no leading or trailing space.
    - **Files.** Profile files move to schema 2, with a migration that only changes the number; a schema 1 profile reads with no ID. The project copy's `machine.toml` holds the ID under project schema 6 (unmerged, so no further bump). The built-in Makera Z1 has a fixed ID, so every project copy of it can be compared with it.
    - **Comparison.** `setup::available_machines(library)` lists the library's profiles, then the built-in ones. `setup::compare_machine` gives one of: no machine; source not recorded; source not found; the same; or different, with the differing fields named. The field list destructures the profile exhaustively, so a new field cannot be missed. Library profiles are searched before built-ins, so a profile the user derived from a built-in, keeping its ID, is the source.
    - **Updating.** Updating the copy is `Command::SetMachine` with the source profile, one undoable step. The project's copy wins until the user updates it (as TL-07 does for tools).
    - **UI.** A Machine node in the tree opens a panel with the profile name, the A direction in words, the values not yet confirmed on the machine, the comparison with an Update button, a chooser of available profiles, and, for a library profile with no ID, a button that gives it one by rewriting the library file.

17. **Dialogs in their own windows (OI-120; owner's request, 14 Oct 2026).**
    - **How.** `openchuck-ui`'s `floating` helper shows a dialog as an egui *immediate* viewport: its own operating-system window, drawn in the same frame as the main window, so it can borrow the shell's state. It can be moved outside the main window, for example to a second screen.
    - **Fallback.** Where the platform cannot open another window, egui draws the dialog inside the main window, so it always works.
    - **No second paint path.** A dialog draws no 3D: the import preview stays in the main window's viewport. The second paint path that OI-120 assumed is not needed.
    - **Closing.** The window's close button means Cancel.
    - **Where it is used.** The import dialog uses it now. Later floating panels (slice 9's playback controls) can use the same helper. The modal dialogs (open, new, unsaved changes, errors) stay inside the main window on purpose.

## Not built yet (rest of slice 8)

| Item | What remains | Note |
| --- | --- | --- |
| 7 (NFR-02) | First CI runs of the `ui-smoke` job (item 14, OI-119); then make Windows and macOS blocking if they pass. The benchmark is built (item 9); its run on the reference hardware is the owner's | |

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
- Parameter forms (item 11): `req_ui_07_form_shows_units_defaults_and_sources` and `req_ui_07_invalid_parameter_is_refused_and_leaves_the_project_unchanged` (`openchuck-app` `tests/operation.rs`; six refused values, each naming the parameter, with the project and the undo history unchanged); UI unit tests for reading typed text and keeping a refusal's reason.
- Tool forms and the profile editor (item 12): `openchuck-app` `tests/tool_edit.rs` has four tests.
  - `req_ui_07_tool_form_lists_the_fields_each_shape_needs`.
  - `req_ui_07_invalid_tool_values_are_refused_and_leave_the_project_unchanged`: six refusals, each naming the field.
  - `req_tl_03_profile_editor_checks_like_the_file_import`: an undercut is refused as "segment 3" both by the editor and by the real profile file parser; a valid V profile reads back through that parser unchanged, applies as one undoable step and sets the diameter.
  - `req_tl_03_profile_editor_adds_moves_and_deletes_segments`.

  The binary launched with a project and ran until stopped with no error.
- Import dialog and model controls (item 13): `openchuck-app` `tests/import_preview.rs`, three tests:
  - the preview highlights exactly the open edges, flipped triangles and non-manifold edges of the seeded defect files, and a clean cube shows none along with the tessellation statement;
  - the preview follows the unit (an inch file asks, and reads 25.4 times larger in inches) and the placement;
  - a model is added with its placement as one undoable step.

  Also `req_prj_02_model_file_warnings_name_changed_and_missing_files`, and UI tests that flagged triangles are outlined on top in one line set and that the panel's steps match the CLI's.
- Dialogs and libraries (item 15): `openchuck-app` `tests/library.rs` covers the configuration folder rule on Unix and macOS (on Unix runners) and on Windows (on Windows runners), and the default folders; a UI unit test covers where the dialogs start.
- Machine source (item 16): `req_set_01_machine_copy_is_compared_with_its_source` (`openchuck-app` `tests/setup.rs`) walks through every outcome:
  - no machine; an unrecorded source; the built-in Z1 the same;
  - a library profile derived from it, keeping its ID, with a faster feed, found as the source with `max_feeds` named;
  - the update and its undo; an orphaned ID.

  `openchuck-project` tests: the profile round trip at schema 2, a schema 1 profile read with no ID, and a bad ID refused.
- Floating dialogs (item 17): a UI unit test runs the helper in an egui context that embeds viewports (the fallback) and checks the dialog is drawn. On X11 in this container (mutter), the real binary with the import dialog open showed "Import model" as its own top-level window with its own frame, next to the "OpenChuckCAM" window (`xwininfo -root -tree`; a temporary start-up hook was used and not committed).
- Not measured: the 60 s product run on hardware; Windows and macOS launch from CI artefacts.

## Owner's local run (14 Oct 2026)

Run by the owner from the prompt in this session, on commit `cf96213` with a release build.

**Machine.** Windows 11 Pro 10.0.26200; AMD Ryzen 5 5600X (6 cores), 32 GB RAM, NVIDIA GeForce RTX 2070 SUPER (driver 32.0.16.1714) through Vulkan; 2560 by 1440 at 144 Hz; mains power. **Not the 4.2 reference class** (4 cores, 16 GB, integrated graphics, 1080p), so the margins are optimistic. The adapter was the real GPU in every run, not a software renderer. Viewport 1751 by 1003 pixels.

**Benchmark** (`openchuck-ui --bench`, 1,002,000 triangles, 60 s path, times in ms):

| Mode | Run | Frames | p50 | p95 | p99 | Max | Result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Shaded with edges | 1 | 122,658 | 0.44 | 0.77 | 1.12 | 10.78 | PASS |
| Shaded with edges | 2 | 110,890 | 0.48 | 0.91 | 1.29 | 35.57 | PASS |
| Shaded with edges | 3 | 128,998 | 0.43 | 0.71 | 0.90 | 1.88 | PASS |
| Shaded | 1 | 122,057 | 0.43 | 0.84 | 1.17 | 7.18 | PASS |
| Wireframe | 1 | 70,515 | 0.78 | 1.42 | 1.67 | 6.95 | PASS |

The median p95 with edges is 0.77 ms against 33 ms, about 43 times inside the limit on this hardware. Run 2's one 35.57 ms frame in about 111,000 did not move its p95.

**Visual check:**

| Step | Result |
| --- | --- |
| V1 Window and scene (UI-01, UI-03) | Pass |
| V2 A direction | Pass. In the Right view the A+ arrow starts at the top and turns counter-clockwise toward -Y, as on the machine |
| V3 Camera (UI-02) | Pass |
| V4 Display modes | Pass |
| V5 Machine panel | Pass |
| V6 Import dialog (OI-51, OI-117) | Pass, with the native dialog |
| V7 Model panel (IMP-05) | Pass |
| V8 Tools and profile editor | Skipped: no tools in the project; covered by tests |
| V9 Unsaved changes | Pass |

**V10, the owner's observations, and what was done:**

1. Some text is small. The UI now starts at 120 % (`DEFAULT_ZOOM`), and View has Larger, Smaller and Reset, alongside egui's Ctrl with + or -. The size is not remembered between runs, because eframe's persistence is not enabled.
2. The library folder should be creatable and openable. The machine panel now offers "Create the library folder" when it is missing and "Open the library folder" when it exists, using the platform's own `explorer`, `open` or `xdg-open` (no new dependency).
3. It was not clear that Placement moves the model in the view. The dialog now says that the 3D view previews the model as it will be added and that the orange line is the rotary axis, and the Placement label has the same tooltip.
4. The import dialog cannot be dragged outside the main window. It now opens in its own operating-system window (item 17, OI-120 closed).
5. The "A+" and "Y" labels overlapped in the Right view: the arrow ended on +Y, where the work Y label sits. The "A+" label now sits an eighth of a turn along the arc, between the top and -Y (`req_set_01_req_ui_03_a_arrow_follows_the_documented_convention` checks it).

**Build note.** The first release build hit a rustc crash compiling `naga` 30.0.1 (Windows `0xc0000374`, heap corruption). A retry with no changes built cleanly, so it is treated as a transient fault of the compiler or the environment; it is recorded in case it recurs.

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
| OI-120 | Dialogs could not leave the main window (owner's visual check, V10 item 4). Closed 14 Oct 2026: the import dialog opens in its own window (item 17). |
| OI-119 | CI smoke test for UI-01 and NFR-02: install a software Vulkan driver (Mesa lavapipe) on the Linux runner and set `OPENCHUCK_REQUIRE_ADAPTER=1` for `openchuck-viewport`'s render tests. Check what the Windows (WARP) and macOS runners offer, and add a scripted launch of the binary. |
