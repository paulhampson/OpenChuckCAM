# 0052: Where the CI time goes, and how to shorten it without weakening a check

- **Status:** accepted by the project owner (16 Oct 2026), relayed by the orchestrator session at the owner's instruction ([decision 0049](0049-relayed-acceptance-of-records.md)); the relay was confirmed in this session by the user. The accepted options are listed under *Owner's decisions*; Options 6(c) and 9 are not accepted. Nothing is built yet. Each accepted option is implemented as its own pull request, in the order given there, on `claude/ci-speed-up-implementation` branches.
- **Date:** 16 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-221 to OI-225 are new; the owner's decisions are recorded against them.

Numbering: this record takes 0052 and OI-221 to OI-225, assigned by the orchestrator session across `main` and the unmerged branches. It uses no project schema number. Lane: one-off CI speed-up (record branch `docs/decision-0052-ci-speed-up`).

## Context

A pull request waits 25 to 50 minutes for CI, and the lanes run several pull requests at once. The owner asked for the time to be explained and shortened **without weakening any check**. The rules this record works within:

- The `CI required checks` job (`required` in `ci.yml`) stays as it is and keeps the name branch protection uses. Individual job names may change because they only feed it.
- Every gate of the verification plan stays: the three-platform test matrix, Gate 3 (LinuxCNC), fuzz smoke, the STEP kernel jobs, coverage reporting, `cargo-semver-checks`, `cargo deny` and `cargo audit`. A change may reorder, cache, shard, schedule, path-filter or share artifacts between jobs, provided every pull request still gets every check it gets today, or the owner explicitly accepts a named move.
- No test is skipped, disabled, quarantined or loosened, and no change may let a required check pass vacuously (a shard that runs nothing, a path filter that matches nothing).

Documents read: `AGENTS.md`, `CLAUDE.md`, `.github/workflows/*.yml`, `.config/nextest.toml`, verification plan sections 4.1, 7 and 8, decisions 0003, 0023, 0027, 0031, 0039, 0048 and 0049, and `docs/open-items.md`. No requirement, architecture section or verification-plan gate needs to change. The verification plan says only that tests over about a minute are listed in `.config/nextest.toml`, that every pull request still runs them, and that the coverage run leaves them out; it names no job layout.

## What was measured

**Sample and method.** Fourteen completed `CI` runs on 16 Oct 2026: pull requests 57 to 62 (runs 38864884980, 38865829309, 38867673495, 38868367262, 38898991685, 38933791810, 38935433295, 38937225797, 38937431714, 38938750427) and the pushes to `main` that completed (38902224422, 38902961367, 38928689071, 38939895430). Per job, *queue* is `started_at - created_at` and *run* is `completed_at - started_at`, from the Actions jobs API. Step and per-test figures come from the full job logs of run 38937431714 (PR 61, a code change that ran every job), extracted by a helper session reading the logs; one run is a small sample for test times. Not measured: the Actions cache listing (sizes, hit rates over time, eviction) because the session's proxy refuses the cache endpoints (OI-225). Runner types are the free hosted ones of a public repository.

### Per job (successful runs; minutes)

