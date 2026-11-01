# 0248: Reduce optional ramp clearance certification cost

- **Status:** accepted by the project owner directly in the RAMPBUDGET session on 26 October 2026, after the complete eight-clause text was presented: "Accept 0247". This authorises Phase 2 under the unchanged accepted clauses.
- **Date:** 26 October 2026.
- **Lane / issue:** RAMPBUDGET, Refs #343.
- **Requirements:** OP-11/12/13/14, SET-09, OP-21, GC-07, NFR-04/06/07/12; verification-plan requirement matrix, acceptance thresholds, and ROUGHPATH boundary and reversing-entry checks.
- **Allocation:** Administratively moved from 0247 / OI-994 to 0248 / OI-998 on 26 October 2026 after refreshed remote refs revealed AROT had published 0247 and OI-994–997. The eight clauses and direct acceptance under original 0247 are unchanged. No schema or library version claimed.

## Evidence and recommendation

[Phase-1 report](../rampbudget-phase1.md) records current-main reproduction, diagnostic counts, uninstrumented timings, options and limits. Recommend adaptive conservative subdivision plus bounded exact-input memoisation. Retain the global stock-clear start and both existing entry limits. Budget scaling and lowering the start are separate choices, not implicit fallbacks.

## Accepted decision

1. **Scope.** Optimise only the optional back-and-forth ramp's clearance certificate. Preserve 0242's motion, angle, fixed A, minimum one cutting diameter, feeds, approach, reversals, final connection and refusal rules. Preserve 0241's boundary policy. Absent/false option output remains byte-identical at the same version and rotary policy. Retain NFR-12 wording unchanged; no hardware or machining validation is claimed.

2. **Adaptive continuous proof.** For every straight fixed-A sweep, first attempt certification with an inflation enclosing that entire interval. If that certificate cannot prove clearance, subdivide deterministically left before right. Accept only if every resulting interval has a full-envelope certificate. An unsuccessful coarse certificate is inconclusive, not evidence of a collision. At the existing tolerance/4 resolution, retain conservative refusal if a certificate cannot be established. Reject non-finite arithmetic or lack of representable subdivision progress. Never substitute endpoint tests or Warn-only samples.

3. **Conservative bound.** Let P be the complete supported tool body, m the interval midpoint, L its translation length, a the tool axis, w the existing conservative written/controller displacement bound and l the nonnegative stock-to-leave. The intended sweep enlarged for leave and written error is contained in m + P + B_s, where s = l + w + L/2 and B_s is the closed radius-s ball. The existing grown profile represents (P + B_s) shifted by +s a to restore its tip origin. If its whole-target drop offset at m is q, retain the existing acceptance q + s <= tolerance (or no contact). This is a set-enclosure argument, not a tolerance-minus-leave step-size formula. It remains meaningful when l exceeds tolerance. Keep machine precision, convex-profile support and rounding checks; demonstrate the bound with thin-obstacle and leave-greater-than-tolerance tests before changing production code. If the implementation cannot establish it, return to the owner.

4. **Independent protections.** Continue checking the whole target, complete cutter/shank, held regions, tabs and machine constraints on the approach, every leg/reversal and final connection. Keep the existing additional target/refusal checks. Do not infer protected-solid clearance from a cached mesh answer. Retain the global stock.max_radius() + approach_distance floor; a local stop_height cannot justify lowering the incoming rapid.

5. **Bounded exact reuse.** Reuse only whole-target drop answers whose target, complete grown tool, axis and origin are unchanged. Scope the cache to one generation with immutable target/tool ownership; bound it to at most 100,000 entries with deterministic insertion-order eviction. Cache negative answers as well as contacts. Do not quantise coordinates or assume that axial translation, changed inflation, changed geometry or prior stock removal preserves an answer. Recheck cancellation, machine and protected constraints even on hits. Count a cached certificate attempt against the work budget just as a query would count; reuse changes cost, not clearance permission.

6. **Existing limits and cancellation.** Retain 10,000 descending legs and 100,000 clearance subinterval attempts shared by one entry, charging successful and unsuccessful adaptive attempts and the existing additional target checks. Every attempt consumes budget before querying or subdivision, bounding work even on hostile geometry. Preserve cancellation checks during search, proof, memoised lookup and emission; no partial operation may escape. An exhausted budget remains an actionable refusal. No angle reduction is a remedy for insufficient corridor length.

