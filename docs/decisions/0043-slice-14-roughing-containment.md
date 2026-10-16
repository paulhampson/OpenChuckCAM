# 0043: Slice 14 item 5: containment of rotary roughing to selected regions (lane L)

- **Status:** accepted by the project owner (15 Oct 2026) for the attribution rule (item 1), the check-surface rule (item 3) and bisection of the region's edge (item 4), "as is for now": the rule is kept while the edge behaviour on slopes (the evidence below) is watched. The rest follows from these choices and stands until the pull request is reviewed. It relies on accepted records [0032](0032-slice-10-mesh-selection.md) (the region constraint, OP-02) and [0033](0033-slice-14-rotary-roughing.md) (rotary roughing).
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-187 and OI-188 were raised and closed in this record. OI-137 (the boundary as a curve) is unchanged.

Numbering: record 0043 (0040 went to lane N, 0041 to lane U and 0042 to lane V; first written as 0040), `OI-187` and `OI-188` (first OI-170 and OI-171, then OI-175 and OI-176, renumbered as lanes M and N took those) and no project schema number (no file changes: the operation's `boundary`, `avoid`, `check` and `selections` fields are schema 7's).

## Context

Work breakdown slice 14 item 5, "Containment to selected regions (OP-02)", is lane L. Lane C built the region constraint and helical finishing honours it (decision 0032); lane E built rotary roughing without it (decision 0033), so an operation with selections on `rotary_roughing` was refused. Decision 0032 item 5 foresaw the question this record answers: avoid and check surfaces differ in effect "for strategies that cut away from the surface (roughing, slice 14)". Requirement OP-02; verification plan row OP-01, OP-02.

## Decision (proposed)

1. **A cut site is attributed to the part under the tool.** Roughing does not ride the surface, so a region cannot be tested at the tool tip. Each site (X, A and the offset across the axis) is attributed to the cutter contact of the tool dropped on the **whole** part with its stock to leave grown on, which does not depend on the layer. The site is cut only where `RegionConstraint::allows` accepts that contact, the same test helical finishing uses: on the selected region, inside the boundary, not on an avoid surface. A site over no part (beyond the part's ends) is excluded by a selected region and otherwise decided by the boundary window at the stock radius. Because the contact does not change with depth, every layer cuts the same sites, and no cut is deeper than a stepdown.
2. **The tool still sees the whole part.** Layers are planned from the floor over the whole part, so material above the selection is removed to the stock to leave without the tool touching the unselected geometry beside it. Material above sites that are not allowed is left as it stands.
3. **Check surface (agreed).** A check surface raises the floor and does not exclude the site: the tool is dropped on the check surfaces alone, and the site's floor is the higher of that and the part's, so material above a check surface is still removed with the tool kept clear of it (by the stock to leave). It is no longer treated like an avoid surface. Only the selection, boundary and avoid surfaces decide whether a site is cut.
4. **Where a pass is cut, and its edges (agreed).** A stretch of path between two cut nodes is kept only if every finer node between them is allowed; otherwise the nodes are cut as separate passes and the shared services link them (OP-14). So nothing is cut on an avoid surface or outside the boundary. Where a run meets the edge of the allowed region within a coarse step, it is carried on to the edge, found by bisection (twelve halvings, as in helical finishing) between the last allowed fine node and the first that is not, so the cut reaches the edge to a small fraction of a fine step. The edge does not depend on the layer, so it is found once per line of nodes, not per layer.
5. **Honouring the region.** `RotaryRoughing::honours_region` is true and `passes_in_region` is implemented. An empty constraint gives the same passes byte for byte. A constraint that leaves nothing to cut is an error naming the selection. Stock to leave per selection (`honours_leave_zones`) stays unsupported for roughing and is refused with the existing message.
6. **Where the code is.** `openchuck-path` `roughing` only (architecture 5.2); `openchuck-app` needs no change, as it asks the strategy. No new dependency, no public struct changed, no format change.

## Alternatives considered

- **Test the region at each tool tip.** The tip is above the surface at all but the last layer, so it is never "on" the region.
- **Region offsets (decision 0034) for the edge.** Far more work for bulk removal than bisection on the allowed test, which gives the same result along the path.
- **Cut sites near an edge by their distance to the region (a margin).** Needs a definition of "near" for a tool of width; bisection on the same test is exact and needs none.

## Evidence

- `crates/openchuck-path/tests/roughing_region.rs`: `req_op_02_*`: the strategy honours regions; an empty constraint changes nothing (both forms); cuts stay on a selected region; an avoid surface is never cut (both forms); a boundary limits X; the nearest point of a pass is on the edge of the region to 0.01 mm (both forms); a check surface keeps the tool clear of it while material over it is still removed and the part beside it is roughed; an unreachable selection is an error; `req_nfr_06_*` identical at 1, 2 and N threads.
- `crates/openchuck-app/tests/selection.rs`: `req_op_02_rotary_roughing_removes_stock_only_above_its_selected_region`: a project operation with a selected top face cuts fewer sites, all within the face's angular reach, and regenerates identically.
- `crates/openchuck-sim/tests/roughing.rs`: `req_op_02_roughing_a_cone_within_a_region_*`: a cone (8 to 12 mm radius, 4 mm ball, 0.5 mm to leave) with the region starting at x = 15 mm, simulated on the dexel stock. Inside the region the leftover is 0.44 to 0.61 mm (uniform); outside, the whole stock stands (3.0 mm); nothing is overcut at the edge. Because the edge falls on a slope and the ball's contact decides, the leftover tapers across it: the stock to leave from about 1.5 mm inside the edge, 0.61 mm at 0.6 mm before it, and the whole stock about 3 mm before it. That taper is the extra stock the rule leaves beside an edge on a slope; finishing takes it.

## Open items

- **OI-187 (closed, 15 Oct 2026):** the region's edge was found only to the node spacing. Item 4 finds it by bisection.
- **OI-188 (closed, 15 Oct 2026):** a check surface left a site uncut where the part's floor rested on it. Item 3 raises the floor instead.