| Job | Run, min / median / max | Queue, median / max | Notes |
| --- | --- | --- | --- |
| Build and test, Ubuntu | 13.6 / 21.3 / 23.8 | 0.2 / 17.0 | nextest phase 21.6 min of 23.8; build 1.1 min |
| Build and test, Windows | 16.1 / 25.0 / 27.0 | 0.3 / 10.7 | nextest 21.0 min; build 2.7 min; cache restore 50 s |
| Build and test, macOS | 9.6 / 20.2 / 26.3 | 0.8 / 15.7 | Tests step 18.9 min in run 38937431714 |
| Coverage trend | 22.7 / 25.4 / 28.0 | 0.4 / 15.7 | 720 tests instrumented, 25.5 min; build 1.1 min |
| Gate 3 (LinuxCNC) | 20.0 / 20.2 / 23.9 | 0.1 / 11.8 | 9 programs one after another in real time, 22.1 min of 23.8 |
| Each library alone, Ubuntu / Windows / macOS | 6.1 to 13.7 | 0.3 to 1.0 / 5.0 to 10.8 | 10.8 / 9.5 / 7.5 min of `cargo test -p openchuck-path` |
| Fuzz smoke (9 legs) | 2.3 to 3.0 each | 0.1 / 3.3 | 1 min of fuzzing, 1.3 to 2.0 min of setup and build |
| UI smoke (Linux / Windows / macOS) | 1.2 / 2.4 / 1.5 | 0.1 to 0.9 / 8.5 to 13.8 | |
| STEP kernel (3 platforms) | 1.2 / 3.5 / 2.3 | up to 14.8 | OCCT cache hit; a cold macOS leg took 30.4 min (run 38868367262) |
| Repository checks, cargo deny, cargo about, semver, cargo audit, rustfmt | 0.2 to 2.0 | up to 15.9 | each starts its own runner |

### Per run

| Run kind | Jobs | Wall clock | Runner minutes | Queue minutes (sum over jobs) |
| --- | --- | --- | --- | --- |
| Code pull request or push (12 runs) | 18 to 31 | 24 to 49, typically 26 to 33 | 125 to 210 | 1 to 207 (4 of 12 runs lost 7 to 17 min on the critical job) |
| Docs-only pull request or push to `main` (runs 38938750427, 38939895430) | 3 | 3.2 to 6.3 | 2 | 1 to 4 |

A code pull request finishes when the slowest of Build and test (three platforms), Coverage trend and Gate 3 finishes, each at 20 to 27 minutes. Those five to seven jobs are the critical path in every run examined (Coverage trend 6 times, Build and test 5 times, STEP kernel macOS once, cold, in the 12 code runs). Shortening one of them alone does not shorten the run.

### Where the time inside the big jobs goes

