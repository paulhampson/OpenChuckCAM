# 0640: Helical finishing, rest machining and simultaneous finishing can keep the links between passes low, and new finishing operations start with low links (OP-14, SET-06, SET-09, SIM-08, NFR-07, NFR-12, GC-07, NFR-06; issue #469; OI-1741; lane PATHORDER)

- **Status:** proposed, 2 Nov 2026, by lane PATHORDER (pull request D). The owner chose the scope on 1 Nov 2026, relayed by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7; decision 0049): low links for helical, rest and simultaneous finishing, as a pull request of its own after the pass order (decision 0639, pull request #496). On 2 Nov 2026 the owner chose, relayed the same way, that new operations of these strategies and of axial and indexed finishing start with `low` (Decision item 6). Not yet accepted.
- **Date:** 2 Nov 2026.
- **Issue:** #469, "Optimise toolpath order for all strategies: avoid going back and forth, and extra machining time".
- **Amends:** [0496](0496-low-links-between-finishing-passes.md) item 1 for new operations: a new axial or indexed finishing operation is written with `low` (item 6). The default for an operation saved without the parameter stays `retract`.
- **Builds on:**
  - [0496](0496-low-links-between-finishing-passes.md): the option, its wording, and its link rule in the shared assembly, unchanged here.
  - [0495](0495-minimum-clearance-and-clearance-from-stock.md): the clearance a link retracts to otherwise.
  - [0035](0035-workholding-and-retaining-tabs.md): held regions and tabs stay in the stock.
- **Open items:** OI-1741 narrowed to pencil clean-up and swarf; the axial pattern of simultaneous finishing and rest machining are done here.

## Context

Decision 0496 gave axial and indexed finishing **Links between passes**. With `low`, a link stays the approach distance above the setup's stock where the whole link is clear of it and of the held regions, and otherwise retracts as before. OI-1741 asked for the same option on the other strategies that retract between passes.

The lane's measurement for issue #469 (decision 0639, Context) found that on the Z1 most of the time between cuts is the retract and approach at the rapid Z of 600 mm/min:
- The order of the passes changes it by seconds.
- A lower link height changes it by minutes.

The strategies with the most links are helical finishing, rest machining and simultaneous finishing:
- On the crankshaft, the helix crosses the off-axis pins in half-turn pieces.
- The face turns add links (0297, 0410).
- The axial pattern and the hidden-surface contours (0368) add a link for every pass.

## Decision

1. **The option.** Helical finishing, rest machining and simultaneous finishing gain 0496's **Links between passes** (`link_height`), with its label, choices and wording:
   - `retract`, the default;
   - `low`.

   With `retract` (and an operation saved without the parameter) each operation is made by exactly the code it was, so its output and every golden file are byte-identical. No project schema change.

   A new operation is written with `low` (item 6).
2. **Where it is decided.** With `low` the strategy marks every pass `low_link` on its complete list:
   - the helix or rings, the face turns, the finishing layers and the free ends;
   - the drive's passes, the turns and the hidden-surface contours.

   The shared assembly then links them by 0496's rule, unchanged: against the setup's stock, never below either end of the link, never over a held region's keep-out, every move still put to the collision oracle, and a link that cannot be lowered retracts. A note says how many could not.

   The setup's stock is the blank, not what earlier operations left. So after roughing, the links are higher than they need be and never lower (OI-1740 keeps the lower height open).
3. **The cuts are unchanged.** Only the moves between passes differ (OP-12): the tests compare the cutting moves move for move.
4. **The generated G-code remains the user's to verify (NFR-12).** The parameter text says the stock is checked at sampled points along each link. It does not claim the links were verified on a machine.
5. **Library.** The API change is additive: the parameter is in three more schemas, and `Pass::low_link` already exists (0496). The default for a new operation changes its output (item 6), so the coordinator assigned a minor version: 0.40.0, applied at the release push, after #496's 0.39.0.
6. **New operations start with low links (the owner's choice, relayed 2 Nov 2026).**
   - `Application::add_operation_on` writes `link_height = low` into a new operation of every strategy that has the parameter: helical finishing, rest machining, simultaneous finishing, axial finishing and indexed finishing. This is the way decision 0205 writes roughing's stock to leave and 0639 the pass order.
   - An operation saved without the parameter keeps `retract`, so saved projects, the golden files and the reference programs are unchanged. None of them is made from a new operation.
   - A link that cannot be lowered retracts, and the note says how many did, as 0496 says.

## Alternatives considered

- **A link height of its own for these strategies** (for example from the part's surface plus the stock to leave): lower again, but not provably clear unless the setup is known to be roughed. The owner said no for 0496 (OI-1740).
- **Low links always, with no option:** would change every saved operation and golden file; 0496 made it a choice.
- **Bridging the gaps of the helix in the air instead of linking:** the low link already does this, as one straight move above the stock, checked like any other.

## Consequences

- **Code:**
  - `crates/openchuck-path/src/axial.rs`: the shared `low_links_asked` and `mark_low_links_if_asked`.
  - `helical.rs`, `rest.rs` and `simultaneous.rs`: the parameter and the marking.
  - `crates/openchuck-app/src/opflow.rs`: new operations (item 6).
- **Tests:** `crates/openchuck-path/tests/finishing_links.rs`, on the crankshaft for all three strategies (each run uninstrumented with `cargo nextest run`, 4 to 10 s):
  - `req_op_14_links_default_to_retract_and_are_unchanged`
  - `req_op_14_req_sim_08_req_nfr_12_low_links_stay_below_the_clearance_above_the_stock`
  - `req_set_09_low_links_do_not_reach_into_a_held_region`
  - `req_gc_07_req_nfr_06_low_links_are_identical_at_1_2_and_n_threads`
  - In `crates/openchuck-app/tests/operation.rs`: `req_op_14_req_sim_08_a_new_finishing_operation_keeps_its_links_low` (all five strategies; others get nothing; a saved operation keeps none). The playback test of adding an operation now expects the value.
- **Documents:**
  - this record and the decision index;
  - the verification plan's OP-14 row;
  - OI-1741 narrowed;
  - the work breakdown (lane PATHORDER);
  - the changelogs of `openchuck-path` and `openchuck-app`.

## Verification

Measured on 2 Nov 2026 on this branch (main `c78e34b` plus this change). The crankshaft model (`testdata/stl/crankshaft.stl`) is in a scratch project, not in the repository:
- a 38 mm blank with a 6 mm held start;
- the Z1 profile, clearance radius 40;
- rotary roughing first, then each finishing operation with a 3.175 mm ball.

Estimated time is the sum of the assembled moves' durations, the figure playback and the setup sheet use (SIM-08). "Radial travel" is the distance the tool moves towards or away from the axis while not cutting.

| Operation | Retract: time, time not cutting, radial travel | Low: time, time not cutting, radial travel | Saving |
| --- | --- | --- | --- |
| Helical finishing (spiral) | 1959.9 s, 1057.9 s, 8407 mm | 1265.5 s, 363.6 s, 1462 mm | −35 % |
| Helical finishing (rings) | 1939.1 s, 1047.7 s, 8320 mm | 1164.9 s, 272.9 s, 576 mm | −40 % |
| Simultaneous finishing (spiral) | 1803.7 s, 820.0 s, 6995 mm | 1111.7 s, 128.0 s, 832 mm | −38 % |
| Simultaneous finishing (axial) | 12054.0 s, 9761.3 s, 83180 mm | 4040.3 s, 1748.2 s, 9529 mm | −66 % |
| Rest machining | 405.8 s, 371.5 s, 2934 mm | 204.8 s, 170.5 s, 926 mm | −50 % |

- The cutting time is unchanged (within 0.6 s: only the plunge into each pass is shorter).
- Of the 1415 links of the axial pattern, 11 could not be lowered and retract, as the note says.

**Simulation (SIM-03, SIM-05, OP-14).** `openchuck-cli simulate` was run on the same project with the roughing, helical finishing, simultaneous spiral and rest machining, first with `retract` and then with `low` (cell 0.1 mm). Both runs passed: no rapid move removes material, and nothing is overcut beyond one cell.

The results are identical:
- each operation removed the same material: 38013.5, 1404.3, 56.0 and 0.3 mm³;
- 17234.0 mm³ is left;
- the most material left is 3.0 mm at the same point, the unreachable held end;
- the deepest overcut is 0.0472 mm at the same point.

With `low` the finishing operations have fewer poses: 144382 instead of 213838 for helical finishing, from the shorter moves between passes.

Axial and indexed finishing on the same project, for the new-operation default (item 6). Indexed uses its default positions, adaptive spacing off.

| Operation | Retract: time, time not cutting, radial travel | Low: time, time not cutting, radial travel | Saving |
| --- | --- | --- | --- |
| Axial finishing | 4414.7 s, 2516.6 s, 23246 mm | 2180.8 s, 283.1 s, 1083 mm | −51 % |
| Indexed finishing | 72.5 s, 48.7 s, 417 mm | 39.9 s, 16.1 s, 91 mm | −45 % |

The figures depend on how far the clearance is above the stock. On the tests' fixture, with the clearance 11.5 mm above the stock, the saving is 1.5 to 2 per cent.