7. **Verification before delivery.** Observe requirement-based tests fail before the behaviour change. Certify the existing deep-entry regression; retain unsafe stock/target/protected/tab/machine refusals, every-leg/reversal and final-connection checks, cancellation mid-check, absent/false byte identity and 1/2/available-N determinism. Add cases with leave greater than tolerance, cache hits/misses/invalidation/eviction and unsuccessful coarse certificates that refine safely. Measure every new test uninstrumented, classify long tests and stop any over ten minutes for a decision. Measure real query counts and successful generation cost before/after; no speed-up claim follows merely from cache-hit counts.

8. **Delivery and remaining limits.** Update the registers, work breakdown, specifications and changelog as affected. A public library break follows 0048; wait for Merge Manager's merge order before selecting its version. Run required formatting, lint, changed-crate tests/coverage/cleanup and xtask checks before the single draft-PR push under 0055. Use Refs #343; keep it draft for Merge Manager and owner feedback. The historical blisk pass-1 missing-direction refusal remains separate. This optimisation does not guarantee every geometrically usable entry can be certified within the retained limits.

## Open item

OI-998 now tracks owner fix feedback on #343. Direct complete-record acceptance, implementation and measured correction are recorded in the [verification report](../rampbudget-verification.md). The owner's direct acceptance amends certification scheduling under 0242; motion rules and limits remain unchanged.

## Verification results (RAMPBUDGET, #343, PR #360)

Verification notes moved from `docs/rampbudget-phase1.md`, `docs/rampbudget-verification.md` and `docs/rampbudget-release-verification.md` on 1 Nov 2026 (DOCS-TIDY). Raw logs: [archive/0248-rampbudget/](../archive/0248-rampbudget/) (README beside it). Base: main `ba10400fff3bb7ad32526a7e540919c810e85434`, library 0.23.0. The owner accepted the record directly on 26 Oct 2026 as provisional 0247; it became 0248 / OI-998 after an AROT collision (administrative, no change of acceptance). Environment: managed Linux, Rust 1.97.0, nextest 0.9.150, four-core CPU quota, debug test profile; synthetic fixtures, not reference-hardware or machining evidence. Requirements: OP-11/12/13/14, SET-09, OP-21, GC-07, NFR-04/06/07.

**Fixture:** analytic cylinder tessellated with `mesh(3,36)`, radius 15 mm, length 40 mm; ball tools 3.175 and 6 mm; A0, first cut at X20, last at X30; approach 2 mm; stock radius 15 + descent - 2 mm; Float32 rotary bound; no protected solids. Command: `cargo nextest run --locked -p openchuck-path --test entry -E 'test(~rampbudget)' --no-capture` (three tests, 25.675 s, one run, no median claimed).

**Baseline before the change (20-case availability matrix, one run each):** the 12 mm, 3.175 mm tool, 0.01 mm, 3 degree case refused with the existing budget in 1.251 s (a time to refusal, not to success).

| Tool mm | Tolerance mm | Angle deg | Descent mm | Baseline result | Baseline s |
| ---: | ---: | ---: | ---: | --- | ---: |
| 3.175 | 0.01 | 3 | 8 | certified | 0.829017 |
| 3.175 | 0.01 | 3 | 10 | certified | 0.941257 |
| 3.175 | 0.01 | 3 | 12 | budget refusal | 1.096588 |
| 3.175 | 0.01 | 3 | 14 | budget refusal | 1.108496 |
| 3.175 | 0.01 | 3 | 24 | budget refusal | 1.145750 |
| 6 | 0.01 | 3 | 8 | certified | 1.567510 |
| 6 | 0.01 | 3 | 10 | certified | 1.418146 |
| 6 | 0.01 | 3 | 12 | budget refusal | 1.568274 |
| 6 | 0.01 | 3 | 14 | budget refusal | 1.616409 |
| 6 | 0.01 | 3 | 24 | budget refusal | 1.548961 |
| 6 | 0.02 | 3 | 8 | certified | 0.564956 |
| 6 | 0.02 | 3 | 10 | certified | 0.679391 |
| 6 | 0.02 | 3 | 12 | certified | 0.878749 |
| 6 | 0.02 | 3 | 14 | certified | 1.021846 |
| 6 | 0.02 | 3 | 24 | certified | 1.559428 |
| 3.175 | 0.01 | 6 | 8 | certified | 0.446766 |
| 3.175 | 0.01 | 6 | 10 | certified | 0.538998 |
| 3.175 | 0.01 | 6 | 12 | certified | 0.609160 |
| 3.175 | 0.01 | 6 | 14 | certified | 0.706090 |
| 3.175 | 0.01 | 6 | 24 | budget refusal | 1.150269 |

