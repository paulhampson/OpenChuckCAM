# ROUGHPATH released main integration

Merge Manager released #340 in [comment6400352887](https://github.com/paulhampson/OpenChuckCAM/pull/340#issuecomment-6400352887). GitHub reported `draft: false` at lane head `32d081254489a02709671c2f63c3f2b6f0d07158` before main `dc610e26dd260d69b5fc859270e7bf5fc913a3bb` was merged locally. One integration push follows validation under 0055. The lane does not merge the PR or enable auto-merge.

This report supplements the [implementation/interface report](roughpath-verification.md); it does not relabel its earlier tests/coverage as new integrated-head measurements. The owner reconfirmed unchanged acceptance after full presentation of 0241 and 0242; the records preserve original acceptance quotes and explicit former-number provenance. No new motion rule, OI or schema is introduced.

## Integration and preserved scope

- Combined nextest comments and coverage/long selections from both sides, deduplicating only identical complete filter lines. Default selection and the owner-approved nightly-only exception remain intact.
- Combined app/path changelogs, preserved every lane row in the work breakdown, and sorted the semantic union of decision-index rows. Existing OI states remain main's states.
- Preserved both complete, independent posting test bodies rather than splicing partial function/argument conflict chunks. Two incoming OUTPUT-1 UI test helpers now pass the existing regeneration-based planning API; their assertions are unchanged. The normal UI caller and application/export interfaces merged automatically.
- Product algorithms from #328/#329 remain unchanged by manual conflict resolution. In particular, `output_write.rs` remains byte-identical to main: its no-clobber/overwrite/result behavior is preserved. The unresolved final-file publication concern #332 and deferred Messages punctuation #331 are not implemented here. No production tempfile dependency, atomic-batch promise, four-step workflow #322 or playback synchronization #341 prerequisite is imported.
- Three CLI blisk tests failed first against four stale terminal-output goldens. The update removes only final `A-5.714`, two final `A0.000` words, and the axial Z1's unnecessary final `G92.4 A0 S0`, each after safe retract and immediately before `M5`. Every cutting/feed word is byte-identical. The three normal non-bless 1/2/N/both-post reruns pass (141.748 seconds). This is accepted 0241 output behavior, not a new policy or comparison tolerance.

## Allocation audit

A fresh enumeration reads all 28 actual remote heads (27 with decision indexes), active owner PRs, actual record filenames/indexes, OI registers, work-breakdown claims, schema constants and persisted management reservations. Relevant map: held STEPALL-M 0234/OI-984; merged 0235/0236/0237/0238/0240; FINSTEP reserved 0239; ROUGHPATH 0241/0242, no OI/schema; CHECKPOINT 0243/OI-989 as reconciled in the management reservation. Project18/machine4 are unchanged. OI-985/986 CLOSED, OI-987 OPEN, OI-988 CLOSED and existing OI-580 OPEN are retained. A local records pass is not treated as cross-branch uniqueness proof.

The all-record scan finds an old 0016 alias on unmerged `docs/decision-0016-sp-9-exact-surfaces`, distinct from main's accepted `0016-sp-7-2d-offsets.md`. It is reported to Merge Manager, not rewritten or claimed by this lane. No conflict involves current ROUGHPATH allocations. The final per-ref audit is refreshed before pushing.

## Review assessment

CodeRabbit reviewed the pre-integration head `32d0812`: one Major availability/cost finding and three Minor findings, no Critical finding. The accepted fixed 100,000-check budget can refuse deep ramps; global stock-radius approaches are intentionally conservative. Lowering the rapid start requires stock-clearance proof, and the suggested `tolerance - leave - written_margin` formula is unsuitable as written when stock-to-leave exceeds tolerance. Motion/proof/limits are unchanged pending Merge Manager's disposition; no suggested reduction of ramp angle is offered.

The two refusal-context observations remain for the coordinator's bundled disposition, preserving the owner-approved minimum/refusal wording and zero-verified-length semantics. The straightforward RAMP_BACK_AND_FORTH rustdoc correction rides this already-required push; it changes no executable logic. Public/private docstring-coverage advice does not initiate a separate optional push. Exact-head CI and CodeRabbit are awaited after the integration push.

## Validation on the integrated working tree

All timings below are uninstrumented. The broad nextest run used four workers while the native OCCT build was also active; its timings describe that environment, not a standalone performance benchmark.

