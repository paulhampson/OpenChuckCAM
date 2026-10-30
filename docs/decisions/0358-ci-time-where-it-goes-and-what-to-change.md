# 0358: Where the CI time goes now, and what to change within 20 workers and free runners (lane CISPEED)

- **Status:** proposed (record). **Owner's choices on the options, 30 Oct 2026, relayed by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7, decision 0049):** Options 1, 2 and 3 approved to implement; Option 4 approved with the owner's refinement (see *Option 4, detection design*); Option 5 not chosen yet. The record itself stays proposed until the owner accepts it. Each approved option is built as its own small draft pull request from lane CISPEED (OI-1410 to OI-1413), merge order (b) and (a) first.
- **Date:** 30 Oct 2026.
- **Lane:** CISPEED (the devops manager role). Record branch `docs/decision-0358-ci-time`, docs only.
- **Requirements:** none change. GC-07 and NFR-06 (deterministic output) are untouched: no option changes a test, a golden file or a build flag of the product.
- **Builds on:** [0052](0052-ci-speed-up.md) (the first speed-up round, accepted 16 Oct 2026), [0055](0055-push-discipline-and-draft-pull-requests.md) (one push per pull request, draft gate), [0039](0039-occt-install-cache.md), [0102](0102-coverage-off-pull-requests.md), [0249](0249-ci-skip-heavy-jobs-for-markdown.md), [0250](0250-ui-smoke-linux-leg-pinned-to-ubuntu-24-04.md), [0251](0251-step-kernel-linux-leg-pinned-and-occt-cache-key.md).
- **Open items:** OI-1410 to OI-1415 (new). OI-224 and OI-225 (0052) stay open: OI-225 (the cache list is not readable through a session's proxy) still blocks the first check below.

## The owner's rules for this record

At most 20 jobs running at once for the whole project, summed over everything that can run together, and no paid compute (no larger runners, no paid cache service). Free hosted runners only. Every option below is checked against both rules; the ones that break either are listed as rejected.

## What was measured

Source: the Actions API for `ci.yml` (25 completed runs with the full matrix, 29 to 30 Oct 2026: 13 pull request runs and 12 pushes to `main`) and `e2e.yml` (12 runs). Times are minutes. The cache listing cannot be read through a session's proxy (403), so cache behaviour is read from step times and is marked as a hypothesis where it is.

1. **The limit that binds is the 20 jobs, not one run's length.** A full pull request run starts 17 to 19 `ci.yml` jobs at the same moment (measured peak, 25 runs), and `e2e.yml` adds one more (9 to 15 min), so **one pull request run uses 18 to 20 of the 20 jobs**. A second run, or a push to `main` after a merge, waits: queue time on a single job reached 10 to 23 min. Half of the runs (12 of 25) are pushes to `main`, each the same size as a pull request run, so a merge costs about two full runs. At about 340 runner minutes a full run (sum of the median job times), the project's 20 jobs can finish only about 3 full runs an hour at best. **Cutting runner minutes and jobs held, on the pull request run and on the `main` run, raises throughput more than cutting one run's length.**
2. **One run takes 30 to 52 min, median about 38.** The last job to finish is almost always `Build and test (windows-latest, shard 1 of 2)` (median 34, max 43.5), then macOS shard 1 (32). Per job medians: Windows 34 and 29, macOS 32 and 25, Ubuntu 28 and 23 (shards 1 and 2); `Each library alone` 23 (Windows), 20 (macOS), 18 (Ubuntu); Gate 3 18 to 22 each; `STEP kernel` 5 to 8 with a cache hit, 25 to 45 without.
3. **The shards are not balanced.** On Ubuntu the `Tests` step took 25 to 27 min in shard 1 and 13 min in shard 2 (runs 41096871005 and 41058487318); on Windows 25 to 28 and 22 to 23. The partition is by a hash of the test name, so the 11 tests of 100 s or more (0052) land where they land. The slower shard sets the run; the faster one idles. `Check the test partition` adds 2 to 8 min a leg (it lists the tests three times and the first listing builds them).
4. **`Each library alone` repeats tests.** Its `Test each library-family crate on its own` step takes 21 to 24 min a leg (serial `cargo test`), mostly `openchuck-path`, whose tests `Build and test` already runs. Three jobs and about 60 runner minutes per run. Not on the critical path (18 to 23 against 34), so shortening it does not shorten the run, but it holds three of the 20 jobs for 20 minutes. Decision 0052 accepted a fix (Option 5, nextest) as branch `claude/ci-speed-up-libs-nextest`; that branch shares no history with `main`, was never merged, and `ci.yml` on `main` still runs `cargo test` there.
5. **The OpenCASCADE cache almost never hits.** The `occt` step took 19 to 36 min (a build) on `STEP kernel` legs in 10 of the 12 runs where the job ran, including pushes to `main`, and `occt-cache.yml` (daily and on `Cargo.toml` changes) takes 26 to 45 min each time. A hit restores in about a minute. Each miss costs 25 to 35 runner minutes a leg, so up to about 90 a run, and on a miss the STEP leg ends close to the Windows tests (run 41096871005: 17:55 against 17:53). **Hypothesis, not yet checked:** every push to a pull request saves new `rust-cache` entries (300 to 800 MB per leg and shard, about 14 legs), and a cache written on a pull request branch is visible to that pull request only; together they fill the 10 GB repository limit and evict the OpenCASCADE installs and the `main` caches before they are used. The check is `gh cache list --limit 100` (sizes, refs, last used) from a machine with the owner's login.
6. **Compile is small, test time is the time** (0052 still holds): the build step is 1 to 3 min, the `Tests` step 13 to 28 min.
7. **Failures show late in effect, not late in time.** In the 6 red pull request runs of the sample, the heavy failure appeared 3 to 26 min in, and the run still went on to about 40 min because `fail-fast: false` is deliberate (0052: one run names every failing check, so one push fixes them all). None of the failures was a rustfmt or clippy failure that a quick gate before the heavy jobs would have caught earlier.
8. **No duplicate runs.** `ci.yml` runs on `pull_request` and on `push` to `main` only; there is no `push` plus `pull_request` pair for a branch. A newer push to a pull request cancels the older run (14 of the 30 runs on 16 Oct; 4 of the 20 listed pull request runs on 30 Oct).
9. **Draft gate and path filters work.** A draft runs only `changes` and `fast-checks`; a Markdown-only change skips the heavy jobs (0249). The `step` filter runs `STEP kernel` for changes to `openchuck-path`, `-app`, `-geom`, `-types` or `-import`, which is most feature pull requests, so narrowing it saves little.

## Options, ranked

"Saving" is wall time of a run (W) and runner minutes (R). "Jobs" is the change in jobs held at once on one pull request run (peak 19 to 20 today). Estimates are from the figures above and are replaced by measurements after each change (OI-1415).

| # | Option | Saving | Jobs | Risk | Cost to keep |
| --- | --- | --- | --- | --- | --- |
| 1 | **Keep the OpenCASCADE and `main` caches alive.** Check the cache list first; then `rust-cache` saves only on `main` (`save-if: ${{ github.ref == 'refs/heads/main' }}`), pull requests only restore (they read `main`'s entries). | R: about 90 per run that misses (10 of 12 STEP runs), plus 80 to 100 per `occt-cache.yml` rebuild. W: 2 to 8 min on those runs, more when a STEP leg is the last job. | 0 | Low. A pull request's repeated push no longer finds its own warm cache; build step is 1 to 3 min, so at most that much. Does not change any test. If the check shows the cache is not full, the cause is something else and this option is dropped. | One line per `rust-cache` use |
| 2 | **`Each library alone`: nextest, as 0052 Option 5 accepted** (still per crate, `-p`, and doctests after). Optional stronger form: on Windows and macOS only compile the tests alone (`cargo test --no-run`), run them on Ubuntu. | R: about 25 to 35 per run (nextest), about 55 (stronger form). W: none (not the last job). | 0 now; frees three jobs about 10 min sooner | Low for nextest (same tests, same crates). The stronger form needs the owner: it stops running the library tests alone on two systems (they still compile alone and run in `Build and test` there). | Small |
| 3 | **Balance the two shards by name.** Put the 11 longest tests, split by their measured times, one list per shard in `.config/nextest.toml` (filter sets), and share the rest by hash. The partition check is extended to prove every test runs once. | W: 3 to 6 min (Ubuntu 25 and 13 toward 19 and 19; Windows 28 and 22 toward 25 and 25). R: 0. | 0 | Low to medium. A long list goes stale (0052 found the same for the long-test list), so the check must fail loudly if a listed name no longer exists or a test runs twice or not at all. No vacuous pass. | One list to refresh when a test grows |
| 4 | **A lighter run on `main`, only when the merged content was already tested.** A push to `main` runs the **light** run (fast checks and Ubuntu build and test) when the pushed commit's git tree is the tree a full, green pull request run already tested; in every other case it runs the **full** matrix as today. The full matrix also runs once a night. The pull request run is unchanged. Owner's refinement approved; detection in the next section. | R: about 200 to 250 per merge (the Windows and macOS legs, `Each library alone`, UI smoke, most STEP legs). W: none directly; a merge frees 12 to 15 jobs for the next pull request, which removes most of the 10 to 23 min waits. | -12 to -15 on `main` runs | Approved with the refinement. A merge that is not byte-for-byte the tested tree still gets the full run, so the case that could hide a Windows or macOS break (two pull requests combining) is not softened. What is lost: the light run does not repeat a test that already passed on the identical tree. | One `if:` per heavy job |
| 5 | **Move work off the Windows leg.** Run `clippy` and the partition check once on Linux (in `Fast checks` or the shard 1 Ubuntu leg) instead of in the Windows and macOS shard 1 legs. | W: 1 to 3 min on the critical leg (clippy 51 s on Windows; the partition check 6 to 8 min of which the build part is kept). Needs a measurement of the split first (OI-1415). | 0 | Low to medium: clippy can differ for `cfg(windows)` and `cfg(macos)` code, so it stays on those systems unless the owner agrees it need not. The partition check can stay per system (test lists can differ by `cfg`). | None |

### Option 4, detection design (owner's refinement)

Goal: a push to `main` is **light** only when the commit's content is exactly what a full green pull request run already tested; otherwise **full**.

**Recording the tested tree.** In a pull request run that is not a draft and whose heavy jobs all passed, the `CI required checks` job uploads a small artifact `tested-tree` holding the git tree object id of the commit the run checked out (`git rev-parse HEAD^{tree}`; for a `pull_request` run that is the test merge of the pull request head into the base branch, `refs/pull/N/merge`) and a flag `full=true` only when every heavy job whose filter is true ran and succeeded (a run where `rust` was false, a docs-only change, records `full=false`).

**Deciding on the push.** A first job in the push run (`push-mode`) does this, on a push to `main` only: (1) find the pull requests of the pushed commit (`GET /repos/{r}/commits/{sha}/pulls`); (2) require exactly one, merged into `main`, whose merge commit is the pushed commit; (3) list that pull request's `ci.yml` runs for event `pull_request` (`GET /actions/workflows/ci.yml/runs?event=pull_request&head_sha=<pr head sha>`), take the newest completed one with conclusion success, and download its `tested-tree` artifact; (4) compare the artifact's tree id with `git rev-parse HEAD^{tree}` of the pushed commit (a squash or rebase merge of an up-to-date branch gives the same tree as the test merge; a merge commit of an up-to-date branch also does). **Equal and `full=true`: light. Anything else: full.** The comparison is on the tree, not on the commit ids, so the merge method does not matter.

**Failure modes, and what each does.**

| Case | Result |
| --- | --- |
| The branch was behind `main` when merged, or `main` moved after the last pull request run (the test merge differs from what landed) | Trees differ: full. This is the "untested true merge" case. |
| Direct push to `main`, a revert, a pull request with no completed full run on its head, or more than one pull request for the commit | No match: full. |
| The API call, the artifact download or the tree read fails (rate limit, expired artifact, network) or the artifact is malformed | Full. Any error means full; the job never fails the run because the check failed. |
| A pull request run was green, then its commit was force-pushed away and a different head was tested | The head sha in step 3 names the exact head that was merged, so only a run on that head counts. |
| Runner image or toolchain changed between the pull request run and the push (same tree, different environment) | Light. The nightly full run catches it within a day. |
| The tree matches but a flaky job passed on the pull request run by luck | Same as today: the pull request run is the gate. |
| `workflow_dispatch` or a schedule run | Always full. |

**Reliability.** The tree id is a content hash of every file, so "equal" means the tested files are exactly the files that landed; there is no heuristic. The risk is in the plumbing (a wrong job condition makes a push light or full wrongly), which is why every uncertain path is full, and why the first implementing pull request is to be measured over 10 merges (OI-1415): the light share and any case where light was chosen while the trees were in fact different (none is possible by construction, the check is the comparison itself).

**What the nightly run still covers.** A scheduled full run of `ci.yml` on `main` (one job set, nothing else is running at night) covers the Windows and macOS legs, `Each library alone` on all three systems, Gate 3, the STEP kernel on all three systems and UI smoke after a light merge. It also refreshes the `main` caches of every leg, which pull requests restore (Option 1 saves only on `main`); a light run alone would leave the Windows and macOS caches un-refreshed, so the nightly run is part of Option 1 working, not an extra.

**Jobs.** `push-mode` is one short Linux job. The light run holds 6 to 8 jobs instead of 18 to 19. The `CI required checks` job on `main` accepts the skipped heavy jobs only when `push-mode` said light.

### Considered and not recommended

| Option | Why not |
| --- | --- |
| Stop the run at the first red job (`fail-fast`) | Saves about 150 runner minutes on a red run, but one run would no longer name every failure, so decision 0055's one push per pull request would turn into two or three. Net loss in time. Owner's call if wanted. |
| A fast gate (rustfmt, clippy) before the heavy jobs | Adds 3 to 6 min to every green run; none of the 6 red runs in the sample would have been caught earlier by it. |
| More shards (3 or 4) of `Build and test` | Adds jobs; a run already holds 18 to 20. Worth reconsidering only after Options 2 and 4 free jobs. |
| More Gate 3 shards | Gate 3 (18 to 22 min) is not the last job; each program runs in real time, so the longest program bounds it. |
| Caching the build with sccache or a shared target directory | Compile is 1 to 3 min of 25 to 35; `rust-cache` restores already hit. |
| Larger runners, paid caching, paid queue priority | Break the free-compute rule. |
| Narrower `step` path filter | `STEP kernel` already only runs for code that can change the wrapper; most feature pull requests touch those crates. |

## Recommendation

Do 1 first, after checking the cache list (it is also the largest unknown), then 2, then 3; they weaken nothing. Decide 4 separately: it is the largest saving of all (about a third of all runner minutes) and the one that most cuts waiting between pull requests, but it is the only one that changes what a merge proves. Take 5 only after the split is measured. Expected result if 1 to 3 land: the last job drops from about 34 to 27 to 30 min, a full run from about 38 to 30 to 33 min, and about 150 fewer runner minutes a run (more with 4). Each change is its own pull request, so each can be measured and reverted.

## Open items

| OI | Item |
| --- | --- |
| OI-1410 | Option 1: read the cache list (`gh cache list`) to confirm what fills the 10 GB limit, then `save-if` on `main` for `rust-cache`; confirm the OpenCASCADE installs are hit afterwards. |
| OI-1411 | Option 2: nextest in `Each library alone` (0052 Option 5 never reached `main`); the owner decides whether the stronger form (compile only on Windows and macOS) is wanted. |
| OI-1412 | Option 3: balance the `Build and test` shards by named long-test lists, with a check that every test runs exactly once. |
| OI-1413 | Option 4 (approved with the owner's refinement, see the detection design): light push-to-main run only when the pushed tree equals the tree a full green pull request run tested, otherwise full; full matrix nightly. |
| OI-1414 | Option 5: measure the build and listing share of `Check the test partition`, then decide whether clippy or the partition check leaves the Windows leg. |
| OI-1415 | After each change: re-measure with the same method (per job time, last job, peak concurrent jobs, queue, cache hit) over at least 10 runs and record the result here; also count `e2e.yml` in the 20-job budget. |
