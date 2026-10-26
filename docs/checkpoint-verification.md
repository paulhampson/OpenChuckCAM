# CHECKPOINT verification

Issue #295; accepted [decision 0243](decisions/0243-project-recovery-checkpoints.md).
The lane starts from `dc610e26dd260d69b5fc859270e7bf5fc913a3bb`;
BUG-EDIT merged at `1a90435654c734c05e4fa3f110c4a7f3f8e5859f`.
Project schema stays 18. Recovery containers have their independent version 1.

This document records the initial implementation validation. Fresh released-main
integration results and allocation/compatibility evidence are in
[CHECKPOINT released verification](checkpoint-release-verification.md).

## Requirement-based regressions

The first scheduler, snapshot reader, store, worker, interval-setting and
Locate tests failed to compile before their corresponding APIs existed.
Subsequent behavioural regressions failed before fixes for stale interim
publication, dismissal after explicit discard, corrupted retirement metadata,
reused moved-project paths, reuse of a session's revision namespace on reopen,
obsolete folder-inspection results, following external ignore/lock symlinks,
duplicate publication on retry, recovery replacing another unsaved project,
missing project-name labels, failed Locate losing its retry choice, and an
extreme capture timestamp reaching an unsafe platform date conversion.
These are requirements and lifecycle checks, not coverage-only assertions.

The subprocess tests kill the child at staged, published and rotation
boundaries. They read every remaining valid choice afterward and then publish
another state to complete retention. Other tests cover missing or changed
models, corrupted/partial/incompatible/truncated containers, invalid file maps,
the size limit, write failures, read-only recovery, Git status and ordinary
project enumeration, same-series stale revisions, Save/close races, worker
failure during close, pending-field exclusion and normal departure warnings.

The headless modal fixtures need a preparation frame before their buttons are
painted. Correcting that test assumption was not a product fix. Native windows
and actual Windows/macOS interaction are distinct from these headless frames.

## Uninstrumented measurements

`cargo run --release -p openchuck-app --example checkpoint_measure -- <case>`
runs each synthetic case in a separate process and publishes five states,
including periodic retention and the independent interim. No user projects
are used. Numbers are from the managed Linux environment, without coverage
instrumentation; they are not reference-hardware claims.

| Synthetic case | Project text bytes | Maximum caller dispatch | Median background publication | Maximum controller poll gap | Additional peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| 10 operations | 2,310 | 0.020 ms | 1.089 ms | 1.074 ms | 756 KiB |
| 1,000 operations | 229,717 | 0.337 ms | 53.113 ms | 1.197 ms | 4,752 KiB |
| 100 operations, 200,000 selected triangles | 2,310,902 | 1.964 ms | 463.219 ms | 2.971 ms | 84,140 KiB |

Dispatch includes the committed-project clone. Controller poll gaps include
a one-millisecond sleep; they are not GPU frame times. Peak RSS is Linux
`VmHWM` minus the pre-worker RSS, with project construction already complete.
Publication includes rendering, checksumming, flushing, validation and rotation.

A separate whole-shell headless test uses 201 operations. An isolated debug
run measured a maximum draw time of 29.004 ms before publication and 29.130 ms
during publication. This does not establish the verification plan's 17 ms
poll-and-draw budget: the baseline already exceeds it in this cloud debug case.
The checkpoint did not introduce an observed additional stall. Native and
reference-hardware measurements remain open in OI-989; no GPU responsiveness
claim follows from the controller benchmark.

## Fuzzing

The new `project_recovery` target exercises raw envelopes and arbitrary inner
payloads with a refreshed checksum, so integrity checks do not prevent fuzzing
the inner reader. A seeded local smoke completed 148,362 executions in 61
seconds without a crash. Local smoke time does not increase nightly CPU-hour
totals. CI runs this target in group B and the nightly matrix; corpus input is
under `fuzz/seeds/project_recovery/`.

## Validation status

