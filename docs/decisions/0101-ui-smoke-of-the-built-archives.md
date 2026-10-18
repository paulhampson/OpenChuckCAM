# 0101: UI smoke test of the built archives (lane TS; OI-129, OI-320, OI-321)

- **Status:** accepted by the project owner on 17 Oct 2026, relayed by the orchestrator session (decision 0049). Built by lane TS on `claude/lane-ts-ui-smoke-ci`.
- **Date:** 17 Oct 2026.
- **Open items:** OI-129 (updated), OI-320 and OI-321 (new).

Numbering: 0101 was assigned to lane TS by the orchestrator session.

## Context

UI-01 and NFR-02 require the application to run on Windows, macOS and Linux. The slice 8 launch box asks for a launch "from CI artefacts", and the slice 22 box for a UI smoke test that passes on all three systems. [Decision 0029](0029-slice-8-viewport-and-shell.md) item 14 added a `ui-smoke` job to `ci.yml` that launches a debug build made from source. Its Windows and macOS legs were left non-blocking until their first runs were seen.

### What the hosted runners were found to allow

From CI run 39049447763 (`main`, 17 Oct 2026; job logs, not run by this lane), the `ui-smoke` job's launch of `openchuck-ui --bench testdata/stl/cube_ascii.stl --seconds 3`:

| System | Adapter | Frames in 3 s | p95 | Exit |
| --- | --- | --- | --- | --- |
| Linux (`ubuntu-latest`) | llvmpipe (Vulkan, Cpu), Mesa 25.2.8, under Xvfb | 165 | 22.8 ms | 0 |
| Windows (`windows-latest`) | Microsoft Basic Render Driver, i.e. WARP (DX12, Cpu) | 27 | 114.9 ms | 2 |
| macOS (`macos-latest`, arm64) | Apple Paravirtual device (Metal, IntegratedGpu) | 284 | 37.6 ms | 2 |

All three passed the offscreen render tests (8 each). On Windows the Vulkan and GL backends found nothing usable (the GL context was 1.1), so the DX12 software adapter was chosen, as decision 0059 expects when DX12 has no hardware. Exit 2 is the 33 ms limit missed in software; the launch itself is sound.

What those runs do not cover: the program the user downloads. The job builds a debug binary from source and never opens a release archive, so a broken archive (a missing file, a binary that does not start outside the build tree) would pass.

## Decision

1. **A new workflow, `.github/workflows/ui-smoke.yml`**, launches the release archives that `nightly-build.yml` produces. It runs on `workflow_run` after a successful "Nightly build", and by hand (`workflow_dispatch`) against any Nightly build run id. It skips quietly when the nightly run built nothing (no change since the last build) and fails when a dispatched run has no archives.
2. **Per platform** (Linux, Windows, macOS) it downloads that run's archive, unpacks it, checks that the program, the command-line tool, the STEP worker, the licence, the third-party notices and the README are present, then runs the archive's own `openchuck-ui --bench testdata/stl/cube_ascii.stl --seconds 3 --out report.txt` (the corpus cube from the run's commit) under Xvfb and Mesa lavapipe on Linux, WARP on Windows, and the runner's Metal device on macOS. It passes only when the exit code is 0 or 2 and the report names an adapter and a frame count above zero. Any other exit, a missing report or a hang fails the job. No leg is `continue-on-error`.
3. **Not a pull request check.** A `workflow_run` workflow runs from the default branch only, and a check per pull request would mean building release archives again for each one (the nightly build takes 8 to 71 minutes cold for the STEP kernel). `ci.yml`'s `ui-smoke` stays the pull request gate. So the new check is **not** added to the required checks; there is no owner action for it.
4. **No change to the program.** The benchmark and the graphics choice are untouched, and no file under `apps/` or `crates/` changes. `ci.yml`, `nightly.yml` and `nightly-build.yml` are not edited.
5. **What this does not show.** CI software and paravirtual rendering proves that the archive starts, draws and exits on a clean runner. It does not prove the launch on each platform's native graphics on the reference machines, which stays a hands-on step, so the slice 8 launch box is not ticked by this change. The slice 22 smoke-test box belongs to lane X's rework of the interface and is not ticked either.

## Alternatives considered

- **Add archive jobs to `nightly-build.yml`.** Simplest wiring, but the lane may not edit it, and it would mix a test into the build that publishes the archives.
- **Build the release binary inside the new workflow on every pull request.** Tests what a pull request changes, but duplicates `ci.yml`'s job at the cost of a release build per platform per pull request, and the STEP worker would need the kernel build too. Rejected as cost without a new finding.
- **Extend `ci.yml`'s job to package and unpack an archive.** The right place for a pull request gate, but `ci.yml` is outside this lane; raised as OI-320 for the change that makes Windows and macOS block.
- **A new `--smoke` option in the program.** Not needed: `--bench` already opens the window, draws a scripted path and exits by itself, and gives a report with an adapter and frame count.

## Consequences

- `.github/workflows/ui-smoke.yml` is new. No code, project format or schema change; no requirement or architecture change; no library-family API change (decision 0048).
- The first run of the workflow can only happen after it merges (OI-321). Until then the archive launch is untested, and the record says so.
- OI-129's CI half is recorded. Making Windows and macOS block in `ci.yml` is OI-320, since both passed.

## Open items

| ID | Item |
| --- | --- |
| OI-320 | Set `blocking: true` for Windows and macOS in `ci.yml`'s `ui-smoke` job, now that both have passed (needs a `ci.yml` change outside lane TS). |
| OI-321 | After the merge, dispatch `ui-smoke.yml` once against a recent Nightly build run and record the three results, or record which platform cannot launch the archive. |
