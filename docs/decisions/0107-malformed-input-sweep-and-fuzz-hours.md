# 0107: Malformed-input sweep: every user file has a fuzz target, and the fuzz CPU hours are counted per parser

- **Status:** proposed (lane FZ, 18 Oct 2026). Only the project owner accepts a record (decision 0049).
- **Date:** 18 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-326 to OI-329 are new.
- **Relates to:** work breakdown slice 20 item 4; NFR-07; verification plan sections 4.1 ("at least 24 cumulative CPU hours per parser with no unresolved crash for release") and 8; decisions [0003](0003-step-kernel-route.md) and [0027](0027-slice-11a-step-import.md) (the STEP mutation run, OI-27), [0052](0052-ci-speed-up.md) (fuzz smoke groups) and [0102](0102-coverage-off-pull-requests.md) (nightly issues; a fuzz issue is closed by a pull request).

## Context

Slice 20 item 4 asks for a malformed-input sweep of every importer, the project loader and every other user-supplied file format, with at least 24 cumulative fuzz CPU hours per parser and no unresolved crash. The inventory in [docs/fuzzing.md](../fuzzing.md) found, on `main` at 22a864e:

- Nine libFuzzer targets in `fuzz/`, all in the pull request smoke job, but only seven in the nightly job: the G-code reader and the post definition targets never ran for an hour.
- The project loader was fuzzed one file at a time (`project_files`, through `file::parse_file`). The checks that need the whole folder (references between items, order keys, file names, the selection-set checks done only by `load`) were not fuzzed.
- The post definition target loaded the definition but never rendered with it, although its comment says it does; the render path (the template engine with its fuel and recursion limits) was not fuzzed.
- No corpus was kept between nights: every night started again from the seeds, and `project_files`, `machine_profile_json` and `toolpath_ir_json` had no seeds at all.
- The nightly STEP mutation run used the default seed every night, so each night imported the same 3,000 mutants (about 3.5 minutes): repeated nights added nothing.
- Nothing recorded the CPU time a target got, so the 24 hours could only be estimated from job durations, and job logs expire after 90 days.
- The one fuzz failure so far (`tool_library_toml`, 13 Oct 2026, run 38376224534: a multi-line string with a line starting `|||||||` read as a merge conflict) was fixed on 13 Oct in 070e7ee; its input no longer fails (checked again on 18 Oct).

## Decision

1. **Every user-supplied file format has a fuzz target or a mutation run,** listed in the inventory in [docs/fuzzing.md](../fuzzing.md) with its entry point, CI jobs and corpus. Two targets change and one is new:
   - `project_folder` (new) loads a whole project folder from memory through `openchuck_project::file::load_files`, the same checks as `file::load` (the loader now reads every text first and then checks them, so the folder and the in-memory form share one code path). The input is the folder's files one after another, each after a line `=== <path>`; the corpus is the fixture folders under `crates/openchuck-project/tests/fixtures/` written in that form.
   - `post_definition` now renders a small toolpath (every kind of move, A beyond one turn) with every definition that loads.
   - `project_files` is seeded from the fixture files, each prefixed with the byte that picks its kind.
