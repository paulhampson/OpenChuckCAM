# 0166: Drawing long toolpaths: one draw per look, hidden layers kept, a path benchmark (lane BUG-PERF; issue 200)

- **Status:** proposed (lane BUG-PERF, 20 Oct 2026). Awaits the owner's acceptance. It adds to [decision 0040](0040-slice-9-display-and-playback.md) item 9 (the 1,000,000-point display budget) and does not change it.
- **Date:** 20 Oct 2026.
- **Open items:** OI-590 (the owner re-measures on the laptop and a reference machine) and OI-591 (the upload of a new path still runs on the interface thread), both opened here. OI-176 stays open. OI-177 (a level of detail that follows the zoom) was put to the owner; the owner chose to build it in this lane, in [decision 0167](0167-path-detail-follows-the-zoom.md).

## Context

Trial feedback F-44 (GitHub issue 200): a long toolpath makes the view slow while orbiting and while playing back. The owner's laptop has Intel UHD 620 graphics, which is below the reference hardware of verification plan 4.2 (Intel Xe or recent AMD integrated graphics): UHD 620 (Gen9.5, 24 execution units) has about a quarter to a fifth of the GPU throughput of an Intel Xe. Lane X8's frame benchmark ([decision 0131](0131-slice-22-verification.md) item 3) used a 1,002,000-triangle model and no path, so no run so far measured a path layer.

The example project attached to issue 200 could not be downloaded from the cloud container (the attachment link answers 403 without a signed-in browser), so the lane first measured a stand-in near the display budget. The coordinator then committed the project (`docs/trial-feedback-projects/blisk_slow_toothpath_rendering/` on branch `claude/upbeat-davinci-jk46yd`, commit 52b87b5), and it was measured too.

**Measured first, before any change.** The `path_frames` example of `openchuck-viewport` draws a blisk finishing path offscreen while the camera follows the 4.2 scripted path, built as the playback panel builds its layer: one line per run of moves of one class. 24 blades, 200 passes, 200 cut points: 19,200 runs, 979,200 moves, 998,400 points (the display budget), 974,400 of the segments 2 pixels wide. Adapter: Mesa llvmpipe (a CPU renderer, 4 vCPU Xeon), 1366 by 768 pixels. These are not NFR-03 results.

| | Before | After |
| --- | --- | --- |
| Draw calls per frame | 19,200 | 4 |
| Uniform buffers written per frame | 19,200 | 4 |
| GPU buffers, memory | 38,400, 34.4 MB | 8, 31.3 MB |
| Upload of the path (once, interface thread) | 394 ms | 96 ms |
| CPU prepare per frame, mean | 586 ms | 0.04 ms |
| CPU encode per frame, mean | 45 ms | 0.1 ms |
| GPU submit and wait per frame, mean | 1,069 ms | 537 ms |
| Frame, mean and p95 | 1,701 ms, 1,829 ms | 537 ms, 593 ms |
| Frame, 250,000 points (4,800 runs), mean | 529 ms | 138 ms |

**The owner's project.** Regenerated with the new `path_runs` example of `openchuck-app` (13.9 s): one operation (rotary roughing, stepover 2 mm), 22,362 moves, 7,499 runs (1,892 rapid, 1,891 link, 1,891 entry, 1,825 cut), 47,199 points, 39,700 segments drawn (16,409 wide; a rapid that turns A is drawn as an arc, so rapids hold half the points). Few points, many runs:

| | Before | After |
| --- | --- | --- |
| Draw calls and uniform writes per frame | 7,499 | 4 |
| Upload of the path | 112 ms | 3.3 ms |
| CPU prepare per frame, mean | 700 ms | 0.04 ms |
| CPU encode per frame, mean | 17 ms | 0.1 ms |
| GPU submit and wait per frame, mean | 193 ms | 28 ms |
| Frame, mean and p95 | 911 ms, 995 ms | 28 ms, 38 ms |