- **Compile is small.** The Tests step builds in 1.1 min (Ubuntu), 2.7 min (Windows) and 1.1 min (coverage). All `rust-cache` restores in run 38937431714 were exact-key hits (291 to 784 MB, 6 to 50 s), and every post step said `Cache up-to-date`. Clippy takes 12 s on Ubuntu and 51 s on Windows and does not warm the test build. So compile and cache are 5 to 10 % of the job, and sccache, a shared target directory or bigger caches would save seconds.
- **The test run is the time.** 726 tests in 84 binaries, 21.6 min on Ubuntu and 21.0 min on Windows (4 vCPU runners). The long tail dominates; the hot crates already build at `opt-level = 3` in the dev profile (`Cargo.toml`, decision 0025), so the tests are slow in the algorithm, not for lack of optimisation. Tests of 100 s or more on Ubuntu (Ubuntu / Windows / coverage, seconds): `req_op_12_the_independent_checker_finds_no_gouge_on_the_reference_parts` 478 / 455 / not run; `req_sim_03_req_sim_05_finest_setting_is_within_two_hundredths_against_a_brute_force_run` 441 / 378 / not run; `req_nfr_06_simulation_is_identical_at_1_2_and_n_threads` 310 / 293 / not run; `req_op_13_feed_at_the_contact_is_within_two_percent_on_the_lobed_part` 221 / 250 / 356; `req_op_12_the_independent_checker_finds_no_gouge_and_full_coverage_on_the_oracle_parts` 211 / 202 / not run; `req_gc_07_reference_jobs_are_identical_at_1_and_n_threads` 190 / 226 / 233; `req_op_06_the_tool_axis_follows_the_normal_within_the_tilt_bound` 161 / 176 / 277; `req_op_15_smoothing_has_fewer_a_reversals_than_following_the_normal` 145 / 153 / 241; `req_sim_05_a_gouging_path_is_reported_as_overcut_with_a_location` 127 / 119 / not run; `req_sim_07_finer_resolution_lowers_the_error_and_costs_more` 126 / 88 / not run; `req_op_03_indexed_and_continuous_forms_reach_the_same_envelope` 103 / 105 / 233. A further 32 tests (Ubuntu) run 30 s or more. None is over the 10-minute flag line; the longest is 8.0 min.
- **The long-test list is stale.** `.config/nextest.toml` lists five filters (times when listed: 368, 252, 137, 122 and 121 s). Five tests of 100 s or more on a hosted runner are not covered by those filters (`req_op_13`, `req_gc_07`, `req_op_06`, `req_op_15`, `req_op_03`), and about five more run 60 to 100 s (the `req_op_04` unroll pair, `req_gc_09`, and two in `openchuck-sim`). The AGENTS.md rule says tests over about a minute are listed. Hosted runners are slower than a development machine, and the tests run 4 at a time while some of them use several threads themselves, so a listed time and a CI time differ. Whether the CI times are mostly contention is not measured (OI-224).
- **Coverage runs the same tests again, slower.** 720 tests, 25.5 min, 1.5 to 3 times slower per test. Its six exclusions remove the biggest tests; the next ones (`req_op_13` at 356 s, `req_op_06` at 277 s, `req_op_15` at 241 s, `req_gc_07` at 233 s) remain.
- **Libraries alone repeats the heaviest crate serially.** `cargo test -p openchuck-path` ran 607 s (simultaneous.rs 363 s, helical.rs 91 s, region 43 s, unroll 31 s, axial 25 s); `cargo test` runs a crate's test binaries one after another. Build and test already ran the same tests. The build part is 33 s; no third-party crate is rebuilt.
- **Gate 3 is real time.** 9 programs, 22.1 min, wall time equal to simulated motion time (35, 295, 278, 223, 40, 124, 159, 114 and 93 s). Pull of the image 28 s, build of `xtask` and the CLI about 23 s. `xtask replay` runs the programs one after another (`xtask/src/replay.rs`, `check`). It cannot go faster on one runner; the simulator also samples every millisecond and a loaded runner can show gaps (hence its one automatic repeat), so running programs side by side on one 4-vCPU runner is a risk.
- **Queue is occasional and large.** Median queue is under 1 min on every job, but in 4 of 12 code runs the critical job waited 7 to 17 min, and one run's queue sum was 207 minutes (the macOS legs: STEP kernel waited 14.8 min). Free hosted runners for a public repository allow about 20 concurrent jobs (5 on macOS), and a code run uses 18 to 31 jobs. When two or three lanes push together the cap binds. **Any option that adds jobs makes this worse unless it also removes some**; options below say which.
- **Cancellation.** `concurrency: group: ci-${{ github.ref }}, cancel-in-progress: true` cancelled 14 of the 30 runs listed for 16 Oct. Twelve are superseded pull request pushes (correct: the newer commit is checked). Two of the six `main` runs are **pushes to `main`** (runs 38897970237 and 38937724629, the second after 20 minutes) cancelled by the next merge, so `main` has no recorded result for those commits and the cache and coverage artifacts for them never appeared. The path filter compares a pull request with its base, not with the previous push, so a docs-only push to a code pull request starts a full run again: cancelling the in-flight run costs the whole elapsed time of the older run and saves nothing.

## Findings, ranked by minutes saved per unit of risk

| # | Finding | Saving on the critical path | Risk |
| --- | --- | --- | --- |
| F1 | Gate 3 runs 9 real-time programs serially on one runner | up to 13 min (22 to about 9) | low |
| F2 | The nextest run is 21 to 22 min on each OS and is test-bound, not compile-bound | 8 to 10 min per OS (to 11 to 13) | low to medium |
| F3 | Coverage repeats the tests instrumented; 25 min once F1 and F2 are done it is the longest job | 10 to 14 min (to about 11 to 15) | needs the owner (coverage content or place) |
| F4 | Each library alone re-runs `openchuck-path` serially (607 s) | 3 to 4 min on three runners; not critical path, but cuts 10 runner minutes per OS | low |
| F5 | `main` runs are cancelled by the next merge | no time saved; restores the record on `main` | none |
| F6 | Nine fuzz legs each install a nightly toolchain and cargo-fuzz and build from scratch | about 12 runner min and 6 slots; none on the critical path | low |
| F7 | Eight small jobs each start a runner for 0.2 to 2 min | slots only | low, but changes check names |
| F8 | Queue at peak (7 to 17 min on 4 of 12 runs) | up to 17 min on those runs | depends on plan or on cutting slots |
| F9 | Stale long-test list; CI times may include self-inflicted contention | unknown until measured | none to measure |

