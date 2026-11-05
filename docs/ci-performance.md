# Keeping CI fast

Rules for any change that adds tests, jobs or build settings, or touches `.github/workflows/`. The design and the measurements behind them are in [decision 0516](decisions/0516-ci-architecture.md); the earlier rounds are [0052](decisions/0052-ci-speed-up.md) and [0358](decisions/0358-ci-time-where-it-goes-and-what-to-change.md). Nothing here changes what is tested: every test still runs on every pull request on Windows and Linux (0052, 0516), and output stays deterministic (GC-07, NFR-06).

## The budget

- **20 jobs at once for the whole project, free hosted runners only, no paid cache** (0358). Several pull requests and pushes to `main` share them, so runner minutes, not one run's length, set how many runs finish an hour.
- **GitHub's Free plan allows 5 macOS jobs at once** (not checked for this account), which is why macOS queues longest. Do not add macOS jobs to a pull request run.
- **A new job needs an estimate before it is added**: its runner minutes per run (set-up of about 1 to 2 min, plus build, plus work), how many runs it is due on, and whether it sits on the critical path (the longest chain from `Detect changes` to `CI required checks`). Put the estimate in the pull request.

## Where checks run

| Run | What it runs |
| --- | --- |
| Draft pull request | `Detect changes` and Fast checks only; `CI required checks` fails by design (0055) |
| Ready pull request | Everything due by the path filters, on Windows and Linux; macOS builds every target and runs UI smoke and the STEP kernel job, but no macOS test share and no macOS Each library alone (0516) |
| Push to `main` | Light when the merged tree is the tested tree (Ubuntu and macOS tests, macOS Each library alone), full otherwise (0358, 0516) |
| Nightly | The full `ci.yml` matrix at 04:23 UTC, coverage (0102), fuzz, sanitizer, long tests, beta |

A check moves off the pull request run only by an owner's decision recorded in a decision record, with the risk named (0516 did this for the macOS tests). Never weaken or skip a test to save time.

## Test shares

- Each system builds once (`.github/workflows/build-and-test.yml`) into a nextest archive and runs it in shares: Windows 8, Ubuntu 6, macOS 2 (counts in `tools/ci/main-run-mode.sh`, tested by `tools/ci/main-run-mode.test.sh`). The shares are nextest's `count` partition; `tools/ci/check-shares.sh` fails the build if a share is empty or the shares do not hold every test exactly once.
- **Shares are dealt by count today, not by time.** `--partition count:K/N` gives share K every Nth test of the full list (by binary, then by name), so the shares hold even numbers of tests but uneven times, and moving a test to another file or module only moves it to whichever share its new place in the list lands on: it is not a way to balance. Dealing by measured time is the planned improvement (OI-2028, not built).
- **Rebalancing by hand**, when one share is regularly the last job by more than a few minutes (read the share times of the last few runs):
  1. Find the longest test in that share (its nextest log lists each test's time). If it is over a few minutes, split it by case (below); this is what fixed the 31-minute Windows share (OI-2036).
  2. If no single test dominates, change that system's share count in `tools/ci/main-run-mode.sh` (`WIN_SHARDS`, `UBU_SHARDS`, `MAC_SHARDS`) and its test, which deals every test again. Each extra share costs about 2 min of runner time on Linux and 3 on Windows, plus a runner while it runs.
  3. Give the before and after times of the slowest and fastest shares in the pull request.
- Windows is listed first in every matrix, and no Windows check may be removed.

## Test binaries

- In `openchuck-app` and `openchuck-path` the integration tests are one binary each (0516). A new file under `tests/` is added as a module of that crate's `tests/integration.rs` (`#[path = "file.rs"] mod file;`); `cargo xtask trace` fails on a file no test target builds. Do not add a new `[[test]]` target to these crates unless it needs a feature the others do not (the `kernel-tests` files).
- In other crates prefer adding to an existing test file over a new one: each file is a binary that links every dependency again.

## Keep each test short

- A test over about a minute uninstrumented is listed in both profiles of `.config/nextest.toml` (excluded from `coverage`, selected by `long`), with its time (AGENTS.md). A test over 10 minutes is flagged for an owner's decision before it is kept.
- The longest test in a share sets that share's time. Split a long test by case where the cases are independent, keeping the requirement IDs in each name and the same checks: the first-cut golden-programs test became one test per thread count, each compared with the golden programs (OI-2036). A determinism test split this way still proves the outputs equal, because each is compared with the same golden file.
- Contention on a 4-core runner makes a long test 1.6 to 3.8 times slower than alone (0052); the times in CI are what count.

## Build size and disk

- CI builds with `CARGO_PROFILE_DEV_DEBUG` and `CARGO_PROFILE_TEST_DEBUG` set to `line-tables-only` (none in Fast checks and coverage). A full-debug-info test build was 23 GB; the Ubuntu runner has about 19 GB free (0516). Do not turn full debug info back on in CI, and do not change `Cargo.toml`'s profiles to work around it.
- Every build job prints `df -h` and `du -sh target` before and after the build. Read them when a change adds dependencies or test binaries.
- Do not add large artefacts: an upload keeps a short retention (`retention-days`) and only what a later job or a person needs. The `rust-cache` entries save on `main` only (0358); a new cache needs room under the 10 GB repository limit.

## Measuring

Before and after any CI change, give: the wall time of a full pull request run, the critical path (which job finished last), the runner minutes (sum of job durations), the peak jobs at once and the queue time, from the Actions API, over as many runs as the change allows (OI-1415, OI-2035). A manual `ci.yml` run on the branch (`workflow_dispatch`) runs the full layout, macOS shares included, before a pull request can (a draft runs no heavy job).

## Checklist for a pull request that touches `.github/workflows/`

- [ ] The runner-minute estimate and the effect on the critical path are in the pull request.
- [ ] Every new heavy job has the draft condition and a path-filter or event condition, is in the `needs` of `CI required checks`, and is checked there with the right "due" rule (a job in `needs` that is never checked fails the run, 0102).
- [ ] A called workflow repeats the settings it needs (`env`, permissions): it does not see the caller's `env`.
- [ ] A job that needs another system's job does not wait on a whole matrix job by accident (`needs` waits for every leg).
- [ ] `tools/ci/main-run-mode.test.sh` passes, and the light, full, pull request and scheduled cases still give the jobs they should.
- [ ] Permissions stay read-only except where a job must write (the issue jobs).
- [ ] Windows stays first; no Windows check is removed; any check that leaves the pull request run has an owner's decision.
- [ ] The change was run once (a manual run, or the ready pull request run) and its times are reported.