Where the 100,000 checks went (temporary diagnostics, removed; the sweep log holds instrumentation times that are not benchmark timings): for 3.175/.01/3/8 mm the corridor took 1,271, approach 5,200, descending legs 62,314, final connection 1,257 and 501 legacy target samples; the refused 12 mm case spent 92,851 on legs before the final connection. Three identical cut passes made 202,126 whole-target queries of which 71,242 were distinct (130,884, 64.75%, exact repeats; pass 1 70,042 queries with 0 repeats, pass 2 66,042 with 64,842, pass 3 66,042 with 66,042).

**After the change (adaptive conservative intervals plus exact-input memo):** all 20 cases certify; matrix 21.005 s before, **0.535 s** after; the 12 mm case passes in **0.028 s**. Selected same-input successes (baseline to new): 3.175/.01/3/8 0.829017 to 0.021073 s; 3.175/.01/3/10 0.941257 to 0.021907 s; 6/.01/3/10 1.418146 to 0.025333 s; 6/.02/3/24 1.559428 to 0.031891 s. Unchanged three-pass 8 mm fixture 2.356 s to **0.044 s**; whole-target queries 202,126 to **1,207** (-99.40%) with 2,411 cache hits, 3,618 charged enclosure attempts and 156 sweep calls (cache hits are still charged to the budget). Ten thousand identical clear sweeps: 126.935 ms through one memo session against 496.275 ms with fresh sessions, both spending exactly 10,000 units. These are single observations under concurrent load, not medians or speed-up guarantees.

**Tests (written first):** the deep-entry availability regression was run ignored to see it fail (1.251 s), then enabled and failed again before the implementation; three adaptive-envelope and budget tests failed at runtime on the old code; the cache-scope, eviction and cancellation tests failed to compile before the session API existed. New cases: leave greater than tolerance, refinement after an inconclusive bound, charged unsuccessful attempts and exact-budget exhaustion, query and cache-hit cancellation, immutable target and body replacement, exact origin/axis/inflation misses, negative and contact reuse, FIFO eviction, every corridor, approach, leg, reversal and final-connection call, global approach despite local stock removal, deep-entry 1/2/N identity, and application tests for option-off byte identity on both posts and indexed roughing through shared entry. Existing thin-obstacle, later-leg, shank, protected-geometry, rounding, counter and travel refusals still pass.

**Results.** Initial: `openchuck-path` 371/371 (807.101 s, one nightly-only exclusion), longest 432.700 s; the existing `fit_interior` blisk helical test (60.800 s) was registered in both `coverage` and `long`; application posting 3/3 (12.753 s); xtask deps, licences, records and trace pass (100/100 P1). First released integration (main `acff825847a5233a84e43f771a933602012338d0` into `a60f6f52ddc77374f80828157194b22b910ce489`; only `crates/openchuck-path/CHANGELOG.md` conflicted): path 372/372 (707.788 s), longest 332.800 s, the existing region/avoid test (62.367 s) registered in both profiles; posting 3/3; a first trace run hit ENOSPC and passed after stale build caches were removed. Second released integration (main `ea3ecf6849a65a839878bb664356867692ee51ae`, STEP CI fix of 0251, no source change): path 372/372 (472.667 s), longest 230.020 s; posting 3/3; xtask passes first time; coverage run 357/357 (298.047 s). Library 0.24.0 and schemas 20/6 are inherited; the public `EntrySafety` API is unchanged (the session type is crate-private).

**Own-crate coverage of `openchuck-path` (lower bound, report-only under 0102):** 13,750/14,925 lines (92.13%), `entry_safety.rs` 185/197 (93.91%), `assemble/entry.rs` 512/602 (85.05%). Uncovered new lines are defensive: `entry_safety.rs:255` (non-finite enclosure or frame arithmetic overflow) and `:296-298` (floating-point subdivision that cannot progress); no extreme-float fixture was added.

**Not verified and limits:** no local semver or STEP-kernel run and no cross-platform CI result at the time of the notes (CI owned by Merge Manager); the historical blisk pass-1 "no usable straight entry direction" refusal is not fixed; unsupported certificates and unprovable or budget-exhausting entries still refuse; the conservative protected-solid AABB can refuse geometrically clear cases; cache benefit needs exact repeated inputs and adds bounded memory and lookup cost; coarse failures can add work near obstacles; no lower stock-clear start, angle reduction, budget increase, machining trial or hardware verification (NFR-12 wording unchanged). CodeRabbit's suggestion to grow profiles only on cache misses was left as a trivial performance follow-up. Owner tests the fix and closes #343 (OI-998).
