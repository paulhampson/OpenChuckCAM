# Changelog

## Unreleased

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
