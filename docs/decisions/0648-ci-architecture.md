# 0648: CI architecture: where each check runs, the disk-full failures, sharding within 20 runners, Windows first (lane CI-ARCH)

- **Status:** accepted, option C, by the project owner on 4 Nov 2026, in this lane's own session (decision 0049: "Proceed with option C and do the change in this lane as part of the same PR"). Option C includes option B's macOS tier, so the macOS tests and macOS Each library alone leave the pull request run (they run on every push to `main` and nightly); verification plan section 8 is amended for it in the same pull request. Proposed the same day by lane CI-ARCH. The number is the next free one across `main` and the unmerged branches on 4 Nov 2026 (highest 0647, on `claude/lane-view-orient-535`); the Merge Manager assigns the final number at release.
- **Date:** 4 Nov 2026.
- **Lane:** CI-ARCH. Record branch `docs/decision-0648-ci-architecture`, docs only.
- **Requirements:** none change. GC-07 and NFR-06 (deterministic output) are kept: no option changes a test, a golden file or a release build flag (see *What stays fixed*). NFR-10 and NFR-14 are untouched (no new dependency).
- **Builds on:** [0052](0052-ci-speed-up.md), [0055](0055-push-discipline-and-draft-pull-requests.md) (draft gate, one push), [0102](0102-coverage-off-pull-requests.md) (coverage nightly, 95% per crate), [0358](0358-ci-time-where-it-goes-and-what-to-change.md) (the 20-job rule, caches on `main`, shards by binary, the light run on `main`), [0498](0498-merge-queue.md) and [0499](0499-up-to-date-rule-relaxed.md) (merges behind `main`, the Merge Manager serialises), [0244](0244-bounded-first-cut-linuxcnc-replay.md) (Gate 3 first-cut chunks).
- **Open items:** OI-2026 to OI-2036 (new; OI-2026, OI-2029 and OI-2032 closed by this pull request). Relies on, and does not close: OI-1414 (folded into OI-2028), OI-1415, OI-1416, OI-1999, OI-225, OI-1060.

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

As built (option C, see *What this pull request builds*). "PR" is a ready pull request run; "main" is the push after a merge (light or full, 0358); "nightly" is the 04:23 scheduled full run and the other scheduled workflows; "local" is a lane's pre-push run (AGENTS.md).

