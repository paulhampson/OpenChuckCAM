# 0460: Recovery is offered only for the project being opened; copied project folders keep their recovery data apart (PRJ-08, NFR-07, NFR-04; issue #429; lane PROJECT-OPEN-RECOVERY)

- **Status:** proposed. Item 1 is the owner's decision (D1, option A of three, 1 Nov 2026, relayed by the coordinator; decision 0049). The coordinator approved the lane's plan for items 2 to 6 the same day. The record as a whole awaits the owner's acceptance. Once accepted, it amends [0243](0243-project-recovery-checkpoints.md) items 7 and 8 and UI specification section 13.1, which this change edits to match.
- **Date:** 1 Nov 2026.
- **Issue:** #429, "User is asked about recovery files when opening a completely different project" (build 0.30.0, Windows). The owner's comment: choosing to keep the saved file then opened the project the recovery belonged to, not the one asked for.
- **Builds on:** [0243](0243-project-recovery-checkpoints.md) (recovery checkpoints, accepted), [0114](0114-slice-22-conformance-report.md) (plain words), [0233](0233-unconfirmed-edits-before-leaving-a-form.md) (pending-edit guard), [0302](0302-first-run-rough-edges.md) item 4 (the unsaved marker; unchanged).
- **Open items:** new OI-1640, native verification of this change on Windows. OI-989 (native verification of CHECKPOINT) stays open. No project schema change, no recovery format change and no library version change.

## Context

Decision 0243 item 7 has a per-user index of every project folder that ever held recovery data, and a startup job that inspects them all. The application queued every project the job found in a crash state for a recovery dialog. A queued dialog waited while another dialog was open or the open project had unsaved changes, then appeared on the first free frame. That was usually just after the user had opened a different project: by a double-click or command-line open (the project opens before the startup job ends), or by File > Open while the job was still reading. The job decodes every checkpoint of every registered project, so it is slow when those projects are large. The dialog named the other project's folder, but nothing said that it was not the project just opened.

**Keep saved project** was implemented as "dismiss, then open the candidate's folder", so it replaced the requested project with the other one. **Discard recovery data…** deleted the other project's checkpoints, which cannot be undone, while the user believed they were discarding their own. Recover installed the other project's checkpoint as that project. Nothing was written until an explicit Save, and Save wrote to that project's own folder. When the opened project also needed recovery, its dialog was queued behind the other one.

Reproduced headless with generated projects: project A left in a crash state, project B saved, the application launched with B. The dialog offered A's checkpoints over B, and Keep saved left A open. Windows path forms were not the cause. The index and the open both use the canonical folder (`\\?\` on Windows), consistently.

A second path was found by reading the code. `.openchuck-recovery/` lives inside the project folder, and its `identity` file was the only identity check. A project folder copied with Explorer carries the original's identity and checkpoints. Opening the copy offered the original's checkpoints, and Recover and then Save wrote the original project's state over the copy's saved files. It is the only path found by which recovery could overwrite a different project's saved files.

## Decision

1. **Recovery is offered only for the project being opened (the owner's option A).** The application never shows a recovery dialog for another project. Startup discovery (0243 item 7) still runs, but it opens nothing. Each other project with kept recovery data is one warning line in Messages:
   - Unsaved changes to the project in its folder were kept when it closed unexpectedly. The action is **Open project**: the ordinary Open dialog, with that folder filled in and the usual unsaved-changes question first. That project then offers its own choices.
   - A project whose recovery files are damaged gets a line of the same kind.
   - A moved or unreadable folder gets **Locate folder** (0243's Locate, moved behind the line). Locate checks the recovery identity, as before, and only updates the line; it opens nothing.
   Nothing is deleted. The line stays until that project is opened, and the data stays until the user decides in that project.
   Not chosen: (B) mention nothing, so a crashed project's changes may go unnoticed; (C) keep the startup dialog, but only while no project is open, with "Not now", so it still interrupts at launch for a project the user did not ask for.
2. **An offer belongs to one open.** The check of the opened folder carries the open's sequence number. Its offer is shown only while that open is current, and a later open drops it. An offer waits, as before, while a dialog is open, a field holds unconfirmed text (0233) or the project has unsaved edits. The opened project's own offer is never queued behind another project.
3. **Keep saved project and Discard never switch projects.** Each records its answer for the opened project: Keep dismisses without deleting, and Discard deletes after its confirmation. The requested project stays open, and the writer used for the answer carries on as that project's checkpoint writer. Recover installs the checkpoint as unsaved memory of the same project, as before. The dialog says in plain words that the changes are for the project being opened, and names its folder. Windows' `\\?\` prefix is not shown in folder names.
4. **Recovery data belongs to the folder that wrote it.** The writer records the canonical folder it owns in `.openchuck-recovery/origin`. Older builds ignore the file, so the recovery format stays at 1.
   - **Copy.** The recorded folder is another one and still holds the same recovery identity. The data is the original's: nothing is offered for the copy, and a recovery choice cannot be read from it. The writer for the copy moves the copied folder aside whole to `.openchuck-recovery-copied-<id>` in the copy (nothing is deleted; its own `.gitignore` keeps it out of Git) and starts a fresh identity. Messages says so in plain words.
   - **Moved.** The recorded folder no longer exists or holds another identity. The data stays this folder's (0243's move and Locate behaviour), and the writer records the new place.
   - **No usable record.** The record is missing (data written by an older build) or damaged (empty, not text, not absolute, or larger than 64 KiB). Behaviour is as before: the data is offered, never hidden, and the writer records the folder.
   A copy made before this change by a build without the record, and opened before the original is next opened, cannot be told apart from a move. That is a limitation, not a regression.
5. **Damaged recovery files of the opened project are reported.** When there is nothing to offer, a damaged or partial file of the opened project is reported in Messages. The file stays preserved (0243 item 9).
6. **No schema or version change.** Project schema stays 25 and recovery format stays 1. `MessageAction` gains `OpenRecovery` and `LocateRecovery`, and `recovery::Catalog` gains `copied_from` and `Store::set_aside` (in `openchuck-app` and `openchuck-project`). Neither crate is in the library family checked by `cargo-semver-checks`, so the workspace version is not bumped (decision 0048).

## Verification

Tests named after PRJ-08 and NFR-07, written from this record. They use generated projects only.
- `openchuck-project`, `tests/recovery_origin.rs`:
  - a copied folder is never offered the original's checkpoints, and they cannot be read from it;
  - the copy's writer sets them aside whole and starts its own identity;
  - a moved folder keeps offering its checkpoints, and a copy made after a move is still recognised;
  - data from before the record is still offered;
  - damaged records never hide recovery or crash;
  - an original whose recovery was restarted makes the other folder the owner.
- `openchuck-ui`, `shell/open_recovery_tests.rs`:
  - two crashed projects: opening each offers only its own checkpoints;
  - Keep saved keeps the requested project open;
  - another project's recovery never interrupts a project opened at launch or by File > Open, and is one Messages line whose action fills the Open dialog;
  - Discard deletes only the opened project's checkpoints;
  - equivalent path forms (`.`, `..`, a trailing separator; on Windows also the `\\?\` form and a different letter case) name the same project;
  - a copied folder offers nothing and keeps the copied data set aside;
  - a moved project is located from its Messages line, and a failed lookup keeps the dialog.
- The existing recovery tests now open the project whose recovery they offer. The test that recovered one project over another project's unsaved edits is replaced: an offer for the opened project waits while that project has unsaved edits.

Native Windows behaviour (Explorer copies, double-click opening, the `\\?\` and drive-letter case forms) needs the owner's check on a Windows build: OI-1640.
