# 0167: The toolpath's level of detail follows the zoom (lane BUG-PERF; issue 200; OI-177)

- **Status:** proposed (lane BUG-PERF, 20 Oct 2026). The owner chose to build it in this lane (option B of the lane's question, relayed by the coordinator session on 20 Oct 2026); the record itself awaits the owner's acceptance. It amends [decision 0040](0040-slice-9-display-and-playback.md) item 9 (one level for the whole path, chosen by the budget) and follows [decision 0166](0166-long-toolpath-drawing.md).
- **Date:** 20 Oct 2026.
- **Open items:** closes OI-177. OI-590 (the owner's re-measurement) covers this change too.

## Context

Decision 0040 item 9 draws the path at the finest of seven levels (0, 0.01, 0.03, 0.1, 0.3, 1 and 3 mm) whose visible points fit the 1,000,000-point budget, whatever the zoom. OI-177 deferred choosing the level from the pixel size until a path was slow to draw. Issue 200 is that case. Decision 0166 removed the cost that grew with the number of runs; what remains is GPU work in proportion to the points drawn, and at the fitted zoom of a long path most segments are shorter than a pixel. The owner asked for the level to follow the zoom, made off the interface thread (NFR-04), with no visible jump, the budget kept, and the output untouched (GC-07, NFR-06).

## Decision

1. **The level is the coarsest within half a pixel.** `pathview::zoom_level(current, mm_per_pixel)` gives the coarsest level whose tolerance is at most half the millimetres one physical pixel spans. A drawn line is then never more than half a pixel from the path.
2. **Where the pixel is measured.** `Viewport::mm_per_pixel` takes the view's height at the camera's target over the view's height in physical pixels; in perspective it scales that to the nearest point of the scene's sphere, where a pixel spans least, so the bound holds on the nearest part of the path. Before the first frame, or with the eye inside the scene, there is no size and level 0 is asked for.
3. **No swapping back and forth.** The level goes finer at once when the zoom makes the current one coarser than half a pixel, and coarser only once half a pixel is 1.25 times the coarser level's tolerance. A zoom within a level asks for nothing, so orbiting, panning and playback never make a new path.
4. **The budget still wins.** `ToolpathDisplay::lines_for(visible, max_points, min_level)` draws at the zoom's level or at the level the 1,000,000-point budget needs, whichever is coarser; `lines` is `lines_for` with level 0, as before. `Application::start_path_lines` takes the zoom's level.
5. **Off the interface thread, with no jump.** The playback panel compares the zoom's level with the one it asked for each frame; when it changes, it starts the existing path-lines job with the new level and keeps drawing the old path until the job hands the new one over. Each level is made once and kept (decision 0040 item 9).
6. **For drawing only.** The level never reaches the runs, the timeline, playback, the simulation or the G-code; the panel's note still says the level drawn ("Drawing N of M points, simplified to within T mm").

## Consequences

- Tests: `req_nfr_03_the_level_follows_the_pixel_size_at_the_zoom` (the choice, the margin, a zoom within a level, bad sizes), `req_nfr_03_req_gc_07_the_zoom_level_never_goes_past_the_budget_or_into_the_output` (the budget wins; the runs, a playback pose, the toolpath and the G-code are unchanged), `req_nfr_03_the_pixel_size_is_taken_where_the_scene_is_nearest` and `req_nfr_03_req_nfr_04_the_path_detail_follows_the_zoom_off_the_interface_thread` (the job is started and the old path stays until it returns; a zoom within the level makes nothing; zoomed in, every point; the playback pose does not move).
- Measured on the owner's project (decision 0166 harness, llvmpipe): at the fitted zoom the level is 2 (39,150 of 39,700 segments, 31 ms a frame, the same as full detail within the noise); far out, level 6 (13,466 segments, 14 ms). On this project the gain is small, since it has few points; it matters for paths near the budget.
- `openchuck-app`: `pathview::zoom_level`, `PIXEL_SHARE`, `ToolpathDisplay::lines_for`; `start_path_lines` gains a parameter (a breaking change in an application crate, outside the library family).
- The level is one for the whole path. Full detail only near the tool, which OI-177 also named, is not built; a part of the path far behind the target is drawn finer than it needs.

## Rejected

- **Several levels uploaded at once and chosen on the GPU:** more memory for every path, for a saving only while zooming.
- **A level per run by its distance from the eye:** the level would change while orbiting, so orbiting would make new paths.
