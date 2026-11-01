# Archive: decision 0248 (RAMPBUDGET), raw evidence

Cited by [decision 0248](../../decisions/0248-ramp-clearance-budget.md) ("Verification results"). Moved here from `docs/handoffs/rampbudget-evidence/` on 1 Nov 2026 (DOCS-TIDY). Lane branch `codex/lane-rampbudget` (PR #360, issue #343). Uninstrumented timings are in the `*tests*`, `nextest-*`, `focused-green`, `immutable-scope-green` and `cost-and-counts` logs; the `*coverage*` logs are instrumented and are never timing evidence.

| Files | What | Commit |
| --- | --- | --- |
| `nextest-baseline.log`, `nextest-red.log`, `baseline-subintervals.log`, `unchanged-query-counts.log` | Phase 1: baseline matrix timings, the failing deep-entry regression, per-call budget diagnostics (instrumentation times, not benchmarks), exact-repeat counts | main `ba10400fff3bb7ad32526a7e540919c810e85434` |
| `phase2-*-red.log`, `focused-green.log`, `immutable-scope-green.log`, `cost-and-counts.log`, `path-tests.log`, `application-posting.log`, `coverage-*.log`, `clippy.log`, `deps.log`, `licences.log`, `trace.log` | Phase 2 implementation: tests failing first, then passing; cost and count comparison; initial coverage and gates | lane head before integration |
| `released-*.log` | First released integration (main `acff825847a5233a84e43f771a933602012338d0` into `a60f6f52ddc77374f80828157194b22b910ce489`) | PR #360 |
| `released2-*.log` | Second released integration (main `ea3ecf6849a65a839878bb664356867692ee51ae` into `997507f5379b69d70bd898e41bc112e037694ef1`) | PR #360 |

Deleted as noise: `released-trace-initial-disk-failure.log` (the trace run that hit ENOSPC; the passing rerun is `released-trace.log`) and `released-coverage-summary.log` / `released-coverage-missing.log` (duplicates of the `released2-` reports kept).
