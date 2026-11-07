# 0534: The nightly STEP mutation run gets a 55-minute budget, and the settings file and facing rows get fuzz targets

- **Status:** accepted by the project owner on 7 Nov 2026, relayed by the coordinator (decision 0049). Proposed by lane FUZZGAPS (wave 2, release blockers) on 6 Nov 2026; item 1 (that the mutation run counts as fuzzing) is the owner's decision of 6 Nov 2026, also relayed by the coordinator. OI-2086 stays open; it is not a release blocker.
- **Date:** 6 Nov 2026.
- **Amends:** [decision 0107](0107-malformed-input-sweep-and-fuzz-hours.md) item 4 (the STEP mutation run: 3,000 mutants a night becomes a time budget) and closes its open items OI-326 and OI-327.
- **Relates to:** NFR-07; verification plan section 4.1; [docs/fuzzing.md](../fuzzing.md); [docs/ci-performance.md](../ci-performance.md) and [decision 0516](0516-ci-architecture.md) (the runner budget); decisions [0003](0003-step-kernel-route.md) and [0027](0027-slice-11a-step-import.md) (the STEP mutation run).
- **Open items:** OI-326 and OI-327 close here. OI-2086 is new.
- **Numbering:** decision 0534 and OI-2086 to OI-2089 were assigned to this lane by the coordinator.

## Context

Decision 0107 left three things for the owner before the first release. The STEP mutation run imports 3,000 mutants a night (about 3.5 minutes), so it cannot reach the advisory 24 CPU hours of verification plan section 4.1, and it was not settled whether a mutation run through the importer counts as fuzzing for NFR-07 (OI-326). The export acknowledgement file, `settings.toml`, had no fuzz target (OI-327). A search of the code for readers of user-editable text found one more with no target (the face rows of a facing operation) and two app-data readers (the checkpoint index and the recovery `state.toml`).

State of the nightly run (Actions API, runs 23 to 29, 1 to 6 Nov 2026): runs 25 to 29 are fully green (11 fuzz jobs, STEP mutation, AddressSanitizer, long tests, three beta jobs); run 23 was the last red (AddressSanitizer in the import tests, STEP mutation at the OpenCASCADE set-up), both fixed by decision 0490. Issues #369 and #451 are closed by passing runs.

## Decision

1. **The mutation run counts.** A mutation run through the STEP importer counts as fuzzing for NFR-07 and towards the advisory 24 cumulative CPU hours (the owner, 6 Nov 2026, option A). The release report names it "mutation run", not a libFuzzer target, so the method stays visible. It is unguided (no coverage feedback) and works from small edits of four valid files; that is the reason for the name.
2. **A time budget for the nightly mutation run (OI-326).** `step_mutations` takes an optional third argument, `max_seconds`: it stops at the count or at that time, whichever comes first, and reports the mutants it imported. The nightly job runs it with a count that is never reached and 3,300 seconds, so the job takes about an hour. The seed stays the run number (decision 0107 item 4), so each night imports different mutants. The annotation `Fuzz CPU time step_mutations` reports the CPU seconds and the mutants imported. At about 55 minutes a night the run reaches 24 hours in about 26 nights. Cost: the job goes from about 5 to about 60 runner minutes, +55 a night (decision 0516 budget: the nightly already starts all its jobs at once, so the peak is unchanged and no pull request job is touched).
3. **`settings_toml` target (OI-327).** One target covers both readers of `settings.toml`: the export acknowledgements (GC-16) through `Acknowledgements::lookup_text` and the `[interface]` table (UI-15) through `SettingsFile::load_text`. Both are new plain `pub fn` on text with no file access, which the file readers now call. The target checks that damaged text always means "ask again" and that what is read is already in range (clamping it again changes nothing). `openchuck-app` and `openchuck-path` join the fuzz workspace as path dependencies; the third-party crates they bring into `fuzz/Cargo.lock` (rayon, sha2 and their dependencies) are already in the main lock, so no new dependency or licence enters the project (NFR-10, NFR-14). It runs nightly for an hour and in smoke group C.
4. **`facing_rows` target.** `openchuck_path::facing::selection::decode_rows` reads JSON kept in a project parameter. The target checks that rows that decode encode and decode again to the same rows. It runs nightly for an hour and in smoke group A (five targets, five minutes).
5. **Left open (OI-2086).** The checkpoint index and the recovery `state.toml` have no target. They are written by the application, read with strict serde structs and size caps, and an unreadable file is reported and kept. A target needs a pure parse function for each. The owner decides whether it is needed for the release.
6. **Seeds.** `fuzz/seeds/settings_toml/` (an application-written file, one with values of the wrong type, one damaged) and `fuzz/seeds/facing_rows/` are copied into the corpus each night and in smoke, like `project_recovery`.

## Alternatives considered

- **Count the mutation run as NFR-07 evidence only, not towards the 24 hours (option B).** A fair fallback if hours were to mean coverage-guided only; the owner chose A.
- **A mutant count instead of a time (for example 50,000).** The rate depends on the runner and on how many mutants make the worker crash or time out, so the hour would drift; a time budget fixes the cost.
- **A separate settings target for each table.** The two tables share one file and one parse, and a damaged file affects both; one target reaches both.
- **Read the files from a temporary folder in the fuzz target.** Slow and leaves files behind (decision 0107 made the same choice for the project folder).

## Consequences

- **Code:** `Acknowledgements::lookup_text` and `SettingsFile::load_text` (new, `openchuck-app`; the file readers call them, with the same messages); `step_mutations` gains `max_seconds`. No library-family API change, no project schema change.
- **CI:** `nightly.yml`: two targets in the fuzz matrix with seed steps, and the STEP mutation step with the budget and the mutants count in the annotation. `ci.yml`: the targets in smoke groups A and C with seed steps. +2 runner hours a night for the two new targets and +55 minutes for the STEP job, about +3 runner hours on a night of about 17. The smoke job's critical path grows by about one minute in groups A and C.
- **Documents:** verification plan section 4.1 (fuzzing row), [docs/fuzzing.md](../fuzzing.md) (inventory, CPU table).
- **Check after the first night:** the first nightly run after the merge is the test of the budget (GitHub runs scheduled workflows from the default branch). Its annotation gives the mutants per hour; if the job nears its 150-minute limit or the rate is far from the estimate, the budget is a one-line change.
