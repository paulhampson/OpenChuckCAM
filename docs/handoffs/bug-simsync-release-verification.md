# BUG-SIMSYNC released-main verification

25 October 2026; PR #341, issue #333; SIM-02/03/04.

## Integration and allocation

The merge coordinator marked the PR ready before releasing the integration in
[comment 6401744818](https://github.com/paulhampson/OpenChuckCAM/pull/341#issuecomment-6401744818).
Integrated main `ab863f6531d1e8fab8c2562e88d92b9277937c3f` (including ROUGHPATH
#340) into PR head `e17e65e4012780f2aa31acb2c9b15c6573f99e9e` with a real two-parent
merge. No textual conflict occurred. The UI specification contains both the
synchronization correction and merged 0241/0242 behavior. Main's decision index,
open-items register, work breakdown, Cargo manifests and lockfile are unchanged.
The existing accepted decision 0040 amendment is retained.

[Allocation audit](bug-simsync-allocation-audit.json): all 27 remote branch trees,
all five open PR heads, and 58 distinct decision-index/open-items/work-breakdown/
merge-queue blobs were read. The persisted management/merge-queue reservations
agree with the actual records: held 0234/OI-984; merged 0235–0238 and 0240–0242;
FINSTEP reservation 0239; CHECKPOINT 0243/OI-989. No new decision, OI or project
schema number is claimed. Project schema 18 and machine schema 4 are unchanged.

The only executable correction during integration replaces two unchecked indexes
in stockplay with checked lookups, as required by the workspace clippy policy.
The deferred ROUGHPATH Minor findings are outside this change.

## Local validation

- Workspace clippy, all targets, locked, warnings denied: passed.
- Formatting and whitespace against released main: passed. Historical trailing
  blank lines inherited from main are retained rather than edited in this fix.
- Records, conflicts, licence headers, test-data and publish checks: passed.
- Traceability: passed, 99/99 P1 requirements have tests.
- Post-integration focused partial-move regression: passed, 20.057 seconds elapsed.
- Post-integration deliberately delayed stock-surface regression: passed,
  12.296 seconds elapsed.

The focused timings are uninstrumented debug-test timings under concurrent load,
not reference-machine frame-rate measurements. The full locked app/UI run passed all 972 tests (501 app and 471 UI),
with no failures or ignored tests. UI binary runtime was 150.79 seconds.
The app/UI doctest command also passed (the app has zero doctests and the UI
is a binary target). The full suite includes the existing software-adapter
cut-stock pixel, picking, opacity and full-HD rendering-cost checks.

Commands:
```sh
cargo test -p openchuck-app -p openchuck-ui --locked -- --test-threads=4
cargo test -p openchuck-app -p openchuck-ui --doc --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cargo xtask records
cargo xtask conflicts
cargo xtask trace --report /tmp/341-traceability.md
cargo xtask licences
cargo xtask testdata
cargo xtask publish
```

## Available-tool limits

`cargo-nextest`, `cargo-llvm-cov`, `cargo-audit`, `cargo-deny`, `cargo-about`
and `cargo-semver-checks` are not installed.
The attempted commands report unavailable subcommands. Tests therefore run with
`cargo test`. Coverage percentages and uncovered changed lines are unmeasured;
there is no new coverage waiver or assertion of reaching the 95% target.
`cargo xtask all` reaches the dependency-graph check but cannot download a
platform-only Tokio 1.53.3 crate because the configured shell proxy is unreachable.
The individually runnable checks above pass. Exact-head CI must supply the full
dependency/security checks; these local limitations must not be represented as
security passes. The PR changes no library-family API, dependency or version
relative to released main; the main 0.21.0 version and lockfile are preserved.

The same proxy prevents command-line Git transport. GitHub objects used for the
local merge were imported through the connected app and verified against their
Git hashes; published source hashes are checked against the local tested tree.
No network policy, proxy bypass or dependency change is used.

Reference-hardware visual smoothness/performance remains unconfirmed. Expensive
surface jobs can limit the viewport update frequency, but each displayed tool
and stock pair uses the same cursor. Issue #333 still requires human fix feedback
at merge, as directed by the merge coordinator. Do not merge or enable auto-merge
from this lane; await CI and CodeRabbit on the one integration push.