| Check | Result |
| --- | --- |
| Default workspace suite | 1,911 tests run in 1,978.109 seconds: 1,906 passed initially, five failed, two intentionally skipped. Three CLI blisk golden failures were corrected as described above and pass their normal rerun; two unchanged geometry timing-budget cases pass their quiet rerun (2/2, 2.077 seconds). Thus all 1,911 executed cases passed across the initial run and targeted reruns; there was no single all-green 1,911-case run. |
| STEP/kernel | Native OCCT 7.8.1 SDK, wrapper and worker built locally. Importer and all six application STEP integration binaries pass: 76/76 in 128.947 seconds, including roughing/rest/pencil (121.966 seconds). Native-feature clippy passes. The 150-mutant fixed-seed importer exercise passes: 62 read, 42 clean errors, 46 isolated worker crashes; no parent panic or inconsistent model. Worker crashes are expected adverse-input outcomes, not hidden successes at importing damaged geometry. |
| Adapter-required UI | 3/3 rendering, palette/picking and surface-tooltip cases pass in 114.324 seconds with `OPENCHUCK_REQUIRE_ADAPTER=1`, GL, llvmpipe/Mesa software adapter. No adapter skip is counted as a pass. |
| Determinism/output | All three corrected CLI blisk cases pass normal 1/2/available-N and both-post checks (141.748 seconds). The broad suite also exercises the option-off byte-identity and rotary assembly/simulation/post agreement tests. |
| API/family | Workspace family 0.21.0; all four standalone library semver checks pass against current main `dc610e2` with the declared pre-release minor bump. |
| Dependencies/licences | `cargo deny --all-features check`, `cargo audit` and `cargo about generate --locked about.hbs` pass. Generated notice HTML stays outside the repository. Duplicate-version warnings are retained, not suppressed. |
| Doctests | Workspace doctests pass. |
| Kernel-enabled release golden component | 10/10 golden-file comparisons pass, 33.171 seconds, including STEP drilling; run with the release component's workspace/exclusions/features and nextest's 600-second individual-test guard. Separate `cargo xtask golden` independent geometry/feed/post audit also passes. |
| Final repository checks | Workspace all-target clippy with warnings denied (11.10 seconds), native-feature clippy (15.06 seconds), `cargo fmt --all -- --check`, `cargo xtask records`, `conflicts`, `trace` and `all` pass; traceability is 99/99 P1 requirements. `git diff --check` passes. |
| LinuxCNC | All 11 programs pass at the original 0.1 mm/degree tolerance in the official 2.9.10 Docker simulator. Nine passed in the broad replay (the small-part case used the built-in retry); two axial cases initially missed sampled safe/index positions under native-build CPU load and then both pass the quiet rerun. `cone_axial_lead`: all 1,344 moves, 285.4 seconds of motion; `cone_axial`: all 608 moves, 270.3 seconds. Both original failures and successful reruns are retained. No tolerance, G61 mode, proof or sampled-path acceptance was weakened. |

The longest individual default-suite test was the existing simulation brute-force comparison, 521.292 seconds. No individual uninstrumented test exceeded ten minutes. Both nextest coverage/long profiles retain the union of main and lane selections; five additional cases observed above a minute are listed in both. The owner-approved nightly-only simultaneous oracle and ignored release-only simulation timing benchmark remain skipped in the default run; they are not represented as newly executed.

## Coverage and practical limits

No executable product logic was changed by manual conflict resolution. The executable resolutions are test-only: retaining both independent posting tests and adapting two incoming UI test-helper calls to the regeneration-based API. Their binaries ran in the broad suite. The RAMP_BACK_AND_FORTH review correction is a documentation comment. The original implementation's crate-own instrumented coverage and changed-line gaps remain recorded in the [implementation report](roughpath-verification.md) and its evidence; they are historical measurements, not a fresh aggregate on this integrated tree. Those lower-bound aggregates remain below the report-only 95% target. No new product branch is hidden or excluded to improve coverage.

The composite tagged `release-gate` command is not a release invocation here: its dependency, all-feature licence, kernel-enabled golden and LinuxCNC components are reported individually, with replay explicitly bounded to 600 seconds per attempt. No release tag, hardware test or material-removal trial is claimed. The conservative reversing-ramp limitations and the historical project's pass-1 refusal remain as documented; full-width slot engagement and one-diameter minimum do not constitute machining validation.

Tooling, raw logs, generated notice HTML, native-build output and large LinuxCNC JSON reports stay in `/workspace/roughpath-evidence/release-integration/`. Selected concise results and the allocation audit are retained in [release evidence](handoffs/roughpath-evidence/release-integration/). Exact-head CI, CodeRabbit and Merge Manager disposition remain required after the one authorized push.
