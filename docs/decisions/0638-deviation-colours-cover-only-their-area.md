# 0638: Deviation colours cover only the area each band applies to, not whole triangles (lane SIMVIS, issue #488)

- **Status:** proposed 2 Nov 2026 by lane SIMVIS. The plan (option A) was approved by the owner on 2 Nov 2026, relayed by the coordinator; only the owner accepts this record (decision 0049).
- **Date:** 2 Nov 2026.
- **Lane:** SIMVIS, `claude/lane-simvis-bite`, stacked on `claude/lane-simvis` (#486) until that merges.
- **Issue:** #488 (the owner's request after accepting decision 0633: "the area highlighted covers the whole face and not just the bite. This makes it confusing to the user. Add the ability to highlight the specific area impacted.").
- **Requirements:** SIM-05, SIM-11 (the probe), SIM-15, NFR-04, NFR-06, NFR-12, NFR-15.
- **Amends:** decision 0633 item 4 (accepted), by the owner's answer of 2 Nov 2026, relayed by the coordinator (item 5 below); specification 11.5 and the verification plan's SIM-05 row, to match.
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

1. **Option A, adaptive** (owner, 2 Nov 2026, relayed; revised to adaptive after the measurements). A triangle whose test points fall in more than one shown band is halved along its longest edge, and each half again while its own test points still disagree, down to about two simulation cells; a strip (four or more times longer than it is wide) is cut across its length instead (see the measurements below). The pieces then follow the band boundaries, not the areas of the triangles that hold them. Each piece is coloured by its parent's rule at its own centre: on the part, the nearest of the parent's own samples; on the cut part, decision 0633 item 4. A triangle of one band is drawn whole, as now.
2. **The part's triangle numbers do not change.** The pieces are drawn as an extra coloured layer and their parents are left out of the bands, so selection sets, picking and the outlines (decision 0216) are untouched. The probe reads the piece under the pointer. The cut part's own mesh may be replaced by the refined one, since its picker is built from it (decision 0238).
3. **Bounded and deterministic.** The pieces are made off the interface thread in the job that makes the colours, in fixed chunks, the same at any thread count (NFR-06). They are capped; past the cap the pieces stay coarser and a note says so (NFR-04). Nothing is drawn finer than about two cells, the simulation's own resolution (NFR-12).
4. **No renderer change.** Rejected: per-vertex colours or a texture in the shader (a renderer change, and blending smears the band edges and the hatches); dots at the affected samples (they show where, not the extent, and would duplicate the unreached dots of lane VIEW-OVERLAYS, #416).
5. **A cut-part face off the part's surface takes the material behind it** (amends decision 0633 item 4; the owner answered yes on 2 Nov 2026, relayed by the coordinator). A cut-part face, or a piece of one, more than two cells from every counted sample is off the part's surface. It takes the value and class of the sample of any kind nearest to the point one search distance behind it, into the stock, when that is a counted part sample whose value is a bound or whose material left is enclosed, as item 4 of decision 0633 says for faces beyond the search. Otherwise it keeps the nearest counted sample within the search, and beyond the search it is Not measured, as before. Within two cells of a counted sample the face is on the part's surface and takes that sample, as before. Item 4 alone left a cap over a bore within the search of the opening's rim taking the rim's "nothing left": on the tube stand-in only 31% of the bore's disc showed Unreachable.

## As built so far, and what the measurements found

Built test-first (`openchuck-app` `pieces.rs`; `stock_surface.rs` and `stockplay.rs` use it; `pathview.rs` `DeviationBands::pieces`, `value_at`, `band_at`; `openchuck-ui` `viewport.rs` `band_layers` and the probe, `playback_panel.rs` the filter on pieces). On the part model a triangle's own samples, within a cell of a piece, decide whether the piece holds more than one band: seven test points missed a band 0.1 mm wide at the rim of a bore (`req_sim_05_a_band_narrower_than_the_test_points_is_found_by_the_samples`). On the cut part the seven test points decide.

Measured on the owner's carving (release build, numbers only):

| Mesh | Parents | Pieces | Pieces by band | Colouring job |
| --- | --- | --- | --- | --- |
| Part model | 1,522 | 133,422 | 117,698 Unreachable, 15,724 on the surface | 6.1 s |
| Cut part at the end | 1,210 (longest edge: median 18.8 mm, at most 40 mm) | 132,134 | 68,711 on the surface, 39,234 Unreachable, 24,189 not measured | 8.3 s (3.2 s before) |

Neither run reached the budget. The adaptive pieces number about as many as even ones. A first reading put this down to bands interleaving inside the triangles, but a probe of the part model's mixed triangles found otherwise. Each of the 1,522 holds exactly three runs of one band along X:

- **On the surface at each end of the bore:** the rim, where the faced end cuts the stock flush. These are 10,056 samples: values 0 to 0.05 mm, not enclosed, within about a cell of each end.
- **Unreachable between them:** 1,304,634 samples, enclosed.

Each strip therefore has two real band edges across it.

**The cut part.** With item 5 (above), the cut part at the end had 1,687 parents and 264,850 pieces. The colouring job took 10.1 to 11.1 s, against 8.3 s under item 4 alone and 3.2 s before pieces. The owner asked for the cause to be found before any cost was accepted (2 Nov 2026, relayed).

The cut part's mixed triangles hold no interleaving either. Every one of them is a strip of the faced end, exactly 0.1 mm wide (one dexel column) and up to 40 mm long (median 25.4 mm). Sampled every quarter cell along its length:

| Runs of one band | Strips |
| --- | --- |
| 1 | 212 |
| 2 | 996 |
| 3 | 282 |
| 4 | 145 |
| 5 to 8 | 52 |

Only 72 runs anywhere are shorter than two cells.

**The cause is the cutting rule, not the bands.** Halving a strip along its longest edge leaves one child as long as the strip, so the pieces spread along the whole strip instead of gathering at the band edge. A model of the rule on a strip 25 to 40 mm long, 0.1 mm wide, with one band edge across it, gave 35 to 151 pieces. The part model's bore strips are slivers too.

**The fix (item 1, as built).** A triangle four or more times longer than it is wide is cut across its length. Its length is halved while a stretch holds more than one band, down to two cells or the strip's width.

- **A clean band edge is one cut.** This covers a short mixed stretch, or two neighbouring ones, with one band on each side. At an end of the strip, the band at that end counts as the neighbour beyond it. The strip is cut once, at the middle of that stretch, within two cells of the edge, and each half joins its side. A half that would stand alone at a strip's end is kept only if its pieces read its band at their centres. This keeps a rim narrower than the stretch.
- **Joining.** Neighbouring stretches of one band are joined.
- **Irregular edges.** A stretch still holding more than one band, where the bands tangle or a band is narrower than the stretch, is cut on as a piece.

One band edge across a strip then costs a few well-shaped pieces. The coordinator relayed the target: about five per parent, and at most seven for a strip with two edges.

Measured on the owner's carving (release build, 4 threads, numbers only):

| Mesh | Parents | Pieces, longest-edge halving | Strips cut across | With one cut per clean edge |
| --- | --- | --- | --- | --- |
| Part model | 1,522 | 133,422 | 16,476 | 10,556 (6.9 per parent: 5,412 on the surface, 5,144 Unreachable) |
| Cut part at the end | 1,687 | 264,850 | 17,801 | 8,975 (5.3 per parent) |

**Timings.** The part model's colouring job takes 3.3 to 3.4 s. For the cut part, the job was run four rounds in one process, alternating with and without pieces:

| Cut-part frame at the end | Without pieces | With pieces |
| --- | --- | --- |
| Still (with its picker) | 4.8 to 5.5 s | 6.2 to 6.5 s |
| Playing (no picker) | 1.1 to 1.6 s | 2.5 to 2.9 s |

Pieces add about 1.3 s either way. Making the picker is most of a still frame's cost. The 3.2 s quoted earlier "before pieces" was not like for like. The rest of the pieces' cost is finding the mixed triangles: 73,558 of the cut part's 1,323,834 triangles are longer than two cells and are tested at seven points.

Whether that cost is accepted, or pieces are made only for still frames, is the owner's decision (asked 2 Nov 2026).

On the tube stand-in, the faced end shows 47.7 mm² of Unreachable over the bore's 153.9 mm² disc (31%): a cap face within the comparison's search (3 mm plus a cell) of the rim takes the rim's value, on the surface, by decision 0633 item 4. Applying item 4's "material behind" whenever the nearest counted sample is more than two cells away (off the part's surface), not only beyond the search, gave 145.1 mm² (94%) in a trial that was not kept; it changed an accepted rule, so it went to the owner, who agreed (item 5). As built, the interface test gives the same 145.1 mm².

## Verification (to be built, test-first)

`req_sim_05_a_triangle_holding_two_classes_is_coloured_by_each_within_two_cells`, `req_sim_05_a_triangle_of_one_class_is_drawn_whole`, `req_sim_05_adaptive_pieces_follow_the_band_boundary`, `req_nfr_06_the_split_colours_are_the_same_at_any_thread_count`, `req_sim_11_the_probe_on_a_split_triangle_reads_its_piece`, `req_ui_05_selection_sets_and_picking_keep_the_part_s_triangle_numbers`, `req_nfr_04_the_added_triangles_stay_within_their_budget`, and on the tube stand-in through the interface: the violet area on the faced end is about the bore's disc (π·7² ≈ 154 mm²), not whole strips. The adaptive counts and times on the owner's project are measured before the interface half.

Built so far for item 5 and the strip rule, test first:

- `openchuck-app` `stock_surface.rs` `req_sim_05_a_stock_face_off_the_part_s_surface_takes_the_material_behind_it`. It failed first. A cap five cells from the rim takes the enclosed bore behind it. A cap one and a half cells from the rim keeps the rim. With nothing bound or enclosed behind a cap, it keeps the nearest counted sample.
- Decision 0238's projection test `req_sim_05_req_sim_11_stock_colours_and_probes_use_nearest_existing_samples_with_bounds` now places its triangles within two cells of the samples they read. At 0.2 mm (two cells, and a little more at the centroid) the bound behind the triangle was taken, as item 5 intends.
- `openchuck-ui` `shell::simulate_tests::req_sim_05_the_faced_end_over_a_bore_is_violet_only_over_the_bore` (11.5 s), through the whole interface on the tube stand-in. It shows 145.1 mm² Unreachable on the faced end against the bore's 153.9 mm², within 20%. Before item 5 it showed 47.7 mm².
- `openchuck-app` `pieces.rs` `req_nfr_04_req_sim_05_a_long_strip_is_cut_across_its_length`. It failed first, with 124 pieces. A strip 25 mm by 0.1 mm (two triangles) with one band edge across it makes at most 20 pieces, with the edge placed to within two cells. With two band edges, as across a bore, it makes at most 14 (tightened with the next test).
- `pieces.rs` `req_nfr_04_req_sim_05_a_clean_band_edge_across_a_strip_is_one_cut`. It failed first, with 22 pieces for the two-triangle strip. A one-triangle strip with two clean edges makes at most 7 pieces, with each band's area within two cells of the true one.
- The bore test `req_sim_05_the_part_s_bore_is_drawn_in_pieces_where_its_strips_hold_two_bands` now finds the piece across the middle of the bore. The joined pieces are long, so no piece centre lies near the middle.