| Check | Local pre-push | PR | main push | Nightly |
| --- | --- | --- | --- | --- |
| rustfmt | yes | yes (Fast checks) | yes | yes |
| clippy (workspace) | yes | yes (each system's build job) | yes | yes |
| `xtask deps`, `licences`, `records`, `trace`, `testdata`, `conflicts`, `publish` | yes (`deps`, `licences`, `records`, `trace` already required) | yes (Fast checks) | yes | yes |
| `cargo deny`, `cargo audit`, `cargo about`, `cargo semver-checks` | no | yes | yes (semver: PR only, as today) | `cargo-unmaintained` weekly |
| Build, clippy, doctests and examples, **Windows** | crates changed, if on Windows | **yes, first in the matrix** | full run only | yes |
| Tests, **Windows** | crates changed, if on Windows | **yes, 8 shares from the archive** | full run only | yes |
| Build, clippy, doctests and examples, Ubuntu | crates changed | yes | yes (light and full) | yes |
| Tests, Ubuntu | crates changed | yes, 6 shares | yes (light and full), 6 shares | yes |
| Build, clippy, doctests and examples, macOS | no | **yes** (every target compiles) | yes, always | yes |
| Tests, macOS | no | **no** (owner's choice of option C) | **yes, always (light and full), 2 shares** | yes |
| Each library alone, Windows | no | **yes, 2 shares** | full run only | yes |
| Each library alone, Ubuntu | no | yes | full run only | yes |
| Each library alone, macOS | no | no (owner's choice of option C) | yes, always | yes |
| UI smoke, all three systems | no | yes | full run only | yes |
| STEP kernel, all three | no | yes, when `step` is due | full run only | yes, with the sanitizer and the mutation run in `nightly.yml` |
| Gate 3 | no | yes, when `gate3` is due, 5 shares | full run only | yes |
| Fuzz smoke | no | yes, when `fuzz` is due | no | 1 hour per target (`nightly.yml`) |
| `e2e.yml` (offline CLI, NFR-09) | no | yes | yes | no |
| Coverage (95% per crate, 0102) | crates changed | no | no | yes |
| Long tests (`long` profile) | when touched | no (the one owner-accepted test, OI-506) | no | yes |
| Beta toolchain, three systems | no | no | no | yes |

## Options

Estimates are from M1 and M4 and are replaced by measurements after each step (OI-1415). "Critical path" is the uncontended wall time of a green full pull request run (longest job plus `Detect changes`); today it is about 51 min (Windows share 2) and the measured median including queue is 69. Build time cut by debug info is taken as half on every system, from the local Linux measurement only; Windows and macOS are measured in step 1.

| | Today | A: tune in place | B: Windows first, tiered (recommended in the proposal) | **C: B plus build once and test wide (chosen by the owner)** |
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

### Option C: B plus build once and test wide (chosen)

Option C is B with the build done once per system and the tests spread over many short jobs, so that a share starts on any runner that comes free. It has three parts, and the second and third pay only once the first is done.

**C1. Fewer test binaries.** Each file under a crate's `tests/` is compiled and linked as its own executable, and each links the whole dependency tree (egui, parry, the path code, the simulator). `openchuck-app` had 55 such files (about 228 MB each with full debug info) and `openchuck-path` 58: about 17 of the 21 GB of test executables (M4), and on Windows each link also writes a PDB. C1 makes each of the two crates' files one test binary: `autotests = false` in the manifest, one `[[test]]` named `integration` whose `tests/integration.rs` is a list of `#[path = "file.rs"] mod file;`, so every file stays where it was and other lanes' edits to it still apply. The files behind the `kernel-tests` feature (the eight `step_*` and `brep_edges_step` files of `openchuck-app`) stay one binary each, because the STEP kernel job runs them by name. Test names gain the file as a module prefix (`held_region::req_set_09_…`), so the requirement IDs are still in every name and `cargo xtask trace`, which reads the last segment, still finds them. What it costs: a lane that adds a test file to these two crates adds a `mod` line too (a one-line conflict at most), and `cargo xtask trace` fails on a file no target builds, so a forgotten line cannot hide tests; a change to one test file recompiles the crate's whole test binary locally; and plain `cargo test` runs the crate's tests as threads of one process, where nextest (CI's runner) gives each its own process, as before.

**C2. Build once per system.** One build job per system builds every test target into a `cargo nextest archive` and uploads it; it also runs clippy, the doctests and the examples, which need the compiler. Test jobs check out the repository (for `testdata/`), download the archive, and run `cargo-nextest nextest run --archive-file … --workspace-remap . --extract-to .` on their share: they compile nothing and need no Rust toolchain (checked locally with no `cargo` or `rustc` on the path). The archive unpacks into the checkout's `target/`, at the same path the build used on the same system, so the paths tests bake in at compile time (`CARGO_MANIFEST_DIR`) still hold; no test outside the STEP kernel job reads a built binary from `target/` (the kernel tests' worker stays in that job).

**C3. Test wide.** A share cannot be shorter than its longest test (several minutes for the longest default-profile tests), so the gain stops at about 8 shares on Windows (about 72 min of tests) and on Ubuntu. Fewer tests per runner also means less contention for its 4 cores, which decision 0052 measured at 1.6 to 3.8 times a long test's time alone. The shares are nextest's `count` partition (every Nth test of the full list, deterministic), which spreads the tests of one file over all shares; the build job checks that no share is empty and that the shares hold the full list exactly once. macOS stays at 2 shares: GitHub documents 5 macOS jobs at once on the Free plan (not checked for this account), which fits the macOS queue of M2, and a wide macOS run would make it longer. Gate 3 goes from 3 to 5 shares so it is not the last job.

What wide sharding does not change: the rate of pull request runs at peak is bounded by runner minutes, not by the number of jobs, so it shortens a run when runners are free and costs a little per job in set-up when they are not; a run that starts wide takes the free runners first and a later run waits a little longer for them; and a test job waits for its system's build, so the build is on the critical path (C1 and the debug-info change keep it short). More jobs also meet more job-level infrastructure failures (about 3 in 1,200 jobs in M3, so roughly 8% of runs at 35 jobs against about 5% at 20); a failed share is re-run on its own.

**Measured locally after C1** (4-vCPU Linux container, `line-tables-only`, cold target directory, one sample): `target/` 4.9 GB (23 GB today, 6.4 GB with `line-tables-only` before C1), 88 test and binary executables of 2.5 GB in all (194 of 21.3 GB today), the nextest archive 467 MB written in 4 s, build 225 s. The test selection of the `default`, `long`, `coverage` and `coverage-full` profiles is identical before and after (2,912, 79, 2,834 and 2,913 tests, compared name by name with the module prefix removed), and all 23 priority overrides of `coverage-full` still match a test. The archive's test list was read with no `cargo` or `rustc` on the path, and share 1 of 8 ran from the archive unpacked into another directory: 390 tests, all passed, 950 s on the container while other builds ran beside it (a check that the archive runs, not a timing). The full check is the first CI run of the new layout (OI-2035).

### Considered and not recommended

| Option | Why not |
| --- | --- |
| sccache with the GitHub cache | It shares the 10 GB repository cache that 0358 had to rescue for OpenCASCADE; the build is the smaller part of a leg; no paid cache. |
| `cargo clean` between steps, per-step target directories | The jobs already run on separate runners; within a job the steps reuse the build. Slim debug info removes the need. |
| A free-disk step (removing Android, .NET, GHC from the runner image) by default | Frees 20 GB or more on Linux but costs 1 to 3 min a leg and more on Windows. Kept in reserve: add it to a job only if the `df` lines of step 1 show it short of space after the debug-info change. |
| `fail-fast` | One run would no longer name every failure (0052, 0055). |
| Dropping Each library alone on Windows | A Windows check; not proposed. |
| Larger runners | Breaks the free-compute rule. |

## Disk-space fixes, in order

1. **Measure first:** a `df -h` line (and `du -sh target` where it builds) before and after the build in every heavy job, on all three systems (OI-2027). No log prints it today.
2. **Slim debug info** (Option A item 1): `line-tables-only` for the test legs, `0` for Fast checks and coverage. Linux: 23 to 6.4 GB (to 3.1 GB at `0`).
3. **Coverage** (`coverage.yml`): debug info `0`, `--no-fail-fast` so one budget test does not stop 1,900 others, and a decision on how timing-budget tests behave under instrumentation (OI-2031: listing them in the `coverage` profile's exclusions like the long tests, or a budget multiplier when instrumented; the test is not to be weakened in the pull request run).
4. **Fewer test binaries** (option C1, built here).
5. **Free-disk step** as a last resort, per job.

The two "runner lost" Ubuntu jobs (M3) are suspected, not proven, disk-full cases; the `df` lines of 1 settle it (OI-2034).

## Windows plan

- Every Windows check stays on every ready pull request: Build and test (all tests, clippy, doctests, examples), Each library alone, UI smoke (WARP), STEP kernel when due, and the beta build nightly.
- The Windows legs are listed first in each matrix.
- Windows tests: from 3 shares dealt 6 / 42 / 24 min of tests, each with its own build and a partition check of about 9.5 min, to one Windows build job (first in the matrix) and 8 shares of about 9 min from its archive. Expected: 49.5 min median today, **about 20 to 23** for the build and the slowest share (to be measured, OI-2035).
- Windows Each library alone: 36 min today, **about 18 to 20** in 2 shares.
- The one Windows failure in the sample, a DNS failure at checkout (M3), was the runner's network, not the project; it needs a re-run, not a change.
- A packaging smoke test of the Windows archive already runs after each nightly build (`ui-smoke.yml`); it stays nightly.

## Gate 3

Keep it as a required job, sharded and real-time. Fix the timing flake by sampling in the servo thread (`sampler`/`halsampler`) rather than polling from Python (OI-2030; the replay driver also carries OI-1060). Until that lands, keep the one retry; when the retry also fails the job fails, as now. Only after the fix pack the shares onto fewer runners (Option B item 8).

## Local pre-push

`AGENTS.md` and `CLAUDE.md` already list the pre-push checks. Propose one command, `cargo xtask prepush`, that runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo xtask deps`, `licences`, `records`, `trace`, and `cargo nextest run` on the crates changed against `origin/main` and the crates that depend on them (from `cargo metadata`), so a lane runs one thing and red Fast checks runs (16 of the real failures in M3 were Fast checks) become rarer (OI-2033). Nothing leaves CI because of it.

## Migration plan

Proposed as one small pull request per step. The owner chose option C on 4 Nov 2026 and asked for it in this lane's pull request, so steps 1, 3, 4 and 7 (and the Gate 3 share count) are built together here; step 2 is replaced by C3's `count` partition (a share check, no dealing by time unless the measurements ask for it, OI-2028); steps 5 and 6 remain. Each part can still be reverted on its own: the debug-info lines, the matrices in `tools/ci/main-run-mode.sh`, and the two crates' `autotests` lines with `tests/integration.rs`.

| Step | Pull request | Revert | Expected effect |
| --- | --- | --- | --- |
| 1 | `df`/`du` lines; `CARGO_PROFILE_*_DEBUG` in the workflow `env` (`line-tables-only`; `0` in Fast checks and coverage); coverage `--no-fail-fast` | Remove the lines | Disk on Linux 23 to 6.4 GB; build a few minutes shorter a leg; the first disk figures on Windows and macOS |
| 2 | `xtask` share command (one listing, dealing by a committed times file, the checks of today's step, unit tests); Windows 4 shares; matrix Windows first | Restore the two lists | Windows legs ~50 to ~26 min; critical path ~36 |
| 3 | Each library alone, 2 shares on Windows | Matrix back to one | Windows Each library alone ~36 to ~19 |
| 4 | (owner) macOS tier and Ubuntu 3 shares; `CI required checks` due-by-event for the macOS jobs; the light run on `main` gains the macOS jobs; `docs/releasing.md` and the `ci.yml` header updated | Matrix back | Critical path ~26 to 29; ~80 runner minutes less a PR run; the macOS queue no longer blocks merges |
| 5 | Gate 3 sampling in the servo thread (a Gate 3 lane that can run LinuxCNC) | Restore the driver | The flake gone; then 5b: pack the shares (~35 runner minutes a run) |
| 6 | `cargo xtask prepush` | Remove the command | Fewer red Fast checks runs |
| 7 | (later, owner) Option C | Per step | Disk and link time; build-once becomes worth it |

## Owner's decision (4 Nov 2026)

The owner chose option C, asked for it to be built in this lane, in the same pull request as this record, and asked for wide sharding (one build, many test jobs that go wide when other jobs finish) to be part of it. Option C includes B's macOS tier, which this record had flagged as a waiver of a pull request check; choosing C settles OI-2026. The verification plan's section 8 said CI "builds and tests on Linux, Windows and macOS for every pull request", which the macOS tier changes, so section 8 is amended in the same pull request (the requirements and the architecture are untouched).

## What this pull request builds

- **Debug info in CI only** (`ci.yml`, `e2e.yml`, `nightly.yml`: `CARGO_PROFILE_DEV_DEBUG` and `_TEST_DEBUG` `line-tables-only`; Fast checks and `coverage.yml`: `0`). `df`/`du` lines before and after the build in each build job.
- **C1** for `openchuck-app` (47 files into `integration`, 8 kernel files kept) and `openchuck-path` (58 files into `integration`); `.config/nextest.toml` filters renamed to the new binary and module names; `cargo xtask trace` reports a test file that no target builds (tests `req_nfr_08_…`); `AGENTS.md` says how to add a test file to these crates.
- **C2 and C3** in `.github/workflows/build-and-test.yml`, a reusable workflow that `ci.yml` calls once per system (jobs `Windows`, `Ubuntu`, `macOS`, in place of `Build and test`): `build` (clippy, the archive, the share check `tools/ci/check-shares.sh`, the archive upload, doctests, examples) and `test` (one job per share, from the archive). One caller per system, not one matrix job, because a job that `needs` a matrix job waits for every leg of it: the Windows shares would wait for the macOS build. Windows 8 shares, Ubuntu 6, macOS 2; Windows listed first.
- **The macOS tier** in `tools/ci/main-run-mode.sh` (and its test): a pull request or merge-queue run builds on macOS and runs no macOS test share and no macOS Each library alone; a push to `main`, light or full, and a scheduled or manual run, run them. `CI required checks` checks the three systems' jobs (due when code changed and the run builds that system) and `libraries-alone` against the run's due flags as before.
- **Each library alone** in 2 shares on Windows and Ubuntu (`openchuck-path` by `count` partition, the other three crates in share 2; Ubuntu added after the first run, where its one share took 32.7 min).
- **Gate 3** in 5 shares instead of 3.
- **Coverage**: `--no-fail-fast` and no debug info.

Not built here: Gate 3 sampling in the servo thread (OI-2030, needs a lane that can run LinuxCNC), the timing-budget policy under coverage (OI-2031), `cargo xtask prepush` (OI-2033), and dealing shares by measured time (OI-2028, only if the `count` shares prove unbalanced).

**First run.** The new jobs cannot run on a draft pull request (decision 0055), so they were first run by hand: `ci.yml` run 41902118150 on this branch (4 Nov 2026, head `c418c31e`, `workflow_dispatch`, so the full layout with the macOS shares). Every job passed. OI-2035 keeps measuring over the next runs.

| | Windows | Ubuntu | macOS |
| --- | --- | --- | --- |
| Free disk before / after the build | 147 / 143 GB (D:) | **19 / 12 GB** | 95 / 91 GiB |
| `target/` after the build; archive | 4.2 GB; 111 MB | 5.4 GB; 446 MB | 3.7 GB; 160 MB |
| Build job (cargo build of the tests alone) | 13.1 min (7.1); clippy 2.2, examples 2.3 | 5.2 min (2.8) | 8.9 min (5.0), after 4.4 min in the queue |
| Archive download in a share | under 0.6 min | under 0.6 min | 0.3 min |
| Test shares (job minutes) | 5.8, 6.1, 8.2, 9.7, 10.1, 10.7, 15.4, **33.0** | 7.7, 8.6, 10.0, 11.1, 14.8, **23.6** | both still running after 25 min (1,472 and 1,440 tests; main and nightly only, OI-2035) |
| Build start to last share | 46.1 min | 30.7 min | over 45 min at the time of writing |
| Each library alone | 21.1 and 18.8 (2 shares) | **32.7** (1 share) | 30.6 (1 share; not on a pull request) |
| Other jobs | UI smoke 5.8, STEP kernel 12.2 | Gate 3 shares 11.3 to 14.8, STEP kernel 7.8, UI smoke 2.9, Fast checks 6.0 | UI smoke 4.3, STEP kernel 6.0 after 10.2 in the queue |

Reading: the Ubuntu runner had 19 GB free, so today's 23 GB build could not fit, which is the disk-full cause of M3; at 5.4 GB it now has 12 GB to spare. Windows has room either way. The Windows build is 13 min against about 10 min of build and partition check in each of today's three legs, and seven of the eight Windows shares take 6 to 15 min; the last share is one test: `first_cut::req_gc_07_req_nfr_06_req_nfr_08_the_committed_project_writes_the_golden_programs_at_1_2_and_n_threads` took 30.8 min on Windows and 19.5 min on Ubuntu, over the 10-minute limit of `AGENTS.md`, so it is flagged for the owner (OI-2036) and is what keeps Windows at 46 min instead of about 29. Each library alone on Ubuntu (32.7 min) was the next longest job on a pull request, so it now has 2 shares as Windows has. The `count` shares hold even numbers of tests (389 to 333 on Windows) but not even times, as expected; dealing by time stays the fallback (OI-2028). The runner minutes of the 36 jobs other than the macOS shares were 415; a pull request run has no macOS shares or macOS Each library alone, about 385 runner minutes in this run, against a median of 442 today. Windows lists 2,906 tests and Ubuntu and macOS 2,912, presumably tests that exist only on some systems (not checked).

## Risks

- **Estimates.** Build-time and archive figures are from Linux; Windows (PDB files) and macOS are measured in the first runs (OI-2035).
- **Archive transfer.** Each test job downloads its system's archive (467 MB on Linux, measured locally); a slow or failed transfer fails a share, which is re-run on its own.
- **The `count` partition is by number, not time.** A share that draws several long tests is longer than the rest; the first runs show the spread, and dealing by time (OI-2028) is the fallback.
- **macOS found after merge** (Option B item 6), named above; the owner decides.
- **More jobs per run.** A full pull request run is about 33 jobs (3 build, 14 test shares, 5 Gate 3, 3 Each library alone, 3 UI smoke, 3 STEP kernel, Fast checks and the short ones) against about 21; most are short, and the test shares start only when their build is done, so the start of a run holds about 15 runners.
- **Slim debug info** makes a debugger session on a CI binary less useful; CI binaries are not debugged, and backtraces keep file and line with `line-tables-only`.
- **Data window.** The sample is 41 hours of a busy period; the failure rates are indications.

## Open items

| OI | Item |
| --- | --- |
| OI-2026 | Closed 4 Nov 2026: the owner chose option C, with the macOS tier. |
| OI-2027 | The debug-info change and the `df`/`du` lines are built here; record the Windows and macOS sizes and build times from the first runs. |
| OI-2028 | Deal the test shares by measured time (from the nightly run's per-test times) only if the `count` shares prove unbalanced in the first runs. Takes over OI-1414's measurement. |
| OI-2029 | Closed 4 Nov 2026: Each library alone in 2 shares on Windows, built here. |
| OI-2030 | Gate 3: sample in the servo thread (`sampler`/`halsampler`); find whether the `first_cut` chunk 8 failure at A about 83,500 degrees (run 41760111114, both attempts) is timing or real; then pack the shares. (Now 5 shares.) |
| OI-2031 | Coverage nightly: debug info 0 and `--no-fail-fast` are built here; open: how timing-budget tests (`req_ui_04_picking_edges_of_a_million_triangle_part_stays_within_its_budget`) are treated under instrumentation, and confirm `openchuck-buildinfo` is listed in `xtask/coverage.toml`. |
| OI-2032 | Closed 4 Nov 2026: option C1 and C2 built here. |
| OI-2033 | `cargo xtask prepush`. |
| OI-2034 | The two Ubuntu `Build and test` jobs lost about 3 min into the first test build (runs 41656760962, 41596929921): confirm or rule out disk from the `df` figures. |
| OI-2035 | First runs of the option C layout: over 10 runs, record per system the build time, archive size and transfer time, the slowest and fastest share, wall time, runner minutes, peak jobs and queue, against M1; adjust the share counts (Windows 8, Ubuntu 6, macOS 2, Gate 3 5) if a share is idle or a system is last. |
| OI-2036 | The first-cut committed-project determinism test (`openchuck-cli::e2e first_cut::req_gc_07_req_nfr_06_req_nfr_08_the_committed_project_writes_the_golden_programs_at_1_2_and_n_threads`) took 30.8 min on Windows and 19.5 min on Ubuntu in a test share of run 41902118150 (413 s alone in a 4-vCPU container), over the 10-minute limit of `AGENTS.md`; it alone sets the Windows time (46 min, about 29 without it). The owner decides: split it into one test per thread count (each against the golden files, so the shares run them side by side; keeps GC-07 and NFR-06 on every pull request), give it a share of its own, or accept it in the nightly run only (as OI-506 did for one test; a check leaves the pull request). |
