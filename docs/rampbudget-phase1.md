# RAMPBUDGET Phase 1 — issue #343

Historical planning checkpoint. The owner subsequently accepted original 0247 directly; its unchanged clauses are administratively numbered 0248 / OI-998 after an AROT allocation collision. See [implementation and current verification](rampbudget-verification.md). Statements of pending approval below describe the Phase-1 boundary.

26 October 2026. Current main checked by GitHub commit lookup and full remote fetch: `ba10400fff3bb7ad32526a7e540919c810e85434`, library 0.23.0. Branch `codex/lane-rampbudget`. ROUGHPATH #340 is merged. CLAUDE.md/AGENTS.md, decisions 0241/0242 (accepted), the work breakdown, open-items register and verification plan were read. No production behaviour change, schema or version bump is made. [Proposed complete decision 0247](decisions/0248-ramp-clearance-budget.md) awaits direct owner acceptance; OI-994 tracks it.

## Requirements and dependencies

OP-11 covers ramp parameters; OP-12 whole-tool gouge protection; OP-13 feeds; OP-14 stock-clear links; SET-09 and OP-21 held regions/tabs; GC-07/NFR-06 byte identity and thread independence; NFR-04 bounded cancellable generation; NFR-07 refusals; NFR-12 remains unchanged. Test names cite the applicable IDs. Verification-plan requirement matrix rows OP-11/12/13/14 and GC-07/NFR-06, acceptance thresholds (responsiveness, determinism, performance), core-count procedure, and its final “ROUGHPATH boundary and reversing-entry checks” govern Phase 2.

0242 clause 6 fixes 10,000 descending legs and 100,000 subintervals. Raising either needs approval. 0241's inter-operation clearance, tool-change and continuous rotary rules remain in force. OI-41/OI-60 retain physical machine/tool-change limits; OI-553 and OI-790 retain the historical ramp limitations; OI-970 confirms Warn-only sampling is no certificate. None is silently closed. The historical blisk's current pass-1 missing-direction refusal is separate, not reproduced or fixed here. STEPALL's later exact-query/tessellation work and library version order remain Merge Manager's responsibility.

## Reproduction and timings

Synthetic analytic cylinder tessellated with `mesh(3,36)`, radius 15 mm, length 40 mm; ball tools 3.175/6 mm, supported body from the fixture profile; A0, first cut at X20 and last at X30. Approach explicitly 2 mm. Stock radius = 15 + descent - 2 mm. Float32 rotary bound; no protected solids in availability fixtures. These characterize the current certificate, not tool suitability or hardware performance.

Uninstrumented command:

```
cargo nextest run --locked -p openchuck-path --test entry -E 'test(~rampbudget)' --no-capture
```

Rust 1.97.0, nextest 0.9.150, debug test profile, Linux, four-core CPU quota. One timed run, no claim of a performance median. Production diagnostics removed before timing. Three tests passed in 25.675 s: matrix 21.005 s, unchanged-pass fixture 2.356 s, exact boundary 2.313 s. No new test exceeds a minute. Raw [nextest log](handoffs/rampbudget-evidence/nextest-baseline.log).

| Diameter mm | Tolerance mm | Angle degrees | Descent mm | Current result | Seconds |
| --- | --- | --- | --- | --- | --- |
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

The isolated sweep test checks below, at and beyond 100,000 subintervals (0.99999/1/1.00001 mm, tolerance 0.00004 mm) with clear geometry. The matrix brackets availability but does not establish a universal descent threshold. A separate intended-availability test is explicitly ignored pending the choice, then explicitly run to observe RED:

```
cargo nextest run --locked -p openchuck-path --test entry -E 'test(=req_op_11_req_op_12_req_nfr_04_deep_rampbudget_entry_is_available)' --run-ignored only --no-capture
```

It failed in 1.251 s with the existing budget refusal at 12 mm descent, 3.175 mm tool, 0.01 mm tolerance, 3 degrees. [Red log](handoffs/rampbudget-evidence/nextest-red.log). Phase 2 must enable it and make it pass after the choice, without preserving baseline-refusal assertions as product requirements.

## Where the checks go

Temporary diagnostic logging recorded each accepted `verify` call's steps, distance, inflation and endpoints, plus budget remaining after additional target checks. Diagnostics were removed; [raw sweep log](handoffs/rampbudget-evidence/baseline-subintervals.log) contains instrumentation times that are **not benchmark timings**. Translation subdivision is the mechanism used for corridor, approach, legs and connection, not an extra additive category.

