# 0642: End facing reaches at most the flute length below the stock top, and faces a deeper end from both sides (OP-24, NFR-07, NFR-12, GC-07, NFR-06; issue #499; lane FACEAPPROACH)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049), unchanged. Proposed by lane FACEAPPROACH the same day; the rule is the owner's decision F1, option (i), relayed by the coordinator.
- **Date:** 2 Nov 2026.
- **Issue:** [#499](https://github.com/paulhampson/OpenChuckCAM/issues/499).
- **Builds on:** [0252](0252-facing-ends-sides-and-extents.md) (End facing: flank bands, tab views, the union-of-views completion proof and "using only the active flute length"), [0641](0641-facing-approach-written-allowance-and-links.md) (the written-motion allowance in the End bands), [0344](0344-finishing-stepdown-and-free-ends.md) item 7 (how a free end deeper than the flutes is refused), [0495](0495-minimum-clearance-and-clearance-from-stock.md) (OI-1782: stick-out is not modelled).
- **Open items:** closes OI-1997. No project schema change and no library API change.

## Context

On the owner's Baluster set-up, End facing cut a 35 mm deep end from a single A with a tool that has 25 mm of flute. The bands ran from the stock's top at Z 17.5 down to Z −17.5. Nothing limited that depth against the flutes:

- The band spacing was limited to the flute length.
- The shank proof (`facing/motion/stock.rs`) showed that the tool above its flutes moved only through the slot already cut.

That geometry is clean against the stock, but the stick-out and the holder are not modelled (OI-1782), so a real collet can reach the stock top. Decision 0252 asks for "only the active flute length" and a refusal on "inadequate flute/shank/holder access". The owner expects the part to be turned through 180 degrees at the flute limit and the facing finished from the other side.

## Decision (the owner's F1, option i)

1. **The depth of one view.**
   - Each End view reaches at most the tool's flute length below the stock's top at that view.
   - The stock's top is the whole stock's support along the tool's axis at that A, the same measure Box facing starts its layers from.
2. **Two views for a deeper end.**
   - When the end coverage seen from the start angle needs more than the flute length, End facing faces it from the start angle A and from A + 180 degrees (in the machine's rotary sense).
   - Each view covers its side down past the axis line by the written-motion allowance of decision 0641. The completion proof that already certifies tab views (decision 0252) certifies the union of the two.
   - Decision 0252's alternating view order keeps one A turn per axial slice: A, A + 180; then A + 180, A; and so on.
3. **Refusal.**
   - When a view would still need more than the flute length, End facing is refused (NFR-07). That happens when the end is deeper than twice the flutes reach, or when a view around a tab must reach further than the flutes to keep clear of the tab.
   - The message follows decision 0344 item 7. It names the end's X, the depth below the stock's top, the A, the flute length, and each remedy: "End facing: the end at X *x* needs the tool *d* mm below the top of the stock at A *a*, more than the tool's *f* mm of flute, even facing it from A *a0* and A *a1*, so the tool cannot face it. Use a tool with longer flutes, or face the stock down with Box facing first".
   - Around a tab, the message says "from each of the *n* views around the tab".
4. **Tab views** take the same per-view limit.
5. **Nothing says the G-code is safe or verified (NFR-12).** The limit is the flute length. The real stick-out and holder are still not modelled (OI-1782), so the user still checks the tool's reach on the machine.

## Alternatives considered (put to the owner)

- **(ii) Always face from A and A + 180.** This is simpler, but it adds A moves and a second set of bands to a short end that one view reaches.
- **(iii) Keep one view and warn when the depth exceeds the flutes.** This leaves the reach to the user.

## Consequences

- **Output.**
  - An End facing whose end is deeper than the flutes from one view is now faced from two views: on the Baluster set-up, A 0 and A 180.
  - An end deeper than twice the flutes, which was cut before, is now refused.
  - Shallower ends are unchanged from decision 0641. No golden file uses End facing.
- **Code:** `crates/openchuck-path/src/facing/end/routing.rs`.
- **Tests:**
  - `crates/openchuck-path/tests/facing_end_plan.rs`:
    - `req_op_24_an_end_the_flutes_reach_is_faced_from_one_view`
    - `req_op_24_req_nfr_12_an_end_deeper_than_the_flutes_is_faced_from_both_sides`
    - `req_nfr_07_req_op_24_an_end_deeper_than_two_sides_reach_refuses_with_remedies`
    - the test tool's flutes are now 25 mm, so the existing 24 mm ends are still faced from one view
  - `crates/openchuck-app/tests/facing_approach.rs`: `req_op_24_req_nfr_12_baluster_end_is_faced_from_both_sides_within_the_flutes`

## Verification

- On the Baluster set-up, every End facing cut is within 25 mm of the stock's top at its A. The cuts are at A 0 and A 180, and both posts write the program.
- At path level:
  - A 24 mm end with 15 mm flutes is faced from A 0 and A 180, each side reaching past the axis, with one A turn per axial slice, and the completion proof certifies the whole end.
  - With 10 mm flutes the operation refuses, naming the X, the flutes, both A and the remedies.
- Not run on a machine.
