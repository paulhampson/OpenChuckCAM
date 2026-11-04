# 0648: CI architecture: where each check runs, the disk-full failures, sharding within 20 runners, Windows first (lane CI-ARCH)

- **Status:** proposed (4 Nov 2026), for the project owner. Nothing here is built: this record changes no workflow. Each migration step below is its own lane once the owner chooses. The number is the next free one across `main` and the unmerged branches on 4 Nov 2026 (highest 0647, on `claude/lane-view-orient-535`); the Merge Manager assigns the final number at release.
- **Date:** 4 Nov 2026.
- **Lane:** CI-ARCH. Record branch `docs/decision-0648-ci-architecture`, docs only.
- **Requirements:** none change. GC-07 and NFR-06 (deterministic output) are kept: no option changes a test, a golden file or a release build flag (see *What stays fixed*). NFR-10 and NFR-14 are untouched (no new dependency).
- **Builds on:** [0052](0052-ci-speed-up.md), [0055](0055-push-discipline-and-draft-pull-requests.md) (draft gate, one push), [0102](0102-coverage-off-pull-requests.md) (coverage nightly, 95% per crate), [0358](0358-ci-time-where-it-goes-and-what-to-change.md) (the 20-job rule, caches on `main`, shards by binary, the light run on `main`), [0498](0498-merge-queue.md) and [0499](0499-up-to-date-rule-relaxed.md) (merges behind `main`, the Merge Manager serialises), [0244](0244-bounded-first-cut-linuxcnc-replay.md) (Gate 3 first-cut chunks).
- **Open items:** OI-2026 to OI-2034 (new). Relies on, and does not close: OI-1414 (folded into OI-2028), OI-1415, OI-1416, OI-1999, OI-225, OI-1060.

## The owner's rules this record keeps

At most 20 jobs at once for the whole project, free hosted runners only, no paid cache (0358). The `CI required checks` aggregator, the draft gate (0055), the CodeRabbit process rule (0499 item 3) and Gate 3 stay. Coverage stays nightly with its 95% per-crate target (0102). Windows is the owner's main platform: no Windows check is removed, and the Windows legs come first. Any check that would leave the pull request run is flagged for the owner below; none is waived by this record.

## What was measured

Source: the Actions API (read through the GitHub tools of the session). `ci.yml`: the 100 most recent completed runs, created 2 Nov 22:42 to 4 Nov 15:43 UTC (about 41 hours; 1,347 runs exist), jobs listed for 65 of them (the other 35 are draft runs of 0 to 17 min; they are counted, not timed). Failure causes: every failed or cancelled job of the same 100 runs, and of `nightly.yml` (27 runs, 12 Oct to 4 Nov), `coverage.yml` (18, 18 Oct to 4 Nov), `e2e.yml` (30), `nightly-build.yml` (30) and `occt-cache.yml` (76), from the job logs. Step times: the scheduled full run 41827937088 (4 Nov). Disk: a local build in a 4-vCPU Linux container (below). Minutes throughout; "runner minutes" is the sum of job durations.

### M1. A pull request run takes about an hour and the Windows share 2 sets it

| | Median | p90 | Max |
| --- | --- | --- | --- |
| Full pull request run, green (27 runs), wall clock including queue | 69 | 101 | 145 |
| Full pull request run, all conclusions (39), wall clock | 59 | 91 | 145 |
| Runner minutes, green full pull request run | 442 | 489 | 507 |
| Jobs held at once by one pull request run | 16 | 19 | 19 |
| Push to `main` (19 runs: 15 light, 4 full), wall clock | 44 | 54 | 63 |
| Push to `main`, runner minutes | 85 | 422 | 451 |
| Scheduled full run (2), wall clock / runner minutes | 52 / 460 | | |

Last job to finish in the 27 green full runs: `Build and test (windows-latest, shard 2 of 3)` 13 times, a macOS `Build and test` share 10 times, other 4.

Per job (median duration, p90; all events):