| Case (tool/tolerance/angle/descent) | Corridor | Approach | Descending legs | Final connection | Whole-target drop queries |
| --- | ---: | ---: | ---: | ---: | ---: |
| 3.175/.01/3/8 | 1,271 | 5,200 | 62,314 | 1,257 | 70,042 |
| 3.175/.01/3/10 | 1,271 | 5,200 | 77,583 | 1,262 | 85,316 |
| 3.175/.01/3/12 (refused) | 1,271 | 5,200 | 92,851 completed | not reached | 99,322 |
| 6/.01/3/8 | 2,400 | 5,200 | 62,462 | 42 | 70,104 |
| 6/.01/3/12 (refused) | 2,400 | 5,200 | 91,352 completed | not reached | 98,952 |
| 6/.02/3/24 | 1,200 | 2,600 | 92,549 | 1,195 | 97,544 |

The 3.175 mm / 8 mm fixture additionally spends 501 budget units on legacy target samples (70,543 total). Thus “100,000 checks” includes multiple kinds of work; the last unfinished call can refuse before spending all remaining units. Current code precharges a complete sweep's steps.

For reuse counting, three identical cut passes preserve target, tool and parameters. A diagnostic BTreeSet recorded actual query origin XYZ bits, axis XYZ bits and inflation bits, with fixed base body and target for the process. It does not treat nearby coordinates or changed inflation as equivalent. [Raw reuse log](handoffs/rampbudget-evidence/unchanged-query-counts.log).

| Pass | Whole-target queries | Exact repeats from earlier queries | Additional target samples |
| --- | ---: | ---: | ---: |
| 1 | 70,042 | 0 | 501 |
| 2 | 66,042 | 64,842 | 501 |
| 3 | 66,042 | 66,042 | 501 |

Total 202,126 queries, 71,242 distinct, 130,884 reusable (64.75%). A 100,000-entry exact cache could avoid those calls in this fixture. No actual cached runtime or memory measurement is claimed; lookup overhead and real-job hit rates remain Phase 2 measurements. Later approaches start from a different position, accounting for the lower query count. Identical-pass geometry is a controlled reuse case, not evidence that every roughing pass repeats these inputs. In particular the written margin changes with height, so memoisation alone does not remove all repeated lateral stations or fix a single deep entry's availability.

## Options

| Option | What changes and what proves safety | Work/cost | Risk / owner choice |
| --- | --- | --- | --- |
| Adaptive conservative subdivision | Try larger full-envelope Minkowski enclosures, refine an inconclusive result in deterministic order. Every accepted interval still encloses the complete motion. Keep additional target/protected/travel checks. | Potentially far fewer queries high above target; bounded by the unchanged 100,000 attempt budget. Savings not yet measured. | Coarse failures can add work near obstacles; no availability guarantee. Recommend this for deep-entry availability. The complete bound is in 0247; no tolerance-minus-leave heuristic. |
| Exact memoisation across unchanged inputs | Reuse only exactly identical whole-target drop results in a generation-scoped bounded cache. Full protection/travel/cancellation checks still run. | 64.75% avoidable whole-target calls in the repeated fixture; consumes memory and lookup time. Hits still count against proof budget. | Stale/incomplete keys are unsafe; changing inflation destroys reuse. Recommend alongside adaptive proof for cost, not as the availability fix. |
| Lower stock-clear start | Prove the entire incoming rapid's full-tool sweep against remaining stock, not just its endpoint. Requires trustworthy remaining-stock enclosure and provenance for every pass. | Could shorten entry and proof; new stock-proof cost, interfaces and invalidation. | No sound local proof established here. A local stop_height is insufficient. Defer; explicit owner approval and more design required. |
| Need-scaled bounded budget | Precompute conservative required intervals from all planned sweeps and additional checks, then permit work up to an explicit hard cap with checked arithmetic/cancellation. | Preserves existing proof at cost proportional to descent; larger worst-case runtime and memory. | Changes 0242's accepted limit. Owner must choose hard cap/resource policy. Not recommended as first correction because it preserves the expensive repetition. |

Recommendation: proposed 0247, adaptive proof plus exact memoisation, keeping both limits and the global stock-clear start. It changes proof scheduling and cost, not machining motion. The existing set-enclosure formula remains valid with leave > tolerance; derive padding geometrically rather than subtracting leave from tolerance. Full proposed text is in 0247; acceptance must be direct in this session before behaviour changes. No approval is inferred from a relay or elapsed time.

## Phase boundary

Production files match main. Only characterization/regression tests and proposal/evidence documents are changed. Phase 2 safety, determinism, cancellation and option-off tests, broad validation, coverage, changelog/version decisions and the final single draft-PR push are outstanding. No claim of those checks passing is made. No PR is opened at this planning boundary; the issue comment carries the report. NFR-12 wording and requirements/architecture are untouched.