## Options

Each option is independent unless noted. Savings are estimates from the figures above, to be replaced by measurement in the implementing pull request (the implementation measures the effect of each change on the runs that follow it and reports it).

### Option 1: Do not cancel runs on `main`; keep cancelling superseded pull request pushes (F5)

Change the concurrency group so a push to `main` is keyed on the commit (`ci-${{ github.sha }}` for `push`, `ci-${{ github.ref }}` for `pull_request`) and `cancel-in-progress` is `${{ github.event_name == 'pull_request' }}`. `main` then always has a complete result per merge. **Impact:** results and artifacts for every merge; cost is the runner minutes of runs that today are cancelled (2 of 6 `main` runs on 16 Oct). **Risk:** none for correctness. **Saving:** no time, but removes a gap in the record. This is the "safer rule" for the cancellation that hit a lane's run: it is safe for `main`. For pull requests see Option 2.

### Option 2: What to do when a docs-only push arrives at a code pull request (F5, OI-221)

Cancelling is right when the new push changes code. When it changes only `docs/` the older run is still valid for the code, and the new run repeats it. Choices:

- **(a) Keep cancelling.** Simplest; frees slots; a docs push costs the elapsed time of the older run plus a full new run.
- **(b) Never cancel pull request runs.** Every push is fully checked; costs runner minutes and slots (the queue gets worse), and the older run's result is useless.
- **(c) Cancel, but let the new run reuse the job results of the previous run for the same code.** Not available in plain Actions; would need an artifact or cache keyed on the tree hash of the `rust` filter's paths, written by each job on success and read by a pre-job that marks the job skippable. A skipped job must then still be shown as success of an earlier identical tree. This is the only variant that saves real time, and it is the one with a vacuity risk, so it is not recommended without a design of its own.
- **(d) Process, not CI:** lanes push docs-only changes before code, or in the same push as the next code change. No change to the workflow.

Recommendation: (a) with Option 1, plus (d). Owner decision, OI-221.

### Option 3: Shard Gate 3 across runners, with a test of the shard assignment (F1)

Add `--shard i/n` to `cargo xtask replay` (partition the sorted default program list by index modulo `n`, or by a fixed weight table checked into `tools/linuxcnc`) and turn `gate3` into a matrix of `n` = 3 legs, each pulling the image and building the CLI. Estimated leg time with longest-first balancing of the measured motion times: about 7.5 to 8 min of replay plus about 1 min of pull and build, so about 9 min against 24 now. **Impact:** Gate 3 stops being the critical path. **Risk:** low. The partition is in the program, so a new golden file is picked up automatically by one shard (a workflow that lists files by hand would silently drop a new program; it must not). The xtask must fail if a shard is empty or if a program appears in two shards or in none (a unit test for the partition, and an `n`-way count check in the job). Separate runners also avoid the sampling-gap risk of running side by side on one runner. Cost: 2 more jobs per code pull request; the artifact name gets the shard number. The `gate3` job id stays and so does the aggregator's `needs`.

### Option 4: Shard the nextest run of Build and test on each OS (F2)

