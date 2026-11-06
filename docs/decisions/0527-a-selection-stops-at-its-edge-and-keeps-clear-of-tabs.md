# 0527: An operation limited by a selection cuts on its selected faces only and keeps clear of the setup's tabs (OP-02, OP-21, NFR-12; issue #555; lane TABCUT)

- **Status:** proposed by lane TABCUT on 6 Nov 2026. The owner's answers below were relayed by the coordinator on 6 Nov 2026; only the owner accepts the record (decision 0049). One answer is **assumed**, not given: refusing (rather than warning) when an assembled path still reaches a tab (item 4; OI-2052).
- **Date:** 6 Nov 2026.
- **Number:** 0527, the next after `main`'s highest (0526) under the owner's rule that decision numbers follow `main`; OI-2050 to OI-2052, free on `main` (highest OI-2049) and on every unmerged branch.
- **Issue:** [#555](https://github.com/paulhampson/OpenChuckCAM/issues/555) (sev1). Related: [#547](https://github.com/paulhampson/OpenChuckCAM/issues/547) and [#548](https://github.com/paulhampson/OpenChuckCAM/issues/548) (decisions 0522 to 0524).
- **Amends:** the region rule of [0032](0032-slice-10-mesh-selection.md) item 6 and [0034](0034-slice-12-baseline-finishing.md) (a contact on the selected region), [0176](0176-swarf-boundary-avoid-and-check-sets.md) (the rule's list), [0035](0035-workholding-and-retaining-tabs.md) item 1 (tabs are part of the target: still so, but an operation limited by a selection keeps clear of them) and [0161](0161-tools-keep-clear-of-the-gripped-length.md) (held regions; unchanged, tabs added beside them).
- **Library:** `openchuck-path` 0.47.0, a breaking change (decision 0048): `RegionConstraint` gains two public fields. No project schema change.

## Context

The owner's carving project (private; numbers only): a tube, X 0 to 35, radius 18.5, held at the start of the stock with a tab on the axis 10 mm across from X -6 to 0. Axial finishing with a 6 mm V-bit of 20 degrees (0.3 mm tip), limited to the selection 'Side only', with low links. Once decisions 0522 to 0524 stopped refusing it, it generated, and its path went past the selected faces and down onto the tab.

- **What the path did.** Every pass started by plunging onto the tab's top, at radius 5.000 mm (the tab's design surface), near X -2.3, then climbed the end face's rim to radius 17.47. The low links between passes stayed at that height, so the tool swept a full ring round the tab top, X -2.50 to -2.17: about 3,400 points at the tab's radius. No point went inside the tab's design shape, but the tool removed whatever earlier operations had left on it there (roughing leaves 0.3 mm).
- **Old, not new.** The build before FLUTE-TABS (`eaeba83`) made the same path and refused it on the flute check, at X -2.235, the same place. Decisions 0522 and 0524 removed that refusal, so the path came out.
- **On a public part.** The cone of `tab_scope.rs` (radius 10 to 5, X 0 to 30, a 6 mm tab from X -4 to 0) with the same V-bit and the cone's side selected: the tip came down to radius 3.000, the tab's surface. Helical, indexed and simultaneous finishing and rotary roughing came onto the tab or within 0.5 mm of it the same way. Rest machining and pencil clean-up refuse a selection.

**Why.**

1. **The tool rolled over the edge of the selection.** The region rule accepted a cut point whenever the drop's contact lay on the selected faces. The rim between the selected side and the unselected end face lies on both. So at the part's end the tool pivoted about the rim with its contact on it, and its tip swept down beside the end face. A 20 degree V-bit keeps the rim contact until its tip is about 5.7 times its offset along X below the rim, so it went all the way down until the tab stopped it. A ball rolls over the rim too, but drops by its radius only.
2. **Nothing kept a limited operation off a tab.** A tab is part of the target (0035 item 1), so a drop never goes inside it, but the tool may rest on it. The keep-out of 0161 covers the gripped held regions only. Links, leads and entries were not checked against tabs at all.

Free ends (OI-2047) were not the cause: the owner's operation has Face free ends off.

## Decision

1. **A selection stops at its edge with an unselected face (the owner's choice, option 1).** A contact that lies on the selected faces and also on the part's faces outside the selection is not on the selection. That is a contact on the edge between a selected face and an unselected one. The tool no longer pivots over that edge: the pass ends where the tool still sits on the selected faces.
   - **The library.** `RegionConstraint::outside` holds the part's faces outside the selection; `allows` refuses a contact on it (the region rule's list, `region.rs`). `None` keeps the old rule.
   - **The host.** It gives the triangles (or, on a STEP part, the faces) the operation's machine selections leave out, whenever there is a selected region.
   - **Changed output.** Every selection-limited finishing path can end a little sooner at the edges of its region (by up to the drop's tolerance where the selection's edge runs across a smooth surface, and by the whole roll where it is a convex edge). The faces it cuts are the same. Whole-part operations are unchanged.
2. **An operation limited by a selection keeps clear of the setup's tabs.**
   - **Which operations.** One with a machine selection or a boundary set. It leaves the tabs out: they are not faces of the model, so no selection holds them.
   - **The clearance.** The tool keeps `HELD_CLEARANCE` (0.5 mm, decision 0161) plus the operation's stock to leave from each tab's design surface. That is the fit's tool, grown by the stock to leave, against the tab grown by 0.5 mm. The owner chose the largest option: also keep clear of the stock earlier operations left on the tab. That part is pull request 2 of this lane (OI-2050).
   - **The library.** `RegionConstraint::keep_clear` holds the tab solids, not grown. `allows` refuses a cut point where the tool, dropped on a tab grown by the clearance, comes to rest at its tip or above. Rotary roughing raises its floor over the grown tab instead, as it does over a check surface (decision 0043), so it still cuts the layers above.
   - **The host.** `workholding::tab_solids` gives the tab solids, with automatic tabs placed against the real held regions first.
   - **Unchanged.** A whole-part operation still cuts a tab to size as part of the target (0035 item 6). The held regions stay kept out of by every operation (0161). The tab warnings of 0522 to 0524 do not change.
3. **Every move is checked.** After assembly, every move of an operation with tabs to keep clear of is followed as the machine makes it, straight in its axes, fed or rapid. That covers cuts, plunges, leads, links, entries and retracts.
   - **Sampling.** Samples are no further apart than the toolpath tolerance, and no closer than 0.01 mm.
   - **The test.** At each sample the tool, grown by the operation's stock to leave (the smallest of its selections' amounts), is dropped on each tab grown by the clearance less the tolerance.
   - **Why less the tolerance.** The fit keeps the full clearance at the points it makes; the tolerance is the room the moves between them have.
   - **Speed.** A move further than the tool's reach from every tab along X is passed over. A tab wholly below the tool's tip along its axis needs no drop.
