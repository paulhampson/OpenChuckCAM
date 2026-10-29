# RAMPBUDGET — implementation and verification

This records the initial delivery checkpoint. Merge Manager subsequently marked #360 ready and released integration on 27 October 2026; see [released main integration](rampbudget-release-verification.md) for current validation.

Refs #343. Based on verified main `ba10400fff3bb7ad32526a7e540919c810e85434`, library 0.23.0. [Phase 1](rampbudget-phase1.md) contains the baseline, analytical limits, options and raw measurements. The owner directly accepted the complete record with **“Accept 0247”** on 26 October 2026 in this lane session. A refreshed remote-ref audit found AROT's separately published 0247 and OI-994–997; this lane's unchanged eight clauses are [0248](decisions/0248-ramp-clearance-budget.md), with OI-998. This is administrative renumbering, not a change or relay of acceptance.

Main subsequently advanced to `8f788bf` / library 0.24.0 through SHORTMOVE. This lane retains its verified starting base until Merge Manager releases main integration under accepted 0055 item 4. No public signature/type, project format or schema changes, no new dependency, and no library version selected. The number follows merge order if integration exposes any required break; there is no semver workaround.

## What changed and why it is conservative

`EntrySafety::verify` retains its public signature and delegates to an adaptive sweep session. At an interval midpoint, growth by leave + written/controller margin + half the interval's full translation length encloses every position of the whole body. The existing grown-profile tip-origin correction and `drop_offset + growth <= tolerance` comparison are unchanged. Positive leave may exceed tolerance; no tolerance-minus-leave step-size expression is used. An inconclusive enclosure subdivides left before right; it cannot be accepted merely from clear endpoints. At tolerance/4 resolution, an unproved interval still refuses. Non-finite arithmetic, non-progress and budget exhaustion refuse.

Every attempted enclosure is charged before the query/lookup, including coarse failures and cache hits. The shared entry budget remains 100,000, and ramp planning still limits descending legs to 10,000. Protected bounds, full tool/shank support, output precision and machine checks are performed independently; legacy target checks still cover every motion. The global stock-clear start and rapid routing are unchanged, including when earlier passes supply locally removed stock. Certification still completes before entry emission; cancellation propagates without returning an operation.

Shared assembly owns one memo session for the immutable whole target and body. Exact origin/axis/inflation bits identify a deterministic grown-body drop query. No coordinate quantisation, axial invariance, mutable identity or prior-stock assumption is used. Negative answers and contacts are cached, with a 100,000-entry FIFO bound. Protected geometry, precision, travel, current tolerance comparison and cancellation are never cached away. A standalone public `verify` call retains no cross-call cache. Option-off creates no sweep session.

Optional existing timing callbacks now include `entry clearance sweep`, `entry clearance query`, and `entry clearance cache hit`. These enable count diagnostics without changing generated output. Timing remains host-opt-in; diagnostic counts are not used as uninstrumented performance measurements.

## Tests and red/green evidence

Verification-plan OP-11/12/13/14 and SET-09/OP-21 checks, GC-07/NFR-06 determinism, NFR-04 cancellation/work bounds and NFR-07 refusals are named in tests. The separate Phase-1 deep-entry availability regression was run explicitly to fail in 1.251 s, then enabled after acceptance and failed again before implementation. Three adaptive-envelope/budget tests failed at runtime on the old implementation. Cache-scope/eviction/cancellation tests were written before their internal session API and first failed compilation because that type did not exist; subsequent runtime runs pass.

New checks cover coarse certification with leave greater than tolerance, safe refinement after an inconclusive bound, charged unsuccessful attempts and exact-budget exhaustion, actual query/cache-hit cancellation, immutable target/body replacement, exact origin/axis/inflation misses, negative/contact reuse, FIFO eviction, every corridor/approach/leg/reversal/final-connection invocation, global approach despite local stock removal, and deep-entry 1/2/available-N output identity. Existing thin-obstacle, later lower leg/reversal/final connection, shank, protected geometry, rounding, controller-counter and travel refusals remain passing. Application tests additionally check absent/false byte identity on both shipped posts, reversing-ramp posting and 1/2/N G-code identity, and indexed roughing through shared entry assembly.

Raw logs are under [rampbudget-evidence](handoffs/rampbudget-evidence/). All new tests are timed uninstrumented. No new test needs a long-test exclusion. The existing `fit_interior` blisk helical test took 60.800 s uninstrumented under concurrent validation and is now listed in both `coverage` and `long` profiles as required. The coverage run had already started with the earlier filter, so this run additionally includes that newly classified existing long test. Coverage instrumentation is measured separately, never used to classify runtime or speed-up.

## Measurements

