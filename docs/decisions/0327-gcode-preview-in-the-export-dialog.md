# 0327: Read-only G-code preview in the export dialog (lane OUTPUT3)

- **Status:** accepted (30 Oct 2026) by the project owner, who read the condensed record in the coordinator session and approved it. The acceptance was relayed to this lane by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7), as decision 0049 allows. Proposed on 30 Oct 2026.
- **Date:** 30 Oct 2026.
- **Lane/issue:** OUTPUT3, [#312](https://github.com/paulhampson/OpenChuckCAM/issues/312) (follow-up of the OUTPUT-1 assessment, #306).
- **Requirements:** GC-05, GC-07, GC-12, GC-15, GC-16, NFR-03, NFR-06, NFR-07, NFR-12, NFR-15, UI-12, UI-16; UI-10 as amended by 0282 (item 1 of that record leaves #312 to this lane).
- **Builds on:** [0267](0267-output-file-names-sheet-hashes-and-staged-writes.md) (accepted: final names, numbering, SHA-256 of the frozen bytes), [0282](0282-four-step-workflow-with-separate-simulate-and-export-actions.md) (accepted: the dialog stays as it is), [0240](0240-export-write-results-and-replacement-confirmation.md), [0233](0233-unconfirmed-edits-before-leaving-a-form.md). None of them is changed.
- **Amends:** UI specification section 11.6 (the right column's "preview of the first lines" becomes the Preview section below), edited in the same change; verification plan rows GC-05/GC-12 and NFR-15 gain the preview tests. No requirement or architecture text changes. No project schema change. `openchuck-app` and `openchuck-ui` only, so no library version bump.
- **Open items:** OI-1300 (search, closed by measurement), OI-1301 to OI-1303 (open, below).

## Context

The export dialog showed the first 14 lines of one file in a box. The owner asked for a read-only preview of the whole program before anything is written: monospaced, with line numbers, jump to line, smooth on programs of a million lines, a file picker when the output is split, the exact final bytes and their hashes, invalidated by edits like the rest of the dialog. Playback linking and G-code editing are out of scope.

## Decision

1. **The bytes shown are the bytes written.** `Application::output_preview(run)` returns the program files by the same names and the same text as `prepare_output` writes. Both take the file list and the position of each text in the run from one function (`program_sources`), so there is no second path. A preview file holds no copy of the text: it points at the run's text, which `prepare_output` also turns into the bytes (`as_bytes`). Each file carries its final name (0267 numbering), size, line count and the SHA-256 the setup sheet lists. A run the project has left behind is refused with the same error as the write (`OutOfDate`).
2. **Where it sits.** The right column of the dialog, below the files table, replaces the 14-line box. It is part of the dialog's scrolling body, so the footer, the safety band and the Write rules (GC-15, GC-16) are untouched. The dialog's size and structure do not change (OI-1301 asks for the native display check).
3. **What it shows.** A heading "Preview" and a sentence "Read-only. These are the bytes that "Write G-code" puts in the file; nothing is written by looking." Then, for the chosen file: its name, "N lines, M bytes", its SHA-256 with a Copy button, a Go to line box, a Find box, and 16 monospaced rows with a line-number gutter and a scroll bar. When the output is split into several files, a "File" box picks the file (the files table also picks it). Long lines are not wrapped; Left and Right shift them, Home returns. The wording never says the program is checked, valid or safe (NFR-12).
4. **Keys (NFR-15).** Click or Tab to the viewer; then Up and Down (one line), Page Up and Page Down (a page less one line), Home and End (top and bottom), Left and Right (columns), Ctrl or Cmd G (moves the keyboard to Go to line; Enter goes). In Find, Enter finds the next line holding the text and Shift Enter the previous, wrapping round the file. The mouse wheel scrolls the preview and not the dialog while the pointer is over it. The viewer states its meaning for assistive technology ("G-code preview, N lines"). A found line is marked with a fill, not by colour alone, because it is also scrolled to the middle of the view.
5. **Smooth on a million lines.** Only the rows in view are drawn, from a line index of one 32-bit offset per line (4 bytes per line, about 4 MB for a million lines; a text over 4 GiB is refused with a message, NFR-07). The position is a line number in `f64`, not a pixel offset: `f32` pixels lose whole rows past about a million rows of 16 pixels, which egui's own row list would hit. The files table's line count now comes from the index; before it scanned the whole text once per frame.
6. **Invalidation.** The preview is made when the dialog has files for the project, post and grouping as they stand, and again when the run or the file name templates change. When an edit makes the files out of date, the dialog already hides the files and says so; the Preview section goes with them, so a hash is never shown for a program that is not the one to write. The pending-edit guard (0233), the running-job restrictions and the GC-15 blocks are unchanged.
7. **Search is added (OI-1300), because the measurement supports it.** A plain, case-sensitive text search is one scan of the text. On a program of 1 000 000 lines (40 MB, release build, this container): building the line index 32 ms, the SHA-256 38 ms, a line lookup 0.1 microsecond, one search scan 22 ms (`output_preview::measurements`, ignored test). It runs when the user presses Enter, not on every frame, so it does not hold up scrolling.
8. **Measured cost in the dialog (OI-1300).** Same program through the whole export column: 79 ms for the frame that makes the preview (index and hash of one million lines), then 43 to 51 microseconds per frame at the top, the middle and the end (`preview_tests::measure_a_million_line_program_in_the_dialog`, ignored). Extra memory: the 4 MB index per file; the text is the run's own. The 79 ms is paid on the user interface thread once per run (OI-1302).

## Alternatives considered

- **A second copy of the bytes for the preview.** Rejected: it could differ from the write, and costs 40 MB per million lines.
- **egui's `ScrollArea::show_rows`.** Rejected for the `f32` pixel limit in item 5.
- **Reading the files back from disk.** Rejected: the preview is wanted before writing.
- **A separate large window.** Not needed: the column fits at 1366 by 768 (layout test); OI-1301 keeps the native check with the owner.
- **Search deferred.** Not chosen: measured cost is small (item 7).

## Not decided here

- Playback linking (assessed separately); G-code editing (out of scope); regular-expression, whole-word and case-insensitive search; making the preview in a background job.

## Verification

`crates/openchuck-app/tests/export_dialog.rs`: preview bytes equal the bytes on disk and the sheet's hash (`req_gc_07_req_gc_05_the_preview_bytes_are_the_bytes_written_with_their_hashes`), numbered names, a stale run refused, line access, search next, previous and wrap. `crates/openchuck-app/src/output_preview.rs`: the line index against `str::lines` for empty, no final newline, CRLF and non-ASCII text. `apps/openchuck-ui/src/export_panel/preview_tests.rs` and `preview.rs`: keys, Ctrl G, the dialog's preview equals the written bytes, the file picker, invalidation after an edit, wording, search marks and scrolls to the line, the go-to parsing and a position that stays exact at ten million lines; `layout_tests.rs`: the Preview section at 1366 by 768 and 1920 by 1080, 100% and 120%, with the footer fixed.
