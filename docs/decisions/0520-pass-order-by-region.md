# 0520: Face turns and hidden-surface passes are cut beside their region (OP-14, OP-15, SIM-08, OP-12, SET-09, GC-07, NFR-06, NFR-12; issue #469; lane PATHORDER)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049), unchanged, after the owner read it in full. Proposed the same day by lane PATHORDER. The owner's answers were relayed by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7) on 1 and 2 Nov 2026 (decision 0049):
  - D1: ordering (this record, pull request C) first, then low links for helical, rest and simultaneous finishing as a separate pull request (D, OI-1741).
  - D2: on for new operations; saved operations unchanged.
  - Objective (b): by X region, A as the tie-break, accepting up to one per cent more estimated time.
  - D4 dropped (see Alternatives).
  - D3, golden changes: none are needed (see Verification).
- **Date:** 2 Nov 2026.
- **Issue:** #469, "Optimise toolpath order for all strategies: avoid going back and forth, and extra machining time".
- **Amends:** [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) item 3 ("each face's turn is its own pass after it") and [0368](0368-simultaneous-finishing-of-hidden-surfaces.md) item 2 ("cut after the drive's passes and the turns at faces"). With **Pass order** set to by region, those passes may be cut beside their region instead. With after main passes they are cut as those records say.
- **Builds on:**
  - [0034](0034-slice-12-baseline-finishing.md) and [0036](0036-slice-13-simultaneous-finishing.md): links, continuity and the unwind stay with the shared services.
  - [0390](0390-helical-finishing-in-rings.md) item 4: rings already put a face's ring in X order.
  - [0410](0410-face-turns-round-an-off-axis-section.md): turns round an off-axis section.
- **Open items:** new OI-2042 (the order inside one hidden area). OI-1741 (low links for these strategies) is pull request D.

## Context

The owner saw on the crankshaft that the turns at the faces across the axis come towards the end of the toolpath, with more clean-ups on the webs, so the tool goes back and forth along the part.

In the code:
- Helical finishing, rest machining (spiral) and simultaneous finishing cut their main passes first: the helix, or the spiral's or axial pattern's zone passes.
- Then they append the face turns (0297, 0410), in X order from the lowest.
- Simultaneous finishing then appends the contours on surfaces hidden from the drive (0368). These are the "clean-ups on the webs".
- No step anywhere reorders passes. The shared assembly keeps the order it is given, fits each pass's A by whole turns to the one before, and links each pass by retracting to the clearance (OP-14).

