# STEPALL-M handoff — paused 24 Oct 2026

The owner paused work for handover, then explicitly requested this handoff be pushed to the existing branch. No benchmark is running. This is a progress checkpoint, not a release or a waiver of outstanding criteria.

## Branch and authority

- Repository: `paulhampson/OpenChuckCAM`; branch: `claude/lane-stepall-m-measure`; [PR #321](https://github.com/paulhampson/OpenChuckCAM/pull/321), **draft**.
- Product and measurement source at pause: `eee448609348e7ecc5bbfe71e8b3377d91170b32`. This handoff commit publishes that source and its preceding local commits together. The previous remote checkpoint was `4c2d728ef51678c0ab9a5154dc1c361ea236c51a`; the next person can fetch this branch rather than relying on a local bundle.
- Main `1a90435654c734c05e4fa3f110c4a7f3f8e5859f` was merged at `1248937398134c21ce0173a09e778929e2f87006` under the owner's explicit exception allowing draft preparation. Read `AGENTS.md`, [documentation precedence](README.md), and records 0209, 0211 and 0234 before continuing.
- Keep the PR draft. Do not mark ready, merge, enable auto-merge or release to Merge Manager.
- The latest implementation instruction was: “Complete all steps and also include changes for #324 in this work.” Work was subsequently paused. The push instruction authorises publishing this checkpoint; leave measurements paused.

## Decisions already made

[0211](decisions/0211-step-accuracy-measured.md) is **accepted directly by the owner** (“Accept 0211”). Do not ask again. [0234](decisions/0234-replay-output-rounding-correction.md) is **proposed**, with implementation explicitly authorised. Show the complete final 0234 before requesting acceptance, and ask about the decision record **last**, after work, results and the final push. `AGENTS.md` reserves record acceptance to the owner; implementation authorisation does not imply record acceptance.

The owner selected a fixed finer toolpath mesh as a direction, subject to all verification, with no setting/default/schema adopted. Query and trim foundations, including OI-839, precede exact ball drops, then other profiles in STEPALL-X. Both whole-job and each-operation generation must be within 2× the import-tessellation baseline using medians of five successful equivalent runs at all threads; retain five-run single-thread reporting. Preserve the replay limit and every verification requirement. Document simultaneous finishing's blisk refusal without separate tracking/investigation. Remeasure pencil before deciding separate performance tracking. Do not start STEPALL-T/X here.

The owner superseded central number allocation: this lane audited 26 remote branches and claimed 0234 / OI-984, published by [issue #324](https://github.com/paulhampson/OpenChuckCAM/issues/324). BUG-EDIT's 0233 / OI-983 are preserved. No new number is needed for this handoff.

## Completed work

**#324:** the post rounds commanded A first at existing precision, transforms the original IR tip at that written A, then rounds XYZ. Replay's collapsed-move recognition uses written A; the original tip comparison and limits remain. No decimal precision, dialect, default or schema changes. Record 0234 contains the endpoint isometry argument and limitations.

Red/green regression covers both rotary axes/senses, nonzero origin, duplicate rapid and fed move, both posts. Original handwritten failing output remains rejected. The 44 post/G-code tests, eight app round-trip tests, reference determinism and independent golden audit pass. Only two lobed goldens changed and were reviewed. The original saved CLI reproducer passes the unchanged gate (printed tip 0.0008 mm, A 0.0005 degrees). The changed lobed LinuxCNC reference passes gate 3: 1,806 moves, no fault. The downloaded simulator image was deleted to free disk; its digest/report evidence remains in [0234 evidence](decisions/0234-replay-output-evidence.txt).

**Full-stepover M4 passes:** a/b/c library serialized results and posted bytes agree at 1/2/4 threads. STEP and STL application jobs pass replay and posted equivalence at every count; b application IR matches the library. Simultaneous finishing is explicitly disabled after refusal: six successful operations, no successful seven-operation claim. Preserve its earlier correction-source provenance (`f052ee8`, before timing hooks/main integration). [Full M4 evidence](decisions/0211-full-stepover-m4-evidence.txt). Those contended correctness durations are not M3 samples.

**M1 is complete and uncapped:** every triangle has 21 stations. Original 0.01 mm / 0.02 rad candidate: 106,928 triangles, maximum underlying-surface station deviation 0.00570 mm, exceeding the 0.005 mm allocation. Tighter 0.001 mm / 0.01 rad investigation: 371,688 triangles, 7,805,448 stations, maximum 0.00176 mm. This is a station result, not a continuous certificate or adoption. Trimmed-plane diagnostic remains 0.04282 mm and blade normal discrepancy 4.32 degrees. The tighter candidate has no M2/M3/M4 qualification; original-candidate results cannot transfer to it. [Evidence](decisions/0211-uncapped-m1-evidence.txt).

**M2 fails qualification:** an analytic plane with the actual flat-cutter sampler demonstrates a 0.006666667 mm miss against required error below 0.001 mm. The harness now prints raw observations then rejects a missing proven bound. No bounded replacement exists. Keep this failure visible; OI-834 carries the criterion and OI-839 remains STEPALL-X. [Diagnostic](decisions/0211-flat-cutter-sampling-diagnostic.txt).

Optional timing hooks measure inclusive/summed fitting scopes and assembly, plus complete job elapsed time. Drop/fitting scopes overlap and can run in parallel: never add or subtract them as an exclusive wall-time breakdown. Job elapsed time includes stock construction, generation, simulation and bookkeeping; excludes pool creation, import, posting and replay. An early incomplete M3 attempt exposed axial/indexed's quiet context losing fitting callbacks. Fix `eee4486` privately forwards timing while suppressing inner progress. Red/green tests check both strategies at 1/4 threads for unchanged paths/progress, disabled timing silence and enabled stage presence. All 18 axial/indexed tests pass; focused changed-line coverage, clippy, release build, format and register checks pass. [Validation](decisions/0211-quiet-pass-timing-evidence.txt).

Integrated validation after the main merge passed 64 selected integration tests (349.460 s), 14 native harness tests (28.836 s), lint, formatting, records, dependency/licence/conflict/test-data/publish checks, traceability 99/99 and cargo deny. Earlier 382 path/types and 44 post/G-code tests passed. Restricted coverage lower bounds are documented, not a 95% suite claim. Three tests over one minute are listed as long; none exceeded ten minutes. Full platform CI/external review remain pending release. [Integrated validation](decisions/0211-integrated-validation.txt).

## M3 checkpoint and remaining measurements

Pinned source: `eee448609348e7ecc5bbfe71e8b3377d91170b32`. Pinned executable SHA-256: `f882e7ea261084a4dbc23f7fd717f99eeafae6a606959b0bd3bf705d4bde6c2e`.

All three **four-thread batches completed**, five successful runs each, independently validated. Six operations run in stock-carrying order: roughing, rest, pencil, helical, axial, indexed. Full stepovers, original 0.01 mm / 0.02 rad candidate, one untimed all-thread warm-up per batch. No simultaneous-refusal time contributes to a successful benchmark.

| Form | Generation median (s) | Complete job elapsed median (s) |
| --- | ---: | ---: |
| a: owner STL | 30.235682 | 168.087635 |
| b: import STEP mesh | 18.702136 | 326.859757 |
| c: finer STEP mesh | 33.528479 | 337.920408 |

The c/b generation ratio is **1.792762×**, passing the whole-job limit. Per-operation ratios: roughing 1.293156×, rest 1.907813×, pencil 1.876483×, **helical 2.454932× (fail)**, axial 1.597802×, **indexed 2.047368× (fail)**. The performance criterion therefore fails overall. No waiver is granted. Job elapsed ratio is 1.033839×; it does not replace the generation criterion.

All three **single-thread batches remain**. The b-1 batch was terminated on the owner's pause before any complete timed run. Its partial log is preserved with `.paused-by-owner`, with no `.done` marker. The earlier attempt with missing timing callbacks is preserved as `.incomplete-quiet-timings` and excluded. Do not count either partial attempt. Runner, child and watcher were stopped; the allow marker was removed.

[Portable checkpoint files](handoffs/stepall-m-2026-10-24/) include raw logs, completion markers, source/binary manifest, partial summary and orchestration/audit scripts. The scripts are handoff utilities, not product APIs; they retain the original `/workspace` paths. No executable, worker, saved project or owner STL is committed.

### Resume in the existing environment when work resumes

The original files remain in `/workspace/stepall-evidence`. Validate the binary hash and completed batches before starting:

```sh
cd /workspace/OpenChuckCAM
source /workspace/.tooling/openchuck-env.sh
sha256sum /workspace/stepall-evidence/m3-step-accuracy
python3 /workspace/stepall-evidence/summarize-full-m3.py --partial
touch /workspace/stepall-evidence/allow-full-m3
python3 /workspace/stepall-evidence/run-full-m3.py
```

Run `python3 /workspace/stepall-evidence/watch-m3.py` separately for progress. The runner skips completed all-thread `.done` files and restarts b-1, then c-1 and a-1. Five runs and simulation can take hours. No CPU-intensive builds, tests or coverage during timing. Do not use quick stepovers, reduce samples, substitute geometry or count refusal time as success.

After all six batches finish, run `python3 /workspace/stepall-evidence/summarize-full-m3.py` without `--partial`. It validates five successful repetitions, finite stage times, stage presence, consistent drop/query counts across runs and threads, manifest hashes, elapsed time enclosing generation/simulation, independently recomputed medians, speedups and c/b ratios. Assess full-sequence pencil threading; its historical lack of benefit is already obsolete for the separate isolated case, but full-sequence speedup is still pending.

### Recovery in another environment

Fetch this branch. Copy the portable checkpoint directory above to `/workspace/stepall-evidence` if retaining its original paths, and inspect scripts before execution. Rebuild the harness from the **pinned source**, not an unrelated later head, and obtain the native worker and authorised trial STL. Do not silently overwrite the manifest while reusing completion markers for a different binary. Preserve existing results with their provenance; single/four-thread speedups must use the same machine and equivalent builds/settings. On a different machine, rerun the necessary comparison sets rather than combining machines into a speedup. The current completed all-thread ratios remain evidence of the original machine.

## Remaining completion steps

1. Complete and independently audit M3. Existing M4 need not be rerun solely for the private timing forwarding fix; preserve its exact earlier provenance.
2. Commit final raw timing evidence, commands, hashes, medians and stage/thread tables. Record whole-job pass and helical/indexed failures truthfully; retain M1/M2 limitations.
3. Update 0211, OI-834/OI-835, the STEPALL work-breakdown row, 0234/OI-984 and any final pencil-tracking recommendation. Existing OI-835 already tracks the 2× criterion. No adopted setting, silent budget waiver or STEPALL-T/X implementation.
4. Run relevant checks for subsequent changes and final format/records/diff checks. Existing validation above need not be repeated without a relevant change or concern.
5. Fetch actual remote head, verify fast-forward, commit/push the final results and verify GitHub head/draft state. This handoff push publishes the current implementation and partial timing evidence; it does not complete the remaining work.
6. Rewrite PR #321 and issue #324 descriptions for final results. At pause both still had progress banners saying local implementation/grouped push pending. That historical text becomes stale after this handoff push. Fetch fresh bodies before editing. Issue #324 remains open awaiting merge; PR remains draft.
7. Report unresolved physical sampling, accuracy qualification and per-operation performance gates. Do not claim full decision-0209 verification. Show full final 0234 and request owner acceptance last; 0211 is already accepted. Acceptance is not lane release.

## Environment and local-only artifacts

- Rust 1.97.0 through `/workspace/.tooling/openchuck-env.sh`, Cargo/Rustup homes under `/workspace/.tooling`, build jobs 4. GCC 14.2, CMake 4.4.4, static OCCT 7.8.1 at `/workspace/OpenChuckCAM/target/OCCT` (`OCCT_ROOT`).
- Native tests/runs need `OPENCHUCK_STEP_WORKER=/workspace/OpenChuckCAM/target/release/openchuck-step-worker`; `RAYON_NUM_THREADS=4`. CPU AMD EPYC 9V74, quota 4, RAM 16 GiB. Approximately 6 GiB disk free at pause. Focused coverage was cleaned.
- Worker SHA-256: `01b0997bb43e7f1aeb5d05e988731f7444bee41f4595086c0590b902fc1408b9`.
- Owner trial STL `/workspace/owner-blisk.stl`, SHA-256 `48351f42635f05eea09aaf77dd685e7b2bd564cd087b4a336fb29554a7e33f37`. **Do not commit it.** If unavailable, original authorised source is `claude/upbeat-davinci-jk46yd:docs/trial-feedback-projects/blisk.stl`; `testdata/stl/blisk.stl` is different.
- STEP fixture SHA-256 `0e472ff5374c6bc40825a4f7e8885d69f6ac99e53fbb6dbc412104fadb55b646`.
- Extra raw evidence, saved projects, scripts and pinned executable: `/workspace/stepall-evidence` (208 MiB at handoff). This directory is not fully committed. Selected verification evidence already lives under `docs/decisions/0211*` and `0234*`; M3 checkpoint text is copied alongside this handoff. `CURRENT.txt` is a historical journal; this handoff takes precedence for pause status.
- Original pasted request: `/workspace/attachments/95ecda79-7a93-42c5-b60b-90c2a1648fa0/Pasted text.txt`. The original request's proposed-0211/central-number instructions were superseded by explicit owner choices above.
- Human Git identity: Paul Hampson `<528002+paulhampson@users.noreply.github.com>`; agent coauthor `Codex <noreply@openai.com>`. Cloud-runtime skill was read during environment setup. Network operations require the environment's network permission. No messages/comments to others were sent. No subagents were used.
