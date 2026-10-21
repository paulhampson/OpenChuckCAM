# 0189: The setup sheet is written with the G-code, from the same run (lane BUG-SWARF; pull request 235)

- **Status:** accepted (21 Oct 2026, by the owner directly in the lane BUG-SWARF session: "Make writing a setup sheet be part of the gcode generation so they happen together"). Items 2 to 5 are how the lane built that instruction. Only the owner accepts this record (decision 0049).
- **What it amends:** the footer of [UI specification](../ui-design/ui-specification.md) section 11.6 (the separate "Export setup sheet (HTML / PDF)" button) and its right column, both edited in the same change. [Decision 0028](0028-slice-19-output-options-and-setup-sheet.md) item 4 (what the sheet holds) and [decision 0110](0110-setup-sheet-traceability.md) are unchanged. GC-12 and the other requirements are unchanged.
- **Date:** 21 Oct 2026.
- **Open items:** none.

## Context

CodeRabbit's review of pull request 235 (decision 0175) found that the setup sheet's new Tolerance column could disagree with the times beside it. The export dialog's "Export setup sheet" button wrote the sheet from the latest run, current or not, while the sheet reads every setting (operations, tools, speeds, and now the tolerance) from the project as it is now. After an edit, the sheet could describe a project that the times, and the G-code already written, were not made from. The lane did not change that behaviour itself, because when a sheet may be written is a product decision. The owner decided it: the sheet is written together with the G-code.

Requirements: GC-12 (setup sheet), GC-05 (output files), GC-15 (checks before writing), NFR-12 (the user verifies the program). Records read: 0028 (the sheet), 0071 (the export dialog), 0110 (sheet traceability), 0148 and 0173 (when the dialog can open).

## Decision

1. **The sheet is written with the G-code.** "Write G-code" writes the G-code files and, beside them, the setup sheet as HTML and PDF, named after the project as before (`<project>-setup-sheet.html` and `.pdf`). The separate "Export setup sheet (HTML / PDF)" button is removed, so a sheet is never written on its own from the dialog.
2. **From the same run.** The sheet is made from the run the G-code was posted from, so its programmed times are the program's. `Application::write_output(folder, run)` in `openchuck-app` writes both and records the export. The dialog only calls it (architecture: the dialog draws and wires).
3. **A run made before the project last changed writes nothing.** `write_output` compares the run's project with the current one and returns the new `AppError::OutOfDate` ("the project changed after the files were made, so they must be made again before writing") instead of writing. Because nothing is written, the sheet always matches the program beside it. The dialog already blocks writing in that case and makes the files again; this is the check behind it (NFR-07).
4. **The dialog says so.** Under the files to write, the dialog names the two sheet files: "The setup sheet is written with the G-code, from the same run: <project>-setup-sheet.html and <project>-setup-sheet.pdf." After writing, the status bar says "Wrote N G-code files and the setup sheet (HTML and PDF) to <folder>."
5. **The command line is unchanged.** `openchuck-cli`'s `--setup-sheet` already writes the sheet from the same run as the G-code it posts. It stays an option, so scripts that write only G-code keep their output.

## Consequences

- The Tolerance column of decision 0175 always records the tolerance the program was made with.
- A user who wants only a sheet writes the G-code too. There is no way to write a sheet without a current run from the dialog.
- `Application::write_setup_sheet` and `Application::setup_sheet_file_names` stay public for the command line and for tests.

## Tests

`crates/openchuck-app/tests/export_dialog.rs`: `req_gc_12_req_gc_05_writing_the_g_code_writes_the_setup_sheet_from_the_same_run` (the G-code and both sheet files, byte for byte the sheet of the run's toolpath) and `req_gc_12_req_gc_15_req_nfr_12_a_run_made_before_an_edit_writes_nothing`. In `apps/openchuck-ui/src/export_panel.rs`, `req_gc_05_req_gc_14_req_gc_15_req_gc_16_the_dialog_writes_files_once_every_check_passes` checks the status line and that both sheet files are written.
