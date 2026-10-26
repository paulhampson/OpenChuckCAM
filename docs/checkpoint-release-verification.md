# CHECKPOINT released integration verification

Issue #295; PR #342; accepted decision [0243](decisions/0243-project-recovery-checkpoints.md); OI-989 **OPEN**.

Merge Manager released the lane in [comment 6402459401](https://github.com/paulhampson/OpenChuckCAM/pull/342#issuecomment-6402459401) after marking the PR ready. The integrated main is `dafef455d5c31a2db032cff5eeaa4b689821247a`, including ROUGHPATH #340 and BUG-SIMSYNC #341. The lane does not mark ready, merge the PR or enable auto-merge.

## Integration and persisted compatibility

Only the app/project changelogs and work-breakdown lane rows conflicted. Both sides were retained. The decision-index union was reordered numerically; accepted ROUGHPATH 0241/0242 and CHECKPOINT 0243 remain distinct. Executable files merged automatically, preserving BUG-EDIT guards, ROUGHPATH and playback synchronization.

At released integration head `31964ad95eb7fbd60e52e3145a8f5d88a0db9424`, the project model, project file codec and both recovery codec/storage source blobs are byte-identical to the pre-integration head `b2e4f859a03108036d955f3f249005558b1982d7`. Project schema stays **18** and independent recovery container format stays **1**; there is no new schema allocation or migration. The workspace version is now main's **0.21.0**. Canonical generation paths and rotary boundaries are generated state, outside checkpoint contents.

The new requirement-based compatibility case migrates a schema-1 fixture, verifies omitted/false/true `ramp_back_and_forth` values survive complete schema-18 project-text and format-1 snapshot round trips, and rejects a future project schema even when the container checksum is valid. Existing incompatible-container, partial/corrupt input, process-kill, publication/rotation, Save/close/instance and model-reference cases are retained. The selected compatibility case passed in 0.030 seconds before the full integration run.

The first locked fuzz check detected six stale workspace path-package versions after the main version bump. `cargo update --workspace --offline` updated those six 0.20.0 entries to 0.21.0. No third-party dependency version or pin changed. The subsequent locked check and seeded recovery-reader smoke passed (285,425 executions, 61 seconds, no crash).

## Allocation audit

The audit enumerated 31 local/remote refs (two local heads, 28 remote branch heads and symbolic `origin/HEAD`), inspected actual high-number record files/statuses, indexes, OI registers and project-schema constants, and checked the persisted Merge Manager queue at `0951ac7c0266e877b5ad12a708f408d6ad8c6d3d`. GitHub's open-PR collection contained #342, draft #321 (active queue position 2, not yet released), draft #284 and dependency draft #131; all four head SHAs matched fetched refs.

| Allocation | Authoritative disposition |
| --- | --- |
| 0234 / OI-984 | STEPALL-M #321, active queue position 2 and draft awaiting release; owner-accepted record |
| 0235–0238 | Merged assignments |
| 0239 | Persisted FINSTEP reservation; not reassigned |
| 0240 | Merged OUTPUT-1 |
| 0241 / 0242 | Merged ROUGHPATH; provisional former 0238/0239 do not supersede current assignments |
| 0243 / OI-989 | CHECKPOINT accepted under original 0241, administrative renumber preserves full acceptance; OI-989 OPEN |
| OI-990 | STEPALL-M performance follow-up; no collision with OI-989 |
| Project schema 18 / recovery format 1 | Unchanged, no new project-schema claim |

The original CHECKPOINT full-text acceptance and substantive record remain unchanged. No new record, acceptance, OI or schema number is claimed. Older merged branch refs can retain provisional filenames; current main and persisted allocations govern.

| Audited ref | Head | High record-file numbers | Highest OI | Project schema |
| --- | --- | --- | ---: | --- |
| `local/codex/lane-checkpoint-crash-recovery` | `b2e4f859a03108036d955f3f249005558b1982d7` | 0230, 0231, 0233, 0235, 0236, 0237, 0238, 0240, 0243 | 989 | 18 |
| `local/work` | `a2605495d4e05d323d4cc3f85a4f4d6be118072f` | 0230, 0231 | 982 | 18 |
| `origin/HEAD` | `dafef455d5c31a2db032cff5eeaa4b689821247a` | 0230, 0231, 0233, 0235, 0236, 0237, 0238, 0240, 0241, 0242 | 988 | 18 |
| `origin/cla-signatures` | `55d93fb3659b0c57f6a7cf1d106ee10db48d3e0f` | — | 0 | — |
| `origin/claude/ci-speed-up-libs-nextest` | `af6a09bc6d3c777a069a32154b2790fc582c493c` | — | 0 | 10 |
| `origin/claude/coordinator-notes-owner-test-list` | `226e082e61f1f4a8e875bf62b276498037582f0f` | — | 0 | 13 |
| `origin/claude/docs-gates-4-5-test-instructions` | `9573685001117c4e71a236c555f34138fb21cbad` | 0232 | 0 | 17 |
| `origin/claude/lane-stepall-m-measure` | `0b4daa3e2af685765428520b9027145b70e60e5d` | 0230, 0231, 0233, 0234 | 990 | 18 |
| `origin/claude/upbeat-davinci-jk46yd` | `c92fa236ccc8862e5a52e80eade69c73b835d7e0` | — | 0 | 13 |
| `origin/codex/lane-checkpoint-crash-recovery` | `b2e4f859a03108036d955f3f249005558b1982d7` | 0230, 0231, 0233, 0235, 0236, 0237, 0238, 0240, 0243 | 989 | 18 |
| `origin/codex/lane-roughpath-unwind-and-ramp` | `3c8b10aaab11e7ee6f37d73f827d2987f272dfed` | 0230, 0231, 0233, 0238, 0239 | 983 | 18 |
| `origin/dependabot/cargo/ui-graphics-49e8759968` | `5f16937630190956027ee9b5f638f8bbdc45c9a0` | 0230, 0231 | 982 | 18 |
| `origin/dlaud` | `9573685001117c4e71a236c555f34138fb21cbad` | 0232 | 0 | 17 |
| `origin/docs/decision-0016-sp-9-exact-surfaces` | `68789930b1eaebdc91f64f33de6e77de46a2ea31` | — | 0 | 4 |
| `origin/main` | `dafef455d5c31a2db032cff5eeaa4b689821247a` | 0230, 0231, 0233, 0235, 0236, 0237, 0238, 0240, 0241, 0242 | 988 | 18 |
| `origin/management/merge-queue` | `0951ac7c0266e877b5ad12a708f408d6ad8c6d3d` | 0230, 0231 | 982 | 18 |
| `origin/spike/linuxcnc-fixture-checks` | `92dd315d11ed0c85c5b4ee50466af5dd595a3bbc` | — | 0 | — |
| `origin/spike/sp-1-step-kernel` | `f4a7d04db83c33e9cb4ea5f03ded1b9bbaa62834` | — | 0 | — |
| `origin/spike/sp-10-template-engine` | `a27134ebc0373714b5dcf10a5b879269c0868840` | — | 0 | 4 |
| `origin/spike/sp-11-swarf` | `d88f11077c1ac41a18115120a5b27e9d173baaf6` | — | 0 | 4 |
| `origin/spike/sp-12-z1-conventions` | `4b1ea01d5eb772e2663e6e6d8698ae671b2ff898` | — | 0 | — |
| `origin/spike/sp-13-tool-profiles` | `cc24ea2848920c6fb4d23a40b9fb2a66a445eded` | — | 0 | 1 |
| `origin/spike/sp-14-short-moves` | `69d532f0723b89667ae792a90a4d4a571300fe39` | — | 0 | 17 |
| `origin/spike/sp-2-viewport` | `d911116d86d5ca369de851991087f933359bd7e4` | — | 0 | — |
| `origin/spike/sp-3-wrapped-path-gcode` | `0e251d2d40973cbc043d31e8672a6932ff3228d3` | — | 0 | — |
| `origin/spike/sp-4-ray-cast-sweep` | `e5ffcd8daaaae4797ed69326fa7783409b36215e` | — | 0 | 3 |
| `origin/spike/sp-5-gcode-reader` | `0fee6129e5a7051a36e49ec26781c2bda493422d` | — | 0 | — |
| `origin/spike/sp-6-simulation-stock` | `7156bc1d196ab0038fa92389b0cbc176a5d13d96` | — | 0 | 4 |
| `origin/spike/sp-7-2d-offsets` | `1f54adde2d9dc46ee4ed5dfcea8e513075921ac2` | — | 0 | 4 |
| `origin/spike/sp-8-project-format` | `db95e1558b0d34aeaa46272012e79674dcc06422` | — | 0 | — |
| `origin/spike/sp-9-exact-surfaces` | `b0dca8b57621b23245992d5db6e70510ee0ec918` | — | 0 | 4 |

## Fresh validation

The complete default-feature project/app/UI run passed **1,164/1,164** in
**1,059.681 seconds**: 166 project, 515 app and 483 UI tests. It includes all
46 PRJ-08 cases, forced termination at staging/publication/rotation, malformed
and incompatible recovery input, pending fields, Save/close/instance races,
model warnings and BUG-SIMSYNC playback synchronization. Fourteen tests exceeded
one minute; all are classified in coverage/long profiles. The longest individual
was the existing reference-job determinism case, **276.091 seconds**. None
exceeded ten minutes. The existing per-selection allowance test took 76.405
seconds and was added to both long-test profiles; default execution is retained.
Background validation builds shared this cloud quota during the full run.

Workspace all-target Clippy with warnings denied, workspace/fuzz formatting,
project/app doctests, all seven `xtask` checks and 99/99 P1 traceability pass.
Root and fuzz lockfile audits pass; `cargo deny --all-features check` passes;
`cargo about generate --locked about.hbs` generated licence notices. All four
library-family semver checks pass against integrated main: types 6.503 s,
path 18.331 s, post 6.526 s and gcode 2.463 s, each with 202 passing lints and
58 inapplicable skips. No further version bump is required. The default-feature
production Linux UI binary also builds. Whitespace is checked on the PR delta
against integrated main; upstream accepted records and raw evidence logs retain
their original bytes.

### Fresh own-test coverage

| Crate | Own coverage tests | Covered source lines | Line coverage |
| --- | --- | ---: | ---: |
| Project | 166 passed, 11.682 s | 3,821 / 4,221 | 90.52% |
| App | 494 passed, 537.612 s; 21 classified long excluded | 16,871 / 18,413 | 91.63% |
| UI | 481 passed, 619.453 s; 2 classified long excluded | 25,756 / 29,390 | 87.64% |

These are instrumented own-crate lower bounds, not workspace coverage or
uninstrumented cost measurements. All remain below the 95% development target,
which is not a merge gate. No product source is excluded to improve coverage.
The changed-source missing lines were reviewed: defensive UUID/path/size and
publication failures, intentionally killed subprocess fixture lines, scheduler
epoch rejection, UI-facing app entry points and worker/index failure messages,
and native UI callbacks/error installation paths remain cold. Consumer UI
coverage is not credited to the app's own-test result. New-module line coverage
is reader 97.67%, store 86.65%, scheduler 98.96%, app orchestration 74.25% and
UI orchestration 90.39%. Reports were retained before cleaning coverage and
fuzz instrumented builds; no instrumentation is committed.

### Isolated uninstrumented measurements

After all concurrent builds/tests finished, each release example case ran in a
separate process and published five states. Timings include normal publication
validation/retention. Dispatch includes the project clone. Poll gaps include a
one-millisecond sleep and are not GPU frame times; additional peak RSS is Linux
`VmHWM` above the pre-worker RSS. No user project was used.

| Case | Project text bytes | Max dispatch | Median publication | Max poll gap | Additional peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| 10 operations | 2,310 | 0.112 ms | 1.111 ms | 1.098 ms | 692 KiB |
| 1,000 operations | 229,717 | 0.746 ms | 52.834 ms | 1.194 ms | 5,168 KiB |
| 100 operations / 200,000 selected triangles | 2,310,902 | 2.444 ms | 495.489 ms | 2.661 ms | 86,884 KiB |

An isolated uninstrumented whole-shell headless case with 201 operations passed
in 0.788 seconds and measured maximum debug draw times of **29.771 ms baseline**
and **28.053 ms during publication**, with only one frame overlapping capture.
That limited cloud observation does not establish the **17 ms** native/reference
budget. It supplies no universal UI responsiveness or filesystem durability
claim; OI-989 remains open.

## Native and reference verification

This environment provides x86_64 Linux only. Windows/macOS native interaction, OS/filesystem behavior and owner reference-hardware responsiveness are unavailable locally and remain **OI-989 OPEN**. Default-feature integration does not execute OCCT-gated STEP tests. Existing CI platform jobs and human hardware confirmation are distinct evidence; no native success, universal power-loss durability or 17 ms reference-hardware claim is made. The original issue requires `awaiting-fix-feedback` at merge unless that human evidence is supplied.


## Windows Clippy correction after completed CI

On released head `31964ad95eb7fbd60e52e3145a8f5d88a0db9424`, CI run
40232949059 completed with one underlying failure: Windows build/test shard 1
job 121288421897 stopped at Clippy because `sync_dir` used its `dir` argument
only inside `#[cfg(unix)]`. Windows therefore reported an unused variable,
and `-D warnings` made that error blocking. The required-check aggregator
failed as a consequence. All other CI jobs succeeded, including macOS build
and tests, Windows shard 2, all three platform STEP/kernel and UI-smoke jobs,
all fuzz groups, all Gate 3 shares, library checks and fast checks. End-to-end
run 40232948955 and CodeRabbit also completed successfully on the exact head.

The correction explicitly discards the path argument under `#[cfg(not(unix))]`.
It adds no lint allowance and preserves the existing behavior: Unix flushes the
directory; non-Unix platforms flush file contents without a directory flush.
No checksum, publication/rotation/retirement behavior, public API, dependency,
project schema, recovery format, allocation or owner policy changes.

Validation of this narrow correction: a strict compile probe of the actual
non-Unix function body reproduced the unused-variable error before and passed
after. It selected the non-Unix cfg body on Linux and is not a native Windows
runtime/filesystem claim. Workspace all-target Clippy with warnings denied and
formatting pass. All 166 project tests pass uninstrumented in 0.841 seconds;
the fresh own-project coverage run also passes 166 tests in 0.865 seconds,
covering 3,821/4,221 source lines (90.52%). Existing missing-line gaps remain;
instrumented artifacts were cleaned. Actual Windows CI must verify the eventual
corrected head.

CodeRabbit's two Minor findings (handled staging-failure cleanup and bounded
busy-lock retry) remain deferred under Merge Manager's correction-batch policy.
No new durability or native/reference-hardware success is inferred from these
automated CI results. OI-989 remains OPEN and the owner feedback requirement
for issue #295 is unchanged.
