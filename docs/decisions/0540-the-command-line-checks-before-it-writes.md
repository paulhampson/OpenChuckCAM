# 0540: The command line checks before it writes (lane MSGREVIEW; issue #567)

- **Status:** accepted by the project owner on 7 Nov 2026, relayed by the coordinator (decision 0049), as written. Proposed by lane MSGREVIEW on 7 Nov 2026. It records the owner's answer to question 6 of [decision 0536](0536-message-levels-operation-reference-and-setting-links.md) (7 Nov 2026, relayed by the coordinator, option A). It changes what the command line does, so it is a record of its own.
- **What it amends, if accepted:** the command line's `regenerate --out` (`apps/openchuck-cli`). The requirements are unchanged; this brings the command line into line with GC-15, as the export dialog already is.
- **Date:** 7 Nov 2026.
- **Open items:** OI-2096 (raised with 0536) closes when this is built.

## Context

`openchuck-cli regenerate --out <folder>` (`apps/openchuck-cli/src/main.rs:634-702`) runs in this order:

1. It regenerates.
2. It refuses a partial path unless `--acknowledge-partial-path` is given (decision 0239 item 10).
3. It writes the G-code files.
4. It prints the replay and G-code check results.
5. It prints the verify notice.
6. It exits with an error if a file failed its checks.

The files stay in the folder after that error.

The export dialog does not write in that case. A G-code file that failed its own checks is a blocking check (GC-15; decision 0071 item 4; `crates/openchuck-app/src/export.rs`, `export_checks_with`).

The command line also prints none of the export dialog's warnings:

- an operation not simulated (GC-15);
- machine values not confirmed on the machine;
- the stock simulation skipped (decision 0168).

A file left on disk after "failed the checks above" is easy to run by mistake (NFR-12).

## Decision (proposed)

1. **One set of checks.** Before it writes, `regenerate --out` asks the application for the same checks as the export dialog (`Application::export_checks`, `crates/openchuck-app/src/lib.rs:1933`), so the two cannot drift.
2. **A blocking check writes nothing.** If any check blocks, the command line writes no file, prints each blocking check as `error:`, and exits with an error. It does not remove files already in the folder from an earlier run.
3. **Warnings are printed** as `warning:`, in the form of decision 0536 section 3, before writing. This covers an operation not simulated, machine values not confirmed, and the stock simulation skipped.

   GC-16 says the command line runs unattended and asks for no confirmation. So the stock-simulation-skipped warning is printed but needs no flag, as today. The partial-path flag stays as it is (decision 0239 item 10). If decision 0538 is accepted, its flag for a possible collision is added beside it.
4. **The order becomes:**
   1. regenerate;
   2. check;
   3. print the checks;
   4. refuse if a check blocks, or if a needed flag is missing;
   5. write;
   6. print the verify notice.

   The notice is still printed at every run that generates G-code (NFR-12, GC-16), including one that writes nothing.
5. **Without `--out`** nothing is written, as today. The checks are printed the same way, so a dry run shows what a write would do.

## Test plan

Each test is named after its requirement, written first and watched fail.

- `req_gc_15_cli_writes_nothing_when_a_check_blocks`:
  - a run whose G-code fails its replay check writes no file and exits with an error;
  - the folder holds no new file;
  - the error names the file and the check.
- `req_gc_15_cli_prints_the_export_warnings`:
  - a project with an operation not simulated prints that warning as `warning:`;
  - a project that skips the stock simulation prints its warning;
  - an unconfirmed machine value prints its warning;
  - each run still writes.
- `req_gc_16_cli_prints_the_notice_when_it_writes_nothing`.
- `req_gc_07_cli_and_export_write_the_same_files`: for a clean run the files are byte-identical to the export dialog's.
- The partial-path tests of decision 0239 still pass unchanged.

## Alternatives considered

- **Keep writing, and add a flag to keep files from a failed check** (question 6, option B). Not chosen: the default would still leave a file that failed its checks.
- **Leave it as it is** (option C). Not chosen.

## Consequences

- A script that relied on the files being written after a failed check now gets none. That is the point of the change.
- The command line's help text for `regenerate` says that a blocking check writes nothing.
- No change to the project file or to the G-code of a run that passes its checks.
