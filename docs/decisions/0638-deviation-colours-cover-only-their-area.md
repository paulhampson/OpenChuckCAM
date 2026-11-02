# 0638: Deviation colours cover only the area each band applies to, not whole triangles (lane SIMVIS, issue #488)

- **Status:** proposed 2 Nov 2026 by lane SIMVIS. The plan (option A) was approved by the owner on 2 Nov 2026, relayed by the coordinator; only the owner accepts this record (decision 0049).
- **Date:** 2 Nov 2026.
- **Lane:** SIMVIS, `claude/lane-simvis-bite`, stacked on `claude/lane-simvis` (#486) until that merges.
- **Issue:** #488 (the owner's request after accepting decision 0633: "the area highlighted covers the whole face and not just the bite. This makes it confusing to the user. Add the ability to highlight the specific area impacted.").
- **Requirements:** SIM-05, SIM-11 (the probe), SIM-15, NFR-04, NFR-06, NFR-12, NFR-15.
- **Amends:** nothing yet; specification 11.5 gains one sentence when built.
- **Numbers:** 0638, assigned by the coordinator as the next free number on 2 Nov 2026. No open item yet (OI-1994 is held for this lane if one is needed). No project schema number; no library-family change.

## Context

The renderer draws each mesh item in one colour, and the deviation bands are sets of whole triangles (decisions 0040, 0238, 0492, 0633). A large triangle therefore takes one colour over its whole area:

- **The cut part** is the dexel stock's mesh. A faced end is made of strips one column wide and as long as the stock is across. A strip that crosses the edge of a bore is coloured "Unreachable" over the solid ring too, as the screenshots of the tube stand-in for #478 showed.
- **The part model** may hold samples of several bands in one large triangle. On a bore made of long strips, each strip takes one colour.

## Measurements

Taken on the owner's carving (kept outside the repository; numbers only) at a 0.1 mm cell, release build, before any change. A triangle was counted as holding more than one band when its own counted samples (the part) or seven test points (the cut part) fell in more than one band.

| Mesh | Triangles | Holding more than one band | Even split to about 2 cells would add |
| --- | --- | --- | --- |
| Cut part at the end | 1,323,834 | 1,210 (1,475 mm²) | about 85,000 pieces (+6%) |
| Part model | 500,738 | 1,522: the bore's long strips | about 159,000 pieces (+32%) |

On the part model the bore's strips hold two bands because the bore is enclosed along most of its length but open within one slab of each end (decision 0633 item 3), where its samples fall in "More than 1 mm left". An even split would add pieces almost all of one colour. Testing the cut part's triangles at seven points took 7.9 s on one thread.

## Decision (planned)

1. **Option A, adaptive** (owner, 2 Nov 2026, relayed; revised to adaptive after the measurements). A triangle whose test points fall in more than one shown band is halved along its longest edge, and each half again while its own test points still disagree, down to about two simulation cells. The pieces then follow the band boundaries, not the areas of the triangles that hold them. Each piece is coloured by its parent's rule at its own centre: on the part, the nearest of the parent's own samples; on the cut part, decision 0633 item 4. A triangle of one band is drawn whole, as now.
2. **The part's triangle numbers do not change.** The pieces are drawn as an extra coloured layer and their parents are left out of the bands, so selection sets, picking and the outlines (decision 0216) are untouched. The probe reads the piece under the pointer. The cut part's own mesh may be replaced by the refined one, since its picker is built from it (decision 0238).
3. **Bounded and deterministic.** The pieces are made off the interface thread in the job that makes the colours, in fixed chunks, the same at any thread count (NFR-06). They are capped; past the cap the pieces stay coarser and a note says so (NFR-04). Nothing is drawn finer than about two cells, the simulation's own resolution (NFR-12).
4. **No renderer change.** Rejected: per-vertex colours or a texture in the shader (a renderer change, and blending smears the band edges and the hatches); dots at the affected samples (they show where, not the extent, and would duplicate the unreached dots of lane VIEW-OVERLAYS, #416).

## As built so far, and what the measurements found

Built test-first (`openchuck-app` `pieces.rs`; `stock_surface.rs` and `stockplay.rs` use it; `pathview.rs` `DeviationBands::pieces`, `value_at`, `band_at`; `openchuck-ui` `viewport.rs` `band_layers` and the probe, `playback_panel.rs` the filter on pieces). On the part model a triangle's own samples, within a cell of a piece, decide whether the piece holds more than one band: seven test points missed a band 0.1 mm wide at the rim of a bore (`req_sim_05_a_band_narrower_than_the_test_points_is_found_by_the_samples`). On the cut part the seven test points decide.

Measured on the owner's carving (release build, numbers only):

| Mesh | Parents | Pieces | Pieces by band | Colouring job |
| --- | --- | --- | --- | --- |
| Part model | 1,522 | 133,422 | 117,698 Unreachable, 15,724 on the surface | 6.1 s |
| Cut part at the end | 1,210 (longest edge: median 18.8 mm, at most 40 mm) | 132,134 | 68,711 on the surface, 39,234 Unreachable, 24,189 not measured | 8.3 s (3.2 s before) |

The bands interleave inside those triangles rather than meeting along one boundary, so the adaptive pieces number about as many as even ones; neither run reached the budget. Why the bore's strips hold samples on the surface is being looked into.

On the tube stand-in, the faced end shows 47.7 mm² of Unreachable over the bore's 153.9 mm² disc (31%): a cap face within the comparison's search (3 mm plus a cell) of the rim takes the rim's value, on the surface, by decision 0633 item 4. Applying item 4's "material behind" whenever the nearest counted sample is more than two cells away (off the part's surface), not only beyond the search, gave 145.1 mm² (94%) in a trial that was not kept; it changes an accepted rule and one test of decision 0238's projection, so it is the owner's decision.

## Verification (to be built, test-first)

`req_sim_05_a_triangle_holding_two_classes_is_coloured_by_each_within_two_cells`, `req_sim_05_a_triangle_of_one_class_is_drawn_whole`, `req_sim_05_adaptive_pieces_follow_the_band_boundary`, `req_nfr_06_the_split_colours_are_the_same_at_any_thread_count`, `req_sim_11_the_probe_on_a_split_triangle_reads_its_piece`, `req_ui_05_selection_sets_and_picking_keep_the_part_s_triangle_numbers`, `req_nfr_04_the_added_triangles_stay_within_their_budget`, and on the tube stand-in through the interface: the violet area on the faced end is about the bore's disc (π·7² ≈ 154 mm²), not whole strips. The adaptive counts and times on the owner's project are measured before the interface half.