| Job | Median | p90 | Queue p90 |
| --- | --- | --- | --- |
| Build and test, Windows share 1 / 2 / 3 | 18.1 / **49.5** / 34.7 | 18.8 / 55.6 / 35.5 | 13 to 15 |
| Build and test, macOS share 1 / 2 | 37.7 / 39.7 | 42.5 / 47.1 | 28 to 30 |
| Build and test, Ubuntu share 1 / 2 | 37.0 / 38.3 | 38.0 / 45.5 | 10 to 17 |
| Each library alone, Windows / macOS / Ubuntu | 36.3 / 33.8 / 34.0 | 38.7 / 37.8 / 35.4 | 13 to 15 |
| Gate 3, shares 1 / 2 / 3 | 19.9 / 19.6 / 22.0 | 22.3 / 21.5 / 22.5 | 16 to 18 |
| STEP kernel, Windows / macOS / Ubuntu (OpenCASCADE cache hit) | 8.1 / 5.9 / 5.3 | 9.2 / 7.1 / 5.7 | |
| Fast checks | 7.3 | 8.3 | 9 |
| UI smoke, Windows / macOS / Ubuntu | 3.8 / 2.8 / 2.1 | | 27 (macOS) |
| Fuzz smoke A / B / C (4 runs) | 6.4 / 5.8 / 5.4 | | |
| `e2e.yml` (one job, its own workflow) | 11.1 | 22.6 | |

Step times of the Windows legs in run 41827937088: set-up, checkout, cache restore and tool install about 1.1 min a leg; **`Check the test partition` 8.8 to 10.1 min a leg** (the workspace test build plus one listing per named binary and three more; OI-1414); `Tests` 5.6 min in share 1, **42.1 min in share 2**; share 1 also clippy 1.0 and the examples 1.5. The Windows test time is about 72 min in all, dealt 6 / 42 / 24: the binary lists in `ci.yml` were dealt from run 41312032946 and the suite has grown since. Ubuntu share 2 in the same run: partition check 3.6, `Tests` 23.6. `Each library alone (windows-latest)`: the test step 36.5 min (the per-crate builds and `openchuck-path` alone).

### M2. Queue time and the 20 runners

| Runner | Jobs | Median | p90 | Max | > 15 min |
| --- | --- | --- | --- | --- | --- |
| Ubuntu | 569 | 0.1 | 10.4 | 50.7 | 42 (7%) |
| Windows | 260 | 0.1 | 13.6 | 50.6 | 24 (9%) |
| macOS | 206 | 0.1 | **28.4** | 52.2 | 41 (**20%**) |

28 more macOS jobs were cancelled before any runner took them (median 15 min waiting). Concurrency over `ci.yml` and `e2e.yml`, sampled each minute: peak 21; 20 or more in 157 of 2,590 minutes (6%) and in 14 of 44 hours. The queue is the 20-job limit being hit when two full runs overlap, plus macOS capacity on top. A full run is about 450 runner minutes with `e2e.yml`, so the 20 runners (1,200 runner minutes an hour) finish at most about 2.6 full pull request runs an hour, fewer with pushes to `main`.

### M3. Failure causes (the same 100 `ci.yml` runs, and the other workflows)

52 of the 100 `ci.yml` runs did not succeed: 30 drafts failing on purpose (decision 0055), 19 cancelled by a newer push to the same pull request (all within 1 to 4 min of that push), and the rest:

| Cause | `ci.yml` jobs | Notes |
| --- | --- | --- |
| Real | 16 | 10 Fast checks on two dependabot pull requests (`xtask records`: the pin differs from `docs/dependencies.md`); 3 STEP kernel legs, one test; 1 compile error found by `xtask trace`; 2 decision-index order rows |
| Disk full | 1 | Run 41756146428, Fast checks, Ubuntu: `ld terminated with signal 7 [Bus error]` linking the `openchuck-app` and `openchuck-ui` test binaries in `xtask trace`, then `No space left on device` in `cargo deny` and in `cargo semver-checks` |
| Runner lost (disk suspected) | 2 | Runs 41656760962 and 41596929921, Ubuntu `Build and test`, both about 3 min into `Check the test partition` (the first full test build); log missing (404), no annotation |
| macOS capacity | 3 (one run) | Run 41860781391: two macOS shares and Each library alone abandoned after 18 min with no runner; also `occt-cache.yml` run 41669093754 |
| Flake | 1 | Gate 3 share 2, run 41760111114 (M5) |
| Infrastructure, other | 1 | Windows share 2, run 41720754819: `Could not resolve host: github.com` at checkout |

No failure in the sample was on Windows or macOS alone except the one DNS failure and the macOS capacity case; every real failure was on Linux-only jobs or on all three systems at once. The window is short (41 hours), so this is an indication, not a rate.

