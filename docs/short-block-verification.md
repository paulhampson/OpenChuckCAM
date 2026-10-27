# Short-block management: configuration and verification

GC-17; accepted decisions [0222](decisions/0222-short-moves-minimum-block-time.md) and [0245](decisions/0245-short-block-report-and-slowdown.md). Branch `codex/lane-shortmove-smooth-running`, released integration workspace **0.24.0**. Unless explicitly identified as integration evidence, the results below are historical pre-integration checks at **0.22.0**, not validation of the merged 0.24.0 tree.

## Configuring a machine

The machine inspector exposes Minimum block time (ms), Block window and mode. The same fields are in `machine.toml`:

```toml
min_block_ms = 7.0
block_window = 32
short_block_mode = "report"
```

A post definition can override any field independently under `[options]`:

```toml
[options]
min_block_ms = 8.0
short_block_mode = "slow_down"
# Omitted block_window retains the project's machine value.
```

Report changes no G-code. It estimates demand and slowdown cost. Slow down lowers affected feeds, retaining coordinates and programmed geometry; a larger target trades machining time and cutting feed for more command-processing margin. It does not change the geometric tolerance. Off or zero disables analysis. Merge remains deferred because it would consume geometric accuracy budget.

The new built-in Z1 starts at Report / 7 ms / 32, based on firmware 1.1.2 through the tested WiFi delivery path. Copies predating the timing fields stay Off until the user edits or explicitly updates them; already-saved Off/Report settings retain their mode. Unknown machines need their own measurements; do not copy the Z1 threshold as a universal default. The Files area and same-run setup sheet show the effective settings and whether each came from the machine or a post override, including overrides that disable analysis.

Calibrate with sustained sections long enough to exceed the controller's buffering capacity and with representative line lengths and rotary commands. Record machine/controller firmware and delivery path. Use the shortest setting that repeatedly passes the agreed timing and smoothness criteria, or a deliberately higher user-selected margin. Repeat when firmware, delivery or controller configuration changes. Feed override above the calibrated setting can invalidate the target.

## Evidence already available

