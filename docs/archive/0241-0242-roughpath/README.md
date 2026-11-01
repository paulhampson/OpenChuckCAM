# Archive: decisions 0241 and 0242 (ROUGHPATH), raw evidence

Cited by [0241](../../decisions/0241-continuous-rotary-operation-boundaries.md) and [0242](../../decisions/0242-optional-back-and-forth-ramp.md) ("Verification results"). Moved here from `docs/handoffs/roughpath-evidence/` on 1 Nov 2026 (DOCS-TIDY). Produced on branch `codex/lane-roughpath-unwind-and-ramp` (PR #340): the top-level logs at the pre-integration head (base `1a90435654c734c05e4fa3f110c4a7f3f8e5859f`), `release-integration/` at the released integration of main `dc610e26dd260d69b5fc859270e7bf5fc913a3bb` (lane head `32d081254489a02709671c2f63c3f2b6f0d07158` before merge).

- `*-before.log` / `*-after.log`: tests watched failing, then passing, for one regression each (cancellation, clearance precision, counter context, half-unit progress, inactive option, slope precision, sweep, ramp integration).
- `fixed-plunge.log`, `reversing-blisk-final.log`: the recovered owner blisk project after the #292 fix and with the 3 degree reversing ramp.
- `nextest-short.log`, `final-*.log`, `semver-*.log`, `xtask-small.log`: test, doctest, clippy, semver and xtask runs.
- `openchuck-*-coverage-*.txt`, `coverage-post.log`: own-crate coverage summaries and changed-line lists (lower bounds; some llvm-cov reports carry function-mismatch warnings, kept with the reports).
- `numbering-reconciliation.txt`: the 0238/0239 to 0241/0242 renumbering audit.
- `release-integration/`: logs of the released integration (adapter, kernel, golden, replay, semver, workspace summary, `allocation-audit.md` snapshot of branch state on 25 Oct 2026).

Deleted as duplicate: `release-integration/allocation-audit.json` (170 KB, same data as the `.md`).