Linux managed environment, Rust 1.97.0, nextest 0.9.150, four-core CPU quota, debug test profile. These are synthetic fixture measurements, not reference-hardware or machining evidence. The availability matrix's 20 cases retain identical geometry, stock, tool profiles, tolerances, angles and feeds from Phase 1. All now certify. Single-run matrix time: **21.005 s before, 0.535 s after**. The standalone 12 mm / 3.175 mm / 0.01 mm / 3 degree availability case now passes in **0.028 s**; the earlier 1.251 s was time to refusal, not successful generation.

Selected successful same-input matrix timings:

| Tool / tolerance / angle / descent | Baseline s | Adaptive + memo s |
| --- | ---: | ---: |
| 3.175 / .01 / 3 / 8 | 0.829017 | 0.021073 |
| 3.175 / .01 / 3 / 10 | 0.941257 | 0.021907 |
| 6 / .01 / 3 / 10 | 1.418146 | 0.025333 |
| 6 / .02 / 3 / 24 | 1.559428 | 0.031891 |

The unchanged three-pass, 8 mm fixture's uninstrumented test time was **2.356 s before, 0.044 s after**. These are individual observations, not five-run medians or universal thresholds. Diagnostic counting on the identical three-pass, 8 mm fixture now records **1,207 whole-target queries, 2,411 cache hits, 3,618 charged enclosure attempts and 156 complete sweep calls**. The baseline made 202,126 whole-target queries; query work falls by 99.40% in this fixture. All original additional target checks still run. The cache avoids 2,411 otherwise identical drop queries after adaptive subdivision, without avoiding their budget charges.

A separate uninstrumented same-certificate comparison performs 10,000 identical clear sweeps through one memo session versus 10,000 fresh standalone sessions (query each time). Both certify and spend exactly 10,000 budget units. Observed times were **126.935 ms cached / 496.275 ms fresh queries**. This is a synthetic single-run comparison under concurrent validation load, not a general speed-up guarantee. [Raw cost and count log](handoffs/rampbudget-evidence/cost-and-counts.log).

## Validation and coverage

Completed checks:

- `cargo fmt --all` and format/whitespace checks pass.
- `cargo clippy --workspace --all-targets` passes; final run 21.42 s.
- `cargo nextest run --locked -p openchuck-path -j 4`: **371/371 pass**, 807.101 s, one existing nightly-only exclusion. The final added cache-cost test passes separately at 0.652 s; the updated same-fixture sweep-count test passes at 0.527 s. Together the broad/focused runs cover all 372 current default path tests. Longest individual uninstrumented test: 432.700 s, below ten minutes. All 13 over a minute are registered; the one newly observed existing blisk case is added to both required profiles.
- Existing application posting checks: **3/3 pass**, 12.753 s, including both shipped posts and 1/2/N G-code identity.
- `cargo xtask deps`, `licences`, `records`, `trace` pass; **100/100 P1 requirements** have tests. The metadata checks required a network-enabled retry to download a missing locked target dependency; the successful results above are the final runs.
- `cargo llvm-cov nextest --profile coverage -p openchuck-path --locked --no-report -j 4`: **359/359 pass**, 497.203 s, 14 skipped by the filter at run start. The existing blisk test was newly classified during the run, so this coverage run additionally includes it (100.692 s instrumented). Reports use the requested `/(tests|examples|benches)/` exclusion only.

Own-test `openchuck-path` line coverage is **13,750/14,925 (92.13%)**, below the 95% development target. `entry_safety.rs` is **185/197 (93.91%)**; shared `assemble/entry.rs` is **512/602 (85.05%)**. The changed files and their uncovered lines were read. All newly added executable cache/assembly lines are reached except the new `entry_safety.rs` defensive returns at **line 255** (finite inputs/frame arithmetic overflow) and **lines 296–298** (floating-point subdivision cannot progress). Ordinary configured machine ranges and ramp progress/output-precision rules avoid those extreme standalone-host inputs; no dedicated extreme-float fixture was added for those guards. Existing unsupported-input/controller-representation validation and unrelated lead/helix/refusal paths remain uncovered in the own-test report. No production code was excluded to raise coverage.

After inspection, **`cargo llvm-cov clean --workspace` completed successfully**. Raw uninstrumented tests, Clippy/xtask results, coverage summary and missing-line report are retained in the evidence directory. CI's cross-platform and library-family semver checks remain pending Merge Manager's released main integration; no local semver-check pass is claimed.

## Remaining limits

This does not fix the historical blisk pass-1 “no usable straight entry direction” refusal. Unsupported target/body certificates and unprovable or budget-exhausting entries still refuse. The conservative protected-solid AABB can refuse geometrically clear cases. Cache benefit depends on exact repeated inputs, and its bounded storage adds memory/lookup overhead. Adaptive coarse failures can add work near obstacles. No lower stock-clear start, angle reduction, budget increase, machining trial or new hardware verification is claimed. NFR-12 wording is unchanged. The owner tests the fix and closes #343; the lane does not close it or mark its PR ready.
