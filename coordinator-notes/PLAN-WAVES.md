# FOLLOW-ON PLAN (waves after the current lanes), recorded 6 Nov 2026 from the owner

Coordinator duty: review this file at every queue report; say which wave we are in and what gates the next one.

## Wave 0: current lanes and any bugs the owner identifies (WE ARE HERE)
Live: FLUTE-TABS (#547, #548, flute check -> warning; sev1), LARGEPATH PR2 (OI-2023), PATHORDER D (#501), UIBUGS-3 (#546), STEPALL-CLOSE (#481), FACEFLOW (#513, docs first), DOCS-WB (#493), DOCS-TIDY (#466).
Exit: all wave-0 PRs merged.

## Wave 1: one docs lane to correct the plan documents (size M)
Apply the audit's corrections (10 groups): P1 counts and the slice status table; the lane rows; the schema table and spikes; the slice 20 and 22 boxes; closing the stale open items. Docs only. Start once Phase 0 is mostly merged, so it does not fight the lanes over the same rows.

## Wave 2: release blockers (the severity decision comes first)
- Triage open bugs by severity. Candidates: #477 (part-off safety, now in LARGEPATH PR 3), #435 and #442 (swarf), #462 (partial path by default), #478 (SIMVIS), #471 and #404.
- Safety notice: the verify-the-G-code notice as a post-header option, with a test (NFR-12), size S.
- Fuzz and ASan: close the fuzz gaps and nightly failures (OI-326, OI-327, #451, #369).
- OCCT notice: add it to the release checklist (OI-29).

## Wave 3: hardening and the owner's native checks
- UIR lane: whole-application review against the specification, to tick the slice 22 boxes (UI-10 to UI-16), plus the keyboard-only run (UI-14).
- Native checks (owner): about 30 items (launch on each OS, 1366x768 screens, eframe 0.37, export preview, recovery, the STEP worker, clipboard). Performance: NFR-03 and NFR-04 on reference hardware (OI-540, OI-173, OI-176).

## Wave 4: packaging
- Lane work: the macOS .app, the glibc floor and the tag ruleset (OI-300, OI-301, OI-305).
- Owner decisions: an installer and signing (OI-135), and the Makera data licence (OI-269).

## Phase 5: release candidate
- Freeze, then rerun gates 4 and 5 completely on the Z1: reference set, first cut, G93, tool change, short blocks, X datum. The earlier trial was on build 0.20.0.
- Docs: user-guide items (screenshots, reference pages, release notes drafted by a lane).
- Report: a verification report naming the commit, with SP-15 recorded as not run.
- Close: add slices 8, 17, 20, 21, 22 and 23 to closed_slices, tag lib-v and app-v, publish to crates.io with the owner's token (OI-303).

## Also gated / parked
- DOCS-REFERENCE (#441) and DOCSUSER-2 start after Tier 1/2 merges; ask the owner about the closing-plan Phases 2-5 then.
- Phase2 issues (no lane): #516, #527, #528, #529, #538, #540.
- Dependabot PRs #498, #497, #453 ignored until the owner says.
- Owner-side items: OI-1780, OI-2020, OI-2021, OI-2022, OI-1921, OI-993.
- CI follow-ups: OI-2028 (time-based share dealing), macOS capacity.