Make `build-test` a matrix of OS and shard (`--partition hash:k/2`, or `count:k/2`), each shard building the workspace (1 to 3 min) and running its share; clippy, doctests and the examples run in one shard only. Estimated: nextest phase 21 min becomes 11 to 13 min per shard, bounded below by the longest single test (8.0 min on Ubuntu, 7.6 on Windows), so Build and test finishes in about 13 to 16 min against 21 to 25. **Impact:** with Options 3 and 6 this takes the code run from 26 to 33 min to about 15 to 17. **Risk:** low to medium. (1) A shard running no tests passes: the job must assert that the shard ran at least one test and that the shard counts add up to the `cargo nextest list` count (a final step or a small aggregate job). (2) Doubled jobs: 3 more per run, 1 more on macOS, whose cap is 5 concurrent; the macOS legs would queue longer under load. Measure before applying to macOS (a single macOS shard is a valid first step; macOS is the fastest of the three). (3) `hash` partitioning is stable but not time-aware; the longest tests can land together. A fixed, time-aware split by test binary is possible but is a list to maintain. Measure two or three partitionings on one real run.

### Option 5: Use nextest in Each library alone (F4)

Replace the loop of `cargo test -p <crate>` with `cargo nextest run -p <crate>` plus `cargo test --doc -p <crate>` for the four library-family crates. Each crate is still built and tested on its own with its own feature set (the point of the job, architecture section 12), only the test binaries of a crate run in parallel instead of one after another. Estimated: `openchuck-path` 607 s to about 380 s (the 363 s simultaneous binary is the floor). **Impact:** 3 to 4 min per OS, about 10 runner minutes; this job is not on the critical path. **Risk:** low; test counts per crate before and after must match (record them in the pull request).

### Option 6: Coverage (F3, OI-223): the owner chooses among three moves

Coverage is the longest job once Options 3 and 4 are done, so one of these is needed for them to pay off.

- **(a) Bring `.config/nextest.toml` up to date** with the AGENTS.md rule (list every test that runs over about a minute, in both profiles). Excluding the ten or so tests of 60 s or more that are not yet listed would take the coverage run from about 25 min to an estimated 8 to 12 (uncertain: it depends on how the rest packs onto 4 cores). **Impact:** coverage numbers drop by whatever only those tests exercise; the tests still run in every pull request (Build and test) and in the nightly run. **Risk:** the coverage trend changes its baseline once. It follows the existing documented policy (verification plan section 8) and needs only the owner's agreement that the list is long, not that a test is dropped. The measured times are from hosted runners; the rule's "about a minute uninstrumented" would be re-measured on one runner first (OI-224).
- **(b) Keep coverage as is on pushes to `main` and nightly; on pull requests keep it but make it advisory about timing.** Not recommended: a required job that is not waited for is a different gate.
- **(c) Move the coverage trend to `main` only** (one of the moves the owner named as acceptable). Largest saving: the job leaves the pull request critical path altogether and about 25 runner minutes per run. Cost: coverage is not reported per pull request, which the verification plan describes as trend tracking, not a gate ("Trend and gap finding, not a target on its own").
- **(d) Shard the instrumented run** across two runners and merge the raw profile data in a final job before `cargo llvm-cov report`. Keeps coverage exactly as it is, about half the time, but depends on `llvm-cov` merging data from different runners built from the same sources (the build must be identical on both). Not recommended without a trial on a branch.

Recommendation: (a), then measure; (c) only if the owner prefers it.

### Option 7: Fewer, shared fuzz jobs (F6)

Run the nine targets in three jobs of three (each: one toolchain install, one `cargo-fuzz` install, one build with `rust-cache` for the `fuzz` workspace, then three one-minute runs), keeping the 1 minute per target of the verification plan (NFR-07). Estimated: wall clock about 6 min against 3 min, off the critical path; runner minutes about 25 down to about 13; slots 9 down to 3. **Risk:** low; the failure annotation and corpus seeding stay per target. Nine named results become three; the failing target is named in the annotation as now.

### Option 8: Fold the fast checks into one Linux job that runs beside the long jobs (F7; accepted, replaces the earlier "not recommended")