The [owner's #284 results](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6399134358) identify OpenChuckCAM 0.20.0, Windows, Z1 firmware 1.1.2 and WiFi upload. ESP32 firmware version was not supplied. Both parts passed 20, 10 and 7 ms and stuttered at 5, 3, 2 and 1 ms. At 7 ms the sections took 6.1 and 5.9 seconds against 6.0 predicted. This supports the shortest tested smooth value of 7 ms, not an exact processing boundary. It validates the need for mitigation, not this new mitigation implementation.

## Fresh 7 ms / 32-block corpus analysis

Run the measurement example from the repository root:

```sh
cargo run --locked --offline -p openchuck-app --example short_block_corpus -- \
  testdata/golden/reference/*_makera-z1.nc testdata/golden/blisk/*_makera-z1.nc
```

It reads existing G-code coordinates and feeds, reconstructs fed runs split at rapid/event boundaries and applies the production slowdown calculation. This analyses command demand; it does not replace static, collision, replay or simulator verification. Effective contact-feed values are not inferred from G-code. The committed blisk corpus differs from the owner's historical pencil project, so these results do not stand in for that project.

| Corpus | Fed blocks | Individual blocks below 7 ms | Deficient windows | Lowest window mean | Added time |
| --- | ---: | ---: | ---: | ---: | ---: |
| Ten reference Z1 programs | 8,127 | 0 | 0 | 42.420 ms | 0 s |
| Committed blisk axial | 8,641 | 191 | 0 | 15.629 ms | 0 s |
| Committed blisk helical, 2 degree | 8,611 | 88 | 0 | 17.787 ms | 0 s |
| Committed blisk helical, 5 degree | 6,896 | 71 | 0 | 17.836 ms | 0 s |

The isolated short blocks in these committed blisk programs are absorbed by compliant windows. A strict per-block cap would unnecessarily slow them.

The #284 first-cut program also has 16,400 fed blocks, one individual block below 7 ms, no deficient windows, a worst mean of 78.122 ms and no predicted slowdown. The far-end owner observation is investigated separately in [first-cut tail analysis](first-cut-tail-tab-investigation.md) (OI-993): command-rate compliance does not establish smooth A-only junction handling.

### Historical owner pencil project, regenerated

The isolated archival copy of `blisk_1_overcut_mismatch` came from `claude/upbeat-davinci-jk46yd` at `c92fa236ccc8862e5a52e80eade69c73b835d7e0`, under `docs/trial-feedback-projects/`. Its operation files were advanced to schema 16 to permit explicit spindle speed 12000 rpm; roughing entry was set to plunge, as in the prior SP-14 reproduction. The project/model files otherwise stayed unchanged and migrated normally on load. Analysis selected the Makera post, operation grouping and Report 7 ms/32; original files were not edited. Generation included predecessor stock carry-over, so its wall time under concurrent verification and a deliberate pause is not a performance benchmark.

| Operation | Individual blocks <7ms | Longest short run | Deficient windows | Worst mean | Predicted added time |
| --- | ---: | ---: | ---: | ---: | ---: |
| Roughing | 0 | 0 | 0 | 207.000ms | 0s |
| Helical finishing | 888 | 4 | 0 | 33.329ms | 0s |
| Pencil finishing (Operation 3) | 1,106 | 10 | 351 | 5.623ms | 0.426419s |

This reproduces the earlier 5.6 ms pencil window and confirms a concrete use for the 7 ms feature. The added time is predicted from the production window algorithm and conservative feed rounding; physical smoothness remains pending OI-992.

## Automated checks (completed 26 Oct 2026)

- Types/project: 205/205 uninstrumented cases passed (2.872s; longest0.936s).
- Post: 44/44 passed after the final controller-rule guard (0.249s; longest0.121s), including 14 GC-17 cases and independent emitted-feed checks for mixed rotary, A-only and G93.
- Application: all 506 cases have passing evidence. The broad run completed 504/506 in 1155.674 s; two outdated fixture expectations were corrected (same-run sheets and a manually downgraded Z1 profile) and passed focused reruns. The final 6-case focused run passed in 3.775 s. Longest individual test 547.257 s, below the 10-minute flag under concurrent load; existing long cases remain registered.
- Desktop: all 473 original cases passed across the 250 initial passes and 225 remaining/repeated cases. A new semantic Report/Slow down Files-pane test also passed (0.976s), giving 474 cases with passing evidence. The mode control was changed to an aligned dropdown after the pane-width regression identified an issue; restart persistence passed after temporary-storage recovery.
- CLI: 27/27 own command cases and the new small GC-17 setup-sheet e2e passed. The latter checks matching HTML/PDF diagnostics and successful replay from the same posted run.
- Workspace clippy, all targets, warnings denied: passed on the final source (including the new Files-pane test); final lean run 5m 38s including dependency rebuilds. Formatting and whitespace checks passed.
- `cargo xtask all`: deps, licences, records, testdata, conflicts, publish and trace passed; 100/100 P1 requirements have tests. Fuzz metadata/lockfile resolves the 0.22 library family; no fuzz campaign was run.

Own-test line-coverage lower bounds are types 77.52%, project 91.07%, post 89.41% (new window module 96%), and CLI 73.55% (all 14 changed CLI executable lines covered). Focused application coverage is 15.60%, from 5 GC-17 cases rather than a whole-suite instrumented run. Focused UI coverage is 4.35% (1254/28859 lines); four targeted cases passed in 2.426 s, including both visible Report and Slow down summaries, effective sources, dropdown interaction and bounded layout. This is a focused subset, not whole-suite UI coverage. These are below the 95% development target, which is report-only. New defensive post error branches, empty-path/legacy paths and host-only type-label formatting account for uncovered changed lines; the full uninstrumented integration tests cover the host flows. Coverage was not inflated by workspace-wide instrumented tests.

Initial verification builds exhausted separate temporary and workspace storage, causing compilation/linker failures; their results are not counted as passes. Generated caches were cleaned, and the remaining builds used debug=0, incremental=0 and one build job. Full uninstrumented test results above remain the timing evidence.

## Independent LinuxCNC check

`openchuck-post/examples/short_block_gate3.rs` generates two simulator-only programs with40 fed moves of 0.01 mm, nominal 1 ms each, using the production LinuxCNC post. Report preserves the original 0.040000 s; Slow down at 7 ms/32 gives 0.280047 s, adding 0.240047 s without coordinate changes.

Both programs ran through the repository's unchanged LinuxCNC driver in the published 2.9.10 image, digest `sha256:22d8b9f0acb8a0fd7a02bf47be7fd9e864e671bd510ca27ab35ff214d1489546`. After the wrapper's initial build attempts hit storage limits, the same driver and already-built CLI were invoked directly. Both reports contain no errors and finish at X0.4 Y0 Z50 A0. Independent replay comparison at the stricter 0.001 mm tolerance visits all 41 moves (initial rapid plus 40 fed moves); largest reported difference 0.0010. The longest driver sampling gaps were 8.7 ms and 19.7 ms; collinear targets are checked in order using the driver's sampled path/interpolation. This checks dialect/path/final position, not Z1 planner smoothness. Timing is checked separately by independent feed-rule replay tests.

Program SHA-256 fingerprints:

- Original: `fb47a64f96d53478049d7a6637b4df2059f7096631819d8563024383df6a1271`.
- Slowed: `47a93a2e6faa130c4861ea193d25ba5a187aa1701a69c5d6146e76d02f79dd51`.

The generated programs, raw reports and strict comparisons are retained under `/workspace/scratch/shortmove-gate3*` for this session. They are simulator-only evidence; they are not an attended Z1 trial.

## Physical validation still pending: OI-992

1. Select an affected representative path and generate original Report and mitigated Slow down outputs, preserving all other inputs. Record settings, changed feeds, geometry identity, predicted added time and file fingerprints.
2. Pass the normal static, independent replay and LinuxCNC simulator gates for both programs. No new real-machine program is authorized by a software test result alone.
3. Follow the existing Z1 gate-4 procedure: attended, no tool/material, clearance checked, emergency stop available. Compare time, audible stutter and visible pauses, retaining video if available.
4. After a successful air cut, use representative material and tooling to compare cutting behaviour, dimensions and surface finish. Lower feed preserves programmed geometry but changes cutting conditions.
5. Record firmware/controller versions, delivery method, override, threshold/window, actual timing and outcome. Do not close OI-992 on nominal software compliance or the older #284 trial alone.

No physical Z1 access, new air cut or material-cutting comparison was available in this session. Generated G-code remains the user's to verify before running (NFR-12).

## Released integration, 26 Oct 2026

The Merge Manager marked PR 356 ready and released one main integration and one push. Main `ba10400fff3bb7ad32526a7e540919c810e85434` includes GATES 19/5 Report compatibility and STEPALL library 0.23.0. SHORTMOVE preserves those APIs, evidence and migrations and uses schemas 20/6 and library **0.24.0**. Decision 0245 is this lane's administrative renumber of its colliding 0244; GATES' separate bounded-replay 0244 acceptance from #357 remains intact. OI-992/993 remain open. Prior local results above do not establish an integration pass. The released integration checks below apply to the resolved 0.24.0 tree.


The owner subsequently requested substantiated CodeRabbit fixes including minor findings. Both findings are included in the released integration push: singular controller-window units in CLI/UI/sheets/warnings, and confirmation lists that omit disabled timing settings and the software mode policy. Regression checks cover active/disabled profiles, published 19/5 compatibility, premature Slow down rejection, and both HTML/PDF unit labels. The first new PDF assertion searched its escaped text stream for unescaped parentheses; it failed in the broad and initial instrumented runs, was corrected to check the visible unit, and passed all four focused posting tests in 3.687 s. Those failures are not counted as passes.

- All 48 focused GC-17 instrumented cases passed in 36.285 s, including all review regressions. This is coverage evidence, not an uninstrumented timing measurement.
- Final workspace all-target Clippy with warnings denied passed after the last test-fixture correction (10.90 s incremental check). Formatting and lane-diff whitespace checks against released main passed; inherited STEP/PDF fixture bytes remain untouched. `cargo xtask all` passed deps, licences, records, testdata, conflicts, publish and trace, with 100/100 P1 requirements having tests. Both `fuzz/` and `benches/` passed locked all-target `cargo check`.
- Final core own-test coverage: types 77.66% (2162/2784 lines; 61 cases), project 90.79% (3923/4321; 175 cases), post 88.20% (942/1068; 55 cases). Focused GC-17 host coverage is application 19.87%, UI 3.97%, CLI 39.48%; these host figures are subset lower bounds, not whole-suite reports. The report-only 95% development target remains unmet. Changed-line gaps include host-only type labels, defensive empty/window fallbacks and feed-representation errors, legacy empty-path handling and plural/off display branches absent from the focused subset. The full uninstrumented host-suite results are reported separately below. Coverage artifacts were exported and instrumented build data cleaned.
- The unchanged repository LinuxCNC wrapper ran freshly generated original and slowed programs in the same pinned 2.9.10 image at 0.001 mm tolerance. Both reached all 41 moves and X0.4 Y0 Z50 A0 without a controller fault. The original first comparison missed the X0.39 sample (6.1 ms longest sampling gap); its one permitted retry passed (5.0 ms), and the first failure remains retained. Slowed passed first attempt (3.5 ms). Per-attempt wall times were 28.072 s, 28.064 s and 44.954 s, including startup; nominal fed times remain 0.040000/0.280047 s. Program hashes match the historical pair above exactly. Raw reports and logs are under `/workspace/scratch/shortmove-integration-gate3*`.
- No committed golden G-code or project fixture differs from released main. Off/Report preserve motion and feed output; normal generator-version comments advance with 0.24.0. Selecting Slow down intentionally changes affected F words and programmed-time comments under accepted decisions 0222/0245. Coordinate words and geometric tolerance are preserved. No new physical machine result is claimed.


The full six-crate uninstrumented run completed in **1722.641 s**: **1384/1386 passed**, with the PDF assertion above and one pre-existing inspector fixture failing. The latter selected the second text box positionally; the inserted machine controls made that the bounded block-window field. Correctly rejecting the fixture's 30-digit window created an error row and changed the visible-box count. The fixture now focuses Stock and selects Length by label, checks the target is visible and the machine window is unchanged, and retains every original width, alignment, typing and post-Enter assertion. An added magnitude assertion was discarded because text inserts at the caret in `14.000`, rather than necessarily replacing it. No product validation was weakened.

After both fixture corrections, the final **51-case uninstrumented rerun passed in 10.508 s**, including every GC-17 case plus the inspector typing, splitter and entry-box regressions. Every case in the six-crate run therefore has passing evidence: types **61**, project **175**, post **55**, application **540**, CLI **69**, desktop **486**. This is broad-plus-focused passing evidence, not a claim that the first broad run was green. The longest individual test was first-cut regeneration at one/two/N threads, **360.121 s**; its simulation passed in **96.086 s**. Nine tests exceeded a minute, all already registered in both long-test profiles; none exceeded ten minutes. No new long-test exception was needed.

The final push-time allocation audit re-fetched all remote heads, including #357: main stayed at `ba10400fff3bb7ad32526a7e540919c810e85434`, no other ref held decision 0245 or library 0.24.0, and OI-992/993 and schemas 20/6 were held only by this lane. Every newer main dependency and register row remains present. The merge author and committer are Paul Hampson using the linked GitHub noreply identity.