The initial changed-crate run passed all 1,147 tests in 666.206 seconds.
After integrating the normal recovery-departure guard and project-name labels,
the complete project/UI run passed 645 tests in 437.962 seconds. After the
Locate retry fix, all 11 CHECKPOINT UI tests passed in 8.611 seconds. After
the display-only timestamp guard, all 12 passed in 1.325 seconds. Timestamps
outside years 1970–9999 remain intact in storage and are labelled as outside
the display calendar; checked platform date arithmetic preserves the choices. No
individual uninstrumented test exceeded ten minutes. The existing cut-surface
rendering matrix crossed one minute (63.867 seconds in the contended run) and
is now excluded from local coverage and selected by the nightly long profile;
the existing whole-playback-dock test took 221.434 seconds in that run and was
already classified. Existing app long tests remain classified in both profiles.

`cargo clippy --workspace --all-targets --locked -- -D warnings`, workspace
and fuzz-workspace formatting, project/app doctests (one project example;
no app doctests), and all seven `cargo xtask all` checks passed. The initial
repository check found a missing SPDX header in the new fuzz seed; the header
was added and the full check passed. The traceability report covers all
99 P1 requirements.

The final CHECKPOINT UI cleanup passed all 12 tests in 1.717 seconds
uninstrumented. Recovery requires an explicit Recover, Keep saved or Discard
choice. Closing without answering preserves the existing recovery data.
Recovery uses the read-only application-layer reader.

### Own-test line coverage

These are each crate's own tests, with other crates' source omitted from the
reported totals. They are lower bounds, not whole-workspace coverage. All
three remain below the repository's 95% development target, which is not a
merge gate. No product source is excluded to improve the numbers.

| Crate | Own coverage run | Covered source lines | Line coverage |
| --- | --- | ---: | ---: |
| `openchuck-project` | 165 passed, 3.435 s | 3,822 / 4,221 | 90.55% |
| `openchuck-app` | 494 passed, 781.373 s; 12 long tests excluded | 16,645 / 18,099 | 91.97% |
| `openchuck-ui` | 480 passed, 630.051 s; 2 long tests excluded; focused own-test refresh after final UI cleanup | 25,720 / 29,353 | 87.62% |

The UI report accumulates the complete own suite and the final 12-test
CHECKPOINT refreshes using `--no-clean` (the final 12 passed in 1.468 seconds); the final binaries produce the report
without profile mismatch warnings. Coverage timings are instrumented and are
not used for long-test classification. The existing long tests still run
uninstrumented in the default/nightly profiles.

New-module line coverage: snapshot reader 97.67%, recovery store 86.65%,
scheduler 98.96%, application recovery/discovery 74.25%, and UI recovery
90.39%. The missing-line reports and changed source were reviewed:

- Persistence gaps include defensive UUID/path/identity checks, failing
  staging/rename/sync/cleanup operations and exhausted ordering. The killed
  subprocess fixture cannot flush its own coverage profile; its parent still
  checks complete surviving snapshots and subsequent retention.
- Application gaps include UI-facing elapsed/readiness/configuration and
  dismissal/discard entry points, periodic worker dispatch, and worker/index
  failure diagnostics. UI consumer coverage is not credited to this own-test
  percentage. Malformed/oversized registry entries and some path/error
  branches remain untested by the own app suite.
- UI gaps include native close callbacks, changing the interval widget,
  startup/attachment failures, worker/retirement errors, failed application
  installation and platform date fallback. Unused writable-recovery handling
  was removed rather than covered artificially. Native interaction and
  hardware verification remain OI-989 work.

Validation disables debug symbols and incremental caches to control generated
artifact size; optimization and assertions are unchanged. Coverage reports
were retained before cleaning the instrumented workspace and fuzz build
artifacts. No generated instrumentation is committed.

## Remaining limitations

- Checkpoints reference model files; they do not back up geometry bytes.
- Encoded containers are limited to 64 MiB; failure retains earlier checkpoints.
- Arbitrarily moved folders need Open or Locate; inaccessible entries remain.
- Damaged and partial files are preserved, so they can use space beyond the
  four valid retained checkpoint slots until explicitly handled by the user.
- Directory flushing is platform-dependent; no universal power-loss guarantee.
- OI-989 remains open for native interaction, OS/filesystem checks and reference
  hardware. Record acceptance does not close it.
