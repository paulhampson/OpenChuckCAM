# Changelog

## Unreleased

- ICON (OI-2106; decision 0542, accepted): OpenChuckCAM has its own icon, a chuck seen end-on inside an A-axis arrow, white on a near-black tile. The window and the taskbar show it instead of the toolkit's default icon, and the Windows program file carries it, so Explorer shows it too. `cargo xtask icon` makes the files from `assets/icon/icon.svg`.

- STALETAB (issue #558, sev1; decision 0528, accepted; UI-11, NFR-12): an Apply of the workholding section is stored when its check finishes, also when the user has moved on (an operation selected) before it finished; before, the result was taken only while the section was drawn, so the tab was not stored and the operations stayed up to date while the view drew the new tab. The section follows the stored setup while hidden, and Messages warns, on the setup's workholding, while a draft is not applied ("The workholding of '<setup>' has changes that are not applied. ..."). No change to generated output.

- VIEWPERF (issue #556; NFR-03, UI-04, UI-05): the outline under the pointer is drawn in its own viewport layer, so zooming or moving the pointer over a shown selection set draws only that outline again, not the set. Zooming had rebuilt and uploaded the whole set on almost every frame because another triangle came under the resting pointer: on the owner's carving with a set of 496,172 triangles a zoom frame took 873 ms (p95 951 ms), now 0.5 ms (p95 0.9 ms), measured without a window and without a GPU. On a public stand-in (the NFR-03 benchmark model, 1,002,000 triangles, a set of 991,980) 1,783 ms (p95 1,903 ms), now 0.3 ms (p95 0.3 ms). What is selected and drawn is unchanged. Showing or editing a large set still takes one long frame (about 0.4 s on the owner's carving, was 0.9 s); building it off the interface thread is held as an open item.

- STEPALL-CLOSE (issue #351; decision 0526, accepted): the operation inspector shows *Exact STEP surface contact* in *Pattern and tolerance*, after *Tolerance*, for helical, indexed, rest, simultaneous and axial finishing and pencil clean-up on a STEP part, with a warning line under it when the tool cannot take it.

- LARGEPATH pull request 3 (#470, OI-2023): the export dialog takes the file plan from the posting job instead of making it on the interface thread when posting ends (a frame of 200 ms on the CrankShaft model, up to 560 ms on the owner's carving). The same files.

- LARGEPATH pull request 2 (#470, decision 0510 item 4): the Results tab keeps its lines, counts, Copy all text and whether the project has changed since the run, per run and revision of the project, instead of grouping every note on every frame it is shown (47 ms a frame for 59,044 notes on the owner's carving). The same lines and text.
- LARGEPATH pull request 2 (#470, decision 0510 item 5): the export dialog copies the project for its key once per revision, not every frame, and keeps what comparing a kept project with it gave (the posted files, the last attempt, the preview's run). On the owner's carving the open dialog drew a frame in 59 ms, now 0.4 ms. The same files and checks.

- FACEAPPROACH follow-up (#499, OI-2018; UI-03): hiding the axes (Show or hide, V) hides only the guides' arrowheads and labels (A+, the A0 marks, the work axes); the Facing extent and margin labels stay with their lines.

- SIMVIS (#488, accepted decision 0514): deviation colours cover only the area each band applies to, not whole triangles. On the cut part, the violet of an enclosed bore covers the bore's disc on a faced end, not whole strips across the ring. The probe reads the piece under the pointer, and the colour scale says when the pieces ran out.

- SIMVIS (#478, accepted decision 0513): Simulate draws material no tool can reach (enclosed across the rotary axis, such as a bore along it) as "Unreachable (enclosed)", violet `#b652ff` under a dark cross-hatch, on the cut part and the part; "Not measured" is slate under a light hatch instead of the grey of "within"; the colour scale lists both; the probe adds "· unreachable (enclosed)"; the results inspector shows "Unreachable (enclosed)" and "Left, not classified", an "of which unreachable" row under each histogram row that has any, and a "Show unreachable (enclosed)" tick box. The run's warning about it is in Results and in Messages, on the setup.
- LINKS-UI (#524, #436; UI-07, OP-14; decision 0496): Links between passes (axial, indexed and Facing End/Box) is in the Entry, links and retract section, and its two choices read "Retract to the clearance" and "Stay low where clear" instead of the stored words.

- FACEAPPROACH follow-up (#499, OI-2016; SIM-02): in the machine view the Facing diagram (extent, margin and cutter sweep, and the extent and margin labels) turns with the workpiece as A turns, as the path lines do.

- FACEAPPROACH (#499, decision 0507, accepted): the Facing cutter sweep is drawn in its own colour (lime `#c2e07a`, a thin line) instead of the Cut amber, and the path legend lists "Cutter sweep" while one is shown.

- POLISH (#381, #379, #418): File > Open recent lists the recent projects by folder name (full path on hover) with Clear recent list; a folder that is gone is dropped from the list; a choice goes through the ordinary Open command and fills the Open dialog with that folder. Typing in an export file-name template no longer splits the toolpath again (the split is kept for the run). Results keeps every feed-limit note of an operation name that two operations share. Whether the recent folders still exist is checked off the UI thread, so a dead network share cannot stall the menu (a late answer never forgets a project added meanwhile). The export Preview is rebuilt from the same kept split. The click log records a recent project as "Open recent project", not its folder name.

- ENDFACE stage 3 (#303, decision 0239, accepted): ticking Adaptive spacing on an operation saved with a Scallop height of 0 asks for the Allowed excess first (0.01 mm suggested, 0 allowed); Cancel changes nothing and Turn on sets both in one undoable step. Export asks for its own tick box, for the files shown only, before it writes files holding a partial path.

- FIRSTRUN (accepted decision 0302): a project with no machine reads "Needs attention", not "Error to fix"; an empty Stepdown says "Required. Enter the depth of each layer, or add cutting data for this tool and material in the tool library."; the "No tabs are stored" line is true when a held region joins the part and the grip warning is in plain words; the title bar says "Edited, not saved" with a tooltip; under Cutting feed a line says what the machine's feed limits did in the last run. Nothing written changes.

- SIMPERF (#382, accepted decision 0322; owner decisions D1, D3, D5): Settings has "Memory for playback checkpoints (MB)" (1,536 by default, 256 to 8,192), capped at a quarter of the computer's memory with "This computer: at most N MB" when it applies. While playback plays, the tool follows the clock and the cut part follows behind it; a cut-part bar under the position bar shows where the cut part is, how far behind, and the progress of the one being made. When playback is still, the tool and the cut part match exactly, as before. Frames made while playing have no picker; once still, the probe works as before.

- OUTPUT3 (accepted decision 0327; #312; GC-05, GC-07, NFR-15): the export dialog has a read-only Preview of the program as it would be written, in place of the first 14 lines: a file picker for split output, line count, size and SHA-256 with Copy, Go to line (Ctrl or Cmd G), Find, monospaced rows with line numbers and a scroll bar, smooth on a million lines. It leaves with the files when an edit makes them out of date. Nothing written, no editing.

- FOURSTEP (accepted decision 0282; #322; UI-10): four workflow steps (Model, Setup, Operations, Simulate). Simulate's footer has two actions in fixed places, the simulation action and "Post and export…" right-most, with exactly one filled (the next required step); Post and export opens the existing export dialog with the old fifth step's rules, through the pending-edit guard, and is disabled while a job runs. The step bar says "G-code written" or "G-code not written" after step 4 and its health is the worst of the four. Alt 1 to Alt 4 open the steps; Alt 5 opens Post and export as Ctrl E does. An enabled footer action shows what it does on keyboard focus, and closing the dialog opened from "Post and export…" gives the keyboard back to it. The export dialog, file names and output bytes are unchanged.

- MODELCOPY (#385, accepted decision 0307): the import dialog's "Copy the file into the project folder" tick box (ticked by default) with the file's size and a line saying what happens; the Model inspector's Source section says where the file is and has "Copy model into project" for a linked model; the project tree names a model by its original file name and marks a linked one with a link mark and "linked"; opening a project with linked models asks whether to copy them in.

- OUTPUT2 (accepted decision 0267; #308, #310): the export dialog has File names fields for the program files and the setup sheet, with Reset, a list of the fields, and the problem in red when a template cannot name the files; the files list and the sheet line preview the names written, with the numbered names and an amber warning when names clash; Write G-code is blocked only while a template cannot name a file. The templates are the user's setting, kept by the shell once a change is confirmed (a draft is not kept, decision 0233).

- FACING review fix (UI-02/03): replacing a scene job preserves an unconsumed camera-fit request; ordinary rebuilds after the fit is consumed leave the camera unchanged.

- BUG-SIMEND, SIM-03/04, issues #334/#330: draw the final cut surface after
  dragging, jumping, stepping or playing to the end without another interaction.
  Failed cut-part preparation disables Deviation colours with its reason while
  retaining Playback's error and existing stale/export rules.
- GC-17: configure minimum block time, window and Off/Report/Slow down in the machine inspector; show effective post settings, command demand and predicted/applied time cost before export and on same-run setup sheets.
- CHECKPOINT, PRJ-08, issue #295: configure a per-user recovery interval,
  retain three scheduled checkpoints plus a separate fifteen-second interim,
  and choose recovery into unsaved memory. Startup discovery, moved-folder
  location, model warnings and asynchronous Save/close handling preserve the
  pending-edit guard. See accepted decision 0243 and OI-989 for native checks.

- OUTPUT-1 documentation: accepted export decision [0240](../../docs/decisions/0240-export-write-results-and-replacement-confirmation.md)
  is administratively renumbered from original 0238 by Merge Manager to
  resolve the SIMUI collision; substantive content and owner acceptance are
  unchanged. No product change in this correction.

- OUTPUT-1, UI-12, GC-15, issue #307: export results stay visible until
  dismissed, list actual full written/failed/not-attempted destinations,
  include both setup sheets and offer Copy paths. Messages retains results
  for the project session with guarded Show result. Existing destinations
  require explicit Replace files approval; Cancel writes nothing, and
  changed choices/readiness/destinations are revalidated before writing.
  The central shortcut guard leaves input to active modals, including export
  and tool-library confirmations: Esc cannot cancel a background job behind
  the result, and global step shortcuts cannot navigate behind a modal.

- OUTPUT-1, UI-16, issue #311: the export dialog reserves a strip for its
  scrollbar, keeping text and controls clear of it. Write G-code and Cancel
  remain in the fixed footer. Export names, generated bytes, readiness checks
  and pending-edit handling are unchanged.
