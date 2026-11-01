# 0580: Simulate display synchronisation fixes (BUG-SIMSYNC #333, BUG-SIMEND #334 and #330), recorded after the fact

- **Status:** recorded after the fact on 1 Nov 2026 by lane DOCS-TIDY; the work was merged earlier (PR #341 for BUG-SIMSYNC, PR #362 for BUG-SIMEND); the owner said no review or acceptance is needed (1 Nov 2026, relayed by the coordinator).
- **Date:** 25 to 27 Oct 2026 (the work); 1 Nov 2026 (this record).
- **Lane:** DOCS-TIDY (`claude/lane-docs-tidy`), summarising the notes of the lanes `codex/bug-simsync-333` and `codex/lane-bug-simend`.
- **Issues:** #333 (BUG-SIMSYNC), #334 and #330 (BUG-SIMEND).
- **Requirements:** SIM-02, SIM-03, SIM-04, SIM-05, UI-02, UI-11, GC-15, NFR-12.
- **Relates to:** [decision 0040](0040-slice-9-display-and-playback.md) ("Issue 333 correction"), [decision 0322](0322-simulate-playback-frame-cost.md) item 8 (the tool follows the clock while playing), [decision 0238](0238-simulate-display-and-operation-navigation.md) (OI-987 hardware verification still open).
- **Numbers:** takes 0580 from DOCS-TIDY's range. No open item, project schema or library change: both fixes are UI and application only (library 0.24.0 at the time of BUG-SIMEND).

## Context

Two display faults in the Simulate view, fixed in two lanes. BUG-SIMSYNC (#333, SIM-02/03/04): the tool, its A rotation and the cut part could show different cursors, and a partial move was not cut at the cursor. BUG-SIMEND (#334, #330): dragging the playback point to the far right (or jumping, stepping or playing to the end) left the previous stock on screen until another interaction; and when the stock could not be prepared, the Deviation colours control gave no reason.

## BUG-SIMSYNC (PR #341)

Fix: partial-move cutting and synchronised tool and surface publication, so that each displayed tool and stock pair uses the same cursor; expensive surface jobs may limit how often the viewport updates but never split the pair. The behaviour is specified in the user interface specification ("Synchronized cut playback") and in decision 0040's amendment; this record adds no rule.

Verification (25 Oct 2026, after the released integration of main `ab863f6531d1e8fab8c2562e88d92b9277937c3` with ROUGHPATH #340 into head `e17e65e4012780f2aa31acb2c9b15c6573f99e9e`, two-parent merge, no textual conflict): the full locked app and UI run passed 972 tests (501 app, 471 UI; UI binary 150.79 s) with no failure and none ignored, including the software-adapter cut-stock pixel, picking, opacity and full-HD rendering-cost checks; the focused partial-move regression passed (20.057 s) and the deliberately delayed stock-surface regression passed (12.296 s), uninstrumented debug timings under concurrent load, not frame-rate measurements; workspace clippy (warnings denied; two unchecked indexes in `stockplay` became checked lookups), formatting, records, conflicts, licence headers, test data, publish and traceability (99/99 P1) pass. Not run: `cargo-nextest`, `cargo-llvm-cov`, `cargo-audit`, `cargo-deny`, `cargo-about` and `cargo-semver-checks` were not installed in that environment (tests ran with `cargo test`; coverage and the security checks are unmeasured there, and `cargo xtask all` could not download a platform-only dependency through the unreachable proxy); the exact-head CI supplied them. Reference-hardware smoothness is unconfirmed. The lane read all 27 remote branch trees and five open PR heads for numbering and claimed no decision, open item or schema number (a 170 KB audit of other branches' state was dropped as noise).

## BUG-SIMEND (PR #362)

**Cause (found before the fix).** The first end-state checks passed because they inspected the updated CPU stock, not the snapshot already handed to rendering; they did not reproduce the failure and did not show that #341 fixed it. `Shell::draw` calls `layout` (including `Viewport::show`) before `run_frame`; the viewport's paint callback captures the current layer `Arc`s during layout; `PlaybackPanel::update_stock` then publishes the final stock surface but returns false when no job or playback is running, so `run_frame` requests no further frame and the last callback keeps the previous stock until the next interaction. The fault is in UI display publication, not in simulation. The requirement regressions follow that real draw order; jump, step and play-to-end each fail on main, the same three fail on pre-#341 main `ab863f6531d1e8fab8c2562e88d92b9277937c3` (10.646, 10.922, 11.120 s), and an actual pointer-drag test (the route the owner identified) failed there at the final drawing assertion (12.960 s). For #330 both injected checkpoint-memory and cut-surface failures failed the new control-explanation assertion (checkpoint exhaustion is injected at the job boundary; the test does not allocate over 1.5 GiB; application tests cover the real checkpoint bound).

**Fix.** Completed checkpoint and surface jobs return a one-frame repaint request, including failure, so layout can show the new stock or the error; idle frames then stop normally (no retry or continuous repaint loop). When stock preparation has failed the Deviation colours control is disabled with the original error beside it and in its disabled tooltip. The remembered colour preference, retained measurements, stock-triangle identities, mapping, picking, staleness and export rules are unchanged; no target-model fallback.

**Verification.** Base: main `ba10400fff3bb7ad32526a7e540919c810e85434` (library 0.23.0), rebased onto `8f788bf` (library 0.24.0); released integration of main `61ebfc493e290dedd64f36ac484ba42874f35f6d` on 27 Oct 2026 (only the owner test list conflicted: BUG-SIMEND owner checks inserted as section B6). No library, schema or export change.

| Test in `playback_panel::tests` (uninstrumented, full suite) | Seconds |
| --- | ---: |
| `req_sim_03_req_sim_04_final_cut_part_is_published_after_jump_to_end` | 43.286 |
| `req_sim_03_req_sim_04_final_cut_part_is_published_after_play_to_end` | 43.115 |
| `req_sim_03_req_sim_04_final_cut_part_is_published_after_step_to_end` | 43.306 |
| `req_sim_03_req_sim_04_req_ui_02_final_cut_part_is_drawn_after_dragging_playhead_to_end` | 45.124 |
| `req_sim_05_req_ui_11_req_nfr_12_req_gc_15_checkpoint_memory_failure_explains_colours` | 3.235 |
| `req_sim_05_req_ui_11_req_nfr_12_req_gc_15_failed_surface_explains_colours` | 7.648 |

The two failure tests cover current and stale states, retained errors, the disabled control and visible reason, stopped progress, no automatic retry, retained run identity, unchanged export readiness for current and blocked export for stale. The drawing-order assertion checks the surface handed to the viewport callback, not monitor pixels (the software-adapter tests cover rasterisation). Initial: `openchuck-ui` 491/491 (285.645 s). Released: **497/497 in 283.752 s** (six more tests from BUG-NEWPROJ), none skipped; new tests take 4.557 to 42.168 s; the whole-dock matrix (81.462 s) is already in both nextest profiles; no new test exceeds a minute. `cargo xtask deps`, `licences`, `records`, `trace` (100/100 P1) and `conflicts` pass; fmt and clippy pass. Own-UI coverage (released): 495/495 in 725.714 s with two long tests excluded; **26,168/29,799 lines (87.82%)**, below the report-only 95% target; `playback_panel.rs` 1,920/2,009 (95.57%); every added production line is covered (checkpoint repaint request 24 times, surface repaint 54 times, unavailable-colour explanation four times); the only new uncovered lines are the test helper's `Shape::Vec` recursion (`playback_panel.rs` 2254 to 2257).

**Not verified:** owner and reference-hardware display verification stays under OI-987 (OI-988 closed); the owner retests #334 and #330 and closes them. Nothing here verifies G-code for machining (NFR-12).

## Consequences

No behaviour beyond the specification in the user interface specification and decision 0040. Notes moved from `docs/handoffs/bug-simend-verification.md` and `docs/handoffs/bug-simsync-release-verification.md` on 1 Nov 2026 (DOCS-TIDY); the owner test checklist item for #334 should link here.
