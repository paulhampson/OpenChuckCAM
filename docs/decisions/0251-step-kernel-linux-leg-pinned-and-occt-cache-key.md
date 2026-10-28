# 0251: Pin the Linux leg of the STEP kernel job to ubuntu-24.04 and put the runner image in the OpenCASCADE cache key

- **Status:** proposed (27 Oct 2026). The project owner instructed the task on 27 Oct 2026; the owner has not yet accepted this record. It amends [decision 0250](0250-ui-smoke-linux-leg-pinned-to-ubuntu-24-04.md) (same cause, second job) and [decision 0039](0039-occt-install-cache.md) (the cache key); it changes no job name, no test and no Rust code.
- **Date:** 27 Oct 2026.
- **Lane:** CIMESA2, `claude/ci-step-kernel-ubuntu-2404`.
- **Requirements:** none directly; the job is the CI proof of IMP-02, NFR-06, NFR-07 and NFR-13 (decision 0003).
- **Numbers:** takes 0251 and OI-1003: the highest numbers on `main` and on every remote branch on 27 Oct 2026 were 0250 and OI-1002 (OI-994 to OI-998 are claimed by open pull requests). No project schema number.

## Context

`STEP kernel (ubuntu-latest)` is required. On PR #360 (run 40557039760, job 122375869083, 27 Oct, runner image `ubuntu-26.04` 20261012.163.1) `cargo test --locked -p openchuck-import --features kernel --test step` ended 15 passed, 3 failed: `req_nfr_13_the_worker_exits_cleanly` (`the worker ended with signal: 11 (SIGSEGV)`), `req_nfr_13_the_worker_reports_a_bad_file_with_exit_code_two` (exit code `None`, expected `Some(2)`) and `req_nfr_06_the_answer_is_byte_identical_between_runs` (`assertion failed: ok`). That pull request changes only `openchuck-path` and docs. The earlier runs on PRs #370 (job 122122858314) and #367 (job 122146475919) passed in about 3 minutes, from the restored cache.

What the logs show: the action `.github/actions/occt` found no cache for `occt-7.8.1-Linux-X64-plain-r1` (evicted), rebuilt OpenCASCADE 7.8.1 on the 26.04 image (rustc 1.97.0, cmake 4.4.4, 21 min 15 s) and saved the install under the same key although the tests then failed, as designed (decision 0039: a later failing step must not lose the install). The 3 failing tests are the ones that run the worker, the process linked against that install.

What they do not show: why the worker crashes. Compiler, glibc and OpenCASCADE 7.8.1 against the new toolchain are all candidates; no log distinguishes them, and the crash was **not reproduced**. This record claims no cause.

## Decision

1. The Linux leg of `step-kernel` in `ci.yml` runs on `ubuntu-24.04` through a matrix field `runner` (`runs-on: ${{ matrix.runner || matrix.os }}`), as decision 0250 did for `ui-smoke`. `matrix.os` stays `ubuntu-latest`, so the job name and the `CI required checks` aggregator are unchanged. Windows and macOS are unchanged.
2. The cache key of `.github/actions/occt` gets the runner image on Linux: `occt-<version>-Linux-X64-<variant>-r<revision>-<ImageOS>` (`ubuntu24` or `ubuntu26`). `ImageOS` is set by the hosted runner images and names the OS release, not the weekly image build, so the key does not churn weekly; the fallback is `unknown` where it is unset (a self-hosted runner). An install built on one image can therefore never be restored on another, which is what let a 26.04-built install sit under a key a 24.04 job reads.
3. `occt-cache.yml`'s Linux `occt` job (the daily seed on `main`) runs on `ubuntu-24.04` too. Without this it would seed the `ubuntu26` key and the pinned job would find no cache on `main`'s scope and rebuild OpenCASCADE on every pull request. The `occt-asan` job, `nightly.yml`, `release.yml` and `build-archives.yml` stay on `ubuntu-latest`; their keys now carry `ubuntu26`, so they build and cache on their own image, consistent with each other, and are no longer affected by the 24.04 pin. They are exposed to the same crash on 26.04 (OI-1003); this record does not change them.
4. `revision` is **not** bumped (the owner's brief suggested `r1` to `r2`). A bump changes the key on every platform and cold-rebuilds Windows (48 to 71 min) and macOS for nothing, since only Linux is affected; the new Linux suffix already makes the key differ from the poisoned `r1` one, and the cache action matches keys exactly (no `restore-keys`). Bump `revision` if a non-Linux install is ever found bad.

GitHub scopes a cache written on a pull request to that pull request, so the 26.04-built install is, as far as the logs show, reachable only from PR #360's ref (and could have reached `main` only through the daily seed). With the new key neither can be restored by the pinned job.

## Alternatives considered

- **Bump `revision` only.** Leaves the daily seed on 26.04 writing `r2`, which the 24.04 job would then restore. Rejected.
- **`ImageVersion` in the key.** Changes weekly, so the 20 to 30 minute rebuild would recur every week. Rejected.
- **Make the job non-blocking or skip the three tests.** Excluded by the owner's instruction.
- **Chase the crash now** (rebuild with other flags, debug the worker). No evidence yet; a guess would change the build without a reason. Tracked in OI-1003.

## Verification

- Run here: `cargo fmt --all --check`, `cargo xtask deps`, `licences`, `records`, `trace`, `conflicts`, `testdata` (results in the pull request); `actionlint` is stated there as run or not available.
- **Not reproduced:** the 26.04 crash (no 26.04 environment here), and the cache key behaviour (GitHub Actions cannot run locally; `ImageOS` availability in the composite action's shell is documented by the runner images, not checked in a log).
- The proof on GitHub is this pull request: it edits `ci.yml`, the action and `occt-cache.yml`, so the full CI and `OpenCASCADE cache` run once it is marked ready; `STEP kernel (ubuntu-latest)` must pass on `ubuntu-24.04` after a cold rebuild (about 20 to 30 minutes on a miss under the new key; timeout 150 minutes).

## Consequences and limits

- The Linux STEP kernel job no longer tests the toolchain Ubuntu 26.04 users get; that is a coverage gap until OI-1003 closes. Both pins (0250 and this one) should be revisited together.
- The first run under the new key rebuilds OpenCASCADE once on Linux.

## Open items

- **OI-1003:** cause of the worker SIGSEGV on 26.04 not established; revisit this pin and the `ui-smoke` pin (OI-1002) together; `nightly.yml`, `release.yml`, `build-archives.yml` and `occt-asan` build OpenCASCADE on `ubuntu-latest` (26.04) and may hit the same crash. Owner: the owner.