The owner asked for this to cut demand on runners: `rustfmt` (0.2 min), `cargo audit` (0.3), `cargo deny` (0.8), `cargo about` (1.2), the semver check (1.1) and the repository checks of `xtask` (1.9) share **one** Linux job, `Fast checks`, which has no `needs` beyond `changes` and so runs at the same time as the long jobs. Its length (about 6 to 8 minutes in one sequence, with one checkout, one toolchain and one cache) is well inside the 20 to 27 minutes of the jobs it runs beside. Other fast checks of about two minutes or less that are not OS-specific join it when found. Rules for the change:

- **Every check still runs wherever it runs today.** Each check keeps its own condition as a step condition (`rust` for `rustfmt`, `cargo audit`, `cargo deny`, `cargo about` and the semver check, which also needs the `pull_request` event; `rust` or `records` for the repository checks). The job runs when `rust` or `records` is true.
- **A failure names its check and hides no other.** Each check is its own step with a clear name, and every later step has `if: always()` plus its condition, so all run and the job fails at the end if any step failed. There is no `continue-on-error`.
- **A step cannot be skipped by mistake.** A final step reads the outcome of every check step and fails the job when a check whose condition was true did not succeed, so a wrong condition cannot pass vacuously.
- **OS-specific jobs stay apart:** UI smoke test, STEP kernel, Build and test, Each library alone.
- **`CI required checks` keeps its name and its rule.** Its `needs` list loses the six merged jobs and gains `fast-checks`; the job is skipped only when the path filter turns it off, as now.

### Option 9: Raise the runner limit (F8, OI-224)

The queue at peak is a plan limit, not a workflow flaw. GitHub's larger or paid runners, or a self-hosted pool, remove it or shorten the test phase (more vCPUs shorten the nextest run directly). It has a cost and, for self-hosted runners on a public repository, a security question, so it is the owner's decision, and nothing in this record depends on it. If the owner wants a figure, the implementing session can price it from the plan once told which plan the repository is on.

### Measurement to do first (OI-224, OI-225)

Two things the implementation needs and this session could not measure: (1) how much of the long tail is contention (run the ten longest tests alone on a 4-vCPU runner against inside the full run; set `threads-required` in nextest for the multi-threaded ones if it helps; this may shorten Build and test with no sharding at all); (2) the Actions cache list (sizes, ages, hits per branch) and the number of concurrent jobs at the queue peaks; needs a token that can read `actions/caches` and `actions/runs`.

## Owner's decisions (16 Oct 2026, relayed by the orchestrator session under decision 0049)

The owner said: "Accept 0052 recommendations. Can we consolidate fast running checks into a scenario where they only use a single runner in parallel with longer jobs. This would help with queuing as it will bring down demand on runners." That accepts the recommendation list below, in this order, and adds the consolidation (Option 8, now accepted):

| Order | Accepted | Pull request |
| --- | --- | --- |
| 1 | Option 8 (fast checks in one job), Option 1 (`main` never cancelled) and Option 5 (nextest in Each library alone): no risk, and the first removes slots | one pull request each |
| 2 | Option 7 (fuzz in three jobs) | one |
| 3 | Option 3 (Gate 3 in three shards, with the partition checks) | one |
| 4 | The OI-224 measurement, then Option 6(a) (the long-test list brought up to date; **the coverage baseline shifts once, and the owner agrees**) and Option 4 (nextest sharded, Linux and Windows first, macOS only if its queue allows) | one each |
| - | Option 2 as (a) plus (d): keep cancelling superseded pull request runs; lanes batch docs-only commits | no change to the workflow |
| - | **Not accepted, stays with the owner:** Option 6(c) (coverage on `main` only) and Option 9 (larger runners) | - |

### Runner slots: what each accepted option adds or removes

Counted on a code pull request that runs every job (31 jobs today, the figure of 16 Oct: `changes`, Gate 3, rustfmt, Build and test x3, UI smoke x3, STEP kernel x3, Each library alone x3, repository checks, cargo deny, cargo audit, cargo about, semver, fuzz x9, coverage, `CI required checks`).

