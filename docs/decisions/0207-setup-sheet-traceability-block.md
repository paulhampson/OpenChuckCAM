# 0207: The setup sheet traceability block is built (lane TB; slice 19 item 6)

- **Status:** accepted (21 Oct 2026, by the project owner directly in the lane TB session: "I accept decision 0207"). The owner's choice of UTC for OI-810 was relayed by the coordinator the same day.
- **What it builds:** [decision 0110](0110-setup-sheet-traceability.md) (accepted 17 Oct 2026), which this record does not change. [Decision 0189](0189-setup-sheet-written-with-the-g-code.md) (Write G-code writes the sheet from the same run) and [decision 0028](0028-slice-19-output-options-and-setup-sheet.md) item 4 are unchanged.
- **Date:** 21 Oct 2026.
- **Open items:** OI-538 (closed by this change), OI-691 (closed by this change), OI-810 (closed: the owner chose UTC, relayed by the coordinator, 21 Oct 2026).

Numbering: the coordinator assigned 0207 and OI-810 to OI-819; `main` and the unmerged branches end at 0191.

## Context

GC-12 as amended by decision 0110 asks for a block that ties a printed sheet to the files it came from. Nothing built it (OI-538), so GC-12 had no test for the amended wording and slice 19 could not close. Requirements: GC-12, GC-07 and NFR-06 (the G-code stays byte-identical), PRJ-01 (nothing new in the project files), NFR-07 (no crash on a missing file), NFR-12.

## Decision

1. **A "Traceability" section is the last section of the sheet**, in the HTML and the PDF, with five lines: *Generated*, *Project last saved*, *Software version*, *Project fingerprint* and *Version control*. It follows the Notes, where a printed copy ends. The Operations table already carries the spindle speed column; lane BUG-SPINDLE adds the operation's spindle speed and direction there, so this change touches no operation column.
2. **Generated** is ISO 8601 to the second in UTC with the `Z` zone designator (`2026-10-21T09:30:05Z`). It is read from an injected `Clock` (`SystemClock` at export, `FixedClock` in tests), held by `Application` and replaced with `Application::set_clock`. UTC is the same on every computer and needs no time-zone library (OI-810, the owner's choice A).
3. **Project last saved** is the newest modification time of the project's files on disk. The files are the ones the project was last opened or saved as (`file::render` of that state), so nothing is stored in the project (PRJ-01). If the project has changed since, the line says "(changes made since are not saved)". When any of those files is missing the line reads "not saved yet".
4. **Project fingerprint** is `blake3:` and 64 hex digits, the hash of each saved file's relative path, length and bytes in path order, read from disk. It reads "not saved yet" with the saved time.
5. **Software version** is the workspace version (`CARGO_PKG_VERSION`). **Version control** reads "not tracked" until `openchuck-vcs` exists.
6. **Where it is built:** `Application::traceability()` reads the clock and the files; `SetupSheet::with_traceability` appends the section; `Application::setup_sheet` calls both, so the dialog's Write G-code (decision 0189), `write_setup_sheet` and the command line all carry the block. `SetupSheet::new` is unchanged.
7. **The golden snapshot** is made with fixed text for the block (the project's random item IDs are in the fingerprint and the file time is the machine's); the live block is tested for its five lines, the injected clock, the mtime, the fingerprint following the saved files, "not saved yet" and byte-identical output with a fixed clock.
8. **Slice 19 is closed** in `xtask/traceability.toml` (`closed_slices`) with this change, as the owner decided on 20 Oct 2026 (build item 6, then close); the work breakdown is updated to match. The records of the slice's other lanes stay as they are.
9. **OI-691 (the xtask check).** `trace.rs` recognised a file-level feature gate only in the form `#![cfg(feature = "`. It now matches the gate in any spacing, so a gated file's tests still count. Closed.

## Alternatives considered

- **Local time with an offset.** Closer to what a shop reads, but the standard library cannot give the offset without a time-zone dependency or unsafe code, and a sheet would differ between computers. Not chosen (OI-810, the owner chose UTC).
- **Store the saved time in the project.** Rejected by decision 0110 (PRJ-01).
- **Fingerprint the in-memory project.** It would also change with unsaved edits and could not be matched to a file; the saved files are what a printed sheet is matched to.

## Consequences

- Work breakdown slice 19 status, item 6 and its checklist; `xtask/traceability.toml`; the register (OI-538 and OI-691 closed, OI-810 open); the decision index.
- `openchuck-app` gains `Clock`, `SystemClock`, `FixedClock`, `Traceability`, `iso8601_utc`, `Application::set_clock` and `Application::traceability`. It is an application crate, so `cargo-semver-checks` (library family) is unaffected.
- The two setup-sheet golden files gain the block.

## Tests

`crates/openchuck-app/tests/output_options.rs`: `req_gc_12_the_sheet_ends_with_a_traceability_block_of_five_labelled_lines`, `req_gc_12_req_nfr_08_the_traceability_block_is_the_same_every_run_with_a_fixed_clock`, `req_gc_12_the_last_saved_time_is_the_project_files_modification_time`, `req_gc_12_the_fingerprint_follows_the_saved_files`, `req_gc_12_a_project_with_no_saved_file_reads_not_saved_yet`, `req_gc_12_req_gc_07_the_traceability_block_does_not_change_the_g_code` and the updated snapshot `req_gc_12_req_nfr_08_the_sheet_content_matches_its_snapshot`. `setup_sheet.rs`: the date and clock unit tests. `xtask/src/trace.rs`: `req_nfr_13_a_gate_in_any_spacing_still_counts_its_tests`.
