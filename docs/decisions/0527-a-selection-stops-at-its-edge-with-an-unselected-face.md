# 0527: A selection stops at its edge with an unselected face (OP-02; issue #555; lane TABCUT)

- **Status:** proposed by lane TABCUT on 6 Nov 2026, cut back to this one rule on the owner's clarification, relayed by the coordinator: "it's not particularly that it's a tab; the issue is it was cutting outside of a selected region". Only the owner accepts the record (decision 0049).
- **Date:** 6 Nov 2026.
- **Number:** 0527, the next after `main`'s highest (0526) under the owner's rule that decision numbers follow `main`. OI-2054, given by the coordinator (OI-2050 to OI-2053 are reserved for VIEWPERF and FACEFLOW). The draft also used OI-2055 and OI-2056; both are released unused.
- **Issue:** [#555](https://github.com/paulhampson/OpenChuckCAM/issues/555) (sev1). Related: decisions [0522](0522-the-flute-check-warns.md) to [0524](0524-flute-check-reads-the-stock-in-the-tools-reach.md).
- **Amends:** the region rule of [0032](0032-slice-10-mesh-selection.md) item 6 and [0034](0034-slice-12-baseline-finishing.md): a contact on the selected region.
- **Library:** `openchuck-path` 0.47.0, a breaking change (decision 0048): `RegionConstraint` gains one public field. No project schema change.

## Context

The owner's carving project is private; only numbers are given here.

- **The part.** A tube, X 0 to 35, radius 18.5, held at the start of the stock with a tab on the axis 10 mm across from X -6 to 0.
- **The operation.** Axial finishing with a 6 mm V-bit of 20 degrees (0.3 mm tip), limited to the selection 'Side only', with low links.
- **What it did.** Once decisions 0522 to 0524 stopped refusing it, it generated, and its path ran on past the selected faces. Every pass started by plunging beside the end face onto the tab's top, at radius 5.000 near X -2.3, then climbed the end face's rim to radius 17.47. About 3,400 points sat at the tab's radius.
- **Old, not new.** The build before FLUTE-TABS (`eaeba83`) made the same path and refused it only on the flute check, at X -2.235, the same place.
- **On a public part.** The cone of `tab_scope.rs` with the same V-bit and the cone's side selected: the tip came down to radius 3.000, the tab's surface. Helical, indexed and simultaneous finishing and rotary roughing came down past the side the same way.

**The cause.** The region rule accepted a cut point whenever the drop's contact lay on the selected faces. The rim between the selected side and the unselected end face lies on both. So the tool pivoted about the rim with its contact on it, and its tip swept down beside the end face, outside the selection. A 20 degree V-bit keeps the rim contact until its tip is about 5.7 times its offset along X below the rim, so it went down until the tab stopped it. A ball rolls over such an edge too, but drops by its radius only.

**Not a tab rule.** The tab is only where the roll showed: anything past the edge could have been met. The lane's first draft also kept every selection-limited operation a clearance from the setup's tabs, checked every assembled move against them and refused a path that reached one. Measured with this rule alone, those parts changed nothing on the owner's project or on the public part, so they were removed.

## Decision

1. **A contact on the edge between a selected face and an unselected one is not on the selection.**
   - **What changes.** A cut point is allowed only where the contact lies on the selected faces and not also on the part's faces outside the selection. The tool no longer pivots over the selection's edge: the pass ends where the tool still sits on the selected faces.
   - **The library.** `RegionConstraint::outside` holds the part's faces outside the selection, and `allows` refuses a contact on it while there is a selected region (the region rule's list, `region.rs`). `None` keeps the old rule.
   - **The host.** It gives the triangles the operation's machine selections leave out (on a STEP part, the faces they leave out) whenever there is a selected region.
   - **Where it applies.** Every strategy whose cut points go through the region rule: helical, axial and indexed finishing, simultaneous finishing and rotary roughing.
2. **Tabs and held regions need no rule of their own.** They are not faces of the model, so no selection holds them, and a contact on one has never been on a selection. The roll over the selection's edge was their only way in.
   - A whole-part operation still cuts a tab to size as part of the target (0035 item 6).
   - The held regions stay kept out of by every operation (0161).
   - The tab and flute warnings of 0522 to 0524 do not change.
3. **No check is added after assembly.** No route remains by which a move cuts outside the selection:
   - every cut point obeys the region rule;
   - the moves between allowed points stay within the tolerance, since a pass is split where a point is not allowed;
   - low links stay the approach distance above the stock and never go below either end (0496 item 3), and retracts go to the clearance;
   - entries come down onto a pass's first point, which is allowed.

   A generic check of the moves the fit never sees is open as OI-2054, belt and braces at the cost of drops on the whole part along every link.
4. **No carried-stock clearance.** The lane's planned second pull request (keep clear of the stock earlier operations left on a tab, the owner's earlier choice) is cancelled. Nothing outside the selection is touched now, so the tab is not either.
5. **Generated G-code remains the user's to verify (NFR-12).**

## Verification

Tests first, named after the requirements:

- **`openchuck-path`, new `tests/selection_edge.rs`** (in `tests/integration.rs`; decision 0516). A cylinder joined to a tab, with the V-bit, selecting the side:
  - `req_op_02_a_selection_stops_at_its_edge_with_an_unselected_face`: without `outside` the case rolls over the rim (a cut below X -1). With it, axial and helical stop on the side, at X 0 to 0.5, and no move comes within 0.5 mm of the tab.
  - `req_op_02_faces_outside_the_selection_far_from_it_change_nothing`: a band of the side is byte-identical with and without `outside`.
- **`openchuck-app`, `tests/tab_scope.rs`.** `req_op_02_req_nfr_12_a_v_bit_on_the_side_stays_on_its_selection`: the owner's case on the public cone. It failed before (the tip at radius 3.000). Now no cut passes X 0 and no move comes within 0.5 mm of the tab.
- **The owner's project, checked locally, never committed.** The axial finishing G-code has no fed move at X below 0, and every pass starts at X 0.000 on the rim. Within the tool's reach of the tab nothing comes lower than radius 16.5 (5.000 before). It has 250,054 moves, the same with or without the removed tab rules, and its estimated time falls from 15,829 s to 10,324 s.

## Consequences

- **Library 0.47.0.** `RegionConstraint` gains `outside` (struct literals need `..Default::default()`).
- **Changed output.** Selection-limited paths end where the tool still sits on the selected faces. At a convex edge that is the whole roll. Where the selection's edge runs across a smooth surface it is within the drop's tolerance. Whole-part operations are unchanged.
- **One existing test moved; no golden file did.**
  - **The test.** `openchuck-app` `selection::req_op_11_a_selection_is_left_its_own_stock_in_place_of_the_operations` selects a cube's top face with its own 0.3 mm, with 1 mm on the rest of the part.
  - **Why it moved.** It expected some cuts beside the cube's edges where the ball rode the neighbouring faces' 1 mm, with its contact on the edge between the top and a side. Those contacts are no longer on the selection, so every cut keeps the selection's 0.3 mm, and the test says so.
  - **What it no longer shows.** The rest of the part's whole-number 1 mm; its first half still shows the operation's.
  - **Unchanged.** The whole `openchuck-path` suite and the rest of the `openchuck-app` suite pass unchanged.
- **Open items.** OI-2054 is new (the optional generic check). OI-2047 (free ends ignore the region) stays open: with Face free ends on, a limited operation still faces both ends of the part.