**Measured first** (lane plan on issue #469, comment 6575830475). Scratch projects with the crankshaft and blisk models (`testdata/stl`) and the baluster example. Estimated time is summed from the assembled moves' durations.
- About 85 per cent of the time between cuts is the retract and approach at the Z1's rapid Z of 600 mm/min. That time is the same in any order.
- Ordering alone therefore changes the time by a few seconds either way. Lowering the clearance from radius 40 to 28 saved 14 per cent on the same operation, which is why low links (pull request D) are the larger saving.
- An order by X distance alone, from local prototypes:
  - removed the back-and-forth;
  - added 5 per cent to the simultaneous spiral, because each turn starts at a fixed angle and A had to reposition;
  - a strict "never longer" objective chose places by A and left the back-and-forth (6 reversals instead of 5).
- The owner chose (b) between those two.

## Decision

1. **The option.** Helical finishing (spiral), rest machining (spiral) and simultaneous finishing gain **Pass order** (`pass_order`):
   - `by_region`: "By region".
   - `after_main`: "After main passes", the default.
   An operation saved without the parameter keeps the old order, so its output and every golden file are byte-identical. A new operation of these strategies is written with `by_region` (`Application::add_operation_on`, as decision 0205 does for roughing's stock to leave). Rings already cut a face's ring in X order (0390), so the option changes nothing there. No project schema change.
2. **What may move, and what may not.** The strategy gives its main passes and its extra passes as groups:
   - Each face turn, all its pieces, is one group.
   - All the contours of one hidden area (0368's grouping), in their order, are one group.
   - The main passes keep their order. Every pass keeps its points, its direction and its way round (OP-15: no cut is reversed; each turn keeps its start angle and direction, 0297 and 0410).
   - A group moves whole and never before the first main pass.
   - Finishing layers and free ends (0344, 0345) are not moved.
   - Roughing, axial and indexed finishing and every other strategy are unchanged. Their order is set by layers and stock, by zigzag, or by the user (0033, 0034).
3. **Where a group goes (the owner's objective).** One after another, in the order given, each group goes where the links it changes travel least along X. A, the short way round, breaks a tie. The new order is kept only when both hold:
   - its links travel less along X in all;
   - its links take no more than one per cent of the operation's estimated time longer than the old order's.
   Otherwise the old order is kept. The links' time comes from the machine's rapid rates along X and in A, moving together. The estimate is a lower bound on the assembled path's time: each cut at the feed or at the A feed limit, whichever is slower, and each link out to the clearance radius and back at the rapid rates. So the allowance is never more than one per cent of the real time. Ties go to the earliest place, so the order is the same at any thread count (GC-07, NFR-06).
4. **Safety is unchanged (OP-12, OP-14, SET-09, NFR-12).**
   - The order is chosen before the shared services. Held-region keep-out, assembly, the A fit, every link and the collision checks run as before.
   - The passes are the same, so the material left is the same: a finishing surface is the envelope of the tool's sweeps, whatever their order. The test checks this cut for cut.
   - The generated G-code remains the user's to verify (NFR-12).
5. **The user is told (SIM-08).** With by region, a path note says what the order did: "the tool travels N mm along the part between passes instead of M mm, and the moves between passes take about ±s". Or it says why the old order was kept, with the seconds and the one per cent. The estimated time shows the result as before.
6. **Library.** Additive API: the module `order` (`PARAMETER`, `BY_REGION`, `AFTER_MAIN`, `spec`). No struct changes. The default for a new operation changes its written order, not the cut geometry, so the library family takes a minor bump: 0.43.0, assigned by the Merge Manager at release (decision 0048).

## Alternatives considered

- **A general sort in the shared services** (after the strategy, before assembly). Not chosen: only the strategy knows what may move. Examples are roughing's layers and stock-aware links (0033 item 6), finishing layers cut from the top, the indexed order the user gives (0034), rings (0390), the controller resets at a helix start (0247), and pecks.
- **A strict "never longer" objective.** Measured (Context). It moves groups by A and leaves the back-and-forth. The owner chose (b).
- **Order by X with no time bound.** It could add 5 per cent where a turn's fixed start angle makes A reposition. The bound keeps (b) within one per cent.
- **A turn starts where the tool arrives** (D4: its start angle, or any of its pieces, chosen per place). Measured with X ordering on the crankshaft: within 3 s of a fixed start. Dropped. It would also amend 0297 and 0410's "start angle as before" for no gain.
- **Reordering inside a hidden area.** On the blisk the back-and-forth is within one area's contours, which move as one group here: OI-2042.

## Consequences

- **Code:**
  - `crates/openchuck-path/src/order.rs` (new).
  - `helical.rs`, `rest.rs`, `simultaneous.rs` (groups, the parameter and the note).
  - `simultaneous/hidden.rs` (each contour knows its area).
  - `crates/openchuck-app/src/opflow.rs` (new operations).
- **Tests** (run uninstrumented with `cargo nextest run`; none is long):
  - `order.rs` unit tests: `req_gc_07_*`, `req_op_14_req_sim_08_*`, `req_op_15_*` (two), `req_sim_08_*` (four), `req_nfr_07_*`.
  - `crates/openchuck-path/tests/pass_order.rs` on the crankshaft with a held region:
    - `req_op_12_req_op_15_req_set_09_{simultaneous,helical}_cuts_are_the_same_in_either_order`
    - `req_op_14_req_sim_08_the_crankshaft_extra_passes_are_cut_beside_their_region`
    - `req_gc_07_req_nfr_06_by_region_is_identical_at_1_2_and_n_threads` (3 to 12 s each)
  - `crates/openchuck-app/tests/operation.rs`: `req_op_14_req_sim_08_a_new_operation_cuts_extra_passes_by_region`.
- **Documents:**
  - this record and the decision index;
  - the verification plan's OP-14 and SIM-08 rows;
  - the work breakdown (lane PATHORDER);
  - the open-items register (OI-2042);
  - the changelogs of `openchuck-path` and `openchuck-app`.
- **Open items:** OI-2042. Low links for these strategies are OI-1741, pull request D.

## Verification

Measured on 2 Nov 2026 on this branch. Estimated time is the sum of the assembled moves' durations, the same figure playback and the setup sheet use (SIM-08). "X sweep" is how far the cuts' X centres travel from one cut to the next. "Reversals" counts changes of direction along X between cuts of more than 0.5 mm.

**Tests on the crankshaft fixture** (`tests/pass_order.rs`; ball 3.175 mm, stepover 1 mm, held region below X 2):

| Strategy | X sweep, after main → by region | Estimated time |
| --- | --- | --- |
| Simultaneous finishing (spiral) | 134.9 → 53.0 mm | 886.2 → 889.6 s (+0.4 %) |
| Helical finishing | 92.5 → 37.8 mm | 1168.9 → 1168.6 s |

**Scratch projects.** These are not in the repository:
- the crankshaft and blisk models from `testdata/stl`, in a 38 mm and a 42 mm blank with a 6 mm held start;
- the baluster example (lane DOCSUSER).

All use the Z1 profile with machine clearance radius 40 and a 3.175 mm ball for the finishing. Every operation is generated in one project with rotary roughing before it. The "after main" figures are from `main` at `4c15579` (the parameter absent gives the same output on this branch); "by region" sets it on this branch. The table shows time, then X sweep, then reversals.

| Part, operation | After main passes | By region | Path note |
| --- | --- | --- | --- |
| Crankshaft, helical finishing | 1959.9 s, 94.6 mm, 2 | 1960.2 s, 39.8 mm, 0 | between passes 7.7 mm along the part instead of 62.1 mm, +0.3 s |
| Crankshaft, simultaneous spiral | 1803.7 s, 138.7 mm, 5 | 1812.1 s (+0.5 %), 73.5 mm, 4 | 31.6 mm instead of 95.6 mm, +8.0 s |
| Crankshaft, simultaneous axial | 12054.0 s, 7851.5 mm, 267 | 12045.4 s, 7824.3 mm, 271 | 338.0 mm instead of 353.9 mm, −9.5 s |
| Crankshaft, rest machining | 405.8 s | 405.8 s (unchanged: nothing to gain) | none |
| Blisk, helical finishing | 2572.8 s, 53.5 mm, 1 | unchanged (nothing to gain) | none |
| Blisk, simultaneous spiral | 16611.0 s, 573.8 mm, 278 | 16609.8 s, 573.0 mm, 277 | 192.2 mm instead of 193.6 mm, −1.3 s |
| Baluster, helical finishing | 897.3 s, 27.5 mm, 0 | unchanged (no faces it can turn at) | none |

**Reading the table.**
- On the crankshaft, the face turns and the web contours now come with their region. Helical finishing no longer goes back along the part (no reversals), and the simultaneous spiral's back-and-forth halves.
- The time changes by between −0.1 and +0.5 per cent.
- The remaining reversals of the simultaneous patterns are inside the web contour groups and the axial zigzag; on the blisk they are inside one hidden area (OI-2042).
- Time is the larger saving, and it waits for pull request D (low links), measured in the lane plan: a lower link height saved 10 to 14 per cent on these operations.

**Golden files and reference projects (D3).** None changes: no saved operation has the parameter, so all of them use after main passes. The goldens and reference programs are byte-identical, checked by the existing golden tests on this branch. Only operations created from now on use by region.