4. **A path that still reaches a tab is refused (assumed, option R; OI-2052).** It is never written. The message names the move's X and A and the tab's X range, and says that a tab holds the part. It offers changing the selection or the link or entry settings. The coordinator relayed no answer on refuse against warn: R was the lane's recommendation, and the owner confirms or changes it.
5. **Which strategies.**
   - **Trimmed.** Every strategy whose cut points go through the region rule: helical, axial and indexed finishing, simultaneous finishing and rotary roughing. These cut a pass short at a tab instead of refusing it.
   - **Refused, not trimmed.** Swarf, edge following, drilling and Facing honour a selection without that rule, so item 3 refuses them if they reach a tab (OI-2051).
   - **Not affected.** Rest machining and pencil clean-up refuse a selection.
6. **Generated G-code remains the user's to verify (NFR-12).** Nothing in the messages says the path is safe.

## Verification

Tests first, named after the requirements:

- **`openchuck-path`, new `tests/tab_keep_clear.rs`** (in `tests/integration.rs`; decision 0516). A cylinder joined to a tab, with the V-bit:
  - `req_op_02_req_op_21_a_selection_stops_at_its_edge_with_an_unselected_face`: it rolled over the rim before; now axial and helical stop on the side, at X 0 to 0.5, with no move within 0.5 mm of the tab.
  - `req_op_21_req_nfr_12_a_limited_operation_keeps_its_clearance_from_a_tab`: without `outside`, no move comes within the clearance less the tolerance. With the rule in `allows` turned off, the check of item 3 refuses the path, so the test fails.
  - `req_op_21_a_tab_kept_clear_of_does_not_change_a_path_far_from_it`: a band of the side is byte-identical with and without the new fields.
- **`openchuck-path` unit tests.**
  - `solid::req_op_21_a_grown_solid_holds_every_point_within_its_growth`.
  - `strategy::req_op_21_req_nfr_12_any_move_that_comes_within_the_clearance_of_a_tab_refuses_the_path`: a rapid and a cut through the tab are refused, and moves with the clearance to spare are kept.
- **`openchuck-app`, `tests/tab_scope.rs`.** `req_op_02_req_op_21_req_nfr_12_a_v_bit_on_the_side_keeps_to_it_and_clear_of_the_tab`: the owner's case on the public cone. It failed before (the tip at radius 3.000) and passes now.
- **The owner's project, checked locally, never committed.** The axial finishing G-code has no move at X below 0. Every pass starts at X 0.000 on the rim (radius 17.48), and nothing within the tool's reach of the tab comes lower than radius 16.5. The estimated time falls from 15,829 s to 10,324 s.

## Consequences

- Library 0.47.0: `RegionConstraint` gains `outside` and `keep_clear` (struct literals need `..Default::default()`), `Solid::grown` is new, and the strategies' paths change as items 1 and 2 say.
- Selection-limited finishing operations with a convex edge at their region end there. The tests and goldens that moved are listed in the pull request.
- Open items: OI-2050 (the carried-stock clearance, pull request 2), OI-2051 (strategies refused, not trimmed, at a tab), OI-2052 (refuse or warn, assumed refuse). OI-2047 (free ends ignore the region) stays open: with Face free ends on, a limited operation still faces both ends. Item 3 now refuses such a path if a ring comes within the clearance of a tab.
