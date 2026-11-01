# 0430: The Results window gives one information line per operation for the feed limits, and its text can be copied (UI-12, NFR-12, GC-07, NFR-06; issues #431, #432, #418; lane RESULTS-MSGS)

- **Status:** accepted by the project owner on 1 Nov 2026, relayed by the coordinator (decision 0049). The owner's answers on the wording, the worst case and what is shown (1 Nov 2026) were relayed the same way.
- **Date:** 1 Nov 2026.
- **Issues:** #431 (the Results text cannot be copied), #432 (many near-identical feed-limit warnings), and part 1 of #418 (the summary found notes by operation name).
- **Requirements:** UI-12, NFR-12, GC-07, NFR-06.
- **Builds on:** issue 255 (repeated notes grouped, `note_groups.rs`) and [0302](0302-first-run-rough-edges.md) item 5 (the feed field's "Last run" line).
- **Open items:** OI-1230 stays open (notes are still read as text); new OI-1580 (Windows copy, not verified here). No project schema change; no library version change.

## Context

The engine writes one note for each move the machine's feed limits slow. On the blisk this was hundreds of near-identical lines, each shown as a warning, and the user could not tell what they meant. The Results lines were also plain labels, so a long message could not be copied into a bug report.

## Decision

1. **One information line per operation.** For each operation the machine's feed limits slowed, the Results window shows one line, as information (the level already exists), not a warning: "N moves during 'Roughing' were limited by the machine's feed limits. In the worst case the target was A mm/min, but the move was reduced to Y mm/min to stay within the machine's capabilities." For one move: "1 move during 'Roughing' was limited ... The target was A ...".
2. **The worst case is the move that lost the most** (target minus result), the first such move on a tie, not the move with the lowest result. A and Y are that move's two figures.
3. **The per-move notes are not shown** in the Results window or in the copied text. They stay in the run's notes (`Regeneration::notes`), unchanged, so G-code, the setup sheet and the tests are not affected (GC-07). Whether they should be reachable somewhere (a details expander) is for the owner; the default is not shown.
4. **Nothing is hidden (NFR-12).** Only the feed-limit notes of an operation that has a summary are left out. A feed-limit note the summary cannot read, and every other note, overcut, rapid cut and check, keeps its level and is never merged into the summary.
5. **By operation id (issue 418, part 1).** `Regeneration::feed_limits` holds the summary for each slowed operation, made in the pipeline from that operation's own notes where its id is known, in machining order. The Results line and the feed field's "Last run" line both read it, so a rename or two operations with one name cannot mix them up. The summary is a count and a maximum over the notes in order, so it is the same at any thread count. The notes are still read as text, so OI-1230 (structured notes) stays open; no `PathNote` or library change.
6. **Copyable results (issue 431).** The Results lines are selectable text. A **Copy all** button copies the summary line, every line the window shows (a grouped line followed by its moves) and the build information block, as plain text for a bug report. A "Results as plain text" box in the window holds the same text in a read-only field that selects and copies with the keyboard alone.
7. **Not verified here.** The owner also reported that the About window's text and its Copy button do not copy. The About Copy button uses the same call as Copy all and its test passes, but the session could not run on Windows. Recorded as OI-1580 for the owner to test on a Windows build.
8. **NFR-12 wording.** Nothing here says the program is safe or checked; generated G-code remains the user's to verify.

## Documents

No change to the requirements or the architecture. The interface specification has no section for the Results window, so none is edited; the severity rule is UI-12's.