Other workflows: `coverage.yml` failed 16 of 18 nights (12 real: `openchuck-buildinfo` not listed in `xtask/coverage.toml`, 20 to 30 Oct and again 2 Nov; 4 flaky: `req_ui_04_picking_edges_of_a_million_triangle_part_stays_within_its_budget` over its time budget under instrumentation, after which nextest stops and about 1,900 of 2,857 tests do not run; the run takes 3 to 6 hours). `nightly.yml` failed 10 of 27 (6 the CMake 4 policy, fixed by #452; 5 AddressSanitizer findings in the STEP wrapper; 1 fuzz crash; 1 suspected beta-toolchain flake). `e2e.yml`, `nightly-build.yml`: no failure.

### M4. Disk: what fills it

No workflow prints the disk use, so the hosted runners' figures are not measured (OI-2027). The size of the build was measured locally (4-vCPU Linux container, `cargo test --workspace --all-targets --locked --no-run`, a cold target directory each time, the registry warm, one sample each):

| Debug info (`CARGO_PROFILE_DEV_DEBUG` and `_TEST_DEBUG`) | `target/` | Build |
| --- | --- | --- |
| Default (`dev`: full) — what CI builds today | **23 GB** (22 GB in `deps`; 21.3 GB in 194 executables) | 376 s |
| `line-tables-only` | **6.4 GB** | 196 s |
| `0` (none) | **3.1 GB** | 229 s |

The space is in the test executables: each integration test file is its own binary and links the whole dependency graph with full debug info. `openchuck-app` has 55 test files (about 9.5 GB of binaries, about 228 MB each) and `openchuck-path` 58 (about 7.5 GB); the two `openchuck-ui` test binaries are about 610 MB each. GitHub documents a standard hosted runner as 14 GB of SSD; the free space a job actually finds depends on the image. Every leg that builds the workspace tests (each `Build and test` share, Fast checks through `xtask trace`, `Each library alone`, `e2e`) builds this 23 GB on Linux, and Fast checks adds the `cargo semver-checks` rustdoc build and the `cargo deny` image on top, which is where the one proven disk-full happened. Windows (PDB files) and macOS sizes are not measured. The coverage build is instrumented on top of full debug info; instrumentation needs no debug info (`-C instrument-coverage` writes its own mapping sections). `CARGO_INCREMENTAL` is already 0 in every workflow.

### M5. Gate 3, the one flake

Run 41760111114, share 2: `first_cut_linuxcnc.ngc`, chunk 8 of 11 (decision 0244). Attempt 1: "line 11120: the simulator never reached [70.104, 0.0, 19.542, 83595.0] after the previous move; the longest sampling gap was 50 ms". The built-in retry (attempt 2) failed too, one line earlier ("line 11119: … [95.912, 0.0, 19.542, 83520.0]"). The next five runs on that branch passed. The check samples the simulator's position from user space (`tools/linuxcnc/sim_driver.py`, `stat.poll()` then 1 ms sleep) and asks whether a sample reached each commanded end point; a 50 ms gap on a loaded 4-vCPU runner can skip a short move's end. Both attempts failing at A about 83,500 degrees is suspicious enough that this is "probably timing, not proven" (OI-2030).

## What stays fixed in every option

- `CI required checks` keeps its rule: `changes` must succeed, a draft fails, every due job must succeed, every job in `needs` is checked. A job's "due" may depend on the event (it already does for fuzz smoke and the user guide).
- The draft gate (0055), the concurrency groups (a pull request run cancels its own older run, never a run on `main`), `merge_group` (0498) and the light run on `main` (0358) keep their meaning.
- Every test still runs on every pull request on Windows and Ubuntu, and the partition check still proves each test runs exactly once.
- GC-07, NFR-06: debug info is metadata, not code generation input for the program's output; the golden G-code tests (byte for byte) and the 1, 2 and N thread tests run in the same jobs and would show any difference. The release profile already uses `line-tables-only`. Thread counts are unchanged.
- Coverage stays nightly (0102); it is not moved onto pull requests.

## Where each check runs

The recommended layout (Option B below). "PR" is a ready pull request run; "main" is the push after a merge (light or full, 0358); "nightly" is the 04:23 scheduled full run and the other scheduled workflows; "local" is a lane's pre-push run (AGENTS.md).

| Check | Local pre-push | PR | main push | Nightly |
| --- | --- | --- | --- | --- |
| rustfmt, clippy (workspace) | yes | yes (Fast checks; clippy on Windows share 1 and Ubuntu share 1) | yes | yes |
| `xtask deps`, `licences`, `records`, `trace`, `testdata`, `conflicts`, `publish` | yes (`deps`, `licences`, `records`, `trace` already required) | yes (Fast checks) | yes | yes |
| `cargo deny`, `cargo audit`, `cargo about`, `cargo semver-checks` | no | yes | yes (semver: PR only, as today) | `cargo-unmaintained` weekly |
| Build and test, **Windows** | crates changed, if on Windows | **yes, 4 shares, first in the matrix** | full run only | yes |
| Build and test, Ubuntu | crates changed | yes, 3 shares | yes (light and full) | yes |
| Build and test, macOS | no | **compile only** (owner's choice, OI-2026) | **yes, always (light and full)** | yes |
| Each library alone, Windows | no | **yes, 2 shares** | full run only | yes |
| Each library alone, Ubuntu | no | yes | full run only | yes |
| Each library alone, macOS | no | no (owner's choice, OI-2026) | yes, always | yes |
| UI smoke, Windows and Ubuntu | no | yes | full run only | yes |
| UI smoke, macOS | no | yes (in the one macOS job) | yes | yes |
| STEP kernel, all three | no | yes, when `step` is due | full run only | yes, with the sanitizer and the mutation run in `nightly.yml` |
| Gate 3 | no | yes, when `gate3` is due | full run only | yes |
| Fuzz smoke | no | yes, when `fuzz` is due | no | 1 hour per target (`nightly.yml`) |
| `e2e.yml` (offline CLI, NFR-09) | no | yes | yes | no |
| Coverage (95% per crate, 0102) | crates changed | no | no | yes |
| Long tests (`long` profile) | when touched | no (the one owner-accepted test, OI-506) | no | yes |
| Beta toolchain, three systems | no | no | no | yes |

## Options

Estimates are from M1 and M4 and are replaced by measurements after each step (OI-1415). "Critical path" is the uncontended wall time of a green full pull request run (longest job plus `Detect changes`); today it is about 51 min (Windows share 2) and the measured median including queue is 69. Build time cut by debug info is taken as half on every system, from the local Linux measurement only; Windows and macOS are measured in step 1.

| | Today | A: tune in place | **B: Windows first, tiered (recommended)** | C: B plus build once |
| --- | --- | --- | --- | --- |
| What changes | — | Slim debug info; one listing in the partition check; shares dealt by measured time; Windows 4 shares; all systems as today | A, plus Ubuntu 3 shares, Each library alone 2 shares on Windows; macOS on the PR reduced to one compile-and-smoke job, macOS tests and Each library alone on every push to `main` and nightly; Gate 3 packed once its flake is fixed | B, plus the test files of `openchuck-app` and `openchuck-path` gathered into a few binaries each, and one build job per system feeding the shares with a `cargo nextest archive` |
| Critical path, PR | ~51 | ~33 to 36 (macOS and Ubuntu shares, Each library alone) | **~25 to 29** (Gate 3 22, Windows shares, Ubuntu shares) | ~20 to 24 |
| Runner minutes, PR | ~442 | ~395 | **~360**; ~325 with Gate 3 packed | ~310 to 330 |
| Jobs over 15 min, PR | 13 (for 35 to 50 min) | 14 (for 25 to 36) | 14 (for 18 to 29); 12 packed | 12 to 16 (for 12 to 20) |
| Peak jobs at the start, PR | 16 to 19 | 20 | 20 to 21 for the first ~5 min, then 12 to 14; 18 to 19 packed | 6 to 8 (build jobs), then up to 16 |
| Push to `main`, light / full | 85 / ~420 | ~75 / ~380 | ~180 / ~440 (macOS always runs) | ~170 / ~410 |
| Pull request runs an hour, 20 runners (runner minutes only) | ~2.6 | ~2.9 | ~3.2; ~3.5 packed | ~3.5 |
| Disk on Linux | 23 GB build | 6.4 GB | 6.4 GB | far less (fewer binaries) |
| Checks leaving the PR | none | none | **macOS Build and test and macOS Each library alone (owner's waiver)** | as B |
| Effort | — | 3 small PRs | 5 to 6 small PRs | B plus a test-layout change that touches every lane |

### Option A: tune in place

1. **Slim debug info in CI only**: `CARGO_PROFILE_DEV_DEBUG` and `CARGO_PROFILE_TEST_DEBUG` set to `line-tables-only` in the workflow `env` (backtraces keep file and line), and `0` in Fast checks (it only builds to list and check) and in `coverage.yml`. Local builds and `Cargo.toml` are unchanged. Measured on Linux: 23 to 6.4 GB, build 376 to 196 s; the `rust-cache` entries shrink too, which gives the 10 GB repository cache room (0358 found it at 8.3 of 10 GiB). Changing the variable changes the cache keys once (`rust-cache` hashes `CARGO_*` variables).
2. **One listing in the partition check.** Today the step runs `cargo nextest list` once per named binary plus three more (Windows: 8.8 to 10.1 min a leg, every leg). List once as JSON and compute the shares and the checks from it with an `xtask` command (unit-tested, as `xtask replay --shard` is). The build part stays.
3. **Deal the shares by measured time, not by hand-kept lists.** A committed file of per-binary times (from the nightly full run's nextest JUnit report) is dealt by the same `xtask` command, longest first; the job fails if a binary is unknown to the file (it is dealt to the lightest share and named in the log, so a new binary still runs), if a share is empty, or if the shares do not add up. The nightly run reports when the slowest share is over 125% of the mean (OI-2028). Windows goes to 4 shares (about 72 min of tests: 6 + ~18 per share).

What it does not fix: macOS shares, Ubuntu shares and Each library alone stay at 33 to 36 min, so the run is shorter by about 15 min, not halved.

### Option B: Windows first, tiered (recommended)

A, and:

4. **Ubuntu Build and test in 3 shares** (about 67 min of tests).
5. **Each library alone in 2 shares on Windows** (the loop over the four crates stays; `openchuck-path`, most of the time, is split with nextest's `--partition hash:k/2`; the other three crates run in share 2). The Ubuntu leg stays one job (about 31 min after the build cut) unless the measurement shows it last.
6. **macOS tiered** (needs the owner, OI-2026). On a pull request, one macOS job compiles every target of the workspace (`cargo test --workspace --all-targets --no-run` and the examples), which catches a `cfg(target_os = "macos")` compile error, and runs the UI smoke steps (Metal render tests, clipboard, launch); the macOS STEP kernel leg stays when `step` is due. The two macOS Build and test shares and macOS Each library alone run on **every** push to `main`, light or full (the light run gains them), on the nightly full run and by hand. In `CI required checks` they are due when the event is not `pull_request`. If a macOS test fails on `main`, decision 0499 item 5 applies (merging stops until `main` is green).
7. **Order the matrix Windows first** (`include` order: Windows shares, then Ubuntu, then the rest), so when the 20 runners are short the Windows legs are the first to get one. GitHub does not promise to start jobs in listed order, so this is a hint, not a guarantee; it costs nothing.
8. **Gate 3** (OI-2030): record the position in the servo thread with LinuxCNC's `sampler` component and `halsampler` instead of polling from Python, so a busy runner cannot leave a gap; keep the one retry. Once the gap is gone, run the three shares' containers side by side on one or two runners (each simulator needs about one core of the four), which frees one or two runners for 20 min on every pull request run that touches the post.

**Why macOS and not Windows.** macOS legs have the worst queue (p90 28 min; one in five waits over 15 min; whole jobs abandoned for lack of a runner), are last in 10 of 27 green runs, and cost about 110 runner minutes a pull request run (two shares and Each library alone). In the sample no real failure was macOS-only. The platform code that differs on macOS (Metal, the clipboard, the OpenCASCADE build and linking) stays on the pull request.

**Risk of the macOS reduction, named:** a test that fails only on macOS (a numeric difference in `libm`, a path or file-locking difference, a timing-budget test on the slower runner) is found after the merge, on the push to `main`, not before. Under decision 0499 the Merge Manager looks at that run before the next merge, so the cost is one red `main` and a fix or revert, and the pull requests merged in between are not tested on macOS until then. Ubuntu and Windows remain as before on every pull request. If the owner prefers, the macOS shares can stay on the pull request as advisory (not in `CI required checks`): then the cost in runners does not fall, only the wait for macOS no longer blocks a merge.

**What could be reduced on Linux, and why it is not proposed:** Ubuntu Build and test is the job that the light run on `main` relies on, and it is the fastest system; Gate 3 and fuzz run only on Linux. No Linux check leaves the pull request.

### Option C: B plus build once

9. **Fewer test binaries.** Gather each of `openchuck-app`'s 55 and `openchuck-path`'s 58 test files into a few binaries (`tests/<group>/main.rs` with modules; test names, and so the requirement IDs in them, unchanged). About 17 GB of the 21 GB of test executables is these two crates linking their dependencies 113 times; fewer binaries cut disk, link time (worst on Windows) and the listing time. The shares deal by test instead of by binary.
10. **Build once per system:** one build job per system runs `cargo nextest archive`, uploads it, and the shares run `cargo nextest run --archive-file … --partition` without compiling. Only worth it after 9: with 194 binaries of 6.4 GB the upload and download (2 to 4 min a job) eat what the shared build saves (5 min a share); 0358's finding still holds that test time, not compile time, is the time.

C touches every lane's test files and the shard lists at once, so it waits for a quiet moment and an owner decision (OI-2032).

### Considered and not recommended

| Option | Why not |
| --- | --- |
| sccache with the GitHub cache | It shares the 10 GB repository cache that 0358 had to rescue for OpenCASCADE; the build is the smaller part of a leg; no paid cache. |
| `cargo clean` between steps, per-step target directories | The jobs already run on separate runners; within a job the steps reuse the build. Slim debug info removes the need. |
| A free-disk step (removing Android, .NET, GHC from the runner image) by default | Frees 20 GB or more on Linux but costs 1 to 3 min a leg and more on Windows. Kept in reserve: add it to a job only if the `df` lines of step 1 show it short of space after the debug-info change. |
| More shards everywhere without the macOS tier | Holds 20 or more runners per run; a second pull request then waits, which is what M2 shows today. |
| `fail-fast` | One run would no longer name every failure (0052, 0055). |
| Dropping Each library alone on Windows | A Windows check; not proposed. |
| Larger runners | Breaks the free-compute rule. |

## Disk-space fixes, in order

1. **Measure first:** a `df -h` line (and `du -sh target` where it builds) before and after the build in every heavy job, on all three systems (OI-2027). No log prints it today.
2. **Slim debug info** (Option A item 1): `line-tables-only` for the test legs, `0` for Fast checks and coverage. Linux: 23 to 6.4 GB (to 3.1 GB at `0`).
3. **Coverage** (`coverage.yml`): debug info `0`, `--no-fail-fast` so one budget test does not stop 1,900 others, and a decision on how timing-budget tests behave under instrumentation (OI-2031: listing them in the `coverage` profile's exclusions like the long tests, or a budget multiplier when instrumented; the test is not to be weakened in the pull request run).
4. **Fewer test binaries** (Option C item 9), if 2 is not enough.
5. **Free-disk step** as a last resort, per job.

The two "runner lost" Ubuntu jobs (M3) are suspected, not proven, disk-full cases; the `df` lines of 1 settle it (OI-2034).

## Windows plan

- Every Windows check stays on every ready pull request: Build and test (all tests, clippy, doctests, examples), Each library alone, UI smoke (WARP), STEP kernel when due, and the beta build nightly.
- The Windows legs are listed first in each matrix.
- Windows Build and test: from 3 shares dealt 6 / 42 / 24 min of tests to 4 shares of about 18, dealt by measured time and re-dealt from the nightly times; partition check from about 9.5 min to the build plus one listing; build shorter by the debug-info change (to be measured on Windows). Expected Windows legs: 49.5 min median today, **about 24 to 28** after.
- Windows Each library alone: 36 min today, **about 18 to 20** in 2 shares.
- The one Windows failure in the sample, a DNS failure at checkout (M3), was the runner's network, not the project; it needs a re-run, not a change.
- A packaging smoke test of the Windows archive already runs after each nightly build (`ui-smoke.yml`); it stays nightly.

## Gate 3

Keep it as a required job, sharded and real-time. Fix the timing flake by sampling in the servo thread (`sampler`/`halsampler`) rather than polling from Python (OI-2030; the replay driver also carries OI-1060). Until that lands, keep the one retry; when the retry also fails the job fails, as now. Only after the fix pack the shares onto fewer runners (Option B item 8).

## Local pre-push

`AGENTS.md` and `CLAUDE.md` already list the pre-push checks. Propose one command, `cargo xtask prepush`, that runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo xtask deps`, `licences`, `records`, `trace`, and `cargo nextest run` on the crates changed against `origin/main` and the crates that depend on them (from `cargo metadata`), so a lane runs one thing and red Fast checks runs (16 of the real failures in M3 were Fast checks) become rarer (OI-2033). Nothing leaves CI because of it.

## Migration plan

Each step is its own small pull request in a CI lane, can be reverted on its own, and is measured over at least 10 runs before the next (OI-1415). Steps 1 to 3 need nothing from the owner beyond choosing an option; step 4 needs the macOS decision.

| Step | Pull request | Revert | Expected effect |
| --- | --- | --- | --- |
| 1 | `df`/`du` lines; `CARGO_PROFILE_*_DEBUG` in the workflow `env` (`line-tables-only`; `0` in Fast checks and coverage); coverage `--no-fail-fast` | Remove the lines | Disk on Linux 23 to 6.4 GB; build a few minutes shorter a leg; the first disk figures on Windows and macOS |
| 2 | `xtask` share command (one listing, dealing by a committed times file, the checks of today's step, unit tests); Windows 4 shares; matrix Windows first | Restore the two lists | Windows legs ~50 to ~26 min; critical path ~36 |
| 3 | Each library alone, 2 shares on Windows | Matrix back to one | Windows Each library alone ~36 to ~19 |
| 4 | (owner) macOS tier and Ubuntu 3 shares; `CI required checks` due-by-event for the macOS jobs; the light run on `main` gains the macOS jobs; `docs/releasing.md` and the `ci.yml` header updated | Matrix back | Critical path ~26 to 29; ~80 runner minutes less a PR run; the macOS queue no longer blocks merges |
| 5 | Gate 3 sampling in the servo thread (a Gate 3 lane that can run LinuxCNC) | Restore the driver | The flake gone; then 5b: pack the shares (~35 runner minutes a run) |
| 6 | `cargo xtask prepush` | Remove the command | Fewer red Fast checks runs |
| 7 | (later, owner) Option C | Per step | Disk and link time; build-once becomes worth it |

## Risks

- **Estimates.** Build-time savings on Windows and macOS are inferred from Linux; step 1 measures them before step 2 relies on them.
- **macOS found after merge** (Option B item 6), named above; the owner decides.
- **Dealing by a times file** goes stale as tests grow; the nightly imbalance report and the "unknown binary" rule keep it from hiding a test, and a stale file costs only balance.
- **Peak of 20 to 21 jobs** in the first minutes of a B run, until Gate 3 is packed; the short jobs (UI smoke, STEP kernel, Fast checks) finish within about 8 min.
- **Slim debug info** makes a debugger session on a CI binary less useful; CI binaries are not debugged, and backtraces keep file and line with `line-tables-only`.
- **Data window.** The sample is 41 hours of a busy period; the failure rates are indications.

## Open items

| OI | Item |
| --- | --- |
| OI-2026 | The owner chooses A, B or C, and for B whether macOS Build and test and macOS Each library alone leave the pull request run (run on every push to `main` and nightly), stay required, or stay as advisory. |
| OI-2027 | Step 1: `df`/`du` in every heavy job on all three systems, and the debug-info change; record the Windows and macOS sizes and build times. |
| OI-2028 | Step 2: the `xtask` share command (one listing, dealing by measured time, the partition checks), Windows 4 shares, matrix Windows first, the nightly imbalance report. Takes over OI-1414's measurement. |
| OI-2029 | Step 3: Each library alone in 2 shares on Windows (and Ubuntu if it is last after step 2). |
| OI-2030 | Gate 3: sample in the servo thread (`sampler`/`halsampler`); find whether the `first_cut` chunk 8 failure at A about 83,500 degrees (run 41760111114, both attempts) is timing or real; then pack the shares. |
| OI-2031 | Coverage nightly: debug info 0, `--no-fail-fast`, and how timing-budget tests (`req_ui_04_picking_edges_of_a_million_triangle_part_stays_within_its_budget`) are treated under instrumentation; confirm `openchuck-buildinfo` is listed in `xtask/coverage.toml`. |
| OI-2032 | Option C: gather `openchuck-app` and `openchuck-path` test files into fewer binaries, then decide on a build-once nextest archive. |
| OI-2033 | `cargo xtask prepush`. |
| OI-2034 | The two Ubuntu `Build and test` jobs lost about 3 min into the first test build (runs 41656760962, 41596929921): confirm or rule out disk from the step 1 figures. |
