# RAMPBUDGET released main integration — PR #360

27 October 2026. Merge Manager [marked #360 ready and released it next](https://github.com/paulhampson/OpenChuckCAM/pull/360#issuecomment-6444112736) under decision 0055. This integrates main **`acff825847a5233a84e43f771a933602012338d0` once** into lane head `a60f6f52ddc77374f80828157194b22b910ce489`.

## Resolution and API audit

The only textual conflict was `crates/openchuck-path/CHANGELOG.md`; both the RAMPBUDGET entry and main's 0.24.0 SHORTMOVE entry are retained. The union merge of the decision index also retained the stale proposed 0244 row alongside main's accepted row. The stale row is removed and rows are ordered 0244/0245/0246/0248/0249/0250. Main's statuses and complete records are preserved; no new acceptance is inferred.

A full remote-ref and open-PR audit confirms this lane still owns **0248 / OI-998**. AROT #364 owns 0247 / OI-994–997. Main's **OI-1000/OI-1002 remain open; OI-999/OI-1001 remain closed**, with their rows preserved. No new record, OI or schema number is claimed.

Public `EntrySafety`, `RotaryPrecision`, and `EntrySafety::verify` declarations match main. The session is crate-private and the assembly type is private. This merge makes **no RAMPBUDGET production or test-code edit** relative to `a60f6f5`; its long-test registrations are retained. Inherit main's **library 0.24.0 and schemas 20/6** without selecting a new version or changing the project format. Requirements and architecture are imported exactly from main, without lane edits.

CodeRabbit's remaining suggestion to grow profiles only on cache misses is a trivial performance follow-up, not a safety finding or release blocker. It is left unchanged in this integration; the production tree remains the previously validated RAMPBUDGET implementation plus main.

## Validation

Completed on the merged tree:

- `cargo fmt --all` and workspace `cargo clippy --workspace --all-targets` pass (Clippy 18.51 s).
- `cargo nextest run --locked -p openchuck-path -j 4`: **372/372 passed**, 707.788 s, one existing nightly-only exclusion. Longest individual test **332.800 s**. All 11 tests over a minute are registered; none exceeded ten minutes. The newly observed existing region/avoid test took **62.367 s** and is added to both `coverage` and `long` profiles; earlier registrations are retained.
- Application posting integration: **3/3 passed**, 2.237 s, including both-post absent/false bytes, reversing-ramp 1/2/N posting identity and indexed roughing through shared entry.
- `cargo xtask deps`, `licences`, `records`, `trace`, **`conflicts`** pass. Trace reports **100/100 P1 requirements** with tests. Its first workspace-wide test-inventory build hit ENOSPC; stale executable/incremental build caches were removed, then the same trace command passed. No test/source failure remains.
- Own-crate `cargo llvm-cov nextest --profile coverage -p openchuck-path --locked --no-report -j 4`: **358/358 passed**, 464.325 s, 15 skipped at run start. This run includes the region/avoid test newly registered during validation; future coverage runs exclude it. The requested summary/missing-line reports were read, then **`cargo llvm-cov clean --workspace` passed**.

Own-test path line coverage remains **13,750/14,925 (92.13%)**, below the report-only 95% target. `entry_safety.rs` remains **185/197 (93.91%)** and assembly entry **512/602 (85.05%)**. Newly added uncovered code remains the same defensive finite-enclosure failure at line 255 and non-progress refusal at lines 296–298; ordinary machine ranges and ramp output precision do not exercise those extreme standalone-host cases. All cache/assembly additions otherwise execute. Existing unrelated validation/lead/helix gaps remain; no production code is excluded. These lines were re-read after integration. Cross-platform CI and semver results are left to the released head's CI; no local semver pass is claimed.

Rebuildable incremental caches and stale executable artifacts were removed to recover disk space while building the all-workspace trace inventory. Validation logs, including the initial resource failure and successful retry, are retained in `docs/handoffs/rampbudget-evidence/released-*.log`.

## Handoff

The merge commit is authored and committed as Paul Hampson with the linked noreply address and a Codex co-author line. One released push and its exact SHA are reported on #360. Merge Manager owns CI review, release acceptance and merging; this lane does not merge the PR or set auto-merge. Issue #343 / OI-998 remain open for owner fix feedback. Original [implementation limits and coverage gaps](rampbudget-verification.md) remain explicit; no hardware/machining validation or NFR-12 wording change is claimed.
