# 0160: The held region is a cylinder of the holding diameter, otherwise the stock's own section (SET-09; issues #187, #188, #202; lane BUG-HELD)

- **Status:** proposed (lane BUG-HELD, 20 Oct 2026). The project owner gave the rule and agreed the SET-09 wording on 20 Oct 2026, relayed by the coordinator session as owner decision 1 of 2 (decision 0049). The owner accepts the record, and the edit to `docs/requirements.md`, directly in the lane BUG-HELD session. Until then the requirement text is not edited. The code on `claude/lane-bug-held-region` builds the rule. It amends [decision 0045](0045-slice-21c-wiring.md) item 2 and refines [decision 0035](0035-workholding-and-retaining-tabs.md) item 1.
- **Date:** 20 Oct 2026.
- **Open items:** closes OI-195 on acceptance. New: OI-550 and OI-551.

Numbering: the coordinator assigned records 0160 and 0161 and OI-550 to OI-559 to lane BUG-HELD. They were checked free on `main` and every remote branch on 20 Oct 2026. No project schema number: the file does not change.

## Context

Trial feedback F-31, F-32 and F-46 (issues #187, #188 and #202) came from one project. A blisk on box stock (X 0 to 60, Y ±21, Z ±20) is held 5 mm at the start with a holding diameter of 40 mm, the stock's smaller side, and has retaining tabs at the head and the tail.

Decision 0045 item 2 made every held region a cylinder. Its diameter was the holding diameter, or the stock's diameter on round stock. A held region on any other stock needed a holding diameter no wider than the stock's smaller side, and without one the setup inspector showed "box stock has no diameter to hold at" (#188).

`openchuck-app::workholding::labelled_solids` builds that cylinder, and the same solid is used everywhere: the target of every strategy (the composite surface of decision 0042), the simulation's target, the deviation report, the selection notes, the overview and the viewport drawing. So the shape drawn in #187 is the shape that is used. On box stock that shape is at most the inscribed cylinder, so it looks part-sized next to the box.

The box's corners in the gripped length, outside the cylinder, were not part of the target, so every strategy machined them. On the lane's test project (a cone on box stock 24 mm across, held 4 mm at both ends with a 24 mm holding diameter, a 3 mm ball), roughing cut the full ball radius into the gripped length at both ends. The simulation removed material at 2318 of 24300 sample points there, and helical, axial, indexed, simultaneous, rest machining and edge following did the same. The deviation report showed no overcut, because its target was the same cylinder. The tools running over the gripped length are the subject of [decision 0161](0161-tools-keep-clear-of-the-gripped-length.md).

## Decision

1. **The owner's rule.** Where a holding diameter is given, the held region is a cylinder of that diameter about the rotary axis over the gripped length, **on any stock**, box stock included. This gives the user a level of custom holding setup, such as a round grip on box stock. Where no holding diameter is given, the held region is **the stock's own section** over the gripped length:
   - round stock: a cylinder of the stock's diameter;
   - box stock and bounding-box-offset stock: the box of its section;
   - imported stock: the box of its bounding box's section (the owner's choice: no new solid type). This contains the outline, so it can only keep more material.

   One shape is used for the strategies' target, the simulation, the deviation report, the selection notes and the drawing.
2. **Box corners outside a holding diameter are outside the held region, by design.** On the owner's project the held region is the 40 mm cylinder, not the box. The stock outside it in the gripped length is not held region, and the deviation report does not count it as part. Decision 0161 still keeps every cutting move off the whole gripped length, so in practice it is not cut either.
3. **No diameter is needed.** The #188 error goes: with no holding diameter the held region is the stock's own outline. A holding diameter on stock that is not round must still fit the stock's smaller side; a wider one is refused, as before, because the cylinder would stand outside the stock. The holding diameter field's hint says what the held region is without one: "Stock diameter (… mm)" on round stock, "Stock outline" on other stock.
4. **A box solid in `openchuck-path`.** `Solid::cuboid(bounds)` is a box with faces parallel to the model's planes. It answers every query of the solid exactly: closest point and normal by clamping, the ray by the slab method, and the tool drop by the mesh drop-cutter's vertex, edge and facet tests on its twelve triangles (`cutter::drop_on_triangles`). `Solid::as_cuboid` tells a box from a cylinder or frustum. A box also reads as a solid along X: base, top and length as usual, and both radii the radius through its section's corners. The display draws a box's end outlines as rectangles, and the Setup panes draw its height along Z. `Solid` keeps its fields private and stays `Copy`, so this is an addition, not a breaking change.
5. **SET-09 as the owner agreed it** (the requirement text to edit when the owner accepts this record): "Workholding: define, for each setup, how much of the stock the chuck or other workholding grips at the start and at the end of the stock along the axis (the gripped length, the chuck depth). The held region is a cylinder of the holding diameter when one is given, otherwise the stock's own section over the gripped length. The held region is never machined, strategies and links keep clear of it, and it stays in the simulated stock."

## Alternatives considered

The lane proposed three options for a holding diameter on box stock: not used (the outline instead), refused, or kept as a cylinder. The owner chose a fourth rule (item 1): a diameter is honoured as a cylinder on any stock, and its absence means the outline.

- **A jaw width and depth for box stock.** The owner ruled it out: the stock outline over the gripped length is the hold.
- **A prism of an imported stock's section.** This would be exact, but it needs a solid for a general mesh section. The bounding box is safe and enough until imported stock (SET-04, P2) is in use (OI-551).

## Consequences

- Tests in `crates/openchuck-app/tests/held_region.rs`, each failing before the fix:
  - `req_set_09_req_nfr_12_roughing_never_enters_the_held_region_at_either_end`: the owner's settings, with the held region taken as the cylinder of the holding diameter.
  - `req_set_09_box_stock_is_held_at_its_own_outline_over_the_gripped_length` (#188).
  - `req_set_09_the_held_region_drawn_is_the_stocks_section` (#187): box and round stock without a diameter, and a holding diameter on round and on box stock.
- In `crates/openchuck-path/tests/solids.rs`: the box's volume, area, bounds and mesh; a refused impossible box; and its closest point, ray and tool drop against the mesh of its own triangles.
- Unit and view tests in `openchuck-app` that held the old rule ("needs a holding diameter") now hold the owner's rule.
- Round stock gives the same solids as before, so this record does not change its G-code; decision 0161 does.
- Open items:
  - OI-195 closes when the owner accepts.
  - OI-550: the owner checks the fix on a nightly build.
  - OI-551: imported stock is held at its bounding box, not its own section.