2. **Every target runs nightly for an hour,** ten targets in all; `project_folder` joins smoke group A (four targets, four minutes). Both jobs pass `-timeout=10`, so one input that takes more than 10 seconds is a failure, as a hang must be (NFR-07).
3. **The corpus is kept between nights** in the Actions cache (`fuzz-corpus-<target>-<run id>`, restored by prefix, saved by runs on `main` only), so the hours add up on what earlier nights found. The seeds are copied in again each night.
4. **The STEP mutation run takes the run number as its seed,** so each night imports different mutants.
5. **How the 24 hours are counted.**
   - A night counts the CPU time (user plus system, bash `time`) of the `cargo fuzz run` step of that target's nightly job on `main`, after the target was built in a step of its own so the build is not counted. libFuzzer runs one process, so an hour of the step is one CPU hour. For STEP it is the mutation step, the worker processes included.
   - Each nightly job writes it in an annotation (`Fuzz CPU time <target>`: CPU seconds, executions, the result and the commit) and in the job summary. Annotations stay with the check run after its logs expire.
   - The running total is the table in [docs/fuzzing.md](../fuzzing.md). A session with the Actions API updates it: at each slice close, before a release, and at least every 60 days (the job logs, the fallback source for nights without the annotation, are kept for 90 days). Pull request smoke minutes and local runs are not counted.
   - A night that fails counts zero, and the crash it found is unresolved until a pull request fixes it with a regression test that names NFR-07 (decision 0102 item 5). Hours from before a fix still count: the plan asks for cumulative hours. A rewrite of a parser is the owner's call to reset.
   - Nights before this change, which had no annotation, are counted from the job durations: 60 minutes for each successful fuzz step (the step's other minute or so was the build), and the STEP mutation nights at their step time once only, since the seed was the same each night.
6. **The totals on 18 Oct 2026** (six scheduled nights, 12 to 17 Oct): `machine_profile_json`, `machine_profile_toml`, `toolpath_ir_json`, `project_files` and `stl_import` 6 h each; `tool_profile_toml` 5 h; `tool_library_toml` 4 h; `gcode_reader`, `post_definition` and `project_folder` 0 h; STEP mutations 0.06 h of distinct mutants (three runs of the same 3,000). At one hour a night, the libFuzzer targets reach 24 hours between 18 and 24 nights after this merges. The STEP run does not reach it at 3,000 mutants a night (OI-326).

## Alternatives considered

- **Count the job durations from the Actions API only, with no annotation.** It works for now (it is how the totals above were found), but logs and job step data expire, and the duration includes the build. The annotation costs one line.
- **Keep the running total in a file that the nightly job commits.** Rejected: the nightly jobs hold a read-only token by design (decision 0102), and a bot commit each night would add noise to `main`.
- **Compute the total in `xtask` from the GitHub API.** Rejected: nothing in the workspace may use a network client (architecture section 4), and lane sessions have no artifact downloads.
- **Fuzz the project folder through a temporary folder on disk.** Rejected: writing files for every input makes each run much slower and leaves files behind; the in-memory form runs the same checks.
- **Run libFuzzer with `-fork=4` to get four CPU hours from each hour on a 4-vCPU runner.** Not done here: it changes how the plan's "1 hour nightly" reads and how crashes stop the run, so it is the owner's choice (OI-328).

## Consequences

- **Code:** `openchuck-project` gains `file::load_files`; `file::load` reads every file and then checks, so an unreadable file is reported before a parse error in an earlier file (the messages are unchanged). A minor API addition, not a breaking change.
- **CI:** `nightly.yml` gains three targets (three more runner hours a night), the corpus cache, a separate build step and the CPU annotation; the STEP mutation step takes the run number as its seed and builds its program in the build step. `ci.yml` gains the `project_folder` target and two seed steps. Neither workflow's new parts can run before the merge (GitHub runs scheduled workflows from the default branch): the first night is the test (OI-329).
- **Documents:** [docs/fuzzing.md](../fuzzing.md) is new; the work breakdown's slice 20 item 4 and its "Parallel work" table name lane FZ. The verification plan is unchanged: its row for fuzzing already gives the limits, and this record says how they are measured.
- **Slice 20 review:** this is evidence toward the release checks that fuzzing ran for a set period on each parser with no unresolved crash (verification plan section 8) and that no severity 1 or 2 defect is open. It does not meet them yet: the hours are short of 24 for every parser, and three gaps are open (OI-326, OI-327, OI-328).

## Open items

- OI-326: the STEP mutation run reaches 24 CPU hours only with a time budget: about an hour a night (for example a mutant count of 50,000, or a run until a time) would take 24 nights. The owner chooses the budget and whether it counts as fuzzing for NFR-07.
- OI-327: the export acknowledgement file (`settings.toml` in the configuration folder) has no fuzz target. It is read with the `toml` crate and looked up through optional values only, and a corpus test covers a damaged file (`req_gc_16_a_bad_settings_file_asks_again_with_a_message_and_is_kept_and_other_keys_survive`). A target would need `openchuck-app` in the fuzz workspace and a pure entry point for the read. The owner decides whether it is needed for the release.
- OI-328: whether the nightly fuzz job uses all the runner's cores (`-fork`), which brings the 24 hours in about 6 nights instead of 24.
- OI-329: the first nightly run after the merge: the three new targets, the corpus cache, the annotations and the seeded STEP run; then the first update of the totals from the annotations.