| Option | Slots | Jobs after |
| --- | --- | --- |
| 8 Fast checks in one job (six jobs become one) | -5 | 26 |
| 1 `main` never cancelled | 0 (more `main` runs finish, so a little more total demand) | 26 |
| 5 nextest in Each library alone | 0 | 26 |
| 7 Fuzz in three jobs (nine become three) | -6 | 20 |
| 3 Gate 3 in three shards | +2 | 22 |
| 4 nextest sharded in two (Linux, Windows, macOS) | +3 (+2 without macOS) | 25 (24) |
| 6(a) Long-test list | 0 | 25 (24) |

Net: 31 to 25 jobs (24 if macOS is not sharded), 6 or 7 fewer slots than today, even with every shard. The macOS count is 4 jobs today (Build and test, UI smoke, Each library alone, STEP kernel) and 5 with a macOS shard; macOS has the lowest concurrency cap, so the macOS shard is the last step and is taken only if the measured macOS queue allows. The short jobs also leave the first minutes of a run: the 15 jobs that start together today (fast checks, fuzz) become 6.

## Recommendation for the owner

In this order, one pull request each, measuring after each:

1. **Option 1** (main never cancelled) and **Option 5** (nextest in libraries-alone). No decisions needed, no risk.
2. **Option 3** (Gate 3 in three shards, with the partition checks). The largest saving for the least risk.
3. **Measurement of OI-224**, then **Option 6(a)** (the long-test list; owner agrees the baseline shift) and **Option 4** (shard nextest, Linux and Windows first, then macOS if its queue allows).
4. **Option 7** (three fuzz jobs) to give back some of the slots that Options 3 and 4 use.
5. Hold **Option 2** at (a) plus (d); **Option 6(c)** and **Option 9** stay with the owner.

