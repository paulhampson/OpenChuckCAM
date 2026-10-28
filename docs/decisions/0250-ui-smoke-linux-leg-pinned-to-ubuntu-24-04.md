# 0250: Pin the Linux leg of the UI smoke test to the ubuntu-24.04 runner image

- **Status:** proposed (27 Oct 2026). The project owner instructed the task on 27 Oct 2026; the owner has not yet accepted this record. It amends [decision 0052](0052-ci-speed-up.md) and [decision 0101](0101-ui-smoke-of-the-built-archives.md) only in which runner image the Linux leg of `UI smoke test` uses, and it changes no job name, no test and no Rust code.
- **Date:** 27 Oct 2026.
- **Lane:** CIMESA, `claude/ci-mesa-llvmpipe-fix`.
- **Requirements:** none directly; the job is the CI proof of UI-01 and NFR-02 (verification plan 4.2, decision 0029, OI-129, OI-320).
- **Numbers:** takes 0250 and OI-1002: the highest numbers on `main` and on every remote branch on 27 Oct 2026 were 0249 and OI-1001. No project schema number.

## Context

`UI smoke test (ubuntu-latest)` is a required leg (OI-320). It passed 16 of 16 render tests on PR #363 (run 40419513133, job 121922748003, 26 Oct 23:26Z) and then failed twice, identically, on PR #358 (run 40465856325, job 122061282971 at 07:15Z and rerun job 122076447307 at 08:01Z, 27 Oct). Read from the job logs:

| | Passing | Failing |
| --- | --- | --- |
| Runner image | `ubuntu-24.04`, version 20261012.334.1, Ubuntu 24.04.5 | `ubuntu-26.04`, version 20261012.163.1, Ubuntu 26.04.1 |
| Archive | `noble` | `resolute` |
| `mesa-vulkan-drivers` | 25.2.8-0ubuntu0.24.04.4 | 26.0.8-1ubuntu0.3 |
| llvmpipe | LLVM 20.1.2 | LLVM 21.1.8 |
| Result | 16 passed | about 10 passed, then `signal: 11, SIGSEGV` |

The hosted runner provisioner version is the same in both (20261021.618), and the runner image versions are both dated 12 Oct. The cause is therefore not an unpinned `apt-get install` picking up a newer package on the same system: the `ubuntu-latest` label itself moved from Ubuntu 24.04 to Ubuntu 26.04 between the two runs, so the install resolved against a different archive. The version-pin option (`mesa-vulkan-drivers=25.2.8-...`) is not available on 26.04, whose archive carries 26.0.8 and not the noble 25.2.8 build.

## Decision

1. The Linux leg of `ui-smoke` in `ci.yml` runs on `ubuntu-24.04` through a new matrix field `runner` (`runs-on: ${{ matrix.runner || matrix.os }}`). The matrix `os` stays `ubuntu-latest`, so the job name `UI smoke test (ubuntu-latest)` and the `CI required checks` aggregator are unchanged; the Windows and macOS legs have no `runner` and run as before.
2. The pin fails loudly by construction: a runner label that no longer exists leaves the job queued and then failed, never silently on another image. The apt install, the tests, `OPENCHUCK_REQUIRE_ADAPTER` and the assertions are unchanged.
3. Nothing else changes. Other jobs on `ubuntu-latest` are not failing and are left alone; `ui-smoke.yml` (the nightly archive smoke) runs on the same runner as the build of its archive, so pinning only its smoke leg would run an archive built against a newer glibc on an older system. It is left unchanged (OI-1002).

## Alternatives considered

- **Pin Mesa to 25.2.8 on the new image.** Not offered by the resolute archive; would need a foreign archive or a downloaded build.
- **`LP_NUM_THREADS=1`, `GALLIUM_DRIVER` or `VK_ICD_FILENAMES`.** No log or upstream text shows that any of them addresses this crash, and the cause is not reproducible here; guessing would change the test environment without evidence.
- **A different software Vulkan source (a pinned Lavapipe build).** Larger and a new supply-chain input (NFR-14); only if the pin below stops working, and then with the owner's agreement.

## Verification

- Not run locally: GitHub Actions cannot run here, and Mesa 26.0.8 with LLVM 21.1.8 is not available to a 24.04 container, so the crash was **not reproduced**. The only reproduction is the two failing jobs above.
- The proof on GitHub is this pull request: `ci.yml` is edited, so the full CI runs once it is marked ready, and `UI smoke test (ubuntu-latest)` must pass on the `ubuntu-24.04` image with `mesa-vulkan-drivers` 25.2.8 as in job 121922748003.

## Consequences and limits

- Ubuntu 24.04 images are supported for years but not forever; the pin is a workaround, not a fix. The upstream cause (which of Mesa 26.0.8 or LLVM 21.1.8 crashes the render tests, and whether it is our code on a driver bug) is not established.
- The Linux leg no longer tests the Mesa that Ubuntu 26.04 users get. That is a coverage gap until OI-1002 closes.

## Open items

- **OI-1002:** revisit the `ubuntu-24.04` pin when the crash is fixed upstream or understood (a newer Mesa 26.0.x or 26.1 in the resolute archive, or an LLVM change), or when GitHub retires the 24.04 image; then return the leg to `ubuntu-latest` and decide whether `ui-smoke.yml` needs the same. Owner: the owner.
