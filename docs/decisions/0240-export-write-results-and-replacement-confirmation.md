# 0240: Actual export results and explicit replacement confirmation

- **Status:** accepted by the project owner in the OUTPUT-1 chat on 24 October 2026, after the full text was shown ("Accept 0238"). The described behaviour had already been approved directly ("accept the proposed approach"). The owner subsequently assigned the narrowly scoped central modal-keyboard fix in item 7 after the failing regression was explained ("Complete the fix if we have a test failure as we will need that before PR").
- **Date:** 24 October 2026.
- **Lane/issues:** OUTPUT-1, #306 (assessment), #307 (write feedback), #311 (layout).
- **Requirements:** UI-07/10/12/14/16, GC-05/07/12/15/16, NFR-06/07/12.
- **Number:** 0240, administratively assigned by the Merge Manager on 24 October 2026 in [PR #329](https://github.com/paulhampson/OpenChuckCAM/pull/329#issuecomment-6386146099) because SIMUI #328 already held 0238. This record was originally selected as 0238 under the owner's delegated numbering instruction after main/all 27 remote branch refs/register checks. The owner's acceptance of the unchanged export content under original 0238 is retained; this allocation is not new acceptance.
- **Amends:** 0071's export feedback, 0189's folder-only status, UI-12, GC-15's desktop overwrite policy, and UI specification 11.6. Existing stale checks, setup-sheet coupling and acknowledgements remain.
- **Open items:** native dialog/display verification stays with existing OI-580. No new OI or schema number.

## Context

The old writer returned paths only after complete success. A later failure
discarded earlier successful paths; the UI ignored even a successful path
list and showed a count and folder in the status bar. The export scrollbar
also painted over text. The owner requested persistent, accurate results
and approved confirmation before replacing existing destinations.

## Decision

1. **Review before replacement.** A read-only plan freezes the same-run NC,
   HTML and PDF bytes and lists all full destinations. Existing destinations
   are listed in an in-main-window confirmation with Replace files and Cancel.
   Cancel, including Esc, creates and changes no files or folders.
2. **Revalidate.** After field resolution, recheck project/readiness,
   selected post/grouping/folder and destination existence before writing.
   Changed inputs or destinations require another review. Preserve accepted
   0172/0173/0192 and BUG-EDIT 0233's semantic deferred actions, validation and
   Keep/Discard/Cancel. No stale run is exported.
3. **Report actual writes.** Write in order and stop at the first failure.
   Record a path as written only after its write completes. Identify the
   failed destination and reason and every file not attempted. Earlier writes
   remain after partial success; the batch has no atomic rollback guarantee.
4. **Preserve existing files.** Claim new destinations exclusively so a new
   collision cannot silently overwrite them. Prepare approved replacements
   at uniquely claimed temporary paths and rename them, without deleting the
   old destination first. Clean failed temporary/new files; report cleanup
   failure and any incomplete file that remains. Preflight does not make the
   batch immune to external filesystem changes.
5. **Persistent feedback.** Show the result in an in-main-window modal until
   Dismiss or Esc. List every actual full written path, including both sheets;
   Copy paths copies those written paths. Retain the per-file result in
   Messages for the open project session, with Show result. It survives result
   dismissal and project edits. Historical errors do not block current export
   readiness. Only full batch success records a completed export.
6. **Keep export semantics.** Final names and generated contents are unchanged.
   Sheets use the same run and existing document model, with an injected clock
   for byte comparisons. Retain the fixed NFR-12 notice and GC-16/skip-stock
   acknowledgements; keep acknowledgements once a file really is written,
   including partial success (0071). A write result does not verify machining.
   The existing unattended CLI behaviour is unchanged.
7. **Layout and boundaries.** Reserve scrollbar space and keep actions outside
   scrolling content in the export dialog and its confirmation/results. Reuse
   the existing scrollbar helper. Shared dock framing/sizing/dispatch and
   Playback are untouched; shell integration is limited to refreshing Messages,
   opening a retained result through the existing guarded dispatcher, and
   leaving global shortcuts to active modals in `Shell::shortcuts`. Reuse
   egui's modal-layer signal so Esc cannot cancel a background job behind the
   result, and step commands cannot act behind a modal. This central guard
   also protects tool-library confirmations; ordinary floating windows keep
   their existing shortcuts. No shared infrastructure is restructured.

## Scope and alternatives

Silent replacement risks unexpected loss; refusing every existing filename
forces another destination. The owner chose explicit confirmation with honest
partial-write reporting. An atomic multi-file transaction is not implemented.

The #306 assessment is complete: the owner approved four steps with separate
Simulate and Post and Export buttons. Its separately raised build #322 still
needs detailed scope agreement and is not implemented here. OUTPUT-2 retains
naming/hashes/sheet expansion and OUTPUT-3 retains full preview browsing.

## Verification

Requirement-first tests cover actual single/split destinations and bytes,
both sheets, read-only/cancelled preflight, approved replacement, changed or
stale inputs, partial/failed writes, history, dismissal/reopening, copying,
and whole-dialog bounds/scrollbars/fixed actions at supported sizes/scales.
Preserve and run the whole-shell/dock and pending-edit regressions. Record
uninstrumented timings, changed-crate coverage/gaps and cleanup in the PR;
headless frames do not replace the native checks in OI-580.
