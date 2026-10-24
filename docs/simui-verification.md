# SIMUI verification (decision 0238)

Completed local SIMUI implementation and verification on `codex/lane-simui-display-and-navigation`, starting at `1a90435`, prepared for one combined draft PR and Merge Manager handoff. The owner accepted decision 0238 on 24 Oct 2026 after full-text presentation as SIMUI 0237; the Merge Manager’s renumbering preserves that acceptance; OI-987 remains open for owner/reference hardware.

## Requirement-first evidence

Observed failing regressions before fixes: Show state/names, three stale messages, inaccessible stale colour toggle, operation-row navigation, missing Playback progress, separated checkboxes, absent opacity controls (2.550 s), absent personal opacity setting, absent projected signed/bounded values and cancellation, translucent mapped-palette contrast (2.947:1) and colour-vision difference (15.992), a pinned stock value showing through a nearer model (7.521 s), automatic requeue after a failed cut-surface job (6.342 s), and row navigation leaving the inspector automatically collapsed in a small window (13.203 s).

The spatial projection also matches an independent exhaustive nearest-sample search. Signed sample values/bounds are copied; the simulation report is unchanged. Pins use actual stock-hit positions; an extended turn-away/reappear/model-occlusion check passed in 8.974 s. An own-app stored-run query check passed after stock/cell edits in 10.367 s; its terminal-cursor extension passed in 8.535 s and compares the raw and mapped final stock, valid triangle IDs and unchanged report.

## Completed focused checks

- Final uninstrumented UI suite: 441/441 passed in 396.350 s while app coverage was active (the preceding 440-case run took 211.457 s without that coverage load). A final five-test colour/filter/opacity/failure subset passed in 43.300 s after caching unchanged mapped colour groups. After the small-window inspector correction, navigation/control/output-preservation checks passed 4/4 in 59.098 s. The direct Turn/Skip/Tool click and independent-project-undo extension passed in 5.302 s. The final app suite: 482/482 passed in 600.848 s. No individual test exceeded ten minutes. Earlier retired-label smoke-fixture failures were corrected.
- Whole-dock current/stale/loading/ready matrix at fixed physical sizes 1366×768, 1440×900 and 1920×1080, 100%/120%, including shorter docks: passed in 107.678 s alongside UI coverage. Earlier logical-size runs took 62.592 s in the first full run and 116.400 s alongside app coverage. It is selected by the long profile and excluded from coverage. At physical sizes whose 120% scale falls below the logical minimum, the check verifies automatic dock collapse and then explicitly reopens it through the existing command before checking the contents; 1920×1080 at 120% stays above that minimum. An existing app workholding roughing regression took 92.703 s and received the same classification; the 81.075 s reference-jobs test was already classified. The remaining-workspace run also measured existing CLI `blisk_helical_2` and `cone_held_tabs` end-to-end cases at 78.996 s and 131.726 s; both were added to the coverage exclusion and nightly long selection without removing default execution.
- Show controls: all seven main combinations, every menu-only flag combination, real clicks in both controls, last-choice protection and per-operation eye masking. Real row clicks/Enter, enable/eye/duplicate, keyboard reorder and actual drag reorder are covered. Navigation explicitly reveals the inspector even under the small-window rule; no shared dock infrastructure is restructured.
- Real first-run/regeneration and failed-job progress, including correct toolpath-only wording; real pending-edit Keep/Discard/Cancel and stale return state.
- Opacity/current-stale colour toggles: 7–9 s after spatial pruning (initially about 22 s).
- Output preservation compares project, neutral paths, actual posted bytes and simulation report after an independent regeneration; expanded to include opacity/mapped-colour toggles before final suite.

## Rendering on available hardware

llvmpipe (LLVM 19.1.7, 256 bits), CPU adapter, Mesa 25.0.7, GL backend, managed cloud with four-core quota and no physical GPU device. Adapter-required tests do not silently skip.

Uninstrumented development build (unoptimised Rust, reduced debug information). Actual stock geometry: 224,240 triangles. At 1920×1080, 10 warmed frames per mode (two warm-up frames omitted), wall time includes submission and GPU completion wait, excludes scene upload/readback:

| Stock display | p50 | p95 |
| --- | --- | --- |
| Plain translucent | 22.955 ms | 28.567 ms |
| Plain solid | 16.391 ms | 23.036 ms |
| Mapped translucent | 20.197 ms | 21.518 ms |
| Mapped solid | 19.752 ms | 20.440 ms |

Offscreen pixels check internal model/toolpath occlusion, actual transparency and mapped palette levels, normal and simulated colour-vision separation, contrast and ordered lightness on the tested front view. The 98% mapped translucent fill passes the same palette thresholds as solid on this view. Plain stock retains its existing 30% alpha. These checks do not establish all-view/monitor/reference-hardware readability; OI-987 remains open.

## Changed-crate coverage

App-only tests: **93.30%**, 15,133/16,219 source lines, below the 95% development target (report-only under 0102). The coverage profile selects 480 own-crate tests and excludes the two measured long cases. Data from 121 completed tests was preserved when applying the newly measured long-test exclusion, followed by the remaining 359 and the extended terminal-cursor regression. No other crate’s test suite contributes to this lower bound.

The only changed app source lines still uncovered are:

- `stock_surface.rs:228–229`: invalid-triangle error after immutable mesh validation already succeeded.
- `stockplay.rs:269–270`: missing-setup error after `mesh_at` already required the same setup successfully.

Both are defensive paths, rather than missing normal behaviour checks. The original meaningful terminal-cursor gap was covered by extending the stored-run query requirement check. No code was excluded to raise coverage.

UI-only tests: **86.68%**, 23,146/26,703 source lines, also below the 95% development target. The coverage profile passed 440/440 selected cases in 1,166.522 s (the whole-dock matrix is the one excluded long case), then the latest small-window navigation and direct grouped-control checks passed in 189.550 s and 35.519 s. These are instrumented durations, not performance measurements. No app test suite contributes to this separate UI dataset.

Changed UI source lines still uncovered:

- `playback_panel.rs:932`: cancellation of a legacy target-band job on entry to Simulate.
- `playback_panel.rs:965–967`: clearing mapped bands when returning to the legacy target-band display path outside Simulate. These compatibility transitions are not exercised by the own-UI suite; they are not claimed as verified by the ordinary navigation cases.
- `playback_panel.rs:1532–1534`: the relocated simplified-points diagnostic for a path exceeding the display budget. Path simplification itself has existing requirement checks; this particular displayed readout is not covered.
- `viewport.rs:67` and `viewport.rs:1581`: closing-brace regions marked uncovered by LCOV, retained in the calculation.

Both reports use the prescribed test/example/bench exclusion pattern. The changed uncovered lines were read; no source exclusions were added to raise the results. Required `cargo llvm-cov clean --workspace` and complete instrumented-target cleanup both succeeded after saving reports. Aggregate dependency totals are not used as either crate’s coverage.

## Local checks

- Formatting: `cargo fmt --all --check` passes after the final test extension.
- Workspace Clippy, all targets, warnings denied: final pass in 7.497 s after the grouped-control test extension.
- `cargo xtask all`: all seven checks pass, including 99/99 P1 traceability, final run 18.425 s; repeated with the new files staged before commit.
- `cargo deny check` (0.20.2): advisories, bans, licences and sources pass in 2.255 s, with existing duplicate-version/unused-exception warnings. No dependency or lockfile change.
- Workspace doc tests: 5/5 pass, 10.074 s including compilation.
- Remaining workspace packages: 912/912 pass in 1,240.542 s. Alongside app 482/482 and UI 441/441, all 1,835 tests in the default selection passed, in package batches; the final affected UI cases were repeated after the last changes. The longest individual case in this batch was the existing fine-simulation reference check, 235.326 s. No individual uninstrumented case exceeded ten minutes. The existing nightly-only reference checker and ignored hardware-budget case are not silently counted as passes.

Rust 1.97.0, cargo-nextest 0.9.149, cargo-llvm-cov 0.9.1. The app and UI coverage datasets were separate; all own-crate selected cases completed. Instrumentation timings are not used to classify long tests or claim rendering speed.

A full default-debuginfo workspace build exhausted the 32 GiB disk before tests started. Generated artifacts were cleaned, validation logs retained, and the rerun uses reduced debug information with incremental compilation disabled and two build jobs. This changes no product source or optimisation settings.
