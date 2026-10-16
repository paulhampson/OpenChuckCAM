# 0037: Slice 10 items 1 and 6: picking, hover and the selection panel; the pick budget (OI-12)

- **Status:** accepted by the project owner (15 Oct 2026): the pick budget (item 1, option 2 of the options put to them) and the rest of the record as written. It relies on accepted records [0002](0002-sp-2-viewport-spike.md) (picking by CPU ray casts against a BVH), [0029](0029-slice-8-viewport-and-shell.md) (the toolkit-free renderer and the scene view model) and [0032](0032-slice-10-mesh-selection.md) (selection sets, roles, the unresolved-reference errors). Built on branch `claude/lane-m-slice-10-picking` (lane M of the work breakdown's parallel work; it needed lanes A and C, both on `main`).
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-12 and OI-139 are closed by this record; OI-170 to OI-173 are new.

Numbering: `main` holds records up to 0039 and OI-169 (workholding took OI-158 to OI-162 and the OpenCASCADE cache OI-169 while this branch was open), and an unmerged branch holds 0038 (the interface specification) and OI-163 to OI-166, so this record keeps 0037 and takes OI-170 to OI-173 (first OI-159 to OI-162). It takes no project schema number: a pick is not stored, the sets and roles it edits are the ones schema 7 already holds, and the viewport's state is not in the project.

## Context

Work breakdown slice 10 item 1 (ray-cast picking and hover highlight; the viewport only draws the highlight) and item 6 (the selection panel listing sets and their contents). UI-04 and UI-05 (P1), PRJ-06 and OP-02 through the sets of record 0032. Architecture sections 5.4 (picking in `openchuck-geom`) and 5.8 (no CAM logic in the UI). Verification plan sections 4.1 and 5. Decision 0029 item 6 left `Camera::ray` for this slice; decision 0032 left the visible warning for an unresolved reference to it (OI-139).

## Decision

1. **The pick budget (OI-12; accepted by the owner).** On a model of one million triangles:
   - a ray cast takes at most **1 ms** at the 95th percentile (1,024 rays on a fixed grid), so hover fits inside one frame of the 33 ms limit of NFR-03 with room to spare;
   - the hierarchy is built in at most **2 s**, once, in the scene job (NFR-04), not on the interface thread.

   Hosted CI runners are noisy, so the test allows five times these figures when `CI` is set; the strict figures are checked locally and on the reference machines. Verification plan section 4.1 gains the row. The 10 ms of decision 0002 was about 500 times the measured ray cast and would not have caught a fall back to a linear scan; these limits keep a margin of over 200 times for the ray cast and about 2 times for the build over the measurements below.

   Measured here (a cloud container, 1,002,000 triangles of the NFR-03 model):

   | Measure | Debug (test) build | Release build |
   | --- | --- | --- |
   | Ray cast, p95 of 1,024 rays | 4.4 µs | 3.1 µs |
   | Hierarchy build | 1.04 s | 0.76 s |
   | Scene job (read, place, feature edges, hierarchy) | not measured | 1.5 s |
   | One pick through the scene's picker | | 16 µs |

   The test is `req_ui_04_picking_a_million_triangles_stays_within_its_budget` in `openchuck-geom/tests/pick.rs`. It runs in about a second, so it is not in `.config/nextest.toml`.
2. **Picking in `openchuck-geom` (item 1; UI-04).** `openchuck_geom::pick::MeshPicker` builds the `parry3d-f64` hierarchy for a mesh and answers `pick(origin, direction)` with the first triangle hit (`PickHit`: triangle index, distance, point), or `None`. Both sides of a triangle can be hit, so a ray that starts inside a closed part meets the wall. The distance comes from our own triangle test in double precision, as in `openchuck-path` (decision 0015); no parry type is public. A direction need not be a unit vector; a zero or non-finite ray, an empty mesh and a bad index are errors, never panics (NFR-07). Of two triangles at the same distance the answer is the same on every run. This is the first use of `parry3d-f64` in `openchuck-geom` (the dependency record already named it; `docs/dependencies.md` is updated).
3. **The scene carries the picker.** `SceneMesh` gains `model` (the model a part mesh shows) and `SceneView` gains `picking` (`openchuck_app::pick::Picking`, a shared handle to a `ScenePicker`; two handles are equal when they are the same picker or pickers built from the same meshes, so two builds of one scene compare equal, as the scene's determinism test needs, NFR-06). `build_scene` builds the picker with the part meshes in the same job, so the interface thread never waits for it. A preview scene has none. `ScenePicker::pick` returns the nearest triangle over all parts with its model, mesh index, point and normal. These are in `openchuck-app`, which the semver check does not cover; no public struct of the library family changed.
4. **What a click selects (UI-04).** `PickMode` maps a pick to the existing `RegionRequest`: one triangle; the connected shell (flood fill); a smooth patch (flood fill by angle, default 30°); every triangle facing the same way (select by direction, default 10°). A click replaces the working selection, Shift adds, Ctrl or Command removes; picking on another model starts a new selection. Triangle numbers mean the file they were picked on, so the working selection is dropped, with a notice, when the hash the project records for its model changes or the model is gone (otherwise saving it would stamp the new file's hash on the wrong triangles); undo, a new placement and another setup leave the hash alone and keep it. The region modes run as the existing `start_select_region` job (cancellable, reported through the job events), so the interface stays responsive on a large model.
5. **Highlights are an overlay in the renderer.** `Renderer::set_overlay` uploads a second small scene that is drawn over the first: its meshes win the depth test over the surface they lie on (a negative depth bias against the part's positive one, no depth write) and are still hidden by anything in front of them. Hover and selection changes replace only the overlay, so moving the pointer never uploads the 1M-triangle scene again. `openchuck_app::scene::triangles_mesh` builds a compact mesh of chosen triangles. Colours: working selection orange, the chosen set green, the triangle under the pointer yellow. Tested with a real software adapter: `req_ui_04_an_overlay_wins_over_the_surface_it_covers_and_not_over_what_hides_it`.
6. **The panel's view model (item 6; UI-05, OP-02).** In `openchuck-app`:
   - `selection_summaries(file_warnings)` lists each set with its name, model, kind, count, the operations that use it and in which roles, and a problem when one can be seen without reading the model: the model is gone, the set was made on a different version of the file than the project holds, it selects faces or edges on a mesh, it selects nothing, or the file on disk has changed since it was added. `file_warnings` is `model_file_warnings`, which hashes the files, so the front end calls it when a project opens and passes the answer for each redraw.
   - `set_operation_selection(operation, role, set, included)` puts a set in or takes it out of an operation's role (`SelectionRole`: machine, boundary, avoid, check), as one undoable step; a boundary holds one set.
   - `delete_selection(set)` removes a set, refusing while an operation uses it and naming the operations.
7. **In the interface.** The tree gains a Selections node: "Select on the part…" and one row per set, in red with the reason on hover when it has a problem. Choosing either shows the panel on the right and turns picking on in the viewport; the panel has the click modes with their angle, the working selection (clear, save as a named set), and for the chosen set its model, uses and problem, add, remove or replace with the working selection, rename and delete. An operation's properties start with its selections by role, with a list to add from and a button to take one out; an operation row in the tree shows a red mark when a set it uses has a problem. Every change is an undoable command of the application.
8. **The visible warning (OI-139, closed).** A set that cannot be used is shown in red in the tree, in the panel with its reason, and on the operations that use it, before any generation. Generation still refuses it with the error of record 0032 item 7. A triangle beyond the end of the model is found only at generation, and there is no offer to select again (OI-172).

## Alternatives considered

- **A 10 ms pick budget (the figure assumed in record 0002):** 500 times the measured cost; it would pass a linear scan.
- **A budget for the selection algorithms as well:** the figure for flood fill on a million triangles is new and was not measured before today. It is now (below) and is left for the owner to set once the job reuses the scene's mesh (OI-170).
- **Drawing highlights by rebuilding the scene with the selected triangles taken out:** uploads the whole mesh on every hover.
- **Picking on the GPU with an ID buffer:** needs the window for every test; the CPU hierarchy is testable without one (architecture section 5.8, record 0002).
- **Computing the file-change warnings in the panel on every frame:** reads and hashes every model file each frame.

## What the document edits are

Verification plan section 4.1: the pick row. `docs/dependencies.md`: `parry3d-f64` is used by `openchuck-geom` as well. The work breakdown: slice 10 status and checklist, the SP-2 status cell (OI-12 closed), the numbering note. Open items: OI-12 and OI-139 closed, OI-170 to OI-173 added. No change to the requirements or the architecture.

## Evidence

- `openchuck-geom/tests/pick.rs`: `req_ui_04_*`: the face met from each of six sides, a miss and a ray pointing away, the nearest of two walls and a ray from inside, a non-unit direction, an edge hit repeated 20 times, bad rays and meshes as errors, hits on the analytic cylinder and sphere to within the facet error, agreement with testing every triangle over 1,800 rays, and the budget above.
- `openchuck-viewport/tests/render.rs`: the overlay covers the face it lies on without speckle and does not show through the part (run on Mesa's lavapipe software adapter).
- `openchuck-app/tests/picking.rs`: the scene's picker finds the triangle under a ray and its normal; each pick mode selects what it says on the cube (one triangle; the 12 of the shell; the 2 of a face at 30° and all 12 at 90°; the 2 facing the same way); compact highlight meshes; the summaries with their uses and roles; roles, boundary replacement and undo; delete refused while used; a set whose file changed or whose model version differs shows its problem.
- `openchuck-ui`: the pointer picks the triangle it is over in the top and front views, finds nothing in empty space, a click carries the keys held, and nothing is reported with picking off (`viewport.rs`); a saved set is highlighted again after the project is reopened (`selection_panel.rs`); how Shift and Ctrl join a click to the selection.
- Timings through the application on the NFR-03 model (release build, 1,002,000 triangles; background jobs): the scene with its hierarchy 1.5 s; one pick 16 µs; flood fill 1.5 s; flood fill by angle 1.7 s; select by direction 0.3 s. The first selects the whole model, the second the cylinder's barrel.

## Open items

- **OI-12 (closed):** the pick budget, item 1.
- **OI-139 (closed):** the visible warning, item 8; the offer to select again and the triangle-beyond-the-end case are OI-172.
- **OI-170:** each region click re-reads, places and (for flood fills) builds the edge map of the whole model in its job: 0.3 to 1.7 s at a million triangles. Reuse the scene's placed mesh and cache the edge map per scene, then set a budget for the selection algorithms.
- **OI-171:** hover shows the one triangle under the pointer, not what the chosen mode would select; highlights have no outline; there is no box or lasso selection and no select by the model's own face groups. Decide after the owner has used the panel.
- **OI-172:** a triangle beyond the end of the model is found only at generation, and a set with a problem has no "select again" button (record 0032 item 7, OI-139).
- **OI-173:** the strict budget figures are measured in one cloud container. Run them on the reference machines (4.2) with the NFR-03 run, and measure the picker's memory (it keeps its own copy of each part mesh beside the scene's).