The owner accepted this list and added Option 8 at the front (see *Owner's decisions*).

Expected result for a code pull request once 1 to 4 are done: critical path of about 15 to 17 min (floor: the longest test, 8 min, plus build and setup) against 26 to 33 now, with about 5 more slots used by sharding (2 for Gate 3, up to 3 for Build and test) and 6 fewer by the fuzz change. These are estimates; the first runs after each change give the real numbers, which the implementing pull requests report.

## What I would not change

- **The `CI required checks` job, its name and its rule** (`changes` must succeed; skipped is accepted only by a path filter or event). Nothing here edits it.
- **Path-filtered `docs/` skipping.** It works: a docs-only run is 3 jobs and 2 runner minutes (runs 38938750427, 38939895430).
- **The pull request run of any gate**, including the 1 minute of fuzzing per target, the 3-platform matrix, the STEP kernel jobs, the semver check, `cargo deny` and `cargo audit`.
- **The run on `main` for every merge.** It repeats what the pull request ran, but it seeds the caches `main` provides, produces the coverage trend, and is the only record of the merged tree.
- **Caching and compile.** Every rust-cache restore was an exact hit; compile is 5 to 10 % of the big jobs. sccache, a shared target directory or a larger cache would save seconds and add a failure mode.
- **Optimisation levels.** The hot crates are already `opt-level = 3` in dev (decision 0025); raising the rest would add compile time for no measured test gain.
- **The 10-minute rule.** Nothing is over it; I do not propose moving a test to the nightly run only. All the long tests keep running in every pull request.
- **Parallel Gate 3 programs on one runner.** The simulator is real time and sensitive to load; use separate runners (Option 3).
- **The cold OpenCASCADE builds** (23 to 71 min): decision 0039 handles them, and their cost showed once (one cold macOS leg in 14 runs).

## Consequences

- This record changes no workflow, test or document other than the registers. The workflow changes follow as pull requests.
- `ci.yml` (and `xtask/src/replay.rs` and `.config/nextest.toml` for the matching options) change on `claude/ci-speed-up-implementation`, one pull request per option. The verification plan section 8 text on tests over a minute is unchanged; Option 6(a) brings `.config/nextest.toml` into line with it.
- The open items below track the choices.

## Evidence added while implementing (16 Oct 2026, evening)

**Queue wait on the day, to judge Option 9 (larger runners).** From the Actions API, the `CI` runs created on 16 Oct 2026 (73 runs). For each run, the longest time any job waited between creation and start:

| Window (UTC) | Runs with a full job set | Median of the longest waits | Longest wait | Queue minutes, all jobs |
| --- | --- | --- | --- | --- |
| 08:00 to 16:00 | 5 | 1.4 min | 2.0 min | 24 |
| 16:00 to 18:30 | 18 | 1.9 min | 11.8 min | 553 |
| 18:30 to 19:35 | 13 | 7.1 min | 42.8 min | 2,331 |

At 19:35 eight pull requests were open, all with CI running or queued, and four runs were waiting for a runner (one since 18:49). A code run uses 17 to 31 jobs. The free hosted runners of a public repository allow about 20 at once (5 on macOS), so two or three lanes pushing together exceed the cap. Overnight (00:00 to 08:00) with five lanes the median longest wait was 8.3 min and the longest 17.7 min. This supports taking slots out first (Options 8 and 7) before adding any shard, and it is the evidence for the owner's decision on Option 9.

**A run is not always slow because of its code.** The Coverage trend job of pull request 64 took 69.4 min (18:32 to 19:41) against 23 to 28 min on every other run of the day, with the same tests; it finished green. No log was available while it ran, so the cause is not known (a slow or shared runner is the likely one). A required job's time therefore varies by a factor of 2.5 on the same code. Any timing in this record is one sample.

**Option 5 gave almost nothing.** Pull request 68 ran the library crates under nextest: the same test counts (40, 189, 24 and 14), but the step took 10.3 min on Ubuntu against 10.8 before, 7.0 against 9.5 on Windows and 10.8 against 7.5 on macOS. The expectation in the Option 5 text above (607 s down to about 380 s) was wrong: `cargo test` already runs the tests of one binary on every core, so the limit is the crate's total CPU work, not its longest binary. Option 5 is harmless and is not counted as a saving.

**The fast-checks job** (Option 8, pull request 64) ran in 6.6 min with every check passing. By step: `cargo xtask trace` 175 s, semver check 59 s, `cargo about` 54 s, `cargo deny` 32 s, repository checks `deps` 14 s, `cargo audit` 6 s, rustfmt 1 s. `xtask trace` compiles every test target of the workspace to list the tests; a cheaper listing is a candidate for a later option (it is the largest single step, on a job that is not on the critical path). A deliberate-failure run on a throwaway pull request (run 38957436177) showed the consolidation behaves as required: the `xtask records` step failed, the other repository checks still ran, conditionally skipped steps were skipped, the last step named the failure, `Fast checks` failed, and `CI required checks` failed with `Required jobs did not pass: fast-checks (failure)`.

## Open items

| # | Item | Needed from |
| --- | --- | --- |
| OI-221 | What a pull request run does when a docs-only push arrives (Option 2) and whether `main` runs are never cancelled (Option 1). | Decided: Option 2 as (a) plus (d); Option 1 accepted |
| OI-222 | Whether Gate 3 (Option 3) and the nextest run (Option 4) may be sharded, and the shard counts; macOS shards only if its queue allows. | Decided: Gate 3 in three shards; nextest in two, Linux and Windows first, macOS if its queue allows |
| OI-223 | Coverage: update the long-test list (Option 6a), move the trend to `main` (6c), or shard the instrumented run (6d). | Decided: 6(a), the baseline shift agreed; 6(c) not accepted; 6(d) not wanted |
| OI-224 | Measure the long tests alone against in the full run (contention), re-measure the long-test list on a hosted runner, and decide whether to pay for larger runners (Option 9). | Measurement accepted as the first step of order 4; larger runners not accepted |
| OI-225 | The Actions cache and concurrency data this session could not read (cache sizes and ages, concurrent jobs at the peaks); needs a token that reads `actions/caches` and `actions/runs`. The queue data and the 69-minute Coverage run are in *Evidence added while implementing*; the cause of that run is still unknown. | Owner or orchestrator; still open |
