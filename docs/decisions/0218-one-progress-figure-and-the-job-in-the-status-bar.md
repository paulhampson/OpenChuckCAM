# 0218: One progress figure for the whole job, and the job in the status bar opens the Jobs tab (lane MSGPROG; issue 201)

- **Status:** accepted by the project owner (22 Oct 2026), given in the lane's own session (decision 0049). Item 4 is the owner's option C, relayed by the coordinator; the owner accepted items 1 to 3 as written.
- **What it amends:** [decision 0195](0195-axial-finishing-speed-and-steady-progress.md) item 3 (a run's progress counted operations) and the progress wording of [decision 0190](0190-cached-stock-per-operation.md) item 5 ("Simulating… 1 of 2" becomes "Simulating… 42 %"; the operation text after it is unchanged). It adds to specification section 10 (the footer) that choosing the job opens the Jobs tab. No requirement changes; it serves UI-09 and NFR-04.
- **Date:** 22 Oct 2026.
- **Open items:** OI-843 (a pass that failed was counted as done) is recorded and closed by this change. OI-890 (the shares are equal per operation; the owner checks on the reference laptop that the figure moves enough) is new.

## Context

Trial feedback F-45 (issue 201): while toolpaths were generated the progress bar filled and then started again at the next stage. The owner asked for one percentage across all stages that never goes back, with the operation being worked on beside it in the status bar ("Roughing: 42 %"), cancel at every stage, and, for several operations, the overall percentage with the name of the one being worked on. The owner's measured pain: long generate runs look frozen (axial finishing took minutes on an Intel Core i5-8265U), so the figure must move with real work.

Before this change (decision 0195 item 3) the pipeline passed each operation's own reports on as the run's count of operations ("2 of 3"), so the total no longer changed mid-run, but the count stood still for the whole of a long operation, and the comparison with the part after the toolpaths was a second count of setups that started again at 0. Adding a model showed the reader's stages one after another, each with its own count or none.

Requirements and records read: UI-09 and INT-04 (progress and cancel), NFR-04 (never block), GC-07 and NFR-06 (progress must not change output), UI-12; decisions 0117 (the job log), 0190, 0195, and the specification's sections 9 (Jobs) and 10 (Footer). Lane SELSET's decision 0215 (in flight) keeps Messages and Jobs in the dock; this change adds no tab.

## Decision

1. **One figure for the whole job.** A job that does several kinds of work reports one share of all of it, out of one total that does not change from its first report to its last (`RUN_PROGRESS_TOTAL`, tenths of a per cent), and never goes back: a report that would move it back is dropped, and reports from several threads leave in order. The pipeline divides a run into equal shares, one per operation and, when the run simulates, one per setup compared with the part. Adding a model is one share.
2. **The work inside a share moves it.** The strategies and the stock simulation report their own stages, each with its own count, and how many stages an operation has is not known beforehand. Within a share, each new stage takes half of what is left: the first stage the first half, the next the next quarter, and so on, its own count moving the figure across its part; a stage with no count stays at its start. An operation that is generated and then run through the stock simulation gives 70 % of its share to the toolpath and 30 % to the simulation; one taken from the cache that is simulated again gives all of it to the simulation; one taken from the cache with its stock jumps to the end of its share. The figure is a rough estimate, but it moves with the work and reaches the end of a share only when that work is done. The percentage shown is rounded down, so 100 % means done.
3. **A pass that fails is not counted as done (OI-843).** The passes of axial and indexed finishing count a pass only when it was fitted, so a run that ends with an error never reads "n of n" first.
4. **The status bar shows the current job, and choosing it opens the Jobs tab (the owner's option C).** The status bar says the current job, its one percentage and the operation it works on, for example "Generating… 42 %: Generating 'Roughing'" or "Simulating… 61 %: Re-running the simulation for 'Roughing'" (decision 0190 item 5's text, with the percentage in place of the counts); when other jobs run as well it adds "(and 1 more job)". Choosing it opens the Jobs tab, opening the dock if it was collapsed: every running job with its progress bar (the same figure), elapsed time and Cancel, the generating job with the operation it works on, and the finished jobs with their outcome. Cancel stays in the footer as well, and works at every stage (the owner's answer 3).

## Alternatives considered

- **Weights from each operation's expected cost** (its strategy, the part's size, the number of passes): closer to time, but the cost is not known before the strategy runs, and a wrong weight still has to be corrected without going back. Equal shares with the halving rule are simple and always honest about direction; OI-890 asks the owner whether they are good enough.
- **Time-based estimates** (time remaining from the speed so far): they change with the machine and the load, and can move back; not chosen.
- **Strategies declaring their stages up front**: a change to every strategy and to the library API for a rough figure; not needed for a figure that only has to move forward.
- **Option A or B of the issue (the status bar only, or the Jobs tab only)**: the owner chose C.

## Consequences

- The run's progress events change meaning: `done` of `RUN_PROGRESS_TOTAL`, under the stage names `STAGE_GENERATE` and `STAGE_SIMULATE`, in place of a count of operations or setups. The operation tree's progress bar (decision 0117) reads the same figure. Nothing reads the counts as operations; the `operation_times` example still counts how often the total changes (now never).
- Fewer progress events: a report that does not move the figure by a tenth of a per cent is not sent.
- Output is unchanged (GC-07, NFR-06): the progress is only reported.
- Changelog: `openchuck-app` gains `RUN_PROGRESS_TOTAL` and `JobEntry::percent`; `openchuck-path`'s pass count change is internal. Neither changes the library family's API, so the version is not bumped (decision 0048).

## Tests

- `jobs::tests::req_ui_09_a_share_moves_forward_through_stages_it_cannot_count_and_never_back`, `req_ui_09_reports_from_many_threads_leave_in_order` and `req_ui_09_a_job_reads_100_per_cent_only_when_done` (`crates/openchuck-app/src/jobs.rs`).
- `pipeline::tests::req_ui_09_the_progress_is_one_share_of_the_whole_run_that_never_goes_back` (replaces decision 0195's `req_ui_09_the_progress_counts_operations_with_one_total_that_only_goes_up`; it failed before the change) and `req_ui_09_a_run_can_be_cancelled_in_each_stage`.
- `req_ui_09_adding_a_model_reports_one_share_that_never_goes_back` (`crates/openchuck-app/tests/import_preview.rs`).
- `req_ui_09_a_pass_that_fails_is_not_counted_as_done` (`crates/openchuck-path/tests/axial.rs`): the last pass is cancelled from inside on one thread; it failed before the change ("32 of 32").
- `shell::frame_tests::req_ui_09_the_job_in_the_status_bar_opens_the_jobs_tab` and `shell::frame::tests::req_ui_09_the_status_bar_says_when_more_jobs_run` (`apps/openchuck-ui`).