For comparison, the X8 run of the 1,002,000-triangle model on the same kind of renderer: mean 117 ms, p95 164 ms.

**Diagnosis.** The cost was not the GPU. Each run was its own draw item, so each frame wrote one uniform buffer and issued one draw call per run. That work grows with the number of runs, is the same on any GPU, and alone was about 18 times the 33 ms frame budget; playback redraws every frame, so it was slow too. What remains is GPU work proportional to the points drawn: 5.86 million vertex shader runs per frame at the budget (six corners per wide segment), most of the segments shorter than a pixel at the fitted zoom.

## Decision

1. **Lines that look alike are drawn as one.** The renderer puts the lines of a layer together by their look (slot, drawn on top or not, colour, width, line style), in the order each look first appears, into one vertex buffer, one uniform buffer and one draw. Each line keeps its own distance along from 0, so its dash pattern is as it was. The order between lines of different looks can change, which shows only where two of them cross at the same depth. A toolpath is then at most four draws (rapid, link, entry, cut) whatever its length. This holds for every layer, not only toolpaths.
2. **A hidden layer stays uploaded.** "Show or hide" and Simulate's "Show" no longer send the renderer an empty layer; the renderer keeps what it uploaded, writes no uniforms for a hidden layer and leaves it out of the frame (`Renderer::set_layer_shown`). Showing it again uploads nothing (NFR-04).
3. **What a layer holds can be counted.** `Renderer::layer_stats(layer)` gives a layer's line draws, meshes, GPU buffers and bytes, and `Renderer::uniform_writes()` the uniform buffers the last frame wrote. The tests and the benchmark use them.
4. **The long-path benchmark.** `cargo run --release -p openchuck-viewport --example path_frames -- [--blades N] [--passes N] [--cut-points N] [--frames N] [--size WxH] [--merged] [--runs FILE]` reports runs, moves, points, draw calls, uniform writes, vertex shader runs, GPU memory, the upload time, and the CPU and GPU time of each frame on whatever adapter wgpu finds. With `--runs FILE` it draws a real project's path, which `cargo run --release -p openchuck-app --example path_runs -- PROJECT FILE [LEVEL]` writes (with the project's moves, runs and points at each level). It is the measurement for OI-590 and OI-176 on the reference machines.
5. **The level of detail that follows the zoom is its own record.** The lane asked the owner whether to build OI-177 now, recommending to wait for OI-590; the owner chose to build it in this lane: [decision 0167](0167-path-detail-follows-the-zoom.md). The budget of decision 0040 item 9 stays.

Toolpaths, the simulation and G-code are not touched (GC-07, NFR-06): only how the viewport draws.

## Consequences

- Tests: `req_nfr_03_lines_of_one_look_share_one_batch` (the batches, their order and dash counts, what keeps lines apart), `req_nfr_03_a_path_of_many_runs_is_one_draw_per_look_and_looks_the_same` (30 runs are one draw and one uniform write, two looks two draws, and the picture equals the runs drawn in separate draws), `req_nfr_04_a_hidden_layer_stays_uploaded_and_is_not_drawn`, and the viewport test that a hidden path is sent to the renderer unchanged.
- The display budget's memory is about 31 MB, not the 24 MB of decision 0040 item 9: two 16-byte vertices per segment.
- `openchuck-viewport` gains `LayerStats`, `Renderer::layer_stats`, `Renderer::uniform_writes`, `Renderer::set_layer_shown` and `Renderer::is_layer_shown` (additive).
- Not measured: the frame time on a GPU, on the owner's laptop or on a 4.2 reference machine (OI-590). Issue 200 is referred to, not closed, until then.

## Rejected

- **Joining the runs of a class in the playback panel** instead of the renderer: it would fix toolpaths only, and the overlay and the findings draw many lines too.
- **Building the level of detail that follows the zoom first** (OI-177): the per-run cost was the measured cause; the level was built after it ([decision 0167](0167-path-detail-follows-the-zoom.md)).
